//! Segment style inspector (Rune Phase D, section 5/40).
//! Declared under `crate::rune_sec` — bin-crate code, not part
//! of the engine library.
//!
//! Style panel for the currently selected segment. Every
//! control calls exactly one real `EditorCore` op — the three
//! selected-node style cyclers in `editor.rs`:
//!
//! * Taper stepper → `cycle_node_taper` (editor.rs:1796):
//!   taper cycles 1.0 → 0.6 → 0.3 → 0.0 → 1.0.
//! * Cap segmented control → `cycle_node_cap` (editor.rs:1785):
//!   Round → Square → Point via `style::cycle_cap`.
//! * Gradient swatches → `cycle_node_gradient` (editor.rs:1812):
//!   `color2` cycles None → white → #222222 → None.
//!
//! There is NO direct-set op for cap or gradient (no
//! `set_node_cap` / `set_node_color2` in the engine), and each
//! cycler snapshots once per call — so the segmented control
//! and the swatch row are each ONE press target that advances
//! the cycle by a single step, exactly like the GPUI app's
//! single Cap/Gradient buttons. Pressing a specific chip does
//! not jump straight to that state.
//!
//! Deliberately absent (no verified op — see the section
//! report): per-segment opacity (`NodeStyle.opacity` exists in
//! the model but no editor op mutates it for the selected
//! node; only figure-level `cycle_figure_opacity` /
//! `step_figure_opacity` exist) and shade steps (fig3d's
//! `segment_shades` is a computed depth shading, not a stored
//! per-segment style an op can set).

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{CapKind, NodeStyle};

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// The cap cycle order, matching `style::cycle_cap`
/// (Round → Square → Point → Round). Display order for the
/// segmented control.
pub const CAP_ORDER: [CapKind; 3] = [CapKind::Round, CapKind::Square, CapKind::Point];

/// The selected node's current style, if a node is selected.
/// Pure read of the core's public selection fields — the same
/// fields `EditorCore::selected_node_mut` resolves internally.
pub fn selected_style(core: &EditorCore) -> Option<NodeStyle> {
    let ni = core.selected_node?;
    core.project
        .figures
        .get(core.selected_figure)?
        .def
        .nodes
        .get(ni)
        .map(|node| node.style)
}

/// Short display label for a cap kind.
pub fn cap_label(cap: CapKind) -> &'static str {
    match cap {
        CapKind::Round => "Round",
        CapKind::Square => "Square",
        CapKind::Point => "Point",
    }
}

/// Position of `cap` in [`CAP_ORDER`] (the segmented control's
/// highlighted segment).
pub fn cap_index(cap: CapKind) -> usize {
    CAP_ORDER.iter().position(|&c| c == cap).unwrap_or(0)
}

/// Which of the gradient cycle states `color2` is in:
/// 0 = off (None), 1 = white, 2 = #222222, 3 = any other
/// colour (reachable only from a loaded file — the cycler
/// sends it back to None on the next press).
pub fn gradient_state(color2: Option<u32>) -> usize {
    match color2 {
        None => 0,
        Some(0xFF_FFFF) => 1,
        Some(0x22_2222) => 2,
        Some(_) => 3,
    }
}

/// Display label for a gradient state: `Off`, or the end
/// colour as `#RRGGBB`.
pub fn gradient_label(color2: Option<u32>) -> String {
    match color2 {
        None => "Off".to_string(),
        Some(c) => format!("#{c:06X}"),
    }
}

/// Display label for a taper value — the engine's own wording
/// (`style::taper_label`, e.g. `Taper 60%`), so the panel and
/// the status line never disagree.
pub fn taper_text(taper: f32) -> String {
    stykanimate_rs::style::taper_label(taper)
}

/// One cap segment chip: accent-filled when it is the current
/// cap, card-filled otherwise. Purely visual — the press
/// target is the segmented row as a whole (see module docs).
fn cap_chip(cap: CapKind, current: bool, t: &Theme) -> El {
    let accent = tok::accent(0, true);
    el()
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if current { accent } else { t.card })
        .child(
            text(cap_label(cap))
                .font(type_::LABEL)
                .color(if current { Color::WHITE } else { t.text }),
        )
}

/// One gradient swatch: an 18×18 colour square (the `Off`
/// state uses the theme's off-track fill). The current state
/// gets an accent frame; purely visual — the press target is
/// the swatch row as a whole (see module docs).
fn gradient_swatch(state: usize, current: bool, t: &Theme) -> El {
    let fill = match state {
        1 => hex_color(0xFF_FFFF).unwrap_or(Color::WHITE),
        2 => hex_color(0x22_2222).unwrap_or(t.text),
        _ => t.control_off,
    };
    let swatch = el().size(18.0, 18.0).radius(4.0).fill(fill);
    let cell = el().padding(2.0).radius(6.0);
    if current {
        cell.fill(tok::accent(0, true)).child(swatch)
    } else {
        cell.child(swatch)
    }
}

/// The segment style panel: a header naming the selected
/// segment, then the taper stepper, cap segmented control and
/// gradient swatches. With no segment selected it renders the
/// header plus a hint and no controls (the cycler ops would
/// only answer "Select a joint first.").
pub fn style_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let mut panel = col().key("inspector-style").w_full().gap(8.0);
    let header = match core.selected_node {
        Some(ni) => format!("Segment style · Segment {}", ni + 1),
        None => "Segment style".to_string(),
    };
    panel = panel.child(text(header).font(type_::LABEL).color(t.text));
    let Some(style) = selected_style(core) else {
        return panel.child(
            text("Select a segment on the stage.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };

    // Taper: current value (engine wording) + one Step button
    // — the only taper op is the forward cycler.
    panel = panel.child(
        row()
            .key("style-taper-row")
            .gap(8.0)
            .center_y()
            .child(text("Taper").font(type_::LABEL).color(t.text))
            .child(
                text(taper_text(style.taper))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .grow(1.0),
            )
            .child(btn("style-taper", "Step", false, t, |s| {
                s.core.cycle_node_taper()
            })),
    );

    // Cap: segmented display of all three kinds, current
    // accented; the row is a single press target cycling once.
    let mut cap_row = row().gap(4.0);
    for cap in CAP_ORDER {
        cap_row = cap_row.child(cap_chip(cap, cap == style.cap, t));
    }
    panel = panel.child(
        row()
            .key("style-cap-row")
            .gap(8.0)
            .center_y()
            .child(text("Cap").font(type_::LABEL).color(t.text))
            .child(
                el()
                    .key("style-cap")
                    .radius(6.0)
                    .hover(t.control_track)
                    .on_press(|s: &mut State| s.core.cycle_node_cap())
                    .child(cap_row),
            ),
    );

    // Gradient: swatches for the three cycle states, current
    // framed; the row is a single press target cycling once.
    let gstate = gradient_state(style.color2);
    let mut swatches = row().gap(4.0).center_y();
    for state in 0..3 {
        swatches = swatches.child(gradient_swatch(state, state == gstate, t));
    }
    panel = panel.child(
        row()
            .key("style-gradient-row")
            .gap(8.0)
            .center_y()
            .child(text("Gradient").font(type_::LABEL).color(t.text))
            .child(
                el()
                    .key("style-gradient")
                    .radius(6.0)
                    .hover(t.control_track)
                    .on_press(|s: &mut State| s.core.cycle_node_gradient())
                    .child(swatches),
            )
            .child(
                text(gradient_label(style.color2))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
    );

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_index_follows_the_engine_cycle_order() {
        // The segmented highlight must advance exactly the way
        // `style::cycle_cap` advances the op's state.
        for (i, &cap) in CAP_ORDER.iter().enumerate() {
            assert_eq!(cap_index(cap), i);
            let next = stykanimate_rs::style::cycle_cap(cap);
            assert_eq!(next, CAP_ORDER[(i + 1) % CAP_ORDER.len()]);
        }
    }

    #[test]
    fn cap_labels_are_the_three_kind_names() {
        assert_eq!(cap_label(CapKind::Round), "Round");
        assert_eq!(cap_label(CapKind::Square), "Square");
        assert_eq!(cap_label(CapKind::Point), "Point");
    }

    #[test]
    fn gradient_states_cover_the_cycle_and_loaded_colours() {
        assert_eq!(gradient_state(None), 0);
        assert_eq!(gradient_state(Some(0xFF_FFFF)), 1);
        assert_eq!(gradient_state(Some(0x22_2222)), 2);
        // A colour loaded from a file is outside the cycle.
        assert_eq!(gradient_state(Some(0x12_3456)), 3);
        assert_eq!(gradient_label(None), "Off");
        assert_eq!(gradient_label(Some(0xFF_FFFF)), "#FFFFFF");
        assert_eq!(gradient_label(Some(0x22_2222)), "#222222");
    }

    #[test]
    fn taper_text_uses_engine_wording() {
        assert_eq!(taper_text(1.0), "Taper 100%");
        assert_eq!(taper_text(0.6), "Taper 60%");
        assert_eq!(taper_text(0.0), "Taper 0%");
    }

    #[test]
    fn selected_style_is_none_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert!(selected_style(&core).is_none());
    }

    #[test]
    fn selected_style_reads_the_selected_nodes_style() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        let style = selected_style(&core).expect("demo figure has a node 1");
        // A fresh demo project carries default-styled nodes.
        assert_eq!(style, NodeStyle::default());
    }
}
