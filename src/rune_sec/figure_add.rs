//! Phase D, section 2/40 — FIGURE ADD / PRESETS for the Rune
//! editor shell. Declared from `main.rs` only (bin-crate code,
//! not part of the engine library).
//!
//! A "+ Add figure" toggle that expands a panel with two
//! groups, every button wired to a real `EditorCore` op:
//!
//! - **Presets** — Humanoid (`EditorCore::add_figure`,
//!   editor.rs:401 — the project's default figure,
//!   `FigureDef::humanoid()` via model.rs:183/1034, 11 nodes,
//!   created as `Styk {n}`), Quadruped
//!   (`EditorCore::add_quadruped`, editor.rs:689 —
//!   `FigureDef::quadruped()`, model.rs:224, 13 nodes,
//!   created as `Quad {n}`) and Mini
//!   (`EditorCore::add_simple`, editor.rs:701 —
//!   `FigureDef::simple()`, model.rs:265, 7 nodes with merged
//!   two-segment limbs, created as `Mini {n}`). The button
//!   labels name exactly the constructor each one runs
//!   (Wave 9 E8 re-verified against the sources, 2026-11-08).
//! - **Library** — one button per entry of the core's figure
//!   library (`EditorCore::library`, editor.rs:64: the
//!   `./library/*.json` definitions plus three built-ins, so
//!   it is never empty in practice). Pressing one points
//!   `library_index` (editor.rs:65) at that entry and calls
//!   `EditorCore::insert_library_figure` (editor.rs:2371),
//!   the same op the GPUI Library panel's Insert button used.
//!
//! Picking any entry closes the panel again, matching the
//! dismiss-on-pick behaviour of the popover this section
//! stands in for.
//!
//! ## Why not the rune-kit `Popover`?
//!
//! The kit's `Popover::build` needs a `rune_kit::Kit` and a
//! `&mut Vec<El>` overlay layer that the app root feeds into
//! `rune_kit::root(...)` (rune-kit overlay.rs:222). The shell
//! does not construct a `Kit` or a layer today — its root is
//! a plain `col()` themed by `rune::tok` — so anchoring this
//! panel is integrator wiring in `main.rs`. Until then this
//! is a state-toggled section: same controls, same ops, no
//! floating layer. Swapping the card below for a kit
//! `Popover` later changes only this file's `figure_add_panel`
//! body and the root call.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod figure_add;
//! }
//! // in `State`:
//! pub figure_add: figure_add::FigureAddState,
//! // in `init()`:
//! figure_add: figure_add::FigureAddState::default(),
//! // in `view()`, in the LEFT column, inside the same
//! // `if vis(Sec::Figures)` block that mounts the figures
//! // panel (main.rs) — figure-add is the Figures area's own
//! // chrome, so it renders in exactly the workspaces whose
//! // table lists `Section::Figures` (workspaces.rs: the
//! // ANIMATE/DRAW/GRAPH/EFFECTS/AUDIO/BUILD3D section lists;
//! // only HOME_SECTIONS and EXPORT_SECTIONS omit it) and
//! // never orphans into Home or Export (UI mission Wave 3, audit
//! // S05 D1; this file carries no gating predicate of its
//! // own — the gate is the shared mount in main.rs, and the
//! // headless tree dumps show `figure-add` and
//! // `figures-panel` co-present in every scene, 18/18):
//! if vis(Sec::Figures) {
//!     left = left.child(rune_sec::figures_panel::figures_panel(s, &t));
//!     left = left.child(rune_sec::figure_add::figure_add_panel(s, &t));
//! }
//! ```

use rune::tok::Theme;
use rune::{col, row, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::State;

/// View-state for this section: whether the add-figure panel
/// is expanded. Owned by `State` (see the module docs for the
/// wiring); the panel itself is rebuilt from it every frame.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FigureAddState {
    /// Panel expanded under the "+ Add figure" toggle.
    pub open: bool,
}

/// What a panel entry does when picked. Kept separate from
/// the element building so the dispatch is unit-testable
/// against a bare `EditorCore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// `EditorCore::add_figure` — humanoid preset.
    Humanoid,
    /// `EditorCore::add_quadruped` — quadruped preset.
    Quadruped,
    /// `EditorCore::add_simple` — mini preset.
    Mini,
    /// Point `library_index` at this library slot, then
    /// `EditorCore::insert_library_figure`.
    Library(usize),
}

/// One pickable row of the panel: element key, button label
/// and the action it dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub label: String,
    pub kind: EntryKind,
}

/// The three preset entries, in panel order.
pub fn preset_entries() -> Vec<Entry> {
    vec![
        Entry {
            key: "fig-preset-humanoid".into(),
            label: "Humanoid".into(),
            kind: EntryKind::Humanoid,
        },
        Entry {
            key: "fig-preset-quadruped".into(),
            label: "Quadruped".into(),
            kind: EntryKind::Quadruped,
        },
        Entry {
            key: "fig-preset-mini".into(),
            label: "Mini".into(),
            kind: EntryKind::Mini,
        },
    ]
}

/// One entry per figure in the core's library, labelled with
/// the library name (`load_library` guarantees the three
/// built-ins, plus any `./library/*.json` definitions).
pub fn library_entries(core: &EditorCore) -> Vec<Entry> {
    core.library
        .iter()
        .enumerate()
        .map(|(i, (name, _def))| Entry {
            key: format!("fig-library-{i}"),
            label: name.clone(),
            kind: EntryKind::Library(i),
        })
        .collect()
}

/// Dispatch a picked entry to its real `EditorCore` op.
/// Library indices are bounds-checked here because
/// `insert_library_figure` indexes `library[library_index]`
/// unchecked (it only guards the empty library).
pub fn apply_entry(core: &mut EditorCore, kind: EntryKind) {
    match kind {
        EntryKind::Humanoid => core.add_figure(),
        EntryKind::Quadruped => core.add_quadruped(),
        EntryKind::Mini => core.add_simple(),
        EntryKind::Library(i) => {
            if i < core.library.len() {
                core.library_index = i;
                core.insert_library_figure();
            }
        }
    }
}

/// Flip the panel open/closed (the toggle button's handler).
pub fn toggle_open(s: &mut State) {
    s.figure_add.open = !s.figure_add.open;
}

/// One entry button: dispatches its op, then closes the
/// panel (dismiss-on-pick).
fn entry_button(entry: Entry, t: &Theme) -> El {
    let key = entry.key.clone();
    let label = entry.label.clone();
    let kind = entry.kind;
    btn(&key, &label, false, t, move |s: &mut State| {
        apply_entry(&mut s.core, kind);
        s.figure_add.open = false;
    })
}

/// The section as mounted in the shell column: the kit
/// panel card (Phase N) holding the "+ Add figure" toggle
/// (accent-filled while open) as its header control, and —
/// when `s.figure_add.open` — a hairline divider over the
/// preset row and the library list in the body column
/// under it. The body keeps its `figure-add-card` key but
/// is a plain column now: the root is the card, so there
/// is no card inside the card.
pub fn figure_add_panel(s: &State, t: &Theme) -> El {
    let open = s.figure_add.open;
    let mut panel = design::panel_col("figure-add", t);
    panel = panel.child(btn(
        "figure-add-toggle",
        "+ Add figure",
        open,
        t,
        |s: &mut State| toggle_open(s),
    ));
    if open {
        panel = panel.child(design::divider(t));
        let mut card = col().key("figure-add-card").gap(design::PANEL_GAP);
        let mut presets = row().key("figure-add-presets").gap(design::PANEL_GAP);
        for entry in preset_entries() {
            presets = presets.child(entry_button(entry, t));
        }
        card = card
            .child(design::section_label("Presets", t))
            .child(presets);
        card = card.child(design::section_label("Library", t));
        let entries = library_entries(&s.core);
        if entries.is_empty() {
            card = card.child(design::empty_state("shapes", "Library is empty", t));
        } else {
            let mut lib = col().key("figure-add-library").gap(design::PANEL_GAP);
            for entry in entries {
                lib = lib.child(entry_button(entry, t));
            }
            card = card.child(lib);
        }
        panel = panel.child(card);
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_defaults_closed() {
        assert!(!FigureAddState::default().open);
    }

    #[test]
    fn preset_entries_are_the_three_presets_in_order() {
        let entries = preset_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].kind, EntryKind::Humanoid);
        assert_eq!(entries[1].kind, EntryKind::Quadruped);
        assert_eq!(entries[2].kind, EntryKind::Mini);
        // Keys unique (headless --tree lookups depend on it).
        let mut keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn library_entries_mirror_the_core_library() {
        let core = EditorCore::new();
        let entries = library_entries(&core);
        assert_eq!(entries.len(), core.library.len());
        // load_library's built-ins come first, in this order.
        assert!(entries.len() >= 3);
        assert_eq!(entries[0].label, "Built-in: Humanoid");
        assert_eq!(entries[1].label, "Built-in: Quadruped");
        assert_eq!(entries[2].label, "Built-in: Mini");
        assert_eq!(entries[2].kind, EntryKind::Library(2));
    }

    #[test]
    fn library_entries_empty_when_library_empty() {
        let mut core = EditorCore::new();
        core.library.clear();
        assert!(library_entries(&core).is_empty());
    }

    #[test]
    fn apply_presets_each_add_one_figure() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        apply_entry(&mut core, EntryKind::Humanoid);
        assert_eq!(core.project.figures.len(), n + 1);
        apply_entry(&mut core, EntryKind::Quadruped);
        assert_eq!(core.project.figures.len(), n + 2);
        assert!(core.status.contains("Quadruped"));
        apply_entry(&mut core, EntryKind::Mini);
        assert_eq!(core.project.figures.len(), n + 3);
        assert!(core.status.contains("Mini"));
    }

    #[test]
    fn apply_library_entry_inserts_that_entry() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        apply_entry(&mut core, EntryKind::Library(1));
        assert_eq!(core.project.figures.len(), n + 1);
        assert_eq!(core.library_index, 1);
        assert!(core.status.starts_with("Inserted"));
        assert!(core.status.contains("Built-in: Quadruped"));
    }

    #[test]
    fn apply_library_out_of_range_is_a_no_op() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        apply_entry(&mut core, EntryKind::Library(usize::MAX));
        assert_eq!(core.project.figures.len(), n);
    }
}
