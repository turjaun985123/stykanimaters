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
//!   key) has not changed. The line also carries the
//!   file's size (Phase M): the byte length of the same
//!   single stat the mtime comes from, formatted by
//!   project_tools' [`human_size`] — the size rides the
//!   mtime cache with the counts, so it costs no extra
//!   disk read, and the scan-only fallback line keeps
//!   its old shape (the scan never stats for a length).
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
//!   success all three caches are dropped wholesale
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
//! - **Rename (UI mission Wave 4, audit S06-4)** — each
//!   row's `home-proj-rename-{i}` button arms the rename
//!   session ([`State::home_rename`], modelled on the
//!   clips/pose rename sessions): the row re-renders as
//!   a rename line — the name field
//!   (`home-proj-rename-field`, opened loaded with the
//!   row's stored name) with `home-proj-rename-confirm`
//!   / `home-proj-rename-cancel`. The confirm runs
//!   [`confirm_rename`] → [`dispatch_rename`], which
//!   calls `projects::rename_project` — a FILE rename
//!   only: the name stored inside the project's JSON
//!   updates on the next save, so the row keeps showing
//!   the stored name and the success status says so.
//!   On success the recents list is re-touched with the
//!   new name and all three caches are invalidated; on
//!   failure the rename stays armed with the typed text
//!   intact. Arming a rename stands down a pending
//!   delete confirm and any armed session discard, and
//!   vice versa — one pending row action at a time.
//! - **Session discards (UI mission Wave 4, audit S16
//!   D2)** — this panel's session-replacing presses run
//!   through the crate-level [`ConfirmAction`] gate
//!   (see main.rs): a row's Open (`home-open-{i}`), the
//!   reopen action (`home-reopen-last`), New
//!   (`home-new`) and the recovery banner's Recover
//!   (`recover`) arm on first press while the session
//!   is dirty (`edits_since_autosave() > 0`) and
//!   re-render as danger "Discard changes?" buttons
//!   under the same keys; the second press runs the
//!   action. The presses live in `open_row_pressed`,
//!   `new_project_pressed` and `recover_banner_pressed`.
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
//! ## Thumbnails (UI mission Wave 4, audit S06-8)
//!
//! Each library row leads with a real frame-0 render of
//! its project — the audit found the machinery proven but
//! unwired (`projects::thumbnail` had zero callers), and
//! this is the wiring. The discipline is the cost rule the
//! audit set: a render is an export-grade rasterise
//! (`export::render_frame_at` renders at stage size ×2
//! supersample), so generation happens **at scan time
//! only** — inside [`HomePanelState::projects`] when the
//! scan cache refills ([`HomePanelState::fill_thumbnails`]),
//! never in the row builder. The row builder only reads
//! the finished handle through
//! [`HomePanelState::thumbnail_for`], which serves it
//! solely while the scan entry's mtime matches the mtime
//! the render was made at — the meta cache's exact-mtime
//! discipline, one level up. A file that will not load or
//! render caches as `None` at that mtime: the row shows
//! no thumb, generation is not retried until the file
//! changes, and nothing is ever faked with a placeholder
//! image.
//!
//! The recipe is the contact sheet's (`export::
//! render_contact_sheet`): render frame 0, convert to
//! RGBA, shrink with `image::imageops::resize` (Triangle)
//! to [`THUMB_W_PX`] wide keeping the stage's aspect. The
//! bytes become a Rune image through `ImageCache::rgba` on
//! a short-lived cache created for the fill; the fill
//! waits for the decode (`Image::wait`) before the cache
//! drops, and the panel keeps the `Image` handle — the
//! decoded pixels live in the handle, not the cache, so
//! the thumbnail outlives it. The row displays the handle
//! at [`THUMB_W_PT`] wide with its aspect-preserved
//! height, clipped to a 6 pt radius. Deleting a project
//! drops the thumbnail with the other caches
//! ([`HomePanelState::invalidate_caches`]), and a scan
//! that stops listing a file drops its handle in the
//! fill.
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
//! ## Phase N adoption — the card language
//!
//! The panel root is the kit card ([`design::panel_col`]
//! on the existing `home-panel` key); the old hand-rolled
//! inner card (`home-card`) is a plain column inside it.
//! The `home-header` row is the kit header
//! ([`design::header_row`] + [`design::panel_title`])
//! with a [`design::divider`] under it. Project rows are
//! kit list rows ([`design::list_row`]): the row for the
//! project the core's selector currently points at
//! ([`is_current_project`] — matched by file name, the
//! panel's established rule) carries the kit selected
//! fill, so the open project is marked in its own list;
//! the armed delete-confirm line renders under the same
//! row key on the selected fill (P10's
//! selected-for-danger read) with the family's
//! [`danger_btn`] as its confirm. The two empty-list
//! messages render through [`design::empty_state`]
//! (folder-open for an empty library, magnifying-glass
//! for a search with no match) — the strings themselves
//! are byte-identical to the bare label rows they
//! replace, as are every key, label and handler in the
//! panel.
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
use rune::{col, el, row, text, text_input, El, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Project;
use stykanimate_rs::projects::{self, ProjectInfo};
use stykanimate_rs::shell;

use crate::chrome_view::{btn, danger_btn};
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn};
use crate::rune_sec::icons::icon_el;
use crate::rune_sec::project_tools::{human_size, session_dirty};
use crate::{ConfirmAction, State};

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
    /// path → `(mtime, figures, frames, size_bytes)` as
    /// last read by [`project_metadata`] (the size joined
    /// in Phase M, from the same stat as the mtime). An
    /// entry is served only while
    /// the file's current mtime equals the stored one —
    /// the same mtime-key discipline as the scan cache,
    /// one level down (per file, not per directory).
    /// Interior mutability for the same reason as
    /// `scan_cache`; `Default` (empty map) keeps every
    /// existing construction of this state working.
    meta_cache: RefCell<HashMap<PathBuf, (SystemTime, usize, usize, u64)>>,
    /// Per-file thumbnail cache (UI mission Wave 4):
    /// path → `(mtime, Option<Image>)` — the frame-0
    /// render of the file as it stood at that mtime,
    /// generated at scan time by
    /// [`HomePanelState::fill_thumbnails`] (never in the
    /// row builder: a render is an export-grade
    /// rasterise). `None` is a cached failure — the file
    /// would not load or render at that mtime, the row
    /// shows no thumb, and generation is not retried
    /// until the file's mtime moves. Entries are served
    /// only on an exact mtime match with the scan entry
    /// being rendered ([`HomePanelState::thumbnail_for`])
    /// — the meta cache's discipline, one level up.
    /// Interior mutability for the same reason as the
    /// other caches; `Default` (empty map) keeps every
    /// existing construction of this state working.
    thumb_cache: RefCell<HashMap<PathBuf, (SystemTime, Option<rune::Image>)>>,
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
        // Thumbnails are generated here — at scan/cache
        // time, the one moment the library's contents are
        // (re)read — and never in the row builder (see
        // the module docs' thumbnail section).
        self.fill_thumbnails(&entries);
        *self.scan_cache.borrow_mut() = Some((key, entries));
        Ref::map(self.scan_cache.borrow(), |c| &c.as_ref().unwrap().1)
    }

    /// Generate (or refresh) the frame-0 thumbnail of
    /// every library entry in a fresh scan: one render
    /// per file whose mtime has moved since its cached
    /// render, keyed on the scan's own stat
    /// (`ProjectInfo::modified`, so the value a render is
    /// stored under is exactly the value
    /// [`HomePanelState::thumbnail_for`] later checks the
    /// row's entry against). Shell-owned and recovery
    /// entries are skipped — they are never rows. Handles
    /// for files the scan no longer lists are dropped, so
    /// a deleted or renamed file's render cannot outlive
    /// its listing. Called from [`HomePanelState::
    /// projects`] on a scan refill only; the cost (a full
    /// load + rasterise per changed file) is the scan's
    /// cost class, paid once per file version.
    fn fill_thumbnails(&self, entries: &[ProjectInfo]) {
        let mut cache = self.thumb_cache.borrow_mut();
        cache.retain(|path, _| entries.iter().any(|e| &e.path == path));
        for info in entries {
            if is_recovery_entry(info) || is_shell_owned_entry(info) {
                continue;
            }
            // No stat from the scan: nothing honest to
            // key a render on (and the light read would
            // fail the same way) — no thumbnail.
            let Some(mtime) = info.modified else {
                continue;
            };
            if matches!(cache.get(&info.path), Some((at, _)) if *at == mtime) {
                continue;
            }
            let image = render_thumbnail(&info.path);
            cache.insert(info.path.clone(), (mtime, image));
        }
    }

    /// The cached thumbnail for one row's scan entry, or
    /// `None` when this file version has none: never
    /// rendered (shell-owned rows are not built, and a
    /// stale scan can name a file the fill skipped), a
    /// cached render failure at this mtime, or an mtime
    /// mismatch (the entry was scanned at a different
    /// moment than the render — the next scan refill
    /// regenerates). A pure cache read: this runs inside
    /// the row builder, where rendering is forbidden by
    /// the cost rule (see the module docs).
    pub fn thumbnail_for(&self, info: &ProjectInfo) -> Option<rune::Image> {
        let cache = self.thumb_cache.borrow();
        match cache.get(&info.path) {
            Some((at, image)) if Some(*at) == info.modified => image.clone(),
            _ => None,
        }
    }

    /// One project file's light metadata
    /// ([`project_metadata`]) through the mtime-keyed
    /// per-file cache: one `stat` per call, a re-read only
    /// when the file's mtime has moved since the cached
    /// read. `None` when the file cannot be stat'ed or
    /// parsed — the caller (the row builder) then falls
    /// back to the scan's data, so a vanished file never
    /// breaks a row that is already on screen. The size
    /// (Phase M) rides the same discipline as the counts:
    /// it is taken from the call's one stat on a miss and
    /// served from the cache on a hit, so a returned tuple
    /// is always one moment's read, never a mix.
    pub fn metadata_for(&self, path: &Path) -> Option<(usize, usize, u64, SystemTime)> {
        let stat = std::fs::metadata(path).ok()?;
        let modified = stat.modified().ok()?;
        let size_bytes = stat.len();
        {
            let cache = self.meta_cache.borrow();
            if let Some((figures, frames, size)) = cached_meta(&cache, path, modified) {
                return Some((figures, frames, size, modified));
            }
        }
        let (figures, frames) = read_metadata_counts(path)?;
        self.meta_cache
            .borrow_mut()
            .insert(path.to_path_buf(), (modified, figures, frames, size_bytes));
        Some((figures, frames, size_bytes, modified))
    }

    /// Drop all three caches wholesale (Phase K; the
    /// thumbnail cache joined in Wave 4): the next render
    /// rescans the projects directory, re-reads every
    /// row's light metadata from disk, and re-renders
    /// thumbnails at that scan. Called after a successful
    /// project delete — the caches are keyed on mtimes
    /// (the directory's, then each file's), and while a
    /// delete does move the directory mtime, the per-file
    /// caches have no way to learn a file is *gone*;
    /// dropping all three is the only way to be sure a
    /// deleted file never renders as a stale row.
    pub fn invalidate_caches(&self) {
        *self.scan_cache.borrow_mut() = None;
        self.meta_cache.borrow_mut().clear();
        self.thumb_cache.borrow_mut().clear();
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

/// Whether `info` is the project `current` points at
/// (Phase N): the row for the currently open project
/// carries the kit selected fill, so the open project is
/// marked in its own list. The comparison follows the
/// panel's established matching rule — by file name,
/// because the scan reports `./projects/…` relative
/// paths while the core's list is cwd-absolute (the
/// same rule [`project_file_index`] and
/// [`reopen_target`] use, with the same exact-path
/// fallback when a name is unavailable on either side).
pub fn is_current_project(info: &ProjectInfo, current: Option<&Path>) -> bool {
    let Some(current) = current else {
        return false;
    };
    match (info.path.file_name(), current.file_name()) {
        (Some(a), Some(b)) => a == b,
        _ => info.path == current,
    }
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
/// size_bytes, modified)` read *without* deserialising
/// the whole `Project`. The file is parsed as a bare
/// `serde_json::Value` and only the lengths of the
/// top-level `figures` and `frames` arrays are counted
/// (model.rs's `Project` serialises both under exactly
/// those keys). The size is the file's byte length,
/// taken from the same single `stat` that yields the
/// mtime (Phase M) — no second filesystem call. `None`
/// — never a guess — when the file is
/// missing, unreadable, not JSON, or either key is absent
/// or not an array: such a file is not a project file,
/// the same verdict `projects::scan` reaches by failing
/// its full parse.
pub fn project_metadata(path: &Path) -> Option<(usize, usize, u64, SystemTime)> {
    let stat = std::fs::metadata(path).ok()?;
    let modified = stat.modified().ok()?;
    let (figures, frames) = read_metadata_counts(path)?;
    Some((figures, frames, stat.len(), modified))
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
/// and size for `path` are served only when the mtime
/// they were read at equals the file's current mtime —
/// a moved mtime is a miss even for the same path, and
/// a hit is served without any re-read.
fn cached_meta(
    cache: &HashMap<PathBuf, (SystemTime, usize, usize, u64)>,
    path: &Path,
    modified: SystemTime,
) -> Option<(usize, usize, u64)> {
    match cache.get(path) {
        Some(&(read_at, figures, frames, size)) if read_at == modified => {
            Some((figures, frames, size))
        }
        _ => None,
    }
}

/// Thumbnail pixel width: the rows display at
/// [`THUMB_W_PT`] points, so 112 px serves a 2x display
/// at full density and a 1x display from the renderer's
/// mips. The fixed small width is the audit's recipe
/// (S06-8) and the contact sheet's shape: render big,
/// shrink once, cache the small result.
pub const THUMB_W_PX: u32 = 112;

/// Thumbnail display width in a project row, points.
/// The height follows the project's own stage aspect
/// ([`thumb_el`]), so a 4:3 project shows a taller thumb
/// than a 16:9 one — the shape on the card is the shape
/// of the stage.
pub const THUMB_W_PT: f32 = 56.0;

/// Render one project file's frame 0 as a small Rune
/// image — the scan-time half of the thumbnail feature
/// (see the module docs). The file is fully loaded
/// (`projects::load`), frame 0 is rasterised by the
/// export renderer (`export::render_frame_at`, stage
/// size ×2 supersample), converted to RGBA and shrunk
/// with the Triangle filter to [`THUMB_W_PX`] wide —
/// the contact sheet's exact recipe
/// (`export::render_contact_sheet`). The bytes become a
/// Rune image through `ImageCache::rgba` on a cache
/// created for this fill; the decode is waited out
/// (`Image::wait`) before that cache drops, and the
/// returned handle owns the decoded pixels from then
/// on, so the thumbnail outlives the cache.
///
/// `None` — cached by the caller as a failed render —
/// when the file cannot be loaded, the project has no
/// frames to render, the render comes back empty, or
/// the image decode fails: a row without a thumbnail is
/// the honest outcome for a file whose first frame
/// cannot be drawn, never a placeholder.
pub fn render_thumbnail(path: &Path) -> Option<rune::Image> {
    let project = projects::load(path).ok()?;
    if project.frames.is_empty() {
        return None;
    }
    let frame = stykanimate_rs::export::render_frame_at(&project, 0.0);
    let rgba = image::DynamicImage::ImageRgb8(frame).to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    if w == 0 || h == 0 {
        return None;
    }
    let thumb_h = ((THUMB_W_PX as f32 * h as f32 / w as f32).round() as u32).max(1);
    let small = image::imageops::resize(
        &rgba,
        THUMB_W_PX,
        thumb_h,
        image::imageops::FilterType::Triangle,
    );
    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
    let image = cache.rgba(THUMB_W_PX, thumb_h, small.into_raw());
    image.wait().ok()?;
    Some(image)
}

/// The row's thumbnail element: the cached image at
/// [`THUMB_W_PT`] wide, its height taken from the
/// image's own aspect (the stage's shape), clipped to a
/// 6 pt radius. Built only when the row has a thumbnail
/// ([`HomePanelState::thumbnail_for`]) — a row without
/// one keeps the text-only anatomy exactly as before.
fn thumb_el(image: &rune::Image) -> El {
    let [w, h] = image.size().unwrap_or([THUMB_W_PX, THUMB_W_PX]);
    let disp_h = THUMB_W_PT * h as f32 / w.max(1) as f32;
    el()
        .w(THUMB_W_PT)
        .h(disp_h)
        .radius(6.0)
        .pixels(image.pixels())
}

/// The metadata line a row actually renders. When the
/// light read ([`HomePanelState::metadata_for`]) produced
/// figures/frames/size/modified for the row's file, those
/// are the numbers shown — re-verified against the file
/// on disk at its current mtime. The size segment (Phase
/// M) is the byte length from that read's one stat,
/// rendered by project_tools' [`human_size`] — the same
/// formatter the Project section's own meta line uses.
/// Fps is not part of the light read's tuple, so it still
/// comes from the scan. When the light read failed (the
/// file vanished or stopped parsing between the scan and
/// this render) the line is exactly the scan's
/// [`meta_line`] — no size segment (the scan never stats
/// for a length) and the row is unchanged rather than
/// blank.
pub fn row_meta_line(
    info: &ProjectInfo,
    meta: Option<(usize, usize, u64, SystemTime)>,
) -> String {
    match meta {
        Some((figures, frames, size_bytes, modified)) => format!(
            "{} · {} · {} fps · {} · modified {}",
            count_label(figures, "figure", "figures"),
            count_label(frames, "frame", "frames"),
            format_fps(info.fps),
            human_size(size_bytes),
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
/// row intact. On success the confirm clears, all
/// three caches are invalidated ([`HomePanelState::
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

/// The Home library's rename session (UI mission
/// Wave 4): which row's rename editor is open and the
/// field's edit buffer. Homed on `State` (as
/// `State::home_rename`), not in [`HomePanelState`] —
/// the same reason `clips_rename` / `pose_rename` live
/// on `State`: a `text_input`'s `on_edit` needs a
/// `&mut TextEdit` path into `State`, so the session
/// cannot ride inside the panel state the row builders
/// only borrow. The struct itself is defined here, with
/// the panel that drives it.
#[derive(Clone, Debug, Default)]
pub struct HomeRenameState {
    /// The project file whose rename is armed: its row
    /// renders the name field instead of its buttons.
    /// `None` — the resting state — means no rename is
    /// open. Set by a row's Rename button
    /// ([`begin_rename`]); cleared by the cancel press,
    /// by a successful rename, by opening a row, and by
    /// any search edit — and arming a delete stands it
    /// down in turn (one pending row action at a time:
    /// both confirm lines carry index-free keys).
    pub armed: Option<PathBuf>,
    /// The in-progress name, loaded from the armed
    /// project's stored name by [`begin_rename`].
    pub edit: TextEdit,
}

impl HomeRenameState {
    pub fn new() -> Self {
        HomeRenameState {
            armed: None,
            edit: TextEdit::new(""),
        }
    }
}

/// True while `path` is the rename session's armed row
/// ([`confirm_pending`]'s twin).
pub fn rename_pending(rename: &HomeRenameState, path: &Path) -> bool {
    rename.armed.as_deref() == Some(path)
}

/// Arm a row's rename (the Rename button's press):
/// record the path and load the field with the
/// project's current stored name, focused — the clips
/// `begin_rename` shape. Nothing on disk changes until
/// [`dispatch_rename`] runs.
pub fn begin_rename(rename: &mut HomeRenameState, path: PathBuf, name: &str) {
    let mut field = TextEdit::new(name.to_string());
    field.focused = true;
    rename.edit = field;
    rename.armed = Some(path);
}

/// Stand a pending rename down (the rename line's
/// Cancel press, and the clear that opening a row, a
/// search edit, or arming a delete applies). The file
/// is untouched, as it always is until a confirm.
pub fn cancel_rename(rename: &mut HomeRenameState) {
    rename.armed = None;
}

/// Carry out the armed rename: rename the armed row's
/// file via `projects::rename_project` — the file-level
/// rename that had no UI caller until now. The path
/// comes from the session state, never from a row
/// index, so a re-sorted or re-filtered list cannot
/// redirect the rename onto another file
/// ([`dispatch_delete_confirm`]'s discipline).
///
/// Reporting mirrors [`dispatch_duplicate`]: success
/// and failure both land on the status line in the
/// family's "{Verb} failed: {e}" / past-tense shapes.
/// On success the session clears and all three caches
/// are invalidated (the renamed file must re-scan under
/// its new name); on failure the rename stays armed
/// with the typed text intact, so the user can fix the
/// name and press again. Returns whether the rename
/// landed — the caller's cue for the one follow-up
/// this fn deliberately does not perform: the recents
/// re-touch lives in [`confirm_rename`], because
/// `shell::touch_recent` writes the shared on-disk
/// `./projects`, and this dispatch stays unit-testable
/// against a temp dir (the suite's no-pollution rule —
/// see the snap tests' note on `./projects`).
///
/// The backend caveat, stated in the success status
/// itself: `rename_project` renames the FILE only — the
/// name stored inside the project's JSON is unchanged
/// until the project is next saved, so the row keeps
/// showing the stored name; the status says exactly
/// that rather than letting the row look unrenamed.
pub fn dispatch_rename(
    core: &mut EditorCore,
    panel: &mut HomePanelState,
    rename: &mut HomeRenameState,
) -> bool {
    let Some(path) = rename.armed.clone() else {
        return false;
    };
    let new_name = rename.edit.text.trim().to_string();
    match projects::rename_project(&path, &new_name) {
        Ok(new_path) => {
            rename.armed = None;
            panel.invalidate_caches();
            let file = new_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            core.status = format!(
                "Renamed project file to {file}. The name stored inside the file updates when the project is saved."
            );
            true
        }
        Err(e) => {
            core.status = format!("Rename failed: {e}");
            false
        }
    }
}

/// The rename line's confirm press (and Enter in its
/// field) at the `State` level: run [`dispatch_rename`]
/// and, only on success, re-touch the recents list
/// with the new name — recents are name-keyed, and the
/// renamed file should head the list the File menu and
/// the reopen action read.
fn confirm_rename(s: &mut State) {
    let new_name = s.home_rename.edit.text.trim().to_string();
    if dispatch_rename(&mut s.core, &mut s.home_panel, &mut s.home_rename) {
        shell::touch_recent(&new_name);
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

// ---- Session-discard gates (UI mission Wave 4, audit
// ---- S16 D2): the State-level press handlers for the
// ---- session-replacing controls this panel owns. Each
// ---- follows the ConfirmAction contract: while the
// ---- session is dirty (project_tools::session_dirty)
// ---- the first press arms and only the second — on
// ---- the danger re-render of the same button — runs.

/// One row's Open press, and the reopen quick action's:
/// stand down the row-level pending actions, then run
/// the open through the `OpenProject` arm. Opening is a
/// session replace ([`dispatch_open`] swaps the project
/// through the core's open flow), so a dirty session
/// arms first — the audit's D2 names Home's opens
/// alongside project-tools'.
fn open_row_pressed(s: &mut State, path: &Path, name: &str) {
    // Opening a row stands down any pending delete
    // confirm (Phase K) and any pending rename: the
    // armed row's rendering is about to be replaced.
    cancel_delete(&mut s.home_panel);
    cancel_rename(&mut s.home_rename);
    if s.confirm_action != Some(ConfirmAction::OpenProject) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::OpenProject);
        s.core.status =
            "Opening a project would discard unsaved changes — press Open again to confirm."
                .into();
        return;
    }
    s.confirm_action = None;
    dispatch_open(&mut s.core, path, name);
}

/// The "New project" button's press: [`dispatch_new`]
/// through the `NewProject` arm — Home's New replaces
/// the session exactly as project-tools' does, so it
/// carries the same gate.
fn new_project_pressed(s: &mut State) {
    if s.confirm_action != Some(ConfirmAction::NewProject) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::NewProject);
        s.core.status =
            "New project would discard unsaved changes — press New again to confirm.".into();
        return;
    }
    s.confirm_action = None;
    dispatch_new(&mut s.core);
}

/// The recovery banner's Recover press:
/// [`dispatch_recover`] through the `RecoverSnapshot`
/// arm (audit S06-3 — both Recover buttons route
/// through the arm/confirm pattern). The banner only
/// renders while the S06-2 offer holds, so the gate is
/// the arm alone; a confirming (or clean) press runs
/// the recover and clears the offer, as the unguarded
/// press did.
fn recover_banner_pressed(s: &mut State) {
    if s.confirm_action != Some(ConfirmAction::RecoverSnapshot) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::RecoverSnapshot);
        s.core.status =
            "Recovering the snapshot would discard unsaved changes — press Recover again to confirm."
                .into();
        return;
    }
    s.confirm_action = None;
    dispatch_recover(&mut s.core);
    s.home_panel.recovery_offer = false;
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

/// The recovery banner: shown only while
/// [`HomePanelState::recovery_offer`] holds (the verdict
/// captured at open time). Recover runs the real snapshot
/// load and clears the offer — through the
/// `RecoverSnapshot` arm while the session is dirty
/// (UI mission Wave 4, audit S06-3): `armed` swaps the
/// button for its danger re-render under the same key,
/// and the confirming press is [`recover_banner_pressed`]
/// again.
fn recovery_banner(s: &State, t: &Theme) -> El {
    let accent = tok::accent(0, true);
    let armed = s.confirm_action == Some(ConfirmAction::RecoverSnapshot);
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
        .child(if armed {
            danger_btn("recover", "Discard changes?", t, recover_banner_pressed)
        } else {
            btn("recover", "Recover", false, t, recover_banner_pressed)
        })
}

/// One project row: the scan-time thumbnail (Wave 4,
/// when this file version has one), then name + real
/// metadata on the left, Open / Duplicate / Delete on
/// the right. Keys carry the GPUI
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
    meta: Option<(usize, usize, u64, SystemTime)>,
    thumb: Option<rune::Image>,
) -> El {
    project_row_confirm(info, i, t, meta, false, thumb)
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
    meta: Option<(usize, usize, u64, SystemTime)>,
    confirming: bool,
    thumb: Option<rune::Image>,
) -> El {
    project_row_marked(info, i, t, meta, confirming, false, thumb, false, None, false)
}

/// [`project_row_confirm`] with the Phase N current
/// marking: the row shell is the kit list row
/// ([`design::list_row`]) under the same `home-row-{i}`
/// key — the currently open project's row (`selected`,
/// from [`is_current_project`]) carries the kit selected
/// fill, and the armed confirm line renders on that
/// same selected fill (P10's selected-for-danger read)
/// with the family's [`danger_btn`] as its confirm.
///
/// UI mission Wave 4 adds the row's two pending states:
/// `renaming` (this row's path is the rename session's
/// armed path, [`rename_pending`]) renders the row
/// instead as the rename line — the name field
/// (`rename_edit`, the session's buffer, passed
/// alongside because the row builders borrow no
/// `State`) with its confirm/cancel — and `open_armed`
/// (a session discard is armed on Open) re-renders the
/// Open button as its danger form under the same key.
#[allow(clippy::too_many_arguments)]
fn project_row_marked(
    info: &ProjectInfo,
    i: usize,
    t: &Theme,
    meta: Option<(usize, usize, u64, SystemTime)>,
    confirming: bool,
    selected: bool,
    thumb: Option<rune::Image>,
    renaming: bool,
    rename_edit: Option<&TextEdit>,
    open_armed: bool,
) -> El {
    if renaming {
        if let Some(edit) = rename_edit {
            return design::list_row(&format!("home-row-{i}"), selected, t)
                .w_full()
                .child(
                    text_input(edit, type_::BODY.lh(26.0))
                        .key("home-proj-rename-field")
                        .single_line()
                        .grow(1.0)
                        .color(t.text)
                        .on_edit(|s: &mut State| &mut s.home_rename.edit)
                        .on_submit(|s: &mut State, _tx: &mut Tx| {
                            confirm_rename(s);
                        }),
                )
                .child(btn(
                    "home-proj-rename-confirm",
                    "Rename",
                    false,
                    t,
                    |s: &mut State| {
                        confirm_rename(s);
                    },
                ))
                .child(btn(
                    "home-proj-rename-cancel",
                    "Cancel",
                    false,
                    t,
                    |s: &mut State| {
                        cancel_rename(&mut s.home_rename);
                    },
                ));
        }
        // `renaming` without the buffer cannot be built
        // honestly (the home_panel loop always pairs
        // them); fall through to the ordinary row.
    }
    if confirming {
        let name = info.name.clone();
        let confirm_name = info.name.clone();
        return design::list_row(&format!("home-row-{i}"), true, t)
            .w_full()
            .child(
                text(format!("Delete {name}?"))
                    .font(type_::TITLE)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(danger_btn(
                "home-proj-del-confirm",
                "Delete file",
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
    let rename_key = format!("home-proj-rename-{i}");
    let del_key = format!("home-proj-del-{i}");
    let path = info.path.clone();
    let dup_path = info.path.clone();
    let rename_path = info.path.clone();
    let del_path = info.path.clone();
    let name = info.name.clone();
    let rename_name = info.name.clone();
    // The thumbnail (Wave 4) leads the row when the
    // scan-time render produced one for this file
    // version; without one the row keeps its text-only
    // anatomy. The confirm line above never carries
    // one — it replaces the row's content wholesale.
    let mut row_el = design::list_row(&format!("home-row-{i}"), selected, t).w_full();
    if let Some(image) = &thumb {
        row_el = row_el.child(thumb_el(image));
    }
    row_el
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
        .child(if open_armed {
            danger_icon_btn(&open_key, "folder-open", "Discard changes?", false, t, move |s: &mut State| {
                open_row_pressed(s, &path, &name);
            })
        } else {
            icon_btn(&open_key, "folder-open", "Open", false, t, move |s: &mut State| {
                // Opening a row stands down any pending
                // delete confirm (Phase K): the armed row is
                // never this one — an armed row renders its
                // confirm line instead of an Open button.
                // The press itself runs through the
                // session-discard gate (Wave 4).
                open_row_pressed(s, &path, &name);
            })
        })
        .child(btn(&dup_key, "Duplicate", false, t, move |s: &mut State| {
            dispatch_duplicate(&mut s.core, &dup_path);
        }))
        .child(btn(&rename_key, "Rename", false, t, move |s: &mut State| {
            // Arming a rename replaces this row's
            // rendering, so it stands down the panel's
            // other pending actions: a delete confirm
            // (one index-free confirm line at a time)
            // and any armed session discard.
            cancel_delete(&mut s.home_panel);
            s.confirm_action = None;
            begin_rename(&mut s.home_rename, rename_path.clone(), &rename_name);
        }))
        .child(btn(&del_key, "Delete", false, t, move |s: &mut State| {
            // Arming a delete likewise stands down a
            // pending rename and any armed session
            // discard (the confirm_delete discipline,
            // extended to the panel's newer sessions).
            cancel_rename(&mut s.home_rename);
            s.confirm_action = None;
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
    let mut panel = design::panel_col("home-panel", t);
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
    // The file the core's selector points at — the open
    // project. Resolved once here: it feeds both the
    // reopen target and the rows' current marking
    // (`is_current_project`).
    let current_path = current_project_path(&s.core);
    let reopen = reopen_target(&library, last_opened.as_deref(), current_path);
    let mut card = col().key("home-card").gap(8.0);
    if let Some(target) = reopen {
        let path = target.path.clone();
        let name = target.name.clone();
        // Reopening is an open: it stands down any
        // pending delete confirm (Phase K) and runs
        // through the session-discard gate (Wave 4) —
        // while that gate is armed on Open, the button
        // re-renders as its danger form under the same
        // key, with the plain label replaced by the
        // discard it would confirm.
        if s.confirm_action == Some(ConfirmAction::OpenProject) {
            card = card.child(danger_icon_btn(
                "home-reopen-last",
                "arrow-rotate-right",
                "Discard changes?",
                true,
                t,
                move |s: &mut State| {
                    open_row_pressed(s, &path, &name);
                },
            ));
        } else {
            card = card.child(icon_btn(
                "home-reopen-last",
                "arrow-rotate-right",
                &reopen_label(target),
                true,
                t,
                move |s: &mut State| {
                    open_row_pressed(s, &path, &name);
                },
            ));
        }
    }
    card = card.child(
        design::header_row("home-header")
            .child(design::panel_title(
                None,
                "Saved projects in ./projects",
                None,
                t,
            ))
            .child(if s.confirm_action == Some(ConfirmAction::NewProject) {
                danger_icon_btn("home-new", "file", "Discard changes?", false, t, new_project_pressed)
            } else {
                icon_btn("home-new", "file", "New project", false, t, new_project_pressed)
            }),
    );
    card = card.child(design::divider(t));
    if home.recovery_offer {
        card = card.child(recovery_banner(s, t));
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
                    // under the armed row. The rename
                    // session and any armed session
                    // discard stand down with it (Wave 4):
                    // both are rendered against the list
                    // the edit is about to change.
                    .on_input_change(|s: &mut State, _tx: &mut Tx| {
                        cancel_delete(&mut s.home_panel);
                        cancel_rename(&mut s.home_rename);
                        s.confirm_action = None;
                    }),
            )
            .child(design::section_label(
                &format!(
                    "{} {}",
                    shown.len(),
                    if shown.len() == 1 { "project" } else { "projects" }
                ),
                t,
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
        card = card.child(design::empty_state(
            if home.search.text.is_empty() {
                "folder-open"
            } else {
                "magnifying-glass"
            },
            if home.search.text.is_empty() {
                "No saved projects yet — use \"Save project\" to add one."
            } else {
                "No projects match the search."
            },
            t,
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
            if let Some((figures, frames, _, _)) = meta {
                metas.push((figures, frames));
            }
            let confirming = confirm_pending(home, &info.path);
            let renaming = rename_pending(&s.home_rename, &info.path);
            // The rename line's field buffer, paired
            // with `renaming` (the row builder borrows
            // no State, so the loop hands it over).
            let rename_edit = if renaming {
                Some(&s.home_rename.edit)
            } else {
                None
            };
            let open_armed = s.confirm_action == Some(ConfirmAction::OpenProject);
            let selected = is_current_project(info, current_path);
            // A cache read only — the render itself
            // happened at scan time (see `projects`).
            let thumb = home.thumbnail_for(info);
            rows = rows.child(project_row_marked(
                info,
                i,
                t,
                meta,
                confirming,
                selected,
                thumb,
                renaming,
                rename_edit,
                open_armed,
            ));
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
    fn project_metadata_reads_counts_size_and_mtime_from_a_light_parse() {
        let dir = temp_dir("metadata-parse");
        let path = dir.join("meta.json");
        let written = project_json(3, 7);
        let expected_size = written.len() as u64;
        std::fs::write(&path, written).unwrap();
        let (figures, frames, size_bytes, modified) = project_metadata(&path).unwrap();
        assert_eq!(figures, 3);
        assert_eq!(frames, 7);
        // The size is the file's byte length — the exact
        // bytes written, from the same stat as the mtime.
        assert_eq!(size_bytes, expected_size);
        // The returned stamp is the file's own mtime.
        let stat = std::fs::metadata(&path).unwrap();
        assert_eq!(modified, stat.modified().unwrap());
        assert_eq!(size_bytes, stat.len());
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
        let mut cache: HashMap<PathBuf, (SystemTime, usize, usize, u64)> = HashMap::new();
        // Empty cache misses.
        assert_eq!(cached_meta(&cache, &path, t0), None);
        cache.insert(path.clone(), (t0, 4, 9, 1_500));
        // Same path, the mtime it was read at: a hit —
        // counts and size served together.
        assert_eq!(cached_meta(&cache, &path, t0), Some((4, 9, 1_500)));
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
        let (figures, frames, size_bytes, read_at) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (2, 5));
        assert_eq!(size_bytes, project_json(2, 5).len() as u64);

        // Rewrite the file with different counts (and a
        // different length) but pin its mtime back to the
        // cached read's stamp: the accessor must serve the
        // cached counts and size — proof it did not
        // re-read (a re-read would see 6, 1 and the new
        // length).
        std::fs::write(&path, project_json(6, 1)).unwrap();
        set_mtime(&path, read_at);
        let (figures, frames, size_bytes, modified) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (2, 5), "cache served, no re-read");
        assert_eq!(
            size_bytes,
            project_json(2, 5).len() as u64,
            "the cached read's size is served with its counts"
        );
        assert_eq!(modified, read_at);

        // Move the mtime: the entry is stale, the file is
        // re-read, and the new counts and size land.
        let later = read_at + std::time::Duration::from_secs(60);
        set_mtime(&path, later);
        let (figures, frames, size_bytes, modified) = state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (6, 1));
        assert_eq!(size_bytes, project_json(6, 1).len() as u64);
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
        // The light read's figures/frames/size/modified
        // win; fps still comes from the scan (it is not
        // part of the light tuple). The size segment sits
        // between fps and modified, formatted by
        // `human_size` (1_500 bytes → "1.5 KB").
        assert_eq!(
            row_meta_line(&entry, Some((3, 12, 1_500, stamp))),
            "3 figures · 12 frames · 24 fps · 1.5 KB · modified 2026-11-08 05:57"
        );
        // No light read: the line is the scan's, unchanged
        // — the old shape exactly, with no size segment.
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
        let _ = project_row(&entry, 0, &t, None, None);
        let stamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426);
        let _ = project_row(&entry, 0, &t, Some((3, 12, 1_500, stamp)), None);
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
        let _ = project_row_confirm(&entry, 0, &t, None, false, None);
        let _ = project_row_confirm(&entry, 0, &t, None, true, None);
    }

    #[test]
    fn is_current_project_matches_by_file_name() {
        // The Phase N current marking follows the panel's
        // established rule: the scan's relative path and
        // the core's cwd-absolute path are the same
        // project when their file names agree.
        let entry = info("Demo", "projects/demo.json");
        assert!(is_current_project(
            &entry,
            Some(Path::new("/abs/projects/demo.json"))
        ));
        assert!(is_current_project(
            &entry,
            Some(Path::new("projects/demo.json"))
        ));
        assert!(!is_current_project(
            &entry,
            Some(Path::new("/abs/projects/other.json"))
        ));
        // No current file (the core lists none): nothing
        // is marked.
        assert!(!is_current_project(&entry, None));
    }

    #[test]
    fn project_row_marked_builds_in_both_markings() {
        // Builder smoke test for the Phase N entry point:
        // the kit list-row shell constructs with and
        // without the current marking, and the armed
        // confirm line constructs on the marked shell.
        let t = tok::theme(true);
        let entry = info("Demo", "projects/demo.json");
        let _ = project_row_marked(&entry, 0, &t, None, false, false, None, false, None, false);
        let _ = project_row_marked(&entry, 0, &t, None, false, true, None, false, None, false);
        let _ = project_row_marked(&entry, 0, &t, None, true, true, None, false, None, false);
        // The Wave 4 states construct too: the rename
        // line (with its field buffer) and the armed
        // Open re-render.
        let edit = TextEdit::new("Demo");
        let _ = project_row_marked(&entry, 0, &t, None, false, false, None, true, Some(&edit), false);
        let _ = project_row_marked(&entry, 0, &t, None, false, false, None, false, None, true);
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

    #[test]
    fn begin_and_cancel_rename_track_the_armed_row() {
        let mut rename = HomeRenameState::new();
        assert_eq!(rename.armed, None);
        assert!(!rename_pending(&rename, Path::new("projects/demo.json")));
        begin_rename(&mut rename, PathBuf::from("projects/demo.json"), "Demo");
        assert!(rename_pending(&rename, Path::new("projects/demo.json")));
        assert!(!rename_pending(&rename, Path::new("projects/other.json")));
        // The field opens loaded with the stored name
        // and focused (the clips begin_rename shape).
        assert_eq!(rename.edit.text, "Demo");
        assert!(rename.edit.focused);
        cancel_rename(&mut rename);
        assert_eq!(rename.armed, None);
        // Cancelling with nothing armed is a no-op.
        cancel_rename(&mut rename);
        assert_eq!(rename.armed, None);
    }

    #[test]
    fn dispatch_rename_with_nothing_armed_changes_nothing() {
        let mut core = EditorCore::new();
        core.status = "Sentinel.".into();
        let mut panel = HomePanelState::default();
        let mut rename = HomeRenameState::new();
        assert!(!dispatch_rename(&mut core, &mut panel, &mut rename));
        assert_eq!(core.status, "Sentinel.");
    }

    #[test]
    fn dispatch_rename_renames_the_file_and_reports_the_caveat() {
        // A real file in a real temp dir: the rename
        // lands on disk under the new name's slug, the
        // session disarms, the caches drop for the
        // re-scan, and the status names the new file —
        // plus the stored-name caveat, honestly.
        let dir = temp_dir("rename");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        *panel.scan_cache.borrow_mut() =
            Some((None, vec![info("Demo", "projects/demo.json")]));
        let mut rename = HomeRenameState::new();
        begin_rename(&mut rename, path.clone(), "Demo");
        rename.edit.replace_text("My Film".to_string());
        let mut core = EditorCore::new();
        assert!(dispatch_rename(&mut core, &mut panel, &mut rename));
        assert!(!path.exists(), "the old file is gone");
        assert!(dir.join("my-film.json").exists(), "renamed to the new slug");
        assert_eq!(rename.armed, None, "success disarms");
        assert!(panel.scan_cache.borrow().is_none(), "scan cache dropped");
        assert_eq!(
            core.status,
            "Renamed project file to my-film.json. The name stored inside the file updates when the project is saved."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_rename_refuses_a_taken_name_and_stays_armed() {
        // The AlreadyExists refusal (projects.rs): the
        // status carries it, both files stay put, and
        // the rename stays armed with the typed text so
        // the user can adjust and press again.
        let dir = temp_dir("rename-taken");
        let path = dir.join("demo.json");
        let taken = dir.join("my-film.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        std::fs::write(&taken, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        let mut rename = HomeRenameState::new();
        begin_rename(&mut rename, path.clone(), "Demo");
        rename.edit.replace_text("My Film".to_string());
        let mut core = EditorCore::new();
        assert!(!dispatch_rename(&mut core, &mut panel, &mut rename));
        assert!(core.status.starts_with("Rename failed:"), "{}", core.status);
        assert!(core.status.contains("already exists"), "{}", core.status);
        assert!(rename_pending(&rename, &path), "failure keeps the rename armed");
        assert_eq!(rename.edit.text, "My Film", "the typed name survives");
        assert!(path.exists() && taken.exists(), "both files untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_rename_rejects_a_name_with_no_file_slug() {
        // A whitespace-only name slugs to nothing:
        // rename_project's InvalidInput refusal, in the
        // family's failure shape.
        let dir = temp_dir("rename-empty");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        let mut rename = HomeRenameState::new();
        begin_rename(&mut rename, path.clone(), "Demo");
        rename.edit.replace_text("   ".to_string());
        let mut core = EditorCore::new();
        assert!(!dispatch_rename(&mut core, &mut panel, &mut rename));
        assert!(core.status.starts_with("Rename failed:"), "{}", core.status);
        assert!(path.exists(), "the file is untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn home_new_arms_first_while_dirty_then_confirms() {
        // UI mission Wave 4 (S16 D2): the gate handlers
        // run on a full State (crate::init's, the
        // command-palette tests' precedent).
        let mut s = crate::init();
        s.core.project.name = "Unsaved Work".into();
        s.core.snapshot();
        new_project_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::NewProject));
        assert_eq!(s.core.project.name, "Unsaved Work", "first press only arms");
        new_project_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "confirm clears the arm");
        assert_eq!(s.core.status, "New project.", "second press runs the swap");
    }

    #[test]
    fn home_open_arms_first_while_dirty() {
        let mut s = crate::init();
        let before = s.core.project.name.clone();
        s.core.snapshot();
        open_row_pressed(&mut s, Path::new("projects/demo.json"), "Demo");
        assert_eq!(s.confirm_action, Some(ConfirmAction::OpenProject));
        assert_eq!(s.core.project.name, before, "arming does not open anything");
        assert!(s.core.status.contains("press Open again"), "{}", s.core.status);
    }

    #[test]
    fn banner_recover_arms_first_while_dirty() {
        let mut s = crate::init();
        s.home_panel.recovery_offer = true;
        s.core.snapshot();
        recover_banner_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::RecoverSnapshot));
        assert!(s.home_panel.recovery_offer, "arming keeps the banner up");
        recover_banner_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "confirm clears the arm");
        assert!(!s.home_panel.recovery_offer, "the confirming press clears the offer");
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
        // it, clears the armed path, drops all three
        // caches — a stale row must never render — and
        // reports the deletion by the project's display
        // name.
        let dir = temp_dir("delete-confirm");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        // Seed all three caches as earlier renders would
        // have.
        *panel.scan_cache.borrow_mut() =
            Some((None, vec![info("Demo", "projects/demo.json")]));
        panel
            .meta_cache
            .borrow_mut()
            .insert(path.clone(), (SystemTime::UNIX_EPOCH, 1, 1, 0));
        panel
            .thumb_cache
            .borrow_mut()
            .insert(path.clone(), (SystemTime::UNIX_EPOCH, None));
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
        assert!(
            panel.thumb_cache.borrow().is_empty(),
            "thumbnail cache dropped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Write a real, renderable project file — the demo
    /// project serialised, the shape `projects::load`
    /// parses back — and return its path and the mtime
    /// the filesystem reports for it (the value a scan
    /// entry for this file would carry).
    fn write_demo_project(dir: &Path, file: &str) -> (PathBuf, SystemTime) {
        let path = dir.join(file);
        let json = serde_json::to_string(&Project::new_demo()).unwrap();
        std::fs::write(&path, json).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        (path, mtime)
    }

    /// A scan entry for `path` carrying the given mtime,
    /// in the shape `projects::scan` would report it.
    fn scan_entry(name: &str, path: &Path, mtime: Option<SystemTime>) -> ProjectInfo {
        ProjectInfo {
            name: name.into(),
            path: path.to_path_buf(),
            figures: 1,
            frames: 5,
            fps: 12.0,
            modified: mtime,
        }
    }

    #[test]
    fn render_thumbnail_renders_frame_zero_at_the_fixed_width() {
        // The generation half of the feature, end to end:
        // a real project file renders to a real Rune
        // image at the recipe's fixed width (the height
        // keeps the demo stage's aspect — the exact value
        // is the stage's business, only its positivity
        // and the width are pinned here).
        let dir = temp_dir("thumb-render");
        let (path, _mtime) = write_demo_project(&dir, "demo.json");
        let image = render_thumbnail(&path).expect("the demo project renders");
        let [w, h] = image.size().expect("a waited image knows its size");
        assert_eq!(w, THUMB_W_PX);
        assert!(h >= 1, "the thumbnail has a real height");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_thumbnail_fails_honestly_for_missing_and_unparseable_files() {
        // No file and no parse both yield no thumbnail —
        // never a placeholder, never a panic.
        let dir = temp_dir("thumb-fail");
        assert!(render_thumbnail(&dir.join("gone.json")).is_none());
        let junk = dir.join("junk.json");
        std::fs::write(&junk, b"not json").unwrap();
        assert!(render_thumbnail(&junk).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn thumbnail_fill_and_serve_follow_the_scan_mtime() {
        // The cache discipline, end to end: nothing is
        // served before a fill; after a fill the render
        // is served for the entry whose mtime it was made
        // at; a refill at the same mtime keeps the same
        // handle (no re-render); an entry whose mtime has
        // moved — or which carries no stat at all — is
        // served nothing.
        let dir = temp_dir("thumb-cache");
        let (path, mtime) = write_demo_project(&dir, "demo.json");
        let entry = scan_entry("Demo", &path, Some(mtime));
        let state = HomePanelState::default();
        assert!(state.thumbnail_for(&entry).is_none());
        state.fill_thumbnails(std::slice::from_ref(&entry));
        let image = state
            .thumbnail_for(&entry)
            .expect("the fill's render is served");
        assert_eq!(image.size().map(|s| s[0]), Some(THUMB_W_PX));
        let id = image.id();
        state.fill_thumbnails(std::slice::from_ref(&entry));
        assert_eq!(
            state.thumbnail_for(&entry).map(|im| im.id()),
            Some(id),
            "same mtime: the cached handle is kept, not re-rendered"
        );
        let moved = scan_entry(
            "Demo",
            &path,
            Some(mtime + std::time::Duration::from_secs(60)),
        );
        assert!(state.thumbnail_for(&moved).is_none());
        let nostat = scan_entry("Demo", &path, None);
        assert!(state.thumbnail_for(&nostat).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn thumbnail_fill_skips_shell_owned_and_caches_failures() {
        // The recovery snapshot — a fully renderable
        // project file — is never thumbnailed: it is not
        // a library row. An unparseable file's failure is
        // cached as `None` at its mtime, so the fill does
        // not re-render it on every scan. And a scan that
        // stops listing a file drops its handle.
        let dir = temp_dir("thumb-skip");
        let (recovery_path, mtime) = write_demo_project(&dir, shell::RECOVERY_FILE);
        let recovery = scan_entry("Untitled", &recovery_path, Some(mtime));
        let junk = dir.join("junk.json");
        std::fs::write(&junk, b"not json").unwrap();
        let junk_mtime = std::fs::metadata(&junk).unwrap().modified().unwrap();
        let junk_entry = scan_entry("Junk", &junk, Some(junk_mtime));
        let state = HomePanelState::default();
        state.fill_thumbnails(&[recovery, junk_entry.clone()]);
        assert!(
            !state.thumb_cache.borrow().contains_key(&recovery_path),
            "shell-owned files get no thumbnail entry"
        );
        assert!(
            matches!(state.thumb_cache.borrow().get(&junk), Some((_, None))),
            "the failed render is cached as None"
        );
        assert!(state.thumbnail_for(&junk_entry).is_none());
        state.fill_thumbnails(&[]);
        assert!(
            state.thumb_cache.borrow().is_empty(),
            "an unlisted file's handle is dropped by the fill"
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
