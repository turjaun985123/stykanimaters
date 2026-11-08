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
//!
//! On top of the two stores sit two session-continuity queries
//! for a future startup prompt: which project was open last,
//! and whether the recovery snapshot is newer than its last
//! named save.
//!
//! A third, smaller store holds the window layout
//! preferences at `.layout.json` (last workspace, theme,
//! timeline collapse, sidebar visibility) so the app shell
//! can restore them on startup. New fields are added with
//! serde defaults so older `.layout.json` files keep loading.

use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// File name of the recovery snapshot inside the projects dir.
pub const RECOVERY_FILE: &str = ".recovery.json";
/// File name of the recents list inside the projects dir.
pub const RECENTS_FILE: &str = "recents.json";
/// File name of the layout preferences inside the projects dir.
pub const LAYOUT_FILE: &str = ".layout.json";
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

// ---- Session continuity ----

/// The display name of the most recently opened project: the
/// front of the recents list. `None` when nothing has been
/// recorded yet (a missing or corrupt recents file already
/// reads as an empty list via [`recents`]). Recents store
/// display names, not paths, so the name is returned as-is.
pub fn last_opened_project() -> Option<String> {
    last_opened_project_in(&default_dir())
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

/// Whether the recovery snapshot holds work newer than the
/// last named save: `true` when a recovery file exists and
/// either no project has been opened yet or the snapshot's
/// mtime beats the most recent project's file mtime. This
/// powers a future "you have unsaved work newer than your
/// last save" startup prompt. Every filesystem error folds
/// into `false` — a recent whose file cannot be stat'ed
/// cannot be compared, so it never claims newer work — and
/// the check must never block startup.
pub fn recovery_is_newer_than_last_project() -> bool {
    recovery_is_newer_than_last_project_in(&default_dir())
}

/// [`recovery_is_newer_than_last_project`] with an explicit
/// base directory.
pub fn recovery_is_newer_than_last_project_in(dir: &Path) -> bool {
    let snapshot = file_mtime(&dir.join(RECOVERY_FILE));
    let saved = match recents_in(dir).into_iter().next() {
        // No recents: nothing saved to compare against.
        None => None,
        Some(name) => {
            let path = dir.join(format!("{}.json", project_file_slug(&name)));
            match file_mtime(&path) {
                Some(mtime) => Some(mtime),
                // A recorded project whose file is gone or
                // unreadable is an error, not "no save".
                None => return false,
            }
        }
    };
    recovery_newer(snapshot, saved)
}

// ---- Layout preferences ----

/// Serde default for boolean layout fields that default to
/// `true` (e.g. [`LayoutPrefs::sidebar_visible`]): files
/// written before the field existed load with it enabled.
fn default_true() -> bool {
    true
}

/// Window layout preferences persisted across runs: which
/// workspace was active, whether the dark theme was on, and
/// the timeline / sidebar arrangement of the context-first
/// shell. Stored as JSON at `.layout.json` in the projects
/// dir. The newer fields carry serde defaults so a
/// `.layout.json` written by an older build still loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutPrefs {
    /// Name of the active workspace (e.g. `"Animate"`).
    pub workspace: String,
    /// Whether the dark theme is enabled.
    pub dark: bool,
    /// Whether the bottom timeline strip is collapsed.
    #[serde(default)]
    pub timeline_collapsed: bool,
    /// Whether the left sidebar (figures / library column)
    /// is visible.
    #[serde(default = "default_true")]
    pub sidebar_visible: bool,
}

impl Default for LayoutPrefs {
    /// The first-run layout: the Animate workspace, dark
    /// theme, timeline expanded, sidebar visible.
    fn default() -> Self {
        LayoutPrefs {
            workspace: "Animate".to_string(),
            dark: true,
            timeline_collapsed: false,
            sidebar_visible: true,
        }
    }
}

/// Load the layout preferences from `./projects/.layout.json`.
/// A missing or corrupt file yields the default preferences —
/// a broken prefs file must never block startup.
pub fn load_layout() -> LayoutPrefs {
    load_layout_in(&default_dir())
}

/// [`load_layout`] with an explicit base directory.
pub fn load_layout_in(dir: &Path) -> LayoutPrefs {
    let Ok(text) = std::fs::read_to_string(dir.join(LAYOUT_FILE)) else {
        return LayoutPrefs::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Save the layout preferences to `./projects/.layout.json`.
/// Best-effort: like the recovery writer (and the recents
/// writer) it creates the projects directory when needed,
/// and write failures are swallowed — preferences are a
/// convenience, never worth an error dialog.
pub fn save_layout(prefs: &LayoutPrefs) {
    save_layout_in(&default_dir(), prefs);
}

/// [`save_layout`] with an explicit base directory.
pub fn save_layout_in(dir: &Path, prefs: &LayoutPrefs) {
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(dir.join(LAYOUT_FILE), text);
        }
    }
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
    fn layout_round_trips_timeline_and_sidebar_fields() {
        let dir = temp_dir("layout-roundtrip-panels");
        let prefs = LayoutPrefs {
            workspace: "Graph".to_string(),
            dark: true,
            timeline_collapsed: true,
            sidebar_visible: false,
        };
        save_layout_in(&dir, &prefs);
        // The non-default values are really on disk, not
        // just reconstructed from defaults on load.
        let text = std::fs::read_to_string(dir.join(LAYOUT_FILE)).unwrap();
        assert!(text.contains("\"timeline_collapsed\": true"), "{text}");
        assert!(text.contains("\"sidebar_visible\": false"), "{text}");
        assert_eq!(load_layout_in(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
