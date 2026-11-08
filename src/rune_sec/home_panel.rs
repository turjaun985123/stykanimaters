//! Home / projects panel (Phase D, section 21/40 of the Rune
//! rebuild): the saved-project library — a search field, one
//! row per project in `./projects` (name, figures, frames,
//! fps, modified), an Open and a Duplicate button per row, a
//! recovery banner when the recovery snapshot is newer than
//! the last named save, and a New project button. Declared
//! under `crate::rune_sec` — bin-crate code, not part of the
//! engine library.
//!
//! Every control calls the same real op the GPUI Home panel's
//! listener called (recovered from `git show b367357:src/app.rs`):
//!
//! - **Open** — `dispatch_open`: select the file in
//!   `EditorCore::project_files` (matched by file name — the
//!   library scan reports `./projects/…` relative paths, the
//!   core's list is cwd-absolute), then
//!   `EditorCore::open_project_file` (editor.rs:3534), then
//!   `shell::touch_recent` (shell.rs:89).
//! - **Duplicate** — `projects::duplicate_project`
//!   (projects.rs:119), status line reports the new file.
//! - **Recover** — `shell::read_recovery` (shell.rs:62) into
//!   `core.project` with the same session reset
//!   `open_project_file` performs.
//! - **New project** — there is no `EditorCore` new-project
//!   op; the button installs `Project::new_demo`
//!   (model.rs:725 — the "Untitled" starter project every
//!   `EditorCore::new` boots with) plus that same reset.
//! - **Search** — the row list is
//!   `projects::filter_projects` (projects.rs:149) over the
//!   scan, driven by a Rune core `TextEdit` in this section's
//!   state (the GPUI panel had no search; the audit's Q5 text
//!   input is what makes it possible here).
//!
//! ## The scan cache (the Wave 15 lesson)
//!
//! `projects::scan` (projects.rs:36) reads and parses *every*
//! project file, so the builder never calls it directly: it
//! goes through [`HomePanelState::projects`], which caches the
//! scan on the `projects` directory's mtime — one `stat` per
//! render, a rescan only when the directory changes (the
//! GPUI panel's `projects_cached`, Wave 15, transplanted).
//! A missing directory stats to `None`, which is itself a
//! cacheable key: `scan` of a missing dir yields an empty
//! vec, so the cached empty result stays correct until the
//! dir appears. The recovery-newer check
//! (`shell::recovery_is_newer_than_last_project`, shell.rs:177)
//! is *not* cached: it is metadata-only (two file stats plus
//! the ≤10-name recents list), the same cost class as the
//! cache key itself, and its inputs include in-place file
//! rewrites a directory mtime cannot see — caching it would
//! risk a stale banner. Instead the verdict is captured once,
//! when the panel opens (see [`toggle_open`]).
//!
//! ## Recovery verdict timing
//!
//! Opening Home snapshots the current work as the recovery
//! file (`shell::write_recovery`, shell.rs:47 — the GPUI
//! toggle did exactly this). That write makes the snapshot
//! the newest file on disk, so a verdict computed *after*
//! opening would always say "newer". [`toggle_open`]
//! therefore captures
//! `shell::recovery_is_newer_than_last_project()` *before*
//! the write and stores it in
//! [`HomePanelState::recovery_offer`] — the semantics
//! shell.rs designed the check for ("unsaved work newer than
//! your last save", judged on the state the user left behind,
//! not on the snapshot the panel itself just took).
//!
//! ## Startup decision (Phase E, section 25/40)
//!
//! At boot the shell asks this section two things:
//! [`startup_recovery`] — does the recovery snapshot hold
//! unsaved work newer than the last named save? — and
//! [`startup_status`] — the status line to show when it
//! does. `init()` builds the panel state with
//! [`initial_state`] instead of `HomePanelState::default()`:
//! on [`StartupRecovery::Offer`] the panel starts open with
//! the recovery banner already offered, so the user meets
//! the offer on the first frame instead of discovering it
//! whenever they happen to open Home.
//!
//! [`initial_state`] performs **no writes** — unlike
//! [`toggle_open`] it does not snapshot the current work,
//! because at boot the snapshot on disk *is* the evidence:
//! rewriting it before judging it would corrupt the
//! verdict. The decision is judged purely on the state the
//! user left behind, and `toggle_open`'s
//! capture-before-write behaviour for later opens is
//! unchanged.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod home_panel;
//! }
//! // in `State`:
//! pub home_panel: home_panel::HomePanelState,
//! // in `init()`:
//! home_panel: home_panel::initial_state(),
//! // in `view()`, in the shell column:
//! .child(rune_sec::home_panel::home_panel(s, &t))
//! ```

use std::cell::{Ref, RefCell};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{self, type_, Theme};
use rune::{col, row, text, text_input, El, TextEdit};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Project;
use stykanimate_rs::projects::{self, ProjectInfo};
use stykanimate_rs::shell;

use crate::chrome_view::btn;
use crate::State;

/// View-state for this section: the search field, whether
/// the panel is expanded, the recovery verdict captured at
/// open time, and the mtime-keyed scan cache. All project
/// state itself lives in `EditorCore` / on disk, as before.
#[derive(Clone, Debug, Default)]
pub struct HomePanelState {
    /// Panel expanded under the "Home" toggle. Starts
    /// open when [`initial_state`] offers a recovery.
    pub open: bool,
    /// Search box contents (Rune core text edit). The row
    /// list filters through `projects::filter_projects` on
    /// this text every render; typing is the only writer.
    pub search: TextEdit,
    /// Whether the recovery banner is offered: the
    /// `shell::recovery_is_newer_than_last_project` verdict
    /// — captured by [`initial_state`] at boot, and by
    /// [`toggle_open`] before its snapshot write on every
    /// later open (see the module docs). Cleared by a
    /// successful Recover press.
    pub recovery_offer: bool,
    /// The Wave 15 scan cache: `(projects-dir mtime, scan)`
    /// — `None` until the first render fills it. Interior
    /// mutability because the panel builder only holds
    /// `&State`, exactly as in the GPUI panel.
    scan_cache: RefCell<Option<(Option<SystemTime>, Vec<ProjectInfo>)>>,
}

impl HomePanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The saved-project library, scanned at most once per
    /// change to the `projects` directory (see the module
    /// docs). The returned borrow covers the raw scan —
    /// including the `.recovery.json` entry, which parses as
    /// a project; row building filters it out via
    /// [`visible_projects`].
    pub fn projects(&self) -> Ref<'_, Vec<ProjectInfo>> {
        let dir = projects_dir();
        let key = dir_mtime(&dir);
        {
            let cache = self.scan_cache.borrow();
            if let Some((cached_key, _)) = cache.as_ref() {
                if *cached_key == key {
                    return Ref::map(cache, |c| &c.as_ref().unwrap().1);
                }
            }
        }
        let entries = projects::scan(&dir);
        *self.scan_cache.borrow_mut() = Some((key, entries));
        Ref::map(self.scan_cache.borrow(), |c| &c.as_ref().unwrap().1)
    }
}

/// The projects directory every backend agrees on:
/// `./projects` relative to the working directory
/// (shell.rs's private `default_dir`, editor.rs's named-save
/// directory, and the GPUI panel's scan root).
pub fn projects_dir() -> PathBuf {
    PathBuf::from("projects")
}

/// A directory's modification time, swallowing every error
/// into `None` (a missing/unreadable dir is a cacheable
/// state, not a failure — `projects::scan` agrees, yielding
/// an empty list for it).
fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

// ---- Pure helpers ----

/// True for the recovery snapshot entry: `projects::scan`
/// parses `.recovery.json` as a project, but it is not a
/// library entry — the recovery banner owns it. (The GPUI
/// panel skipped it the same way.)
pub fn is_recovery_entry(info: &ProjectInfo) -> bool {
    info.path
        .file_name()
        .map(|n| n == shell::RECOVERY_FILE)
        .unwrap_or(false)
}

/// True for a file the shell itself owns inside the
/// projects dir — never a user project, so never a library
/// row. The shell keeps three files beside the saved
/// projects (shell.rs's `default_dir` is `./projects`):
/// the layout prefs (`.layout.json`), the recovery
/// snapshot (`.recovery.json`) and the recents list
/// (`recents.json`). The snapshot parses as a `Project`
/// and reaches the scan; the prefs and recents normally
/// fail the parse — but the exclusion must not depend on
/// that accident: any dotfile, and any file carrying one
/// of the shell's names, is shell-owned. (A user project
/// saved under the name "Layout" slugs to `layout.json` —
/// no dot, not a shell name — and stays listed.)
pub fn is_shell_owned_entry(info: &ProjectInfo) -> bool {
    let Some(name) = info.path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with('.')
        || name == shell::LAYOUT_FILE
        || name == shell::RECOVERY_FILE
        || name == shell::RECENTS_FILE
}

/// The rows the panel lists: library entries whose name
/// matches `query`, via `projects::filter_projects`
/// (projects.rs:149 — case-insensitive substring; empty
/// query lists all). Shell-owned files are excluded here,
/// at the single choke point every row (and the row
/// count) flows through: the recovery snapshot, which the
/// banner owns, and the shell's other files in the
/// projects dir (`.layout.json`, `recents.json`, any
/// dotfile) — the layout prefs surfaced as a ".layout"
/// row on a real screen before this filter existed.
pub fn visible_projects(entries: &[ProjectInfo], query: &str) -> Vec<ProjectInfo> {
    let library: Vec<ProjectInfo> = entries
        .iter()
        .filter(|info| !is_recovery_entry(info) && !is_shell_owned_entry(info))
        .cloned()
        .collect();
    projects::filter_projects(&library, query)
}

/// Whole-number fps prints bare (`12`), fractional keeps
/// one decimal (`23.9`).
pub fn format_fps(fps: f32) -> String {
    if (fps - fps.round()).abs() < 0.01 {
        format!("{}", fps.round() as i32)
    } else {
        format!("{fps:.1}")
    }
}

/// `"<n> figure(s)"` with correct singular.
fn count_label(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("{n} {singular}")
    } else {
        format!("{n} {plural}")
    }
}

/// A file's mtime as `YYYY-MM-DD HH:MM` in UTC, or
/// `"unknown"` when the scan could not stat the file.
pub fn format_modified(modified: Option<SystemTime>) -> String {
    let Some(t) = modified else {
        return "unknown".into();
    };
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute) = civil_from_unix(secs);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// `(year, month, day, hour, minute)` in UTC for a Unix
/// timestamp, via Howard Hinnant's civil-from-days
/// algorithm (the civil year starts in March, so January
/// and February belong to the following civil year count).
fn civil_from_unix(secs: u64) -> (i64, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    let hour = (secs_of_day / 3_600) as u32;
    let minute = ((secs_of_day % 3_600) / 60) as u32;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };
    (year, month as u32, day as u32, hour, minute)
}

/// One row's metadata line: real scan data only
/// (`ProjectInfo` figures/frames/fps/modified).
pub fn meta_line(info: &ProjectInfo) -> String {
    format!(
        "{} · {} · {} fps · modified {}",
        count_label(info.figures, "figure", "figures"),
        count_label(info.frames, "frame", "frames"),
        format_fps(info.fps),
        format_modified(info.modified)
    )
}

// ---- Dispatch: one function per control, against a bare
// ---- EditorCore so the wiring is unit-testable ----

/// The session reset `EditorCore::open_project_file`
/// performs after swapping the project (editor.rs:3534):
/// first frame, playhead home, first figure, no node.
fn reset_session(core: &mut EditorCore) {
    core.current = 0;
    core.playhead = 0.0;
    core.selected_figure = 0;
    core.selected_node = None;
}

/// Index of `path` in the core's project-file list, matched
/// by file name (the scan reports `./projects/…` relative
/// paths; the core's list is cwd-absolute — the GPUI panel
/// matched by name for the same reason).
pub fn project_file_index(files: &[PathBuf], path: &Path) -> Option<usize> {
    let fname = path.file_name()?;
    files.iter().position(|p| p.file_name() == Some(fname))
}

/// Open a library row: route through the editor's existing
/// cycle+open flow (select the matching entry in
/// `core.project_files`, adding it first if the core has
/// not listed it yet), then record the recent. The GPUI
/// panel's `open_home_project`, verbatim.
pub fn dispatch_open(core: &mut EditorCore, path: &Path, name: &str) {
    if let Some(idx) = project_file_index(&core.project_files, path) {
        core.project_index = idx;
    } else {
        core.project_files.push(path.to_path_buf());
        core.project_index = core.project_files.len() - 1;
    }
    core.open_project_file();
    shell::touch_recent(name);
}

/// Duplicate a library row's file
/// (`projects::duplicate_project`, projects.rs:119 — the
/// copy lands beside the source as `<stem>-copy.json`,
/// incrementing if taken) and report it on the status
/// line. The duplicate appears in the list on the next
/// render: the write bumps the projects-dir mtime, which
/// is the scan cache's key.
pub fn dispatch_duplicate(core: &mut EditorCore, path: &Path) {
    core.status = match projects::duplicate_project(path) {
        Ok(new_path) => format!(
            "Duplicated project: {}.",
            new_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        Err(e) => format!("Duplicate failed: {e}"),
    };
}

/// Recover the snapshot from `./projects/.recovery.json`
/// (`shell::read_recovery`, shell.rs:62) as the current
/// project, with the standard session reset. The GPUI
/// Recover button's body, verbatim.
pub fn dispatch_recover(core: &mut EditorCore) {
    match shell::read_recovery() {
        Some(project) => {
            core.project = project;
            reset_session(core);
            core.status = "Recovered the snapshot from ./projects/.recovery.json.".into();
        }
        None => {
            core.status = "No recovery snapshot found in ./projects/.".into();
        }
    }
}

/// New project: install the starter project
/// (`Project::new_demo`, model.rs:725) with the standard
/// session reset. The undo stack is left alone, as
/// `open_project_file` leaves it — undoing across the swap
/// restores the previous project, which is the behaviour
/// the GPUI shell had for opens.
pub fn dispatch_new(core: &mut EditorCore) {
    core.project = Project::new_demo();
    reset_session(core);
    core.status = "New project.".into();
}

/// Flip the panel open/closed (the "Home" toggle's
/// handler). Opening captures the recovery verdict *first*
/// and only then snapshots the current work as the
/// recovery file — in that order, or the snapshot the
/// panel itself just wrote would always read as "newer
/// unsaved work" (see the module docs).
pub fn toggle_open(s: &mut State) {
    s.home_panel.open = !s.home_panel.open;
    if s.home_panel.open {
        s.home_panel.recovery_offer = shell::recovery_is_newer_than_last_project();
        let _ = shell::write_recovery(&s.core.project);
    }
}

// ---- Startup decision (Phase E) ----

/// The boot-time recovery decision: does the recovery
/// snapshot hold unsaved work newer than the last named
/// save? The shell asks once, at startup, via
/// [`startup_recovery`]; `Offer` means the Home panel
/// should start open with its recovery banner showing
/// (see [`initial_state`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupRecovery {
    /// No newer snapshot: boot normally, Home closed.
    None,
    /// The snapshot is newer than the last named save:
    /// offer it.
    Offer,
}

impl StartupRecovery {
    /// The decision from the shell's newer-than verdict
    /// (`shell::recovery_is_newer_than_last_project`,
    /// shell.rs:177): newer → `Offer`, anything else →
    /// `None`.
    pub fn from_newer(newer: bool) -> Self {
        if newer {
            StartupRecovery::Offer
        } else {
            StartupRecovery::None
        }
    }
}

/// The status line the shell shows at boot when a
/// recovery is offered (see [`startup_status`]).
const STARTUP_RECOVERY_STATUS: &str =
    "A newer recovery file exists — open Home to restore it.";

/// The startup recovery decision for the real
/// `./projects` directory, from
/// `shell::recovery_is_newer_than_last_project`
/// (shell.rs:177). The shell check is metadata-only and
/// folds every filesystem error into `false`, so this is
/// safe to call from `init()` — it can never block
/// startup.
pub fn startup_recovery() -> StartupRecovery {
    StartupRecovery::from_newer(shell::recovery_is_newer_than_last_project())
}

/// [`startup_recovery`] with an explicit base directory
/// (the testable core; shell.rs's `_in` convention).
pub fn startup_recovery_in(dir: &Path) -> StartupRecovery {
    StartupRecovery::from_newer(shell::recovery_is_newer_than_last_project_in(dir))
}

/// The status line the shell shows at boot when
/// [`startup_recovery`] offers a snapshot: `Some` on
/// `Offer`, `None` otherwise (boot normally, no message).
pub fn startup_status() -> Option<String> {
    startup_status_for(startup_recovery())
}

/// [`startup_status`] with an explicit base directory.
pub fn startup_status_in(dir: &Path) -> Option<String> {
    startup_status_for(startup_recovery_in(dir))
}

fn startup_status_for(decision: StartupRecovery) -> Option<String> {
    match decision {
        StartupRecovery::Offer => Some(STARTUP_RECOVERY_STATUS.into()),
        StartupRecovery::None => None,
    }
}

/// The panel state `init()` should start from (instead
/// of `HomePanelState::default()`): the startup decision,
/// run once. On [`StartupRecovery::Offer`] the panel
/// starts open with the recovery banner offered, so the
/// offer is on screen from the first frame; on `None` it
/// is exactly the default state.
///
/// Unlike [`toggle_open`], this performs **no writes**:
/// it never snapshots the current project as the recovery
/// file. At boot the snapshot on disk *is* the evidence —
/// rewriting it before judging it would corrupt the
/// verdict (see the module docs).
pub fn initial_state() -> HomePanelState {
    initial_state_for(startup_recovery())
}

/// [`initial_state`] with an explicit base directory.
pub fn initial_state_in(dir: &Path) -> HomePanelState {
    initial_state_for(startup_recovery_in(dir))
}

fn initial_state_for(decision: StartupRecovery) -> HomePanelState {
    let offer = decision == StartupRecovery::Offer;
    HomePanelState {
        open: offer,
        recovery_offer: offer,
        ..HomePanelState::default()
    }
}

// ---- Elements ----

/// A small muted section heading inside the panel card.
fn section_label(t: &Theme, label: &str) -> El {
    text(label).font(type_::LABEL).color(t.text_secondary)
}

/// The recovery banner: shown only while
/// [`HomePanelState::recovery_offer`] holds (the verdict
/// captured at open time). Recover runs the real snapshot
/// load and clears the offer.
fn recovery_banner(t: &Theme) -> El {
    let accent = tok::accent(0, true);
    row()
        .key("home-recovery")
        .w_full()
        .gap(8.0)
        .center_y()
        .px(10.0)
        .py(6.0)
        .radius(8.0)
        .fill(accent.alpha(0.18))
        .child(
            text("Unsaved recovery snapshot is newer than the last save.")
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0),
        )
        .child(btn("recover", "Recover", false, t, |s: &mut State| {
            dispatch_recover(&mut s.core);
            s.home_panel.recovery_offer = false;
        }))
}

/// One project row: name + real metadata on the left,
/// Open / Duplicate on the right. Keys carry the GPUI
/// tuple ids as strings (`home-open-{i}`, `home-dup-{i}`),
/// matching the `frame-{n}` key style `chrome_view` uses.
fn project_row(info: &ProjectInfo, i: usize, t: &Theme) -> El {
    let open_key = format!("home-open-{i}");
    let dup_key = format!("home-dup-{i}");
    let path = info.path.clone();
    let dup_path = info.path.clone();
    let name = info.name.clone();
    row()
        .key(format!("home-row-{i}"))
        .w_full()
        .gap(8.0)
        .center_y()
        .px(10.0)
        .py(6.0)
        .radius(8.0)
        .fill(t.bg)
        .child(
            col()
                .gap(2.0)
                .grow(1.0)
                .child(text(info.name.clone()).font(type_::TITLE).color(t.text))
                .child(
                    text(meta_line(info))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(btn(&open_key, "Open", false, t, move |s: &mut State| {
            dispatch_open(&mut s.core, &path, &name);
        }))
        .child(btn(&dup_key, "Duplicate", false, t, move |s: &mut State| {
            dispatch_duplicate(&mut s.core, &dup_path);
        }))
}

/// The section as mounted in the shell column: a "Home"
/// toggle (accent-filled while open, the GPUI `home` id),
/// and — when open — the library card: recovery banner,
/// search field, New project, and the project rows.
pub fn home_panel(s: &State, t: &Theme) -> El {
    let home = &s.home_panel;
    let mut panel = col().key("home-panel").gap(6.0);
    panel = panel.child(btn("home", "Home", home.open, t, |s: &mut State| {
        toggle_open(s);
    }));
    if !home.open {
        return panel;
    }
    let entries = home.projects();
    let shown = visible_projects(&entries, &home.search.text);
    let mut card = col()
        .key("home-card")
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .fill(t.card);
    card = card.child(
        row()
            .key("home-header")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                text("Saved projects in ./projects")
                    .font(type_::TITLE)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(btn("home-new", "New project", false, t, |s: &mut State| {
                dispatch_new(&mut s.core);
            })),
    );
    if home.recovery_offer {
        card = card.child(recovery_banner(t));
    }
    card = card.child(
        row()
            .key("home-search-row")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                text_input(&home.search, type_::BODY.lh(26.0))
                    .key("home-search")
                    .single_line()
                    .grow(1.0)
                    .color(t.text)
                    .placeholder("Search projects", t.text_secondary)
                    .on_edit(|s: &mut State| &mut s.home_panel.search),
            )
            .child(section_label(
                t,
                &format!(
                    "{} {}",
                    shown.len(),
                    if shown.len() == 1 { "project" } else { "projects" }
                ),
            )),
    );
    if shown.is_empty() {
        card = card.child(section_label(
            t,
            if home.search.text.is_empty() {
                "No saved projects yet — use \"Save project\" to add one."
            } else {
                "No projects match the search."
            },
        ));
    } else {
        let mut rows = col().key("home-rows").gap(6.0);
        for (i, info) in shown.iter().enumerate() {
            rows = rows.child(project_row(info, i, t));
        }
        card = card.child(rows);
    }
    panel.child(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a scan entry without touching the disk.
    fn info(name: &str, path: &str) -> ProjectInfo {
        ProjectInfo {
            name: name.into(),
            path: PathBuf::from(path),
            figures: 1,
            frames: 5,
            fps: 12.0,
            modified: None,
        }
    }

    /// A unique temp directory per test so parallel test
    /// runs cannot collide (the projects.rs helper's rule).
    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-home-panel-{test}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn state_defaults_closed_empty_search_no_offer() {
        let state = HomePanelState::default();
        assert!(!state.open);
        assert!(state.search.text.is_empty());
        assert!(!state.recovery_offer);
    }

    #[test]
    fn recovery_entry_is_the_snapshot_file_only() {
        assert!(is_recovery_entry(&info(
            "Untitled",
            "projects/.recovery.json"
        )));
        assert!(!is_recovery_entry(&info("Untitled", "projects/untitled.json")));
        // A project merely *named* like the snapshot in
        // another directory is still a library entry.
        assert!(!is_recovery_entry(&info(
            "Untitled",
            "elsewhere/.recovery.json.bak"
        )));
    }

    #[test]
    fn visible_projects_drops_recovery_and_filters_by_query() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
            info("Alpha Two", "projects/alpha-two.json"),
            info("Untitled", "projects/.recovery.json"),
        ];
        let all = visible_projects(&entries, "");
        assert_eq!(all.len(), 3, "recovery snapshot is not a row");
        let filtered = visible_projects(&entries, "ALPHA");
        assert_eq!(filtered.len(), 2, "query is case-insensitive");
        assert_eq!(filtered[0].name, "Alpha");
        assert_eq!(filtered[1].name, "Alpha Two");
        assert!(visible_projects(&entries, "gamma").is_empty());
    }

    #[test]
    fn shell_owned_entry_matches_shell_files_and_dotfiles_only() {
        // The shell's own files in the projects dir.
        assert!(is_shell_owned_entry(&info(
            "Animate",
            "projects/.layout.json"
        )));
        assert!(is_shell_owned_entry(&info(
            "Untitled",
            "projects/.recovery.json"
        )));
        assert!(is_shell_owned_entry(&info(
            "recents",
            "projects/recents.json"
        )));
        // Any other dotfile is shell-owned by the same rule.
        assert!(is_shell_owned_entry(&info(
            "Hidden",
            "projects/.hidden.json"
        )));
        // Ordinary projects are not, wherever they live.
        assert!(!is_shell_owned_entry(&info(
            "Alpha",
            "projects/alpha.json"
        )));
        // A user project *named* Layout slugs to
        // `layout.json` — no dot, not a shell constant —
        // and must stay listed.
        assert!(!is_shell_owned_entry(&info(
            "Layout",
            "projects/layout.json"
        )));
        assert!(!is_shell_owned_entry(&info(
            "Recovery",
            "projects/recovery.json"
        )));
    }

    #[test]
    fn visible_projects_drops_shell_owned_files() {
        // Scan-shaped entries as the shell's own files
        // would produce them if they parsed as projects
        // (the recovery snapshot really does; the layout
        // prefs are the row the user saw on screen).
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Animate", "projects/.layout.json"),
            info("Untitled", "projects/.recovery.json"),
            info("recents", "projects/recents.json"),
            info("Beta", "projects/beta.json"),
        ];
        let all = visible_projects(&entries, "");
        assert_eq!(
            all.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Alpha", "Beta"],
            "only real projects are rows"
        );
        // A search cannot resurrect a shell file either:
        // the exclusion runs before the name filter.
        assert!(visible_projects(&entries, "recents").is_empty());
        assert!(visible_projects(&entries, "Animate").is_empty());
        // The recovery exclusion from the banner's side is
        // unchanged by the wider filter.
        assert!(is_recovery_entry(&entries[2]));
        assert_eq!(visible_projects(&entries, "Untitled").len(), 0);
    }

    #[test]
    fn format_fps_whole_and_fractional() {
        assert_eq!(format_fps(12.0), "12");
        assert_eq!(format_fps(24.0), "24");
        assert_eq!(format_fps(23.976), "24.0");
        assert_eq!(format_fps(12.5), "12.5");
    }

    #[test]
    fn format_modified_known_timestamps() {
        let at = |secs: u64| {
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs))
        };
        assert_eq!(format_modified(at(0)), "1970-01-01 00:00");
        // Leap day (2000-02-29) exercises the civil era math.
        assert_eq!(format_modified(at(951_782_400)), "2000-02-29 00:00");
        assert_eq!(format_modified(at(1_794_117_426)), "2026-11-08 05:57");
        assert_eq!(format_modified(None), "unknown");
    }

    #[test]
    fn meta_line_carries_real_metadata() {
        let mut entry = info("Demo", "projects/demo.json");
        entry.figures = 3;
        entry.frames = 12;
        entry.fps = 24.0;
        entry.modified =
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426));
        assert_eq!(
            meta_line(&entry),
            "3 figures · 12 frames · 24 fps · modified 2026-11-08 05:57"
        );
        let mut single = info("Demo", "projects/demo.json");
        single.frames = 1;
        assert!(meta_line(&single).starts_with("1 figure · 1 frame"));
    }

    #[test]
    fn project_file_index_matches_by_file_name() {
        let files = vec![
            PathBuf::from("/abs/projects/alpha.json"),
            PathBuf::from("/abs/projects/beta.json"),
        ];
        // The scan's relative path finds the core's
        // absolute entry by file name.
        assert_eq!(
            project_file_index(&files, Path::new("projects/beta.json")),
            Some(1)
        );
        assert_eq!(
            project_file_index(&files, Path::new("projects/gamma.json")),
            None
        );
    }

    #[test]
    fn dispatch_new_installs_the_demo_project_and_resets() {
        let mut core = EditorCore::new();
        core.project.name = "Something Else".into();
        core.current = 2;
        core.selected_node = Some(1);
        dispatch_new(&mut core);
        assert_eq!(core.project.name, "Untitled");
        assert_eq!(core.current, 0);
        assert_eq!(core.playhead, 0.0);
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, None);
    }

    #[test]
    fn dispatch_duplicate_copies_the_file_and_reports() {
        let dir = temp_dir("duplicate");
        let src = dir.join("demo.json");
        std::fs::write(&src, b"{}").unwrap();
        let mut core = EditorCore::new();
        dispatch_duplicate(&mut core, &src);
        assert!(dir.join("demo-copy.json").exists());
        assert_eq!(core.status, "Duplicated project: demo-copy.json.");
        // A second duplicate increments instead of clobbering.
        dispatch_duplicate(&mut core, &src);
        assert!(dir.join("demo-copy-2.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_duplicate_missing_file_reports_failure() {
        let mut core = EditorCore::new();
        dispatch_duplicate(&mut core, Path::new("projects/does-not-exist.json"));
        assert!(core.status.starts_with("Duplicate failed:"), "{}", core.status);
    }

    /// Pin a file's modification time (the std handle API,
    /// shell.rs's test pattern — no extra crates).
    fn set_mtime(path: &Path, t: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn startup_decision_maps_the_newer_verdict() {
        assert_eq!(StartupRecovery::from_newer(true), StartupRecovery::Offer);
        assert_eq!(StartupRecovery::from_newer(false), StartupRecovery::None);
        assert_eq!(
            startup_status_for(StartupRecovery::Offer).as_deref(),
            Some("A newer recovery file exists — open Home to restore it.")
        );
        assert_eq!(startup_status_for(StartupRecovery::None), None);
    }

    #[test]
    fn startup_recovery_follows_the_snapshot_age() {
        let dir = temp_dir("startup-recovery");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let t2 = t1 + std::time::Duration::from_secs(60);

        // Nothing on disk: no offer, no status line.
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::None);
        assert_eq!(startup_status_in(&dir), None);

        // A snapshot with no named save to beat: offer.
        let snapshot = shell::write_recovery_in(&dir, &Project::new_demo()).unwrap();
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::Offer);
        assert_eq!(
            startup_status_in(&dir).as_deref(),
            Some("A newer recovery file exists — open Home to restore it.")
        );

        // Last save older than the snapshot: still an offer.
        shell::touch_recent_in(&dir, "Old Save");
        let saved = dir.join("old-save.json");
        std::fs::write(&saved, "{}").unwrap();
        set_mtime(&snapshot, t1);
        set_mtime(&saved, t0);
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::Offer);

        // The save overtakes the snapshot: no offer.
        set_mtime(&saved, t2);
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::None);
        assert_eq!(startup_status_in(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn initial_state_opens_only_on_an_offer_and_writes_nothing() {
        // No snapshot: exactly the default (closed) state,
        // and the check leaves the directory untouched — a
        // boot-time write of the recovery file would
        // manufacture the very evidence being judged.
        let dir = temp_dir("startup-initial-none");
        let state = initial_state_in(&dir);
        assert!(!state.open);
        assert!(!state.recovery_offer);
        assert!(state.search.text.is_empty());
        assert!(!dir.join(shell::RECOVERY_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);

        // Newer snapshot: open, banner offered.
        let dir = temp_dir("startup-initial-offer");
        shell::write_recovery_in(&dir, &Project::new_demo()).unwrap();
        let state = initial_state_in(&dir);
        assert!(state.open);
        assert!(state.recovery_offer);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
