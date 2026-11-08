//! Draw tools toolbar section for the Rune editor (Phase D,
//! section 11 — Draw & Build workspaces). Bin-crate code,
//! declared from `main.rs`; not part of the engine library.
//!
//! Every control calls a real `EditorCore` op or a `draw_ops`
//! function on the core's project — the same targets the GPUI
//! app's Draw and Strokes tool groups used. Element keys carry
//! the old GPUI control ids where one existed (`draw-mode`,
//! `clear-drawing`, `brush-cycle`, `draw-tool`, `stamp-poly`,
//! `stamp-star`, `stamp-arrow`, `mirror-strokes`,
//! `smooth-strokes`); new controls follow the same kebab style
//! (`brush-{i}`, `draw-shape-line`, `simplify-strokes`).
//!
//! Two honest mappings, where the engine's surface differs from
//! a naive reading of the old toolbar:
//!
//! * **Brush buttons.** The core exposes the whole preset list
//!   (`paint::brush_library`) but no set-brush op — only
//!   `cycle_brush`. `brush_index` is a public field, so a
//!   per-brush button writes it directly (the same sanctioned
//!   field-write pattern as the `draw_mode`/`onion` flips) and
//!   sets the exact status string `cycle_brush` sets. Picking a
//!   brush does not switch the draw tool back from the eraser:
//!   in the core the preset "applies to whichever tool is
//!   active" (it also sizes the eraser), so the buttons stay
//!   faithful to that.
//! * **Line / Rect / Ellipse.** `paint::ShapeKind` has exactly
//!   these three drag shapes and there is no EditorCore op for
//!   them (MCP's `draw_shape` builds the stroke itself). The
//!   buttons stamp the shape into a centred box on the current
//!   frame, with geometry from `paint::shape_points` and the
//!   stroke construction of the editor's own stamp helper
//!   (`EditorCore::stamp_points`: one snapshot, the selected
//!   figure's colour, the active brush's width recorded per
//!   point). Polygon / Star / Arrow need no such construction —
//!   the core's `stamp_polygon` / `stamp_star` / `stamp_arrow`
//!   ops back those buttons directly.
//! * **Mirror.** There is no mirror-while-drawing toggle in the
//!   core; the old `mirror-strokes` button was the action
//!   `mirror_strokes_x` (mirror this frame's strokes across the
//!   stage centre), and that is what the button here calls.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::paint::ShapeKind;

use crate::chrome_view::btn;
use crate::State;

/// Douglas-Peucker epsilon (stage units) for the Simplify
/// button. MCP's `simplify_strokes` takes epsilon as a required
/// argument, so the button needs a fixed, documented value:
/// hand-drawn strokes record points only when the pointer has
/// moved more than 2 units (`EditorCore::draw_to`), so an
/// epsilon of 2.0 removes sub-recording jitter without eating
/// deliberate corners at typical stroke scales.
pub const SIMPLIFY_EPSILON: f32 = 2.0;

/// Half-extents of the centred box the Line / Rect / Ellipse
/// buttons stamp into: 140 x 100 stage units around the stage
/// centre — the same visual scale as the core's stamp ops
/// (pentagon radius 60, arrow shaft ±70).
const SHAPE_HALF: [f32; 2] = [70.0, 50.0];

/// Element key for the per-brush button of library index `i`.
pub fn brush_key(i: usize) -> String {
    format!("brush-{i}")
}

/// The brush presets the core exposes, in library order
/// (`paint::brush_library` — Pen first, Spray last).
pub fn brush_names() -> Vec<&'static str> {
    stykanimate_rs::paint::brush_library()
        .iter()
        .map(|b| b.name)
        .collect()
}

/// The active brush's library index, clamped exactly as
/// `EditorCore::active_brush` clamps it, so a stale
/// `brush_index` can never highlight (or set) a brush that
/// does not exist.
pub fn active_brush_index(core: &EditorCore) -> usize {
    let len = stykanimate_rs::paint::brush_library().len();
    if len == 0 {
        return 0;
    }
    core.brush_index.min(len - 1)
}

/// Select brush preset `i`: writes the public `brush_index`
/// field (clamped) and sets the same status string
/// `EditorCore::cycle_brush` sets. See the module docs for why
/// this is a field write rather than an op call.
pub fn select_brush(core: &mut EditorCore, i: usize) {
    let lib = stykanimate_rs::paint::brush_library();
    if lib.is_empty() {
        return;
    }
    core.brush_index = i.min(lib.len() - 1);
    let b = lib[core.brush_index];
    core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
}

/// The centred drag box the shape buttons stamp into, as
/// `(from, to)` stage corners: the stage centre ±
/// [`SHAPE_HALF`], clamped into the stage so a stage smaller
/// than the box still yields a valid (smaller) box.
pub fn shape_box(stage: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let cx = stage[0] / 2.0;
    let cy = stage[1] / 2.0;
    let hx = SHAPE_HALF[0].min(cx);
    let hy = SHAPE_HALF[1].min(cy);
    ([cx - hx, cy - hy], [cx + hx, cy + hy])
}

/// Outline points for a shape button: `paint::shape_points`
/// over [`shape_box`] (Line: the box diagonal; Rect: 5 points,
/// closed; Ellipse: 25 points, closed).
pub fn shape_points_for(kind: ShapeKind, stage: [f32; 2]) -> Vec<[f32; 2]> {
    let (a, b) = shape_box(stage);
    stykanimate_rs::paint::shape_points(kind, a, b)
}

/// Handler body for the Line / Rect / Ellipse buttons: stamp
/// the shape as one stroke on the current frame, built exactly
/// like the editor's own stamps (`EditorCore::stamp_points`):
/// one snapshot, the selected figure's colour, the active
/// brush's width recorded for every point, no fill. Only the
/// geometry source differs — `paint::shape_points` instead of
/// a stamp generator. Sets the stamp-style status line.
pub fn draw_shape_button(core: &mut EditorCore, kind: ShapeKind) {
    let what = match kind {
        ShapeKind::Line => "a line",
        ShapeKind::Rect => "a rectangle",
        ShapeKind::Ellipse => "an ellipse",
    };
    let points = shape_points_for(kind, core.project.stage);
    let brush = core.active_brush();
    let color = core.project.figures[core.selected_figure].color;
    let n = points.len();
    core.snapshot();
    core.project.strokes.push(stykanimate_rs::model::Stroke {
        frame: core.current,
        points,
        color,
        width: brush.width,
        widths: vec![brush.width; n],
        fill: None,
        spray: None,
    });
    core.status = format!("Stamped {what} on frame {} ({n} points).", core.current + 1);
}

/// Handler body for the Simplify button — the MCP
/// `simplify_strokes` tool's exact sequence (snapshot, then
/// `draw_ops::simplify_strokes` on the current frame, then its
/// status string), with the epsilon fixed at
/// [`SIMPLIFY_EPSILON`]. Returns how many strokes were reduced.
pub fn simplify_current_frame(core: &mut EditorCore) -> usize {
    let frame = core.current;
    core.snapshot();
    let reduced =
        stykanimate_rs::draw_ops::simplify_strokes(&mut core.project, frame, SIMPLIFY_EPSILON);
    core.status = format!("Simplified strokes on frame {} ({reduced} reduced).", frame + 1);
    reduced
}

/// Handler body for the Smooth button — the old Strokes tool
/// group's listener verbatim: one Chaikin pass
/// (`draw_ops::smooth_frame` with subdiv 1) over the current
/// frame's strokes, then its status string. Returns how many
/// strokes were smoothed.
pub fn smooth_current_frame(core: &mut EditorCore) -> usize {
    let n = stykanimate_rs::draw_ops::smooth_frame(&mut core.project, core.current, 1);
    core.status = format!("Smoothed {n} stroke(s) on this frame.");
    n
}

/// The Draw toolbar section: a "Draw" label over three rows —
/// tools (mode, eraser, brush cycle, mirror, simplify, smooth,
/// clear), the brush library (one button per preset, the
/// active one highlighted), and the shape tools (Line / Rect /
/// Ellipse stamps plus the core's Polygon / Star / Arrow stamp
/// ops). Active states read from the core's own fields
/// (`draw_mode`, `draw_tool_is_eraser`, `brush_index` via
/// [`active_brush_index`]).
pub fn draw_tools(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let active_brush = active_brush_index(core);

    let tools = row()
        .key("draw-tools-row")
        .gap(8.0)
        .center_y()
        .child(btn("draw-mode", "Draw", core.draw_mode, t, |s| {
            // The old GPUI listener verbatim: flip the mode
            // field and say what the mode does.
            s.core.draw_mode = !s.core.draw_mode;
            s.core.status = if s.core.draw_mode {
                "Draw mode: drag on the canvas to paint a stroke on this frame.".into()
            } else {
                "Pose mode.".into()
            };
        }))
        .child(btn(
            "draw-tool",
            "Eraser",
            core.draw_tool_is_eraser(),
            t,
            |s| s.core.toggle_draw_tool(),
        ))
        .child(btn(
            "brush-cycle",
            &format!("Brush: {}", core.active_brush().name),
            false,
            t,
            |s| s.core.cycle_brush(),
        ))
        .child(btn("mirror-strokes", "Mirror strokes", false, t, |s| {
            s.core.mirror_strokes_x()
        }))
        .child(btn("simplify-strokes", "Simplify", false, t, |s| {
            simplify_current_frame(&mut s.core);
        }))
        .child(btn("smooth-strokes", "Smooth", false, t, |s| {
            smooth_current_frame(&mut s.core);
        }))
        .child(btn("clear-drawing", "Clear drawing", false, t, |s| {
            s.core.clear_frame_drawings()
        }));

    let mut brushes = row().key("draw-brushes").gap(8.0).center_y();
    for (i, name) in brush_names().iter().enumerate() {
        brushes = brushes.child(btn(
            &brush_key(i),
            *name,
            i == active_brush,
            t,
            move |s: &mut State| select_brush(&mut s.core, i),
        ));
    }

    let shapes = row()
        .key("draw-shapes")
        .gap(8.0)
        .center_y()
        .child(btn("draw-shape-line", "Line", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Line)
        }))
        .child(btn("draw-shape-rect", "Rect", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Rect)
        }))
        .child(btn("draw-shape-ellipse", "Ellipse", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Ellipse)
        }))
        .child(btn("stamp-poly", "Polygon", false, t, |s| {
            s.core.stamp_polygon()
        }))
        .child(btn("stamp-star", "Star", false, t, |s| s.core.stamp_star()))
        .child(btn("stamp-arrow", "Arrow", false, t, |s| {
            s.core.stamp_arrow()
        }));

    col()
        .key("draw-tools")
        .gap(6.0)
        .child(text("Draw").font(type_::LABEL).color(t.text_secondary))
        .child(tools)
        .child(brushes.scroll_x().w_full())
        .child(shapes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_names_match_the_core_library_order() {
        let names = brush_names();
        assert_eq!(names.len(), 10, "brush library size changed: {names:?}");
        assert_eq!(names[0], "Pen");
        assert_eq!(names[9], "Spray");
    }

    #[test]
    fn brush_keys_are_stable_and_indexed() {
        assert_eq!(brush_key(0), "brush-0");
        assert_eq!(brush_key(9), "brush-9");
    }

    #[test]
    fn active_brush_index_clamps_a_stale_index() {
        let mut core = EditorCore::new();
        core.brush_index = 999;
        assert_eq!(active_brush_index(&core), 9);
        core.brush_index = 3;
        assert_eq!(active_brush_index(&core), 3);
    }

    #[test]
    fn select_brush_sets_the_field_and_cycle_brushs_status() {
        let mut core = EditorCore::new();
        select_brush(&mut core, 2);
        assert_eq!(core.brush_index, 2);
        // Same wording EditorCore::cycle_brush produces.
        assert_eq!(core.status, "Brush: Marker (width 8).");
        assert_eq!(core.active_brush().name, "Marker");
        // Out-of-range selections clamp instead of panicking.
        select_brush(&mut core, 999);
        assert_eq!(core.brush_index, 9);
    }

    #[test]
    fn shape_box_is_centred_on_the_stage() {
        let (a, b) = shape_box([960.0, 720.0]);
        assert_eq!(a, [410.0, 310.0]);
        assert_eq!(b, [550.0, 410.0]);
        // A stage smaller than the box clamps to the full stage.
        let (a, b) = shape_box([100.0, 80.0]);
        assert_eq!(a, [0.0, 0.0]);
        assert_eq!(b, [100.0, 80.0]);
    }

    #[test]
    fn shape_points_have_the_paint_conventions() {
        let stage = [960.0, 720.0];
        let line = shape_points_for(ShapeKind::Line, stage);
        assert_eq!(line, vec![[410.0, 310.0], [550.0, 410.0]]);
        let rect = shape_points_for(ShapeKind::Rect, stage);
        assert_eq!(rect.len(), 5);
        assert_eq!(rect.first(), rect.last(), "rect outline is closed");
        let ellipse = shape_points_for(ShapeKind::Ellipse, stage);
        assert_eq!(ellipse.len(), 25);
        assert_eq!(ellipse.first(), ellipse.last(), "ellipse outline is closed");
    }

    #[test]
    fn draw_shape_button_stamps_one_stroke_on_the_current_frame() {
        let mut core = EditorCore::new();
        let before = core.project.strokes.len();
        draw_shape_button(&mut core, ShapeKind::Rect);
        assert_eq!(core.project.strokes.len(), before + 1);
        let stroke = core.project.strokes.last().unwrap();
        assert_eq!(stroke.frame, core.current);
        assert_eq!(stroke.points.len(), 5);
        // Built like a drawn stroke: the selected figure's
        // colour and a width recorded for every point.
        assert_eq!(stroke.color, core.project.figures[core.selected_figure].color);
        assert_eq!(stroke.widths.len(), stroke.points.len());
        assert!(core.status.starts_with("Stamped a rectangle on frame 1"));
    }

    #[test]
    fn simplify_and_smooth_handlers_are_safe_on_an_empty_frame() {
        let mut core = EditorCore::new();
        assert_eq!(simplify_current_frame(&mut core), 0);
        assert!(core.status.contains("Simplified strokes on frame 1 (0 reduced)"));
        assert_eq!(smooth_current_frame(&mut core), 0);
        assert!(core.status.contains("Smoothed 0 stroke(s) on this frame."));
    }
}
