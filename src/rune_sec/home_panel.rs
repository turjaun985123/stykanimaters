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
//! - **Reopen last** — [`reopen_target`] resolves
//!   `shell::last_opened_project` (shell.rs:127 — the
//!   *display name* at the front of the recents list;
//!   recents store names, not paths) against the same
//!   cached scan the rows use. When the named project is
//!   still in the library and is not the file the core's
//!   selector currently points at, an accent
//!   `home-reopen-last` button ("Reopen {name}") sits at
//!   the top of the card and opens it through
//!   [`dispatch_open`] — the identical path a row's Open
//!   button takes, no parallel open logic.
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
//! - **Icons (Phase H)** — the Reopen, New and per-row Open
//!   buttons render through `icon_btn` with leading Font
//!   Awesome glyphs (`arrow-rotate-right`, `file`,
//!   `folder-open`), and the search row leads with a
//!   `magnifying-glass` `icon_el`; keys, labels, active
//!   state and handlers are unchanged.
//! - **Row metadata (Phase J)** — each row's metadata line
//!   (keyed `home-proj-meta-{i}`) is fed by
//!   [`project_metadata`]: a *light* read of the project
//!   file (a `serde_json::Value` parse counting only the
//!   top-level `figures`/`frames` arrays — never a full
//!   `Project` deserialise), cached per file on its mtime
//!   in [`HomePanelState::metadata_for`]. The scan's
//!   `ProjectInfo` stays the fallback (see
//!   [`row_meta_line`]): the scan had to fully parse every
//!   file anyway to learn its display name, so its numbers
//!   are already paid for — the light read's value is that
//!   the line's figures/frames/modified are re-verified
//!   against the file on disk whenever its mtime moves,
//!   even when the projects *directory* (the scan cache's
//!   key) has not changed.
//! - **Delete (Phase K)** — each row's `home-proj-del-{i}`
//!   button only *arms* a deletion: it records the row's
//!   path in [`HomePanelState::confirm_delete`] and the
//!   row re-renders as a confirm line ("Delete {name}?"
//!   with `home-proj-del-confirm` /
//!   `home-proj-del-cancel`). Nothing touches the disk
//!   until the confirm press runs
//!   [`dispatch_delete_confirm`], which calls
//!   `shell::delete_project_file` (Phase K, section 15 —
//!   the shell helper refuses shell-owned filenames). On
//!   success both caches are dropped wholesale
//!   ([`HomePanelState::invalidate_caches`]): the scan
//!   cache is keyed on the directory mtime and the meta
//!   cache on per-file mtimes, and a deleted file must
//!   never render as a stale row from either. On failure
//!   the confirm stays armed and the status line carries
//!   the error. Opening another row or editing the
//!   search clears a pending confirm. Deleting the file
//!   of the project that is currently open removes only
//!   the file: the in-memory project in the editor is
//!   untouched and keeps editing as an unsaved copy.
//! - **Sort (Phase K)** — the `home-sort` toggle in the
//!   search row switches the row order between the scan's
//!   own order (the default: `projects::scan` sorts by
//!   modification time, newest first — projects.rs:62)
//!   and name order ([`sort_entries`]: stable,
//!   case-insensitive). The toggle's label names the
//!   order in effect: "Sort: recent" / "Sort: A–Z".
//! - **Totals footer (Phase L)** — under the project
//!   rows (non-empty list only), a muted line keyed
//!   `home-totals` sums the *visible* rows — the list
//!   after the search filter and the sort — via
//!   [`totals_line`]: "{n} projects · {f} figures ·
//!   {m} frames". The figures/frames come from the same
//!   light reads ([`HomePanelState::metadata_for`],
//!   through the mtime meta cache) that feed the rows'
//!   own metadata lines, collected during the row loop,
//!   so the footer costs no extra disk reads. A project
//!   whose light read fails (it vanished or stopped
//!   parsing between the scan and this render) counts
//!   toward the project total with 0 figures and 0
//!   frames — it is still a listed project, and the
//!   footer never guesses its contents. Wording reuses
//!   `count_label`, the rows' own helper, so 1 is
//!   singular ("1 project · 1 figure · 1 frame") and
//!   every other count plural — decided once, there.
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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{self, type_, Theme};
use rune::{col, row, text, text_input, El, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Project;
use stykanimate_rs::projects::{self, ProjectInfo};
use stykanimate_rs::shell;

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::icons::icon_el;
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
    /// Per-file light-metadata cache (Phase J):
    /// path → `(mtime, figures, frames)` as last read by
    /// [`project_metadata`]. An entry is served only while
    /// the file's current mtime equals the stored one —
    /// the same mtime-key discipline as the scan cache,
    /// one level down (per file, not per directory).
    /// Interior mutability for the same reason as
    /// `scan_cache`; `Default` (empty map) keeps every
    /// existing construction of this state working.
    meta_cache: RefCell<HashMap<PathBuf, (SystemTime, usize, usize)>>,
    /// The project file whose deletion is armed and
    /// awaiting its confirm press (Phase K): the row for
    /// this path renders the confirm line instead of its
    /// Open / Duplicate / Delete buttons. `None` — the
    /// `Default` — means no deletion is pending. Set by a
    /// row's Delete button ([`arm_delete`]); cleared by
    /// the cancel press, by a successful delete, by
    /// opening another row, and by any search edit.
    pub confirm_delete: Option<PathBuf>,
    /// Row order (Phase K): `false` (the `Default`) keeps
    /// the scan's order — most recently modified first
    /// (projects.rs:62); `true` sorts the rows by name
    /// via [`sort_entries`]. Flipped by the `home-sort`
    /// toggle in the search row.
    pub sort_by_name: bool,
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

    /// One project file's light metadata
    /// ([`project_metadata`]) through the mtime-keyed
    /// per-file cache: one `stat` per call, a re-read only
    /// when the file's mtime has moved since the cached
    /// read. `None` when the file cannot be stat'ed or
    /// parsed — the caller (the row builder) then falls
    /// back to the scan's data, so a vanished file never
    /// breaks a row that is already on screen.
    pub fn metadata_for(&self, path: &Path) -> Option<(usize, usize, SystemTime)> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        {
            let cache = self.meta_cache.borrow();
            if let Some((figures, frames)) = cached_meta(&cache, path, modified) {
                return Some((figures, frames, modified));
            }
        }
        let (figures, frames) = read_metadata_counts(path)?;
        self.meta_cache
            .borrow_mut()
            .insert(path.to_path_buf(), (modified, figures, frames));
        Some((figures, frames, modified))
    }

    /// Drop both caches wholesale (Phase K): the next
    /// render rescans the projects directory and re-reads
    /// every row's light metadata from disk. Called after
    /// a successful project delete — the caches are keyed
    /// on mtimes (the directory's, then each file's), and
    /// while a delete does move the directory mtime, the
    /// per-file meta cache has no way to learn a file is
    /// *gone*; dropping both is the only way to be sure a
    /// deleted file never renders as a stale row.
    pub fn invalidate_caches(&self) {
        *self.scan_cache.borrow_mut() = None;
        self.meta_cache.borrow_mut().clear();
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

/// The row order the panel renders (Phase K): with
/// `by_name` false the entries pass through untouched —
/// the scan's order, most recently modified first
/// (projects.rs:62). With `by_name` true they are sorted
/// by display name, case-insensitively; the sort is
/// stable, so entries whose names fold to the same
/// string keep their scan (recency) order relative to
/// each other. Applied to the [`visible_projects`]
/// projection only — the reopen quick action keeps
/// resolving against the scan order, as before.
pub fn sort_entries(entries: Vec<ProjectInfo>, by_name: bool) -> Vec<ProjectInfo> {
    if !by_name {
        return entries;
    }
    let mut entries = entries;
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
    });
    entries
}

/// Whether the row for `path` is the one whose deletion
/// is armed (Phase K): the row builder renders that row
/// as the confirm line. The comparison is by exact path
/// — the scan's path, the same value the Delete button
/// armed — so a re-render under a different search or
/// sort can never move the confirm onto another row.
pub fn confirm_pending(panel: &HomePanelState, path: &Path) -> bool {
    panel.confirm_delete.as_deref() == Some(path)
}

/// The file the core's selector currently points at:
/// `project_files[project_index]`, the entry
/// `EditorCore::open_project_file` would open — and the
/// entry [`dispatch_open`] points at the file it opened,
/// so after an open this is the open project's file.
/// `None` when the core lists no project files.
pub fn current_project_path(core: &EditorCore) -> Option<&Path> {
    core.project_files
        .get(core.project_index)
        .map(|p| p.as_path())
}

/// The "Reopen last project" target: the library entry
/// for the most recently opened project, when there is
/// one worth offering.
///
/// `last` is the display name
/// `shell::last_opened_project` returns (shell.rs:127 —
/// recents store display names, not paths), so the target
/// is resolved by exact name match against `projects` —
/// pass the library list (the scan as [`visible_projects`]
/// filters it, *without* the search query: the quick
/// action does not follow the row filter). Shell-owned
/// and recovery entries are skipped even in a raw scan:
/// the snapshot can carry the same stored name as a real
/// project ("Untitled" is the common case), and reopening
/// must land on the real file.
///
/// Returns `None` — no button, no error — when there is
/// no last project, when its file is no longer in the
/// library (deleted or renamed since), or when it is the
/// currently open one: `current_path` matches the target
/// by *file name*, the panel's established matching rule
/// ([`project_file_index`]), because the scan reports
/// `./projects/…` relative paths while the core's list is
/// cwd-absolute.
pub fn reopen_target<'a>(
    projects: &'a [ProjectInfo],
    last: Option<&str>,
    current_path: Option<&Path>,
) -> Option<&'a ProjectInfo> {
    let last = last?;
    if last.is_empty() {
        return None;
    }
    let target = projects
        .iter()
        .find(|info| {
            info.name == last && !is_recovery_entry(info) && !is_shell_owned_entry(info)
        })?;
    if let Some(current) = current_path {
        let same_file = match (target.path.file_name(), current.file_name()) {
            (Some(a), Some(b)) => a == b,
            _ => target.path == current,
        };
        if same_file {
            return None;
        }
    }
    Some(target)
}

/// The reopen button's label: the resolved display name
/// from the scan, never the raw file stem.
pub fn reopen_label(target: &ProjectInfo) -> String {
    format!("Reopen {}", target.name)
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

/// A project file's light metadata: `(figures, frames,
/// modified)` read *without* deserialising the whole
/// `Project`. The file is parsed as a bare
/// `serde_json::Value` and only the lengths of the
/// top-level `figures` and `frames` arrays are counted
/// (model.rs's `Project` serialises both under exactly
/// those keys). `None` — never a guess — when the file is
/// missing, unreadable, not JSON, or either key is absent
/// or not an array: such a file is not a project file,
/// the same verdict `projects::scan` reaches by failing
/// its full parse.
pub fn project_metadata(path: &Path) -> Option<(usize, usize, SystemTime)> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let (figures, frames) = read_metadata_counts(path)?;
    Some((figures, frames, modified))
}

/// The parse half of [`project_metadata`] (the counts
/// only; the caller supplies the stat): split out so the
/// cached accessor can stat once and parse once.
fn read_metadata_counts(path: &Path) -> Option<(usize, usize)> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let figures = value.get("figures")?.as_array()?.len();
    let frames = value.get("frames")?.as_array()?.len();
    Some((figures, frames))
}

/// The metadata cache's lookup rule, pure so it is
/// testable without the filesystem: the cached counts
/// for `path` are served only when the mtime they were
/// read at equals the file's current mtime — a moved
/// mtime is a miss even for the same path, and a hit is
/// served without any re-read.
fn cached_meta(
    cache: &HashMap<PathBuf, (SystemTime, usize, usize)>,
    path: &Path,
    modified: SystemTime,
) -> Option<(usize, usize)> {
    match cache.get(path) {
        Some(&(read_at, figures, frames)) if read_at == modified => Some((figures, frames)),
        _ => None,
    }
}

/// The metadata line a row actually renders. When the
/// light read ([`HomePanelState::metadata_for`]) produced
/// figures/frames/modified for the row's file, those are
/// the numbers shown — re-verified against the file on
/// disk at its current mtime. Fps is not part of the
/// light read's tuple, so it still comes from the scan.
/// When the light read failed (the file vanished or
/// stopped parsing between the scan and this render) the
/// line is exactly the scan's [`meta_line`] — the row is
/// unchanged rather than blank.
pub fn row_meta_line(info: &ProjectInfo, meta: Option<(usize, usize, SystemTime)>) -> String {
    match meta {
        Some((figures, frames, modified)) => format!(
            "{} · {} · {} fps · modified {}",
            count_label(figures, "figure", "figures"),
            count_label(frames, "frame", "frames"),
            format_fps(info.fps),
            format_modified(Some(modified))
        ),
        None => meta_line(info),
    }
}

/// The project list's totals footer line (Phase L):
/// "{n} projects · {f} figures · {m} frames" over the
/// *visible* rows — the list after the search filter
/// and the sort. `entries_meta` holds one
/// `(figures, frames)` pair per visible project whose
/// light read ([`project_metadata`], served through
/// [`HomePanelState::metadata_for`]) succeeded, in row
/// order; `project_count` is the visible count itself.
/// A project whose read failed is *absent* from
/// `entries_meta` but present in `project_count`: it
/// counts as a project with 0 figures and 0 frames —
/// the sums simply never see it, and nothing about it
/// is guessed (see the module docs). Singular/plural
/// wording is `count_label`'s — the same helper the
/// rows' own metadata lines use — so exactly 1 is
/// singular and every other count, 0 included, plural.
pub fn totals_line(entries_meta: &[(usize, usize)], project_count: usize) -> String {
    let figures: usize = entries_meta.iter().map(|(figures, _)| figures).sum();
    let frames: usize = entries_meta.iter().map(|(_, frames)| frames).sum();
    format!(
        "{} · {} · {}",
        count_label(project_count, "project", "projects"),
        count_label(figures, "figure", "figures"),
        count_label(frames, "frame", "frames")
    )
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

/// Arm a project file's deletion (Phase K): the row's
/// Delete button records the path and nothing else — no
/// file is touched until [`dispatch_delete_confirm`].
/// Arming a second row replaces the first: only one
/// confirm line can exist at a time (its buttons carry
/// index-free keys).
pub fn arm_delete(panel: &mut HomePanelState, path: PathBuf) {
    panel.confirm_delete = Some(path);
}

/// Stand a pending deletion down (Phase K): the confirm
/// line's "Keep" press, and the clear every other
/// navigation applies (opening another row, editing the
/// search). The armed file is untouched, as it always is
/// until a confirm.
pub fn cancel_delete(panel: &mut HomePanelState) {
    panel.confirm_delete = None;
}

/// Carry out the armed deletion (Phase K): the confirm
/// line's "Delete file" press. The path comes from the
/// panel state — the row that armed it — never from a
/// row index, so a re-sorted or re-filtered list cannot
/// redirect the delete onto another file. The removal
/// itself is the shell's `delete_project_file`
/// (shell.rs, Phase K section 15), which refuses
/// shell-owned filenames: a row should never name one
/// ([`visible_projects`] filters them out), but the
/// refusal is the backstop, and it lands here as the
/// failure status with the confirm still armed and the
/// row intact. On success the confirm clears, both
/// caches are invalidated ([`HomePanelState::
/// invalidate_caches`]) so the deleted row cannot
/// reappear from a cached scan, and the status names
/// the project. Only the file is removed: if it is the
/// currently open project, the editor's in-memory copy
/// is left exactly as it is. With nothing armed the
/// press cannot happen (the confirm line is the only
/// caller), and the function simply returns.
pub fn dispatch_delete_confirm(
    core: &mut EditorCore,
    panel: &mut HomePanelState,
    name: &str,
) {
    let Some(path) = panel.confirm_delete.clone() else {
        return;
    };
    match shell::delete_project_file(&path) {
        Ok(()) => {
            panel.confirm_delete = None;
            panel.invalidate_caches();
            core.status = format!("Deleted project file {name}.");
        }
        Err(e) => {
            core.status = format!("Delete failed: {e}");
        }
    }
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
/// Open / Duplicate / Delete on the right. Keys carry the GPUI
/// tuple ids as strings (`home-open-{i}`, `home-dup-{i}`,
/// `home-proj-del-{i}`),
/// matching the `frame-{n}` key style `chrome_view` uses.
/// The metadata line under the name is keyed
/// `home-proj-meta-{i}` and renders [`row_meta_line`]:
/// the file's light metadata (`meta`, from
/// [`HomePanelState::metadata_for`]) preferred, the
/// scan's line as the fallback.
fn project_row(
    info: &ProjectInfo,
    i: usize,
    t: &Theme,
    meta: Option<(usize, usize, SystemTime)>,
) -> El {
    project_row_confirm(info, i, t, meta, false)
}

/// [`project_row`] with the Phase K delete state: while
/// `confirming` holds (this row's path is the armed one,
/// [`confirm_pending`]) the row renders *instead* as the
/// confirm line — "Delete {name}?" with a "Delete file"
/// confirm (`home-proj-del-confirm`) and a "Keep" cancel
/// (`home-proj-del-cancel`) — under the same
/// `home-row-{i}` key and row chrome, so the list does
/// not jump. Otherwise it is the ordinary row, plus its
/// Delete button (`home-proj-del-{i}`), which only arms
/// the deletion ([`arm_delete`]).
fn project_row_confirm(
    info: &ProjectInfo,
    i: usize,
    t: &Theme,
    meta: Option<(usize, usize, SystemTime)>,
    confirming: bool,
) -> El {
    if confirming {
        let name = info.name.clone();
        let confirm_name = info.name.clone();
        return row()
            .key(format!("home-row-{i}"))
            .w_full()
            .gap(8.0)
            .center_y()
            .px(10.0)
            .py(6.0)
            .radius(8.0)
            .fill(t.bg)
            .child(
                text(format!("Delete {name}?"))
                    .font(type_::TITLE)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(btn(
                "home-proj-del-confirm",
                "Delete file",
                false,
                t,
                move |s: &mut State| {
                    dispatch_delete_confirm(&mut s.core, &mut s.home_panel, &confirm_name);
                },
            ))
            .child(btn(
                "home-proj-del-cancel",
                "Keep",
                false,
                t,
                |s: &mut State| {
                    cancel_delete(&mut s.home_panel);
                },
            ));
    }
    let open_key = format!("home-open-{i}");
    let dup_key = format!("home-dup-{i}");
    let del_key = format!("home-proj-del-{i}");
    let path = info.path.clone();
    let dup_path = info.path.clone();
    let del_path = info.path.clone();
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
                    text(row_meta_line(info, meta))
                        .key(format!("home-proj-meta-{i}"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(icon_btn(&open_key, "folder-open", "Open", false, t, move |s: &mut State| {
            // Opening a row stands down any pending
            // delete confirm (Phase K): the armed row is
            // never this one — an armed row renders its
            // confirm line instead of an Open button.
            cancel_delete(&mut s.home_panel);
            dispatch_open(&mut s.core, &path, &name);
        }))
        .child(btn(&dup_key, "Duplicate", false, t, move |s: &mut State| {
            dispatch_duplicate(&mut s.core, &dup_path);
        }))
        .child(btn(&del_key, "Delete", false, t, move |s: &mut State| {
            arm_delete(&mut s.home_panel, del_path.clone());
        }))
}

/// The section as mounted in the shell column: a "Home"
/// toggle (accent-filled while open, the GPUI `home` id),
/// and — when open — the library card: the reopen-last
/// quick action (when offered), recovery banner, search
/// field, New project, and the project rows.
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
    let shown = sort_entries(
        visible_projects(&entries, &home.search.text),
        home.sort_by_name,
    );
    // The reopen quick action resolves against the same
    // cached scan, through the library filter but *not*
    // the search query (see `reopen_target`).
    let library = visible_projects(&entries, "");
    let last_opened = shell::last_opened_project();
    let reopen = reopen_target(
        &library,
        last_opened.as_deref(),
        current_project_path(&s.core),
    );
    let mut card = col()
        .key("home-card")
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .fill(t.card);
    if let Some(target) = reopen {
        let path = target.path.clone();
        let name = target.name.clone();
        card = card.child(icon_btn(
            "home-reopen-last",
            "arrow-rotate-right",
            &reopen_label(target),
            true,
            t,
            move |s: &mut State| {
                // Reopening is an open: it stands down
                // any pending delete confirm (Phase K).
                cancel_delete(&mut s.home_panel);
                dispatch_open(&mut s.core, &path, &name);
            },
        ));
    }
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
            .child(icon_btn("home-new", "file", "New project", false, t, |s: &mut State| {
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
            .child(icon_el("magnifying-glass", 12.0, t.text_secondary))
            .child(
                text_input(&home.search, type_::BODY.lh(26.0))
                    .key("home-search")
                    .single_line()
                    .grow(1.0)
                    .color(t.text)
                    .placeholder("Search projects", t.text_secondary)
                    .on_edit(|s: &mut State| &mut s.home_panel.search)
                    // Any search edit stands down a
                    // pending delete confirm (Phase K):
                    // the row list is about to change
                    // under the armed row.
                    .on_input_change(|s: &mut State, _tx: &mut Tx| {
                        cancel_delete(&mut s.home_panel);
                    }),
            )
            .child(section_label(
                t,
                &format!(
                    "{} {}",
                    shown.len(),
                    if shown.len() == 1 { "project" } else { "projects" }
                ),
            ))
            .child(btn(
                "home-sort",
                if home.sort_by_name {
                    "Sort: A–Z"
                } else {
                    "Sort: recent"
                },
                home.sort_by_name,
                t,
                |s: &mut State| {
                    s.home_panel.sort_by_name = !s.home_panel.sort_by_name;
                },
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
        // The totals footer's inputs (Phase L): the
        // (figures, frames) of every visible project
        // whose light read succeeded, collected from the
        // same reads that feed the rows themselves — no
        // extra disk traffic. A failed read pushes
        // nothing: that project still counts in
        // `shown.len()` with 0 figures / 0 frames (see
        // `totals_line`).
        let mut metas: Vec<(usize, usize)> = Vec::new();
        for (i, info) in shown.iter().enumerate() {
            let meta = home.metadata_for(&info.path);
            if let Some((figures, frames, _)) = meta {
                metas.push((figures, frames));
            }
            let confirming = confirm_pending(home, &info.path);
            rows = rows.child(project_row_confirm(info, i, t, meta, confirming));
        }
        card = card.child(rows);
        // Inside the non-empty branch on purpose: with
        // no visible projects the panel keeps its empty
        // message and no footer element is rendered.
        card = card.child(
            text(totals_line(&metas, shown.len()))
                .key("home-totals")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
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
    fn reopen_target_is_none_without_a_last_project() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
        ];
        // No recents recorded yet — or an empty name —
        // offers nothing and errors nowhere.
        assert!(reopen_target(&entries, None, None).is_none());
        assert!(reopen_target(&entries, Some(""), None).is_none());
        assert!(reopen_target(&[], Some("Alpha"), None).is_none());
    }

    #[test]
    fn reopen_target_hides_the_currently_open_project() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
        ];
        // The core's list is cwd-absolute, the scan's paths
        // relative: the match is by file name, so pointing
        // the selector at beta.json hides the button.
        assert!(reopen_target(
            &entries,
            Some("Beta"),
            Some(Path::new("/home/user/projects/beta.json")),
        )
        .is_none());
        // Pointing it at a *different* file keeps the offer.
        let target = reopen_target(
            &entries,
            Some("Beta"),
            Some(Path::new("/home/user/projects/alpha.json")),
        )
        .unwrap();
        assert_eq!(target.name, "Beta");
    }

    #[test]
    fn reopen_target_is_none_when_the_file_is_gone() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
        ];
        // The recents front names a project whose file was
        // deleted or renamed since: no button, no error.
        assert!(reopen_target(&entries, Some("Gamma"), None).is_none());
        // Display names match exactly — recents store the
        // name verbatim, so a case variant is a different
        // (missing) project, not a fuzzy hit.
        assert!(reopen_target(&entries, Some("beta"), None).is_none());
    }

    #[test]
    fn reopen_target_resolves_the_last_project_from_the_scan() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
        ];
        let target = reopen_target(&entries, Some("Beta"), None).unwrap();
        assert_eq!(target.name, "Beta");
        assert_eq!(target.path, PathBuf::from("projects/beta.json"));
        assert_eq!(reopen_label(target), "Reopen Beta");
    }

    #[test]
    fn reopen_target_skips_the_recovery_snapshot_twin() {
        // The recovery snapshot parses as a project and can
        // carry the same stored name as the real file —
        // scan order puts the (newer) snapshot first. The
        // reopen must land on the real library file.
        let entries = vec![
            info("Untitled", "projects/.recovery.json"),
            info("Untitled", "projects/untitled.json"),
        ];
        let target = reopen_target(&entries, Some("Untitled"), None).unwrap();
        assert_eq!(target.path, PathBuf::from("projects/untitled.json"));
        // With *only* the snapshot present there is nothing
        // to reopen — the banner owns that entry.
        assert!(reopen_target(&entries[..1], Some("Untitled"), None).is_none());
    }

    #[test]
    fn current_project_path_follows_the_core_selector() {
        let mut core = EditorCore::new();
        core.project_files = vec![
            PathBuf::from("/abs/projects/alpha.json"),
            PathBuf::from("/abs/projects/beta.json"),
        ];
        core.project_index = 1;
        assert_eq!(
            current_project_path(&core),
            Some(Path::new("/abs/projects/beta.json"))
        );
        core.project_files.clear();
        assert_eq!(current_project_path(&core), None);
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

    /// Minimal project-file JSON with the two arrays the
    /// light parse counts, plus unrelated keys it must
    /// tolerate (a real save carries far more).
    fn project_json(figures: usize, frames: usize) -> String {
        let figure = "{\"def\":{},\"x\":0.0,\"y\":0.0}";
        let frame = "{\"poses\":[]}";
        format!(
            "{{\"name\":\"Meta\",\"fps\":12.0,\"figures\":[{}],\"frames\":[{}],\"camera\":[]}}",
            vec![figure; figures].join(","),
            vec![frame; frames].join(",")
        )
    }

    #[test]
    fn project_metadata_reads_counts_and_mtime_from_a_light_parse() {
        let dir = temp_dir("metadata-parse");
        let path = dir.join("meta.json");
        std::fs::write(&path, project_json(3, 7)).unwrap();
        let (figures, frames, modified) = project_metadata(&path).unwrap();
        assert_eq!(figures, 3);
        assert_eq!(frames, 7);
        // The returned stamp is the file's own mtime.
        let stat = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(modified, stat);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_metadata_rejects_missing_and_unparseable_files() {
        let dir = temp_dir("metadata-reject");
        // Missing file.
        assert!(project_metadata(&dir.join("nope.json")).is_none());
        // Not JSON at all.
        let junk = dir.join("junk.json");
        std::fs::write(&junk, b"not json").unwrap();
        assert!(project_metadata(&junk).is_none());
        // JSON, but neither array is present.
        let empty = dir.join("empty.json");
        std::fs::write(&empty, b"{}").unwrap();
        assert!(project_metadata(&empty).is_none());
        // A key of the wrong type is not an array to count.
        let wrong = dir.join("wrong.json");
        std::fs::write(&wrong, br#"{"figures":3,"frames":[]}"#).unwrap();
        assert!(project_metadata(&wrong).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cached_meta_hits_only_on_an_exact_path_and_mtime_match() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let path = PathBuf::from("projects/alpha.json");
        let mut cache: HashMap<PathBuf, (SystemTime, usize, usize)> = HashMap::new();
        // Empty cache misses.
        assert_eq!(cached_meta(&cache, &path, t0), None);
        cache.insert(path.clone(), (t0, 4, 9));
        // Same path, the mtime it was read at: a hit.
        assert_eq!(cached_meta(&cache, &path, t0), Some((4, 9)));
        // Same path, a moved mtime: a miss — the file may
        // have changed under the same name.
        assert_eq!(cached_meta(&cache, &path, t1), None);
        // A different path at the same mtime: a miss.
        assert_eq!(
            cached_meta(&cache, Path::new("projects/beta.json"), t0),
            None
        );
    }

    #[test]
    fn metadata_for_serves_the_cache_until_the_mtime_moves() {
        let dir = temp_dir("metadata-cache");
        let path = dir.join("cached.json");
        std::fs::write(&path, project_json(2, 5)).unwrap();
        let state = HomePanelState::default();
        let (figures, frames, read_at) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (2, 5));

        // Rewrite the file with different counts but pin
        // its mtime back to the cached read's stamp: the
        // accessor must serve the cached counts — proof it
        // did not re-read (a re-read would see 6 and 1).
        std::fs::write(&path, project_json(6, 1)).unwrap();
        set_mtime(&path, read_at);
        let (figures, frames, modified) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (2, 5), "cache served, no re-read");
        assert_eq!(modified, read_at);

        // Move the mtime: the entry is stale, the file is
        // re-read, and the new counts land.
        let later = read_at + std::time::Duration::from_secs(60);
        set_mtime(&path, later);
        let (figures, frames, modified) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (6, 1));
        assert_eq!(modified, later);

        // A missing file yields nothing, cache or not.
        assert!(state.metadata_for(&dir.join("gone.json")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn row_meta_line_prefers_the_light_read_and_falls_back_to_the_scan() {
        let mut entry = info("Demo", "projects/demo.json");
        entry.figures = 9;
        entry.frames = 9;
        entry.fps = 24.0;
        entry.modified = None;
        let stamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426);
        // The light read's figures/frames/modified win;
        // fps still comes from the scan (it is not part of
        // the light tuple).
        assert_eq!(
            row_meta_line(&entry, Some((3, 12, stamp))),
            "3 figures · 12 frames · 24 fps · modified 2026-11-08 05:57"
        );
        // No light read: the line is the scan's, unchanged.
        assert_eq!(row_meta_line(&entry, None), meta_line(&entry));
        assert_eq!(
            row_meta_line(&entry, None),
            "9 figures · 9 frames · 24 fps · modified unknown"
        );
    }

    #[test]
    fn project_row_builds_with_and_without_light_metadata() {
        // Builder smoke test: the row (including the keyed
        // metadata line) constructs in both modes — the
        // fallback must never cost the row itself.
        let t = tok::theme(true);
        let entry = info("Demo", "projects/demo.json");
        let _ = project_row(&entry, 0, &t, None);
        let stamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426);
        let _ = project_row(&entry, 0, &t, Some((3, 12, stamp)));
    }

    #[test]
    fn totals_line_sums_the_visible_projects() {
        // Fabricated light-read results, in row order:
        // the figures and frames sum across every entry.
        let metas = [(2, 5), (3, 10), (0, 1)];
        assert_eq!(totals_line(&metas, 3), "3 projects · 5 figures · 16 frames");
    }

    #[test]
    fn totals_line_singularises_one() {
        // The wording is `count_label`'s, decided once:
        // exactly 1 is singular for all three counts.
        assert_eq!(totals_line(&[(1, 1)], 1), "1 project · 1 figure · 1 frame");
    }

    #[test]
    fn totals_line_counts_an_unreadable_project_as_zero() {
        // Two projects are visible, but only one light
        // read succeeded: the other still counts as a
        // project, contributing 0 figures / 0 frames —
        // its absence from the slice is the whole rule.
        assert_eq!(totals_line(&[(2, 5)], 2), "2 projects · 2 figures · 5 frames");
    }

    #[test]
    fn totals_line_for_no_readable_projects() {
        // The unreadable extreme: projects counted,
        // nothing read — and an empty slice with a zero
        // count still formats (0 is plural, like the
        // rows). The builder renders the footer only
        // under a non-empty list, so neither shape can
        // appear beside the empty message.
        assert_eq!(totals_line(&[], 2), "2 projects · 0 figures · 0 frames");
        assert_eq!(totals_line(&[], 0), "0 projects · 0 figures · 0 frames");
    }

    #[test]
    fn project_row_confirm_builds_the_row_and_the_confirm_line() {
        // Builder smoke test for the Phase K shapes: the
        // ordinary row (with its Delete button) and the
        // armed row's confirm line both construct.
        let t = tok::theme(true);
        let entry = info("Demo", "projects/demo.json");
        let _ = project_row_confirm(&entry, 0, &t, None, false);
        let _ = project_row_confirm(&entry, 0, &t, None, true);
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

    #[test]
    fn state_defaults_no_delete_armed_and_scan_order_sort() {
        // The Phase K fields ride on `Default`: no
        // deletion pending, rows in scan order.
        let state = HomePanelState::default();
        assert_eq!(state.confirm_delete, None);
        assert!(!state.sort_by_name);
    }

    #[test]
    fn sort_entries_default_order_passes_through_untouched() {
        // by_name = false: the scan's order is the
        // panel's order, exactly as given — even when
        // the input is not name-sorted.
        let entries = vec![
            info("Zulu", "projects/zulu.json"),
            info("Alpha", "projects/alpha.json"),
            info("Mike", "projects/mike.json"),
        ];
        let out = sort_entries(entries, false);
        assert_eq!(
            out.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Zulu", "Alpha", "Mike"]
        );
    }

    #[test]
    fn sort_entries_by_name_is_case_insensitive_and_stable() {
        let entries = vec![
            info("beta", "projects/beta.json"),
            info("Alpha", "projects/alpha.json"),
            info("BETA", "projects/beta-two.json"),
            info("apple", "projects/apple.json"),
        ];
        let out = sort_entries(entries, true);
        assert_eq!(
            out.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Alpha", "apple", "beta", "BETA"]
        );
        // Stability: the two entries whose names fold to
        // "beta" keep their input (scan) order — the
        // earlier "beta" still precedes "BETA".
        assert_eq!(out[2].path, PathBuf::from("projects/beta.json"));
        assert_eq!(out[3].path, PathBuf::from("projects/beta-two.json"));
    }

    #[test]
    fn delete_arm_replaces_and_cancel_clears() {
        // The confirm state machine at the helper level:
        // arming records the path, arming again replaces
        // it (one confirm line at a time), cancel clears.
        let mut panel = HomePanelState::default();
        assert!(!confirm_pending(&panel, Path::new("projects/alpha.json")));
        arm_delete(&mut panel, PathBuf::from("projects/alpha.json"));
        assert!(confirm_pending(&panel, Path::new("projects/alpha.json")));
        assert!(!confirm_pending(&panel, Path::new("projects/beta.json")));
        arm_delete(&mut panel, PathBuf::from("projects/beta.json"));
        assert!(confirm_pending(&panel, Path::new("projects/beta.json")));
        assert!(!confirm_pending(&panel, Path::new("projects/alpha.json")));
        cancel_delete(&mut panel);
        assert_eq!(panel.confirm_delete, None);
        // Cancelling with nothing armed is a no-op.
        cancel_delete(&mut panel);
        assert_eq!(panel.confirm_delete, None);
    }

    #[test]
    fn delete_confirm_with_nothing_armed_changes_nothing() {
        let mut core = EditorCore::new();
        core.status = "Sentinel.".into();
        let mut panel = HomePanelState::default();
        dispatch_delete_confirm(&mut core, &mut panel, "Demo");
        assert_eq!(core.status, "Sentinel.");
        assert_eq!(panel.confirm_delete, None);
    }

    #[test]
    fn dispatch_delete_confirm_removes_a_real_file_and_clears_state() {
        // A real file in a real temp dir (the metadata
        // tests' precedent): the confirm press removes
        // it, clears the armed path, drops BOTH caches —
        // a stale row must never render — and reports
        // the deletion by the project's display name.
        let dir = temp_dir("delete-confirm");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        // Seed both caches as earlier renders would have.
        *panel.scan_cache.borrow_mut() =
            Some((None, vec![info("Demo", "projects/demo.json")]));
        panel
            .meta_cache
            .borrow_mut()
            .insert(path.clone(), (SystemTime::UNIX_EPOCH, 1, 1));
        arm_delete(&mut panel, path.clone());
        let mut core = EditorCore::new();
        dispatch_delete_confirm(&mut core, &mut panel, "Demo");
        assert!(!path.exists(), "the file is gone");
        assert_eq!(panel.confirm_delete, None);
        assert_eq!(core.status, "Deleted project file Demo.");
        assert!(
            panel.scan_cache.borrow().is_none(),
            "scan cache dropped"
        );
        assert!(
            panel.meta_cache.borrow().is_empty(),
            "meta cache dropped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_delete_confirm_refuses_a_shell_owned_name() {
        // The shell helper refuses shell-owned filenames:
        // a confirm over `.recovery.json` deletes
        // nothing, the confirm stays armed (the row state
        // survives) and the status carries the failure.
        let dir = temp_dir("delete-refused");
        let path = dir.join(shell::RECOVERY_FILE);
        std::fs::write(&path, b"{}").unwrap();
        let mut panel = HomePanelState::default();
        arm_delete(&mut panel, path.clone());
        let mut core = EditorCore::new();
        dispatch_delete_confirm(&mut core, &mut panel, "Untitled");
        assert!(path.exists(), "the snapshot is untouched");
        assert_eq!(panel.confirm_delete, Some(path));
        assert!(
            core.status.starts_with("Delete failed:"),
            "{}",
            core.status
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
