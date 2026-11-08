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
//! ```

use std::path::PathBuf;

use rune::tok::{type_, Theme};
use rune::{col, row, text, text_input, El, ScrollSpec, TextEdit};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
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
/// collapsed, plus the search field. Owned by `State` (see
/// the module docs for the wiring); everything about the
/// library itself lives in `EditorCore`.
///
/// The derive drops `Eq` (kept through Phase D) because
/// Rune's `TextEdit` is `PartialEq` but not `Eq`
/// (rune `edit.rs`: `#[derive(Clone, Default, PartialEq)]`).
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

/// One row per library entry whose name matches `query`,
/// in library order. Row indices are the real library
/// slots, unaffected by filtering.
pub fn library_rows_q(core: &EditorCore, query: &str) -> Vec<LibraryRow> {
    core.library
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| name_matches(name, query))
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

/// Flip the panel collapsed/expanded (the header toggle's
/// handler).
pub fn toggle_collapsed(s: &mut State) {
    s.library_panel.collapsed = !s.library_panel.collapsed;
}

/// A small muted group heading inside the panel.
fn group_label(t: &Theme, label: &str) -> El {
    text(label).font(type_::LABEL).color(t.text_secondary)
}

/// One library row: name (growing), segment count muted,
/// its Insert button, and — for saved entries only — a
/// Delete button. Built-in rows never get one: built-ins
/// are constructed in code, have no file, and
/// [`delete_saved`] would refuse them anyway.
fn entry_row(entry: &LibraryRow, t: &Theme) -> El {
    let i = entry.index;
    let mut r = row()
        .key(format!("library-row-{i}"))
        .gap(8.0)
        .center_y()
        .child(text(entry.name.clone()).font(type_::LABEL).color(t.text).grow(1.0))
        .child(
            text(format!("{} seg", entry.segments))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(btn(&format!("library-insert-{i}"), "Insert", false, t, move |s: &mut State| {
            insert_at(&mut s.core, i);
        }));
    if !entry.builtin {
        r = r.child(btn(
            &format!("library-delete-{i}"),
            "Delete",
            false,
            t,
            move |s: &mut State| delete_saved(&mut s.core, i),
        ));
    }
    r
}

/// One labelled group of rows ("Built-in" / "Saved").
/// Renders nothing when the group is empty.
fn group(key: &str, label: &str, rows: &[LibraryRow], t: &Theme) -> El {
    let mut g = col().key(key.to_string()).gap(4.0);
    if !rows.is_empty() {
        g = g.child(group_label(t, label));
        for entry in rows {
            g = g.child(entry_row(entry, t));
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
    let mut panel = col().key("library-panel").gap(6.0);
    panel = panel.child(
        row()
            .key("library-header")
            .gap(8.0)
            .center_y()
            .child(
                text("Library")
                    .font(type_::TITLE)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(
                text(format!("{} figures", core.library.len()))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            )
            .child(btn(
                "library-collapse",
                if collapsed { "Show" } else { "Hide" },
                collapsed,
                t,
                |s: &mut State| toggle_collapsed(s),
            )),
    );
    if !collapsed {
        let mut card = col()
            .key("library-card")
            .gap(6.0)
            .padding(10.0)
            .radius(8.0)
            .fill(t.card);
        card = card.child(btn(
            "library-save",
            "Save current figure to library",
            false,
            t,
            |s: &mut State| s.core.save_figure_to_library(),
        ));
        let query = s.library_panel.search.text.clone();
        card = card.child(
            row()
                .key("library-search-row")
                .gap(8.0)
                .center_y()
                .child(
                    text_input(&s.library_panel.search, type_::BODY.lh(26.0))
                        .key("library-search")
                        .single_line()
                        .grow(1.0)
                        .color(t.text)
                        .placeholder("Search library", t.text_secondary)
                        .on_edit(|s: &mut State| &mut s.library_panel.search),
                ),
        );
        if core.library.is_empty() {
            card = card.child(group_label(
                t,
                "Library is empty — save a figure to add one.",
            ));
        } else {
            let builtins = builtin_rows_q(core, &query);
            let saved = saved_rows_q(core, &query);
            if builtins.is_empty() && saved.is_empty() {
                card = card.child(group_label(
                    t,
                    &format!("No library figures match \"{}\".", query.trim()),
                ));
            }
            let list = col()
                .key("library-list")
                .gap(6.0)
                .child(group("library-builtins", "Built-in", &builtins, t))
                .child(group("library-saved", "Saved", &saved, t));
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
}
