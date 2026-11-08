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

use rune::tok::{type_, Theme};
use rune::{col, row, text, El, ScrollSpec};
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
/// collapsed. Owned by `State` (see the module docs for the
/// wiring); everything about the library itself lives in
/// `EditorCore`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LibraryPanelState {
    /// When true, only the header renders (the save button
    /// and the list are omitted). Toggled by the header's
    /// Hide/Show control — a pure view-state change, no
    /// engine op.
    pub collapsed: bool,
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

/// One row per library entry, in library order.
pub fn library_rows(core: &EditorCore) -> Vec<LibraryRow> {
    core.library
        .iter()
        .enumerate()
        .map(|(index, (name, def))| LibraryRow {
            index,
            name: name.clone(),
            segments: def.nodes.len(),
            builtin: is_builtin(name),
        })
        .collect()
}

/// The built-in rows (panel order preserved).
pub fn builtin_rows(core: &EditorCore) -> Vec<LibraryRow> {
    library_rows(core).into_iter().filter(|r| r.builtin).collect()
}

/// The saved-figure rows (everything not built-in).
pub fn saved_rows(core: &EditorCore) -> Vec<LibraryRow> {
    library_rows(core)
        .into_iter()
        .filter(|r| !r.builtin)
        .collect()
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
/// and its Insert button.
fn entry_row(entry: &LibraryRow, t: &Theme) -> El {
    let i = entry.index;
    row()
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
        }))
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
        if core.library.is_empty() {
            card = card.child(group_label(
                t,
                "Library is empty — save a figure to add one.",
            ));
        } else {
            let list = col()
                .key("library-list")
                .gap(6.0)
                .child(group(
                    "library-builtins",
                    "Built-in",
                    &builtin_rows(core),
                    t,
                ))
                .child(group("library-saved", "Saved", &saved_rows(core), t));
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
}
