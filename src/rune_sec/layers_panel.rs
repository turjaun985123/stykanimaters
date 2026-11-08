//! Layers panel for the Rune editor shell (Phase D,
//! section 32). Bin-crate code — declared under `rune_sec`
//! by the integrator, not part of the engine library.
//!
//! ## What a "layer" is in this engine (the truth from the code)
//!
//! There is **no separate layer list and no per-figure layer
//! assignment field**. The layers engine (`src/layers.rs`
//! module docs) defines draw order as the order of
//! `Project::figures`: index 0 is drawn first, i.e. at the
//! back; the last index is drawn last, i.e. in front. Every
//! figure *is* a layer, and "move up / move down" is figure
//! reordering. The panel states this in its note text rather
//! than pretending a distinct layer stack exists.
//!
//! ## Controls → real ops (all via `s.core`)
//!
//! * Row name press → `EditorCore::select_figure(fi)`
//!   (editor.rs:1646).
//! * Move up (toward front) / Move down (toward back) →
//!   the handler first selects the row's figure, then calls
//!   `EditorCore::move_figure_full(+1 / -1)` (editor.rs:1088).
//!   The "full" wrapper is used deliberately: the simpler
//!   `EditorCore::move_figure` (editor.rs:1655) only swaps
//!   `figures` + frame poses, while `move_figure_full` goes
//!   through `layers::move_figure` (layers.rs:162), which
//!   permutes poses and remaps camera-follow, particle, FX
//!   clip and graph-channel figure references identically,
//!   and the wrapper remaps the editor's selection and solo
//!   index. At the ends of the list the op is a safe no-op.
//! * Top / Bottom (Phase G) → the same handler shape with
//!   the *whole* distance as the delta:
//!   `move_figure_full(target - fi)` where `target` comes
//!   from [`jump_target`] (`len - 1` for Top — the panel's
//!   top row is the front figure — `0` for Bottom). One op
//!   call, not a loop of single steps: `layers::move_figure`
//!   takes a delta of any size, clamps it to the ends, and
//!   does a single remove + insert with the same reference
//!   remap, and the wrapper takes exactly one snapshot —
//!   so a jump costs one undo entry, same as a step. When
//!   [`jump_target`] is `None` (already at that end, or
//!   fewer than two figures) the handler is inert: it only
//!   posts a status line — no selection change, no op call.
//! * Opacity − / + steppers →
//!   `EditorCore::step_figure_opacity(fi, ∓OPACITY_STEP)`
//!   (editor.rs:1177), which writes through
//!   `layers::set_opacity` (layers.rs:126) — clamped to
//!   `MIN_OPACITY..=MAX_OPACITY` (0.05..=1.0, layers.rs:34-36),
//!   so a figure can be faint but never invisible via opacity.
//! * Opacity drag slider (Phase F) → the same write for the
//!   *selected* figure, at an absolute value: the backing
//!   [`set_figure_opacity`] rounds to the stepper's whole-
//!   percent grid and calls `layers::set_opacity` exactly as
//!   `step_figure_opacity` does, with the op's own status
//!   string — but only on release (see below).
//!
//! The slider follows the scene panel's weather sliders
//! (Phase E): the Rune settings example's `speed_control`
//! shape — `on_drag` with the press value stored in
//! `Drag::data`, a write on every Move, `tx.instant()` while
//! scrubbing, and a cancelled drag restoring the press
//! value. The value comes from the pure [`opacity_at`]
//! mapping over the element's own width (no grab offset —
//! a track click jumps, as in the example), and the status
//! line updates **only on release** — mid-drag writes stay
//! silent so a scrub does not spam the status line or the
//! toasts fed from it. The per-row steppers stay for fine
//! control.
//!
//! No undo snapshots, on purpose: opacity writes were never
//! snapshotted — `step_figure_opacity` and
//! `layers::set_opacity` never call `EditorCore::snapshot()`,
//! so a stepper press costs zero undo entries, and the
//! slider mirrors that exactly: a whole drag also costs
//! zero. (Contrast the camera dial, whose backing op *does*
//! snapshot; this one does not.)
//!
//! Rows are listed in draw order with the front figure on
//! top (reverse index order). The selected figure's row name
//! renders active (accent), matching `core.selected_figure`.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::State;

/// Opacity change applied by one stepper press.
pub const OPACITY_STEP: f32 = 0.1;

/// The panel's honest note about ordering in this engine.
pub const ORDER_NOTE: &str =
    "Order = figure order: top row is drawn in front, bottom row at the back.";

/// One display row of the panel, in panel order (front first).
#[derive(Clone, Debug, PartialEq)]
pub struct LayerRow {
    /// Index into `Project::figures`.
    pub fi: usize,
    /// Figure name as shown.
    pub name: String,
    /// Layer opacity as a whole percentage (5..=100).
    pub opacity_pct: i64,
    /// True when this is the editor's selected figure.
    pub selected: bool,
    /// True when this row can still move up (toward front).
    pub can_move_up: bool,
    /// True when this row can still move down (toward back).
    pub can_move_down: bool,
}

/// Figure indices in panel order: front (highest index,
/// drawn last) first, back (index 0, drawn first) last.
pub fn display_order(figure_count: usize) -> Vec<usize> {
    (0..figure_count).rev().collect()
}

/// A figure's layer opacity as a whole percentage.
pub fn opacity_pct(opacity: f32) -> i64 {
    (opacity * 100.0).round() as i64
}

/// The opacity readout text, e.g. `"100%"`.
pub fn opacity_label(opacity: f32) -> String {
    format!("{}%", opacity_pct(opacity))
}

/// True when figure `fi` can move one slot toward the front
/// (a higher draw index exists).
pub fn can_move_up(fi: usize, figure_count: usize) -> bool {
    figure_count > 0 && fi + 1 < figure_count
}

/// True when figure `fi` can move one slot toward the back
/// (it is not already index 0).
pub fn can_move_down(fi: usize, _figure_count: usize) -> bool {
    fi > 0
}

/// Where a Top/Bottom jump would land figure `fi` in a list
/// of `len` figures: `Some(len - 1)` for Top (the front —
/// the panel's top row), `Some(0)` for Bottom (the back).
/// `None` when the jump would not move anything: the figure
/// is already at that end, the list has fewer than two
/// figures, or `fi` is not a figure index at all — the
/// caller treats `None` as inert (status line only).
pub fn jump_target(fi: usize, len: usize, to_top: bool) -> Option<usize> {
    if len < 2 || fi >= len {
        return None;
    }
    let target = if to_top { len - 1 } else { 0 };
    if target == fi {
        None
    } else {
        Some(target)
    }
}

/// Build the panel's row data from the core, front first.
pub fn rows(core: &EditorCore) -> Vec<LayerRow> {
    let figures = &core.project.figures;
    display_order(figures.len())
        .into_iter()
        .map(|fi| LayerRow {
            fi,
            name: figures[fi].name.clone(),
            opacity_pct: opacity_pct(figures[fi].opacity),
            selected: fi == core.selected_figure,
            can_move_up: can_move_up(fi, figures.len()),
            can_move_down: can_move_down(fi, figures.len()),
        })
        .collect()
}

/// Slider track width in points (the scene panel's slider
/// geometry, so every drag slider in the shell feels the
/// same).
const SLIDER_W: f32 = 200.0;
/// Slider track height in points.
const SLIDER_H: f32 = 22.0;
/// Slider knob diameter in points.
const KNOB: f32 = 16.0;

/// Slider mapping: the opacity at local `x` on a track `w`
/// points wide — 0.0 at the left edge, 1.0 at the right,
/// linear between, clamped outside. A zero-width track (no
/// layout yet) maps to 0.0 rather than dividing by zero.
/// The engine's 0.05 floor is *not* applied here — it lives
/// in `layers::set_opacity`, the one write path, exactly as
/// a stepper press can also request a below-floor value.
pub fn opacity_at(x: f32, w: f32) -> f32 {
    if w <= 0.0 {
        return 0.0;
    }
    (x / w).clamp(0.0, 1.0)
}

/// Track fill fraction for an opacity — the inverse of
/// [`opacity_at`], so the knob sits where a press at the
/// same fraction would land.
pub fn opacity_frac(opacity: f32) -> f32 {
    opacity.clamp(0.0, 1.0)
}

/// Slider backing: set figure `fi`'s opacity to an absolute
/// `value`, through the stepper's own write — rounded to
/// the whole-percent grid `step_figure_opacity` computes,
/// then `layers::set_opacity` (which clamps to
/// 0.05..=1.0). No snapshot, like the op. `report` is true
/// only on drag release: a reporting write posts the op's
/// own status string (or its "No such figure." refusal);
/// mid-drag writes stay silent.
pub fn set_figure_opacity(core: &mut EditorCore, fi: usize, value: f32, report: bool) {
    let next = (value * 100.0).round() / 100.0;
    if stykanimate_rs::layers::set_opacity(&mut core.project, fi, next) {
        if report {
            let o = core.project.figures[fi].opacity;
            core.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0);
        }
    } else if report {
        core.status = "No such figure.".into();
    }
}

/// Opacity slider drag (the settings example's shape, via
/// the scene panel): Down stores the selected figure's
/// opacity in `Drag::data` and jumps to the pressed point,
/// Move rewrites the opacity and asks for an instant
/// redraw, Up reports — or, when the gesture was cancelled,
/// restores the press value (silently, through the same
/// backing write).
fn opacity_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let core = &mut s.core;
    let fi = core.selected_figure;
    match d.phase {
        Phase::Down => {
            d.data = core
                .project
                .figures
                .get(fi)
                .map(|f| f.opacity)
                .unwrap_or(1.0);
            set_figure_opacity(core, fi, opacity_at(d.x, d.w), false);
        }
        Phase::Move => {
            set_figure_opacity(core, fi, opacity_at(d.x, d.w), false);
            tx.instant();
        }
        Phase::Up => {
            if d.cancelled() {
                set_figure_opacity(core, fi, d.data, false);
            } else {
                set_figure_opacity(core, fi, opacity_at(d.x, d.w), true);
            }
        }
    }
}

/// The selected figure's opacity slider (the scene panel's
/// `weather_slider` shape): a fixed-size track element with
/// track, fill and knob children drawn absolutely inside
/// it. The fill runs from the left knob centre to the
/// current value's knob centre. All gesture logic lives in
/// [`opacity_drag`].
fn opacity_slider(frac: f32, t: &Theme) -> El {
    let frac = frac.clamp(0.0, 1.0);
    let travel = SLIDER_W - KNOB;
    let knob_cx = KNOB / 2.0 + frac * travel;
    el()
        .key("layers-opacity-slider")
        .size(SLIDER_W, SLIDER_H)
        .on_drag(opacity_drag)
        .child(
            el()
                .size(SLIDER_W, 4.0)
                .absolute(0.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(t.control_off),
        )
        .child(
            el()
                .size((knob_cx - KNOB / 2.0).max(4.0), 4.0)
                .absolute(KNOB / 2.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(tok::accent(0, true)),
        )
        .child(
            el()
                .size(KNOB, KNOB)
                .absolute(frac * travel, (SLIDER_H - KNOB) / 2.0)
                .radius(KNOB / 2.0)
                .fill(t.text),
        )
}

/// Move handler shared by the up/down buttons: select the
/// row's figure, then reorder it with the full layers engine.
/// `dir` is +1 toward the front, -1 toward the back.
fn move_layer(core: &mut EditorCore, fi: usize, dir: i32) {
    core.select_figure(fi);
    core.move_figure_full(dir);
}

/// Jump handler shared by the Top/Bottom buttons: like
/// [`move_layer`], but the delta is the whole distance to
/// the end, so the figure lands at the front (`to_top`) or
/// the back in a single `move_figure_full` call — one
/// snapshot, one reference remap, exactly like a step.
/// When [`jump_target`] has nowhere to go the press is
/// inert: only the status line changes.
fn jump_layer(core: &mut EditorCore, fi: usize, to_top: bool) {
    let len = core.project.figures.len();
    match jump_target(fi, len, to_top) {
        Some(target) => {
            core.select_figure(fi);
            core.move_figure_full(target as i32 - fi as i32);
        }
        None => {
            core.status = if len < 2 {
                "Only one figure: nothing to reorder.".into()
            } else if to_top {
                "Already at the top.".into()
            } else {
                "Already at the bottom.".into()
            };
        }
    }
}

/// One layer row: name (press selects; accent when selected),
/// opacity readout with −/+ steppers, and Bottom/down/up/Top
/// reorder buttons. Keys are per-figure: `layer-row-{fi}`,
/// `layer-op-dec-{fi}`, `layer-op-inc-{fi}`,
/// `layer-bottom-{fi}`, `layer-down-{fi}`, `layer-up-{fi}`,
/// `layer-top-{fi}` (1-based in the key is avoided — keys
/// carry the engine index so headless `--tree` checks map
/// straight back to `Project::figures`).
fn layer_row(r: &LayerRow, t: &Theme) -> El {
    let fi = r.fi;
    row()
        .key(format!("layer-row-{fi}"))
        .gap(6.0)
        .center_y()
        .child(btn(
            &format!("layer-name-{fi}"),
            &r.name,
            r.selected,
            t,
            move |s: &mut State| s.core.select_figure(fi),
        ))
        .child(el().grow(1.0))
        .child(btn(
            &format!("layer-op-dec-{fi}"),
            "-",
            false,
            t,
            move |s: &mut State| s.core.step_figure_opacity(fi, -OPACITY_STEP),
        ))
        .child(
            text(format!("{}%", r.opacity_pct))
                .font(type_::LABEL)
                .color(t.text),
        )
        .child(btn(
            &format!("layer-op-inc-{fi}"),
            "+",
            false,
            t,
            move |s: &mut State| s.core.step_figure_opacity(fi, OPACITY_STEP),
        ))
        .child(btn(
            &format!("layer-bottom-{fi}"),
            "Bottom",
            false,
            t,
            move |s: &mut State| jump_layer(&mut s.core, fi, false),
        ))
        .child(btn(
            &format!("layer-down-{fi}"),
            "Down",
            false,
            t,
            move |s: &mut State| move_layer(&mut s.core, fi, -1),
        ))
        .child(btn(
            &format!("layer-up-{fi}"),
            "Up",
            false,
            t,
            move |s: &mut State| move_layer(&mut s.core, fi, 1),
        ))
        .child(btn(
            &format!("layer-top-{fi}"),
            "Top",
            false,
            t,
            move |s: &mut State| jump_layer(&mut s.core, fi, true),
        ))
}

/// The full layers panel: a "Layers" header, one row per
/// figure in draw order (front on top), the selected
/// figure's opacity slider with its % readout, and the
/// ordering note. With no figures the panel is just
/// header + note (no slider — there is no selected figure
/// to fade).
pub fn panel(s: &State, t: &Theme) -> El {
    let mut body = col()
        .key("layers-panel")
        .gap(6.0)
        .child(text("Layers").font(type_::LABEL).color(t.text_secondary));
    for r in rows(&s.core) {
        body = body.child(layer_row(&r, t));
    }
    if let Some(fig) = s.core.project.figures.get(s.core.selected_figure) {
        body = body
            .child(
                row()
                    .key("layers-opacity-row")
                    .gap(6.0)
                    .center_y()
                    .child(text("Opacity").font(type_::LABEL).color(t.text_secondary))
                    .child(el().grow(1.0))
                    .child(
                        text(opacity_label(fig.opacity))
                            .font(type_::LABEL)
                            .color(t.text),
                    ),
            )
            .child(opacity_slider(opacity_frac(fig.opacity), t));
    }
    body.child(
        text(ORDER_NOTE)
            .font(type_::LABEL)
            .color(t.text_secondary),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_order_is_front_first() {
        assert_eq!(display_order(3), vec![2, 1, 0]);
        assert_eq!(display_order(1), vec![0]);
        assert!(display_order(0).is_empty());
    }

    #[test]
    fn opacity_label_rounds_to_whole_percent() {
        assert_eq!(opacity_label(1.0), "100%");
        assert_eq!(opacity_label(0.6), "60%");
        assert_eq!(opacity_label(0.05), "5%");
        assert_eq!(opacity_pct(0.3), 30);
    }

    #[test]
    fn move_bounds_follow_draw_order() {
        // Back figure (0) can only move up; front figure (2 of
        // 3) can only move down; the middle moves both ways.
        assert!(!can_move_down(0, 3) && can_move_up(0, 3));
        assert!(can_move_down(1, 3) && can_move_up(1, 3));
        assert!(can_move_down(2, 3) && !can_move_up(2, 3));
        assert!(!can_move_up(0, 0) && !can_move_down(0, 0));
    }

    #[test]
    fn rows_are_front_first_with_selection_marked() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.select_figure(1);
        let rs = rows(&core);
        assert_eq!(rs.len(), 3);
        assert_eq!(rs[0].name, "C");
        assert_eq!(rs[0].fi, 2);
        assert!(!rs[0].can_move_up && rs[0].can_move_down);
        assert_eq!(rs[1].name, "B");
        assert!(rs[1].selected);
        assert_eq!(rs[2].fi, 0);
        assert!(rs[2].can_move_up && !rs[2].can_move_down);
        assert!(rs.iter().all(|r| r.opacity_pct == 100));
    }

    #[test]
    fn move_layer_reorders_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // Move the back figure up: it becomes the front figure
        // and the selection follows it to index 1.
        move_layer(&mut core, 0, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        assert_eq!(core.selected_figure, 1);
        // Moving up again at the front is a no-op.
        move_layer(&mut core, 1, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        // And back down restores the original order.
        move_layer(&mut core, 1, -1);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
    }

    #[test]
    fn jump_target_covers_ends_middle_and_degenerate_lists() {
        // Ends and middle of a 3-figure list (Top = front =
        // highest index, Bottom = back = index 0).
        assert_eq!(jump_target(0, 3, true), Some(2));
        assert_eq!(jump_target(1, 3, true), Some(2));
        assert_eq!(jump_target(2, 3, true), None);
        assert_eq!(jump_target(2, 3, false), Some(0));
        assert_eq!(jump_target(1, 3, false), Some(0));
        assert_eq!(jump_target(0, 3, false), None);
        // Two figures: the only possible jumps.
        assert_eq!(jump_target(0, 2, true), Some(1));
        assert_eq!(jump_target(1, 2, false), Some(0));
        assert_eq!(jump_target(1, 2, true), None);
        assert_eq!(jump_target(0, 2, false), None);
        // Degenerate lists never jump.
        assert_eq!(jump_target(0, 1, true), None);
        assert_eq!(jump_target(0, 1, false), None);
        assert_eq!(jump_target(0, 0, true), None);
        // A stale index past the end never jumps either.
        assert_eq!(jump_target(3, 3, true), None);
        assert_eq!(jump_target(5, 3, false), None);
    }

    #[test]
    fn jump_layer_moves_to_the_ends_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // One press: the back figure lands at the front and
        // the op's own status names its new 1-based position.
        jump_layer(&mut core, 0, true);
        assert_eq!(core.project.figures[2].name, back_name);
        assert_eq!(core.selected_figure, 2);
        assert_eq!(core.status, "Figure moved to position 3.");
        // And one press back to the bottom restores the order.
        jump_layer(&mut core, 2, false);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.status, "Figure moved to position 1.");
    }

    #[test]
    fn jump_layer_carries_figure_references_in_the_same_call() {
        // The jump goes through `layers::move_figure`, so the
        // references the engine tracks travel with their
        // figures — this pins that the panel's one call is
        // the full engine path, not the bare swap.
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.project.camera_follow = Some(2); // follows C
        core.project.channels.push(stykanimate_rs::graph::Channel {
            figure: 2, // on C
            target: stykanimate_rs::graph::ChannelTarget::RootX,
            keys: Vec::new(),
        });
        core.project.fx_clips.push(stykanimate_rs::model::FxClipEntry {
            clip: stykanimate_rs::vfx::ParticleClip {
                start: 0.0,
                duration: 8.0,
                kind: stykanimate_rs::vfx::ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 1,
                angle: 0.0,
                length: 100.0,
                color2: None,
            },
            figure: 0, // on the jumping figure
            node: 2,
        });
        core.project.particles.push(stykanimate_rs::model::ParticleEvent {
            frame: 0,
            figure: Some(1), // on B
            node: Some(3),
            x: 0.0,
            y: 0.0,
            kind: stykanimate_rs::model::ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 0,
        });

        // Figure 0 jumps to the front (2); B and C shift down.
        jump_layer(&mut core, 0, true);
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.particles[0].figure, Some(0));
        assert_eq!(core.project.channels[0].figure, 1);
        assert_eq!(core.project.camera_follow, Some(1));
    }

    #[test]
    fn jump_layer_is_status_only_when_already_at_the_end() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        let names_before: Vec<String> =
            core.project.figures.iter().map(|f| f.name.clone()).collect();
        core.select_figure(1);
        // The front figure is already at the top: no reorder,
        // no selection steal — only the status line changes.
        jump_layer(&mut core, 2, true);
        assert_eq!(core.status, "Already at the top.");
        assert_eq!(core.selected_figure, 1);
        // Same for the back figure and the bottom.
        jump_layer(&mut core, 0, false);
        assert_eq!(core.status, "Already at the bottom.");
        assert_eq!(core.selected_figure, 1);
        let names_after: Vec<String> =
            core.project.figures.iter().map(|f| f.name.clone()).collect();
        assert_eq!(names_before, names_after);
    }

    #[test]
    fn jump_layer_is_status_only_for_a_single_figure() {
        let mut core = EditorCore::new();
        assert_eq!(core.project.figures.len(), 1);
        jump_layer(&mut core, 0, true);
        assert_eq!(core.status, "Only one figure: nothing to reorder.");
        jump_layer(&mut core, 0, false);
        assert_eq!(core.status, "Only one figure: nothing to reorder.");
        assert_eq!(core.project.figures.len(), 1);
    }

    #[test]
    fn opacity_stepper_op_clamps_through_the_layers_engine() {
        let mut core = EditorCore::new();
        core.step_figure_opacity(0, -OPACITY_STEP);
        assert_eq!(opacity_label(core.project.figures[0].opacity), "90%");
        // Stepping above 1.0 clamps to fully opaque.
        core.step_figure_opacity(0, 5.0);
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn opacity_at_maps_edges_middle_and_clamps() {
        assert_eq!(opacity_at(0.0, 200.0), 0.0);
        assert_eq!(opacity_at(200.0, 200.0), 1.0);
        assert_eq!(opacity_at(100.0, 200.0), 0.5);
        assert_eq!(opacity_at(50.0, 200.0), 0.25);
        assert_eq!(opacity_at(-30.0, 200.0), 0.0);
        assert_eq!(opacity_at(260.0, 200.0), 1.0);
        // A zero-width track maps to 0, never NaN.
        assert_eq!(opacity_at(10.0, 0.0), 0.0);
    }

    #[test]
    fn opacity_frac_is_the_inverse_of_opacity_at() {
        for x in [0.0, 25.0, 50.0, 100.0, 150.0, 200.0] {
            assert_eq!(opacity_frac(opacity_at(x, 200.0)), x / 200.0);
        }
        // Out-of-range stored values clamp onto the track.
        assert_eq!(opacity_frac(1.4), 1.0);
        assert_eq!(opacity_frac(-0.2), 0.0);
        assert_eq!(opacity_frac(0.05), 0.05);
    }

    #[test]
    fn slider_set_writes_percent_grid_and_reports_only_on_release() {
        let mut core = EditorCore::new();
        core.status = "untouched".into();
        set_figure_opacity(&mut core, 0, 0.456, false);
        // Same whole-percent grid the stepper writes.
        assert_eq!(core.project.figures[0].opacity, 0.46);
        assert_eq!(core.status, "untouched", "mid-drag writes stay silent");
        set_figure_opacity(&mut core, 0, 0.5, true);
        assert_eq!(core.project.figures[0].opacity, 0.5);
        assert_eq!(core.status, "Figure 1 opacity 50%.");
        // The layers floor and ceiling apply on release too.
        set_figure_opacity(&mut core, 0, 0.0, true);
        assert_eq!(core.project.figures[0].opacity, 0.05);
        assert_eq!(core.status, "Figure 1 opacity 5%.");
        set_figure_opacity(&mut core, 0, 2.0, true);
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn slider_set_matches_the_stepper_ops_status_and_grid() {
        // The release report is the op's own string: drive
        // both write paths to the same value and compare.
        let mut via_op = EditorCore::new();
        via_op.step_figure_opacity(0, -0.5);
        let mut via_slider = EditorCore::new();
        set_figure_opacity(&mut via_slider, 0, 0.5, true);
        assert_eq!(
            via_slider.project.figures[0].opacity,
            via_op.project.figures[0].opacity
        );
        assert_eq!(via_slider.status, via_op.status);
    }

    #[test]
    fn slider_set_is_a_silent_no_op_for_a_missing_figure() {
        let mut core = EditorCore::new();
        core.status = "untouched".into();
        set_figure_opacity(&mut core, 9, 0.5, false);
        assert_eq!(core.status, "untouched");
        set_figure_opacity(&mut core, 9, 0.5, true);
        assert_eq!(core.status, "No such figure.");
        assert_eq!(core.project.figures.len(), 1);
    }

    #[test]
    fn slider_builds_at_the_track_ends() {
        let t = tok::theme(true);
        let _ = opacity_slider(0.0, &t);
        let _ = opacity_slider(0.05, &t);
        let _ = opacity_slider(1.0, &t);
    }
}
