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
//!   its name and its segment count (`FigureDef::nodes`,
//!   model.rs:164). Entries whose name carries the core's own
//!   `Built-in: ` prefix (the three `load_library` built-ins)
//!   are grouped under a **Built-in** heading; the rest —
//!   figures saved from the editor — under **Saved**.
//! - One **Insert** button per entry: points
//!   `EditorCore::library_index` (editor.rs:65) at the entry
//!   and calls `EditorCore::insert_library_figure`
//!   (editor.rs:2100), the same op the GPUI Library panel's
//!   Insert button used.
//! - A **Save current figure to library** button calling
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
//!   [`rename_saved`]) sanitises the typed name into a
//!   file stem ([`sanitize_stem`] — the save path's own
//!   slug), refuses empty stems and collisions (another
//!   entry's name, via [`name_taken`], or an existing
//!   file at the target path), renames the backing file
//!   FIRST with `std::fs::rename`, and only then updates
//!   the vec entry's name — the delete contract mirrored:
//!   any fs error keeps the entry under its old name and
//!   reports on `core.status`. The entry keeps its slot,
//!   so `library_index` needs no clamp. A refused or
//!   failed rename keeps the editor open so the name can
//!   be fixed; a settled one closes it. Deleting the row
//!   being renamed cancels the rename session first, so
//!   `renaming` can never point at a shifted entry.
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
use rune::{col, row, text, text_input, El, ScrollSpec, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
use crate::State;

/// The name prefix `load_library` gives its three built-in
/// entries (editor.rs:164-166). It is the only separation
/// the core keeps between built-ins and saved figures, so
/// the grouping below is derived from it.
pub const BUILTIN_PREFIX: &str = "Built-in: ";

/// Height of the scrolling list viewport. A scroll viewport
/// needs a bounded height (rune `el/scroll.rs`); a fixed
/// height keeps the panel predictable wherever the sidebar
/// mounts it.
pub const LIST_HEIGHT: f32 = 200.0;

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
    name.starts_with(BUILTIN_PREFIX)
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
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("library")
        .join(format!("{name}.json"))
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
    raw.trim()
        .replace(' ', "-")
        .replace('/', "-")
        .replace('\\', "-")
        .to_lowercase()
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
    core.library
        .iter()
        .enumerate()
        .any(|(j, (name, _))| j != except && name == stem)
}

/// Rename saved library entry `i` to the stem sanitised
/// from `raw_new`: rename the backing file FIRST
/// (`std::fs::rename`), then update the vec entry — the
/// [`delete_saved`] contract mirrored. The entry keeps
/// its slot, so `library_index` needs no clamp.
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
    let Some((name, _)) = core.library.get(i) else {
        return true;
    };
    let old = name.clone();
    if is_builtin(&old) {
        core.status = format!("\"{old}\" is built in and can't be renamed.");
        return false;
    }
    let new_stem = sanitize_stem(raw_new);
    if new_stem.is_empty() {
        core.status = "Type a new name for the library figure first.".to_string();
        return false;
    }
    if new_stem == old {
        core.status = format!("\"{old}\" already has that name.");
        return true;
    }
    if name_taken(core, &new_stem, i) {
        core.status = format!("A library figure named \"{new_stem}\" already exists.");
        return false;
    }
    let old_path = library_file_for(&old);
    let new_path = library_file_for(&new_stem);
    if new_path.exists() {
        core.status =
            format!("A file named \"{new_stem}.json\" already exists in the library folder.");
        return false;
    }
    match std::fs::rename(&old_path, &new_path) {
        Ok(()) => {
            core.library[i].0 = new_stem.clone();
            core.status = format!("Renamed \"{old}\" to \"{new_stem}\".");
            true
        }
        Err(e) => {
            core.status = format!("Library rename failed for \"{old}\": {e}");
            false
        }
    }
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

/// One library row: name (growing), segment count muted,
/// its Insert button, and — for saved entries only —
/// Rename and Delete buttons. Built-in rows never get
/// them: built-ins are constructed in code, have no
/// file, and [`rename_saved`]/[`delete_saved`] would
/// refuse them anyway. While this row is the rename
/// target, the rename editor renders directly under it.
///
/// Phase N: the row rides the kit's [`design::list_row`]
/// shell (min height 28, px 8, radius 8, hover overlay)
/// under the same `library-row-{i}` key; the shell's
/// selected fill marks the rename target — the only
/// per-row state the panel carries. The shell takes no
/// handler: the row's pressable children below keep
/// their keys and handlers exactly as before.
fn entry_row(entry: &LibraryRow, s: &State, t: &Theme) -> El {
    let i = entry.index;
    let mut r = design::list_row(
        &format!("library-row-{i}"),
        s.library_panel.renaming == Some(i),
        t,
    )
        .child(text(entry.name.clone()).font(type_::LABEL).color(t.text).grow(1.0))
        .child(
            text(format!("{} seg", entry.segments))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(icon_btn(&format!("library-insert-{i}"), "plus", "Insert", false, t, move |s: &mut State| {
            insert_at(&mut s.core, i);
        }));
    if !entry.builtin {
        // The rename target's mark lives on the row shell
        // alone (the list_row selected fill above); this
        // button stays inactive so the state is never
        // painted twice — the text_tool precedent, applied
        // by the integrator after the V-wave audit found
        // the doubling.
        r = r.child(icon_btn(
            &format!("library-rename-{i}"),
            "pencil",
            "Rename",
            false,
            t,
            move |s: &mut State| start_rename(s, i),
        ));
        r = r.child(icon_only_btn(
            &format!("library-delete-{i}"),
            "trash-can",
            "Delete entry",
            26.0,
            false,
            t,
            move |s: &mut State| {
                // Deleting the row under rename would shift
                // every later slot; cancel the session first
                // so `renaming` never points at another entry.
                if s.library_panel.renaming == Some(i) {
                    cancel_rename(s);
                }
                delete_saved(&mut s.core, i);
            },
        ));
    }
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
            |s: &mut State| s.core.save_figure_to_library(),
        ));
        card = card.child(
            row()
                .key("library-search-row")
                .gap(8.0)
                .center_y()
                .child(icon_el("magnifying-glass", 14.0, t.text_secondary))
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
                list.scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .h(LIST_HEIGHT),
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
    }
}
