//! Project library backend: scan, load, save, rename, duplicate,
//! filter and thumbnail the named projects stored as serde JSON
//! under `./projects/`.
//!
//! The file convention matches the editor's named save
//! (`editor.rs::save_named_project`): one `<slug>.json` per project,
//! where the slug is the project name lowercased with every
//! non-alphanumeric character replaced by `-`.

use crate::editor::EditorCore;
use crate::export::render_frame_at;
use crate::model::Project;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// List every parseable project in `dir`, newest modified first.
/// Files that cannot be read or parsed as a [`Project`] are
/// skipped silently; a missing directory yields an empty list.
pub fn scan(dir: &Path) -> Vec<ProjectInfo> {
    let mut infos: Vec<ProjectInfo> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return infos;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|x| x != "json").unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(project) = serde_json::from_str::<Project>(&text) else {
            continue;
        };
        let modified = entry.metadata().and_then(|m| m.modified()).ok();
        infos.push(ProjectInfo {
            name: project.name,
            figures: project.figures.len(),
            frames: project.frames.len(),
            fps: project.fps,
            path,
            modified,
        });
    }
    infos.sort_by(|a, b| b.modified.cmp(&a.modified));
    infos
}

/// Load a project from a JSON file.
pub fn load(path: &Path) -> std::io::Result<Project> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(std::io::Error::other)
}

/// Save `project` into `dir` under its name slug (creating the
/// directory if needed) and return the written path.
pub fn save(project: &Project, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", slug(&project.name)));
    let text = serde_json::to_string_pretty(project).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
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
    let stem = slug(new_name);
    if stem.is_empty() {
        return Err("project name produces an empty file name".to_string());
    }
    let path = dir.join(format!("{stem}.json"));
    if path.exists() {
        return Err(format!(
            "project file already exists: {}",
            path.display()
        ));
    }
    let mut copy = core.project.clone();
    copy.name = new_name.to_string();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&copy).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
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
pub fn rename_project(path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    let stem = slug(new_name);
    if stem.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project name produces an empty file name",
        ));
    }
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let new_path = dir.join(format!("{stem}.json"));
    if new_path == path.to_path_buf() {
        return Ok(new_path);
    }
    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("project file already exists: {}", new_path.display()),
        ));
    }
    std::fs::rename(path, &new_path)?;
    Ok(new_path)
}

/// Copy the project file at `path` to a sibling file named
/// `<stem>-copy.json` and return the copy's path. If that name is
/// taken, the suffix increments: `<stem>-copy-2.json`,
/// `<stem>-copy-3.json`, and so on. The copy's bytes are identical
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
    let mut n = 1u32;
    loop {
        let file_name = if n == 1 {
            format!("{stem}-copy.json")
        } else {
            format!("{stem}-copy-{n}.json")
        };
        let candidate = dir.join(file_name);
        if !candidate.exists() {
            std::fs::copy(path, &candidate)?;
            return Ok(candidate);
        }
        n += 1;
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
}
