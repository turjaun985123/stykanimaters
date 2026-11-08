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
//!   `EditorCore::move_figure_full(+1 / -1)` (editor.rs:1065).
//!   The "full" wrapper is used deliberately: the simpler
//!   `EditorCore::move_figure` (editor.rs:1623) only swaps
//!   `figures` + frame poses, while `move_figure_full` goes
//!   through `layers::move_figure` (layers.rs:149), which
//!   permutes poses and remaps camera-follow, particle, FX
//!   clip and graph-channel figure references identically,
//!   and the wrapper remaps the editor's selection and solo
//!   index. At the ends of the list the op is a safe no-op.
//! * Opacity − / + steppers →
//!   `EditorCore::step_figure_opacity(fi, ∓OPACITY_STEP)`
//!   (editor.rs:1177), which writes through
//!   `layers::set_opacity` (layers.rs:126) — clamped to
//!   `MIN_OPACITY..=MAX_OPACITY` (0.05..=1.0, layers.rs:34-36),
//!   so a figure can be faint but never invisible via opacity.
//!
//! Rows are listed in draw order with the front figure on
//! top (reverse index order). The selected figure's row name
//! renders active (accent), matching `core.selected_figure`.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
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

/// Move handler shared by the up/down buttons: select the
/// row's figure, then reorder it with the full layers engine.
/// `dir` is +1 toward the front, -1 toward the back.
fn move_layer(core: &mut EditorCore, fi: usize, dir: i32) {
    core.select_figure(fi);
    core.move_figure_full(dir);
}

/// One layer row: name (press selects; accent when selected),
/// opacity readout with −/+ steppers, and down/up reorder
/// buttons. Keys are per-figure: `layer-row-{fi}`,
/// `layer-op-dec-{fi}`, `layer-op-inc-{fi}`,
/// `layer-down-{fi}`, `layer-up-{fi}` (1-based in the key is
/// avoided — keys carry the engine index so headless `--tree`
/// checks map straight back to `Project::figures`).
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
}

/// The full layers panel: a "Layers" header, one row per
/// figure in draw order (front on top), and the ordering
/// note. With no figures the panel is just header + note.
pub fn panel(s: &State, t: &Theme) -> El {
    let mut body = col()
        .key("layers-panel")
        .gap(6.0)
        .child(text("Layers").font(type_::LABEL).color(t.text_secondary));
    for r in rows(&s.core) {
        body = body.child(layer_row(&r, t));
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
    fn opacity_stepper_op_clamps_through_the_layers_engine() {
        let mut core = EditorCore::new();
        core.step_figure_opacity(0, -OPACITY_STEP);
        assert_eq!(opacity_label(core.project.figures[0].opacity), "90%");
        // Stepping above 1.0 clamps to fully opaque.
        core.step_figure_opacity(0, 5.0);
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }
}
