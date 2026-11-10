//! Home / projects panel (Phase D, section 21/40 of the Rune
//! rebuild): the saved-project library — a search field, one
//! row per project in `./projects` (name, figures, frames,
//! duration, fps, modified), an Open and a Duplicate button per row, a
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
//! - **Row duration (UI mission Wave 9)** — both forms of
//!   the metadata line now carry the project's runtime
//!   between the frame count and the fps:
//!   [`duration_segment`] computes it as frames ÷ fps at
//!   one frame-period per frame, from numbers already in
//!   hand (the light read's or the scan's frame count,
//!   the scan's fps) — no new disk read, no new parse.
//!   It is a library-level estimate: the model's
//!   per-frame duration multipliers are not part of
//!   either data source (`ProjectInfo` has no summed
//!   durations, and the light read counts arrays only),
//!   so they are not folded in. An fps that is zero,
//!   negative or non-finite yields no segment at all —
//!   the line keeps its previous shape rather than
//!   printing a duration it cannot honestly compute.
//!   The line's rendering was already the kit's
//!   secondary treatment (`LABEL` in `text_secondary`,
//!   the same pairing the kit's own list-row meta uses),
//!   and the sort default was already the scan's
//!   newest-first order with the `home-sort` toggle as
//!   its control — both audited under Wave 9, neither
//!   needed a change.
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
//! ## Wave 10 round 2 — rename honesty, the row budget
//!
//! A second audit pass over this panel, hunting the
//! states the first rounds never rendered (no headless
//! tree has ever contained a populated row: the CI
//! library is empty, and the `home` scene renders this
//! panel closed). Findings, all fixed in this file:
//!
//! - **Rename vs reopen.** [`confirm_rename`] re-touches
//!   the recents with the typed name, but rows (and
//!   [`reopen_target`]) keyed on the *stored* name,
//!   which a file-only rename does not move — the
//!   reopen offer silently vanished after every rename,
//!   evicting whatever valid offer the recents front
//!   held before it. `reopen_target` now falls back to
//!   the file-stem match the shell's recovery check
//!   already stats through.
//! - **Rename vs the core's file list.** A successful
//!   rename points `EditorCore::project_files` at the
//!   new file name ([`dispatch_rename`]); the list
//!   otherwise named a path that no longer existed,
//!   losing the row's current marking and breaking a
//!   later open through the core. (The File menu's
//!   Open-recent block resolves recents by stored name
//!   the same pre-Wave-10 way — menus.rs, outside this
//!   panel; its own fallback status covers the miss.)
//! - **The no-op rename.** A typed name that slugs to
//!   the file's own name is a backend no-op returning
//!   the same path; it is reported as one instead of
//!   claiming a rename, and does not re-touch the
//!   recents.
//! - **The row budget.** Measured with the calibrated
//!   charge model ([`label_char_width`]) against the
//!   left column's 276 pt content width: the old
//!   single-line row's buttons alone came to 286 pt,
//!   the metadata line to ~375–575 pt unwrapped, the
//!   delete-confirm question's share to ~104 pt, the
//!   recovery banner's sentence to ~365 pt in a ~176 pt
//!   share, the reopen label unbounded, and the header
//!   title to ~183 pt into a ~150 pt cluster. The row
//!   now stacks (name truncated to [`ROW_NAME_BUDGET`],
//!   metadata wrapped, buttons on a wrapping row inside
//!   the text column), the confirm question wraps on
//!   its own line over its buttons (the full name stays
//!   readable — it is a destructive confirm), the
//!   banner text and the totals line wrap, the reopen
//!   label is bounded ([`REOPEN_LABEL_BUDGET`]), and
//!   the header title reads "Saved projects". Keys and
//!   handlers are byte-identical throughout. Audited
//!   clean in the same pass: the search row (~213 pt of
//!   fixed anatomy at a three-digit count), the rename
//!   line (167 pt fixed; the field takes the rest), the
//!   mutual stand-down discipline between the rename,
//!   delete and session-discard arms, the empty-name
//!   and collision refusals (both land as
//!   "Rename failed: …", the status grammar's Error
//!   arm, with the session kept armed), and the New
//!   button, which installs the starter project and
//!   touches no file, recent or list entry — exactly
//!   its label.
//!
//! ## Wave 11 — state sync when the project changes outside the GUI
//!
//! A live MCP client drives the same `EditorCore` the
//! GUI renders (the `live_mcp` drain in `main.rs::poll`),
//! and a stdio MCP server shares the `./projects`
//! directory. Audited step by step against the GUI's
//! own swap paths, the panel's side of that sync is:
//!
//! - **Baseline / session reset** — core-side, and the
//!   MCP swap paths already call them
//!   (`note_project_swapped` in `mcp_tools::tool_new_project`
//!   and `mcp_io::replace_project_open` /
//!   `replace_project_import`). Nothing panel-side.
//! - **The library listing** — sound for every MCP
//!   write that creates or replaces a directory entry
//!   (stdio and live `save_project` write atomically,
//!   temp + rename: the directory mtime moves and the
//!   scan refills). The one blind spot was an *in-place*
//!   rewrite of the open project's file — the shape of
//!   `save_named_project` (GUI and live MCP alike) —
//!   which moves no directory mtime: the row kept its
//!   pre-save thumbnail indefinitely (the light metadata
//!   already re-stats per file, so counts were never
//!   the stale part). [`HomePanelState::sync_open_file`]
//!   now watches that one file's mtime per render and
//!   drops the caches when it moves.
//! - **The recovery banner** — the offer is a verdict
//!   about the session it was captured in; a swap
//!   behind the panel (no panel code runs on the MCP
//!   path) now suppresses it via the captured
//!   [`CoreMark`] ([`HomePanelState::recovery_offer_live`])
//!   instead of offering the old session's snapshot over
//!   the MCP-loaded project — a press the dirty gate
//!   cannot guard, because the swap itself restarted
//!   the baseline the gate reads.
//! - **What the panel cannot fix** (integrator items,
//!   E9's Wave 11 report): the core's `project_files` /
//!   `project_index` after an MCP load (so
//!   [`current_project_path`] — and with it the rows'
//!   current marking and the reopen suppression — names
//!   the loaded file), the recents list on MCP
//!   loads/saves (no MCP path calls
//!   `shell::touch_recent`, so Reopen and the File menu
//!   keep pointing at the last GUI-touched project), and
//!   the shell-side index selections (`State::
//!   stroke_selected` and its siblings) after a live
//!   drain swaps the project — the C6 clear lives in
//!   GUI press handlers the drain never runs.
//!
//! ## Wave 12 (D1) — the populated-Home audit
//!
//! The first pass over this panel aimed at a populated
//! library (the `home-populated` headless scene, D2).
//! Three defects, all fixed in this file:
//!
//! - **The scan cache's blind spot, generalised.**
//!   The Wave 11 watch ([`HomePanelState::
//!   sync_open_file`]) covers an in-place rewrite of
//!   the *open* project's file; a rewrite of any other
//!   listed file — a stdio MCP server saving a project
//!   (its core's current file is not the GUI's), an
//!   external editor — moves no directory mtime and
//!   was invisible to the cache: the row kept its
//!   pre-rewrite name and fps, and
//!   [`HomePanelState::thumbnail_for`] kept serving
//!   the pre-rewrite frame 0, because both key on the
//!   scan entry's mtime, which never moved. A cache
//!   hit in [`HomePanelState::projects`] now verifies
//!   the fingerprint first ([`scan_entries_stale`]):
//!   one stat per listed library file, the row loop's
//!   existing cost class. A refill is the ordinary
//!   one — `projects::scan` re-reads only the changed
//!   file (its own stamp cache, projects.rs), and the
//!   thumbnail fill re-renders only the file whose
//!   mtime moved.
//! - **Thumbnail render bound.** [`render_thumbnail`]
//!   rasterises at the exporter's stage ×2 before any
//!   size check ran, and a project file's `stage` is
//!   raw JSON — the editor's own setter clamps each
//!   axis to 4096 (editor.rs `set_stage_size`); a file
//!   is bound by nothing. A stage past that made the
//!   scan-time fill attempt an allocation the machine
//!   cannot serve, taking the app down over one bad
//!   file; the render is now refused up front
//!   ([`THUMB_MAX_RENDER_SIDE`]) and cached as no
//!   thumbnail, like any other unrenderable file.
//! - **Control characters in budgeted labels.** A
//!   stored name is file content; a newline in one
//!   defeated the single-line width budgets the row
//!   name and the reopen label are truncated under
//!   (Wave 10's overflow class, through the vertical
//!   door). [`display_name`] folds control characters
//!   to spaces for those two display strings only —
//!   byte-identical for every name without them. The
//!   delete confirm still quotes the stored name
//!   verbatim (it wraps, and a destructive confirm
//!   must name the real file); handlers, the search
//!   and the rename field keep the stored name itself.
//!
//! ## Wave S (S7) — the missing-audio marker
//!
//! Wave P surfaced missing audio files when a project
//! *opens* (P14) and on MP4 export (P15); until this
//! wave nothing on Home said a *listed* project had
//! missing audio before it was opened. The light read
//! now answers it from the same parse: alongside the
//! figures/frames counts it deserialises the file's
//! top-level `audio` array into clips and classifies
//! them with the one shared helper —
//! `audio::missing_audio_clips`, the classification
//! the open-time note, the export path and the Audio
//! panel all use (Wave Q, Q20) — so a muted clip never
//! counts and an existing file never counts, here
//! exactly as there. The count rides the mtime meta
//! cache with the counts (fifth element of the light
//! tuple), and a row whose count is non-zero closes
//! its metadata line with the marker segment
//! ([`missing_audio_segment`]: "· 1 audio clip
//! missing" / "· N audio clips missing"). No new
//! element keys: the marker is text in the existing
//! keyed meta line. The honest scope, stated plainly:
//! the marker rides the *light-read* line only — the
//! scan fallback line ([`meta_line`], shown when the
//! file cannot be re-read at render time) carries no
//! marker, because the scan never inspects audio; and
//! clip paths resolve exactly as the shared helper
//! resolves them (relative to the process working
//! directory), the same resolution the open-time
//! check makes, so the marker can never disagree
//! with the verdict opening the project would give
//! for the same file version. A file whose `audio`
//! array will not deserialise as clips fails the
//! whole light read (the row falls back to the scan
//! line) — the scan's full `Project` parse rejects
//! such a file too, so a listed row can only reach
//! that state if the file changed between the scan
//! and this render, which is the fallback's existing
//! case. A file with no `audio` key at all carries
//! none (`Project::audio` is serde-defaulted): its
//! count is 0 and its line is unchanged.
//!
//! Wave T (T6) closed the fallback half of that scope:
//! the scan in `projects.rs` now classifies the clips
//! of the project it already fully parses, with the
//! same shared helper, and carries the count on
//! `ProjectInfo::missing_audio` — so the scan fallback
//! line ([`meta_line`]) closes with the same marker
//! whenever the scan's count is known and non-zero.
//! A `None` count (an entry no scan ever parsed)
//! renders no marker: unknown is not zero, and the
//! line claims no health it did not verify. The two
//! lines can therefore disagree only in freshness —
//! the light read re-verifies at render time, the
//! scan's count is as of its parse (cached under the
//! project file's stamp) — never in classification,
//! which is the one helper's everywhere.
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
use rune::{col, el, row, text, El, TextEdit, Tx};
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
    /// successful Recover press, or by a Discard press
    /// (which also deletes the snapshot file, so the
    /// offer does not return at the next capture).
    pub recovery_offer: bool,
    /// The Wave 15 scan cache: `(projects-dir mtime, scan)`
    /// — `None` until the first render fills it. Interior
    /// mutability because the panel builder only holds
    /// `&State`, exactly as in the GPUI panel.
    scan_cache: RefCell<Option<(Option<SystemTime>, Vec<ProjectInfo>)>>,
    /// Per-file light-metadata cache (Phase J):
    /// path → `(mtime, figures, frames, size_bytes,
    /// missing_audio)` as last read by
    /// [`project_metadata`] (the size joined in Phase
    /// M, from the same stat as the mtime; the
    /// missing-audio count in Wave S, from the same
    /// parse). An entry is served only while
    /// the file's current mtime equals the stored one —
    /// the same mtime-key discipline as the scan cache,
    /// one level down (per file, not per directory).
    /// Interior mutability for the same reason as
    /// `scan_cache`; `Default` (empty map) keeps every
    /// existing construction of this state working.
    meta_cache: RefCell<HashMap<PathBuf, (SystemTime, usize, usize, u64, usize)>>,
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
    /// The open project's file stamp (Wave 11, E9):
    /// `(path, mtime)` of the file the core's selector
    /// points at, as last seen by the panel builder
    /// ([`HomePanelState::sync_open_file`]). The scan
    /// cache is keyed on the projects *directory's*
    /// mtime, which cannot see a save that replaces an
    /// existing file inside the same mtime tick as the
    /// cached scan — and a save is exactly such a
    /// replacement (`save_named_project_to` writes
    /// atomically since Wave 11: temp sibling + rename),
    /// whether the press was the GUI's or a live MCP
    /// client's `save_named_project`. Without
    /// this stamp the panel kept serving the pre-save
    /// scan entry for that file: its row's thumbnail —
    /// keyed on the scan entry's mtime, which never
    /// moved — showed frame 0 as it stood *before* the
    /// save, indefinitely. Interior mutability for the
    /// same reason as the caches.
    open_file_stamp: RefCell<Option<(PathBuf, SystemTime)>>,
    /// The core-project identity the current recovery
    /// offer was captured for (Wave 11, E9):
    /// [`core_mark`] of the project that was current
    /// when [`toggle_open`] (or the first render of a
    /// boot offer) captured the verdict. The banner's
    /// offer is a statement about *that* session — "the
    /// snapshot holds unsaved work newer than your last
    /// save" — so when the core's project is replaced
    /// behind the panel's back (a live MCP `load_project`
    /// / `new_project` drain in `main.rs::poll` calls no
    /// panel code at all) the offer must stop rendering:
    /// its Recover press would install the snapshot over
    /// the MCP-loaded project, and the discard gate
    /// cannot save the user — the swap restarted the
    /// dirty baseline, so the session reads clean and
    /// the first press runs. `None` until the first
    /// render records it. Interior mutability for the
    /// same reason as the caches.
    offer_mark: RefCell<Option<CoreMark>>,
}

/// A cheap identity for the core's current project
/// (Wave 11, E9): the stored name, the shape counts and
/// the file the core's selector points at. Two renders
/// of the same project — however much posing, drawing
/// or FX editing happened between them — produce the
/// same mark; every project *swap* path (GUI open / New
/// / Recover, MCP `load_project` / `import_file` /
/// `new_project`) replaces the project wholesale and
/// moves at least one component in practice. This is a
/// staleness guard for the panel's session claims (the
/// recovery offer), not a content hash: a swap to a
/// project with the same name, the same counts and the
/// same selector file is indistinguishable from no swap
/// — and leaves the panel showing an equivalent session,
/// so nothing it renders is a lie about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreMark {
    name: String,
    figures: usize,
    frames: usize,
    strokes: usize,
    file: Option<std::ffi::OsString>,
}

/// The [`CoreMark`] of the project the core currently
/// holds: pure reads, no disk access beyond the
/// selector's file *name* (no stat — the name is read
/// off the path the core already carries).
pub fn core_mark(core: &EditorCore) -> CoreMark {
    CoreMark {
        name: core.project.name.clone(),
        figures: core.project.figures.len(),
        frames: core.project.frames.len(),
        strokes: core.project.strokes.len(),
        file: current_project_path(core)
            .and_then(|p| p.file_name())
            .map(|n| n.to_os_string()),
    }
}

/// Whether a recovery offer captured for `mark` still
/// describes the session the core holds now (Wave 11,
/// E9): an offer with no recorded mark (a boot offer on
/// its first render) matches — the caller records the
/// current mark in that case; a recorded mark matches
/// only itself.
fn offer_matches(mark: Option<&CoreMark>, current: &CoreMark) -> bool {
    match mark {
        None => true,
        Some(m) => m == current,
    }
}

/// The open-file stamp transition (Wave 11, E9), pure
/// so it is testable without the filesystem: the caches
/// are dropped only when the *same* file's mtime moved
/// — an in-place rewrite of the open project. A
/// different path is a swap, which changes no file and
/// invalidates nothing; a missing stamp on either side
/// (first sight, an unstat'able file) learns silently.
fn stamp_action(
    old: Option<&(PathBuf, SystemTime)>,
    new: Option<&(PathBuf, SystemTime)>,
) -> bool {
    match (old, new) {
        (Some((old_path, old_time)), Some((new_path, new_time))) => {
            old_path == new_path && old_time != new_time
        }
        _ => false,
    }
}

impl HomePanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The saved-project library, scanned at most once per
    /// change to the `projects` directory or to any file
    /// it lists (see the module docs). The returned
    /// borrow covers the raw scan —
    /// including the `.recovery.json` entry, which parses as
    /// a project; row building filters it out via
    /// [`visible_projects`].
    pub fn projects(&self) -> Ref<'_, Vec<ProjectInfo>> {
        let dir = projects_dir();
        let key = dir_mtime(&dir);
        {
            let cache = self.scan_cache.borrow();
            if let Some((cached_key, entries)) = cache.as_ref() {
                // The directory key alone cannot see an
                // in-place rewrite of a listed file (see
                // `scan_entries_stale`): a hit is served
                // only while every listed library file
                // still stands at the mtime the scan
                // recorded for it.
                if *cached_key == key && !scan_entries_stale(entries, file_mtime) {
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
    /// is always one moment's read, never a mix. The
    /// missing-audio count (Wave S) rides it too, from
    /// the same parse as the counts.
    pub fn metadata_for(
        &self,
        path: &Path,
    ) -> Option<(usize, usize, u64, SystemTime, usize)> {
        let stat = std::fs::metadata(path).ok()?;
        let modified = stat.modified().ok()?;
        let size_bytes = stat.len();
        {
            let cache = self.meta_cache.borrow();
            if let Some((figures, frames, size, missing_audio)) =
                cached_meta(&cache, path, modified)
            {
                return Some((figures, frames, size, modified, missing_audio));
            }
        }
        let (figures, frames, missing_audio) = read_metadata_counts(path)?;
        self.meta_cache.borrow_mut().insert(
            path.to_path_buf(),
            (modified, figures, frames, size_bytes, missing_audio),
        );
        Some((figures, frames, size_bytes, modified, missing_audio))
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

    /// Watch the open project's file for an in-place
    /// rewrite (Wave 11, E9): one `stat` of the file the
    /// core's selector points at, per render — the same
    /// cost class as the directory stat the scan cache
    /// already pays. When the same file's mtime has
    /// moved since the last render, the file was
    /// rewritten (a save: the GUI's Save press and a
    /// live MCP `save_named_project` both replace the
    /// file — atomically, temp + rename, since Wave 11 —
    /// and a replacement inside the same mtime tick
    /// leaves the scan cache's directory key reading
    /// unchanged, so the cache cannot see it), and all
    /// three caches are dropped: the next
    /// [`HomePanelState::projects`] call rescans, and the
    /// row for the saved project re-reads its counts and
    /// re-renders its thumbnail at the new mtime instead
    /// of serving the pre-save frame 0 indefinitely.
    /// Called from the panel builder before the scan is
    /// read, open or closed — dropping the caches while
    /// closed costs nothing (the refill waits for the
    /// next open). Saves that create or replace a
    /// directory entry (MCP `save_project`'s atomic
    /// temp+rename, a duplicate, a delete) move the
    /// directory mtime and never needed this watch.
    pub fn sync_open_file(&self, current: Option<&Path>) {
        let stamp = current.and_then(|path| {
            std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|mtime| (path.to_path_buf(), mtime))
        });
        let invalidate = stamp_action(self.open_file_stamp.borrow().as_ref(), stamp.as_ref());
        if invalidate {
            self.invalidate_caches();
        }
        *self.open_file_stamp.borrow_mut() = stamp;
    }

    /// Whether the recovery banner may render for the
    /// project the core holds *now* (Wave 11, E9): the
    /// stored verdict ([`HomePanelState::recovery_offer`])
    /// and the identity it was captured for. The first
    /// render of an offer records the current
    /// [`CoreMark`] (a boot offer's mark is unrecorded —
    /// [`initial_state`] has no core to read); once a
    /// mark is recorded, a core whose mark no longer
    /// matches — the project was swapped behind the
    /// panel, the live-MCP case — suppresses the banner
    /// instead of offering to recover a snapshot over a
    /// project it was never captured about. The stored
    /// verdict itself is left untouched: closing and
    /// reopening Home re-captures it against the
    /// then-current project ([`toggle_open`]).
    pub fn recovery_offer_live(&self, core: &EditorCore) -> bool {
        if !self.recovery_offer {
            return false;
        }
        let current = core_mark(core);
        let mut recorded = self.offer_mark.borrow_mut();
        match recorded.as_ref() {
            None => {
                *recorded = Some(current);
                true
            }
            Some(mark) => offer_matches(Some(mark), &current),
        }
    }
}

/// The projects directory every backend agrees on:
/// `./projects` relative to the working directory
/// (shell.rs's [`shell::projects_dir`], editor.rs's named-save
/// directory, and the GPUI panel's scan root).
pub fn projects_dir() -> PathBuf {
    shell::projects_dir()
}

/// A directory's modification time, swallowing every error
/// into `None` (a missing/unreadable dir is a cacheable
/// state, not a failure — `projects::scan` agrees, yielding
/// an empty list for it).
fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// A file's modification time, swallowing every error
/// into `None` — [`dir_mtime`]'s rule applied to a
/// file: an unstat'able file is a state the scan
/// fingerprint ([`scan_entries_stale`]) compares like
/// any other, not a failure.
fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
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

/// Whether any *library* file in a cached scan has
/// changed since the scan read it (Wave 12, D1): each
/// entry's recorded mtime against the file's current
/// one — one `stat` per entry, the same cost class the
/// row loop already pays in
/// [`HomePanelState::metadata_for`]. An entry whose
/// file vanished (a stat that now fails where the scan
/// recorded one), appeared, or moved its mtime makes
/// the cached scan stale. This closes the scan cache's
/// blind spot: the cache is keyed on the projects
/// *directory's* mtime, which an in-place rewrite of an
/// existing file does not move, so a stdio MCP save
/// (that server's core holds its own current file) or
/// an external edit to any listed project other than
/// the GUI's open one left the rows serving the
/// pre-rewrite name/fps and the thumbnail cache
/// serving the pre-rewrite frame 0 indefinitely. (The
/// Wave 11 watch, [`HomePanelState::sync_open_file`],
/// covers the open project's file the same way, one
/// render earlier and wholesale.) Shell-owned and
/// recovery entries are excluded: they are never rows,
/// and the recovery snapshot is rewritten by the shell
/// itself (every Home open) — keying the scan to it
/// would rescan on the panel's own writes. The stamp
/// source is a parameter so the rule is testable
/// without the filesystem.
fn scan_entries_stale(
    entries: &[ProjectInfo],
    stamp_of: impl Fn(&Path) -> Option<SystemTime>,
) -> bool {
    entries
        .iter()
        .filter(|info| !is_recovery_entry(info) && !is_shell_owned_entry(info))
        .any(|info| stamp_of(&info.path) != info.modified)
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
///
/// Resolution is by exact stored name first. Since
/// Wave 10 there is a second pass, by file stem: the
/// recents front after a Home rename is the *typed*
/// name ([`confirm_rename`] re-touches with it), which
/// no row's stored name matches until the project is
/// saved again — the rename is file-only — so exact
/// matching alone made the reopen offer silently
/// vanish after every rename (and evicted whatever
/// valid offer the recents front held before it). The
/// typed name slugs to exactly the renamed file's stem
/// ([`name_file_slug`] — the same derivation the shell's
/// recovery check stats through), so a stem match is
/// that file, not a fuzzy guess. The exact match keeps
/// precedence, and the stem pass applies the same
/// shell-owned/recovery exclusion, so it can never land
/// on the snapshot twin either.
pub fn reopen_target<'a>(
    projects: &'a [ProjectInfo],
    last: Option<&str>,
    current_path: Option<&Path>,
) -> Option<&'a ProjectInfo> {
    let last = last?;
    if last.is_empty() {
        return None;
    }
    let usable = |info: &ProjectInfo| {
        !is_recovery_entry(info) && !is_shell_owned_entry(info)
    };
    let target = projects
        .iter()
        .find(|info| usable(info) && info.name == last)
        .or_else(|| {
            let stem = name_file_slug(last);
            if stem.is_empty() {
                None
            } else {
                projects.iter().find(|info| {
                    usable(info)
                        && info.path.file_stem().and_then(|s| s.to_str())
                            == Some(stem.as_str())
                })
            }
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
/// from the scan, never the raw file stem — and, since
/// Wave 10, bounded to [`REOPEN_LABEL_BUDGET`]: the
/// button sizes to its label, and a long stored name
/// would paint it past the card's right edge. The name
/// is middle-truncated for display only (the press
/// opens by path, and the full name is on the row
/// below); short names are returned byte-identical.
pub fn reopen_label(target: &ProjectInfo) -> String {
    let prefix = "Reopen ";
    let room = REOPEN_LABEL_BUDGET - text_width(prefix, label_char_width);
    format!(
        "{prefix}{}",
        truncate_middle(&display_name(&target.name), room, label_char_width)
    )
}

/// A stored name made safe for the panel's two
/// single-line, width-budgeted display strings — the
/// row name ([`row_display_name`]) and the reopen
/// label ([`reopen_label`]): control characters (a
/// newline above all) fold to spaces, so the painted
/// string is the string the budget model charged — a
/// name carrying one otherwise breaks the single line
/// the budget bought (Wave 12, D1). Display only, and
/// byte-identical for every name without control
/// characters: handlers, the search, the rename field
/// and the delete confirm all keep the stored name
/// itself.
fn display_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The reopen button label's width budget, points
/// (Wave 10): the button's fixed anatomy — kit `btn`
/// padding (10 + 10), the 13 pt glyph, the 6 pt
/// icon/label gap — is 39 pt, so a label charged at
/// most this keeps the whole button inside the left
/// column's 276 pt content width with margin.
pub const REOPEN_LABEL_BUDGET: f32 = 228.0;

/// Conservative advance estimate for one character at
/// the kit's LABEL font (12 pt, weight 500 —
/// Inter-Medium, the face Rune bundles and the shell's
/// LABEL style renders). Every class is an UPPER bound
/// against that font's real advances, measured from the
/// bundled TTF (upm 2048, advance × 12 / 2048); the
/// widest member of each class is named in its arm
/// below. The charges are fx_browser's
/// `preset_char_width` model (Wave 10 recalibration),
/// duplicated because that copy is private to its
/// module — the shell's slug rule is duplicated the
/// same way (shell.rs). Keep the two in sync: a label
/// this model passes truly fits; it may truncate a
/// borderline name early, never late. Non-ASCII
/// characters are charged the full em (12 pt): Inter's
/// own non-ASCII letters sit under their Latin classes,
/// "…" paints 10.92, and CJK/emoji ride the fallback
/// font at ~1 em.
fn label_char_width(c: char) -> f32 {
    match c {
        // Real space advance: 3.20.
        ' ' => 4.0,
        // Widest member: 'f' at 4.55.
        'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' => 5.0,
        // Widest members: '-' and '_' at 5.55.
        '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' => 6.0,
        // 'm' at 10.66 ('w' is 9.95).
        'm' | 'w' => 11.0,
        // 'W' at 12.04 ('M' is 10.95).
        'M' | 'W' => 12.5,
        // Widest digit: '4' at 7.88.
        '0'..='9' => 8.0,
        // Widest remaining capital: 'Q' at 9.22.
        'A'..='Z' => 9.5,
        // Widest remaining lowercase: 'b'/'d'/'g'/'p'/'q'
        // at 7.42.
        'a'..='z' => 7.5,
        // Widest remaining ASCII: '%' at 11.92 ('@' is
        // 11.79) — a full em, like any wide glyph.
        _ if c.is_ascii() => 12.0,
        _ => 12.0,
    }
}

/// One character's charge at the row name's TITLE font
/// (13 pt, weight 600): the LABEL charge scaled by the
/// size ratio and rounded UP to the next half point, so
/// it stays an upper bound at the larger size.
fn title_char_width(c: char) -> f32 {
    (label_char_width(c) * 13.0 / 12.0 * 2.0).ceil() / 2.0
}

/// The estimated painted width of `s` under a
/// char-width model ([`label_char_width`] /
/// [`title_char_width`]).
fn text_width(s: &str, char_width: fn(char) -> f32) -> f32 {
    s.chars().map(char_width).sum()
}

/// Middle-truncate `s` with "…" until its estimated
/// width fits `budget` under the given char-width
/// model: head and tail both kept, growing alternately
/// so neither side starves (fx_browser's
/// `preset_button_label` algorithm, generalised over
/// the model). A string that already fits is returned
/// unchanged. The truncating branch is only reached
/// when the whole string exceeds the budget, which is
/// more than the ellipsis leaves room for, so head and
/// tail can never meet.
fn truncate_middle(s: &str, budget: f32, char_width: fn(char) -> f32) -> String {
    if text_width(s, char_width) <= budget {
        return s.to_string();
    }
    // The ellipsis is charged by the same model (a
    // full em at LABEL, its scaled charge at TITLE).
    let room = budget - char_width('…');
    let chars: Vec<char> = s.chars().collect();
    let (mut head, mut tail, mut used) = (0usize, 0usize, 0.0f32);
    loop {
        let mut progressed = false;
        if head + tail < chars.len() {
            let w = char_width(chars[head]);
            if used + w <= room {
                head += 1;
                used += w;
                progressed = true;
            }
        }
        if head + tail < chars.len() {
            let w = char_width(chars[chars.len() - 1 - tail]);
            if used + w <= room {
                tail += 1;
                used += w;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

/// The file-name slug for a project name — lowercased,
/// every non-alphanumeric character replaced by '-':
/// the crate-wide naming convention
/// (`editor.rs::save_named_project`, `projects::slug`,
/// `shell::project_file_slug`). Each owner keeps a
/// private copy (shell.rs's is explicit about it);
/// keep this one in sync with them.
fn name_file_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
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

/// A project's runtime as the row's duration segment
/// (UI mission Wave 9): `frames` frame-periods at `fps`,
/// i.e. frames ÷ fps seconds, computed from the counts
/// both metadata lines already hold — the scan's or the
/// light read's frame count and the scan's fps. Nothing
/// is read or parsed for it. Under a minute the label
/// is decimal seconds with a trailing `.0` dropped
/// (`"0.5s"`, `"10s"`); from a minute up it is `m:ss`
/// of the rounded total (`"1:05"`). Per-frame duration
/// multipliers are *not* folded in: neither data source
/// carries them (see the module docs), so this is the
/// one-frame-period-per-frame runtime.
///
/// `None` — the segment is omitted, never faked — when
/// the fps cannot divide honestly: zero, negative or
/// non-finite. A zero frame count at a usable fps is a
/// real `Some("0s")`: an empty project lasts no time.
pub fn duration_segment(frames: usize, fps: f32) -> Option<String> {
    if !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    let secs = frames as f32 / fps;
    if secs < 60.0 {
        if (secs - secs.round()).abs() < 0.001 {
            Some(format!("{}s", secs.round() as u64))
        } else {
            Some(format!("{secs:.1}s"))
        }
    } else {
        let total = secs.round() as u64;
        Some(format!("{}:{:02}", total / 60, total % 60))
    }
}

/// [`duration_segment`] as a ` · `-prefixed interpolation
/// for the metadata lines: the segment with its
/// separator when it exists, empty when it does not —
/// so a line built with it keeps its previous shape
/// byte-for-byte at an unusable fps.
fn duration_suffix(frames: usize, fps: f32) -> String {
    match duration_segment(frames, fps) {
        Some(segment) => format!(" · {segment}"),
        None => String::new(),
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
/// size_bytes, modified, missing_audio)` read *without*
/// deserialising the whole `Project`. The file is parsed
/// as a bare `serde_json::Value` and only the lengths of
/// the top-level `figures` and `frames` arrays are
/// counted (model.rs's `Project` serialises both under
/// exactly those keys). The size is the file's byte
/// length, taken from the same single `stat` that yields
/// the mtime (Phase M) — no second filesystem call. The
/// missing-audio count (Wave S, S7) comes from the same
/// parse: the top-level `audio` array deserialised into
/// clips and classified by the shared
/// `audio::missing_audio_clips` — see
/// [`read_metadata_counts`]. `None` — never a guess —
/// when the file is missing, unreadable, not JSON,
/// either counted key is absent or not an array, or the
/// `audio` array will not deserialise as clips: such a
/// file is not a project file, the same verdict
/// `projects::scan` reaches by failing its full parse.
pub fn project_metadata(
    path: &Path,
) -> Option<(usize, usize, u64, SystemTime, usize)> {
    let stat = std::fs::metadata(path).ok()?;
    let modified = stat.modified().ok()?;
    let (figures, frames, missing_audio) = read_metadata_counts(path)?;
    Some((figures, frames, stat.len(), modified, missing_audio))
}

/// The parse half of [`project_metadata`] (the counts
/// and the missing-audio count; the caller supplies the
/// stat): split out so the cached accessor can stat
/// once and parse once. The audio half deserialises
/// only the `audio` array — never the whole project —
/// into `model::AudioClip`s and hands them to the one
/// shared classification (`audio::missing_audio_clips`:
/// unmuted clips whose file does not exist), the same
/// helper the open-time surfacing, the MP4 export path
/// and the Audio panel use, so the count here can never
/// apply a different rule than the verdict shown when
/// the project is opened. An absent `audio` key is an
/// empty clip list (`Project::audio` is
/// serde-defaulted), not a failure; a present array
/// that will not deserialise fails the whole read, the
/// same verdict the scan's full parse reaches on it.
fn read_metadata_counts(path: &Path) -> Option<(usize, usize, usize)> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let figures = value.get("figures")?.as_array()?.len();
    let frames = value.get("frames")?.as_array()?.len();
    let missing_audio = match value.get("audio") {
        None => 0,
        Some(raw) => {
            let clips: Vec<stykanimate_rs::model::AudioClip> =
                serde_json::from_value(raw.clone()).ok()?;
            stykanimate_rs::audio::missing_audio_clips(&clips).len()
        }
    };
    Some((figures, frames, missing_audio))
}

/// The metadata cache's lookup rule, pure so it is
/// testable without the filesystem: the cached counts,
/// size and missing-audio count for `path` are served
/// only when the mtime they were read at equals the
/// file's current mtime — a moved mtime is a miss even
/// for the same path, and a hit is served without any
/// re-read.
fn cached_meta(
    cache: &HashMap<PathBuf, (SystemTime, usize, usize, u64, usize)>,
    path: &Path,
    modified: SystemTime,
) -> Option<(usize, usize, u64, usize)> {
    match cache.get(path) {
        Some(&(read_at, figures, frames, size, missing_audio)) if read_at == modified => {
            Some((figures, frames, size, missing_audio))
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

/// The largest thumbnail render side, pixels: the
/// exporter renders at stage size ×2 supersample, and
/// the editor's own stage setter clamps each stage
/// axis to 4096 pt (editor.rs `set_stage_size`), so no
/// project the editor can produce renders past 8192 px
/// on a side. A project *file's* stage is raw JSON,
/// bound by no clamp — [`render_thumbnail`] refuses a
/// stage whose render would exceed this side rather
/// than attempt the allocation at scan time (see the
/// module docs, Wave 12).
pub const THUMB_MAX_RENDER_SIDE: u32 = 8192;

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
/// frames to render, the project's stage would render
/// past [`THUMB_MAX_RENDER_SIDE`] on a side (or to a
/// zero side), the render comes back empty, or the
/// image decode fails: a row without a thumbnail is
/// the honest outcome for a file whose first frame
/// cannot be drawn, never a placeholder.
pub fn render_thumbnail(path: &Path) -> Option<rune::Image> {
    let project = projects::load(path).ok()?;
    if project.frames.is_empty() {
        return None;
    }
    // The renderer's own dimensions (export::
    // render_frame_at: stage ×2 supersample, each axis
    // truncated to u32 — a negative or NaN axis lands
    // on 0), checked BEFORE the rasterise: a file whose
    // stage the editor itself could never set must not
    // take the scan down with an unservable allocation.
    // The multiply saturates: a huge axis casts to a
    // huge u32, and the plain `* 2` the exporter does
    // would overflow it (a debug-build panic) before
    // the comparison could refuse it.
    let (render_w, render_h) = (
        (project.stage[0] as u32).saturating_mul(2),
        (project.stage[1] as u32).saturating_mul(2),
    );
    if render_w == 0
        || render_h == 0
        || render_w > THUMB_MAX_RENDER_SIDE
        || render_h > THUMB_MAX_RENDER_SIDE
    {
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
/// comes from the scan; the duration segment (Wave 9,
/// [`duration_segment`]) is computed from that same fps
/// and whichever frame count the line is showing, so it
/// costs no read of its own. The light-read form also
/// closes with the missing-audio marker (Wave S, S7 —
/// [`missing_audio_segment`]) when the same read counted
/// clips whose files are missing; the fallback line
/// carries the same marker from the scan's own
/// parse-time count (Wave T, T6 — the scan classifies
/// the clips of the project it already fully parsed, so
/// [`meta_line`] closes with the marker whenever the
/// scan's count is known and non-zero, and with none
/// when the count is unknown). When the light read failed
/// (the file vanished or stopped parsing between the scan
/// and this render) the line is exactly the scan's
/// [`meta_line`] — no size segment (the scan never stats
/// for a length) and the row is unchanged rather than
/// blank.
pub fn row_meta_line(
    info: &ProjectInfo,
    meta: Option<(usize, usize, u64, SystemTime, usize)>,
) -> String {
    match meta {
        Some((figures, frames, size_bytes, modified, missing_audio)) => {
            let mut line = format!(
                "{} · {}{} · {} fps · {} · modified {}",
                count_label(figures, "figure", "figures"),
                count_label(frames, "frame", "frames"),
                duration_suffix(frames, info.fps),
                format_fps(info.fps),
                human_size(size_bytes),
                format_modified(Some(modified))
            );
            // Wave S (S7): the missing-audio marker
            // closes the line when the light read's
            // count is non-zero — the only segment the
            // line ever gains conditionally on file
            // *health* rather than file contents, and
            // absent entirely (never a "0 missing"
            // placeholder) when every clip's file is
            // present.
            if let Some(segment) = missing_audio_segment(missing_audio) {
                line.push_str(" · ");
                line.push_str(&segment);
            }
            line
        }
        None => meta_line(info),
    }
}

/// The missing-audio marker segment for a row's
/// metadata line (Wave S, S7): "1 audio clip missing" /
/// "N audio clips missing" for a non-zero count of
/// clips the shared classification
/// (`audio::missing_audio_clips`, via the light read)
/// found missing their files, `None` at zero — a
/// healthy project, a muted-only project and a
/// no-audio project all render the line the panel has
/// always rendered. Singular/plural wording is
/// [`count_label`]'s, the rows' own helper.
pub fn missing_audio_segment(missing: usize) -> Option<String> {
    if missing == 0 {
        None
    } else {
        Some(format!(
            "{} missing",
            count_label(missing, "audio clip", "audio clips")
        ))
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
/// (`ProjectInfo` figures/frames/fps/modified), plus
/// the duration (Wave 9) computed from the scan's own
/// frames and fps — still nothing but scan data. Since
/// Wave T (T6) the scan also carries a missing-audio
/// count from its full parse, so the line closes with
/// the same marker ([`missing_audio_segment`]) the
/// light-read line uses when that count is known and
/// non-zero. An entry whose count was never computed
/// (`ProjectInfo::missing_audio` is `None`) renders no
/// marker: unknown is not zero, and the line claims no
/// health the scan did not verify.
pub fn meta_line(info: &ProjectInfo) -> String {
    let mut line = format!(
        "{} · {}{} · {} fps · modified {}",
        count_label(info.figures, "figure", "figures"),
        count_label(info.frames, "frame", "frames"),
        duration_suffix(info.frames, info.fps),
        format_fps(info.fps),
        format_modified(info.modified)
    );
    if let Some(segment) = info.missing_audio.and_then(missing_audio_segment) {
        line.push_str(" · ");
        line.push_str(&segment);
    }
    line
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
        Ok(new_path) => {
            // Wave 12 (B3): the duplicate is a new file in
            // ./projects — refresh the core's cached selector
            // list so it names the copy too (the MCP drain
            // refresh in main.rs covers tool writes, not this
            // panel action).
            projects::refresh_project_files(core);
            format!(
                "Duplicated project: {}.",
                new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        }
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
/// family's "{Verb} failed: {e}" / past-tense shapes
/// (the failure shape carries " failed", the status
/// grammar's Error arm). On a real rename the session
/// clears, all three caches are invalidated (the
/// renamed file must re-scan under its new name), and
/// the core's project-file list is pointed at the new
/// file name — it otherwise keeps naming the
/// renamed-away path, so the rows' current marking
/// ([`is_current_project`], matched by file name) would
/// be lost and a later open through the core would hit
/// a file that no longer exists. On failure the rename
/// stays armed with the typed text intact, so the user
/// can fix the name and press again.
///
/// One success-shaped case is *not* a rename (Wave 10):
/// when the typed name slugs to the file the project
/// already has, `rename_project` returns that same
/// path and nothing on disk moved. The session clears
/// (the name in the field is the file's name; there is
/// nothing to fix and press again), the status says
/// exactly that — "Project file is named {file} —
/// nothing was renamed.", worded to stay in the
/// grammar's Info family (no "failed" / "cannot" /
/// "already") — and the function returns `false`, so
/// [`confirm_rename`] does not re-touch the recents for
/// a rename that did not happen. Returns whether a file
/// actually moved — the caller's cue for the one
/// follow-up this fn deliberately does not perform:
/// the recents re-touch lives in [`confirm_rename`],
/// because `shell::touch_recent` writes the shared
/// on-disk `./projects`, and this dispatch stays
/// unit-testable against a temp dir (the suite's
/// no-pollution rule — see the snap tests' note on
/// `./projects`).
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
            let file = new_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if new_path == path {
                // The backend no-op: the typed name
                // slugs to this file's own name. Report
                // it as the non-event it is (see the
                // docs above) — no cache drop, no
                // file-list rewrite, no recents re-touch.
                core.status =
                    format!("Project file is named {file} — nothing was renamed.");
                return false;
            }
            // Point the core's file list at the new
            // name (matched by file name, the panel's
            // established rule; each entry keeps its own
            // directory — the core's list is cwd-absolute,
            // the scan's paths relative).
            if let Some(old_file_name) = path.file_name() {
                for entry in core.project_files.iter_mut() {
                    if entry.file_name() == Some(old_file_name) {
                        *entry = entry.with_file_name(&file);
                    }
                }
            }
            panel.invalidate_caches();
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
/// and, only when a file actually moved, re-touch the
/// recents list with the new name — recents are
/// name-keyed, and the renamed file should head the
/// list the File menu and the reopen action read. The
/// typed name stays resolvable for both readers: the
/// shell's recovery check stats the file the name slugs
/// to, and [`reopen_target`] falls back to that same
/// file-stem match when no stored name matches (the
/// rename is file-only, so the stored name lags).
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
            core.note_project_swapped();
            reset_session(core);
            core.status = "Recovered the snapshot from ./projects/.recovery.json.".into();
        }
        None => {
            core.status = "No recovery snapshot found in ./projects/.".into();
        }
    }
}

/// Discard the recovery snapshot in `dir`: delete the
/// file (`shell::clear_recovery_in` — the delete command
/// existed in shell.rs since the store landed but had no
/// caller anywhere in the app; Wave P, P42 wires it
/// here) and report whether it is actually gone. The
/// verdict is read back off the disk rather than assumed:
/// `clear_recovery_in` swallows the remove result by
/// design (no error if already gone), so existence is
/// the only honest success signal. Already-gone counts
/// as discarded — the user's intent ("no snapshot") is
/// the state either way.
pub fn discard_recovery_in(dir: &Path) -> bool {
    shell::clear_recovery_in(dir);
    !dir.join(shell::RECOVERY_FILE).exists()
}

/// [`discard_recovery_in`] for the real `./projects`
/// directory (the `_in` convention of
/// [`startup_recovery`] / [`startup_recovery_in`]).
pub fn discard_recovery() -> bool {
    discard_recovery_in(&projects_dir())
}

/// New project: install the starter project
/// (`Project::new_demo`, model.rs:725) with the standard
/// session reset. The undo stack is left alone, as
/// `open_project_file` leaves it — undoing across the swap
/// restores the previous project, which is the behaviour
/// the GPUI shell had for opens.
pub fn dispatch_new(core: &mut EditorCore) {
    core.project = Project::new_demo();
    core.note_project_swapped();
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
    // Wave 10 (C6): the stroke selection indexes the old
    // project's strokes — clear it on every swap (see
    // project_tools' new_pressed for the full hazard).
    s.stroke_selected = None;
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
    s.stroke_selected = None;
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
    s.stroke_selected = None;
    s.home_panel.recovery_offer = false;
}

/// The recovery banner's Discard press (Wave P, P42):
/// delete the snapshot through [`discard_recovery`] and
/// clear the offer. The session itself is untouched —
/// discarding declines the *snapshot*, never the work on
/// screen — so no ConfirmAction arm gates it (arming
/// exists to protect the session; a new arm variant
/// would be an integrator action in main.rs). Like every
/// press in this panel it stands down an arm left by
/// another button. The offer is cleared only when the
/// file is verified gone; on failure the banner stays
/// and the status says so, rather than claiming a
/// discard the disk disagrees with.
fn discard_recovery_pressed(s: &mut State) {
    s.confirm_action = None;
    if discard_recovery() {
        s.home_panel.recovery_offer = false;
        *s.home_panel.offer_mark.borrow_mut() = None;
        s.core.status = "Recovery snapshot discarded.".into();
    } else {
        s.core.status = "Could not discard the recovery snapshot — it is still on disk.".into();
    }
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
        // Wave 11 (E9): the verdict just captured is a
        // statement about the project current *now* —
        // record its identity, so a later swap behind
        // the panel (live MCP) stops the banner instead
        // of letting it offer this session's snapshot
        // over a different project (see
        // `recovery_offer_live`).
        let mark = if s.home_panel.recovery_offer {
            Some(core_mark(&s.core))
        } else {
            None
        };
        *s.home_panel.offer_mark.borrow_mut() = mark;
        // Wave 13 (integrator, E2's flag): the note is
        // gated on the write actually landing, like the
        // save path — before, a failed snapshot write
        // still reset the autosave baseline.
        let written = shell::write_recovery(&s.core.project).is_ok();
        crate::rune_sec::project_tools::note_recovery_if_written(&mut s.core, written);
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
/// again. Discard (Wave P, P42) deletes the snapshot
/// file itself ([`discard_recovery_pressed`]) — before
/// it landed, the offer could only be cleared in memory
/// by recovering, and the file (still newer) re-offered
/// itself at the next startup.
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
                .grow(1.0)
                // Wave 10: the sentence measures ~365 pt
                // at LABEL charges; unwrapped it painted
                // across the Recover button and past the
                // card's edge (its grow share is ~176 pt).
                // Prose wraps (the kit's treatment).
                .wrap(),
        )
        .child(btn(
            "home-recovery-discard",
            "Discard",
            false,
            t,
            discard_recovery_pressed,
        ))
        .child(if armed {
            danger_btn("recover", "Discard changes?", t, recover_banner_pressed)
        } else {
            btn("recover", "Recover", false, t, recover_banner_pressed)
        })
}

/// The row name's display budget, points (Wave 10):
/// the name renders at TITLE in the row's text column,
/// whose narrowest form — beside a thumbnail, in the
/// left column at its default 300 pt — is 196 pt wide
/// (276 content − 16 row padding − 56 thumb − 8 gap).
/// Display names are middle-truncated to this budget
/// ([`row_display_name`]) so the budget holds with or
/// without a thumbnail; the full name stays in the
/// rename field, the search and every handler.
pub const ROW_NAME_BUDGET: f32 = 188.0;

/// A row's display name: the stored name itself while
/// it fits [`ROW_NAME_BUDGET`] at the TITLE charges,
/// middle-truncated with "…" past it (display only —
/// callers keep using `info.name` for everything real).
/// Control characters fold to spaces first
/// ([`display_name`], Wave 12): the budget charges a
/// single line, so the painted string must be one.
fn row_display_name(name: &str) -> String {
    truncate_middle(&display_name(name), ROW_NAME_BUDGET, title_char_width)
}

/// One project row: the scan-time thumbnail (Wave 4,
/// when this file version has one), then a column —
/// the name, the real metadata line, and the action
/// row (Open / Duplicate / Rename / Delete, wrapped
/// inside the column since Wave 10; see
/// [`project_row_marked`]). Keys carry the GPUI
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
    meta: Option<(usize, usize, u64, SystemTime, usize)>,
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
    meta: Option<(usize, usize, u64, SystemTime, usize)>,
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
    meta: Option<(usize, usize, u64, SystemTime, usize)>,
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
                    design::text_field(edit, type_::BODY.lh(26.0), edit.focused, t)
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
        // Wave 10: the question sits on its own line,
        // wrapped, with the buttons on a row beneath —
        // as a single line the question (TITLE, unwrapped,
        // growing) shared its row with both buttons
        // (88.5 + 52 pt + gaps), leaving ~104 pt of the
        // 260 pt inner width for "Delete {name}?", so any
        // name past ~13 characters painted across the
        // buttons. A destructive confirm is the one place
        // the full name must stay readable, so the name
        // is NOT truncated — the line wraps instead. Keys
        // and handlers are unchanged.
        return design::list_row(&format!("home-row-{i}"), true, t)
            .w_full()
            .child(
                col()
                    .gap(6.0)
                    .grow(1.0)
                    .child(
                        text(format!("Delete {name}?"))
                            .font(type_::TITLE)
                            .color(t.text)
                            .wrap()
                            .w_full(),
                    )
                    .child(
                        row()
                            .gap(8.0)
                            .child(danger_btn(
                                "home-proj-del-confirm",
                                "Delete file",
                                t,
                                move |s: &mut State| {
                                    dispatch_delete_confirm(
                                        &mut s.core,
                                        &mut s.home_panel,
                                        &confirm_name,
                                    );
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
                            )),
                    ),
            );
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
    // Wave 10 row anatomy: the four action buttons ride
    // in the text column, on a wrapping row under the
    // metadata line — as siblings of the column they
    // measured 286 pt of buttons alone (kit padding +
    // glyph + LABEL charges, [`label_char_width`])
    // against the row's 260 pt inner width, before any
    // name, meta or thumbnail: the trailing buttons
    // painted past the card's right edge and the name
    // column collapsed to nothing. In the column
    // (196 pt beside a thumbnail, 260 pt without) the
    // same charges pack Open + Duplicate and Rename +
    // Delete onto two lines — flex_wrap does the
    // packing, keys, order and handlers are unchanged.
    // The name is truncated to [`ROW_NAME_BUDGET`] and
    // the metadata line wraps (`.wrap().w_full()`, the
    // kit's prose treatment): unwrapped it measures
    // ~375–575 pt, wider than any share of the row.
    let buttons = row()
        .gap(8.0)
        .flex_wrap()
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
        }));
    row_el.child(
        col()
            .gap(2.0)
            .grow(1.0)
            .child(
                text(row_display_name(&info.name))
                    .font(type_::TITLE)
                    .color(t.text),
            )
            .child(
                text(row_meta_line(info, meta))
                    .key(format!("home-proj-meta-{i}"))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            )
            .child(buttons),
    )
}

/// The section as mounted in the shell column: a "Home"
/// toggle (accent-filled while open, the GPUI `home` id),
/// and — when open — the library card: the reopen-last
/// quick action (when offered), recovery banner, search
/// field, New project, and the project rows.
pub fn home_panel(s: &State, t: &Theme) -> El {
    let home = &s.home_panel;
    // Wave 11 (E9): watch the open project's file for an
    // in-place rewrite (a save the directory mtime
    // cannot see) before anything reads the caches —
    // every frame, open or closed, so a closed panel
    // never opens onto a pre-save scan either.
    home.sync_open_file(current_project_path(&s.core));
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
                // Wave 10: shortened from "Saved projects
                // in ./projects" — at SECTION charges the
                // full string measures ~183 pt against the
                // header cluster's ~150 pt (276 content −
                // 8 gap − the New button's ~119), so its
                // tail painted behind the New button. The
                // directory still names itself in the
                // recovery status lines and the panel docs.
                "Saved projects",
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
    if home.recovery_offer_live(&s.core) {
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
                design::text_field(&home.search, type_::BODY.lh(26.0), home.search.focused, t)
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
            if let Some((figures, frames, _, _, _)) = meta {
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
                .color(t.text_secondary)
                // Wave 10: three-digit counts push the
                // line past the card's 276 pt at LABEL
                // charges; the prose treatment wraps the
                // extreme case instead of spilling it.
                .wrap()
                .w_full(),
        );
    }
    panel.child(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a scan entry without touching the disk.
    /// Its missing-audio count is `None` — never
    /// computed — which is the "unknown" the fallback
    /// line must not present as health.
    fn info(name: &str, path: &str) -> ProjectInfo {
        ProjectInfo {
            name: name.into(),
            path: PathBuf::from(path),
            figures: 1,
            frames: 5,
            fps: 12.0,
            modified: None,
            missing_audio: None,
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

    // ---- Wave 11 (E9): MCP-direction state sync ----

    #[test]
    fn stamp_action_fires_only_on_an_in_place_rewrite() {
        let path = PathBuf::from("projects/demo.json");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200);
        let seen = (path.clone(), t0);
        // First sight and unstat'able files learn
        // silently — no invalidation.
        assert!(!stamp_action(None, Some(&seen)));
        assert!(!stamp_action(Some(&seen), None));
        assert!(!stamp_action(None, None));
        // The same file at the same mtime: no rewrite.
        assert!(!stamp_action(Some(&seen), Some(&seen)));
        // A swap to a different file changes no file.
        let other = (PathBuf::from("projects/other.json"), t1);
        assert!(!stamp_action(Some(&seen), Some(&other)));
        // The same file, a moved mtime: the in-place
        // rewrite (a save) the directory mtime cannot
        // see — drop the caches.
        assert!(stamp_action(Some(&seen), Some(&(path, t1))));
    }

    #[test]
    fn core_mark_tracks_the_project_not_its_edits() {
        let mut core = EditorCore::new();
        let before = core_mark(&core);
        // A pure pose edit moves no mark component:
        // the mark is a swap guard, not a content hash.
        core.project.frames[0].poses[0].root = [1.0, 2.0];
        assert_eq!(core_mark(&core), before);
        // A wholesale replacement — the shape every
        // MCP swap takes — moves it.
        dispatch_new(&mut core);
        core.project.name = "Agent Film".into();
        assert_ne!(core_mark(&core), before);
    }

    #[test]
    fn recovery_offer_stops_rendering_after_an_outside_swap() {
        let mut core = EditorCore::new();
        let mut panel = HomePanelState::default();
        // No offer: never live.
        assert!(!panel.recovery_offer_live(&core));
        panel.recovery_offer = true;
        // The first live render records the identity
        // the offer was captured for — the boot-offer
        // path (initial_state has no core to read).
        assert!(panel.recovery_offer_live(&core));
        // Renders of the same session stay live.
        assert!(panel.recovery_offer_live(&core));
        // The project is replaced behind the panel —
        // the live-MCP drain: no panel code runs, the
        // core simply holds a different project. The
        // offer was about the old session and must stop
        // rendering (its Recover press would install
        // the old snapshot over this project).
        dispatch_new(&mut core);
        core.project.name = "Loaded By MCP".into();
        assert!(!panel.recovery_offer_live(&core));
        // The stored verdict itself is untouched —
        // reopening Home re-captures it (toggle_open).
        assert!(panel.recovery_offer);
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
    fn reopen_target_is_none_when_no_file_matches() {
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Beta", "projects/beta.json"),
        ];
        // The recents front names a project whose file was
        // deleted outright: no stored name matches it and
        // no library file carries its slug — no button,
        // no error.
        assert!(reopen_target(&entries, Some("Gamma"), None).is_none());
        // A name of pure punctuation slugs to dashes,
        // which no file stem carries either.
        assert!(reopen_target(&entries, Some("!!!"), None).is_none());
    }

    #[test]
    fn reopen_target_resolves_a_renamed_project_by_file_stem() {
        // The post-rename library (Wave 10): the file
        // moved to my-film.json but its stored name is
        // still "Demo" (the rename is file-only), and
        // the recents front holds the typed name,
        // "My Film" — no stored name matches it, so the
        // exact pass misses and the file-stem pass lands
        // on the renamed file.
        let entries = vec![
            info("Alpha", "projects/alpha.json"),
            info("Demo", "projects/my-film.json"),
        ];
        let target = reopen_target(&entries, Some("My Film"), None).unwrap();
        assert_eq!(target.path, PathBuf::from("projects/my-film.json"));
        // The label names what will open — the stored
        // name the row shows — not the typed string.
        assert_eq!(reopen_label(target), "Reopen Demo");
        // A case variant of the typed name resolves the
        // same way: the slug is case-insensitive by
        // construction. (This test's predecessor pinned
        // a case variant as missing; that pin predates
        // the rename feature, whose typed names are
        // arbitrary-case by design, and it is exactly
        // what made the reopen offer vanish after a
        // rename.)
        let target = reopen_target(&entries, Some("my film"), None).unwrap();
        assert_eq!(target.path, PathBuf::from("projects/my-film.json"));
        // The currently-open suppression still applies
        // to a stem-resolved target (matched by file
        // name, as ever).
        assert!(reopen_target(
            &entries,
            Some("My Film"),
            Some(Path::new("/home/user/projects/my-film.json")),
        )
        .is_none());
    }

    #[test]
    fn reopen_target_stem_fallback_never_lands_on_the_snapshot() {
        // Only the recovery snapshot is in the scan:
        // neither pass may offer it — the exact pass
        // excludes it by rule and its stem (".recovery")
        // is no project's slug.
        let entries = vec![info("Untitled", "projects/.recovery.json")];
        assert!(reopen_target(&entries, Some("Untitled"), None).is_none());
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
    fn duration_segment_formats_frames_over_fps() {
        // Sub-minute runtimes are decimal seconds; a
        // whole second drops its trailing ".0".
        assert_eq!(duration_segment(5, 12.0).as_deref(), Some("0.4s"));
        assert_eq!(duration_segment(12, 24.0).as_deref(), Some("0.5s"));
        assert_eq!(duration_segment(1, 3.0).as_deref(), Some("0.3s"));
        assert_eq!(duration_segment(30, 12.0).as_deref(), Some("2.5s"));
        assert_eq!(duration_segment(120, 12.0).as_deref(), Some("10s"));
        assert_eq!(duration_segment(719, 12.0).as_deref(), Some("59.9s"));
        // An empty project at a usable fps honestly
        // lasts no time.
        assert_eq!(duration_segment(0, 12.0).as_deref(), Some("0s"));
        // From a minute up the label is m:ss of the
        // rounded total.
        assert_eq!(duration_segment(720, 12.0).as_deref(), Some("1:00"));
        assert_eq!(duration_segment(780, 12.0).as_deref(), Some("1:05"));
        assert_eq!(duration_segment(43_200, 12.0).as_deref(), Some("60:00"));
    }

    #[test]
    fn duration_segment_omits_an_unusable_fps() {
        // Zero, negative and non-finite rates cannot
        // divide honestly: no segment, never a guess.
        assert_eq!(duration_segment(10, 0.0), None);
        assert_eq!(duration_segment(10, -12.0), None);
        assert_eq!(duration_segment(10, f32::NAN), None);
        assert_eq!(duration_segment(10, f32::INFINITY), None);
    }

    #[test]
    fn meta_lines_keep_their_old_shape_at_an_unusable_fps() {
        // At fps 0 the duration segment vanishes and
        // both lines are byte-identical to their
        // pre-Wave-9 shapes — the omission is total,
        // not a placeholder.
        let mut entry = info("Demo", "projects/demo.json");
        entry.fps = 0.0;
        assert_eq!(
            meta_line(&entry),
            "1 figure · 5 frames · 0 fps · modified unknown"
        );
        let stamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426);
        assert_eq!(
            row_meta_line(&entry, Some((3, 12, 1_500, stamp, 0))),
            "3 figures · 12 frames · 0 fps · 1.5 KB · modified 2026-11-08 05:57"
        );
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
            "3 figures · 12 frames · 0.5s · 24 fps · modified 2026-11-08 05:57"
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
        let (figures, frames, size_bytes, modified, missing_audio) =
            project_metadata(&path).unwrap();
        assert_eq!(figures, 3);
        assert_eq!(frames, 7);
        // The fixture carries no `audio` key at all:
        // an absent array is an empty clip list
        // (`Project::audio` is serde-defaulted), so
        // nothing is missing.
        assert_eq!(missing_audio, 0);
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
        let mut cache: HashMap<PathBuf, (SystemTime, usize, usize, u64, usize)> =
            HashMap::new();
        // Empty cache misses.
        assert_eq!(cached_meta(&cache, &path, t0), None);
        cache.insert(path.clone(), (t0, 4, 9, 1_500, 2));
        // Same path, the mtime it was read at: a hit —
        // counts, size and the missing-audio count
        // served together.
        assert_eq!(cached_meta(&cache, &path, t0), Some((4, 9, 1_500, 2)));
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
        let (figures, frames, size_bytes, read_at, missing_audio) =
            state.metadata_for(&path).unwrap();
        assert_eq!((figures, frames), (2, 5));
        assert_eq!(missing_audio, 0);
        assert_eq!(size_bytes, project_json(2, 5).len() as u64);

        // Rewrite the file with different counts (and a
        // different length) but pin its mtime back to the
        // cached read's stamp: the accessor must serve the
        // cached counts and size — proof it did not
        // re-read (a re-read would see 6, 1 and the new
        // length).
        std::fs::write(&path, project_json(6, 1)).unwrap();
        set_mtime(&path, read_at);
        let (figures, frames, size_bytes, modified, _) = state.metadata_for(&path).unwrap();
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
        let (figures, frames, size_bytes, modified, _) = state.metadata_for(&path).unwrap();
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
        // `human_size` (1_500 bytes → "1.5 KB"), and the
        // duration (12 frames at the scan's 24 fps →
        // "0.5s") sits between frames and fps.
        assert_eq!(
            row_meta_line(&entry, Some((3, 12, 1_500, stamp, 0))),
            "3 figures · 12 frames · 0.5s · 24 fps · 1.5 KB · modified 2026-11-08 05:57"
        );
        // No light read: the line is the scan's, unchanged
        // — the old shape exactly, with no size segment
        // (its duration comes from the scan's own numbers:
        // 9 frames at 24 fps → "0.4s").
        assert_eq!(row_meta_line(&entry, None), meta_line(&entry));
        assert_eq!(
            row_meta_line(&entry, None),
            "9 figures · 9 frames · 0.4s · 24 fps · modified unknown"
        );
    }

    #[test]
    fn row_meta_line_closes_with_the_missing_audio_marker() {
        // Wave S (S7): a non-zero missing count appends
        // the marker as the line's closing segment —
        // pluralised by `count_label` — and a zero count
        // leaves the line byte-identical to its pre-Wave-S
        // shape (pinned by the tests above; the segment
        // helper itself is pinned here).
        let mut entry = info("Demo", "projects/demo.json");
        entry.fps = 24.0;
        let stamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426);
        assert_eq!(
            row_meta_line(&entry, Some((3, 12, 1_500, stamp, 2))),
            "3 figures · 12 frames · 0.5s · 24 fps · 1.5 KB · modified 2026-11-08 05:57 · 2 audio clips missing"
        );
        assert!(
            row_meta_line(&entry, Some((3, 12, 1_500, stamp, 1)))
                .ends_with("· 1 audio clip missing"),
            "a single missing clip singularises"
        );
        assert_eq!(missing_audio_segment(0), None);
        assert_eq!(
            missing_audio_segment(1).as_deref(),
            Some("1 audio clip missing")
        );
        assert_eq!(
            missing_audio_segment(3).as_deref(),
            Some("3 audio clips missing")
        );
    }

    #[test]
    fn meta_line_closes_with_the_scan_missing_audio_marker() {
        // Wave T (T6): the scan fallback line carries
        // the same marker, sourced from the scan's own
        // parse-time count on the ProjectInfo — so a row
        // whose light read failed still reports the
        // missing audio the scan verified. The base
        // shape is the one `meta_line_carries_real_metadata`
        // pins; the marker is purely additive at its end.
        let mut entry = info("Demo", "projects/demo.json");
        entry.figures = 3;
        entry.frames = 12;
        entry.fps = 24.0;
        entry.modified =
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_794_117_426));
        const BASE: &str =
            "3 figures · 12 frames · 0.5s · 24 fps · modified 2026-11-08 05:57";

        entry.missing_audio = Some(2);
        assert_eq!(
            meta_line(&entry),
            format!("{} · 2 audio clips missing", BASE)
        );
        // The marker reaches the rendered row through
        // the fallback arm itself, not only via
        // `meta_line` directly.
        assert_eq!(
            row_meta_line(&entry, None),
            format!("{} · 2 audio clips missing", BASE)
        );

        entry.missing_audio = Some(1);
        assert_eq!(
            meta_line(&entry),
            format!("{} · 1 audio clip missing", BASE)
        );

        // A verified zero: the scan parsed the file and
        // found nothing missing — the line is the old
        // shape exactly, no "0 missing" placeholder.
        entry.missing_audio = Some(0);
        assert_eq!(meta_line(&entry), BASE);

        // Unknown is not zero: an entry no scan ever
        // parsed carries `None`, and the line claims no
        // health it did not verify — byte-identical to
        // the verified-zero line, by the absence of the
        // marker rather than by a count.
        entry.missing_audio = None;
        assert_eq!(meta_line(&entry), BASE);
    }

    #[test]
    fn project_metadata_counts_missing_audio_with_the_shared_classification() {
        // Wave S (S7): the light read classifies a listed
        // project's clips with the shared helper's rule —
        // an unmuted clip whose file does not exist counts;
        // a present file does not; a MUTED clip whose file
        // is gone does not (muted clips are excluded from
        // playback and export, so the open-time surfacing
        // excludes them too, and the marker must agree).
        // Clip paths are absolute (the temp dir), so the
        // existence checks do not depend on the process
        // working directory.
        let dir = temp_dir("metadata-audio");
        let present = dir.join("present.wav");
        std::fs::write(&present, b"RIFF").unwrap();
        let gone = dir.join("gone.wav");
        let gone_muted = dir.join("gone-muted.wav");
        let doc = serde_json::json!({
            "name": "Audio",
            "fps": 24.0,
            "figures": [],
            "frames": [],
            "audio": [
                { "path": present.to_string_lossy(), "start_frame": 0 },
                { "path": gone.to_string_lossy(), "start_frame": 0 },
                { "path": gone_muted.to_string_lossy(), "start_frame": 0, "muted": true },
            ],
        });
        let path = dir.join("audio.json");
        std::fs::write(&path, doc.to_string()).unwrap();
        let (_, _, _, _, missing_audio) = project_metadata(&path).unwrap();
        assert_eq!(
            missing_audio, 1,
            "exactly the one unmuted missing clip counts"
        );
        // The same count reaches the row through the
        // cached accessor the row loop actually calls.
        let state = HomePanelState::default();
        let (_, _, _, _, missing_audio) = state.metadata_for(&path).unwrap();
        assert_eq!(missing_audio, 1);

        // Every clip's file present: no marker.
        let healthy = serde_json::json!({
            "name": "Healthy",
            "fps": 24.0,
            "figures": [],
            "frames": [],
            "audio": [
                { "path": present.to_string_lossy(), "start_frame": 0 },
            ],
        });
        let healthy_path = dir.join("healthy.json");
        std::fs::write(&healthy_path, healthy.to_string()).unwrap();
        let (_, _, _, _, missing_audio) = project_metadata(&healthy_path).unwrap();
        assert_eq!(missing_audio, 0);

        // An `audio` array that will not deserialise as
        // clips fails the whole light read — the row
        // falls back to the scan line, the documented
        // scope (the scan's full parse rejects such a
        // file too, so it can only be a listed row if
        // the file changed after the scan).
        let broken = serde_json::json!({
            "name": "Broken",
            "fps": 24.0,
            "figures": [],
            "frames": [],
            "audio": [ { "path": 3, "start_frame": 0 } ],
        });
        let broken_path = dir.join("broken.json");
        std::fs::write(&broken_path, broken.to_string()).unwrap();
        assert!(project_metadata(&broken_path).is_none());
        let _ = std::fs::remove_dir_all(&dir);
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
        let _ = project_row(&entry, 0, &t, Some((3, 12, 1_500, stamp, 0)), None);
        // And with a missing-audio count: the marker
        // rides the same keyed meta line, no new element.
        let _ = project_row(&entry, 0, &t, Some((3, 12, 1_500, stamp, 2)), None);
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
    fn dispatch_rename_to_its_own_file_name_is_an_honest_no_op() {
        // Wave 10: the typed name slugs to the file the
        // project already has — rename_project returns
        // the same path and nothing on disk moves. The
        // panel must say so (in the status grammar's
        // Info family: the string carries no "failed",
        // "cannot" or "already"), clear the session
        // (there is nothing to fix and press again),
        // keep its caches and the core's list, and
        // report not-landed, so `confirm_rename` does
        // not re-touch the recents for a rename that did
        // not happen.
        let dir = temp_dir("rename-noop");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        *panel.scan_cache.borrow_mut() =
            Some((None, vec![info("Demo", "projects/demo.json")]));
        let mut rename = HomeRenameState::new();
        begin_rename(&mut rename, path.clone(), "Demo");
        // Typed whitespace around the name trims to the
        // same slug — still the no-op.
        rename.edit.replace_text("  Demo  ".to_string());
        let mut core = EditorCore::new();
        core.project_files = vec![path.clone()];
        core.project_index = 0;
        assert!(!dispatch_rename(&mut core, &mut panel, &mut rename));
        assert_eq!(rename.armed, None, "a no-op still clears the session");
        assert_eq!(
            core.status,
            "Project file is named demo.json — nothing was renamed."
        );
        assert!(path.exists(), "the file is untouched");
        assert!(
            panel.scan_cache.borrow().is_some(),
            "no rename, no cache drop"
        );
        assert_eq!(core.project_files, vec![path.clone()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_rename_points_the_core_file_list_at_the_new_name() {
        // Wave 10: renaming the file of a listed (here:
        // the selected) project rewrites the core's list
        // by file name, so the selector, the rows'
        // current marking and a later open through the
        // core follow the renamed file instead of a path
        // that no longer exists. Unrelated entries keep
        // their paths; each rewritten entry keeps its
        // own directory (the core's list is cwd-absolute,
        // the scan's paths relative — only the file name
        // is replaced).
        let dir = temp_dir("rename-list");
        let path = dir.join("demo.json");
        std::fs::write(&path, project_json(1, 1)).unwrap();
        let mut panel = HomePanelState::default();
        let mut rename = HomeRenameState::new();
        begin_rename(&mut rename, path.clone(), "Demo");
        rename.edit.replace_text("My Film".to_string());
        let mut core = EditorCore::new();
        core.project_files = vec![
            PathBuf::from("/abs/projects/demo.json"),
            PathBuf::from("/abs/projects/other.json"),
        ];
        core.project_index = 0;
        assert!(dispatch_rename(&mut core, &mut panel, &mut rename));
        assert_eq!(
            core.project_files[0],
            PathBuf::from("/abs/projects/my-film.json")
        );
        assert_eq!(
            core.project_files[1],
            PathBuf::from("/abs/projects/other.json")
        );
        assert_eq!(
            current_project_path(&core),
            Some(Path::new("/abs/projects/my-film.json"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The four row buttons' widths under the charge
    /// model, from the kit anatomies: `icon_btn` (Open)
    /// is padding 20 + the 13 pt glyph + the 6 pt
    /// icon/label gap + its label; `btn` is padding 20 +
    /// its label.
    fn row_button_widths() -> [f32; 4] {
        [
            20.0 + 13.0 + 6.0 + text_width("Open", label_char_width),
            20.0 + text_width("Duplicate", label_char_width),
            20.0 + text_width("Rename", label_char_width),
            20.0 + text_width("Delete", label_char_width),
        ]
    }

    /// Greedy flex_wrap packing: how many lines `widths`
    /// take in a column `avail` wide with 8 pt gaps
    /// (the row's gap) — the layout the row builder
    /// delegates to Rune, modelled so the budget claim
    /// is executable.
    fn wrapped_lines(widths: &[f32], avail: f32) -> usize {
        let mut lines = 1;
        let mut used = 0.0f32;
        for &w in widths {
            if used > 0.0 && used + 8.0 + w > avail {
                lines += 1;
                used = w;
            } else if used == 0.0 {
                used = w;
            } else {
                used += 8.0 + w;
            }
        }
        lines
    }

    #[test]
    fn row_buttons_pack_into_two_lines_in_the_text_column() {
        // The Wave 10 row budget, executable. In the
        // text column — 196 pt beside a thumbnail,
        // 260 pt without — the four buttons wrap onto
        // exactly two lines under the charge model, and
        // every single button fits the narrower column
        // alone, so a wrap can never strand one past
        // the edge.
        let widths = row_button_widths();
        for &w in &widths {
            assert!(w <= 196.0, "button width {w} fits the narrow column");
        }
        assert_eq!(wrapped_lines(&widths, 196.0), 2);
        assert_eq!(wrapped_lines(&widths, 260.0), 2);
        // And the pre-Wave-10 single-line anatomy really
        // did overflow: buttons + inter-button gaps +
        // the list row's own padding exceed the panel's
        // 276 pt content width before any name, metadata
        // or thumbnail is allowed a single point.
        let single_line: f32 = widths.iter().sum::<f32>() + 8.0 * 3.0 + 16.0;
        assert!(
            single_line > 276.0,
            "the old anatomy measured {single_line}"
        );
    }

    #[test]
    fn row_display_name_truncates_to_the_title_budget() {
        // Short names pass through byte-identical.
        assert_eq!(row_display_name("Demo"), "Demo");
        // A long name is middle-truncated: the estimate
        // fits the budget, the ellipsis marks the cut,
        // and both ends of the name survive.
        let shown = row_display_name("The Extremely Long Director's Cut Final Version Two");
        assert!(text_width(&shown, title_char_width) <= ROW_NAME_BUDGET);
        assert!(shown.contains('…'), "{shown}");
        assert!(shown.starts_with("The "), "{shown}");
        assert!(shown.ends_with("Two"), "{shown}");
    }

    #[test]
    fn reopen_label_truncates_long_names_to_its_budget() {
        // Short names keep the exact historical label
        // (also pinned by the reopen resolution tests).
        assert_eq!(reopen_label(&info("Beta", "projects/beta.json")), "Reopen Beta");
        // A long stored name is middle-truncated so the
        // button fits the card; the prefix is never cut.
        let label = reopen_label(&info(
            "A Project Name That Runs Far Beyond The Card Edge",
            "projects/long.json",
        ));
        assert!(text_width(&label, label_char_width) <= REOPEN_LABEL_BUDGET);
        assert!(label.starts_with("Reopen A"), "{label}");
        assert!(label.contains('…'), "{label}");
    }

    #[test]
    fn name_file_slug_follows_the_crate_wide_rule() {
        // The derivation both rename resolution and the
        // shell's recovery check rely on: lowercased,
        // every non-alphanumeric character a dash —
        // including a trailing one (no trimming).
        assert_eq!(name_file_slug("My Film"), "my-film");
        assert_eq!(name_file_slug("My Copy!"), "my-copy-");
        assert_eq!(name_file_slug("Café"), "café");
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
    fn discard_recovery_deletes_the_snapshot_and_ends_the_offer() {
        // Wave P (P42): the Discard wiring's testable core.
        // A live offer, discarded: the file is gone, the
        // startup verdict flips to None, and a second
        // discard is still a success (already gone is the
        // state the user asked for).
        let dir = temp_dir("discard-recovery");
        shell::write_recovery_in(&dir, &Project::new_demo()).unwrap();
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::Offer);
        assert!(discard_recovery_in(&dir), "the delete reports success");
        assert!(!dir.join(shell::RECOVERY_FILE).exists(), "snapshot file gone");
        assert!(shell::read_recovery_in(&dir).is_none());
        assert!(!shell::recovery_is_newer_than_last_project_in(&dir));
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::None);
        assert!(discard_recovery_in(&dir), "discarding nothing still succeeds");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clean_save_refresh_no_longer_offers_a_content_identical_snapshot() {
        // Wave P (P42) found the spurious offer and pinned
        // it as the then-current truth: the GUI save path
        // (`save_named_pressed`, project_tools.rs) writes
        // the named save and THEN refreshes the recovery
        // snapshot with the same project, so the snapshot
        // is strictly newer than the save while holding
        // byte-identical content — and the mtime-only
        // offer condition offered it at the next startup.
        // Wave R (R22) lands the content-comparison gate
        // the user approved ("Build both", 2026-11-10):
        // the same fixture now yields NO offer, because
        // `recovery_is_newer_than_last_project` compares
        // content once the mtime verdict fires (see
        // shell.rs `project_documents_match`). This pin
        // replaces P42's — the fixture is unchanged, only
        // the expected verdict flipped.
        let dir = temp_dir("spurious-offer");
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let mut project = Project::new_demo();
        project.name = "Spur Check".into();
        // The named save, holding exactly the bytes the
        // snapshot will hold.
        let snapshot = shell::write_recovery_in(&dir, &project).unwrap();
        let bytes = std::fs::read(&snapshot).unwrap();
        let saved = dir.join("spur-check.json");
        std::fs::write(&saved, &bytes).unwrap();
        shell::touch_recent_in(&dir, "Spur Check");
        // The save path's write order, as mtimes: save
        // first, snapshot refresh second.
        set_mtime(&saved, t0);
        set_mtime(&snapshot, t1);
        assert_eq!(
            std::fs::read(&saved).unwrap(),
            std::fs::read(&snapshot).unwrap(),
            "the two files are byte-identical"
        );
        assert!(
            !shell::recovery_is_newer_than_last_project_in(&dir),
            "identical content is the clean-save refresh, not newer work"
        );
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::None);
        // Discard is unaffected by the gate: it still
        // retires the snapshot file itself.
        assert!(discard_recovery_in(&dir));
        assert!(!snapshot.exists(), "discard removed the snapshot file");
        assert_eq!(startup_recovery_in(&dir), StartupRecovery::None);
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
            missing_audio: None,
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

    // ---- Wave 12 (D1): the populated-Home audit ----

    /// A scan entry carrying a recorded mtime (the
    /// `info` helper leaves it `None`).
    fn info_at(name: &str, path: &str, modified: Option<SystemTime>) -> ProjectInfo {
        let mut entry = info(name, path);
        entry.modified = modified;
        entry
    }

    #[test]
    fn scan_entries_stale_follows_each_listed_files_mtime() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let entries = vec![
            info_at("Alpha", "projects/alpha.json", Some(t0)),
            info_at("Beta", "projects/beta.json", Some(t0)),
        ];
        // Every file still at its recorded mtime: the
        // cached scan stands.
        assert!(!scan_entries_stale(&entries, |_| Some(t0)));
        // An in-place rewrite of ANY listed file moves
        // its mtime and stales the scan — the directory
        // mtime never moved, which is the whole point.
        assert!(scan_entries_stale(&entries, |p| {
            if p.ends_with("beta.json") {
                Some(t1)
            } else {
                Some(t0)
            }
        }));
        // A listed file that vanished (its stat now
        // fails) stales the scan too: the row must not
        // outlive the file.
        assert!(scan_entries_stale(&entries, |p| {
            if p.ends_with("alpha.json") {
                None
            } else {
                Some(t0)
            }
        }));
        // And the mirror image: an entry the scan could
        // not stat (recorded None) whose file now stats
        // has changed state — rescan.
        let nostat = vec![info_at("Alpha", "projects/alpha.json", None)];
        assert!(!scan_entries_stale(&nostat, |_| None));
        assert!(scan_entries_stale(&nostat, |_| Some(t0)));
        // An empty scan is never stale.
        assert!(!scan_entries_stale(&[], |_| None));
    }

    #[test]
    fn scan_entries_stale_ignores_shell_owned_and_recovery_entries() {
        // The shell's own files churn under the panel —
        // the recovery snapshot is rewritten on every
        // Home open — and none of them is a row: their
        // mtimes must never stale the scan.
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let t1 = t0 + std::time::Duration::from_secs(60);
        let entries = vec![
            info_at("Alpha", "projects/alpha.json", Some(t0)),
            info_at("Untitled", "projects/.recovery.json", Some(t0)),
            info_at("Animate", "projects/.layout.json", Some(t0)),
            info_at("recents", "projects/recents.json", Some(t0)),
        ];
        assert!(!scan_entries_stale(&entries, |p| {
            if p.ends_with("alpha.json") {
                Some(t0)
            } else {
                Some(t1)
            }
        }));
    }

    #[test]
    fn render_thumbnail_refuses_a_stage_the_editor_cannot_set() {
        // A project file's stage is raw JSON, bound by
        // no clamp (the editor's setter caps each axis
        // at 4096): past the render bound the rasterise
        // is refused up front — no thumbnail, and no
        // attempt at the allocation. A zero or negative
        // axis renders to a zero side and is refused the
        // same way.
        let dir = temp_dir("thumb-stage");
        let mut project = Project::new_demo();
        project.stage = [100_000.0, 100_000.0];
        let huge = dir.join("huge.json");
        std::fs::write(&huge, serde_json::to_string(&project).unwrap()).unwrap();
        assert!(render_thumbnail(&huge).is_none());
        project.stage = [0.0, 720.0];
        let zero = dir.join("zero.json");
        std::fs::write(&zero, serde_json::to_string(&project).unwrap()).unwrap();
        assert!(render_thumbnail(&zero).is_none());
        project.stage = [-960.0, 720.0];
        let negative = dir.join("negative.json");
        std::fs::write(&negative, serde_json::to_string(&project).unwrap()).unwrap();
        assert!(render_thumbnail(&negative).is_none());
        // An axis so large its u32 cast saturates the
        // supersample multiply: refused, not overflowed.
        project.stage = [3.0e9, 720.0];
        let saturating = dir.join("saturating.json");
        std::fs::write(&saturating, serde_json::to_string(&project).unwrap()).unwrap();
        assert!(render_thumbnail(&saturating).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn display_name_folds_control_characters_only() {
        // Ordinary names — punctuation, accents, the
        // ellipsis included — pass through byte-identical.
        assert_eq!(display_name("Demo"), "Demo");
        assert_eq!(
            display_name("Café — Director's Cut…"),
            "Café — Director's Cut…"
        );
        // Control characters fold to spaces, one for one.
        assert_eq!(display_name("A\nB\tC\rD"), "A B C D");
        // Through the row's display name, a newline name
        // stays a single line inside the title budget.
        let shown = row_display_name("First Line\nSecond Line");
        assert!(!shown.contains('\n'), "{shown}");
        assert!(text_width(&shown, title_char_width) <= ROW_NAME_BUDGET);
        // And through the reopen label.
        let label = reopen_label(&info("A\nB", "projects/ab.json"));
        assert_eq!(label, "Reopen A B");
    }
}
