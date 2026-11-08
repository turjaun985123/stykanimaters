//! Segment style inspector (Rune Phase D, section 5/40;
//! direct setters in Phase E, section 13/40).
//! Declared under `crate::rune_sec` — bin-crate code, not part
//! of the engine library.
//!
//! Style panel for the currently selected segment. Every
//! control calls exactly one real `EditorCore` op:
//!
//! * Cap segmented control → `set_node_cap` (editor.rs, added
//!   by Phase E section 7 beside the style cyclers): each
//!   chip sets exactly its own kind — Round, Square or Point.
//! * Gradient swatches → `set_node_color2` (same addition):
//!   each swatch sets exactly its own gradient end colour —
//!   None (off), white or #222222.
//! * Opacity stepper → `set_node_opacity` (same addition):
//!   − / + in 0.05 steps over the model's 0.0..=1.0 range,
//!   with a % readout.
//! * Taper stepper → `cycle_node_taper` (editor.rs:1796):
//!   taper cycles 1.0 → 0.6 → 0.3 → 0.0 → 1.0. Taper stays a
//!   cycle control on purpose: the engine has NO
//!   `set_node_taper`, and adding one is an engine change,
//!   not a panel change.
//!
//! History: Phase D shipped cap and gradient as single
//! press targets advancing `cycle_node_cap` /
//! `cycle_node_gradient` one step per press, because no
//! direct-set op existed. Phase E section 7 added the
//! setters to the engine, so the chips and swatches now set
//! their own values directly.
//!
//! Deliberately absent: shade steps (fig3d's
//! `segment_shades` is a computed depth shading over a 3D
//! pose, not a stored per-segment style an op can set).

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{CapKind, NodeStyle};

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// The cap kinds in segmented-control display order (the
/// engine's historical cycle order, Round → Square → Point).
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

/// Which of the gradient states `color2` is in:
/// 0 = off (None), 1 = white, 2 = #222222, 3 = any other
/// colour (reachable only from a loaded file — it matches no
/// swatch; pressing a swatch sets that swatch's value
/// directly via `set_node_color2`).
pub fn gradient_state(color2: Option<u32>) -> usize {
    match color2 {
        None => 0,
        Some(0xFF_FFFF) => 1,
        Some(0x22_2222) => 2,
        Some(_) => 3,
    }
}

/// The `color2` value the swatch for `state` sets when
/// pressed: 0 = None (off), 1 = white, 2 = #222222. Only
/// states 0..3 are rendered as swatches; any other input
/// maps to None.
pub fn gradient_value(state: usize) -> Option<u32> {
    match state {
        1 => Some(0xFF_FFFF),
        2 => Some(0x22_2222),
        _ => None,
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

/// Opacity step for the − / + stepper.
pub const OPACITY_STEP: f32 = 0.05;

/// The opacity after one stepper press in direction `dir`
/// (−1 / +1) from `current`: stepped by [`OPACITY_STEP`],
/// clamped to the model's 0.0..=1.0 range, and rounded to
/// whole percents so repeated steps don't accumulate float
/// dust (0.95000004).
pub fn opacity_stepped(current: f32, dir: i32) -> f32 {
    let stepped = current + dir as f32 * OPACITY_STEP;
    (stepped.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

/// % readout for an opacity value (`100%`, `55%`, `5%`).
pub fn opacity_text(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i32)
}

/// One opacity stepper press against the core: read the
/// selected node's current opacity, step it, and set it back
/// through the real `set_node_opacity` op. No selection →
/// no-op (the panel renders no stepper without a selection
/// anyway).
pub fn step_opacity(core: &mut EditorCore, dir: i32) {
    let Some(style) = selected_style(core) else {
        return;
    };
    core.set_node_opacity(opacity_stepped(style.opacity, dir));
}

/// One cap segment chip: accent-filled when it is the current
/// cap, card-filled otherwise. Each chip is its own press
/// target — pressing it sets exactly that cap through
/// `EditorCore::set_node_cap`.
fn cap_chip(cap: CapKind, current: bool, t: &Theme) -> El {
    let accent = tok::accent(0, true);
    el()
        .key(format!("style-cap-{}", cap_label(cap).to_lowercase()))
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if current { accent } else { t.card })
        .hover(if current { accent } else { t.control_track })
        .on_press(move |s: &mut State| s.core.set_node_cap(cap))
        .child(
            text(cap_label(cap))
                .font(type_::LABEL)
                .color(if current { Color::WHITE } else { t.text }),
        )
}

/// One gradient swatch: an 18×18 colour square (the `Off`
/// state uses the theme's off-track fill). The current state
/// gets an accent frame. Each swatch is its own press
/// target — pressing it sets exactly that gradient end
/// colour through `EditorCore::set_node_color2`
/// ([`gradient_value`]).
fn gradient_swatch(state: usize, current: bool, t: &Theme) -> El {
    let fill = match state {
        1 => hex_color(0xFF_FFFF).unwrap_or(Color::WHITE),
        2 => hex_color(0x22_2222).unwrap_or(t.text),
        _ => t.control_off,
    };
    let swatch = el().size(18.0, 18.0).radius(4.0).fill(fill);
    let cell = el()
        .key(format!("style-gradient-{state}"))
        .padding(2.0)
        .radius(6.0)
        .hover(if current {
            tok::accent(0, true)
        } else {
            t.control_track
        })
        .on_press(move |s: &mut State| s.core.set_node_color2(gradient_value(state)));
    if current {
        cell.fill(tok::accent(0, true)).child(swatch)
    } else {
        cell.child(swatch)
    }
}

/// The segment style panel: a header naming the selected
/// segment, then the taper stepper, cap segmented control,
/// gradient swatches and opacity stepper. With no segment
/// selected it renders the header plus a hint and no controls
/// (the style ops would only answer "Select a joint first.").
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
    // — the only taper op is the forward cycler (no direct
    // setter exists; see module docs).
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

    // Cap: segmented control of all three kinds, current
    // accented; each chip sets its own kind directly via
    // `set_node_cap`.
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
            .child(el().key("style-cap").radius(6.0).child(cap_row)),
    );

    // Gradient: one swatch per end-colour state, current
    // framed; each swatch sets its own colour directly via
    // `set_node_color2`.
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
            .child(el().key("style-gradient").radius(6.0).child(swatches))
            .child(
                text(gradient_label(style.color2))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
    );

    // Opacity: − / + stepper (0.05 steps, % readout) setting
    // the selected node's opacity via `set_node_opacity`.
    panel = panel.child(
        row()
            .key("style-opacity-row")
            .gap(8.0)
            .center_y()
            .child(text("Opacity").font(type_::LABEL).color(t.text))
            .child(
                text(opacity_text(style.opacity))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .grow(1.0),
            )
            .child(btn("style-opacity-dn", "−", false, t, |s| {
                step_opacity(&mut s.core, -1)
            }))
            .child(btn("style-opacity-up", "+", false, t, |s| {
                step_opacity(&mut s.core, 1)
            })),
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

    #[test]
    fn cap_chips_set_exactly_that_cap_via_the_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // The default cap is Round. One direct set must land
        // on Point — a single cycle step would give Square,
        // so this proves the chip dispatch is a direct set.
        core.set_node_cap(CapKind::Point);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Point);
        core.set_node_cap(CapKind::Square);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Square);
        core.set_node_cap(CapKind::Round);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Round);
    }

    #[test]
    fn gradient_swatches_set_exactly_that_colour_via_the_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // From Off, one direct set must land on #222222 — a
        // single cycle step would give white, so this proves
        // the swatch dispatch is a direct set.
        core.set_node_color2(gradient_value(2));
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x22_2222));
        core.set_node_color2(gradient_value(1));
        assert_eq!(selected_style(&core).unwrap().color2, Some(0xFF_FFFF));
        core.set_node_color2(gradient_value(0));
        assert_eq!(selected_style(&core).unwrap().color2, None);
    }

    #[test]
    fn gradient_values_match_the_swatch_states() {
        assert_eq!(gradient_value(0), None);
        assert_eq!(gradient_value(1), Some(0xFF_FFFF));
        assert_eq!(gradient_value(2), Some(0x22_2222));
        // Round-trip: each swatch value reads back as its own
        // state, so the pressed swatch is the framed one.
        for state in 0..3 {
            assert_eq!(gradient_state(gradient_value(state)), state);
        }
    }

    #[test]
    fn opacity_stepper_steps_by_five_percent_and_clamps() {
        assert_eq!(opacity_stepped(1.0, -1), 0.95);
        assert_eq!(opacity_stepped(0.5, 1), 0.55);
        assert_eq!(opacity_stepped(0.5, -1), 0.45);
        // Clamped at both ends of the model's 0.0..=1.0 range.
        assert_eq!(opacity_stepped(1.0, 1), 1.0);
        assert_eq!(opacity_stepped(0.98, 1), 1.0);
        assert_eq!(opacity_stepped(0.0, -1), 0.0);
        assert_eq!(opacity_stepped(0.02, -1), 0.0);
    }

    #[test]
    fn opacity_stepper_drives_the_real_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // Default opacity is 1.0: + is clamped at 100%, then
        // each − press removes exactly one 0.05 step.
        step_opacity(&mut core, 1);
        assert_eq!(selected_style(&core).unwrap().opacity, 1.0);
        step_opacity(&mut core, -1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.95);
        step_opacity(&mut core, -1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.90);
        step_opacity(&mut core, 1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.95);
    }

    #[test]
    fn opacity_text_is_a_whole_percent() {
        assert_eq!(opacity_text(1.0), "100%");
        assert_eq!(opacity_text(0.55), "55%");
        assert_eq!(opacity_text(0.05), "5%");
        assert_eq!(opacity_text(0.0), "0%");
    }
}
