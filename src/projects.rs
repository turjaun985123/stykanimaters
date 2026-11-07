//! Project library backend: scan, load, save and thumbnail the
//! named projects stored as serde JSON under `./projects/`.
//!
//! The file convention matches the editor's named save
//! (`editor.rs::save_named_project`): one `<slug>.json` per project,
//! where the slug is the project name lowercased with every
//! non-alphanumeric character replaced by `-`.

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
}
