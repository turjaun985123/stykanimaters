//! App-shell support data: a recovery snapshot of the project
//! being edited and the most-recently-used project list.
//!
//! Where the stores live is [`store_dirs`]' one decision
//! (Roadmap Wave 1, RM9 / F-8): the recovery snapshot at
//! `.recovery.json` in the **state** dir (a full [`Project`]
//! JSON document, same format as a named save), the recents
//! list at `recents.json` in the state dir (a JSON array of
//! project names, most recent first, capped at
//! [`MAX_RECENTS`]), the layout preferences in the **config**
//! dir, and project files / libraries / the import folder
//! under the **data** dir — all resolved from Rune's
//! per-user [`rune::dirs::AppDirs`], with a portable mode
//! (see [`PORTABLE_ENV`] / [`PORTABLE_FLAG`]) that keeps the
//! historical `./projects/` layout rooted at the working
//! directory.
//!
//! The `*_in` variants take the directory explicitly so tests
//! (and any future settings screen) never touch the real folder.
//!
//! On top of the two stores sit two session-continuity queries
//! for a future startup prompt: which project was open last,
//! and whether the recovery snapshot is newer than its last
//! named save.
//!
//! A third, smaller store holds the window layout
//! preferences at `.layout.json` (last workspace, theme,
//! UI language, timeline collapse, sidebar visibility,
//! column widths)
//! so the app shell can restore them on startup. New
//! fields are added with serde defaults so older
//! `.layout.json` files keep loading. A file the
//! whole-struct parse rejects is salvaged field by field
//! ([`salvage_layout`]): one mistyped value loses only
//! its own field, never every other preference with it.
//!
//! The module also keeps the session's live-MCP start
//! record ([`LiveMcpStartOutcome`]): whether the most
//! recent attempt to start the editor-hosted MCP bridge
//! actually got it listening. The bridge module reports
//! start failure as a bare `false` and keeps no reason,
//! so without this record a failed start (in practice:
//! another editor instance already holds the port) is
//! invisible everywhere except the settings toggle's
//! position.
//!
//! Every write in this module goes through the same
//! atomic replace ([`write_file_atomic`]): the new
//! document is written to a uniquely named temp file in
//! the same directory and renamed over the real path, so
//! a crash mid-write — or two saves racing — leaves the
//! previous complete file or the new one at the real
//! path, never a truncated one. (Every reader here
//! already folds a missing or corrupt file into a
//! default; the atomic replace means a torn file should
//! never be the reason it has to.)

use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::SystemTime;

/// File name of the recovery snapshot inside the state dir.
pub const RECOVERY_FILE: &str = ".recovery.json";
/// File name of the recents list inside the state dir.
pub const RECENTS_FILE: &str = "recents.json";
/// File name of the layout preferences inside the config dir.
pub const LAYOUT_FILE: &str = ".layout.json";
/// Maximum number of entries kept in the recents list.
pub const MAX_RECENTS: usize = 10;

/// The reverse-DNS application id the per-user folders are
/// resolved under (Rune's [`rune::dirs::AppDirs`] convention):
/// the app's GitHub identity, `turjaun985123/stykanimaters`.
pub const APP_ID: &str = "io.github.turjaun985123.stykanimaters";

/// Environment variable requesting portable mode: when it
/// holds a truthy value (`1`, `true`, `yes`, `on`,
/// case-insensitive), every store resolves to the historical
/// working-directory layout (`./projects/`, `./library/`,
/// `./library3d/`, `./import/`). The headless CI probes run
/// this way — they wipe and re-seed `./projects/` per scene —
/// and a portable install carried on a stick keeps working
/// exactly as it always has.
pub const PORTABLE_ENV: &str = "STYKANIMATE_PORTABLE";

/// Command-line flag requesting portable mode, the
/// [`PORTABLE_ENV`] equivalent for a single launch.
pub const PORTABLE_FLAG: &str = "--portable";

/// The resolved store directories: the one answer to "where
/// does the app keep its files" (Roadmap Wave 1, RM9 / F-8,
/// replacing the CWD-relative `./projects/` of A27 G1).
///
/// The category split follows Rune's [`rune::dirs::AppDirs`]:
/// project files, the figure libraries and the import folder
/// are the user's *documents* and live under the data dir
/// (in same-named subfolders, so their relative shape — and
/// the project scan's one-directory contract — is unchanged);
/// layout preferences are *settings* and live in the config
/// dir; the recents list and the recovery snapshot are
/// machine-local *history / safety state* and live in the
/// state dir.
///
/// Two layouts collapse the split back to the historical
/// shape: portable mode ([`PORTABLE_ENV`] / [`PORTABLE_FLAG`])
/// and the no-home fallback (AppDirs cannot resolve without
/// a home folder — a broken environment must degrade to the
/// layout that always worked, never to no storage at all).
/// Both root everything at the process working directory,
/// byte-identical to the pre-F-8 paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreDirs {
    /// The AppDirs data dir itself (the migration gate reads
    /// it; stores live in its subfolders below).
    data: PathBuf,
    /// Project files: `<data>/projects` (portable: `./projects`).
    projects: PathBuf,
    /// The 2D figure library: `<data>/library` (portable: `./library`).
    library: PathBuf,
    /// The 3D figure library: `<data>/library3d` (portable: `./library3d`).
    library3d: PathBuf,
    /// The import staging folder: `<data>/import` (portable: `./import`).
    import: PathBuf,
    /// Layout preferences: the AppDirs config dir itself.
    config: PathBuf,
    /// Recents + recovery snapshot: the AppDirs state dir itself.
    state: PathBuf,
    /// True only for the explicitly requested portable
    /// layout (the no-home fallback reports `false`: it is
    /// the legacy shape, but nobody chose it).
    portable: bool,
}

impl StoreDirs {
    /// Project files live here (`projects_dir`).
    pub fn projects(&self) -> &Path {
        &self.projects
    }

    /// The 2D figure library lives here (`library_dir`).
    pub fn library(&self) -> &Path {
        &self.library
    }

    /// The 3D figure library lives here (`library3d_dir`).
    pub fn library3d(&self) -> &Path {
        &self.library3d
    }

    /// Import staging lives here (`import_dir`).
    pub fn import(&self) -> &Path {
        &self.import
    }

    /// Layout preferences live here (`config_dir`).
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// Recents and the recovery snapshot live here (`state_dir`).
    pub fn state(&self) -> &Path {
        &self.state
    }

    /// The data root the category subfolders hang off; the
    /// legacy-migration gate reads it (see
    /// [`legacy_migration_plan`]).
    pub fn data_root(&self) -> &Path {
        &self.data
    }

    /// Whether this is the explicitly requested portable
    /// layout (see [`PORTABLE_ENV`]).
    pub fn is_portable(&self) -> bool {
        self.portable
    }

    /// The category split over a resolved Rune AppDirs.
    fn from_app_dirs(dirs: &rune::dirs::AppDirs) -> StoreDirs {
        let data = dirs.data().to_path_buf();
        StoreDirs {
            projects: data.join("projects"),
            library: data.join("library"),
            library3d: data.join("library3d"),
            import: data.join("import"),
            config: dirs.config().to_path_buf(),
            state: dirs.state().to_path_buf(),
            data,
            portable: false,
        }
    }

    /// The historical layout rooted at `cwd`: the shell
    /// stores together in `./projects`, the libraries and
    /// the import folder beside it. The no-home fallback
    /// and (flagged `portable`) portable mode both take
    /// this shape.
    fn legacy_at(cwd: &Path) -> StoreDirs {
        let projects = cwd.join("projects");
        StoreDirs {
            data: cwd.to_path_buf(),
            projects: projects.clone(),
            library: cwd.join("library"),
            library3d: cwd.join("library3d"),
            import: cwd.join("import"),
            config: projects.clone(),
            state: projects,
            portable: false,
        }
    }

    /// The legacy layout as an explicitly requested
    /// portable install rooted at `cwd`.
    fn portable_at(cwd: &Path) -> StoreDirs {
        let mut dirs = Self::legacy_at(cwd);
        dirs.portable = true;
        dirs
    }
}

/// Whether portable mode is requested, from an injected
/// argument list and environment lookup (the pure seam
/// [`store_dirs_at`] resolves through, so the truthiness
/// table is testable without touching the process state):
/// the [`PORTABLE_FLAG`] anywhere in the arguments, or
/// [`PORTABLE_ENV`] holding `1` / `true` / `yes` / `on`
/// (trimmed, case-insensitive). Any other value — `0`,
/// `false`, an unrelated string — is *not* a request:
/// a stray variable must never silently move a user's
/// library back to the working directory.
fn portable_requested_from(
    args: impl Iterator<Item = std::ffi::OsString>,
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> bool {
    if args.into_iter().any(|a| a.to_str() == Some(PORTABLE_FLAG)) {
        return true;
    }
    match var(PORTABLE_ENV) {
        Some(v) => matches!(
            v.to_string_lossy().trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

/// Whether portable mode is requested for this process.
fn portable_requested() -> bool {
    portable_requested_from(std::env::args_os(), |k| std::env::var_os(k))
}

/// Resolve the store directories for this process: the
/// ONE resolver every store in this module — and every
/// caller that must agree with them (the Home panel's
/// scan root, the project selector, the MCP project
/// tools, the library ops) — takes its paths from.
///
/// Resolution is deliberately *not* cached: it reads the
/// process environment and working directory on each
/// call (a handful of `env` lookups), so a caller can
/// never hold a stale answer across a portable flag, a
/// `RUNE_DIRS_ROOT` isolation root (which AppDirs honours
/// itself), or a working-directory change. Nothing here
/// touches the disk — directories are created by the
/// writers, as before.
pub fn store_dirs() -> StoreDirs {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    store_dirs_at(&cwd)
}

/// [`store_dirs`] resolved against an explicit working
/// directory (the process environment still supplies the
/// portable request and the AppDirs bases).
fn store_dirs_at(cwd: &Path) -> StoreDirs {
    if portable_requested() {
        return StoreDirs::portable_at(cwd);
    }
    match rune::dirs::AppDirs::new(APP_ID) {
        Some(dirs) => StoreDirs::from_app_dirs(&dirs),
        // No home folder to resolve against: degrade to
        // the layout that always worked rather than
        // failing every save in the app.
        None => StoreDirs::legacy_at(cwd),
    }
}

/// The projects directory: where project files live and
/// the Home scan roots — [`store_dirs`]' projects entry.
/// Code that must agree with the stores about *where*
/// projects live takes the path from here instead of
/// spelling `"projects"` again.
pub fn projects_dir() -> PathBuf {
    store_dirs().projects().to_path_buf()
}

/// The 2D figure-library directory: [`store_dirs`]'
/// library entry (the pose library is its `poses`
/// subfolder).
pub fn library_dir() -> PathBuf {
    store_dirs().library().to_path_buf()
}

/// The 3D figure-library directory: [`store_dirs`]'
/// library3d entry (its pose library is the `poses`
/// subfolder).
pub fn library3d_dir() -> PathBuf {
    store_dirs().library3d().to_path_buf()
}

/// The import staging directory: [`store_dirs`]' import
/// entry — the named folder inside the data dir the
/// folder-import flows resolve against (A27 R1).
pub fn import_dir() -> PathBuf {
    store_dirs().import().to_path_buf()
}

/// The config directory, holding the layout preferences:
/// [`store_dirs`]' config entry.
pub fn config_dir() -> PathBuf {
    store_dirs().config().to_path_buf()
}

/// The state directory, holding the recents list and the
/// recovery snapshot: [`store_dirs`]' state entry.
pub fn state_dir() -> PathBuf {
    store_dirs().state().to_path_buf()
}

// ---- Legacy migration (one-time, prompted, never silent) ----

/// One file the legacy migration would copy: from the
/// historical working-directory layout to its resolved
/// per-user home, with its size at plan time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationCopy {
    /// The legacy file (under `./projects/`, `./library/`,
    /// `./library3d/` or `./import/`).
    pub from: PathBuf,
    /// Its destination under the resolved store dirs.
    pub to: PathBuf,
    /// The file's size in bytes when the plan was built.
    pub bytes: u64,
}

/// A built legacy-migration plan: the inventory a prompt
/// shows and the exact copies a confirmation performs.
/// Building a plan changes nothing on disk — the plan IS
/// the prompt's arm, in the shell's two-press discipline:
/// the first press (or startup check) builds and presents
/// it, and only an explicit second act —
/// [`confirm_legacy_migration`] — writes anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyMigrationPlan {
    copies: Vec<MigrationCopy>,
}

impl LegacyMigrationPlan {
    /// The file copies the confirmation would perform,
    /// in deterministic (sorted-source) order.
    pub fn copies(&self) -> &[MigrationCopy] {
        &self.copies
    }

    /// How many files the migration would copy.
    pub fn file_count(&self) -> usize {
        self.copies.len()
    }

    /// Total bytes the migration would copy.
    pub fn total_bytes(&self) -> u64 {
        self.copies.iter().map(|c| c.bytes).sum()
    }
}

/// What a confirmed migration did: files copied, files
/// skipped because their destination had appeared since
/// the plan was built (never overwritten), bytes copied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Files copied to their new home.
    pub copied: usize,
    /// Planned files whose destination already existed at
    /// confirm time and were left untouched.
    pub skipped_existing: usize,
    /// Bytes copied.
    pub bytes: u64,
}

/// Every regular file under `root`, recursively (missing
/// root → none). Symlinks and special files are not
/// followed and do not count: the migration copies what
/// the app itself wrote, and the app writes regular files.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

/// The migration gate: may a legacy migration be offered
/// against `dirs` at all? Only while the new data root
/// holds nothing the migration did not itself put there —
/// every file under it must be the byte-identical copy of
/// the legacy file at the same relative path (the shape a
/// *completed or interrupted* migration leaves). A data
/// root with any other file in it is a layout already in
/// use: no offer, ever — the user's new work is never
/// merged into, or measured against, their old by a
/// migration prompt. (The shell-owned files live in the
/// config/state dirs, not the data root, so they are not
/// the gate's business; their copies are planned
/// individually, only while their destination is absent.)
fn migration_gate_open(cwd: &Path, dirs: &StoreDirs) -> bool {
    let data = dirs.data_root();
    for file in files_under(data) {
        let Ok(rel) = file.strip_prefix(data) else {
            return false;
        };
        // Data files live under one of the four category
        // subfolders, each named exactly as its legacy
        // root — so the legacy source of a migrated file
        // is the same relative path under `cwd`.
        let top = rel.components().next();
        let known_root = matches!(
            top.and_then(|c| c.as_os_str().to_str()),
            Some("projects" | "library" | "library3d" | "import")
        );
        if !known_root || rel.components().count() < 2 {
            return false;
        }
        let source = cwd.join(rel);
        let identical = match (std::fs::read(&source), std::fs::read(&file)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if !identical {
            return false;
        }
    }
    true
}

/// Plan the copies for one legacy tree (`from_root` and
/// everything under it) into `to_root`, preserving the
/// relative shape. A destination that already exists is
/// not planned (never overwritten); a source equal to its
/// destination (the fallback layout) plans nothing.
fn collect_tree_copies(from_root: &Path, to_root: &Path, copies: &mut Vec<MigrationCopy>) {
    for from in files_under(from_root) {
        let Ok(rel) = from.strip_prefix(from_root) else {
            continue;
        };
        let to = to_root.join(rel);
        if to == from || to.exists() {
            continue;
        }
        let bytes = std::fs::metadata(&from).map(|m| m.len()).unwrap_or(0);
        copies.push(MigrationCopy { from, to, bytes });
    }
}

/// Build the legacy-migration plan for this process, or
/// `None` when there is nothing to offer: portable mode
/// (already legacy), a closed gate (the new layout is in
/// use), or no legacy files at all. **This function only
/// reads** — it is safe to call at startup before any
/// store write, and calling it never migrates anything.
pub fn legacy_migration_plan() -> Option<LegacyMigrationPlan> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    legacy_migration_plan_at(&cwd, &store_dirs_at(&cwd))
}

/// [`legacy_migration_plan`] against explicit roots (the
/// testable seam).
/// Plan one file copy, unless the destination equals the
/// source (the collapsed layouts) or already exists
/// (never overwritten).
fn plan_one_copy(from: &Path, to: PathBuf, copies: &mut Vec<MigrationCopy>) {
    if to == from || to.exists() {
        return;
    }
    let bytes = std::fs::metadata(from).map(|m| m.len()).unwrap_or(0);
    copies.push(MigrationCopy {
        from: from.to_path_buf(),
        to,
        bytes,
    });
}

fn legacy_migration_plan_at(
    cwd: &Path,
    dirs: &StoreDirs,
) -> Option<LegacyMigrationPlan> {
    // A collapsed layout (portable, or the no-home
    // fallback where every category is the legacy dir
    // itself) has nowhere to migrate to.
    let collapsed =
        dirs.config() == dirs.projects() && dirs.state() == dirs.projects();
    if dirs.is_portable() || collapsed || !migration_gate_open(cwd, dirs) {
        return None;
    }
    let mut copies: Vec<MigrationCopy> = Vec::new();
    // The legacy `./projects/` tree routes by name: the
    // three shell-owned files go to their category dirs,
    // everything else — project files and any subfolder —
    // keeps its shape under the new projects dir.
    let legacy_projects = cwd.join("projects");
    for entry in std::fs::read_dir(&legacy_projects).into_iter().flatten().flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let routed = match name.to_str() {
            Some(n) if n == LAYOUT_FILE && kind.is_file() => {
                Some(dirs.config().join(LAYOUT_FILE))
            }
            Some(n) if n == RECENTS_FILE && kind.is_file() => {
                Some(dirs.state().join(RECENTS_FILE))
            }
            Some(n) if n == RECOVERY_FILE && kind.is_file() => {
                Some(dirs.state().join(RECOVERY_FILE))
            }
            _ => None,
        };
        match routed {
            Some(to) => plan_one_copy(&entry.path(), to, &mut copies),
            None if kind.is_file() => {
                plan_one_copy(&entry.path(), dirs.projects().join(&name), &mut copies)
            }
            None => collect_tree_copies(
                &entry.path(),
                &dirs.projects().join(&name),
                &mut copies,
            ),
        }
    }
    // The sibling legacy roots keep their shape under the
    // data dir's same-named subfolders.
    collect_tree_copies(&cwd.join("library"), dirs.library(), &mut copies);
    collect_tree_copies(&cwd.join("library3d"), dirs.library3d(), &mut copies);
    collect_tree_copies(&cwd.join("import"), dirs.import(), &mut copies);
    copies.sort_by(|a, b| a.from.cmp(&b.from));
    if copies.is_empty() {
        None
    } else {
        Some(LegacyMigrationPlan { copies })
    }
}

/// Perform a planned legacy migration — the confirmation
/// half of the two-press discipline, and the ONLY half
/// that writes. Copies, never moves: the legacy tree is
/// left exactly as it was, so a user who declines the new
/// layout (or a build that still resolves it) loses
/// nothing. Each file is claimed with `create_new`, so a
/// destination that appeared after the plan was built is
/// skipped and reported, never overwritten; a hard error
/// (unreadable source, uncreatable directory) aborts with
/// the copies already made left in place — re-planning
/// resumes where it stopped, because the gate recognises
/// the migration's own output (see [`migration_gate_open`]).
pub fn confirm_legacy_migration(plan: &LegacyMigrationPlan) -> io::Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for copy in &plan.copies {
        if copy.to.exists() {
            report.skipped_existing += 1;
            continue;
        }
        if let Some(parent) = copy.to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut src = std::fs::File::open(&copy.from)?;
        let mut dst = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy.to)
        {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                report.skipped_existing += 1;
                continue;
            }
            Err(e) => return Err(e),
        };
        let n = std::io::copy(&mut src, &mut dst)?;
        report.copied += 1;
        report.bytes += n;
    }
    Ok(report)
}

/// Sequence number for atomic-write temp files (see
/// [`write_file_atomic`]): combined with the process id
/// it makes every temp name unique, so two threads of
/// this process — or two editor processes saving at the
/// same moment — never write into the same temp file.
static WRITE_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `text` to `path` atomically: serialise to a
/// uniquely named temp file in the same directory, then
/// rename it over `path`.
///
/// The temp file must live in the same directory because
/// a rename is only atomic within one filesystem. The
/// name is `.{file}.tmp-{pid}-{seq}`: the pid keeps two
/// editor processes apart, the process-wide sequence
/// keeps two threads of one process apart, and the
/// leading dot plus the non-`.json` tail keep the temp
/// out of every reader's way (all readers in this module
/// look files up by their exact names, and the Home
/// panel's project listing matches `*.json` names).
/// `std::fs::rename` replaces an existing destination
/// on both POSIX and Windows, so a reader opening `path`
/// at any moment sees a complete old or complete new
/// document — never the truncated half-write a direct
/// `std::fs::write` onto `path` could leave behind if
/// the process died (or another writer raced) mid-write.
///
/// Failure handling: a failed temp write or rename
/// removes the temp file on a best-effort basis and
/// propagates the error; a crash between the write and
/// the rename can orphan one temp file, which is inert
/// litter (nothing ever reads it). Deliberately no
/// `fsync`: this buys untorn reads, not power-loss
/// durability — an OS crash can still lose the newest
/// save, which every reader here tolerates by falling
/// back to its default, and skipping the sync keeps the
/// splitter drag's save-per-pointer-move cheap.
fn write_file_atomic(path: &Path, text: &str) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("write");
    let seq = WRITE_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{seq}",
        std::process::id()
    ));
    let written = std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(text.as_bytes()));
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

// ---- Recovery snapshot ----

/// Write `project` as the recovery snapshot in `./projects/`,
/// creating the directory if needed, and return its path.
pub fn write_recovery(project: &Project) -> io::Result<PathBuf> {
    write_recovery_in(&state_dir(), project)
}

/// [`write_recovery`] with an explicit base directory.
///
/// The snapshot is replaced atomically
/// ([`write_file_atomic`]): it is the user's crash
/// safety net, so a crash *during* an autosave must not
/// be able to truncate the very file that exists for
/// recovering from crashes.
pub fn write_recovery_in(dir: &Path, project: &Project) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(RECOVERY_FILE);
    let text = serde_json::to_string_pretty(project).map_err(io::Error::other)?;
    write_file_atomic(&path, &text)?;
    Ok(path)
}

/// Read the recovery snapshot from `./projects/`, if one exists
/// and parses. A missing or corrupt file yields `None`.
pub fn read_recovery() -> Option<Project> {
    read_recovery_in(&state_dir())
}

/// [`read_recovery`] with an explicit base directory.
pub fn read_recovery_in(dir: &Path) -> Option<Project> {
    let text = std::fs::read_to_string(dir.join(RECOVERY_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Delete the recovery snapshot from `./projects/` (no error if
/// it is already gone).
pub fn clear_recovery() {
    clear_recovery_in(&state_dir());
}

/// [`clear_recovery`] with an explicit base directory.
pub fn clear_recovery_in(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(RECOVERY_FILE));
}

// ---- Recents ----

/// Record `name` as the most recently used project in
/// `./projects/recents.json`: moved to the front, deduplicated,
/// capped at [`MAX_RECENTS`]. Write failures are ignored — the
/// recents list is a convenience, never worth an error dialog.
pub fn touch_recent(name: &str) {
    touch_recent_in(&state_dir(), name);
}

/// [`touch_recent`] with an explicit base directory.
pub fn touch_recent_in(dir: &Path, name: &str) {
    let mut list = recents_in(dir);
    list.retain(|n| n != name);
    list.insert(0, name.to_string());
    list.truncate(MAX_RECENTS);
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(&list) {
            let _ = write_file_atomic(&dir.join(RECENTS_FILE), &text);
        }
    }
}

/// The recents list from `./projects/recents.json`,
/// most recent first. Missing or corrupt file → empty list.
pub fn recents() -> Vec<String> {
    recents_in(&state_dir())
}

/// [`recents`] with an explicit base directory.
pub fn recents_in(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join(RECENTS_FILE)) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

// ---- Session continuity ----

/// The display name of the most recently opened project: the
/// front of the recents list. `None` when nothing has been
/// recorded yet (a missing or corrupt recents file already
/// reads as an empty list via [`recents`]). Recents store
/// display names, not paths, so the name is returned as-is.
pub fn last_opened_project() -> Option<String> {
    last_opened_project_in(&state_dir())
}

/// [`last_opened_project`] with an explicit base directory.
pub fn last_opened_project_in(dir: &Path) -> Option<String> {
    recents_in(dir).into_iter().next()
}

/// The file-name slug for a project name: the crate-wide
/// naming convention (`editor.rs::save_named_project`,
/// `projects::save`) — the name lowercased with every
/// non-alphanumeric character replaced by `-`, giving the
/// project's file as `<slug>.json` in the projects directory.
/// Duplicated here because both owners keep their copy
/// private; keep the rule in sync with them.
fn project_file_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
}

/// A file's modification time, swallowing every error into
/// `None` (the caller treats an unreadable clock as "no
/// timestamp", never as a failure worth surfacing).
fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The recovery snapshot's modification time — when the
/// shell last autosaved — or `None` when no snapshot
/// exists. The status strip's autosave segment (Phase J)
/// reads this to show how fresh the safety net is; every
/// filesystem error folds into `None`, like `file_mtime`.
pub fn recovery_modified() -> Option<SystemTime> {
    file_mtime(&state_dir().join(RECOVERY_FILE))
}

/// Delete a project file from disk — the one destructive
/// helper in this module, written for its single caller:
/// the Home panel's two-step delete (Phase K), which asks
/// for confirmation first and then calls this with the
/// row's path.
///
/// The shell's own files are refused by name: a path whose
/// file name is [`LAYOUT_FILE`], [`RECOVERY_FILE`] or
/// [`RECENTS_FILE`] returns an `InvalidInput` error
/// without touching the disk. The recovery snapshot is the
/// user's safety net, and the panel must never be able to
/// delete it (or the layout prefs, or the recents list)
/// through this path — the panel only ever lists named
/// project saves, so a refusal here means a bug or a
/// hand-crafted path, never a real project. Any other
/// path is handed to `std::fs::remove_file` and its result
/// propagated untouched, so a missing file surfaces as an
/// honest `NotFound` for the caller to report.
pub fn delete_project_file(path: &Path) -> io::Result<()> {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name == LAYOUT_FILE || name == RECOVERY_FILE || name == RECENTS_FILE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to delete a shell-owned file",
            ));
        }
    }
    std::fs::remove_file(path)
}

/// The pure decision behind
/// [`recovery_is_newer_than_last_project_in`]: no recovery
/// snapshot → `false`; a snapshot with no last-saved project
/// to compare against → `true`; otherwise the snapshot must
/// be *strictly* newer than the last save.
fn recovery_newer(recovery: Option<SystemTime>, last_project: Option<SystemTime>) -> bool {
    match (recovery, last_project) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(snapshot), Some(saved)) => snapshot > saved,
    }
}

/// The pure content decision behind the recovery gate in
/// [`recovery_is_newer_than_last_project_in`]: do two
/// project documents hold the *same* project?
///
/// Both texts are parsed into [`Project`] and the parsed
/// values re-serialised for comparison, so the verdict is
/// about content, not bytes: pretty vs compact formatting,
/// key order, number spelling and fields an older file
/// omits (every field carries a serde default) cannot
/// manufacture a difference, and no field is excluded —
/// the `Project` document carries no volatile field (no
/// timestamps, no session state; it is the same document
/// the undo system snapshots with `serde_json::to_string`,
/// so "same content" here means exactly what "same work"
/// means there). The compact re-serialisation of a parsed
/// struct is deterministic — field order fixed by the
/// type, floats in serde_json's shortest round-trip form.
///
/// Fail-open: `false` means "not proven identical", never
/// "proven different". A document that fails to parse —
/// a corrupt snapshot, a corrupt save, a format this
/// build cannot read — proves nothing, so the two never
/// "match", not even byte-identical corrupt texts. The
/// caller reads `false` as "the offer stands": the gate
/// may only suppress an offer it can *prove* spurious,
/// never one the user might need.
fn project_documents_match(a: &str, b: &str) -> bool {
    let (Ok(pa), Ok(pb)) = (
        serde_json::from_str::<Project>(a),
        serde_json::from_str::<Project>(b),
    ) else {
        return false;
    };
    match (serde_json::to_string(&pa), serde_json::to_string(&pb)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// Whether the recovery snapshot in `dir` holds the same
/// project content as the last-saved project file at
/// `saved_path` — the recovery gate's disk half. Every
/// read failure folds into `false` (see
/// [`project_documents_match`] for why unprovable always
/// keeps the offer).
fn recovery_matches_last_save(dir: &Path, saved_path: &Path) -> bool {
    let Ok(snapshot_text) = std::fs::read_to_string(dir.join(RECOVERY_FILE)) else {
        return false;
    };
    let Ok(saved_text) = std::fs::read_to_string(saved_path) else {
        return false;
    };
    project_documents_match(&snapshot_text, &saved_text)
}

/// Whether the recovery snapshot holds work the last named
/// save does not: `true` when a recovery file exists and
/// either no project has been opened yet, or the snapshot's
/// mtime beats the most recent project's file mtime *and*
/// the snapshot's content actually differs from that save.
///
/// The content clause is the Wave R (R22) gate on the
/// spurious offer P42 audited: a clean GUI save refreshes
/// the snapshot *after* writing the save file, so the
/// snapshot of a just-saved project is strictly newer by
/// mtime while holding identical content — recovering it
/// would hand the user their own saved work as if it were
/// crash salvage. When the mtime verdict fires and there
/// is a last save to compare against, the contents are
/// compared ([`recovery_matches_last_save`]) and an
/// identical snapshot is not offered. The gate fails
/// open: a snapshot or save that cannot be read or parsed
/// keeps the offer, exactly as the mtime-only condition
/// behaved (see [`project_documents_match`]).
///
/// Every filesystem error folds into `false` — a recent
/// whose file cannot be stat'ed cannot be compared, so it
/// never claims newer work — and the check must never
/// block startup.
pub fn recovery_is_newer_than_last_project() -> bool {
    // The snapshot and recents live in the state dir; the
    // project file they are compared against lives in the
    // projects dir — the two moved apart in the F-8 split,
    // so the live verdict resolves each from its own home.
    let dirs = store_dirs();
    recovery_is_newer_than_last_project_in_dirs(dirs.state(), dirs.projects())
}

/// [`recovery_is_newer_than_last_project`] with an explicit
/// base directory (snapshot, recents and project file all
/// in `dir` — the one-dir shape the `*_in` seams and the
/// headless probes have always used).
pub fn recovery_is_newer_than_last_project_in(dir: &Path) -> bool {
    recovery_is_newer_than_last_project_in_dirs(dir, dir)
}

/// [`recovery_is_newer_than_last_project`] with the state
/// dir (snapshot + recents) and the projects dir (the
/// recorded project file) given separately — the shape the
/// F-8 category split resolves to in production.
pub fn recovery_is_newer_than_last_project_in_dirs(
    state_dir: &Path,
    projects_dir: &Path,
) -> bool {
    let snapshot = file_mtime(&state_dir.join(RECOVERY_FILE));
    let (saved, saved_path) = match recents_in(state_dir).into_iter().next() {
        // No recents: nothing saved to compare against.
        None => (None, None),
        Some(name) => {
            let path =
                projects_dir.join(format!("{}.json", project_file_slug(&name)));
            match file_mtime(&path) {
                Some(mtime) => (Some(mtime), Some(path)),
                // A recorded project whose file is gone or
                // unreadable is an error, not "no save".
                None => return false,
            }
        }
    };
    if !recovery_newer(snapshot, saved) {
        return false;
    }
    match saved_path {
        // No last project (fresh install, snapshot only):
        // nothing to compare content against, so the
        // mtime verdict stands as it always has.
        None => true,
        // The content gate: an mtime-newer snapshot whose
        // content is the last save's own content is the
        // clean-save refresh, not unsaved work.
        Some(path) => !recovery_matches_last_save(state_dir, &path),
    }
}

// ---- Layout preferences ----

/// Serde default for boolean layout fields that default to
/// `true` (e.g. [`LayoutPrefs::sidebar_visible`]): files
/// written before the field existed load with it enabled.
fn default_true() -> bool {
    true
}

/// Serde default for [`LayoutPrefs::timeline_zoom`]:
/// the pre-zoom chip width, 30.0 — the value of the
/// bin-side `timeline_marks::CHIP_W`, spelled as a
/// literal because the library cannot name the
/// binary's constant (see the field's own docs).
fn default_timeline_zoom() -> f32 {
    30.0
}

/// Serde default for [`LayoutPrefs::col_left_w`]: the left
/// column's long-standing fixed width, so files written
/// before the field existed load unchanged.
fn default_col_left_w() -> f32 {
    300.0
}

/// Serde default for [`LayoutPrefs::col_right_w`]: the
/// right column's long-standing fixed width, like
/// [`default_col_left_w`].
fn default_col_right_w() -> f32 {
    340.0
}

/// Clamp a persisted column width into `[min, max]` on
/// load: a hand-edited or corrupt `.layout.json` must
/// never squeeze the stage away or push a column off
/// screen. A non-finite value falls back to the field's
/// default rather than propagating into the layout.
fn clamp_col_width(w: f32, default: f32, min: f32, max: f32) -> f32 {
    if w.is_finite() {
        w.clamp(min, max)
    } else {
        default
    }
}

/// Serde default for [`LayoutPrefs::lang`]: English, the
/// UI's long-standing first-run language, so files
/// written before the field existed load unchanged.
fn default_lang() -> crate::i18n::Lang {
    crate::i18n::Lang::En
}

/// The language for a persisted BCP 47 code, matched
/// case-insensitively over the i18n table (the same
/// resolution the shell settings tools perform): an
/// unknown code — a hand edit's typo — resolves to
/// `None`, never to a guessed language.
fn lang_from_code(code: &str) -> Option<crate::i18n::Lang> {
    crate::i18n::Lang::all()
        .iter()
        .find(|l| l.code().eq_ignore_ascii_case(code))
        .copied()
}

/// Serialise [`LayoutPrefs::lang`] as its BCP 47 code —
/// the settings-file shape a human editing
/// `.layout.json` reads (`"lang": "bn"`), and the code
/// `Lang::code()` documents itself as existing for.
fn serialize_lang<S: serde::Serializer>(
    lang: &crate::i18n::Lang,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(lang.code())
}

/// Deserialise [`LayoutPrefs::lang`] from its BCP 47
/// code. An unknown code is an error, so the strict
/// whole-struct parse fails over to [`salvage_layout`],
/// which defaults just this field (see its lang arm).
fn deserialize_lang<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<crate::i18n::Lang, D::Error> {
    use serde::de::Error as _;
    let code = String::deserialize(deserializer)?;
    lang_from_code(&code).ok_or_else(|| {
        D::Error::custom(format!("unknown language code: {code}"))
    })
}

/// Window layout preferences persisted across runs: which
/// workspace was active, whether the dark theme was on, the
/// UI language, the timeline / sidebar arrangement of the
/// context-first shell, whether live MCP control starts
/// with the editor, and the side-column widths. Stored as
/// JSON at `.layout.json` in the projects dir. The newer
/// fields carry serde defaults so a `.layout.json` written
/// by an older build still loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutPrefs {
    /// Name of the active workspace (e.g. `"Animate"`).
    pub workspace: String,
    /// Whether the dark theme is enabled.
    pub dark: bool,
    /// The UI language (a [`crate::i18n::Lang`]), stored
    /// on disk as its BCP 47 code (`"lang": "bn"`). Seeded
    /// into `EditorCore.lang` by the shell's `init` (the
    /// `dark` field's exact precedent, Roadmap F-9) and
    /// written back by `poll`'s shadow compare when the
    /// user changes language — before this field existed
    /// the choice was session-only and every relaunch
    /// reset the UI to English, while the settings docs
    /// and the shell settings tools claimed it persisted
    /// with the project file. Files written before the
    /// field existed load as English; an unknown code in
    /// a hand-edited file is salvaged to English by
    /// [`salvage_layout`] without losing the other fields.
    #[serde(
        default = "default_lang",
        serialize_with = "serialize_lang",
        deserialize_with = "deserialize_lang"
    )]
    pub lang: crate::i18n::Lang,
    /// Whether the bottom timeline strip is collapsed.
    #[serde(default)]
    pub timeline_collapsed: bool,
    /// The timeline's chip width in points (Roadmap
    /// Wave 1, RM13 — P-1): the persisted mirror of the
    /// bin-side `FrameOpsState::timeline_zoom`. The
    /// serde default is the pre-zoom chip width; it is
    /// a literal-backed fn ([`default_timeline_zoom`])
    /// because this crate cannot name the bin-side
    /// `timeline_marks::CHIP_W` — the two must stay
    /// equal (30.0), and the bin-side zoom accessors
    /// clamp every read to the zoom range regardless,
    /// so a hand-edited file can at worst ask for the
    /// nearest in-range width.
    #[serde(default = "default_timeline_zoom")]
    pub timeline_zoom: f32,
    /// Whether the timeline follows the playhead during
    /// playback (Roadmap Wave 1, RM13 — P-1): the
    /// persisted mirror of the bin-side
    /// `FrameOpsState::timeline_follow`.
    #[serde(default)]
    pub timeline_follow: bool,
    /// Whether the left sidebar (figures / library column)
    /// is visible.
    #[serde(default = "default_true")]
    pub sidebar_visible: bool,
    /// Command keys most recently run from the command
    /// palette, most recent first. Stored here so the
    /// palette's recents survive a restart; read and
    /// written through [`palette_recents`] /
    /// [`save_palette_recents`], which preserve every
    /// other field.
    #[serde(default)]
    pub palette_recents: Vec<String>,
    /// Whether live MCP control — the editor-hosted MCP
    /// endpoint (`live_mcp` in the shell; loopback HTTP on
    /// port 32117) — starts with the editor. Read by the
    /// shell's `init`, written back by the Settings card's
    /// Live MCP toggle (the resulting state, so a failed
    /// start is never remembered as on). Defaults to OFF
    /// (Wave W, W4): the endpoint is unauthenticated, so it
    /// is explicit-enable — a prefs file that predates the
    /// field, and a mistyped value salvaged by
    /// [`salvage_layout`], both load off, and only a stored
    /// `true` the user's toggle wrote starts the bridge.
    #[serde(default)]
    pub live_mcp: bool,
    /// Width in points of the left column (figures /
    /// library), persisted so the shell's column
    /// arrangement survives a restart. Files written
    /// before the field existed load at the long-standing
    /// fixed width (300). Clamped to 220..=420 on load
    /// (see [`load_layout_in`]).
    #[serde(default = "default_col_left_w")]
    pub col_left_w: f32,
    /// Width in points of the right column (inspector),
    /// the companion to [`LayoutPrefs::col_left_w`]:
    /// defaults to the long-standing fixed width (340)
    /// and is clamped to 260..=480 on load.
    #[serde(default = "default_col_right_w")]
    pub col_right_w: f32,
}

impl Default for LayoutPrefs {
    /// The first-run layout: the Animate workspace, dark
    /// theme, English UI language, timeline expanded,
    /// sidebar visible, live MCP control off (see
    /// [`LayoutPrefs::live_mcp`] — it starts only when
    /// the user enables it), side columns at their
    /// long-standing fixed widths.
    fn default() -> Self {
        LayoutPrefs {
            workspace: "Animate".to_string(),
            dark: true,
            lang: crate::i18n::Lang::En,
            timeline_collapsed: false,
            timeline_zoom: default_timeline_zoom(),
            timeline_follow: false,
            sidebar_visible: true,
            palette_recents: Vec::new(),
            live_mcp: false,
            col_left_w: 300.0,
            col_right_w: 340.0,
        }
    }
}

/// Load the layout preferences from `./projects/.layout.json`.
/// A missing file yields the default preferences, and a
/// corrupt one is salvaged field by field (see
/// [`load_layout_in`]) — a broken prefs file must never
/// block startup.
pub fn load_layout() -> LayoutPrefs {
    load_layout_in(&config_dir())
}

/// [`load_layout`] with an explicit base directory.
///
/// The persisted column widths are clamped to their sane
/// ranges here, on load — the one place every reader
/// (startup, palette recents, the settings toggles'
/// load-modify-save) passes through — so a hand-edited
/// or corrupt width is never honoured as written. The
/// clamp also covers the non-finite cases a hand edit
/// can produce: `1e999` reaches the field as +∞, and a
/// NaN would slip straight through a bare `f32::clamp`
/// (every NaN comparison is false), so
/// [`clamp_col_width`] tests finiteness *before*
/// clamping and falls back to the field's default.
pub fn load_layout_in(dir: &Path) -> LayoutPrefs {
    let Ok(text) = std::fs::read_to_string(dir.join(LAYOUT_FILE)) else {
        return LayoutPrefs::default();
    };
    let mut prefs: LayoutPrefs = serde_json::from_str(&text)
        .unwrap_or_else(|_| salvage_layout(&text));
    prefs.col_left_w =
        clamp_col_width(prefs.col_left_w, default_col_left_w(), 220.0, 420.0);
    prefs.col_right_w =
        clamp_col_width(prefs.col_right_w, default_col_right_w(), 260.0, 480.0);
    prefs
}

/// Field-by-field salvage for a `.layout.json` the
/// whole-struct parse rejected.
///
/// Serde's struct parse is all-or-nothing: one field
/// with a mistyped value (a hand edit's `"dark": "yes"`,
/// a `"live_mcp"` written as a string) fails the whole
/// document, and the loader used to fall back to the
/// full default — silently resetting *every* preference,
/// and worse, the shell's load-modify-save writers (the
/// settings toggles, the workspace switch, palette
/// recents) would then cement that reset back to disk on
/// their next save. The salvage pass instead re-reads
/// the document as untyped JSON and adopts each field
/// that individually deserialises to its declared type,
/// defaulting only the fields that do not. A document
/// that is not JSON at all, or not a JSON object, has
/// nothing salvageable and still yields the full
/// default. Only ever called after the strict parse
/// failed, so a well-formed file's behaviour is
/// unchanged — including its serde-default handling of
/// fields the file predates.
fn salvage_layout(text: &str) -> LayoutPrefs {
    let defaults = LayoutPrefs::default();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return defaults;
    };
    let Some(obj) = value.as_object() else {
        return defaults;
    };
    fn field<T: serde::de::DeserializeOwned>(
        obj: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Option<T> {
        obj.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    LayoutPrefs {
        workspace: field(obj, "workspace").unwrap_or(defaults.workspace),
        dark: field(obj, "dark").unwrap_or(defaults.dark),
        // Not `field::<Lang>`: the persisted shape is the
        // BCP 47 code string, so salvage reads the string
        // and resolves it — a valid code survives another
        // field's mistyping, an unknown one defaults.
        lang: field::<String>(obj, "lang")
            .and_then(|code| lang_from_code(&code))
            .unwrap_or(defaults.lang),
        timeline_collapsed: field(obj, "timeline_collapsed")
            .unwrap_or(defaults.timeline_collapsed),
        timeline_zoom: field(obj, "timeline_zoom").unwrap_or(defaults.timeline_zoom),
        timeline_follow: field(obj, "timeline_follow")
            .unwrap_or(defaults.timeline_follow),
        sidebar_visible: field(obj, "sidebar_visible")
            .unwrap_or(defaults.sidebar_visible),
        palette_recents: field(obj, "palette_recents")
            .unwrap_or(defaults.palette_recents),
        live_mcp: field(obj, "live_mcp").unwrap_or(defaults.live_mcp),
        col_left_w: field(obj, "col_left_w").unwrap_or(defaults.col_left_w),
        col_right_w: field(obj, "col_right_w").unwrap_or(defaults.col_right_w),
    }
}

/// Save the layout preferences to `./projects/.layout.json`.
/// Best-effort: like the recovery writer (and the recents
/// writer) it creates the projects directory when needed,
/// and write failures are swallowed — preferences are a
/// convenience, never worth an error dialog. The write
/// itself is an atomic replace ([`write_file_atomic`]),
/// which matters here more than anywhere: this is the
/// shell's most frequently rewritten file (workspace
/// switches, settings toggles, palette recents, and one
/// save per pointer-move increment of a column-splitter
/// drag), so it has the most chances to be caught
/// mid-write by a crash or a racing save.
pub fn save_layout(prefs: &LayoutPrefs) {
    save_layout_in(&config_dir(), prefs);
}

/// [`save_layout`] with an explicit base directory.
pub fn save_layout_in(dir: &Path, prefs: &LayoutPrefs) {
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(prefs) {
            let _ = write_file_atomic(&dir.join(LAYOUT_FILE), &text);
        }
    }
}

// ---- Live MCP start record ----

/// The outcome of the most recent attempt to start (or
/// stop) the live-MCP bridge, as recorded by
/// [`note_live_mcp_start`].
///
/// Why this exists: the bridge module's start API reports
/// failure as a bare `false` and keeps no reason — the
/// bind error is swallowed at the start call — so when a
/// start fails (in practice: another editor instance
/// already holds port 32117) the only trace anywhere is
/// the settings toggle sitting in the off position, and
/// the persisted preference still says "on". That makes a
/// port-busy startup indistinguishable from a user who
/// turned live control off. This record is the shell-side
/// fact a settings surface (or a future status line) can
/// read to say *which* happened. It is session state,
/// deliberately not persisted: the preference already
/// records the user's wish across restarts, and each
/// launch makes a fresh attempt with a fresh outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveMcpStartOutcome {
    /// No start was attempted yet this session (the
    /// preference was off at launch, or nothing has
    /// touched the toggle).
    NotAttempted,
    /// The last start attempt succeeded — the bridge is
    /// listening. (The bridge's own enabled flag remains
    /// the live truth; this records the attempt's result
    /// at the moment it was made.)
    Listening,
    /// The last start attempt failed — the bridge did
    /// not come up. The start API does not say why; a
    /// port already held by another editor instance is
    /// the known cause.
    FailedToStart,
    /// Live control was on and the last action stopped
    /// it deliberately — distinct from `FailedToStart`,
    /// so a stale failure is never presented after a
    /// deliberate stop.
    Stopped,
}

/// Backing store for the start record: the outcome's
/// discriminant as a byte (`WRITE_TMP_SEQ`'s atomic
/// idiom — the record is written from press handlers and
/// read from view code, and must never need a lock).
static LIVE_MCP_START_OUTCOME: AtomicU8 = AtomicU8::new(0);

impl LiveMcpStartOutcome {
    fn to_byte(self) -> u8 {
        match self {
            LiveMcpStartOutcome::NotAttempted => 0,
            LiveMcpStartOutcome::Listening => 1,
            LiveMcpStartOutcome::FailedToStart => 2,
            LiveMcpStartOutcome::Stopped => 3,
        }
    }

    fn from_byte(byte: u8) -> LiveMcpStartOutcome {
        match byte {
            1 => LiveMcpStartOutcome::Listening,
            2 => LiveMcpStartOutcome::FailedToStart,
            3 => LiveMcpStartOutcome::Stopped,
            // 0, and anything unexpected, is "no attempt
            // recorded" — the record must never invent an
            // outcome.
            _ => LiveMcpStartOutcome::NotAttempted,
        }
    }
}

/// Record the outcome of a live-MCP start/stop attempt.
/// Callers are the shell's bridge touch-points (startup
/// init, the settings toggle): each attempt notes its
/// RESULTING state, the same discipline the toggle uses
/// when persisting `LayoutPrefs.live_mcp`.
pub fn note_live_mcp_start(outcome: LiveMcpStartOutcome) {
    LIVE_MCP_START_OUTCOME.store(outcome.to_byte(), Ordering::SeqCst);
}

/// The recorded outcome of the most recent live-MCP
/// start/stop attempt this session
/// ([`LiveMcpStartOutcome::NotAttempted`] until the
/// first one is noted).
pub fn live_mcp_start_outcome() -> LiveMcpStartOutcome {
    LiveMcpStartOutcome::from_byte(LIVE_MCP_START_OUTCOME.load(Ordering::SeqCst))
}

// ---- Command-palette recents ----

/// The command keys most recently run from the command
/// palette, most recent first, as persisted in the layout
/// preferences. An empty list when nothing has been
/// recorded yet (or the prefs file is missing or corrupt —
/// [`load_layout`] already folds those into the default
/// preferences, whose recents are empty).
pub fn palette_recents() -> Vec<String> {
    palette_recents_in(&config_dir())
}

/// [`palette_recents`] with an explicit base directory.
pub fn palette_recents_in(dir: &Path) -> Vec<String> {
    load_layout_in(dir).palette_recents
}

/// Persist the command palette's recents list into the
/// layout preferences: a load-modify-save that replaces
/// only [`LayoutPrefs::palette_recents`] and preserves
/// every other field exactly (the same pattern
/// `workspaces.rs`' `prefs_with_workspace` uses for the
/// workspace name). Best-effort, like [`save_layout`] —
/// write failures are swallowed.
pub fn save_palette_recents(recents: &[String]) {
    save_palette_recents_in(&config_dir(), recents);
}

/// [`save_palette_recents`] with an explicit base directory.
pub fn save_palette_recents_in(dir: &Path, recents: &[String]) {
    let mut prefs = load_layout_in(dir);
    prefs.palette_recents = recents.to_vec();
    save_layout_in(dir, &prefs);
}

/// Test-module note (Wave 13, E6) — the "root-clone
/// tests" oddity, pinned to source truth.
///
/// A Wave 12 side-report (D9, whose remit added
/// [`projects_dir`] above) flagged this file as carrying
/// pre-existing "root-clone" tests that would fail until
/// "shell clones" are implemented. Checked against the
/// source, that claim does not correspond to anything in
/// this file, and this note records what is actually
/// true so the claim is not chased again:
///
/// - The module below holds 28 plain `#[test]` fns.
///   There is no `#[ignore]`, no feature gate and no
///   second `cfg` anywhere in the file — `#[cfg(test)]`
///   on this module is the only conditional compilation
///   present. Nothing is being skipped.
/// - No test here names or exercises a root, a clone or
///   a "shell clone" API (a repo-wide search over `src/`
///   and `tests/` finds no such identifier). The only
///   `clone` in the module is `dir.clone()` — a
///   `PathBuf` handed to the racing writer threads in
///   `concurrent_layout_saves_never_tear_the_file`.
/// - Every function under test is defined in this file
///   (the three stores, [`salvage_layout`],
///   [`delete_project_file`], the live-MCP start record,
///   the palette-recents seam, [`projects_dir`]); the
///   only outside fixture is `Project::new_demo`. There
///   is no unimplemented dependency for a test to wait
///   on. `shell` is a library module (`pub mod shell`
///   in `lib.rs`), so these tests build into the lib
///   test binary and run in CI's `cargo test --release`
///   step like every other lib test — which is why CI
///   is green with them, not despite them.
///
/// What *would* make individual tests fail, for the
/// record: (1)
/// `live_mcp_start_outcome_records_each_attempt` relies
/// on being the only writer of the process-wide start
/// record in the LIB test binary — production code that
/// notes outcomes lives in the bin crate (a different
/// test process); a second lib-side writer, or lib
/// production code learning to note an outcome, breaks
/// its first assertion. (2)
/// `store_accessors_are_the_resolvers_own_dirs` reads
/// the real stores through both the accessors and the
/// no-dir entry points and passes because both resolve
/// through [`store_dirs`] against the same process
/// environment and working directory; it fails if an
/// accessor or a no-dir wrapper grows a second opinion
/// instead of routing through the resolver (the trap
/// [`projects_dir`]'s own doc names). (3) The atomicity
/// pair passes because every write goes through
/// [`write_file_atomic`]'s rename; reverting any store
/// to a direct `fs::write` is what would tear the
/// concurrent-saves file and litter temp files.
#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temp directory per test so parallel test runs and
    /// other agents' tests cannot collide.
    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-shell-test-{}-{test}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn store_accessors_are_the_resolvers_own_dirs() {
        // The public accessors are the resolver's own
        // answer, not a second opinion: each one is the
        // matching entry of a fresh `store_dirs()`
        // resolution against the same process state.
        let dirs = store_dirs();
        assert_eq!(projects_dir(), dirs.projects().to_path_buf());
        assert_eq!(library_dir(), dirs.library().to_path_buf());
        assert_eq!(library3d_dir(), dirs.library3d().to_path_buf());
        assert_eq!(import_dir(), dirs.import().to_path_buf());
        assert_eq!(config_dir(), dirs.config().to_path_buf());
        assert_eq!(state_dir(), dirs.state().to_path_buf());
        // Behavioural pins (read-only): the no-directory
        // entry points read the files their category
        // accessor names — recents from the state dir,
        // layout from the config dir — whichever
        // directories the test process resolves.
        assert_eq!(recents_in(&state_dir()), recents());
        assert_eq!(load_layout_in(&config_dir()), load_layout());
    }

    #[test]
    fn app_dirs_layout_splits_the_categories() {
        // Over an isolated AppDirs root the split is the
        // F-8 map exactly: documents under data in
        // same-named subfolders, prefs in config,
        // history/state in state.
        let root = temp_dir("appdirs");
        let app = rune::dirs::AppDirs::rooted(root.clone(), APP_ID)
            .expect("a rooted AppDirs always resolves");
        let dirs = StoreDirs::from_app_dirs(&app);
        let data = root.join("data").join(APP_ID);
        assert_eq!(dirs.data_root(), data.as_path());
        assert_eq!(dirs.projects(), data.join("projects").as_path());
        assert_eq!(dirs.library(), data.join("library").as_path());
        assert_eq!(dirs.library3d(), data.join("library3d").as_path());
        assert_eq!(dirs.import(), data.join("import").as_path());
        assert_eq!(dirs.config(), root.join("config").join(APP_ID).as_path());
        assert_eq!(dirs.state(), root.join("state").join(APP_ID).as_path());
        assert!(!dirs.is_portable());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn portable_layout_is_the_legacy_layout() {
        // Portable mode collapses every category onto the
        // historical working-directory paths, byte for
        // byte — the headless probes' wipe-and-run store.
        let cwd = temp_dir("portable");
        let dirs = StoreDirs::portable_at(&cwd);
        assert!(dirs.is_portable());
        assert_eq!(dirs.projects(), cwd.join("projects").as_path());
        assert_eq!(dirs.config(), cwd.join("projects").as_path());
        assert_eq!(dirs.state(), cwd.join("projects").as_path());
        assert_eq!(dirs.library(), cwd.join("library").as_path());
        assert_eq!(dirs.library3d(), cwd.join("library3d").as_path());
        assert_eq!(dirs.import(), cwd.join("import").as_path());
        // The no-home fallback takes the same shape but
        // is not a portable *request*.
        let fallback = StoreDirs::legacy_at(&cwd);
        assert_eq!(fallback.projects(), dirs.projects());
        assert!(!fallback.is_portable());
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn portable_request_truth_table() {
        use std::ffi::OsString;
        let none = |_: &str| None;
        // Nothing set: not portable.
        assert!(!portable_requested_from(Vec::new().into_iter(), none));
        // Truthy environment values request it.
        for v in ["1", "true", "TRUE", " yes ", "on"] {
            let var = |k: &str| {
                (k == PORTABLE_ENV).then(|| OsString::from(v))
            };
            assert!(
                portable_requested_from(Vec::new().into_iter(), var),
                "{v:?} should request portable mode"
            );
        }
        // Falsy or unrelated values do not.
        for v in ["0", "false", "no", "", "portable"] {
            let var = |k: &str| {
                (k == PORTABLE_ENV).then(|| OsString::from(v))
            };
            assert!(
                !portable_requested_from(Vec::new().into_iter(), var),
                "{v:?} must not request portable mode"
            );
        }
        // The flag requests it on its own.
        let args = vec![OsString::from("stykanimate"), OsString::from(PORTABLE_FLAG)];
        assert!(portable_requested_from(args.into_iter(), none));
    }

    /// A legacy working directory plus the resolved
    /// per-user dirs a migration would target, in
    /// isolated temp roots.
    fn migration_fixture() -> (PathBuf, PathBuf, StoreDirs) {
        let cwd = temp_dir("migration-cwd");
        let root = temp_dir("migration-root");
        let app = rune::dirs::AppDirs::rooted(root.clone(), APP_ID)
            .expect("a rooted AppDirs always resolves");
        let dirs = StoreDirs::from_app_dirs(&app);
        // Legacy stores: two project files, a project
        // subfolder, the three shell-owned files, and a
        // library figure.
        std::fs::create_dir_all(cwd.join("projects").join("sub")).unwrap();
        std::fs::write(cwd.join("projects").join("Alpha.json"), "alpha").unwrap();
        std::fs::write(cwd.join("projects").join("Beta.json"), "beta!").unwrap();
        std::fs::write(cwd.join("projects").join("sub").join("Deep.json"), "deep")
            .unwrap();
        std::fs::write(cwd.join("projects").join(LAYOUT_FILE), "layout").unwrap();
        std::fs::write(cwd.join("projects").join(RECENTS_FILE), "recents").unwrap();
        std::fs::write(cwd.join("projects").join(RECOVERY_FILE), "recovery")
            .unwrap();
        std::fs::create_dir_all(cwd.join("library")).unwrap();
        std::fs::write(cwd.join("library").join("Hero.json"), "hero").unwrap();
        (cwd, root, dirs)
    }

    #[test]
    fn migration_plan_routes_every_legacy_store() {
        let (cwd, root, dirs) = migration_fixture();
        let plan = legacy_migration_plan_at(&cwd, &dirs)
            .expect("legacy files with an empty data dir plan a migration");
        // 3 project-side files + 3 routed shell files + 1
        // library figure; planning wrote nothing.
        assert_eq!(plan.file_count(), 7);
        assert!(!dirs.projects().exists(), "planning must not write");
        let target = |from: &Path| {
            plan.copies()
                .iter()
                .find(|c| c.from == from)
                .map(|c| c.to.clone())
        };
        assert_eq!(
            target(&cwd.join("projects").join("Alpha.json")),
            Some(dirs.projects().join("Alpha.json"))
        );
        assert_eq!(
            target(&cwd.join("projects").join("sub").join("Deep.json")),
            Some(dirs.projects().join("sub").join("Deep.json"))
        );
        assert_eq!(
            target(&cwd.join("projects").join(LAYOUT_FILE)),
            Some(dirs.config().join(LAYOUT_FILE))
        );
        assert_eq!(
            target(&cwd.join("projects").join(RECENTS_FILE)),
            Some(dirs.state().join(RECENTS_FILE))
        );
        assert_eq!(
            target(&cwd.join("projects").join(RECOVERY_FILE)),
            Some(dirs.state().join(RECOVERY_FILE))
        );
        assert_eq!(
            target(&cwd.join("library").join("Hero.json")),
            Some(dirs.library().join("Hero.json"))
        );
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_confirm_copies_and_keeps_the_legacy_tree() {
        let (cwd, root, dirs) = migration_fixture();
        let plan = legacy_migration_plan_at(&cwd, &dirs).unwrap();
        let report = confirm_legacy_migration(&plan).unwrap();
        assert_eq!(report.copied, 7);
        assert_eq!(report.skipped_existing, 0);
        assert_eq!(report.bytes, plan.total_bytes());
        // Byte-identical at the destination…
        assert_eq!(
            std::fs::read(dirs.projects().join("Alpha.json")).unwrap(),
            b"alpha"
        );
        assert_eq!(
            std::fs::read(dirs.state().join(RECOVERY_FILE)).unwrap(),
            b"recovery"
        );
        // …and the legacy tree is untouched (copy, never
        // move): declining the new layout loses nothing.
        assert_eq!(
            std::fs::read(cwd.join("projects").join("Alpha.json")).unwrap(),
            b"alpha"
        );
        // One-time: with the migration's own output in
        // place there is nothing left to offer.
        assert!(legacy_migration_plan_at(&cwd, &dirs).is_none());
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_never_overwrites_a_destination() {
        let (cwd, root, dirs) = migration_fixture();
        let plan = legacy_migration_plan_at(&cwd, &dirs).unwrap();
        // A destination appears between plan and confirm:
        // it is skipped and reported, its content kept.
        std::fs::create_dir_all(dirs.projects()).unwrap();
        std::fs::write(dirs.projects().join("Alpha.json"), "mine").unwrap();
        let report = confirm_legacy_migration(&plan).unwrap();
        assert_eq!(report.skipped_existing, 1);
        assert_eq!(report.copied, 6);
        assert_eq!(
            std::fs::read(dirs.projects().join("Alpha.json")).unwrap(),
            b"mine"
        );
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_gate_closes_on_a_populated_data_dir() {
        let (cwd, root, dirs) = migration_fixture();
        // The user's own new work already lives in the
        // data dir (not byte-identical to any legacy
        // file): no migration is offered, ever.
        std::fs::create_dir_all(dirs.projects()).unwrap();
        std::fs::write(dirs.projects().join("New Work.json"), "new").unwrap();
        assert!(legacy_migration_plan_at(&cwd, &dirs).is_none());
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_is_never_planned_in_portable_mode() {
        let (cwd, root, _dirs) = migration_fixture();
        // Portable mode already resolves to the legacy
        // layout: there is nowhere to migrate to.
        let portable = StoreDirs::portable_at(&cwd);
        assert!(legacy_migration_plan_at(&cwd, &portable).is_none());
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn recovery_round_trips_and_clears() {
        let dir = temp_dir("recovery");
        assert!(read_recovery_in(&dir).is_none(), "nothing written yet");

        let mut project = Project::new_demo();
        project.name = "Crash Bait".into();
        let path = write_recovery_in(&dir, &project).unwrap();
        assert_eq!(path, dir.join(RECOVERY_FILE));
        assert!(path.exists());

        let back = read_recovery_in(&dir).expect("snapshot reads back");
        assert_eq!(back.name, "Crash Bait");
        assert_eq!(back.stage, project.stage);
        assert_eq!(back.figures.len(), project.figures.len());
        assert_eq!(back.frames.len(), project.frames.len());

        clear_recovery_in(&dir);
        assert!(!path.exists());
        assert!(read_recovery_in(&dir).is_none());
        // Clearing twice is fine.
        clear_recovery_in(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_in_missing_or_corrupt_dir_is_none() {
        let dir = temp_dir("recovery-bad");
        let missing = dir.join("nope");
        assert!(read_recovery_in(&missing).is_none());
        std::fs::write(dir.join(RECOVERY_FILE), "{ not a project").unwrap();
        assert!(read_recovery_in(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recents_are_most_recent_first_deduped_and_persisted() {
        let dir = temp_dir("recents");
        assert!(recents_in(&dir).is_empty());

        touch_recent_in(&dir, "Alpha");
        touch_recent_in(&dir, "Beta");
        touch_recent_in(&dir, "Alpha"); // re-touch moves it to front
        assert_eq!(recents_in(&dir), vec!["Alpha".to_string(), "Beta".to_string()]);
        // The list is a plain JSON file a fresh read can see.
        assert!(dir.join(RECENTS_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recents_are_capped_at_max() {
        let dir = temp_dir("recents-cap");
        for i in 0..(MAX_RECENTS + 2) {
            touch_recent_in(&dir, &format!("P{i}"));
        }
        let list = recents_in(&dir);
        assert_eq!(list.len(), MAX_RECENTS);
        assert_eq!(list[0], format!("P{}", MAX_RECENTS + 1));
        assert_eq!(list[MAX_RECENTS - 1], "P2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recents_with_corrupt_file_is_empty() {
        let dir = temp_dir("recents-bad");
        std::fs::write(dir.join(RECENTS_FILE), "not json").unwrap();
        assert!(recents_in(&dir).is_empty());
        // And a touch repairs it.
        touch_recent_in(&dir, "Fixed");
        assert_eq!(recents_in(&dir), vec!["Fixed".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_opened_project_is_the_recents_front() {
        let dir = temp_dir("last-opened");
        assert!(last_opened_project_in(&dir).is_none(), "no recents yet");
        touch_recent_in(&dir, "Alpha");
        touch_recent_in(&dir, "Beta");
        assert_eq!(last_opened_project_in(&dir).as_deref(), Some("Beta"));
        // Re-touching moves a project back to the front.
        touch_recent_in(&dir, "Alpha");
        assert_eq!(last_opened_project_in(&dir).as_deref(), Some("Alpha"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_newer_decision_table() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        assert!(!recovery_newer(None, None), "no snapshot, no save");
        assert!(!recovery_newer(None, Some(t0)), "no snapshot");
        assert!(recovery_newer(Some(t0), None), "snapshot, nothing saved");
        assert!(recovery_newer(Some(t1), Some(t0)), "snapshot strictly newer");
        assert!(!recovery_newer(Some(t0), Some(t1)), "save is newer");
        assert!(!recovery_newer(Some(t0), Some(t0)), "equal is not newer");
    }

    /// Pin a file's modification time (the std handle API, so
    /// the tests need no extra crates).
    fn set_mtime(path: &Path, t: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn recovery_is_newer_compares_snapshot_against_last_save() {
        let dir = temp_dir("recovery-newer");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let t2 = t1 + std::time::Duration::from_secs(60);

        // No recovery snapshot at all.
        assert!(!recovery_is_newer_than_last_project_in(&dir));

        // Snapshot exists, no project ever opened → newer.
        let snapshot = write_recovery_in(&dir, &Project::new_demo()).unwrap();
        assert!(recovery_is_newer_than_last_project_in(&dir));

        // Last save older than the snapshot → still newer.
        touch_recent_in(&dir, "Old Save");
        let saved = dir.join("old-save.json");
        std::fs::write(&saved, "{}").unwrap();
        set_mtime(&snapshot, t1);
        set_mtime(&saved, t0);
        assert!(recovery_is_newer_than_last_project_in(&dir));

        // Save catches up to the snapshot's mtime → not newer,
        // and overtaking it is not newer either.
        set_mtime(&saved, t1);
        assert!(!recovery_is_newer_than_last_project_in(&dir));
        set_mtime(&saved, t2);
        assert!(!recovery_is_newer_than_last_project_in(&dir));

        // A recorded project whose file has vanished cannot be
        // compared → false, never a startup-blocking claim.
        std::fs::remove_file(&saved).unwrap();
        assert!(!recovery_is_newer_than_last_project_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_documents_match_decision_table() {
        // Wave R (R22): the recovery gate's pure decision.
        let project = Project::new_demo();
        let pretty = serde_json::to_string_pretty(&project).unwrap();
        let compact = serde_json::to_string(&project).unwrap();
        assert!(project_documents_match(&pretty, &pretty));
        assert!(
            project_documents_match(&pretty, &compact),
            "formatting is not content"
        );
        // An older-format document that omits every
        // defaulted field parses to the all-default
        // project, so it matches that project's own
        // serialisation — and not the demo's.
        let defaulted: Project = serde_json::from_str("{}").unwrap();
        let default_text = serde_json::to_string_pretty(&defaulted).unwrap();
        assert!(project_documents_match("{}", &default_text));
        assert!(!project_documents_match("{}", &pretty));
        // One field differs → different content.
        let mut edited = Project::new_demo();
        edited.background = edited.background.wrapping_add(1);
        let edited_text = serde_json::to_string_pretty(&edited).unwrap();
        assert!(!project_documents_match(&pretty, &edited_text));
        // Unparseable proves nothing — not even against
        // itself (fail-open: the offer must stand).
        assert!(!project_documents_match("{ not a project", &pretty));
        assert!(!project_documents_match(
            "{ not a project",
            "{ not a project"
        ));
    }

    #[test]
    fn recovery_gate_suppresses_only_the_proven_spurious_offer() {
        // Wave R (R22): the content gate on
        // `recovery_is_newer_than_last_project_in`, over
        // the P42 spurious-offer shape — named save first,
        // snapshot refreshed after it (the GUI save path's
        // write order), so the snapshot is strictly newer
        // by mtime throughout this test.
        let dir = temp_dir("recovery-gate");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let mut project = Project::new_demo();
        project.name = "Gate Check".into();
        let saved = dir.join("gate-check.json");
        std::fs::write(&saved, serde_json::to_string_pretty(&project).unwrap()).unwrap();
        touch_recent_in(&dir, "Gate Check");
        let snapshot = write_recovery_in(&dir, &project).unwrap();
        set_mtime(&saved, t0);
        set_mtime(&snapshot, t1);
        assert!(
            !recovery_is_newer_than_last_project_in(&dir),
            "identical content is the clean-save refresh, not unsaved work"
        );

        // One field of real work past the save: the offer
        // stands, exactly as the mtime-only condition had it.
        project.background = project.background.wrapping_add(1);
        let snapshot = write_recovery_in(&dir, &project).unwrap();
        set_mtime(&snapshot, t1);
        assert!(recovery_is_newer_than_last_project_in(&dir));

        // A corrupt snapshot cannot be proven identical to
        // anything: fail-open, the offer stands.
        std::fs::write(&snapshot, "{ not a project").unwrap();
        set_mtime(&snapshot, t1);
        assert!(recovery_is_newer_than_last_project_in(&dir));

        // A corrupt save cannot prove the snapshot is its
        // own content either: the offer stands.
        let snapshot = write_recovery_in(&dir, &project).unwrap();
        set_mtime(&snapshot, t1);
        std::fs::write(&saved, "{ not a project").unwrap();
        set_mtime(&saved, t0);
        assert!(recovery_is_newer_than_last_project_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_gate_compares_content_not_bytes() {
        // Wave R (R22): the snapshot as compact JSON against
        // the pretty-printed save — different bytes, same
        // project (another writer's formatting, a hand
        // edit that only reflowed the file) — is still the
        // spurious offer, and is still suppressed.
        let dir = temp_dir("recovery-gate-format");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let mut project = Project::new_demo();
        project.name = "Format Check".into();
        let saved = dir.join("format-check.json");
        std::fs::write(&saved, serde_json::to_string_pretty(&project).unwrap()).unwrap();
        touch_recent_in(&dir, "Format Check");
        let snapshot_path = dir.join(RECOVERY_FILE);
        std::fs::write(&snapshot_path, serde_json::to_string(&project).unwrap()).unwrap();
        set_mtime(&saved, t0);
        set_mtime(&snapshot_path, t1);
        assert_ne!(
            std::fs::read(&saved).unwrap(),
            std::fs::read(&snapshot_path).unwrap(),
            "the two files really do differ as bytes"
        );
        assert!(!recovery_is_newer_than_last_project_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The on-disk state one session of the real
    /// save → window-close path leaves for the next
    /// launch, laid down through the same writers the
    /// path itself uses (Wave S, S13):
    ///
    /// * the named save — the save file's pretty bytes
    ///   under the project's slug, plus the recents
    ///   record (the two on-disk traces the panel's
    ///   `save_named_pressed` leaves beside the
    ///   snapshot);
    /// * the close-flush's snapshot — written with
    ///   [`write_recovery_in`], which is the flush's own
    ///   write body with the directory made explicit:
    ///   `EditorCore::flush_recovery_on_close` (editor.rs,
    ///   wired to the window's exit hooks in main.rs)
    ///   writes through `crate::shell::write_recovery`,
    ///   and that is `write_recovery_in` at
    ///   [`state_dir`]. Only the directory differs.
    ///
    /// The mtimes are pinned in the path's real order —
    /// save first, flush at close strictly after — so the
    /// mtime half of the verdict fires in every fixture
    /// below and only the content gate can suppress an
    /// offer. (The flush itself is not driven from this
    /// module: it writes only to the resolved state dir,
    /// and redirecting it would take a process-global
    /// `set_current_dir` inside the shared lib-test
    /// process. P19 pinned the flush's own behaviour —
    /// dirty writes the current project, clean writes
    /// nothing — in a dedicated integration binary,
    /// tests/wave_p_close_guard.rs; these pins cover the
    /// half that file cannot: the next launch's verdict
    /// on the state the flush leaves.)
    fn save_then_close_flush(
        dir: &Path,
        saved_project: &Project,
        flushed_project: &Project,
    ) -> (PathBuf, PathBuf) {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let saved = dir.join(format!("{}.json", project_file_slug(&saved_project.name)));
        std::fs::write(
            &saved,
            serde_json::to_string_pretty(saved_project).unwrap(),
        )
        .unwrap();
        touch_recent_in(dir, &saved_project.name);
        let snapshot = write_recovery_in(dir, flushed_project).unwrap();
        set_mtime(&saved, t0);
        set_mtime(&snapshot, t1);
        (saved, snapshot)
    }

    #[test]
    fn close_flush_of_the_saved_project_is_not_offered_at_next_launch() {
        // Wave S (S13): the close-flush × recovery-gate
        // interaction — the exact P42 spurious-offer path
        // through the flush. The edit counter counts
        // edits, never content: an edit made after the
        // save and then undone returns the project to
        // exactly the saved content while the counter
        // still stands (`edit_count` is only ever
        // incremented or reset at a project swap), so the
        // window-close flush fires and writes a snapshot
        // OF THE JUST-SAVED PROJECT — strictly newer than
        // the save by mtime. Before the gate, the next
        // launch offered it as crash salvage.
        let dir = temp_dir("close-flush-identical");
        let mut project = Project::new_demo();
        project.name = "Flush Check".into();
        let (saved, snapshot) = save_then_close_flush(&dir, &project, &project);

        // The fixture really is the flush shape: the
        // snapshot is the saved project, readable back,
        // and strictly newer by mtime — the mtime-only
        // verdict would offer it.
        let flushed = read_recovery_in(&dir).expect("the flush wrote a snapshot");
        assert_eq!(flushed.name, "Flush Check");
        assert_eq!(flushed.background, project.background);
        assert!(
            recovery_newer(file_mtime(&snapshot), file_mtime(&saved)),
            "the snapshot must be strictly newer for this pin to mean anything"
        );

        // The gate: identical content is the close-flush
        // of saved work, not unsaved work — no offer.
        assert!(
            !recovery_is_newer_than_last_project_in(&dir),
            "a close-flush snapshot of the just-saved project must not be offered"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn close_flush_capturing_unsaved_work_is_offered_at_next_launch() {
        // Wave S (S13): the flush's other face — the
        // reason it exists. The user edited after the
        // save and closed without saving again; the
        // close-flush captured that work into the
        // snapshot. The next launch MUST offer it: the
        // gate suppresses only what it can prove is the
        // save's own content.
        let dir = temp_dir("close-flush-unsaved");
        let mut project = Project::new_demo();
        project.name = "Flush Check".into();
        let mut unsaved = project.clone();
        unsaved.background = unsaved.background.wrapping_add(1);
        let (_saved, _snapshot) = save_then_close_flush(&dir, &project, &unsaved);

        let flushed = read_recovery_in(&dir).expect("the flush wrote a snapshot");
        assert_eq!(
            flushed.background, unsaved.background,
            "the snapshot holds the unsaved edit, not the save"
        );
        assert!(
            recovery_is_newer_than_last_project_in(&dir),
            "work past the save, captured by the close-flush, must be offered"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn close_flush_snapshot_corrupted_after_close_still_offers() {
        // Wave S (S13): the flush wrote a good snapshot
        // of the saved project, but the file was damaged
        // after the close (torn write, disk fault, a hand
        // edit). Nothing about it can be proven identical
        // to the save, so the gate fails open through the
        // real path: the offer stands.
        let dir = temp_dir("close-flush-corrupt");
        let mut project = Project::new_demo();
        project.name = "Flush Check".into();
        let (_saved, snapshot) = save_then_close_flush(&dir, &project, &project);
        std::fs::write(&snapshot, "{ not a project").unwrap();
        let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_120);
        set_mtime(&snapshot, t1);
        assert!(
            recovery_is_newer_than_last_project_in(&dir),
            "a corrupt snapshot proves nothing — the offer must stand"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discard_after_close_flush_offer_deletes_the_snapshot() {
        // Wave S (S13): Discard on the offered state,
        // through this path. The Discard command's body
        // at this seam is [`clear_recovery_in`] (the Home
        // banner's `discard_recovery_in` wraps exactly
        // it, plus an existence read-back): it deletes
        // the snapshot file, and with the file gone the
        // verdict has nothing left to offer.
        let dir = temp_dir("close-flush-discard");
        let mut project = Project::new_demo();
        project.name = "Flush Check".into();
        let mut unsaved = project.clone();
        unsaved.background = unsaved.background.wrapping_add(1);
        let (_saved, snapshot) = save_then_close_flush(&dir, &project, &unsaved);
        assert!(
            recovery_is_newer_than_last_project_in(&dir),
            "the offer is live before the discard"
        );

        clear_recovery_in(&dir);
        assert!(!snapshot.exists(), "discard deleted the snapshot file");
        assert!(read_recovery_in(&dir).is_none());
        assert!(
            !recovery_is_newer_than_last_project_in(&dir),
            "no snapshot, no offer"
        );
        // Discarding again is inert, never an error.
        clear_recovery_in(&dir);
        assert!(!recovery_is_newer_than_last_project_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_defaults_when_file_is_missing() {
        let dir = temp_dir("layout-missing");
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs, LayoutPrefs::default());
        // The default itself is the first-run layout.
        assert_eq!(prefs.workspace, "Animate");
        assert!(prefs.dark);
        // A directory that does not exist at all is also just
        // "no prefs yet".
        assert_eq!(
            load_layout_in(&dir.join("nope")),
            LayoutPrefs::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_round_trips_workspace_and_theme() {
        let dir = temp_dir("layout-roundtrip");
        let prefs = LayoutPrefs {
            workspace: "Draw".to_string(),
            dark: false,
            ..Default::default()
        };
        save_layout_in(&dir, &prefs);
        let path = dir.join(LAYOUT_FILE);
        assert!(path.exists());
        // The on-disk shape is the plain field-named JSON.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"workspace\": \"Draw\""), "{text}");
        assert!(text.contains("\"dark\": false"), "{text}");
        assert_eq!(load_layout_in(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_corrupt_file_yields_the_default() {
        let dir = temp_dir("layout-corrupt");
        std::fs::write(dir.join(LAYOUT_FILE), "{ not layout prefs").unwrap();
        assert_eq!(load_layout_in(&dir), LayoutPrefs::default());
        // Valid JSON of the wrong shape is corrupt for us too.
        std::fs::write(dir.join(LAYOUT_FILE), "[1, 2, 3]").unwrap();
        assert_eq!(load_layout_in(&dir), LayoutPrefs::default());
        // And a save repairs it.
        let prefs = LayoutPrefs {
            workspace: "Audio".to_string(),
            dark: true,
            ..Default::default()
        };
        save_layout_in(&dir, &prefs);
        assert_eq!(load_layout_in(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_save_twice_overwrites() {
        let dir = temp_dir("layout-overwrite");
        save_layout_in(
            &dir,
            &LayoutPrefs {
                workspace: "Animate".to_string(),
                dark: true,
                ..Default::default()
            },
        );
        let second = LayoutPrefs {
            workspace: "3D".to_string(),
            dark: false,
            ..Default::default()
        };
        save_layout_in(&dir, &second);
        assert_eq!(load_layout_in(&dir), second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_old_format_file_loads_with_new_field_defaults() {
        let dir = temp_dir("layout-old-format");
        // A `.layout.json` written before the timeline /
        // sidebar fields existed: only the original two keys.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":false}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.workspace, "Draw");
        assert!(!prefs.dark);
        // The new fields take their defaults, they do not
        // make the whole file read as corrupt.
        assert!(!prefs.timeline_collapsed);
        assert!(prefs.sidebar_visible);
        // Live MCP defaults OFF for a pre-field file (the
        // serde default is bool's own `false` — explicit
        // enable only, Wave W W4; the sidebar above keeps
        // its `default_true`).
        assert!(!prefs.live_mcp);
        // Column widths likewise default to the
        // long-standing fixed widths (300 / 340) for a
        // pre-field file.
        assert_eq!(prefs.col_left_w, 300.0);
        assert_eq!(prefs.col_right_w, 340.0);
        assert_eq!(
            prefs,
            LayoutPrefs {
                workspace: "Draw".to_string(),
                dark: false,
                ..Default::default()
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_mcp_defaults_off_until_explicitly_enabled() {
        // Wave W (W4): live control is explicit-enable. The
        // endpoint is unauthenticated (see live_mcp's module
        // docs), so no default anywhere in the load path may
        // turn it on: not the struct default, not a missing
        // prefs file, not a file written before the field
        // existed. A stored `true` — the user's own toggle
        // choice — is honoured as written.
        assert!(
            !LayoutPrefs::default().live_mcp,
            "the first-run default is off"
        );
        let dir = temp_dir("layout-live-mcp-default");
        // No prefs file at all (fresh install).
        assert!(
            !load_layout_in(&dir).live_mcp,
            "a missing prefs file loads off"
        );
        // A pre-field file (written before `live_mcp`
        // existed) holds no stored choice: off.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":false}",
        )
        .unwrap();
        assert!(
            !load_layout_in(&dir).live_mcp,
            "a pre-field prefs file loads off"
        );
        // An explicit stored choice is honoured.
        std::fs::write(dir.join(LAYOUT_FILE), "{\"live_mcp\":true}").unwrap();
        assert!(
            load_layout_in(&dir).live_mcp,
            "an explicitly enabled pref loads on"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_round_trips_timeline_and_sidebar_fields() {
        let dir = temp_dir("layout-roundtrip-panels");
        let prefs = LayoutPrefs {
            workspace: "Graph".to_string(),
            dark: true,
            lang: crate::i18n::Lang::Bn,
            timeline_collapsed: true,
            timeline_zoom: 30.0,
            timeline_follow: false,
            sidebar_visible: false,
            palette_recents: Vec::new(),
            live_mcp: false,
            col_left_w: 276.0,
            col_right_w: 316.0,
        };
        save_layout_in(&dir, &prefs);
        // The non-default values are really on disk, not
        // just reconstructed from defaults on load.
        let text = std::fs::read_to_string(dir.join(LAYOUT_FILE)).unwrap();
        assert!(text.contains("\"lang\": \"bn\""), "{text}");
        assert!(text.contains("\"timeline_collapsed\": true"), "{text}");
        assert!(text.contains("\"sidebar_visible\": false"), "{text}");
        assert!(text.contains("\"live_mcp\": false"), "{text}");
        assert!(text.contains("\"col_left_w\": 276.0"), "{text}");
        assert!(text.contains("\"col_right_w\": 316.0"), "{text}");
        assert_eq!(load_layout_in(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_lang_persists_across_a_reload() {
        // Roadmap F-9 (A25 R1): the language is a real
        // persisted preference, not session state. A user
        // picks Bengali in the Settings card; `poll`'s
        // write-back stores the choice (the save half of
        // this test), and the next launch's `init` seeds
        // `core.lang` from the reloaded prefs (the load
        // half) — so the UI comes back in Bengali instead
        // of resetting to English on every launch, which
        // is what happened before the field existed.
        let dir = temp_dir("layout-lang-roundtrip");
        // Struct default and missing file: English.
        assert_eq!(LayoutPrefs::default().lang, crate::i18n::Lang::En);
        assert_eq!(load_layout_in(&dir).lang, crate::i18n::Lang::En);
        // The choice as written: the BCP 47 code on disk.
        let prefs = LayoutPrefs {
            lang: crate::i18n::Lang::Bn,
            ..Default::default()
        };
        save_layout_in(&dir, &prefs);
        let text = std::fs::read_to_string(dir.join(LAYOUT_FILE)).unwrap();
        assert!(text.contains("\"lang\": \"bn\""), "{text}");
        // Re-init: fresh prefs load carries the language
        // the seeding in `main.rs`'s init consumes.
        assert_eq!(load_layout_in(&dir), prefs);
        assert_eq!(load_layout_in(&dir).lang, crate::i18n::Lang::Bn);
        // Codes resolve case-insensitively, as the shell
        // settings tools resolve them.
        std::fs::write(dir.join(LAYOUT_FILE), "{\"lang\":\"PT-br\"}").unwrap();
        assert_eq!(load_layout_in(&dir).lang, crate::i18n::Lang::PtBr);
        // A file written before the field existed loads
        // as English — the serde default, not corruption.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":false}",
        )
        .unwrap();
        assert_eq!(load_layout_in(&dir).lang, crate::i18n::Lang::En);
        // An unknown code (a hand edit's typo) defaults
        // just this field through the salvage pass; the
        // valid neighbours survive.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":\"yes\",\"lang\":\"xx\"}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.lang, crate::i18n::Lang::En);
        assert_eq!(prefs.workspace, "Draw");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_column_widths_are_clamped_on_load() {
        let dir = temp_dir("layout-col-clamp");
        // Out-of-range persisted widths (hand-edited or
        // corrupt) are pulled back to the sane range by
        // the load: left 220..=420, right 260..=480.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":true,\
             \"col_left_w\":999.0,\"col_right_w\":10.0}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.col_left_w, 420.0);
        assert_eq!(prefs.col_right_w, 260.0);
        // The other direction clamps the other way.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":true,\
             \"col_left_w\":100.0,\"col_right_w\":900.0}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.col_left_w, 220.0);
        assert_eq!(prefs.col_right_w, 480.0);
        // In-range widths — including the boundaries
        // themselves — pass through untouched.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":true,\
             \"col_left_w\":220.0,\"col_right_w\":480.0}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.col_left_w, 220.0);
        assert_eq!(prefs.col_right_w, 480.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clamp_col_width_rejects_non_finite_values() {
        // The unit-level pin for the NaN trap: a bare
        // `f32::clamp` cannot catch NaN (every comparison
        // with NaN is false, so clamp returns its input),
        // which is why the helper tests finiteness first.
        // NaN and both infinities fall back to the
        // field's default, never propagate into layout.
        assert_eq!(clamp_col_width(f32::NAN, 300.0, 220.0, 420.0), 300.0);
        assert_eq!(
            clamp_col_width(f32::INFINITY, 300.0, 220.0, 420.0),
            300.0
        );
        assert_eq!(
            clamp_col_width(f32::NEG_INFINITY, 340.0, 260.0, 480.0),
            340.0
        );
        // Finite values still clamp/pass through as
        // before — the finiteness check changed nothing
        // for them.
        assert_eq!(clamp_col_width(276.0, 300.0, 220.0, 420.0), 276.0);
        assert_eq!(clamp_col_width(999.0, 300.0, 220.0, 420.0), 420.0);
        assert_eq!(clamp_col_width(10.0, 340.0, 260.0, 480.0), 260.0);
    }

    #[test]
    fn layout_non_finite_widths_in_file_fall_back_to_defaults() {
        let dir = temp_dir("layout-col-nonfinite");
        // A hand-edited file can carry a width JSON
        // itself cannot spell as NaN but serde reads as
        // an infinity: the overflowing exponent literal.
        // (If a future serde instead rejected the whole
        // document, the load's corrupt-file fallback
        // lands on the same defaults — either way the
        // widths must come back 300 / 340, never ∞.)
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":true,\
             \"col_left_w\":1e999,\"col_right_w\":-1e999}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.col_left_w, 300.0);
        assert_eq!(prefs.col_right_w, 340.0);
        assert!(prefs.col_left_w.is_finite());
        assert!(prefs.col_right_w.is_finite());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Names of every entry in `dir` (dotfiles included),
    /// for the temp-litter assertions.
    fn entry_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn atomic_writes_leave_no_temp_files_behind() {
        let dir = temp_dir("atomic-no-litter");
        // One write through each of the module's three
        // writers — all of them go through the atomic
        // replace, so after each completes its temp file
        // must be gone (renamed), not littering the
        // projects dir.
        save_layout_in(&dir, &LayoutPrefs::default());
        write_recovery_in(&dir, &Project::new_demo()).unwrap();
        touch_recent_in(&dir, "Alpha");
        let names = entry_names(&dir);
        assert!(
            names.iter().all(|n| !n.contains(".tmp-")),
            "no temp litter after atomic writes: {names:?}"
        );
        assert_eq!(
            names,
            vec![
                LAYOUT_FILE.to_string(),
                RECOVERY_FILE.to_string(),
                RECENTS_FILE.to_string()
            ],
            "exactly the three store files exist"
        );
        // And each store still reads back what was
        // written through the new path.
        assert_eq!(load_layout_in(&dir), LayoutPrefs::default());
        assert!(read_recovery_in(&dir).is_some());
        assert_eq!(recents_in(&dir), vec!["Alpha".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_layout_saves_never_tear_the_file() {
        let dir = temp_dir("layout-concurrent");
        // The prefs file's writers are all load-modify-
        // save over one shared file (workspace switch,
        // settings toggles, splitter drags, palette
        // recents). Four writers racing 50 saves each:
        // with the old direct `fs::write` a reader could
        // catch the file mid-truncate; with the atomic
        // replace the file on disk is always SOME
        // writer's complete document.
        let names = ["Alpha", "Beta", "Gamma", "Delta"];
        let mut handles = Vec::new();
        for name in names {
            let dir = dir.clone();
            handles.push(std::thread::spawn(move || {
                let prefs = LayoutPrefs {
                    workspace: name.to_string(),
                    ..Default::default()
                };
                for _ in 0..50 {
                    save_layout_in(&dir, &prefs);
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        // Strict read of the raw bytes (not the lenient
        // loader): the surviving document must parse as
        // a whole LayoutPrefs...
        let text = std::fs::read_to_string(dir.join(LAYOUT_FILE)).unwrap();
        let parsed: LayoutPrefs = serde_json::from_str(&text)
            .expect("the surviving prefs file is never torn");
        // ...and be one writer's values — a torn file
        // that happened to parse, or the corrupt-file
        // default ("Animate"), fails here.
        assert!(
            names.contains(&parsed.workspace.as_str()),
            "surviving prefs are one writer's complete save: {parsed:?}"
        );
        // No temp litter from the race either.
        assert!(
            entry_names(&dir)
                .iter()
                .all(|n| !n.contains(".tmp-")),
            "no temp litter after racing saves"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn palette_recents_round_trip_and_preserve_other_fields() {
        let dir = temp_dir("palette-recents-roundtrip");
        // Distinct values in every other field first, so the
        // load-modify-save has something to lose.
        let prefs = LayoutPrefs {
            workspace: "Effects".to_string(),
            dark: false,
            lang: crate::i18n::Lang::De,
            timeline_collapsed: true,
            timeline_zoom: 30.0,
            timeline_follow: false,
            sidebar_visible: false,
            palette_recents: Vec::new(),
            live_mcp: false,
            col_left_w: 252.0,
            col_right_w: 388.0,
        };
        save_layout_in(&dir, &prefs);

        let keys = vec![
            "go-export".to_string(),
            "toggle-loop-playback".to_string(),
            "add-frame".to_string(),
        ];
        save_palette_recents_in(&dir, &keys);
        assert_eq!(palette_recents_in(&dir), keys);
        // The recents really are on disk inside the layout
        // file, not just reconstructed on load.
        let text = std::fs::read_to_string(dir.join(LAYOUT_FILE)).unwrap();
        assert!(text.contains("\"palette_recents\""), "{text}");
        assert!(text.contains("\"go-export\""), "{text}");

        // Every other field survived the save untouched.
        let after = load_layout_in(&dir);
        assert_eq!(after.workspace, "Effects");
        assert!(!after.dark);
        assert!(after.timeline_collapsed);
        assert!(!after.sidebar_visible);
        assert!(!after.live_mcp);
        assert_eq!(after.col_left_w, 252.0);
        assert_eq!(after.col_right_w, 388.0);
        assert_eq!(
            after,
            LayoutPrefs {
                palette_recents: keys,
                ..prefs
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn palette_recents_missing_file_is_empty() {
        let dir = temp_dir("palette-recents-missing");
        assert!(palette_recents_in(&dir).is_empty());
        // A directory that does not exist at all is also
        // just "nothing recorded yet".
        assert!(palette_recents_in(&dir.join("nope")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_without_palette_recents_field_loads_with_empty_recents() {
        let dir = temp_dir("layout-no-palette-recents");
        // A `.layout.json` written before the palette
        // recents field existed: every other field present,
        // no `palette_recents` key at all.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":false,\
             \"timeline_collapsed\":true,\"sidebar_visible\":false}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        assert_eq!(prefs.workspace, "Draw");
        assert!(!prefs.dark);
        assert!(prefs.timeline_collapsed);
        assert!(!prefs.sidebar_visible);
        // The missing field takes its default — an empty
        // list — instead of making the file read as corrupt.
        assert!(prefs.palette_recents.is_empty());
        assert!(palette_recents_in(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_project_file_removes_a_real_project_file() {
        let dir = temp_dir("delete-project");
        let path = dir.join("my-film.json");
        std::fs::write(&path, "{}").unwrap();
        assert!(path.exists());
        delete_project_file(&path).unwrap();
        assert!(!path.exists(), "the file is gone after deletion");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_project_file_refuses_every_shell_owned_name() {
        let dir = temp_dir("delete-shell-owned");
        for name in [LAYOUT_FILE, RECOVERY_FILE, RECENTS_FILE] {
            let path = dir.join(name);
            std::fs::write(&path, "{}").unwrap();
            let err = delete_project_file(&path)
                .expect_err("a shell-owned file must be refused");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{name}");
            assert!(path.exists(), "{name} must remain on disk");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_project_file_missing_path_is_not_found_not_a_panic() {
        let dir = temp_dir("delete-missing");
        let path = dir.join("never-saved.json");
        let err = delete_project_file(&path)
            .expect_err("a missing file must be an error");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_one_bad_field_does_not_reset_the_others() {
        let dir = temp_dir("layout-salvage");
        // A hand-edited file with two mistyped values
        // (`sidebar_visible` and `live_mcp` as strings)
        // among valid ones. The whole-struct parse
        // rejects this document outright; before the
        // salvage pass the load fell back to the FULL
        // default, silently resetting the workspace,
        // theme and widths along with the bad fields —
        // and a load-modify-save writer (a settings
        // toggle, a workspace switch) would then cement
        // that reset back to disk.
        std::fs::write(
            dir.join(LAYOUT_FILE),
            "{\"workspace\":\"Draw\",\"dark\":false,\
             \"timeline_collapsed\":true,\"sidebar_visible\":\"yes\",\
             \"live_mcp\":\"off\",\"col_left_w\":252.0}",
        )
        .unwrap();
        let prefs = load_layout_in(&dir);
        // Every valid field survives...
        assert_eq!(prefs.workspace, "Draw");
        assert!(!prefs.dark);
        assert!(prefs.timeline_collapsed);
        assert_eq!(prefs.col_left_w, 252.0);
        // ...and only the mistyped fields take defaults.
        assert!(prefs.sidebar_visible, "mistyped bool defaults on");
        assert!(!prefs.live_mcp, "mistyped bool defaults off");
        // A missing required field is salvaged the same
        // way: `dark` alone keeps its value instead of
        // condemning the file.
        std::fs::write(dir.join(LAYOUT_FILE), "{\"dark\":false}").unwrap();
        let prefs = load_layout_in(&dir);
        assert!(!prefs.dark);
        assert_eq!(prefs.workspace, "Animate", "missing field defaults");
        // But a document that is not a JSON object still
        // has nothing to salvage.
        std::fs::write(dir.join(LAYOUT_FILE), "[1, 2, 3]").unwrap();
        assert_eq!(load_layout_in(&dir), LayoutPrefs::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_mcp_start_outcome_records_each_attempt() {
        // The record is one process-wide atomic; this is
        // the only test that touches it, and its writes
        // are sequential, so no cross-test coordination
        // is needed. (The value a fresh process starts
        // with is NotAttempted by construction — the
        // static's initial byte is 0 — which the first
        // assertion relies on only if no earlier code in
        // this process noted an outcome; nothing in the
        // library does, by design: noting is the shell
        // glue's job.)
        assert_eq!(
            live_mcp_start_outcome(),
            LiveMcpStartOutcome::NotAttempted
        );
        note_live_mcp_start(LiveMcpStartOutcome::FailedToStart);
        assert_eq!(
            live_mcp_start_outcome(),
            LiveMcpStartOutcome::FailedToStart
        );
        // A later successful attempt replaces the failure
        // — the record always names the MOST RECENT one.
        note_live_mcp_start(LiveMcpStartOutcome::Listening);
        assert_eq!(live_mcp_start_outcome(), LiveMcpStartOutcome::Listening);
        // And a deliberate stop clears the stage for a
        // truthful "off" instead of a stale outcome.
        note_live_mcp_start(LiveMcpStartOutcome::Stopped);
        assert_eq!(live_mcp_start_outcome(), LiveMcpStartOutcome::Stopped);
    }
}
