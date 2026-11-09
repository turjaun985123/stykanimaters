//! Project library backend: scan, load, save, rename, duplicate,
//! filter and thumbnail the named projects stored as serde JSON
//! under `./projects/`.
//!
//! The file convention matches the editor's named save
//! (`editor.rs::save_named_project`): one `<slug>.json` per project,
//! where the slug is the project name lowercased with every
//! non-alphanumeric character replaced by `-`.
//!
//! Two safety rules run through the module:
//!
//! - A name that cannot identify a file — blank, or with no
//!   alphanumeric character at all, so its slug would be nothing
//!   but dashes — is refused by every write path ([`file_stem_for`]).
//!   The slug also *is* the path-safety rule: it maps every
//!   non-alphanumeric character — path separators and `.`
//!   included — to `-`, so no name can produce a file outside
//!   the library directory or a `..` component.
//! - The operations that must never overwrite ([`save_copy`],
//!   [`duplicate_project`], [`rename_project`]) claim their target
//!   atomically (`create_new`, or a hard-link claim for the
//!   rename), so the refusal and the write cannot be split by a
//!   file appearing in between. And [`save`], which overwrites
//!   by design, replaces atomically (temp file + rename, the
//!   shell recovery writer's pattern — see [`write_file_atomic`]):
//!   a crash mid-save leaves the old complete file or the new
//!   one, never a torn one where a good project stood.
//!
//! [`scan`] caches each file's parsed summary keyed by the
//! file's (mtime, length) stamp, so rescanning a directory of
//! unchanged projects costs one stat per file instead of a
//! full read + JSON parse per file; a file whose stamp moves
//! is re-read and re-parsed on the next scan.
//!
//! [`refresh_project_files`] is the seam every save path
//! calls after writing a project file, so the editor core's
//! cached selector list can never miss a file a save just
//! created (Wave 12; the Wave 11 §6.6 remainder).

use crate::editor::EditorCore;
use crate::export::render_frame_at;
use crate::model::Project;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Monotonic counter making each atomic write's temp name
/// unique within the process (combined with the process id,
/// it is unique across processes too).
static WRITE_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Replace `path` with `text` atomically: the bytes go to a
/// temp file in the same directory first, which is then
/// renamed over `path`, so a reader only ever sees the old
/// complete contents or the new ones — a crash mid-write
/// cannot leave a half-written file where a good one stood.
/// The temp name is dot-prefixed (hidden) and does not end
/// in `.json`, so [`scan`] never lists it. The same pattern
/// as the shell's recovery writer (`shell::write_file_atomic`,
/// private to that module); deliberately no `fsync` — this
/// buys untorn reads, not power-loss durability.
pub(crate) fn write_file_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("write");
    let seq = WRITE_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{seq}",
        std::process::id()
    ));
    let written = std::fs::File::create(&tmp).and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Summary of one saved project file, for library/home listings.
#[derive(Clone, Debug)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    pub figures: usize,
    pub frames: usize,
    pub fps: f32,
    pub modified: Option<SystemTime>,
}

/// The file-name slug for a project name (same rule as the
/// editor's named save).
fn slug(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
}

/// The file stem a project name maps to, or `None` when the
/// name cannot name a library file: it is blank (empty or
/// whitespace-only), or it contains no alphanumeric character
/// at all — its slug would be nothing but dashes, carrying no
/// identity and colliding with every other such name. Write
/// paths refuse those names instead of planting files like
/// `-.json` or `---.json` (or, for an empty name, the hidden
/// `.json`). `pub(crate)` since Wave 10: the editor's named
/// save applies the same rule.
pub(crate) fn file_stem_for(name: &str) -> Option<String> {
    if name.trim().is_empty() || !name.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    Some(slug(name))
}

/// The identity of a file's contents for cache purposes: the
/// (modified time, length) pair from one stat. Two stamps are
/// equal only when both halves match, so an in-place rewrite
/// is seen whether it moved the mtime, changed the length,
/// or (as usual) both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FileStamp {
    fn of(metadata: &std::fs::Metadata) -> Self {
        FileStamp {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        }
    }
}

/// The part of a parsed project a listing needs — everything
/// in [`ProjectInfo`] except the path and the stamp, which a
/// scan always knows first-hand.
#[derive(Clone, Debug, PartialEq)]
struct ScanSummary {
    name: String,
    figures: usize,
    frames: usize,
    fps: f32,
}

/// One file's cached scan outcome: the stamp it was computed
/// under, and the summary — `None` for a file that could not
/// be read or parsed at that stamp, so a broken file is not
/// re-read on every scan either (it is retried as soon as its
/// stamp moves).
#[derive(Clone, Debug)]
struct CachedScan {
    stamp: FileStamp,
    summary: Option<ScanSummary>,
}

/// Look up `path` in the scan cache under `stamp`. The outer
/// `None` is a miss (no entry, or the entry's stamp differs —
/// the file changed and must be re-read); `Some(None)` is a
/// cached unparseable file; `Some(Some(summary))` is a hit.
fn cache_get(
    cache: &HashMap<PathBuf, CachedScan>,
    path: &Path,
    stamp: FileStamp,
) -> Option<Option<ScanSummary>> {
    let entry = cache.get(path)?;
    if entry.stamp != stamp {
        return None;
    }
    Some(entry.summary.clone())
}

/// Record `path`'s scan outcome under `stamp`, replacing any
/// older-stamp entry for the file.
fn cache_put(
    cache: &mut HashMap<PathBuf, CachedScan>,
    path: PathBuf,
    stamp: FileStamp,
    summary: Option<ScanSummary>,
) {
    cache.insert(path, CachedScan { stamp, summary });
}

/// Drop the cached entries for files directly under `dir`
/// that the latest scan did not see, so a deleted or renamed
/// file's entry cannot linger (or be revived if a different
/// file later reuses the path with a colliding stamp).
/// Entries for other directories are untouched.
fn cache_prune_dir(cache: &mut HashMap<PathBuf, CachedScan>, dir: &Path, seen: &[PathBuf]) {
    cache.retain(|path, _| !path.starts_with(dir) || seen.iter().any(|p| p == path));
}

thread_local! {
    /// Per-file scan summaries, keyed by path and validated
    /// by [`FileStamp`] — the shell's `thread_local` pattern
    /// for process state (as in `stage_view` / `audio_play`).
    /// Session-only: never persisted, and correctness never
    /// depends on it — a miss simply re-reads the file.
    static SCAN_CACHE: RefCell<HashMap<PathBuf, CachedScan>> =
        RefCell::new(HashMap::new());
}

/// List every parseable project in `dir`, newest modified first.
/// Files that cannot be read or parsed as a [`Project`] are
/// skipped silently; a missing directory yields an empty list.
///
/// Cost: one directory listing plus one stat per `.json`
/// file. A file whose (mtime, length) stamp matches its
/// cached summary is not re-read or re-parsed; only new or
/// changed files pay the read + parse.
pub fn scan(dir: &Path) -> Vec<ProjectInfo> {
    let mut infos: Vec<ProjectInfo> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return infos;
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|x| x != "json").unwrap_or(true) {
            continue;
        }
        seen.push(path.clone());
        // A file whose metadata cannot be read is never
        // cached: it is re-read on every scan and listed
        // with `modified: None`, exactly as before.
        let stamp = entry.metadata().ok().map(|m| FileStamp::of(&m));
        let cached = stamp.and_then(|st| SCAN_CACHE.with(|c| cache_get(&c.borrow(), &path, st)));
        let summary = match cached {
            Some(summary) => summary,
            None => {
                let parsed = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Project>(&text).ok())
                    .map(|project| ScanSummary {
                        name: project.name,
                        figures: project.figures.len(),
                        frames: project.frames.len(),
                        fps: project.fps,
                    });
                if let Some(st) = stamp {
                    SCAN_CACHE.with(|c| {
                        cache_put(&mut c.borrow_mut(), path.clone(), st, parsed.clone())
                    });
                }
                parsed
            }
        };
        if let Some(summary) = summary {
            infos.push(ProjectInfo {
                name: summary.name,
                figures: summary.figures,
                frames: summary.frames,
                fps: summary.fps,
                path,
                modified: stamp.and_then(|st| st.modified),
            });
        }
    }
    SCAN_CACHE.with(|c| cache_prune_dir(&mut c.borrow_mut(), dir, &seen));
    infos.sort_by(|a, b| b.modified.cmp(&a.modified));
    infos
}

/// List the project files directly under `dir`, sorted by
/// path — the selector's view of the library: every `.json`
/// file except the shell's own (its layout, recovery and
/// recents files, and dotfiles generally, are not openable
/// projects — the stray ".layout" row, Phase F). Unlike
/// [`scan`], parseability is NOT required and nothing is
/// parsed: the selector names files, and a corrupt project
/// file is still a file the user may want to see named.
/// A missing directory yields an empty list.
/// `EditorCore`'s private copy of this listing predates
/// it; the Wave 12 integration patch delegates that copy
/// here so the two cannot drift.
pub fn list_project_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                // The shell keeps its own files in this
                // directory (layout prefs, recovery snapshot,
                // recents); they are not openable projects
                // and must never surface in the selector.
                .filter(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    !name.starts_with('.')
                        && name != crate::shell::LAYOUT_FILE
                        && name != crate::shell::RECOVERY_FILE
                        && name != crate::shell::RECENTS_FILE
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Rescan `./projects/` into the core's cached selector list
/// (`EditorCore::project_files` / `project_index`) — THE
/// refresh seam every save path calls after it writes a
/// project file (Wave 12, closing the Wave 11 §6.6
/// remainder: an MCP save that created a new file left the
/// cache stale, so the selector and Home's current-project
/// marking missed the file until something else happened to
/// rescan). The GUI's named save always refreshed inline
/// (inside `EditorCore::save_named_project_to`); the MCP
/// save / save-a-copy / duplicate / rename paths could not
/// reach that refresh and now call this instead.
///
/// The seam is list + selection only, deliberately: recents
/// and the recovery snapshot stay with the save call sites,
/// exactly as the GUI splits them (the core op refreshes the
/// list; the press handler's `after_save` follow-up writes
/// recents and recovery). A copy or a duplicate is not the
/// session's current file and must not enter recents; the
/// named-save paths, GUI and MCP alike, already touch
/// recents themselves.
///
/// Wave 13 (B5, the dirty-marking census): the seam also
/// never touches the edit counters (`edit_count` /
/// `last_autosave_count`, read via
/// `EditorCore::edits_since_autosave`) — deliberately.
/// It runs immediately AFTER saves, where the saved
/// baseline must survive untouched, and inside the MCP
/// `project_rename` handler, whose own name write
/// (`core.project.name = …` in `mcp_tools.rs`) is a
/// project mutation that must mark dirty at ITS call
/// site, not here: a refresh that bumped or reset the
/// counters would dirty every just-saved project and
/// launder every rename. The same audit cleared this
/// file's other write paths: [`rename_project`] and
/// [`duplicate_project`] are file primitives with no
/// core at all, and [`save_copy`] renames a clone — the
/// live project's name, file association and counters
/// are untouched by design. The counters are pinned by
/// `refresh_project_files_never_touches_the_edit_counters`
/// and `save_copy_leaves_the_sessions_dirty_state_untouched`
/// in this file's tests.
pub fn refresh_project_files(core: &mut EditorCore) {
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("projects");
    refresh_project_files_in(core, &dir);
}

/// [`refresh_project_files`] against an explicit projects
/// directory (the codebase's split, as in [`save_copy_to`],
/// so tests can aim the refresh at a temp dir).
///
/// The selection is preserved BY FILE IDENTITY: the path the
/// index names before the refresh is the path it names
/// after, wherever the rescan moved it — a new file sorting
/// before the current one must not silently retarget the
/// selector onto a different project. If the current file
/// itself vanished (a rename's old path, say), the index
/// clamps onto the last live entry — the fallback
/// `save_named_project_to`'s inline refresh already used.
/// A missing directory is a no-op rather than an empty
/// list: a transient working-directory problem must not
/// empty the selector.
fn refresh_project_files_in(core: &mut EditorCore, dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let current = core.project_files.get(core.project_index).cloned();
    core.project_files = list_project_files(dir);
    let followed =
        current.and_then(|path| core.project_files.iter().position(|p| p == &path));
    if let Some(pos) = followed {
        core.project_index = pos;
    } else if core.project_index >= core.project_files.len() {
        core.project_index = core.project_files.len().saturating_sub(1);
    }
}

/// Load a project from a JSON file.
pub fn load(path: &Path) -> std::io::Result<Project> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(std::io::Error::other)
}

/// Save `project` into `dir` under its name slug (creating the
/// directory if needed) and return the written path. Saving
/// overwrites the project's own file by design; a project
/// whose name cannot name a file ([`file_stem_for`]) is
/// refused with an `InvalidInput` error instead of writing a
/// hidden `.json`. The replace itself is atomic
/// ([`write_file_atomic`]): a crash mid-save leaves the old
/// complete file or the new one, never a torn one.
pub fn save(project: &Project, dir: &Path) -> std::io::Result<PathBuf> {
    let Some(stem) = file_stem_for(&project.name) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project name produces an empty file name",
        ));
    };
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{stem}.json"));
    let text = serde_json::to_string_pretty(project).map_err(std::io::Error::other)?;
    write_file_atomic(&path, &text)?;
    Ok(path)
}

/// Save a copy of the editor's current project into
/// `./projects/` under `new_name`, and return the written
/// path. This is the primitive a later "Save a copy as…" UI
/// action wires to: the file is named by the same [`slug`]
/// rule as every other library path, and the JSON stored in
/// it carries `new_name` as its project name, so a later
/// [`scan`] lists the copy under its new name.
///
/// The core is only read: its own project name, current
/// file and autosave/recovery association are all untouched
/// (the project is cloned and the clone renamed). Unlike
/// [`save`], an existing file at the target is never
/// overwritten — the call fails with an "already exists"
/// error, mirroring [`rename_project`], and writes nothing.
pub fn save_copy(core: &EditorCore, new_name: &str) -> Result<PathBuf, String> {
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("projects");
    save_copy_to(core, new_name, &dir)
}

/// `save_copy` against an explicit projects directory (the
/// codebase's split, as in `editor.rs::save_named_project`,
/// so tests can aim the write at a temp dir).
fn save_copy_to(core: &EditorCore, new_name: &str, dir: &Path) -> Result<PathBuf, String> {
    let Some(stem) = file_stem_for(new_name) else {
        return Err("project name produces an empty file name".to_string());
    };
    let path = dir.join(format!("{stem}.json"));
    let mut copy = core.project.clone();
    copy.name = new_name.to_string();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&copy).map_err(|e| e.to_string())?;
    // The target is claimed with `create_new`: the refusal
    // and the write are one atomic step, so a file that
    // already exists — or appears at the target while this
    // call runs — is never silently overwritten (a plain
    // exists-check followed by `fs::write` would truncate
    // whatever won that race).
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!("project file already exists: {}", path.display())
            } else {
                e.to_string()
            }
        })?;
    // If the write itself fails, remove the file this call
    // claimed: a half-written project must not be left
    // squatting on the name, refusing every later save of it
    // while never parsing. The claim was this call's alone
    // (`create_new`), so removing it restores the exact
    // pre-call state.
    if let Err(e) = file.write_all(text.as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(e.to_string());
    }
    Ok(path)
}

/// Rename the project file at `path` to the file for `new_name`,
/// following the library naming scheme (`<slug>.json`, see
/// [`save`]) in the same directory, and return the new path.
/// Renaming onto a *different* existing file is refused with an
/// `AlreadyExists` error and leaves both files untouched;
/// renaming to the name the file already carries is a no-op
/// success. This renames the file on disk only — the `name`
/// stored inside the JSON is unchanged, so a later [`scan`]
/// still reports the stored name until the project is re-saved.
///
/// The no-overwrite refusal is atomic, not an exists-check
/// followed by a rename: a Unix `rename` silently replaces an
/// existing destination, so that pair could be split by a file
/// appearing in between. Instead the target is claimed with a
/// hard link (which fails with `AlreadyExists` if anything is
/// there — same directory, so always the same filesystem) and
/// the old directory entry is then removed; if that removal
/// fails, the claim is rolled back and the error reported.
pub fn rename_project(path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    let Some(stem) = file_stem_for(new_name) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project name produces an empty file name",
        ));
    };
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let new_path = dir.join(format!("{stem}.json"));
    if new_path == path.to_path_buf() {
        return Ok(new_path);
    }
    std::fs::hard_link(path, &new_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("project file already exists: {}", new_path.display()),
            )
        } else {
            e
        }
    })?;
    if let Err(e) = std::fs::remove_file(path) {
        // Roll the claim back: the rename did not happen, so
        // the new entry must not survive it.
        let _ = std::fs::remove_file(&new_path);
        return Err(e);
    }
    Ok(new_path)
}

/// Copy the project file at `path` to a sibling file named
/// `<stem>-copy.json` and return the copy's path. If that name is
/// taken, the suffix increments: `<stem>-copy-2.json`,
/// `<stem>-copy-3.json`, and so on. Each candidate is claimed
/// with `create_new`, so a file that already exists (or appears
/// while the loop runs) is never overwritten — the loop simply
/// moves to the next suffix. The copy's bytes are identical
/// to the source's; the source is left in place.
pub fn duplicate_project(path: &Path) -> std::io::Result<PathBuf> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "project path has no usable file stem",
            )
        })?;
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let mut source = std::fs::File::open(path)?;
    let mut n = 1u32;
    loop {
        let file_name = if n == 1 {
            format!("{stem}-copy.json")
        } else {
            format!("{stem}-copy-{n}.json")
        };
        let candidate = dir.join(file_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut dest) => {
                // A failed copy removes the file this call
                // claimed, like `save_copy_to`: no
                // half-written duplicate left behind (the
                // source is never touched either way).
                if let Err(e) = std::io::copy(&mut source, &mut dest) {
                    drop(dest);
                    let _ = std::fs::remove_file(&candidate);
                    return Err(e);
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                n += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// The subset of `projects` whose name contains `query` as a
/// case-insensitive substring, in the original order. An empty
/// query returns every project (cloned).
pub fn filter_projects(projects: &[ProjectInfo], query: &str) -> Vec<ProjectInfo> {
    let needle = query.to_lowercase();
    projects
        .iter()
        .filter(|info| needle.is_empty() || info.name.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Render `frame` of the project and save it as a PNG thumbnail,
/// creating parent directories as needed.
pub fn thumbnail(project: &Project, frame: usize, out: &Path) -> std::io::Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let img = render_frame_at(project, frame as f32);
    img.save(out).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temp directory per test so parallel test runs and
    /// other agents' tests cannot collide.
    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-projects-test-{}-{test}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_scan_lists_both_newest_first() {
        let dir = temp_dir("scan");
        let demo = Project::new_demo(); // "Untitled", 1 figure, 5 frames, 12 fps
        let first = save(&demo, &dir).unwrap();
        assert_eq!(first, dir.join("untitled.json"));
        assert!(first.exists());

        // Give the second save a strictly later mtime.
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut other = Project::new_demo();
        other.name = "My Cool Project!".into();
        let second = save(&other, &dir).unwrap();
        assert_eq!(second, dir.join("my-cool-project-.json"));
        assert!(second.exists());

        // Unparseable and non-JSON files are skipped silently.
        std::fs::write(dir.join("broken.json"), "{ not a project").unwrap();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();

        let infos = scan(&dir);
        assert_eq!(infos.len(), 2, "scan found {infos:?}");
        // Newest modified first.
        assert_eq!(infos[0].name, "My Cool Project!");
        assert_eq!(infos[0].path, second);
        assert_eq!(infos[1].name, "Untitled");
        assert_eq!(infos[1].path, first);
        for info in &infos {
            assert_eq!(info.figures, 1);
            assert_eq!(info.frames, 5);
            assert!((info.fps - 12.0).abs() < f32::EPSILON);
            assert!(info.modified.is_some());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_round_trips_a_saved_project() {
        let dir = temp_dir("load");
        let mut project = Project::new_demo();
        project.name = "Round Trip".into();
        let path = save(&project, &dir).unwrap();

        let back = load(&path).unwrap();
        assert_eq!(back.name, "Round Trip");
        assert!((back.fps - project.fps).abs() < f32::EPSILON);
        assert_eq!(back.stage, project.stage);
        assert_eq!(back.figures.len(), project.figures.len());
        assert_eq!(back.frames.len(), project.frames.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn thumbnail_writes_a_stage_sized_png() {
        let dir = temp_dir("thumb");
        let project = Project::new_demo();
        // Nested output path also exercises parent creation.
        let out = dir.join("thumbs").join("frame0.png");
        thumbnail(&project, 0, &out).unwrap();
        assert!(out.exists());

        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG file");
        let img = image::open(&out).unwrap().to_rgb8();
        assert_eq!(img.width(), project.stage[0] as u32);
        assert_eq!(img.height(), project.stage[1] as u32);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_moves_the_file_and_refuses_to_overwrite() {
        let dir = temp_dir("rename");
        let mut project = Project::new_demo();
        project.name = "Old Name".into();
        let original = save(&project, &dir).unwrap();
        assert_eq!(original, dir.join("old-name.json"));
        let bytes_before = std::fs::read(&original).unwrap();

        // The file moves to the new name's slug, bytes untouched.
        let renamed = rename_project(&original, "New Name").unwrap();
        assert_eq!(renamed, dir.join("new-name.json"));
        assert!(renamed.exists());
        assert!(!original.exists());
        assert_eq!(std::fs::read(&renamed).unwrap(), bytes_before);

        // Renaming to the name it already has is a no-op success.
        let same = rename_project(&renamed, "New Name").unwrap();
        assert_eq!(same, renamed);
        assert!(same.exists());

        // Renaming onto a different existing file is refused and
        // leaves both files exactly as they were.
        let mut other = Project::new_demo();
        other.name = "Taken".into();
        let taken = save(&other, &dir).unwrap();
        let taken_bytes = std::fs::read(&taken).unwrap();
        let err = rename_project(&renamed, "Taken").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(renamed.exists());
        assert_eq!(std::fs::read(&renamed).unwrap(), bytes_before);
        assert_eq!(std::fs::read(&taken).unwrap(), taken_bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_copies_bytes_and_increments_on_collision() {
        let dir = temp_dir("duplicate");
        let mut project = Project::new_demo();
        project.name = "Copy Me".into();
        let original = save(&project, &dir).unwrap();
        assert_eq!(original, dir.join("copy-me.json"));
        let bytes = std::fs::read(&original).unwrap();

        let first = duplicate_project(&original).unwrap();
        assert_eq!(first, dir.join("copy-me-copy.json"));
        assert_eq!(std::fs::read(&first).unwrap(), bytes);

        let second = duplicate_project(&original).unwrap();
        assert_eq!(second, dir.join("copy-me-copy-2.json"));
        assert_eq!(std::fs::read(&second).unwrap(), bytes);

        let third = duplicate_project(&original).unwrap();
        assert_eq!(third, dir.join("copy-me-copy-3.json"));
        assert_eq!(std::fs::read(&third).unwrap(), bytes);

        // The source is left in place throughout.
        assert!(original.exists());
        assert_eq!(std::fs::read(&original).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn filter_matches_case_insensitively_and_preserves_order() {
        let info = |name: &str| ProjectInfo {
            name: name.into(),
            path: PathBuf::from(format!("{name}.json")),
            figures: 0,
            frames: 0,
            fps: 12.0,
            modified: None,
        };
        let projects = vec![
            info("Alpha Run"),
            info("beta"),
            info("ALPHA Bet"),
            info("Gamma"),
        ];
        fn names<'a>(list: &'a [ProjectInfo]) -> Vec<&'a str> {
            list.iter().map(|p| p.name.as_str()).collect()
        }

        // Empty query returns everything, in order.
        assert_eq!(
            names(&filter_projects(&projects, "")),
            ["Alpha Run", "beta", "ALPHA Bet", "Gamma"]
        );
        // Case does not matter, on either side.
        assert_eq!(
            names(&filter_projects(&projects, "alpha")),
            ["Alpha Run", "ALPHA Bet"]
        );
        assert_eq!(
            names(&filter_projects(&projects, "ALPHA")),
            ["Alpha Run", "ALPHA Bet"]
        );
        assert_eq!(
            names(&filter_projects(&projects, "Bet")),
            ["beta", "ALPHA Bet"]
        );
        // No match yields an empty list.
        assert!(filter_projects(&projects, "zzz").is_empty());
    }

    #[test]
    fn save_copy_writes_current_state_under_the_new_name() {
        let dir = temp_dir("save-copy").join("projects");
        let core = EditorCore::new();
        assert_eq!(core.project.name, "Untitled");

        // The file lands under the NEW name's slug — the same
        // rule `save` applies ("My Copy!" -> "my-copy-.json") —
        // in a directory the call creates, as `./projects`
        // creation works in production.
        let path = save_copy_to(&core, "My Copy!", &dir).unwrap();
        assert_eq!(path, dir.join("my-copy-.json"));
        assert!(path.exists());

        // The stored project is the current state, renamed:
        // it scans and loads under the new name with the
        // core's figures, frames, fps and stage intact.
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "My Copy!");
        let back = load(&path).unwrap();
        assert_eq!(back.name, "My Copy!");
        assert_eq!(back.figures.len(), core.project.figures.len());
        assert_eq!(back.frames.len(), core.project.frames.len());
        assert!((back.fps - core.project.fps).abs() < f32::EPSILON);
        assert_eq!(back.stage, core.project.stage);

        // The core itself is untouched: still "Untitled".
        assert_eq!(core.project.name, "Untitled");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn save_copy_refuses_to_overwrite_an_existing_file() {
        let dir = temp_dir("save-copy-collision");
        let core = EditorCore::new();

        // A pre-existing file at the target is left exactly
        // as it was, and the error says so honestly.
        let target = dir.join("taken.json");
        std::fs::write(&target, b"sentinel").unwrap();
        let err = save_copy_to(&core, "Taken", &dir).unwrap_err();
        assert!(err.contains("already exists"), "unexpected error: {err}");
        assert_eq!(std::fs::read(&target).unwrap(), b"sentinel");

        // Saving the same copy twice collides the same way:
        // the first write survives the second call untouched.
        let path = save_copy_to(&core, "Once Only", &dir).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let err = save_copy_to(&core, "Once Only", &dir).unwrap_err();
        assert!(err.contains("already exists"), "unexpected error: {err}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_copy_rejects_a_name_with_an_empty_slug() {
        let dir = temp_dir("save-copy-empty").join("projects");
        let core = EditorCore::new();
        let err = save_copy_to(&core, "", &dir).unwrap_err();
        assert!(err.contains("empty file name"), "unexpected error: {err}");
        // The refusal happens before anything is created.
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn file_stem_for_rejects_names_that_cannot_name_a_file() {
        // Blank names: empty, and whitespace-only (whose slug
        // would be all dashes).
        assert_eq!(file_stem_for(""), None);
        assert_eq!(file_stem_for("  \t "), None);
        // Names with no alphanumeric character at all: the
        // slug carries no identity and collides with every
        // other such name.
        assert_eq!(file_stem_for("!!!"), None);
        assert_eq!(file_stem_for("☕"), None);
        // Ordinary names slugify exactly as before.
        assert_eq!(file_stem_for("Untitled").as_deref(), Some("untitled"));
        assert_eq!(file_stem_for("My Copy!").as_deref(), Some("my-copy-"));
        // Unicode letters are alphanumeric and survive.
        assert_eq!(file_stem_for("Café").as_deref(), Some("café"));
    }

    #[test]
    fn save_copy_rejects_blank_and_punctuation_only_names() {
        let dir = temp_dir("save-copy-bad-names").join("projects");
        let core = EditorCore::new();
        for name in ["   ", "!!!", "☕ ☕"] {
            let err = save_copy_to(&core, name, &dir).unwrap_err();
            assert!(
                err.contains("empty file name"),
                "name {name:?}: unexpected error: {err}"
            );
        }
        // Every refusal happens before anything is created.
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn save_copy_keeps_a_unicode_name_verbatim() {
        let dir = temp_dir("save-copy-unicode");
        let core = EditorCore::new();
        // 'é' is alphanumeric and is kept; the space and the
        // emoji each become a dash.
        let path = save_copy_to(&core, "Café ☕", &dir).unwrap();
        assert_eq!(path, dir.join("café--.json"));
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Café ☕");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_copy_refuses_a_slug_collision_with_a_different_name() {
        let dir = temp_dir("save-copy-slug-collision");
        let core = EditorCore::new();
        // "Foo Bar" and "Foo-Bar" are different names that
        // slugify to the same file. The second save must be
        // refused — never a silent overwrite of the first
        // project.
        let first = save_copy_to(&core, "Foo Bar", &dir).unwrap();
        let err = save_copy_to(&core, "Foo-Bar", &dir).unwrap_err();
        assert!(err.contains("already exists"), "unexpected error: {err}");
        assert_eq!(load(&first).unwrap().name, "Foo Bar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_copy_fails_honestly_on_an_overlong_name() {
        let dir = temp_dir("save-copy-long-name");
        let core = EditorCore::new();
        // A 300-char name makes a 305-byte file name, over
        // the 255-byte filesystem limit: the call errors and
        // writes nothing, rather than truncating the slug
        // (which could collide with another long name).
        let name = "a".repeat(300);
        assert!(save_copy_to(&core, &name, &dir).is_err());
        assert!(scan(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_rename_refuse_names_that_cannot_name_a_file() {
        let dir = temp_dir("bad-names-save-rename");
        let mut project = Project::new_demo();
        project.name = "   ".into();
        let err = save(&project, &dir).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(scan(&dir).is_empty());

        project.name = "Real".into();
        let path = save(&project, &dir).unwrap();
        let err = rename_project(&path, "!!!").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(path.exists(), "a refused rename leaves the file in place");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_cache_hits_only_on_an_exact_stamp_match() {
        use std::time::Duration;
        let path = PathBuf::from("some/dir/proj.json");
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let stamp = FileStamp {
            modified: Some(t0),
            len: 42,
        };
        let summary = ScanSummary {
            name: "Proj".into(),
            figures: 1,
            frames: 2,
            fps: 12.0,
        };
        let mut cache: HashMap<PathBuf, CachedScan> = HashMap::new();
        assert_eq!(cache_get(&cache, &path, stamp), None, "empty cache misses");
        cache_put(&mut cache, path.clone(), stamp, Some(summary.clone()));
        assert_eq!(cache_get(&cache, &path, stamp), Some(Some(summary)));
        // An in-place rewrite that only changed the length…
        assert_eq!(
            cache_get(&cache, &path, FileStamp { len: 43, ..stamp }),
            None,
            "a length change invalidates"
        );
        // …or only the mtime…
        assert_eq!(
            cache_get(
                &cache,
                &path,
                FileStamp {
                    modified: Some(t0 + Duration::from_secs(1)),
                    ..stamp
                }
            ),
            None,
            "an mtime change invalidates"
        );
        // …and an unknown path all miss.
        assert_eq!(cache_get(&cache, Path::new("other.json"), stamp), None);
        // Unparseable files are cached too: a hit at the same
        // stamp, a miss as soon as the stamp moves.
        cache_put(&mut cache, path.clone(), stamp, None);
        assert_eq!(cache_get(&cache, &path, stamp), Some(None));
        assert_eq!(
            cache_get(&cache, &path, FileStamp { len: 1, ..stamp }),
            None
        );
    }

    #[test]
    fn scan_cache_prune_drops_only_vanished_files_in_the_scanned_dir() {
        let dir = PathBuf::from("lib");
        let kept = dir.join("kept.json");
        let gone = dir.join("gone.json");
        let elsewhere = PathBuf::from("elsewhere/x.json");
        let stamp = FileStamp {
            modified: None,
            len: 1,
        };
        let mut cache: HashMap<PathBuf, CachedScan> = HashMap::new();
        for p in [&kept, &gone, &elsewhere] {
            cache_put(&mut cache, p.clone(), stamp, None);
        }
        cache_prune_dir(&mut cache, &dir, &[kept.clone()]);
        assert!(cache_get(&cache, &kept, stamp).is_some(), "seen file kept");
        assert!(cache_get(&cache, &gone, stamp).is_none(), "vanished file dropped");
        assert!(
            cache_get(&cache, &elsewhere, stamp).is_some(),
            "other directories are untouched"
        );
    }

    #[test]
    fn scan_repicks_a_file_rewritten_in_place() {
        let dir = temp_dir("scan-cache-rewrite");
        let mut project = Project::new_demo();
        project.name = "Alpha".into();
        let path = save(&project, &dir).unwrap();
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Alpha");
        assert_eq!(infos[0].figures, 1);

        // Rewrite the SAME path in place with a different
        // project (the editor's own save pattern — no
        // directory entry changes). The next scan must show
        // the new contents: the stamp moved, so the cached
        // summary is not served.
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut other = Project::new_demo();
        other.name = "Beta With A Longer Name".into();
        other.figures.push(other.figures[0].clone());
        std::fs::write(&path, serde_json::to_string_pretty(&other).unwrap()).unwrap();
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Beta With A Longer Name");
        assert_eq!(infos[0].figures, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_repicks_a_file_that_breaks_and_is_fixed() {
        let dir = temp_dir("scan-cache-break-fix");
        let mut project = Project::new_demo();
        project.name = "Stable".into();
        let path = save(&project, &dir).unwrap();
        assert_eq!(scan(&dir).len(), 1);

        // The file stops parsing: it leaves the listing (the
        // negative outcome is cached under the broken stamp,
        // not the good one).
        std::fs::write(&path, "{ broken").unwrap();
        assert!(scan(&dir).is_empty());

        // And when it is fixed in place it comes back.
        let mut fixed = Project::new_demo();
        fixed.name = "Fixed Again".into();
        std::fs::write(&path, serde_json::to_string_pretty(&fixed).unwrap()).unwrap();
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Fixed Again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_stem_for_neutralises_path_separators_and_dots() {
        // The slug is the path-safety rule: every
        // non-alphanumeric character — separators and dots
        // included — becomes a dash, so no name yields a
        // path outside the library directory or a `..`
        // component.
        assert_eq!(file_stem_for(".."), None, "no alphanumeric at all");
        assert_eq!(file_stem_for("../.."), None);
        assert_eq!(file_stem_for("a/b").as_deref(), Some("a-b"));
        assert_eq!(file_stem_for("a\\b").as_deref(), Some("a-b"));
        assert_eq!(file_stem_for("../../etc/passwd").as_deref(), Some("------etc-passwd"));
        // For every adversarial name that maps at all, the
        // stem is a single safe component.
        for name in ["../x", "..\\x", "/abs/path", "a/../b", ".hidden"] {
            let stem = file_stem_for(name).expect("these names carry letters");
            assert!(
                !stem.chars().any(|c| c == '/' || c == '\\') && stem != ".." && !stem.starts_with('.'),
                "name {name:?} produced an unsafe stem {stem:?}"
            );
        }
    }

    #[test]
    fn save_with_a_traversal_name_stays_inside_the_dir() {
        let dir = temp_dir("traversal-name");
        let mut project = Project::new_demo();
        project.name = "../../Escape Attempt".into();
        let path = save(&project, &dir).unwrap();
        // The written file is a direct child of the library
        // dir, under the slugged name — nothing was written
        // anywhere else.
        assert_eq!(path.parent(), Some(dir.as_path()));
        assert_eq!(path, dir.join("------escape-attempt.json"));
        assert!(path.exists());
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "../../Escape Attempt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_replaces_atomically_and_leaves_no_temp_files() {
        let dir = temp_dir("atomic-save");
        let mut project = Project::new_demo();
        project.name = "Atomic".into();
        let path = save(&project, &dir).unwrap();

        // A second save of changed state fully replaces the
        // first — the overwrite `save` promises by design.
        std::thread::sleep(std::time::Duration::from_millis(30));
        project.fps = 24.0;
        let again = save(&project, &dir).unwrap();
        assert_eq!(again, path);
        let back = load(&path).unwrap();
        assert!((back.fps - 24.0).abs() < f32::EPSILON);
        assert_eq!(scan(&dir).len(), 1, "scan sees the replaced save");

        // The temp file the atomic replace wrote through is
        // renamed away, not left as litter for a later scan
        // or listing to trip over.
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["atomic.json"], "directory holds {names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_repicks_a_same_length_rewrite_by_another_writer() {
        let dir = temp_dir("scan-cache-same-length");
        let mut project = Project::new_demo();
        project.name = "Alpha".into();
        let path = save(&project, &dir).unwrap();
        assert_eq!(scan(&dir)[0].name, "Alpha");

        // Another process (an MCP server driving the same
        // library directory) rewrites the file in place with
        // the SAME byte length — only the mtime moves. The
        // stamp's mtime half must carry the invalidation:
        // the next scan shows the new contents, never the
        // cached summary.
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut other = Project::new_demo();
        other.name = "Betta".into(); // same length as "Alpha"
        let text = serde_json::to_string_pretty(&other).unwrap();
        assert_eq!(
            text.len(),
            std::fs::read_to_string(&path).unwrap().len(),
            "the rewrite really is the same length"
        );
        std::fs::write(&path, text).unwrap();
        let infos = scan(&dir);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Betta");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An `EditorCore` whose cached selector list is exactly
    /// `files` with `index` selected — the state any save
    /// path hands to a refresh call.
    fn core_with_files(files: Vec<PathBuf>, index: usize) -> EditorCore {
        let mut core = EditorCore::new();
        core.project_files = files;
        core.project_index = index;
        core
    }

    #[test]
    fn list_project_files_excludes_shell_files_and_sorts() {
        let dir = temp_dir("list-files");
        // The shell's own files live in the projects dir but
        // are not openable projects: they never appear in the
        // selector list, and neither do dotfiles or non-JSON
        // files.
        std::fs::write(dir.join(crate::shell::LAYOUT_FILE), "{}").unwrap();
        std::fs::write(dir.join(crate::shell::RECOVERY_FILE), "{}").unwrap();
        std::fs::write(dir.join(crate::shell::RECENTS_FILE), "[]").unwrap();
        std::fs::write(dir.join(".hidden.json"), "{}").unwrap();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();
        let mut project = Project::new_demo();
        project.name = "Bravo".into();
        let bravo = save(&project, &dir).unwrap();
        project.name = "Alpha".into();
        let alpha = save(&project, &dir).unwrap();
        // An unparseable `.json` still lists: the selector
        // names files; parseability is `scan`'s concern.
        let broken = dir.join("broken.json");
        std::fs::write(&broken, "{ not a project").unwrap();

        let files = list_project_files(&dir);
        assert_eq!(
            files,
            vec![alpha, bravo, broken],
            "path-sorted, shell files excluded"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_project_files_picks_up_a_newly_created_file() {
        let dir = temp_dir("refresh-new");
        let mut project = Project::new_demo();
        project.name = "Alpha".into();
        let alpha = save(&project, &dir).unwrap();
        let mut core = core_with_files(list_project_files(&dir), 0);
        assert_eq!(core.project_files, vec![alpha.clone()]);

        // A save path (an MCP save, say) creates a second
        // project file behind the core's back: the cache
        // misses it until the refresh runs — the Wave 11
        // §6.6 bug, reproduced.
        project.name = "Beta".into();
        let beta = save(&project, &dir).unwrap();
        assert_eq!(
            core.project_files.len(),
            1,
            "the cache is stale until refreshed"
        );

        refresh_project_files_in(&mut core, &dir);
        assert_eq!(core.project_files, vec![alpha.clone(), beta]);
        // The selection still names the file it named before.
        assert_eq!(core.project_files[core.project_index], alpha);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_project_files_selection_follows_its_file() {
        let dir = temp_dir("refresh-identity");
        let mut project = Project::new_demo();
        project.name = "Bravo".into();
        let bravo = save(&project, &dir).unwrap();
        let mut core = core_with_files(vec![bravo.clone()], 0);

        // A new file that sorts BEFORE the selected one: a
        // raw-index refresh would silently retarget the
        // selection onto it; the identity refresh must not.
        project.name = "Alpha".into();
        let alpha = save(&project, &dir).unwrap();
        refresh_project_files_in(&mut core, &dir);
        assert_eq!(core.project_files, vec![alpha, bravo.clone()]);
        assert_eq!(core.project_index, 1);
        assert_eq!(core.project_files[core.project_index], bravo);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_project_files_on_a_missing_dir_is_a_no_op() {
        let dir = temp_dir("refresh-missing");
        let missing = dir.join("no-such-dir");
        assert!(!missing.exists());
        let files = vec![dir.join("a.json"), dir.join("b.json")];
        let mut core = core_with_files(files.clone(), 1);
        refresh_project_files_in(&mut core, &missing);
        // Not an empty list, not a panic: list and selection
        // are exactly as they were.
        assert_eq!(core.project_files, files);
        assert_eq!(core.project_index, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_project_files_drops_vanished_files_and_clamps() {
        let dir = temp_dir("refresh-vanished");
        let mut project = Project::new_demo();
        project.name = "Alpha".into();
        let alpha = save(&project, &dir).unwrap();
        project.name = "Bravo".into();
        let bravo = save(&project, &dir).unwrap();
        let mut core = core_with_files(vec![alpha.clone(), bravo.clone()], 1);

        // The selected file is renamed away — its old path
        // is gone, exactly the MCP rename's leftover in the
        // cache. The refresh drops the dead entry and the
        // index clamps onto a live file instead of pointing
        // past the end of the list.
        std::fs::remove_file(&bravo).unwrap();
        refresh_project_files_in(&mut core, &dir);
        assert_eq!(core.project_files, vec![alpha]);
        assert_eq!(core.project_index, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave 13 (B5, dirty-marking census): the refresh
    /// seam is list + selection ONLY — it must never move
    /// the edit counters in either direction. It runs
    /// right after saves (a bump would dirty a just-saved
    /// project; a reset would hide real edits from the
    /// autosave cadence) and inside the MCP rename, whose
    /// name write marks dirty at its own call site in
    /// `mcp_tools.rs` — never here.
    #[test]
    fn refresh_project_files_never_touches_the_edit_counters() {
        let dir = temp_dir("refresh-counters");
        let mut project = Project::new_demo();
        project.name = "Alpha".into();
        let alpha = save(&project, &dir).unwrap();
        let mut core = core_with_files(vec![alpha], 0);

        // A clean core (the just-saved state: counters
        // level) stays clean across a refresh.
        assert_eq!(core.edits_since_autosave(), 0);
        refresh_project_files_in(&mut core, &dir);
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "a refresh must not dirty a just-saved project"
        );

        // A dirty core (one counted edit, via the same
        // snapshot every mutating op takes) keeps exactly
        // its count: the refresh neither manufactures a
        // second edit nor launders the first.
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        refresh_project_files_in(&mut core, &dir);
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "a refresh must not move the autosave cadence"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave 13 (B5, dirty-marking census): saving a copy
    /// renames a CLONE — the session project is not the
    /// copy's file, so the copy is neither a save of the
    /// session (it must not clear the dirty baseline) nor
    /// an edit to it (it must not bump the count). The
    /// session's name and counters are exactly as they
    /// were; only the file on disk carries the new name.
    #[test]
    fn save_copy_leaves_the_sessions_dirty_state_untouched() {
        let dir = temp_dir("save-copy-dirty").join("projects");
        let mut core = EditorCore::new();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);

        let path = save_copy_to(&core, "Session Copy", &dir).unwrap();
        assert!(path.exists());
        assert_eq!(load(&path).unwrap().name, "Session Copy");

        assert_eq!(core.project.name, "Untitled");
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "a copy is not a save of the session file: the baseline must not move"
        );
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
