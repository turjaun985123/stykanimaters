//! Phase D, section 18/40 — FIGURE LIBRARY PANEL for the Rune
//! editor shell. Declared from `main.rs` only (bin-crate code,
//! not part of the engine library).
//!
//! The panel over the core's figure library
//! (`EditorCore::library`, editor.rs:64 —
//! `Vec<(String, FigureDef)>` loaded by `EditorCore::new()`
//! from `./library/*.json` plus three built-ins, so it is
//! never empty in practice; `load_library` at editor.rs:162):
//!
//! - A vertically scrolling list of every library figure:
//!   a live thumbnail of its def (Wave 12 — see
//!   [`library_thumb_shape`]), its name and its segment
//!   count (`FigureDef::nodes`, model.rs:164). Entries
//!   whose name carries the core's own
//!   `Built-in: ` prefix (the three `load_library` built-ins)
//!   are grouped under a **Built-in** heading; the rest —
//!   figures saved from the editor — under **Saved**.
//! - One **Insert** button per entry: points
//!   `EditorCore::library_index` (editor.rs:65) at the entry
//!   and calls `EditorCore::insert_library_figure`
//!   (editor.rs:2100), the same op the GPUI Library panel's
//!   Insert button used.
//! - A **Save current figure to library** button calling
//!   [`save_current_figure`] →
//!   `EditorCore::save_figure_to_library` (editor.rs:2115),
//!   which writes `./library/<figure-name>.json` and reloads
//!   the library, so the new entry appears in the list on the
//!   next frame.
//! - An empty-state line when the library is empty (only
//!   reachable if the vec is cleared — `load_library` seeds
//!   the three built-ins).
//!
//! ## Phase E additions (section 36/40)
//!
//! - **Search**: a Rune `TextEdit` in
//!   [`LibraryPanelState::search`] filters both groups by
//!   case-insensitive name substring every render, through
//!   the `*_rows_q` variants of the row helpers (the plain
//!   helpers remain as empty-query wrappers). Rows keep
//!   their real library slot indices while filtered, so
//!   Insert/Delete still target the right entry.
//! - **Delete** (saved entries only — the button is not
//!   rendered for built-ins, and [`delete_saved`] refuses
//!   them again defensively): removes the entry's backing
//!   file and then the `core.library` vec entry, in that
//!   order. If the file removal fails, the vec entry is
//!   KEPT and the error is reported on `core.status` —
//!   the panel never claims a delete the disk refused.
//!   After a successful removal, `core.library_index` is
//!   clamped back into range (see [`clamped_index`]).
//!
//! ## Phase F additions (section 33/40)
//!
//! - **Rename** (saved entries only — the button is not
//!   rendered for built-ins, and [`rename_saved`] refuses
//!   them again defensively): the per-row **Rename**
//!   button ([`start_rename`]) loads the entry's name
//!   into the [`LibraryPanelState::rename`] field and
//!   marks the row in [`LibraryPanelState::renaming`];
//!   an editor row (field + Apply + Cancel) renders
//!   under that row. Apply ([`apply_rename`] →
//!   [`rename_saved`]) delegates to the rename op in
//!   the engine library
//!   (`stykanimate_rs::library_ops` — [`rename_saved`]
//!   itself is now only a thin wrapper over it, kept
//!   under its long-standing name and signature for
//!   this panel and its tests). The lib op sanitises
//!   the typed name into a file stem ([`sanitize_stem`]
//!   — the save path's own slug), refuses empty stems
//!   and collisions (another entry's name, via
//!   [`name_taken`], or an existing file at the target
//!   path), renames the backing file FIRST with
//!   `std::fs::rename`, and only then updates the vec
//!   entry's name — the delete contract mirrored: any
//!   fs error keeps the entry under its old name and
//!   reports on `core.status`. The entry keeps its
//!   slot, so `library_index` needs no clamp. A refused
//!   or failed rename keeps the editor open so the name
//!   can be fixed; a settled one closes it. Deleting
//!   the row being renamed cancels the rename session
//!   first, so `renaming` can never point at a shifted
//!   entry.
//!
//! ## Phase H additions (section 31/40)
//!
//! - **Icons**: the search row leads with a
//!   `magnifying-glass` glyph ([`icon_el`]); the per-row
//!   Insert button renders through [`icon_btn`] with
//!   `plus` (its handler inserts — adds — the entry into
//!   the project), Rename through [`icon_btn`] with
//!   `pencil`, Delete as an icon-only [`icon_only_btn`]
//!   with `trash-can`, and the rename editor's Apply /
//!   Cancel through [`icon_btn`] with `check` / `xmark`.
//!   Keys, labels, handlers and the active flags are
//!   byte-identical to the text-button versions; the
//!   header Hide/Show toggle and the save button keep
//!   the plain [`btn`] rendering.
//!
//! ## Phase I additions (section 37/40)
//!
//! - **File-name matching**: the search matched entry
//!   names only. A saved entry also has a file name —
//!   `./library/<name>.json`, the stem mapping proven
//!   below — so the row filter ([`library_rows_q`]) now
//!   goes through [`entry_matches`], which accepts a hit
//!   on the display name OR on the backing file name
//!   ([`entry_file_name`]): a query like `hero.json` or
//!   `.json` finds saved entries it could not find
//!   before. Built-ins have no file and still match by
//!   name only.
//! ## Phase J additions (section 25/40)
//!
//! - **Enter-to-insert in the search field**: the field
//!   now wires `on_submit` ([`submit_library_search`]),
//!   which inserts the FIRST VISIBLE entry by its real
//!   library slot ([`enter_insert_index`] —
//!   [`library_rows_q`]'s first row, so the visible→real
//!   mapping is the rows' own). A filter with no matches
//!   makes Enter a no-op. Recorded honestly: before
//!   this phase the field had NO submit path at all (a
//!   Phase I summary's claim of a buggy one was never
//!   borne out by the code), so this is a new path
//!   built on the correct mapping, not a remapping of
//!   an existing one.
//!
//! - **Count discipline**: the header count followed no
//!   filter discipline — it always read `{total}
//!   figures`. It now renders through [`count_line`],
//!   the figures panel's standard (Phase G §17): while
//!   a filter is active ([`filter_active`]) it reads
//!   `{shown} of {total} figures`; idle it keeps the
//!   header's existing `{total} figures` form,
//!   byte-identical to before. The no-match line
//!   (`No library figures match "…".`) already mirrored
//!   the figures panel's and is unchanged.
//!
//! ## Phase L additions (section 20/40)
//!
//! - **Sort toggle**: the search row gains a `lib-sort`
//!   button (plain [`btn`] — the Home panel's Phase K
//!   sort precedent) flipping
//!   [`LibraryPanelState::sort_by_name`]. The label
//!   names the order in effect: "Sort: A–Z" while
//!   sorting, "Sort: added" otherwise — the library's
//!   own order, the order `load_library` added the
//!   entries: the three built-ins first (constructed in
//!   code), then saved entries in the order their files
//!   were read (file paths sorted, editor.rs:183-189).
//!   Sorting applies to the rows projection only
//!   ([`sort_rows`]: stable, case-insensitive on the
//!   display name), within each group — the Built-in /
//!   Saved grouping itself is unchanged — and rows keep
//!   their real library slots, so Insert, Rename and
//!   Delete target exactly the same entries as
//!   unsorted. Enter-to-insert follows the visible
//!   order too ([`enter_insert_index_sorted`]): with
//!   the sort on, Enter inserts the first row the user
//!   actually sees.
//!
//! ## Phase N adoption
//!
//! - **Card language**: the panel root is the kit's
//!   [`design::panel_col`] (card fill, hairline border,
//!   radius 12, padding 12) under the same `library-panel`
//!   key; the old inner card (`library-card`) keeps its
//!   key as a plain grouping column — the root is the
//!   card now, not a card inside a card. The header rides
//!   the kit's keyed [`design::header_row`] shell with a
//!   [`design::panel_title`] cluster (`book-open` glyph,
//!   per P13; the count line is the cluster's meta), and
//!   a kit [`design::divider`] sits under it while the
//!   panel is expanded.
//! - **Sort toggle**: `lib-sort` renders through
//!   [`icon_btn`] with `arrow-down-a-z` (P13: the label
//!   is literally "Sort: A–Z"); key, labels, active flag
//!   and handler are unchanged from the plain-[`btn`]
//!   version.
//! - **Rows**: entry rows ride the kit's
//!   [`design::list_row`] shell under the same
//!   `library-row-{i}` keys; the shell's selected fill
//!   marks the rename target — the only per-row state
//!   the panel has. Row children (name, segment count,
//!   Insert / Rename / Delete) keep their keys and
//!   handlers verbatim.
//! - **Labels & empty states**: group headings are the
//!   kit's [`design::section_label`]; the two bare
//!   "nothing here" lines are the kit's
//!   [`design::empty_state`] with their message strings
//!   byte-identical.
//!
//! ## Wave 10 (round-2 audit)
//!
//! - **Row widths**: entry rows are two-line now (name
//!   + segment count, then the buttons) with the name
//!   in Rune's truncating one-line mode — the
//!   single-line row's fixed children alone overran
//!   the card for every saved entry, and the longest
//!   built-in name overran its share too; see
//!   [`entry_row`]'s note for the measured arithmetic.
//!   Invisible to headless scans because CI checkouts
//!   have no `./library` files, so saved rows never
//!   rendered there.
//! - **Rename-session slot discipline**: the session
//!   is a raw library slot, and two of the panel's own
//!   actions can move slots — a successful delete of
//!   an earlier entry ([`delete_entry`] now shifts the
//!   session down with its entry; a failed delete
//!   shifts nothing) and the save button's library
//!   reload ([`save_current_figure`] captures the
//!   target by name and re-resolves it afterwards via
//!   [`reresolve_renaming`]). Before this wave either
//!   action could leave the editor open on a different
//!   entry than the one the user picked.
//! - **Search scope, recorded**: an entry IS a name
//!   plus a `FigureDef { nodes }` (model.rs) — there
//!   is no kind or tag field anywhere in the library
//!   model, so matching the display name and the
//!   backing file name ([`entry_matches`]) covers
//!   everything an entry carries. The factory audit
//!   also closed clean: `FigureDef` has exactly three
//!   named constructors (`humanoid`, `quadruped`,
//!   `simple`) and the three built-ins map onto them
//!   one-to-one, so no constructible figure type is
//!   missing from the library.
//!
//! ## Wave 12 (D5 — row thumbnails)
//!
//! - **Thumbnails**: until this wave a row was text
//!   only — name, segment count, buttons; no card drew
//!   its figure at all (the codebase's only figure
//!   miniature was the figures panel's, over PROJECT
//!   figures). Each row now leads with a
//!   [`THUMB_SIZE`] canvas (key `library-thumb-{i}`)
//!   drawing the entry's OWN def, derived fresh from
//!   `core.library[i]` on every render — there is no
//!   cached or stored image anywhere in the path, so
//!   a renamed, duplicated or re-saved entry's
//!   thumbnail can never go stale, and built-in and
//!   saved entries render by exactly one rule (the
//!   same [`library_thumb_prims`] over their defs).
//! - **Fidelity**: the thumbnail is the stage's own
//!   construction shrunk, not a glyph: the def is
//!   evaluated at its rest pose
//!   (`FigureDef::rest_pose` + `model::evaluate`),
//!   fitted by the figures panel's [`thumb_fit`], and
//!   every segment goes through
//!   `paint::styled_segment_pieces` — the shared
//!   styled-segment source the stage itself adopted
//!   this wave — so per-node colours, taper, cap
//!   geometry, `color2` gradients and style opacity
//!   all reach the miniature; shape nodes and head
//!   rings follow the stage's passes (ring/disc at
//!   the node's style opacity; head rings at the
//!   base colour, unscaled [`HEAD_RING_RADIUS`] — a
//!   def has no project to scale it by, which is the
//!   constant's documented default).
//! - **Named divergences from the stage** (accepted,
//!   by the nature of a library entry): the pose is
//!   the REST pose — an entry has no project pose to
//!   show; the bend is 0 — bends live in project
//!   frames, not in a def; segment media is NOT drawn
//!   — a def's asset indices belong to the project it
//!   was saved from and may not resolve in this one,
//!   so drawing them could show the wrong image; and
//!   stroke widths floor at [`THUMB_MIN_WIDTH`] so a
//!   hard taper stays visible at 44 points (the
//!   stage's own 0.8-point dot floor is the precedent;
//!   endpoints and every width above the floor are
//!   exact).
//! - **Base colour**: a def carries no figure colour
//!   — `insert_library_figure` assigns one from the
//!   core's palette by the project's figure count at
//!   insert time, so no colour is intrinsic to an
//!   entry. Segments that inherit (node colour
//!   `None`) are drawn in the theme's text colour;
//!   every colour the def itself carries (per-node
//!   overrides, gradient ends) is drawn as authored.
//!
//! [`thumb_fit`]: crate::rune_sec::figures_panel::thumb_fit
//! [`HEAD_RING_RADIUS`]: stykanimate_rs::model::HEAD_RING_RADIUS
//!
//! The successful-rename fs dispatch (a real file moved
//! under `./library`) is deliberately **untested**, for
//! exactly the delete reason given below: test binaries
//! share the process CWD. The sanitiser, the collision
//! check, the built-in refusal, the same-name settle and
//! the failure-keeps-entry path (via a synthetic entry
//! whose file does not exist) are all covered below.
//!
//! ## Entry name ↔ backing file mapping
//!
//! Proven from `load_library` (editor.rs:162): it reads
//! `./library/*.json` in sorted order and names each entry
//! from the file's **stem** (`f.file_stem()`), so a saved
//! entry's stored name IS its file stem and the backing
//! file is exactly `./library/<name>.json` — see
//! [`library_file_for`]. The write side agrees:
//! `save_figure_to_library` (editor.rs:2182) writes
//! `./library/<figure name, spaces→'-', lowercased>.json`
//! and reloads, so the reloaded entry's name is that stem.
//! Built-ins are not files at all (constructed in code),
//! which is why they can never be deleted here.
//!
//! The successful-delete fs dispatch (a real file removed
//! from `./library`) is deliberately **untested**: test
//! binaries share the process CWD, so writing/deleting
//! files under `./library` from a unit test is fragile
//! and could destroy a developer's real saved figures.
//! The pure mapping, the filter, the index clamp, the
//! built-in refusal and the failure-keeps-entry path
//! (via a synthetic entry whose file does not exist) are
//! all covered below.
//!
//! Section 2 (`figure_add`) also lists the library, as one
//! flat pick-list inside the add-figure popover stand-in;
//! this panel is the library's own home, with segment
//! counts, grouping and the save op the popover omits.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod library_panel;
//! }
//! // in `State`:
//! pub library_panel: library_panel::LibraryPanelState,
//! // in `init()`:
//! library_panel: library_panel::LibraryPanelState::default(),
//! // in `view()`, in the sidebar column:
//! .child(rune_sec::library_panel::library_panel(s, &t))
//! // in `text_focused()` (Phase F — the rename field joins
//! // the search field there, so global shortcuts stay out
//! // of the user's typing):
//! || s.library_panel.rename.focused
//! ```

use std::path::PathBuf;

use rune::tok::{type_, Theme};
use rune::{canvas, col, row, text, text_input, Color, El, LineCap, ScrollSpec, Shape, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, FigureDef};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::figures_panel;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
use crate::stage_view::{hex_color, ngon};
use crate::State;

/// The name prefix `load_library` gives its three built-in
/// entries (editor.rs:164-166). It is the only separation
/// the core keeps between built-ins and saved figures, so
/// the grouping below is derived from it.
pub const BUILTIN_PREFIX: &str = stykanimate_rs::library_ops::LIBRARY_BUILTIN_PREFIX;

/// Height of one library entry row, in points: the
/// [`THUMB_SIZE`] thumbnail line (44 — the name text
/// beside it is a 15 pt LABEL line, so the canvas sets
/// the line height), the row body's 4 pt gap, and the
/// button line at 26 (the icon-only Delete button's
/// 26 pt box; the labelled icon buttons are 25 — a
/// 15 pt label line inside 5 pt vertical padding). The
/// `list_row` shell adds no vertical padding (its
/// `row_base` pads horizontally only), so the shell is
/// exactly the body: 44 + 4 + 26 = 74.
pub const ROW_H: f32 = 74.0;

/// Vertical gap between two rows of one group (the
/// group column's gap in `group` below).
pub const ROW_GAP: f32 = 4.0;

/// Cap (points) on the list's scroll viewport: one
/// group's label line (a SECTION text line,
/// 11.5 × 1.25 = 14.375), its 4 pt gap, five rows at
/// [`ROW_H`] and the four [`ROW_GAP`]s between them —
/// 404.375 pt. Five visible rows is the shell's list
/// convention (the figures panel's `FIG_LIST_MAX_H`,
/// the graph channel list's `CHAN_LIST_MAX_H`); the
/// label rides inside this cap because a library list
/// is never rendered without one. The viewport is
/// `.min_h(0)` / `.max_h(LIST_MAX_H)` (the figures
/// panel's pattern), so at or under five rows it is
/// exactly the content height — no dead space under a
/// short library — and past five rows (or once a
/// second group's label joins the same budget) it
/// scrolls internally.
///
/// Wave 13 (D4) replaces the fixed 200 pt `LIST_HEIGHT`
/// this cap supersedes: Wave 12's thumbnail rows grew
/// to 74 pt, so 200 showed two full rows and a cut
/// third — the three built-in entries did not fit
/// whole (their group is 248.375 pt). The panel's
/// place in the sidebar column is unchanged: the
/// column itself scrolls as a whole, and the figures
/// panel's 342 pt cap already sits in the same column
/// under the same rule.
pub const LIST_MAX_H: f32 = 14.375 + ROW_GAP + 5.0 * ROW_H + 4.0 * ROW_GAP;

/// View-state for this section: whether the panel body is
/// collapsed, the search field, the rename session, and
/// the list sort order. Owned by `State` (see the module
/// docs for the wiring); everything about the library
/// itself lives in `EditorCore`.
///
/// The derive drops `Eq` (kept through Phase D) because
/// Rune's `TextEdit` is `PartialEq` but not `Eq`
/// (rune `edit.rs`: `#[derive(Clone, Default, PartialEq)]`).
/// The derived `Default` covers the Phase F fields too:
/// `TextEdit` defaults to empty (the `search` field
/// already relies on it) and `Option` defaults to `None`
/// (and `bool` to `false`, the Phase L sort field), so
/// `LibraryPanelState::default()` keeps working for the
/// integrator's `init()`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LibraryPanelState {
    /// When true, only the header renders (the save button
    /// and the list are omitted). Toggled by the header's
    /// Hide/Show control — a pure view-state change, no
    /// engine op.
    pub collapsed: bool,
    /// Search box contents (Rune core text edit). The row
    /// list filters through the `*_rows_q` helpers on this
    /// text every render; typing is the only writer.
    pub search: TextEdit,
    /// Rename editor contents (Rune core text edit):
    /// [`start_rename`] loads the entry's current name
    /// here; Apply reads it back. Meaningful only while
    /// [`Self::renaming`] is `Some`.
    pub rename: TextEdit,
    /// Library slot currently being renamed, if a rename
    /// session is open. Set by [`start_rename`], cleared
    /// by [`apply_rename`] (when the rename settles) and
    /// [`cancel_rename`].
    pub renaming: Option<usize>,
    /// Row order (Phase L): `false` (the `Default`) keeps
    /// the library's own order — the order `load_library`
    /// added the entries (built-ins first, then saved
    /// entries in sorted file order); `true` sorts the
    /// rendered rows by name via [`sort_rows`]. Flipped
    /// by the `lib-sort` toggle in the search row.
    pub sort_by_name: bool,
}

/// Display-ready data for one library row, derived from the
/// core in one pass so the builder never re-reads the
/// library per element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRow {
    /// Library slot (the `library_index` an insert targets).
    pub index: usize,
    /// Entry name as stored in the library (built-ins keep
    /// their `Built-in: ` prefix).
    pub name: String,
    /// Segment count (`FigureDef::nodes.len()`).
    pub segments: usize,
    /// True for the core's built-in entries (name prefix).
    pub builtin: bool,
}

/// True when a library entry is one of `load_library`'s
/// built-ins (name carries [`BUILTIN_PREFIX`]).
pub fn is_builtin(name: &str) -> bool {
    stykanimate_rs::library_ops::is_builtin(name)
}

/// True when `name` matches the search `query`:
/// case-insensitive substring; an empty (or whitespace-
/// only) query matches everything.
pub fn name_matches(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

/// True when the search `query` actually narrows the
/// list: non-empty once trimmed. A whitespace-only query
/// is no filter at all. Mirrors
/// `figures_panel::filter_active` — the two panels share
/// the shell's filter discipline.
pub fn filter_active(query: &str) -> bool {
    !query.trim().is_empty()
}

/// The backing file name of the library entry `name`:
/// `<name>.json` for a saved entry (see
/// [`library_file_for`] — `load_library` names entries
/// from file stems, so the file name is exactly the stem
/// plus the extension). Built-ins are constructed in
/// code and have no file: `None`.
pub fn entry_file_name(name: &str) -> Option<String> {
    if is_builtin(name) {
        None
    } else {
        Some(format!("{name}.json"))
    }
}

/// True when the library entry `name` matches the search
/// `query`: a case-insensitive substring hit on the
/// display name OR — for saved entries — on the backing
/// file name ([`entry_file_name`]), so `hero.json` and
/// `.json` find the entry `my-hero`. An empty (or
/// whitespace-only) query matches everything, via
/// [`name_matches`].
pub fn entry_matches(name: &str, query: &str) -> bool {
    if name_matches(name, query) {
        return true;
    }
    match entry_file_name(name) {
        Some(file) => name_matches(&file, query),
        None => false,
    }
}

/// The header count line, at the figures panel's
/// discipline (`figures_panel::count_line`): with a
/// filter active, "{shown} of {total} figures"; with no
/// filter, the "{total} figures" form the header has
/// always shown — the idle rendering is unchanged, only
/// the filtered form is new.
pub fn count_line(shown: usize, total: usize, query: &str) -> String {
    if filter_active(query) {
        format!("{shown} of {total} figures")
    } else {
        format!("{total} figures")
    }
}

/// One row per library entry matching `query` (display
/// name or backing file name — [`entry_matches`]), in
/// library order. Row indices are the real library
/// slots, unaffected by filtering.
pub fn library_rows_q(core: &EditorCore, query: &str) -> Vec<LibraryRow> {
    core.library
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| entry_matches(name, query))
        .map(|(index, (name, def))| LibraryRow {
            index,
            name: name.clone(),
            segments: def.nodes.len(),
            builtin: is_builtin(name),
        })
        .collect()
}

/// One row per library entry, in library order.
pub fn library_rows(core: &EditorCore) -> Vec<LibraryRow> {
    library_rows_q(core, "")
}

/// The built-in rows matching `query` (panel order preserved).
pub fn builtin_rows_q(core: &EditorCore, query: &str) -> Vec<LibraryRow> {
    library_rows_q(core, query)
        .into_iter()
        .filter(|r| r.builtin)
        .collect()
}

/// The built-in rows (panel order preserved).
pub fn builtin_rows(core: &EditorCore) -> Vec<LibraryRow> {
    builtin_rows_q(core, "")
}

/// The saved-figure rows matching `query` (everything not
/// built-in).
pub fn saved_rows_q(core: &EditorCore, query: &str) -> Vec<LibraryRow> {
    library_rows_q(core, query)
        .into_iter()
        .filter(|r| !r.builtin)
        .collect()
}

/// The saved-figure rows (everything not built-in).
pub fn saved_rows(core: &EditorCore) -> Vec<LibraryRow> {
    saved_rows_q(core, "")
}

/// The rows in the order the panel renders them
/// (Phase L): with `by_name` false the rows pass
/// through untouched — the library's own order, the
/// order `load_library` added the entries (the three
/// built-ins first, then saved entries in sorted file
/// order). With `by_name` true they are sorted by
/// display name, case-insensitively; the sort is
/// stable, so entries whose names fold to the same
/// string keep their library order relative to each
/// other. Rows carry their real library slots
/// ([`LibraryRow::index`]) through the reorder, so
/// sorting the projection never changes what Insert,
/// Rename or Delete target. The panel applies this
/// within each group (Built-in / Saved) — the
/// grouping itself does not change. The Home panel's
/// [`sort_entries`] discipline, carried over.
///
/// [`sort_entries`]: crate::rune_sec::home_panel::sort_entries
pub fn sort_rows(rows: Vec<LibraryRow>, by_name: bool) -> Vec<LibraryRow> {
    if !by_name {
        return rows;
    }
    let mut rows = rows;
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

/// The backing file of a saved library entry:
/// `./library/<name>.json`. Exact by construction —
/// `load_library` names entries from file stems (see the
/// module docs for the evidence), so the stem→file step
/// is the precise inverse. Only meaningful for saved
/// entries; built-ins have no file.
pub fn library_file_for(name: &str) -> PathBuf {
    stykanimate_rs::library_ops::library_file_for(name)
}

/// Pure index clamp: keep a library selection valid for a
/// library of `len` entries — `0` when empty, otherwise at
/// most `len - 1`; an in-range index is returned unchanged.
pub fn clamped_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}

/// Clamp `core.library_index` back into range after the
/// library vec shrank, so the core's unchecked
/// `library[library_index]` insert can never go stale.
pub fn clamp_library_index(core: &mut EditorCore) {
    core.library_index = clamped_index(core.library_index, core.library.len());
}

/// Delete saved library entry `i`: remove its backing
/// file first, then the vec entry, then clamp
/// `library_index`.
///
/// Honest failure rule: if the file removal fails (missing
/// file included), the vec entry is KEPT and the error is
/// reported on `core.status` — the panel never claims a
/// delete the disk refused. Built-ins are refused before
/// any fs access (they have no file). Out-of-range `i` is
/// a silent no-op, matching [`insert_at`].
pub fn delete_saved(core: &mut EditorCore, i: usize) {
    let Some((name, _)) = core.library.get(i) else {
        return;
    };
    let name = name.clone();
    if is_builtin(&name) {
        core.status = format!("\"{name}\" is built in and can't be deleted.");
        return;
    }
    match std::fs::remove_file(library_file_for(&name)) {
        Ok(()) => {
            core.library.remove(i);
            clamp_library_index(core);
            core.status = format!("Deleted \"{name}\" from the library.");
        }
        Err(e) => {
            core.status = format!("Library delete failed for \"{name}\": {e}");
        }
    }
}

/// Sanitise typed rename text into a library file stem:
/// trim, then the save path's own slug — spaces→`-`,
/// lowercased (`save_figure_to_library`, editor.rs:2187)
/// — plus one guard the save path never needs because
/// figure names come from inside the app: path
/// separators (`/`, `\`) also become `-`, so a typed
/// name can never make [`library_file_for`] point
/// outside `./library`. May return an empty string
/// (blank input); the caller refuses that case.
pub fn sanitize_stem(raw: &str) -> String {
    stykanimate_rs::library_ops::sanitize_stem(raw)
}

/// True when library slot `except` may NOT take the stem
/// `stem` because a DIFFERENT entry already carries it.
/// The entry's own current name is not a collision (the
/// same-name case is settled by the caller before this
/// is consulted). Built-ins are entries too and are
/// checked like any other, though a sanitised stem can
/// never actually equal a `Built-in: ` name (the slug
/// lowercases and dashes the prefix away).
pub fn name_taken(core: &EditorCore, stem: &str, except: usize) -> bool {
    stykanimate_rs::library_ops::name_taken(core, stem, except)
}

/// Rename saved library entry `i` to the stem sanitised
/// from `raw_new` — a thin wrapper over the rename op in
/// the engine library
/// (`EditorCore::rename_library_entry` in
/// `stykanimate_rs::library_ops`), which owns the logic
/// now: it sanitises the typed name,
/// renames the backing file FIRST (`std::fs::rename`),
/// then updates the vec entry — the [`delete_saved`]
/// contract mirrored. The entry keeps its slot, so
/// `library_index` needs no clamp. This wrapper keeps the
/// panel's long-standing name and signature: the rename
/// session ([`apply_rename`]) and this module's tests call
/// it, and panel behaviour is the lib op's behaviour,
/// unchanged.
///
/// Returns whether the rename session is settled:
/// `true` when the editor should close (renamed, the
/// sanitised name already IS the entry's name, or `i`
/// is a stale out-of-range slot — a silent no-op, as in
/// [`delete_saved`]); `false` when the user should keep
/// editing, with the reason on `core.status` (built-in
/// refusal, empty stem, name/file collision, or the fs
/// rename failing). On any failure the entry keeps its
/// old name — the panel never claims a rename the disk
/// refused. A target file that exists but is not a
/// loaded entry (e.g. an unparseable `.json` the loader
/// skipped) is also a refusal: `fs::rename` would
/// silently overwrite it on Unix.
pub fn rename_saved(core: &mut EditorCore, i: usize, raw_new: &str) -> bool {
    core.rename_library_entry(i, raw_new)
}

/// Insert library slot `i` into the project: point
/// `library_index` at it, then the core's insert op.
/// Bounds-checked here because `insert_library_figure`
/// indexes `library[library_index]` unchecked (it only
/// guards the empty library).
pub fn insert_at(core: &mut EditorCore, i: usize) {
    if i < core.library.len() {
        core.library_index = i;
        core.insert_library_figure();
    }
}

/// Delete library entry `i` from the panel (the per-row
/// Delete button's handler): [`delete_saved`] plus the
/// rename-session bookkeeping a removal can disturb.
///
/// The session is a raw library slot. Deleting the row
/// under rename cancels it first (the long-standing
/// rule — the session's entry is going away). A
/// SUCCESSFUL delete of an entry BEFORE the target
/// shifts every later slot down one, so the session
/// follows its entry to `r - 1`; without the follow,
/// the editor row would render under — and Apply would
/// rename — a different entry than the one the user
/// opened. A FAILED delete (the entry is kept, per
/// [`delete_saved`]'s contract) shifts nothing, so the
/// session is left exactly as it was: the length check
/// is what tells the two apart.
pub fn delete_entry(s: &mut State, i: usize) {
    if s.library_panel.renaming == Some(i) {
        cancel_rename(s);
    }
    let len_before = s.core.library.len();
    delete_saved(&mut s.core, i);
    if s.core.library.len() < len_before {
        if let Some(r) = s.library_panel.renaming {
            if r > i {
                s.library_panel.renaming = Some(r - 1);
            }
        }
    }
}

/// Re-point the open rename session at the entry named
/// `target_name`, wherever it sits now; cancel the
/// session when no entry carries that name any more.
/// Library entry names are unique in practice (saved
/// entries are named from file stems, and a directory
/// cannot hold two files with one stem), so the name
/// is a stable identity across a reload that may have
/// moved the entry's slot.
pub fn reresolve_renaming(s: &mut State, target_name: &str) {
    match s.core.library.iter().position(|(n, _)| n == target_name) {
        Some(slot) => s.library_panel.renaming = Some(slot),
        None => cancel_rename(s),
    }
}

/// Save the current figure to the library (the
/// `library-save` button's handler): the core's
/// `save_figure_to_library`, plus rename-session care —
/// a successful save RELOADS the library in sorted
/// file order (editor.rs `load_library`), which can
/// move the slot of the entry being renamed when the
/// new figure's stem sorts ahead of it. The session's
/// target is captured by NAME before the save and
/// re-resolved after it ([`reresolve_renaming`]), so
/// the editor can never end up renaming a different
/// entry than the one the user opened. A failed save
/// leaves the library untouched, and the re-resolution
/// lands on the same slot it started from.
pub fn save_current_figure(s: &mut State) {
    let target = s
        .library_panel
        .renaming
        .and_then(|i| s.core.library.get(i))
        .map(|(name, _)| name.clone());
    s.core.save_figure_to_library();
    if let Some(name) = target {
        reresolve_renaming(s, &name);
    }
}

/// The library slot pressing Enter in the search field
/// inserts: the first row the current filter shows, by
/// its REAL library slot — the same visible→real mapping
/// the row Insert buttons use ([`library_rows_q`] rows
/// keep real indices while filtered). `None` when the
/// filter matches nothing, in which case Enter inserts
/// nothing.
///
/// Phase J (section 25/40): this helper did not exist
/// before this phase. A Phase I summary described an
/// Enter-to-insert path that used the filtered position
/// as a full-list index, but the code never had one —
/// the search field wired no `on_submit` at all, so
/// Enter inserted nothing. The submit path is new here
/// and resolves through the rows' mapping from the
/// start, so the described wrong-figure insert can
/// never happen.
pub fn enter_insert_index(core: &EditorCore, query: &str) -> Option<usize> {
    library_rows_q(core, query).first().map(|row| row.index)
}

/// The search field's Enter dispatch at the core level:
/// insert the first visible entry by its real slot
/// ([`enter_insert_index`] + [`insert_at`]); a filter
/// with no matches is a silent no-op.
pub fn insert_on_enter(core: &mut EditorCore, query: &str) {
    if let Some(i) = enter_insert_index(core, query) {
        insert_at(core, i);
    }
}

/// The library slot pressing Enter inserts, under the
/// panel's sort order (Phase L): the first row the user
/// actually sees. With `by_name` false this IS
/// [`enter_insert_index`] — the same rows in the same
/// order, so the unsorted Enter is byte-identical to
/// before. With `by_name` true the visible order is
/// the grouped projection the panel renders — the
/// Built-in group first, each group name-sorted via
/// [`sort_rows`] — so the first visible row is the
/// first sorted built-in row, or the first sorted
/// saved row when no built-in matches. Either way the
/// row carries its real library slot.
pub fn enter_insert_index_sorted(
    core: &EditorCore,
    query: &str,
    by_name: bool,
) -> Option<usize> {
    if !by_name {
        return enter_insert_index(core, query);
    }
    let builtins = sort_rows(builtin_rows_q(core, query), true);
    if let Some(row) = builtins.first() {
        return Some(row.index);
    }
    sort_rows(saved_rows_q(core, query), true)
        .first()
        .map(|row| row.index)
}

/// The sort-aware Enter dispatch at the core level:
/// insert the first visible entry under the panel's
/// sort order ([`enter_insert_index_sorted`] +
/// [`insert_at`]); a filter with no matches is a
/// silent no-op.
pub fn insert_on_enter_sorted(core: &mut EditorCore, query: &str, by_name: bool) {
    if let Some(i) = enter_insert_index_sorted(core, query, by_name) {
        insert_at(core, i);
    }
}

/// The search field's `on_submit` handler: read the
/// typed query, then the sort-aware core-level dispatch
/// above — Enter inserts the first row visible under
/// the panel's current sort order.
pub fn submit_library_search(s: &mut State) {
    let query = s.library_panel.search.text.clone();
    let by_name = s.library_panel.sort_by_name;
    insert_on_enter_sorted(&mut s.core, &query, by_name);
}

/// Flip the panel collapsed/expanded (the header toggle's
/// handler).
pub fn toggle_collapsed(s: &mut State) {
    s.library_panel.collapsed = !s.library_panel.collapsed;
}

/// Flip the list order between the library's own order
/// and name order (the `lib-sort` toggle's handler in
/// the search row). Pure view state — no engine op;
/// the rows keep their real library slots either way.
pub fn toggle_sort(s: &mut State) {
    s.library_panel.sort_by_name = !s.library_panel.sort_by_name;
}

/// Open the rename editor for library slot `i` (the
/// per-row Rename button's handler): load the entry's
/// current name into the rename field and mark the row
/// as being renamed. The fresh `TextEdit` puts the caret
/// at the end of the name; its `focused` flag claims
/// keyboard focus on the next commit (the command
/// palette's open path relies on the same mechanism), so
/// the integrator's `text_focused` must include the
/// rename field — see the module docs. Built-ins and
/// out-of-range slots open nothing (no Rename button
/// renders for built-ins anyway).
pub fn start_rename(s: &mut State, i: usize) {
    let Some((name, _)) = s.core.library.get(i) else {
        return;
    };
    if is_builtin(name) {
        return;
    }
    let name = name.clone();
    s.library_panel.rename = TextEdit::new(name);
    s.library_panel.rename.focused = true;
    s.library_panel.renaming = Some(i);
}

/// Apply the open rename (the Apply button's handler):
/// run [`rename_saved`] with the field's text, and close
/// the session only when it settles — a refused or
/// failed rename stays open, status line explaining why,
/// so the name can be corrected.
pub fn apply_rename(s: &mut State) {
    let Some(i) = s.library_panel.renaming else {
        return;
    };
    let raw = s.library_panel.rename.text.clone();
    if rename_saved(&mut s.core, i, &raw) {
        s.library_panel.renaming = None;
        s.library_panel.rename = TextEdit::default();
    }
}

/// Abandon the open rename (the Cancel button's
/// handler): close the session and drop the typed text.
/// Nothing on disk or in the library was touched —
/// renames only happen in [`apply_rename`].
pub fn cancel_rename(s: &mut State) {
    s.library_panel.renaming = None;
    s.library_panel.rename = TextEdit::default();
}

/// Edge length (points) of a row thumbnail's canvas:
/// the figures panel's own thumbnail size, shared so
/// every figure miniature in the shell is one size.
pub const THUMB_SIZE: f32 = figures_panel::THUMB_SIZE;

/// Legibility floor (points) for thumbnail stroke
/// widths: a hard taper's far pieces would otherwise
/// shrink under a fifth of a point at this scale and
/// the segment would visually detach from its joint.
/// The stage floors its spray dots at the same 0.8
/// (stage_view's `Dots` pass); endpoints — and every
/// width above the floor — stay exact.
pub const THUMB_MIN_WIDTH: f32 = 0.8;

/// Sides of the thumbnail head-ring polygon (the
/// figures panel's thumbnail choice at this size; the
/// full stage uses 24). Circle shape nodes keep the
/// stage's 24 — see [`library_thumb_prims`].
const THUMB_RING_SIDES: usize = 16;

/// One piece of library-thumbnail geometry in canvas
/// points. [`library_thumb_shape`] turns these into a
/// `Shape`; keeping them separate makes the geometry
/// unit-testable (a built `Shape`'s ops are
/// crate-private to rune) — the figures panel's
/// `ThumbPrim` discipline, extended with the width,
/// colour and alpha a styled piece carries.
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryThumbPrim {
    /// One styled segment piece: draw `a`→`b` as a
    /// round-capped line of `width` points in `color`
    /// at `alpha` (the piece's style opacity).
    Piece {
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// A closed ring — a head ring or a `Circle` shape
    /// node: stroke `pts` closed at `width` points in
    /// `color` at `alpha`.
    Ring {
        pts: Vec<[f32; 2]>,
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// A filled disc — a `Polygon` shape node: fill
    /// `pts` in `color` at `alpha`.
    Disc {
        pts: Vec<[f32; 2]>,
        color: Color,
        alpha: f32,
    },
}

/// A rune `Color` as the `0xRRGGBB` hex the engine's
/// colour math speaks: through the colour's own sRGB
/// encoding (`Color::hex` is `#rrggbbaa` in sRGB), so
/// the round trip `hex_color(color_hex_u32(c))` lands
/// back on `c`'s hue. Used to hand the theme's text
/// colour to `paint::styled_segment_pieces` as the
/// inherited-segment base (see the module docs for why
/// the base is the theme ink, not a figure colour).
fn color_hex_u32(c: Color) -> u32 {
    let hex = c.hex();
    u32::from_str_radix(hex.get(1..7).unwrap_or(""), 16).unwrap_or(0xFF_FFFF)
}

/// The thumbnail geometry for one library entry's def:
/// the def at its REST pose, fitted into a `size`
/// square by the figures panel's [`thumb_fit`] (the
/// box grows by the head-ring radius exactly as the
/// project-figure thumbnails grow it), then — in the
/// stage's own draw order — one styled piece run per
/// parented node, the shape-node rings/discs, and one
/// head ring per head node.
///
/// Every segment goes through
/// `paint::styled_segment_pieces` with the node's own
/// thickness, style and resolved colour
/// (`node.color`, else the `base` colour the caller
/// passes — the panel passes the theme's text colour,
/// a def carrying no figure colour of its own), so
/// taper, caps, `color2` gradients and style opacity
/// are the stage's, not a miniature's approximation of
/// them. The bend argument is 0.0: bends are per-frame
/// project data and a def carries none. Segment media
/// is deliberately not drawn (module docs: a def's
/// asset indices belong to the project it was saved
/// from). Piece widths are the stage's widths scaled
/// by the fit, floored at [`THUMB_MIN_WIDTH`].
///
/// Wave 14 (C8 — clip-def media): no clip-def media
/// can reach this thumbnail even in principle, and
/// none is needed for consistency with the stage.
/// Clip-def media is a relation between a figure
/// INSTANCE and its clip (`fig.clip` →
/// `Project::clips[ci].def`), resolved in
/// `media::media_items` per `docs/wave13/CLIP_MEDIA.md`
/// §6.1; this fn's whole input is one bare
/// `FigureDef` — no instance, no clip index, no
/// project, no asset table — so there is no clip def
/// to enumerate, and this path does not consume the
/// funnel in any case. A library entry's own node
/// media is the only media its def can carry, and it
/// stays undrawn under the module docs' standing
/// argument (its asset indices name the project the
/// entry was saved from, where they may resolve to a
/// different image or to nothing). The test below
/// pins it: a media-carrying def thumbs prim-for-prim
/// identically to the same def without media.
///
/// An empty def (no nodes) yields no prims.
///
/// [`thumb_fit`]: crate::rune_sec::figures_panel::thumb_fit
pub fn library_thumb_prims(def: &FigureDef, size: f32, base: Color) -> Vec<LibraryThumbPrim> {
    if def.nodes.is_empty() {
        return Vec::new();
    }
    let pos = model::evaluate(def, &def.rest_pose([0.0, 0.0]));
    let has_head = def.nodes.iter().any(|n| n.head);
    let fit = figures_panel::thumb_fit(&pos, has_head, size, model::HEAD_RING_RADIUS);
    let s = fit.scale;
    let base_hex = color_hex_u32(base);
    let mut prims = Vec::new();
    // Segments: the stage's construction verbatim
    // (stage_view's figure pass) — pieces are produced
    // in stage space from the raw joints, thickness and
    // style, then mapped; piece widths scale by the
    // fit's scale exactly as the stage scales them by
    // its view scale.
    for (i, node) in def.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let pieces = stykanimate_rs::paint::styled_segment_pieces(
                pos[p],
                pos[i],
                0.0,
                node.thickness,
                &node.style,
                node.color.unwrap_or(base_hex),
            );
            for piece in pieces {
                prims.push(LibraryThumbPrim::Piece {
                    a: fit.map(piece.a),
                    b: fit.map(piece.b),
                    width: (piece.width * s).max(THUMB_MIN_WIDTH),
                    color: hex_color(piece.color).unwrap_or(Color::WHITE),
                    alpha: piece.alpha,
                });
            }
        }
    }
    // Shape nodes: the stage's shape pass — a `Circle`
    // node rings at its own thickness, a `Polygon`
    // fills, both centred on the node with radius half
    // its length and both at the node's style opacity.
    for (i, node) in def.nodes.iter().enumerate() {
        let centre = fit.map(pos[i]);
        let r = node.length / 2.0 * s;
        let color = node
            .color
            .map(|hex| hex_color(hex).unwrap_or(base))
            .unwrap_or(base);
        match node.shape {
            model::Shape::Circle => prims.push(LibraryThumbPrim::Ring {
                pts: ngon(centre, r, 24),
                width: (node.thickness * s).max(THUMB_MIN_WIDTH),
                color,
                alpha: node.style.opacity,
            }),
            model::Shape::Polygon(sides) => prims.push(LibraryThumbPrim::Disc {
                pts: ngon(centre, r, sides as usize),
                color,
                alpha: node.style.opacity,
            }),
            model::Shape::Line => {}
        }
    }
    // Head rings last, at the base colour plain, as the
    // stage draws them (no node colour, no style
    // opacity): radius is the model's unscaled
    // HEAD_RING_RADIUS — a def has no project's head
    // scale, and the constant is the documented default
    // the project's scale multiplies.
    for (i, node) in def.nodes.iter().enumerate() {
        if node.head {
            prims.push(LibraryThumbPrim::Ring {
                pts: ngon(fit.map(pos[i]), model::HEAD_RING_RADIUS * s, THUMB_RING_SIDES),
                width: (5.0 * s).max(THUMB_MIN_WIDTH),
                color: base,
                alpha: 1.0,
            });
        }
    }
    prims
}

/// Library def `def` as a miniature `Shape` for a
/// `size`-point square canvas: the prims of
/// [`library_thumb_prims`] folded into canvas ops —
/// pieces as round-capped lines, rings as closed
/// strokes, discs as fills, each at its prim's alpha.
pub fn library_thumb_shape(def: &FigureDef, size: f32, base: Color) -> Shape {
    let mut shape = Shape::new();
    for prim in library_thumb_prims(def, size, base) {
        shape = match prim {
            LibraryThumbPrim::Piece {
                a,
                b,
                width,
                color,
                alpha,
            } => shape.line(a, b, width, LineCap::Round, color.alpha(alpha)),
            LibraryThumbPrim::Ring {
                pts,
                width,
                color,
                alpha,
            } => shape.stroke_with(pts, true, width, LineCap::Round, color.alpha(alpha)),
            LibraryThumbPrim::Disc { pts, color, alpha } => shape.fill(pts, color.alpha(alpha)),
        };
    }
    shape
}

/// The rename editor row rendered under the row being
/// renamed: the name field plus Apply and Cancel. Only
/// one rename session can be open, so the keys are
/// unindexed — the row's own identity comes from its
/// position under the renamed entry.
fn rename_editor_row(s: &State, t: &Theme) -> El {
    row()
        .key("library-rename-row")
        .gap(8.0)
        .center_y()
        .child(
            text_input(&s.library_panel.rename, type_::BODY.lh(26.0))
                .key("library-rename")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder("New name", t.text_secondary)
                .on_edit(|s: &mut State| &mut s.library_panel.rename),
        )
        .child(icon_btn("library-rename-apply", "check", "Apply", false, t, |s: &mut State| {
            apply_rename(s)
        }))
        .child(icon_btn("library-rename-cancel", "xmark", "Cancel", false, t, |s: &mut State| {
            cancel_rename(s)
        }))
}

/// One library row: name and segment count on the
/// first line, the action buttons on a second line —
/// Insert, and — for saved entries only — Rename and
/// Delete. Built-in rows never get Rename/Delete:
/// built-ins are constructed in code, have no file,
/// and [`rename_saved`]/[`delete_saved`] would refuse
/// them anyway. While this row is the rename target,
/// the rename editor renders directly under it.
///
/// Phase N: the row rides the kit's [`design::list_row`]
/// shell (min height 28, px 8, radius 8, hover overlay)
/// under the same `library-row-{i}` key; the shell's
/// selected fill marks the rename target — the only
/// per-row state the panel carries. The shell takes no
/// handler: the row's pressable children below keep
/// their keys and handlers exactly as before.
///
/// Wave 10 (round-2 width audit): the row used to be a
/// SINGLE line — name + count + buttons side by side.
/// Measured with the shell's own LABEL width model
/// (fx_browser's `preset_char_width`), a saved row's
/// FIXED children alone — "13 seg" ≈ 39 pt, Insert
/// ≈ 80 pt, Rename ≈ 86 pt, Delete 26 pt, plus 32 pt
/// of gaps — total ≈ 263 pt against the 236 pt the
/// row has inside the card (276 pt column content −
/// 24 pt card padding − 16 pt row padding), so Rename
/// and Delete painted past the card's right edge for
/// ANY saved entry, and the longest built-in name
/// ("Built-in: Quadruped", ≈ 118 pt modelled) also
/// overran its ≈ 101 pt share of a built-in row. No
/// headless tree ever showed it: CI checkouts have no
/// `./library` files, so saved rows never rendered
/// there. The two-line anatomy is the figures panel's
/// answer for action-rich rows (its FIG-1 rows): the
/// name line keeps the full 236 pt (the name itself
/// now also carries `.truncate()`, the shell's
/// one-line elision, so an unbounded file-stem name
/// elides instead of forcing the line wide), and the
/// button line fits at ≈ 208 pt worst case.
///
/// Wave 12 (D5): the first line now leads with the
/// entry's thumbnail canvas ([`THUMB_SIZE`] square,
/// key `library-thumb-{i}` — the figures panel's
/// `fig-thumb-{fi}` key discipline), drawn from the
/// entry's def in `core.library` at this very render
/// ([`library_thumb_shape`], base colour the theme's
/// text — a def carries no figure colour, see the
/// module docs). The button line keeps the full
/// 236 pt it was measured against above (the canvas
/// rides the FIRST line only), and the name's share
/// of the first line narrows to ≈ 184 pt (236 −
/// 44 canvas − 8 gap), still behind `.truncate()`, so
/// the Wave 10 width verdicts stand: the fixed
/// children that cannot shrink — the canvas and the
/// segment count — total ≈ 83 pt of that line.
fn entry_row(entry: &LibraryRow, s: &State, t: &Theme) -> El {
    let i = entry.index;
    let mut buttons = row()
        .gap(8.0)
        .center_y()
        .child(icon_btn(&format!("library-insert-{i}"), "plus", "Insert", false, t, move |s: &mut State| {
            insert_at(&mut s.core, i);
        }));
    if !entry.builtin {
        // The rename target's mark lives on the row shell
        // alone (the list_row selected fill below); this
        // button stays inactive so the state is never
        // painted twice — the text_tool precedent, applied
        // by the integrator after the V-wave audit found
        // the doubling.
        buttons = buttons.child(icon_btn(
            &format!("library-rename-{i}"),
            "pencil",
            "Rename",
            false,
            t,
            move |s: &mut State| start_rename(s, i),
        ));
        buttons = buttons.child(icon_only_btn(
            &format!("library-delete-{i}"),
            "trash-can",
            "Delete entry",
            26.0,
            false,
            t,
            move |s: &mut State| delete_entry(s, i),
        ));
    }
    // The thumbnail: this entry's own def, drawn at
    // this render from the same `core.library` the row
    // was derived from — no cache, no stored image, so
    // it cannot show another entry's figure or a stale
    // one. The slot lookup cannot miss (rows derive
    // from this library in this same pass, in
    // `library_panel` below); the empty-shape fallback
    // is defensive only.
    let thumb = match s.core.library.get(i) {
        Some((_, def)) => library_thumb_shape(def, THUMB_SIZE, t.text),
        None => Shape::new(),
    };
    let body = col()
        .grow(1.0)
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    canvas(thumb)
                        .key(format!("library-thumb-{i}"))
                        .size(THUMB_SIZE, THUMB_SIZE),
                )
                .child(
                    row()
                        .grow(1.0)
                        .gap(8.0)
                        .center_y()
                        .child(
                            text(entry.name.clone())
                                .font(type_::LABEL)
                                .color(t.text)
                                .grow(1.0)
                                .truncate(),
                        )
                        .child(
                            text(format!("{} seg", entry.segments))
                                .font(type_::LABEL)
                                .color(t.text_secondary),
                        ),
                ),
        )
        .child(buttons);
    let r = design::list_row(
        &format!("library-row-{i}"),
        s.library_panel.renaming == Some(i),
        t,
    )
    .child(body);
    if s.library_panel.renaming == Some(i) {
        return col()
            .key(format!("library-row-wrap-{i}"))
            .gap(4.0)
            .child(r)
            .child(rename_editor_row(s, t));
    }
    r
}

/// One labelled group of rows ("Built-in" / "Saved").
/// Renders nothing when the group is empty.
fn group(key: &str, label: &str, rows: &[LibraryRow], s: &State, t: &Theme) -> El {
    let mut g = col().key(key.to_string()).gap(4.0);
    if !rows.is_empty() {
        g = g.child(design::section_label(label, t));
        for entry in rows {
            g = g.child(entry_row(entry, s, t));
        }
    }
    g
}

/// The library panel: header (title, entry count, Hide/Show
/// toggle), then — when expanded — the save button and the
/// scrolling, grouped list in a card.
pub fn library_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let collapsed = s.library_panel.collapsed;
    // One query read for the whole panel: the header
    // count and the card's row groups both derive from
    // it, so the count can never disagree with the rows.
    let query = s.library_panel.search.text.clone();
    // The sort order in effect (Phase L): applied to
    // the row projections below and named by the
    // `lib-sort` toggle's label.
    let sort_by_name = s.library_panel.sort_by_name;
    let header_count = count_line(
        library_rows_q(core, &query).len(),
        core.library.len(),
        &query,
    );
    // Phase N: the root is the kit card (panel_col) under
    // the same key; the header rides the kit's keyed
    // header shell — panel_title cluster (book-open glyph
    // per P13, the count line as its meta) with the
    // Hide/Show toggle trailing — over a hairline divider
    // while expanded. The old inner card keeps its key as
    // a plain grouping column: the root is the card now.
    let mut panel = design::panel_col("library-panel", t);
    panel = panel.child(
        design::header_row("library-header")
            .child(design::panel_title(
                Some("book-open"),
                "Library",
                Some(&header_count),
                t,
            ))
            .child(btn(
                "library-collapse",
                if collapsed { "Show" } else { "Hide" },
                collapsed,
                t,
                |s: &mut State| toggle_collapsed(s),
            )),
    );
    if !collapsed {
        panel = panel.child(design::divider(t));
        let mut card = col().key("library-card").gap(6.0);
        card = card.child(btn(
            "library-save",
            "Save current figure to library",
            false,
            t,
            |s: &mut State| save_current_figure(s),
        ));
        card = card.child(
            row()
                .key("library-search-row")
                .gap(8.0)
                .center_y()
                .child(icon_el("magnifying-glass", 13.0, t.text_secondary))
                .child(
                    text_input(&s.library_panel.search, type_::BODY.lh(26.0))
                        .key("library-search")
                        .single_line()
                        .grow(1.0)
                        .color(t.text)
                        .placeholder("Search library", t.text_secondary)
                        .on_edit(|s: &mut State| &mut s.library_panel.search)
                        .on_submit(|s: &mut State, _tx: &mut Tx| submit_library_search(s)),
                )
                .child(icon_btn(
                    "lib-sort",
                    "arrow-down-a-z",
                    if sort_by_name { "Sort: A–Z" } else { "Sort: added" },
                    sort_by_name,
                    t,
                    |s: &mut State| toggle_sort(s),
                )),
        );
        if core.library.is_empty() {
            card = card.child(design::empty_state(
                "shapes",
                "Library is empty — save a figure to add one.",
                t,
            ));
        } else {
            let builtins = sort_rows(builtin_rows_q(core, &query), sort_by_name);
            let saved = sort_rows(saved_rows_q(core, &query), sort_by_name);
            if builtins.is_empty() && saved.is_empty() {
                card = card.child(design::empty_state(
                    "magnifying-glass",
                    &format!("No library figures match \"{}\".", query.trim()),
                    t,
                ));
            }
            let list = col()
                .key("library-list")
                .gap(6.0)
                .child(group("library-builtins", "Built-in", &builtins, s, t))
                .child(group("library-saved", "Saved", &saved, s, t));
            card = card.child(
                // Wave 13 (D4): the viewport sizes to its
                // rows up to the five-row cap and scrolls
                // past it (LIST_MAX_H — the figures list's
                // min_h(0) / max_h pattern), replacing the
                // fixed 200 pt height the taller Wave 12
                // thumbnail rows outgrew.
                list.scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .min_h(0.0)
                .max_h(LIST_MAX_H),
            );
        }
        panel = panel.child(card);
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_defaults_expanded() {
        assert!(!LibraryPanelState::default().collapsed);
    }

    #[test]
    fn builtin_prefix_matches_load_library_names() {
        assert!(is_builtin("Built-in: Humanoid"));
        assert!(is_builtin("Built-in: Mini"));
        assert!(!is_builtin("My Figure"));
        assert!(!is_builtin("Built-in:"));
        assert!(!is_builtin(""));
    }

    #[test]
    fn rows_mirror_the_core_library() {
        let core = EditorCore::new();
        let rows = library_rows(&core);
        assert_eq!(rows.len(), core.library.len());
        for (row, (name, def)) in rows.iter().zip(core.library.iter()) {
            assert_eq!(row.name, *name);
            assert_eq!(row.segments, def.nodes.len());
        }
        // Indices are the library slots, in order.
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.index, i);
        }
    }

    #[test]
    fn fresh_library_is_exactly_the_three_builtins() {
        // `load_library` seeds Humanoid/Quadruped/Mini; a test
        // run may add ./library/*.json entries, so the check
        // is on the builtin group, not the total.
        let core = EditorCore::new();
        let builtins = builtin_rows(&core);
        assert_eq!(builtins.len(), 3);
        assert_eq!(builtins[0].name, "Built-in: Humanoid");
        assert_eq!(builtins[1].name, "Built-in: Quadruped");
        assert_eq!(builtins[2].name, "Built-in: Mini");
        assert!(builtins.iter().all(|r| r.segments > 0));
    }

    #[test]
    fn saved_rows_exclude_builtins() {
        let mut core = EditorCore::new();
        core.library
            .push(("My Hero".into(), stykanimate_rs::model::FigureDef::simple()));
        let saved = saved_rows(&core);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "My Hero");
        assert!(!saved[0].builtin);
        // Slot index still points at the real library slot.
        assert_eq!(saved[0].index, core.library.len() - 1);
    }

    #[test]
    fn rows_empty_when_library_empty() {
        let mut core = EditorCore::new();
        core.library.clear();
        assert!(library_rows(&core).is_empty());
        assert!(builtin_rows(&core).is_empty());
        assert!(saved_rows(&core).is_empty());
    }

    #[test]
    fn insert_at_inserts_that_entry() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        insert_at(&mut core, 2);
        assert_eq!(core.project.figures.len(), n + 1);
        assert_eq!(core.library_index, 2);
        assert!(core.status.contains("Built-in: Mini"));
        // The selection lands on the inserted figure
        // (the core op's own state write, editor.rs
        // `insert_library_figure`): whatever figure any
        // other panel had selected is replaced by the
        // new one, and the node selection clears.
        assert_eq!(core.selected_figure, n);
        assert_eq!(core.selected_node, None);
    }

    #[test]
    fn insert_at_during_playback_still_inserts_and_selects() {
        // Inserting mid-playback is the same op: the
        // figure is appended and selected, and the op
        // does not stop playback to do it (it never
        // touches the flag) — the insert is honest
        // either way, its status line reporting it.
        let mut core = EditorCore::new();
        core.playing = true;
        let n = core.project.figures.len();
        insert_at(&mut core, 0);
        assert_eq!(core.project.figures.len(), n + 1);
        assert_eq!(core.selected_figure, n);
        assert!(core.playing, "the insert leaves playback running");
    }

    #[test]
    fn delete_entry_cancels_the_session_on_its_own_row() {
        // The delete dispatch itself fails here (the
        // synthetic entry has no backing file — the
        // failure path the fs discipline allows), but
        // the session on the deleted row is cancelled
        // BEFORE the attempt, as it always has been.
        let mut s = crate::init();
        s.core.library.push((
            "zzz-test-nonexistent-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let idx = s.core.library.len() - 1;
        start_rename(&mut s, idx);
        assert_eq!(s.library_panel.renaming, Some(idx));
        delete_entry(&mut s, idx);
        assert_eq!(s.library_panel.renaming, None);
        // The failed delete kept the entry.
        assert_eq!(s.core.library.len(), idx + 1);
    }

    #[test]
    fn delete_entry_keeps_the_session_slot_when_the_delete_fails() {
        // Renaming the LATER entry while deleting an
        // EARLIER one: the delete fails (synthetic
        // entry, no file), nothing is removed, so no
        // slot moves and the session must stay exactly
        // where it was — the shift-follow in
        // `delete_entry` is gated on the library
        // actually shrinking.
        let mut s = crate::init();
        s.core.library.push((
            "zzz-test-earlier-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let earlier = s.core.library.len() - 1;
        s.core.library.push((
            "zzz-test-later-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let later = s.core.library.len() - 1;
        start_rename(&mut s, later);
        assert_eq!(s.library_panel.renaming, Some(later));
        delete_entry(&mut s, earlier);
        assert!(
            s.core.status.contains("Library delete failed"),
            "status: {}",
            s.core.status
        );
        assert_eq!(s.core.library.len(), later + 1, "nothing was removed");
        assert_eq!(s.library_panel.renaming, Some(later));
    }

    #[test]
    fn reresolve_renaming_follows_the_entry_name() {
        // The save-reload shape, minus the fs: the
        // library vec is reordered under an open
        // session (as `load_library`'s sorted order can
        // do after a save), and the session must land
        // back on the entry it was opened for — found
        // by name, not by the stale slot.
        let mut s = crate::init();
        s.core.library
            .push(("alpha-w10".into(), stykanimate_rs::model::FigureDef::simple()));
        let alpha = s.core.library.len() - 1;
        s.core.library
            .push(("beta-w10".into(), stykanimate_rs::model::FigureDef::simple()));
        let beta = s.core.library.len() - 1;
        start_rename(&mut s, alpha);
        assert_eq!(s.library_panel.renaming, Some(alpha));
        s.core.library.swap(alpha, beta);
        reresolve_renaming(&mut s, "alpha-w10");
        assert_eq!(s.library_panel.renaming, Some(beta), "the session followed its entry");
        // A name no entry carries any more closes the
        // session rather than leaving it on a stranger.
        reresolve_renaming(&mut s, "gone-w10");
        assert_eq!(s.library_panel.renaming, None);
    }

    #[test]
    fn insert_at_out_of_range_is_a_no_op() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        insert_at(&mut core, usize::MAX);
        assert_eq!(core.project.figures.len(), n);
    }

    #[test]
    fn state_default_search_is_empty() {
        let st = LibraryPanelState::default();
        assert!(st.search.text.is_empty());
        assert!(!st.collapsed);
    }

    #[test]
    fn name_matches_is_case_insensitive_substring() {
        assert!(name_matches("Built-in: Humanoid", "human"));
        assert!(name_matches("my-hero", "HERO"));
        assert!(name_matches("my-hero", "y-he"));
        assert!(!name_matches("my-hero", "villain"));
        // Empty / whitespace queries match everything.
        assert!(name_matches("anything", ""));
        assert!(name_matches("anything", "   "));
    }

    #[test]
    fn filtered_rows_keep_real_library_indices() {
        let mut core = EditorCore::new();
        core.library
            .push(("Alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        core.library
            .push(("Beta".into(), stykanimate_rs::model::FigureDef::simple()));
        // "alp" matches only the saved Alpha entry, at its
        // real slot (after the three built-ins).
        let rows = saved_rows_q(&core, "alp");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Alpha");
        assert_eq!(rows[0].index, 3);
        // Built-ins are filterable too, with real indices.
        let rows = builtin_rows_q(&core, "mini");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].index, 2);
        // A query matching nothing yields no rows at all.
        assert!(library_rows_q(&core, "zzz-no-match").is_empty());
        // Empty query = the unfiltered helpers' output.
        assert_eq!(library_rows_q(&core, ""), library_rows(&core));
        assert_eq!(saved_rows_q(&core, ""), saved_rows(&core));
    }

    #[test]
    fn library_file_maps_name_to_library_dir_json() {
        let p = library_file_for("my-hero");
        assert_eq!(p.file_name().unwrap(), "my-hero.json");
        assert_eq!(p.parent().unwrap().file_name().unwrap(), "library");
        // Dotted stems (file "a.b.json" loads as entry "a.b")
        // map back to the same file.
        assert_eq!(library_file_for("a.b").file_name().unwrap(), "a.b.json");
    }

    #[test]
    fn clamped_index_bounds() {
        assert_eq!(clamped_index(0, 0), 0);
        assert_eq!(clamped_index(5, 0), 0);
        assert_eq!(clamped_index(0, 3), 0);
        assert_eq!(clamped_index(2, 3), 2);
        assert_eq!(clamped_index(3, 3), 2);
        assert_eq!(clamped_index(usize::MAX, 3), 2);
    }

    #[test]
    fn delete_saved_refuses_builtins_without_touching_fs() {
        let mut core = EditorCore::new();
        let n = core.library.len();
        delete_saved(&mut core, 0);
        assert_eq!(core.library.len(), n);
        assert!(core.status.contains("built in"), "status: {}", core.status);
    }

    #[test]
    fn delete_saved_out_of_range_is_a_no_op() {
        let mut core = EditorCore::new();
        let n = core.library.len();
        let status_before = core.status.clone();
        delete_saved(&mut core, usize::MAX);
        assert_eq!(core.library.len(), n);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn delete_saved_keeps_entry_when_file_removal_fails() {
        // The failure path, exercised honestly: a synthetic
        // saved entry whose backing file does not exist, so
        // remove_file fails (NotFound) and the entry must be
        // KEPT with the error on the status line. The
        // success path (a real file under ./library actually
        // removed) is untested by design — see the module
        // docs: tests share the process CWD and must never
        // delete files from a real ./library.
        let mut core = EditorCore::new();
        core.library.push((
            "zzz-test-nonexistent-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let idx = core.library.len() - 1;
        core.library_index = idx;
        delete_saved(&mut core, idx);
        assert_eq!(core.library.len(), idx + 1, "entry must be kept");
        assert_eq!(core.library[idx].0, "zzz-test-nonexistent-entry");
        assert!(
            core.status.contains("Library delete failed"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn state_default_rename_session_is_closed() {
        // The derived Default must cover the Phase F
        // fields: no session open, empty rename field.
        let st = LibraryPanelState::default();
        assert_eq!(st.renaming, None);
        assert!(st.rename.text.is_empty());
        assert!(!st.rename.focused);
    }

    #[test]
    fn sanitize_stem_slugs_like_the_save_path() {
        // The engine's own slug (editor.rs:2187):
        // spaces→'-', lowercased.
        assert_eq!(sanitize_stem("My Hero"), "my-hero");
        assert_eq!(sanitize_stem("CAPS LOCK"), "caps-lock");
        assert_eq!(sanitize_stem("already-a-stem"), "already-a-stem");
        // Typed input is trimmed first.
        assert_eq!(sanitize_stem("  Padded Name  "), "padded-name");
        // Path separators can never survive into a stem.
        assert_eq!(sanitize_stem("a/b\\c"), "a-b-c");
        // Blank input sanitises to nothing (caller refuses).
        assert_eq!(sanitize_stem(""), "");
        assert_eq!(sanitize_stem("   "), "");
    }

    #[test]
    fn name_taken_ignores_the_entry_itself() {
        let mut core = EditorCore::new();
        core.library
            .push(("alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        core.library
            .push(("beta".into(), stykanimate_rs::model::FigureDef::simple()));
        let alpha = core.library.len() - 2;
        let beta = core.library.len() - 1;
        assert!(name_taken(&core, "beta", alpha), "another entry's name is taken");
        assert!(!name_taken(&core, "alpha", alpha), "own name is not a collision");
        assert!(!name_taken(&core, "gamma", alpha), "unused name is free");
        assert!(name_taken(&core, "alpha", beta));
    }

    #[test]
    fn rename_saved_refuses_builtins_without_touching_fs() {
        let mut core = EditorCore::new();
        let n = core.library.len();
        assert!(!rename_saved(&mut core, 0, "New Name"));
        assert_eq!(core.library.len(), n);
        assert_eq!(core.library[0].0, "Built-in: Humanoid");
        assert!(core.status.contains("built in"), "status: {}", core.status);
    }

    #[test]
    fn rename_saved_out_of_range_settles_silently() {
        // A stale session index closes without a status
        // change — the delete out-of-range contract.
        let mut core = EditorCore::new();
        let n = core.library.len();
        let status_before = core.status.clone();
        assert!(rename_saved(&mut core, usize::MAX, "New Name"));
        assert_eq!(core.library.len(), n);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn rename_saved_rejects_empty_and_colliding_names() {
        // Both refusals happen before any fs access, so
        // these run against synthetic entries safely.
        let mut core = EditorCore::new();
        core.library
            .push(("alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        core.library
            .push(("beta".into(), stykanimate_rs::model::FigureDef::simple()));
        let alpha = core.library.len() - 2;
        // Empty after sanitising.
        assert!(!rename_saved(&mut core, alpha, "   "));
        assert_eq!(core.library[alpha].0, "alpha");
        assert!(core.status.contains("Type a new name"), "status: {}", core.status);
        // "Beta" sanitises to the other entry's stem.
        assert!(!rename_saved(&mut core, alpha, "Beta"));
        assert_eq!(core.library[alpha].0, "alpha");
        assert!(
            core.status.contains("already exists"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn rename_saved_same_name_settles_without_fs() {
        // The synthetic entry has no backing file, so any
        // fs attempt would fail — settling here proves the
        // same-name case short-circuits before the rename.
        let mut core = EditorCore::new();
        core.library.push((
            "ghost-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let idx = core.library.len() - 1;
        assert!(rename_saved(&mut core, idx, " Ghost-Entry "));
        assert_eq!(core.library[idx].0, "ghost-entry");
        assert!(
            core.status.contains("already has that name"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn rename_saved_keeps_entry_when_file_rename_fails() {
        // The failure path, exercised honestly like the
        // delete twin: a synthetic saved entry whose
        // backing file does not exist, so fs::rename fails
        // (NotFound) and the entry must keep its OLD name
        // with the error on the status line. The success
        // path (a real file under ./library actually
        // moved) is untested by design — see the module
        // docs: tests share the process CWD and must never
        // move files in a real ./library.
        let mut core = EditorCore::new();
        core.library.push((
            "zzz-test-nonexistent-entry".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let idx = core.library.len() - 1;
        assert!(!rename_saved(&mut core, idx, "A New Name"));
        assert_eq!(core.library[idx].0, "zzz-test-nonexistent-entry");
        assert!(
            core.status.contains("Library rename failed"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn clamp_library_index_after_shrink() {
        let mut core = EditorCore::new();
        core.library_index = 2;
        clamp_library_index(&mut core);
        assert_eq!(core.library_index, 2, "in-range index unchanged");
        core.library.truncate(1);
        clamp_library_index(&mut core);
        assert_eq!(core.library_index, 0);
        core.library.clear();
        core.library_index = 7;
        clamp_library_index(&mut core);
        assert_eq!(core.library_index, 0, "empty library clamps to 0");
    }

    #[test]
    fn filter_active_needs_real_text() {
        assert!(!filter_active(""));
        assert!(!filter_active("   "));
        assert!(filter_active("hero"));
        assert!(filter_active("  hero  "));
    }

    #[test]
    fn entry_file_name_only_for_saved_entries() {
        // A saved entry's file name is its stem plus the
        // extension — the inverse of the loader's naming.
        assert_eq!(entry_file_name("my-hero").as_deref(), Some("my-hero.json"));
        assert_eq!(entry_file_name("a.b").as_deref(), Some("a.b.json"));
        // Built-ins are constructed in code: no file.
        assert_eq!(entry_file_name("Built-in: Mini"), None);
    }

    #[test]
    fn entry_matches_name_or_file_name() {
        // Display-name hits behave exactly like
        // name_matches (case-insensitive substring).
        assert!(entry_matches("my-hero", "HERO"));
        assert!(entry_matches("Built-in: Mini", "mini"));
        // File-name hits the display name alone would miss:
        // the query reaches into the ".json" extension.
        assert!(entry_matches("my-hero", "hero.json"));
        assert!(entry_matches("my-hero", ".JSON"));
        // Built-ins have no file name to hit.
        assert!(!entry_matches("Built-in: Mini", ".json"));
        assert!(!entry_matches("Built-in: Mini", "mini.json"));
        // Genuine misses still miss, and the empty query
        // still matches everything.
        assert!(!entry_matches("my-hero", "villain"));
        assert!(entry_matches("my-hero", ""));
        assert!(entry_matches("my-hero", "   "));
    }

    #[test]
    fn filtered_rows_match_file_name_queries() {
        let mut core = EditorCore::new();
        core.library
            .push(("Alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        // "alpha.json" matches only through the file name,
        // and the row keeps its real library slot.
        let rows = saved_rows_q(&core, "alpha.json");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Alpha");
        assert_eq!(rows[0].index, 3);
        // ".json" matches saved entries (Alpha among
        // them) and never a built-in.
        let rows = library_rows_q(&core, ".json");
        assert!(rows.iter().all(|r| !r.builtin));
        assert!(rows.iter().any(|r| r.name == "Alpha"));
        assert!(builtin_rows_q(&core, ".json").is_empty());
    }

    #[test]
    fn enter_insert_index_is_the_first_visible_rows_real_slot() {
        let mut core = EditorCore::new();
        let alpha = core.library.len();
        core.library
            .push(("Alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        let beta = core.library.len();
        core.library
            .push(("Beta".into(), stykanimate_rs::model::FigureDef::simple()));
        // The pushed entries sit past every preloaded one,
        // so filtered positions and real slots provably
        // differ (filtered position 0 vs real slot `beta`).
        assert!(beta > 0);
        // Unfiltered: the first visible row is slot 0.
        assert_eq!(enter_insert_index(&core, ""), Some(0));
        // Filtered: the first (only) visible row resolves
        // to its REAL slot, never its filtered position.
        assert_eq!(enter_insert_index(&core, "bet"), Some(beta));
        assert_eq!(enter_insert_index(&core, "alp"), Some(alpha));
        // File-name queries resolve through the same rows.
        assert_eq!(enter_insert_index(&core, "beta.json"), Some(beta));
        // No matches: nothing for Enter to insert.
        assert_eq!(enter_insert_index(&core, "zzz-no-match"), None);
    }

    #[test]
    fn insert_on_enter_inserts_the_visible_entry_not_the_first() {
        let mut core = EditorCore::new();
        core.library
            .push(("Alpha".into(), stykanimate_rs::model::FigureDef::simple()));
        let beta = core.library.len();
        core.library
            .push(("Beta".into(), stykanimate_rs::model::FigureDef::simple()));
        let figures_before = core.project.figures.len();
        // "bet" filters the list down to Beta alone; Enter
        // must insert Beta — not the library's first entry
        // (the built-in at slot 0) and not Alpha.
        insert_on_enter(&mut core, "bet");
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert_eq!(core.library_index, beta);
        assert!(
            core.status.contains("Inserted Beta"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn insert_on_enter_unfiltered_inserts_the_first_entry() {
        // The unfiltered Enter keeps the long-standing
        // meaning of "first entry": slot 0.
        let mut core = EditorCore::new();
        let figures_before = core.project.figures.len();
        insert_on_enter(&mut core, "");
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert_eq!(core.library_index, 0);
        assert!(
            core.status.contains("Built-in: Humanoid"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn insert_on_enter_with_no_matches_does_nothing() {
        // No panic, no insert, no state drift at all.
        let mut core = EditorCore::new();
        let figures_before = core.project.figures.len();
        let status_before = core.status.clone();
        let index_before = core.library_index;
        insert_on_enter(&mut core, "zzz-no-match");
        assert_eq!(core.project.figures.len(), figures_before);
        assert_eq!(core.library_index, index_before);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn count_line_follows_the_filter_discipline() {
        // Idle: the header's long-standing form, unchanged.
        assert_eq!(count_line(3, 3, ""), "3 figures");
        assert_eq!(count_line(3, 3, "   "), "3 figures");
        assert_eq!(count_line(0, 0, ""), "0 figures");
        // Filtering: shown of total, including zero shown.
        assert_eq!(count_line(1, 4, "alp"), "1 of 4 figures");
        assert_eq!(count_line(0, 4, "zzz"), "0 of 4 figures");
        assert_eq!(count_line(4, 4, "a"), "4 of 4 figures");
    }

    #[test]
    fn state_default_sort_is_library_order() {
        // The Phase L field rides on the derived
        // Default: rows start in the library's own
        // order, not name order.
        assert!(!LibraryPanelState::default().sort_by_name);
    }

    #[test]
    fn sort_rows_orders_by_name_and_indices_follow_their_entries() {
        let mut core = EditorCore::new();
        let zebra = core.library.len();
        core.library
            .push(("Zebra".into(), stykanimate_rs::model::FigureDef::simple()));
        let apple = core.library.len();
        core.library
            .push(("apple".into(), stykanimate_rs::model::FigureDef::simple()));
        let mango = core.library.len();
        core.library
            .push(("Mango".into(), stykanimate_rs::model::FigureDef::simple()));
        let sorted = sort_rows(saved_rows(&core), true);
        // Case-insensitive name order…
        assert_eq!(
            sorted.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["apple", "Mango", "Zebra"]
        );
        // …and each row still carries the real library
        // slot of its entry, so Insert / Rename / Delete
        // pressed from the sorted list hit the same
        // entries as from the unsorted one.
        assert_eq!(sorted[0].index, apple);
        assert_eq!(sorted[1].index, mango);
        assert_eq!(sorted[2].index, zebra);
    }

    #[test]
    fn sort_rows_sorts_the_builtin_group_by_name_too() {
        // Library order is Humanoid / Quadruped / Mini;
        // name order moves Mini ahead of Quadruped, and
        // the slots move with their entries.
        let core = EditorCore::new();
        let sorted = sort_rows(builtin_rows(&core), true);
        assert_eq!(
            sorted.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["Built-in: Humanoid", "Built-in: Mini", "Built-in: Quadruped"]
        );
        assert_eq!(
            sorted.iter().map(|r| r.index).collect::<Vec<_>>(),
            [0, 2, 1]
        );
    }

    #[test]
    fn sort_rows_by_name_is_stable_for_folded_equals() {
        let mut core = EditorCore::new();
        let first = core.library.len();
        core.library
            .push(("beta".into(), stykanimate_rs::model::FigureDef::simple()));
        let second = core.library.len();
        core.library
            .push(("BETA".into(), stykanimate_rs::model::FigureDef::simple()));
        let sorted = sort_rows(saved_rows(&core), true);
        // Both names fold to "beta"; the stable sort
        // keeps their library order relative to each
        // other.
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].index, first);
        assert_eq!(sorted[1].index, second);
    }

    #[test]
    fn sort_rows_off_passes_rows_through_untouched() {
        // The toggle-off projection is the rows
        // themselves, byte-identical — the same Vec the
        // panel rendered before Phase L, in the
        // library's own order.
        let mut core = EditorCore::new();
        core.library
            .push(("Zebra".into(), stykanimate_rs::model::FigureDef::simple()));
        core.library
            .push(("apple".into(), stykanimate_rs::model::FigureDef::simple()));
        assert_eq!(sort_rows(library_rows(&core), false), library_rows(&core));
        assert_eq!(sort_rows(saved_rows(&core), false), saved_rows(&core));
        assert_eq!(sort_rows(builtin_rows(&core), false), builtin_rows(&core));
    }

    #[test]
    fn enter_insert_index_sorted_matches_unsorted_when_sort_is_off() {
        let mut core = EditorCore::new();
        core.library
            .push(("Jinx".into(), stykanimate_rs::model::FigureDef::simple()));
        core.library
            .push(("Jade".into(), stykanimate_rs::model::FigureDef::simple()));
        for q in ["", "j", "jad", "zzz-no-match"] {
            assert_eq!(
                enter_insert_index_sorted(&core, q, false),
                enter_insert_index(&core, q),
                "query {q:?}"
            );
        }
    }

    #[test]
    fn enter_insert_index_sorted_is_the_first_visible_row() {
        let mut core = EditorCore::new();
        let jinx = core.library.len();
        core.library
            .push(("Jinx".into(), stykanimate_rs::model::FigureDef::simple()));
        let jade = core.library.len();
        core.library
            .push(("Jade".into(), stykanimate_rs::model::FigureDef::simple()));
        // "j" matches Jinx and Jade among the saved
        // entries and no built-in. In library order Jinx
        // is the first visible row; in name order Jade
        // is — Enter must follow what the user sees.
        assert_eq!(enter_insert_index(&core, "j"), Some(jinx));
        assert_eq!(enter_insert_index_sorted(&core, "j", true), Some(jade));
        // Unfiltered, the first visible row under the
        // sort is the name-smallest built-in — still
        // slot 0 ("Built-in: Humanoid" sorts before
        // Mini and Quadruped).
        assert_eq!(enter_insert_index_sorted(&core, "", true), Some(0));
        // No matches: nothing for Enter to insert,
        // sorted or not.
        assert_eq!(enter_insert_index_sorted(&core, "zzz-no-match", true), None);
    }

    #[test]
    fn insert_on_enter_sorted_inserts_the_first_visible_entry() {
        let mut core = EditorCore::new();
        core.library
            .push(("Jinx".into(), stykanimate_rs::model::FigureDef::simple()));
        let jade = core.library.len();
        core.library
            .push(("Jade".into(), stykanimate_rs::model::FigureDef::simple()));
        let figures_before = core.project.figures.len();
        // "j" shows Jinx then Jade in library order,
        // Jade then Jinx sorted; Enter inserts Jade —
        // the first VISIBLE row — by its real slot.
        insert_on_enter_sorted(&mut core, "j", true);
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert_eq!(core.library_index, jade);
        assert!(
            core.status.contains("Inserted Jade"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn panel_builds_in_every_shape() {
        // Phase N construction smoke test: element
        // internals are opaque to a unit test (the design
        // kit's own convention), so this pins only that
        // the kit-shell builder constructs without
        // panicking in each of the panel's shapes —
        // expanded, collapsed, a filter matching nothing
        // (the no-match empty state), an empty library
        // (the empty-library empty state) and a rename
        // session open (the selected row shell + editor).
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        let _ = library_panel(&s, &t);
        s.library_panel.collapsed = true;
        let _ = library_panel(&s, &t);
        s.library_panel.collapsed = false;
        s.library_panel.search = TextEdit::new("zzz-no-match".to_string());
        let _ = library_panel(&s, &t);
        s.library_panel.search = TextEdit::default();
        s.core.library.clear();
        let _ = library_panel(&s, &t);
        s.core = EditorCore::new();
        s.core.library.push((
            "smoke-saved".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let idx = s.core.library.len() - 1;
        start_rename(&mut s, idx);
        assert_eq!(s.library_panel.renaming, Some(idx));
        let _ = library_panel(&s, &t);
        // Wave 10: a saved entry whose file-stem name is
        // far longer than the row — the two-line,
        // truncating row must construct for it too (its
        // width arithmetic is in `entry_row`'s docs;
        // element internals stay opaque here, as above).
        s.library_panel.renaming = None;
        s.core.library.push((
            "a-very-long-saved-figure-name-that-cannot-fit-on-one-line".into(),
            stykanimate_rs::model::FigureDef::simple(),
        ));
        let _ = library_panel(&s, &t);
    }

    #[test]
    fn color_hex_u32_round_trips_through_srgb() {
        assert_eq!(color_hex_u32(Color::WHITE), 0xFF_FFFF);
        assert_eq!(color_hex_u32(Color::BLACK), 0x00_0000);
        // Pure linear red encodes to full sRGB red.
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        assert_eq!(color_hex_u32(red), 0xFF_0000);
        // Alpha is not part of the hex the engine speaks.
        assert_eq!(color_hex_u32(Color::WHITE.alpha(0.25)), 0xFF_FFFF);
    }

    fn piece_count(prims: &[LibraryThumbPrim]) -> usize {
        prims
            .iter()
            .filter(|p| matches!(p, LibraryThumbPrim::Piece { .. }))
            .count()
    }

    fn ring_count(prims: &[LibraryThumbPrim]) -> usize {
        prims
            .iter()
            .filter(|p| matches!(p, LibraryThumbPrim::Ring { .. }))
            .count()
    }

    #[test]
    fn thumb_prims_draw_each_builtins_own_def() {
        // One piece per parented node at the built-ins'
        // flat default styles, plus exactly one head
        // ring — and the counts differ per def, so a
        // thumbnail can never be a shared glyph or
        // another entry's figure: humanoid has 11 nodes,
        // quadruped 13, simple 7, one root each.
        let base = Color::WHITE;
        let humanoid = library_thumb_prims(&FigureDef::humanoid(), THUMB_SIZE, base);
        assert_eq!(piece_count(&humanoid), 10);
        assert_eq!(ring_count(&humanoid), 1);
        let quadruped = library_thumb_prims(&FigureDef::quadruped(), THUMB_SIZE, base);
        assert_eq!(piece_count(&quadruped), 12);
        assert_eq!(ring_count(&quadruped), 1);
        let simple = library_thumb_prims(&FigureDef::simple(), THUMB_SIZE, base);
        assert_eq!(piece_count(&simple), 6);
        assert_eq!(ring_count(&simple), 1);
        // The geometry itself differs, not just counts.
        assert_ne!(humanoid, quadruped);
        assert_ne!(humanoid, simple);
        // Built-ins carry no per-node colours or styles:
        // every piece is the base colour at full alpha.
        for prim in humanoid.iter().chain(quadruped.iter()).chain(simple.iter()) {
            if let LibraryThumbPrim::Piece { color, alpha, .. } = prim {
                assert_eq!(*color, base);
                assert_eq!(*alpha, 1.0);
            }
        }
    }

    #[test]
    fn thumb_prims_fit_inside_the_canvas() {
        // The fit grows the box by the head radius, so
        // every mapped point — ring points included —
        // lands inside the size square.
        for def in [
            FigureDef::humanoid(),
            FigureDef::quadruped(),
            FigureDef::simple(),
        ] {
            let prims = library_thumb_prims(&def, THUMB_SIZE, Color::WHITE);
            assert!(!prims.is_empty());
            for prim in &prims {
                let pts: Vec<[f32; 2]> = match prim {
                    LibraryThumbPrim::Piece { a, b, .. } => vec![*a, *b],
                    LibraryThumbPrim::Ring { pts, .. } => pts.clone(),
                    LibraryThumbPrim::Disc { pts, .. } => pts.clone(),
                };
                for p in pts {
                    assert!(
                        (-0.01..=THUMB_SIZE + 0.01).contains(&p[0]),
                        "x {} out of box",
                        p[0]
                    );
                    assert!(
                        (-0.01..=THUMB_SIZE + 0.01).contains(&p[1]),
                        "y {} out of box",
                        p[1]
                    );
                }
            }
        }
    }

    #[test]
    fn thumb_prims_honour_node_colour_style_and_shape() {
        // A hand-built def exercising every styled path:
        // node 1 plain (inherits the base) and the head;
        // node 2 a red override tapered to a point;
        // node 3 a colour→color2 gradient; node 4 style
        // opacity 0.5; node 5 a Circle shape node at
        // opacity 0.25; node 6 a Polygon shape node.
        // Shape nodes still draw their segment (the
        // stage's rule), so the piece run is one run per
        // parented node, in node order.
        let n = |parent: Option<usize>, length: f32, rest: f32| model::NodeDef {
            name: "n".into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: model::Shape::Line,
            style: model::NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n(None, 0.0, 0.0),       // 0 root
            n(Some(0), 40.0, 180.0), // 1 plain, head
            n(Some(1), 30.0, 0.0),   // 2 red point taper
            n(Some(1), 30.0, 90.0),  // 3 gradient
            n(Some(0), 35.0, -90.0), // 4 opacity 0.5
            n(Some(0), 20.0, 45.0),  // 5 circle
            n(Some(0), 20.0, -45.0), // 6 polygon
        ];
        nodes[1].head = true;
        nodes[2].color = Some(0xFF0000);
        nodes[2].style = model::NodeStyle {
            taper: 0.0,
            cap: model::CapKind::Point,
            color2: None,
            opacity: 1.0,
        };
        nodes[3].style = model::NodeStyle {
            taper: 1.0,
            cap: model::CapKind::Round,
            color2: Some(0x0000FF),
            opacity: 1.0,
        };
        nodes[4].style.opacity = 0.5;
        nodes[5].shape = model::Shape::Circle;
        nodes[5].style.opacity = 0.25;
        nodes[6].shape = model::Shape::Polygon(5);
        let def = FigureDef { nodes };
        let prims = library_thumb_prims(&def, THUMB_SIZE, Color::WHITE);
        let pieces: Vec<&LibraryThumbPrim> = prims
            .iter()
            .filter(|p| matches!(p, LibraryThumbPrim::Piece { .. }))
            .collect();
        // Node order: 1 flat piece, 4 taper pieces, 4
        // gradient pieces, then nodes 4, 5, 6 flat.
        assert_eq!(pieces.len(), 12);
        // Node 1 inherits the base at full alpha.
        match pieces[0] {
            LibraryThumbPrim::Piece { color, alpha, .. } => {
                assert_eq!(*color, Color::WHITE);
                assert_eq!(*alpha, 1.0);
            }
            _ => unreachable!(),
        }
        // Node 2: every piece pure red; widths shrink
        // along the taper, the tail reaching the
        // legibility floor rather than vanishing.
        let mut widths = Vec::new();
        for prim in &pieces[1..5] {
            match prim {
                LibraryThumbPrim::Piece {
                    width,
                    color,
                    alpha,
                    ..
                } => {
                    assert_eq!(*color, Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
                    assert_eq!(*alpha, 1.0);
                    widths.push(*width);
                }
                _ => unreachable!(),
            }
        }
        assert!(widths.windows(2).all(|w| w[0] >= w[1]), "widths: {widths:?}");
        assert!(widths[0] > widths[3]);
        assert_eq!(widths[3], THUMB_MIN_WIDTH);
        // Node 3: the gradient actually travels — first
        // and last piece colours differ, the last one
        // blue-dominant (white → blue).
        let grad: Vec<Color> = pieces[5..9]
            .iter()
            .map(|prim| match prim {
                LibraryThumbPrim::Piece { color, .. } => *color,
                _ => unreachable!(),
            })
            .collect();
        assert_ne!(grad[0], grad[3]);
        assert!(grad[3].b > grad[3].r, "last gradient colour: {:?}", grad[3]);
        // Node 4: the style opacity is the piece alpha.
        match pieces[9] {
            LibraryThumbPrim::Piece { alpha, .. } => assert_eq!(*alpha, 0.5),
            _ => unreachable!(),
        }
        // After the pieces: the Circle node's ring (24
        // pts, at ITS style opacity), the Polygon node's
        // disc (5 pts), and finally the head ring (16
        // pts) at the base colour, full alpha — the
        // stage's head-ring rule.
        assert_eq!(prims.len(), 15);
        match &prims[12] {
            LibraryThumbPrim::Ring { pts, alpha, .. } => {
                assert_eq!(pts.len(), 24);
                assert_eq!(*alpha, 0.25);
            }
            other => panic!("expected circle ring, got {other:?}"),
        }
        match &prims[13] {
            LibraryThumbPrim::Disc { pts, .. } => assert_eq!(pts.len(), 5),
            other => panic!("expected polygon disc, got {other:?}"),
        }
        match &prims[14] {
            LibraryThumbPrim::Ring {
                pts,
                color,
                alpha,
                ..
            } => {
                assert_eq!(pts.len(), 16);
                assert_eq!(*color, Color::WHITE);
                assert_eq!(*alpha, 1.0);
            }
            other => panic!("expected head ring, got {other:?}"),
        }
    }

    #[test]
    fn thumb_prims_ignore_segment_media() {
        // Wave 14 (C8): a library def carrying segment
        // media — an image on one node, text on another —
        // thumbs prim-for-prim identically to the same
        // def without it: own-def media is not drawn on
        // the card, and clip-def media is not addressable
        // from a bare def at all (see
        // `library_thumb_prims`' docs).
        let n = |parent: Option<usize>, length: f32, rest: f32| model::NodeDef {
            name: "n".into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: model::Shape::Line,
            style: model::NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n(None, 0.0, 0.0),       // 0 root
            n(Some(0), 40.0, 180.0), // 1 head
            n(Some(1), 30.0, 0.0),   // 2
        ];
        nodes[1].head = true;
        let plain = FigureDef {
            nodes: nodes.clone(),
        };
        nodes[1].media = Some(model::SegmentMedia::Image { asset: 0 });
        nodes[2].media = Some(model::SegmentMedia::Text {
            content: "hi".into(),
            size: 12.0,
        });
        let carried = FigureDef { nodes };
        let plain_prims = library_thumb_prims(&plain, THUMB_SIZE, Color::WHITE);
        assert!(!plain_prims.is_empty(), "the fixture draws a card");
        assert_eq!(
            library_thumb_prims(&carried, THUMB_SIZE, Color::WHITE),
            plain_prims
        );
    }

    #[test]
    fn thumb_shape_empty_only_for_an_empty_def() {
        let empty = FigureDef { nodes: Vec::new() };
        assert!(library_thumb_prims(&empty, THUMB_SIZE, Color::WHITE).is_empty());
        assert!(library_thumb_shape(&empty, THUMB_SIZE, Color::WHITE).is_empty());
        assert!(!library_thumb_shape(&FigureDef::humanoid(), THUMB_SIZE, Color::WHITE).is_empty());
    }

    #[test]
    fn list_viewport_caps_at_five_rows() {
        // The Wave 13 (D4) viewport rule, as an arithmetic
        // pin in the figures panel's style
        // (`fig_list_viewport_caps_at_five_rows`). One row
        // lays out at 74 pt: the 44 pt thumbnail line, the
        // row body's 4 pt gap, and the 26 pt button line
        // (the icon-only Delete button's box; the labelled
        // icon buttons are 25).
        assert_eq!(ROW_H, THUMB_SIZE + 4.0 + 26.0);
        assert_eq!(ROW_H, 74.0);
        // The cap is one group's SECTION label line
        // (11.5 pt at the 1.25 default line height), its
        // gap, and five rows with the four gaps between
        // them — the shell's five-visible-rows list
        // convention (FIG_LIST_MAX_H / CHAN_LIST_MAX_H).
        assert_eq!(LIST_MAX_H, 14.375 + ROW_GAP + 5.0 * ROW_H + 4.0 * ROW_GAP);
        assert_eq!(LIST_MAX_H, 404.375);
        // One group's content height for n rows, in the
        // group column's own terms (label, gap, rows).
        let content_h = |n: f32| 14.375 + ROW_GAP + n * ROW_H + (n - 1.0) * ROW_GAP;
        // Five rows exactly fill the viewport: the fifth
        // row is fully visible and nothing scrolls (the
        // min_h(0) / max_h viewport is then exactly the
        // content height).
        assert_eq!(content_h(5.0), LIST_MAX_H);
        // Exactly five FULL rows fit under the cap: the
        // visible-row arithmetic the rule promises.
        let full_rows = (LIST_MAX_H - 14.375 - ROW_GAP + ROW_GAP) / (ROW_H + ROW_GAP);
        assert_eq!(full_rows, 5.0);
        // The sixth row is the first that scrolls.
        assert!(content_h(6.0) > LIST_MAX_H, "six rows must exceed the cap");
        // The Wave 12 remainder this fixes: the retired
        // fixed height was 200 pt, under which the three
        // built-in entries (248.375 pt of group) did NOT
        // fit whole — two full rows and a cut third. The
        // same three rows sit comfortably under the cap,
        // so a fresh library shows every built-in without
        // scrolling.
        assert_eq!(content_h(3.0), 248.375);
        assert!(content_h(3.0) > 200.0, "the old 200 pt viewport cut the third built-in");
        assert!(content_h(3.0) < LIST_MAX_H, "three rows size the viewport, no scroll");
        // And two rows at 170.375 pt explain the old
        // viewport's behaviour: 200 pt held two full rows
        // plus a ~30 pt slice of the third.
        assert_eq!(content_h(2.0), 170.375);
    }
}
