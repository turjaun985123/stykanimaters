//! App-shell support data: a recovery snapshot of the project
//! being edited and the most-recently-used project list.
//!
//! Both live in the projects directory (`./projects/` by
//! default): the recovery snapshot at `.recovery.json` (a full
//! [`Project`] JSON document, same format as a named save) and
//! the recents list at `recents.json` (a JSON array of project
//! names, most recent first, capped at [`MAX_RECENTS`]).
//!
//! The `*_in` variants take the directory explicitly so tests
//! (and any future settings screen) never touch the real folder.

use crate::model::Project;
use std::io;
use std::path::{Path, PathBuf};

/// File name of the recovery snapshot inside the projects dir.
pub const RECOVERY_FILE: &str = ".recovery.json";
/// File name of the recents list inside the projects dir.
pub const RECENTS_FILE: &str = "recents.json";
/// Maximum number of entries kept in the recents list.
pub const MAX_RECENTS: usize = 10;

fn default_dir() -> PathBuf {
    PathBuf::from("projects")
}

// ---- Recovery snapshot ----

/// Write `project` as the recovery snapshot in `./projects/`,
/// creating the directory if needed, and return its path.
pub fn write_recovery(project: &Project) -> io::Result<PathBuf> {
    write_recovery_in(&default_dir(), project)
}

/// [`write_recovery`] with an explicit base directory.
pub fn write_recovery_in(dir: &Path, project: &Project) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(RECOVERY_FILE);
    let text = serde_json::to_string_pretty(project).map_err(io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Read the recovery snapshot from `./projects/`, if one exists
/// and parses. A missing or corrupt file yields `None`.
pub fn read_recovery() -> Option<Project> {
    read_recovery_in(&default_dir())
}

/// [`read_recovery`] with an explicit base directory.
pub fn read_recovery_in(dir: &Path) -> Option<Project> {
    let text = std::fs::read_to_string(dir.join(RECOVERY_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Delete the recovery snapshot from `./projects/` (no error if
/// it is already gone).
pub fn clear_recovery() {
    clear_recovery_in(&default_dir());
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
    touch_recent_in(&default_dir(), name);
}

/// [`touch_recent`] with an explicit base directory.
pub fn touch_recent_in(dir: &Path, name: &str) {
    let mut list = recents_in(dir);
    list.retain(|n| n != name);
    list.insert(0, name.to_string());
    list.truncate(MAX_RECENTS);
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(dir.join(RECENTS_FILE), text);
        }
    }
}

/// The recents list from `./projects/recents.json`,
/// most recent first. Missing or corrupt file → empty list.
pub fn recents() -> Vec<String> {
    recents_in(&default_dir())
}

/// [`recents`] with an explicit base directory.
pub fn recents_in(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join(RECENTS_FILE)) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

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
}
