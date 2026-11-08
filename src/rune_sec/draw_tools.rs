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
//! * **Stamp parameters (Phase F).** Those three ops take no
//!   parameters — they hard-code 5 sides, radius 60 and a
//!   vertex-up start (editor.rs) — so the Sides / Size /
//!   Rotation steppers cannot flow into them. Instead the
//!   stamp buttons call the very generators the ops call
//!   (`paint::regular_polygon_points`, `paint::star_points`,
//!   `paint::arrow_points`) with the panel's parameters and
//!   stamp through the same stroke construction as the shape
//!   buttons. At the default parameters (5 sides, 100%, 0°)
//!   the generated points are identical to the ops' — a test
//!   stamps both ways and compares the strokes point for
//!   point. The parameters live in [`DrawToolsState`], a
//!   panel-local struct declared in this file.
//! * **Mirror.** There is no mirror-while-drawing toggle in the
//!   core; the old `mirror-strokes` button was the action
//!   `mirror_strokes_x` (mirror this frame's strokes across the
//!   stage centre), and that is what the button here calls.
//! * **Icons (Phase H).** The tool, brush and shape buttons
//!   render through `icon_btn` — a Font Awesome glyph plus
//!   the same text label as before, keys unchanged. Each
//!   glyph is chosen by what the button does (see
//!   [`brush_icon`] for the preset mapping): the Draw mode
//!   toggle paints strokes, so "paintbrush"; the eraser tool
//!   takes "eraser"; the shape stamps take the glyph of the
//!   shape they stamp. Two buttons stay text-only because
//!   the curated icon set has no honest glyph for them:
//!   Mirror strokes (no flip/mirror glyph) and the Arrow
//!   stamp (no plain arrow glyph). The stamp parameter
//!   steppers stay text too — a − / value / + readout row
//!   reads better without glyphs.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::paint::ShapeKind;

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::icon_btn;
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

/// The Font Awesome glyph for the brush preset named `name`
/// — the icon the preset's own button carries, and the one
/// the brush-cycle button shows for the preset it currently
/// names. Mapped by what the preset IS, from its name: the
/// Pen preset takes "pen" and the Pencil preset "pencil";
/// the markers and calligraphy presets are nib tools (the
/// FA "pen" glyph is a nib), so they take "pen" too; the
/// brushes and Spray apply paint broadly, so they take
/// "paintbrush" — which is also the fallback for any name
/// this mapping does not know.
pub fn brush_icon(name: &str) -> &'static str {
    match name {
        "Pen" | "Marker" | "Chisel Marker" | "Calligraphy" | "Calligraphy L" => "pen",
        "Pencil" => "pencil",
        _ => "paintbrush",
    }
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
    push_stamp_stroke(core, points, what);
}

/// Push a generated outline as one stroke on the current
/// frame — the exact construction the editor's private
/// `stamp_points` uses (one snapshot, the selected figure's
/// colour, the active brush's width recorded per point, no
/// fill), ending with its status string. Shared by the shape
/// buttons and the parameterized stamp buttons below.
pub fn push_stamp_stroke(core: &mut EditorCore, points: Vec<[f32; 2]>, what: &str) {
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

/// Sides stepper bounds: a polygon needs at least 3 sides;
/// 12 is a practical panel ceiling (a 12-gon already reads
/// as a near-circle at stamp scale).
pub const SIDES_MIN: i32 = 3;
pub const SIDES_MAX: i32 = 12;

/// Size stepper range and step, in percent of the core stamp
/// ops' built-in scale (radius 60, arrow shaft ±70, head 22).
pub const SIZE_MIN_PCT: i32 = 25;
pub const SIZE_MAX_PCT: i32 = 400;
pub const SIZE_STEP_PCT: i32 = 25;

/// Rotation stepper step, in degrees.
pub const ROT_STEP_DEG: i32 = 15;

/// The core stamp ops' built-in geometry (editor.rs
/// `stamp_polygon` / `stamp_star` / `stamp_arrow`), scaled by
/// the panel's size parameter.
const STAMP_RADIUS: f32 = 60.0;
const STAMP_STAR_INNER: f32 = 26.0;
const STAMP_ARROW_HALF: f32 = 70.0;
const STAMP_ARROW_HEAD: f32 = 22.0;

/// Panel-local stamp parameters, driven by the Sides / Size /
/// Rotation steppers and read by the Polygon / Star / Arrow
/// stamp buttons. The defaults (5, 100%, 0°) reproduce the
/// core ops' built-in stamps exactly. Declared here; the
/// shell's `State` gains a field of this type at integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawToolsState {
    /// Polygon sides / star points, [`SIDES_MIN`]..=[`SIDES_MAX`].
    pub sides: i32,
    /// Stamp scale in percent, [`SIZE_MIN_PCT`]..=[`SIZE_MAX_PCT`].
    pub size_pct: i32,
    /// Stamp rotation in degrees, wrapped to 0..360 in
    /// [`ROT_STEP_DEG`] steps.
    pub rotation_deg: i32,
}

impl Default for DrawToolsState {
    fn default() -> Self {
        DrawToolsState {
            sides: 5,
            size_pct: 100,
            rotation_deg: 0,
        }
    }
}

impl DrawToolsState {
    /// One Sides step (clamped at the bounds, so a press at
    /// the limit is a no-op rather than a wrap).
    pub fn step_sides(&mut self, delta: i32) {
        self.sides = (self.sides + delta).clamp(SIDES_MIN, SIDES_MAX);
    }

    /// One Size step (delta in [`SIZE_STEP_PCT`] steps,
    /// clamped at the bounds).
    pub fn step_size(&mut self, delta_steps: i32) {
        self.size_pct =
            (self.size_pct + delta_steps * SIZE_STEP_PCT).clamp(SIZE_MIN_PCT, SIZE_MAX_PCT);
    }

    /// One Rotation step (delta in [`ROT_STEP_DEG`] steps,
    /// wrapping around the circle: down from 0° lands on
    /// 345°).
    pub fn step_rotation(&mut self, delta_steps: i32) {
        self.rotation_deg = (self.rotation_deg + delta_steps * ROT_STEP_DEG).rem_euclid(360);
    }

    /// The size parameter as a multiplier of the ops'
    /// built-in scale.
    pub fn scale(&self) -> f32 {
        self.size_pct as f32 / 100.0
    }

    /// The paint generators' rotation argument: the ops start
    /// with one vertex up (-90°) and the panel's rotation
    /// turns from there.
    pub fn generator_rotation(&self) -> f32 {
        -90.0 + self.rotation_deg as f32
    }
}

/// Where every stamp lands: the stage centre (the ops'
/// convention).
fn stage_centre(stage: [f32; 2]) -> [f32; 2] {
    [stage[0] / 2.0, stage[1] / 2.0]
}

/// Rotate `p` around `centre` by `deg` degrees in the paint
/// generators' own convention (y down, so an increasing
/// angle runs clockwise on screen — point `[c + r·cos t,
/// c + r·sin t]`). Exact at 0°: cos/sin are 1/0, so the
/// point comes back unchanged bit for bit.
pub fn rotate_about(p: [f32; 2], centre: [f32; 2], deg: f32) -> [f32; 2] {
    let t = deg.to_radians();
    let (c, s) = (t.cos(), t.sin());
    let d = [p[0] - centre[0], p[1] - centre[1]];
    [
        centre[0] + d[0] * c - d[1] * s,
        centre[1] + d[0] * s + d[1] * c,
    ]
}

/// Polygon outline for the current parameters: the generator
/// the core's `stamp_polygon` calls, fed with the panel's
/// sides, scaled radius and rotation.
pub fn polygon_points_for(stage: [f32; 2], params: &DrawToolsState) -> Vec<[f32; 2]> {
    stykanimate_rs::paint::regular_polygon_points(
        stage_centre(stage),
        STAMP_RADIUS * params.scale(),
        params.sides as usize,
        params.generator_rotation(),
    )
}

/// Star outline for the current parameters: the generator
/// the core's `stamp_star` calls; the inner radius scales
/// with the outer, keeping the ops' 26:60 ratio.
pub fn star_points_for(stage: [f32; 2], params: &DrawToolsState) -> Vec<[f32; 2]> {
    stykanimate_rs::paint::star_points(
        stage_centre(stage),
        STAMP_RADIUS * params.scale(),
        STAMP_STAR_INNER * params.scale(),
        params.sides as usize,
        params.generator_rotation(),
    )
}

/// Arrow outline for the current parameters: the generator
/// the core's `stamp_arrow` calls. It takes no rotation, so
/// the shaft endpoints (centre ± the scaled half-length) are
/// rotated around the centre first, and the head length
/// scales with the shaft.
pub fn arrow_points_for(stage: [f32; 2], params: &DrawToolsState) -> Vec<[f32; 2]> {
    let centre = stage_centre(stage);
    let half = STAMP_ARROW_HALF * params.scale();
    let rot = params.rotation_deg as f32;
    let a = rotate_about([centre[0] - half, centre[1]], centre, rot);
    let b = rotate_about([centre[0] + half, centre[1]], centre, rot);
    stykanimate_rs::paint::arrow_points(a, b, STAMP_ARROW_HEAD * params.scale())
}

/// Handler body for the Polygon stamp button: the
/// parameterized outline, stamped like the ops stamp.
pub fn stamp_polygon_button(core: &mut EditorCore, params: &DrawToolsState) {
    let points = polygon_points_for(core.project.stage, params);
    push_stamp_stroke(core, points, &format!("a {}-sided polygon", params.sides));
}

/// Handler body for the Star stamp button.
pub fn stamp_star_button(core: &mut EditorCore, params: &DrawToolsState) {
    let points = star_points_for(core.project.stage, params);
    push_stamp_stroke(core, points, &format!("a {}-point star", params.sides));
}

/// Handler body for the Arrow stamp button.
pub fn stamp_arrow_button(core: &mut EditorCore, params: &DrawToolsState) {
    let points = arrow_points_for(core.project.stage, params);
    push_stamp_stroke(core, points, "an arrow");
}

/// Handler body for the Sides steppers: step the parameter
/// and report the new value on the status line.
pub fn step_stamp_sides(core: &mut EditorCore, params: &mut DrawToolsState, delta: i32) {
    params.step_sides(delta);
    core.status = format!("Stamp sides: {}.", params.sides);
}

/// Handler body for the Size steppers.
pub fn step_stamp_size(core: &mut EditorCore, params: &mut DrawToolsState, delta_steps: i32) {
    params.step_size(delta_steps);
    core.status = format!("Stamp size: {}%.", params.size_pct);
}

/// Handler body for the Rotation steppers.
pub fn step_stamp_rotation(core: &mut EditorCore, params: &mut DrawToolsState, delta_steps: i32) {
    params.step_rotation(delta_steps);
    core.status = format!("Stamp rotation: {}°.", params.rotation_deg);
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

/// The Draw toolbar section: a "Draw" label over four rows —
/// tools (mode, eraser, brush cycle, mirror, simplify, smooth,
/// clear), the brush library (one button per preset, the
/// active one highlighted), the shape tools (Line / Rect /
/// Ellipse stamps plus the Polygon / Star / Arrow stamps), and
/// the stamp parameter steppers (Sides / Size / Rotation).
/// Active states read from the core's own fields
/// (`draw_mode`, `draw_tool_is_eraser`, `brush_index` via
/// [`active_brush_index`]); the stamp parameters read from
/// the panel-local [`DrawToolsState`].
pub fn draw_tools(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let active_brush = active_brush_index(core);

    let tools = row()
        .key("draw-tools-row")
        .gap(8.0)
        .center_y()
        .child(icon_btn("draw-mode", "paintbrush", "Draw", core.draw_mode, t, |s| {
            // The old GPUI listener verbatim: flip the mode
            // field and say what the mode does.
            s.core.draw_mode = !s.core.draw_mode;
            s.core.status = if s.core.draw_mode {
                "Draw mode: drag on the canvas to paint a stroke on this frame.".into()
            } else {
                "Pose mode.".into()
            };
        }))
        .child(icon_btn(
            "draw-tool",
            "eraser",
            "Eraser",
            core.draw_tool_is_eraser(),
            t,
            |s| s.core.toggle_draw_tool(),
        ))
        .child(icon_btn(
            "brush-cycle",
            brush_icon(core.active_brush().name),
            &format!("Brush: {}", core.active_brush().name),
            false,
            t,
            |s| s.core.cycle_brush(),
        ))
        .child(btn("mirror-strokes", "Mirror strokes", false, t, |s| {
            s.core.mirror_strokes_x()
        }))
        .child(icon_btn("simplify-strokes", "wand-magic-sparkles", "Simplify", false, t, |s| {
            simplify_current_frame(&mut s.core);
        }))
        .child(icon_btn("smooth-strokes", "bezier-curve", "Smooth", false, t, |s| {
            smooth_current_frame(&mut s.core);
        }))
        .child(icon_btn("clear-drawing", "trash-can", "Clear drawing", false, t, |s| {
            s.core.clear_frame_drawings()
        }));

    let mut brushes = row().key("draw-brushes").gap(8.0).center_y();
    for (i, name) in brush_names().iter().enumerate() {
        brushes = brushes.child(icon_btn(
            &brush_key(i),
            brush_icon(name),
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
        .child(icon_btn("draw-shape-line", "shapes", "Line", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Line)
        }))
        .child(icon_btn("draw-shape-rect", "square", "Rect", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Rect)
        }))
        .child(icon_btn("draw-shape-ellipse", "circle", "Ellipse", false, t, |s| {
            draw_shape_button(&mut s.core, ShapeKind::Ellipse)
        }))
        .child(icon_btn("stamp-poly", "draw-polygon", "Polygon", false, t, |s| {
            stamp_polygon_button(&mut s.core, &s.draw_tools)
        }))
        .child(icon_btn("stamp-star", "star", "Star", false, t, |s| {
            stamp_star_button(&mut s.core, &s.draw_tools)
        }))
        .child(btn("stamp-arrow", "Arrow", false, t, |s| {
            stamp_arrow_button(&mut s.core, &s.draw_tools)
        }));

    // Stamp parameter steppers (Phase F): the values the
    // three stamp buttons above read, each with a readout
    // between its − / + buttons (the camera panel's zoom
    // idiom). Sides drives the polygon and the star alike.
    let params = &s.draw_tools;
    let readout = |label: String, t: &Theme| text(label).font(type_::LABEL).color(t.text_secondary);
    let stamp_params = row()
        .key("draw-stamp-params")
        .gap(8.0)
        .center_y()
        .child(btn("draw-sides-dn", "-", false, t, |s| {
            step_stamp_sides(&mut s.core, &mut s.draw_tools, -1)
        }))
        .child(readout(format!("Sides {}", params.sides), t))
        .child(btn("draw-sides-up", "+", false, t, |s| {
            step_stamp_sides(&mut s.core, &mut s.draw_tools, 1)
        }))
        .child(btn("draw-size-dn", "-", false, t, |s| {
            step_stamp_size(&mut s.core, &mut s.draw_tools, -1)
        }))
        .child(readout(format!("Size {}%", params.size_pct), t))
        .child(btn("draw-size-up", "+", false, t, |s| {
            step_stamp_size(&mut s.core, &mut s.draw_tools, 1)
        }))
        .child(btn("draw-rot-dn", "-", false, t, |s| {
            step_stamp_rotation(&mut s.core, &mut s.draw_tools, -1)
        }))
        .child(readout(format!("Rot {}°", params.rotation_deg), t))
        .child(btn("draw-rot-up", "+", false, t, |s| {
            step_stamp_rotation(&mut s.core, &mut s.draw_tools, 1)
        }));

    col()
        .key("draw-tools")
        .gap(6.0)
        .child(text("Draw").font(type_::LABEL).color(t.text_secondary))
        .child(tools)
        .child(brushes.scroll_x().w_full())
        .child(shapes)
        .child(stamp_params)
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

    // ---- Stamp parameters (Phase F) ----

    fn approx(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
    }

    #[test]
    fn default_stamp_params_reproduce_the_core_ops_point_for_point() {
        let params = DrawToolsState::default();
        assert_eq!((params.sides, params.size_pct, params.rotation_deg), (5, 100, 0));

        let mut panel = EditorCore::new();
        stamp_polygon_button(&mut panel, &params);
        let mut ops = EditorCore::new();
        ops.stamp_polygon();
        assert_eq!(
            panel.project.strokes.last().unwrap().points,
            ops.project.strokes.last().unwrap().points,
            "default polygon must equal stamp_polygon's pentagon"
        );

        let mut panel = EditorCore::new();
        stamp_star_button(&mut panel, &params);
        let mut ops = EditorCore::new();
        ops.stamp_star();
        assert_eq!(
            panel.project.strokes.last().unwrap().points,
            ops.project.strokes.last().unwrap().points,
            "default star must equal stamp_star's star"
        );

        let mut panel = EditorCore::new();
        stamp_arrow_button(&mut panel, &params);
        let mut ops = EditorCore::new();
        ops.stamp_arrow();
        assert_eq!(
            panel.project.strokes.last().unwrap().points,
            ops.project.strokes.last().unwrap().points,
            "default arrow must equal stamp_arrow's arrow"
        );
    }

    #[test]
    fn sides_stepper_clamps_between_3_and_12() {
        let mut p = DrawToolsState::default();
        for _ in 0..20 {
            p.step_sides(-1);
        }
        assert_eq!(p.sides, SIDES_MIN);
        for _ in 0..20 {
            p.step_sides(1);
        }
        assert_eq!(p.sides, SIDES_MAX);
        // The generators follow: a triangle outline has 3
        // vertices + the closing point; a 12-point star has
        // 24 alternating vertices + the closing point.
        p.sides = 3;
        assert_eq!(polygon_points_for([960.0, 720.0], &p).len(), 4);
        p.sides = 12;
        assert_eq!(polygon_points_for([960.0, 720.0], &p).len(), 13);
        assert_eq!(star_points_for([960.0, 720.0], &p).len(), 25);
    }

    #[test]
    fn size_stepper_clamps_and_scales_the_stamps() {
        let mut p = DrawToolsState::default();
        for _ in 0..20 {
            p.step_size(-1);
        }
        assert_eq!(p.size_pct, SIZE_MIN_PCT);
        for _ in 0..20 {
            p.step_size(1);
        }
        assert_eq!(p.size_pct, SIZE_MAX_PCT);

        let stage = [960.0, 720.0];
        let centre = [480.0, 360.0];
        p.size_pct = 200;
        let poly = polygon_points_for(stage, &p);
        assert!(
            (dist(poly[0], centre) - 120.0).abs() < 1e-3,
            "polygon radius doubles at 200%: {:?}",
            poly[0]
        );
        p.size_pct = 50;
        let star = star_points_for(stage, &p);
        assert!((dist(star[0], centre) - 30.0).abs() < 1e-3, "star outer scales");
        assert!((dist(star[1], centre) - 13.0).abs() < 1e-3, "star inner keeps the ratio");
        let arrow = arrow_points_for(stage, &p);
        assert!(
            (dist(arrow[0], arrow[1]) - 70.0).abs() < 1e-3,
            "arrow shaft halves at 50%"
        );
    }

    #[test]
    fn rotation_stepper_steps_15_degrees_and_wraps() {
        let mut p = DrawToolsState::default();
        p.step_rotation(-1);
        assert_eq!(p.rotation_deg, 345, "down from 0 wraps to 345");
        p.step_rotation(1);
        assert_eq!(p.rotation_deg, 0);
        for _ in 0..24 {
            p.step_rotation(1);
        }
        assert_eq!(p.rotation_deg, 0, "24 steps of 15° make a full turn");
    }

    #[test]
    fn rotate_about_zero_degrees_is_exact() {
        let p = [123.0, 456.0];
        assert_eq!(rotate_about(p, [480.0, 360.0], 0.0), p);
    }

    #[test]
    fn rotation_turns_the_stamped_points() {
        let stage = [960.0, 720.0];
        let mut p = DrawToolsState::default();
        let up = polygon_points_for(stage, &p);
        assert!(
            approx(up[0], [480.0, 300.0]),
            "first vertex points up by default: {:?}",
            up[0]
        );
        p.rotation_deg = 90;
        let right = polygon_points_for(stage, &p);
        assert!(
            approx(right[0], [540.0, 360.0]),
            "90° turns the first vertex to the right: {:?}",
            right[0]
        );
        // Rotation is rigid: every vertex keeps its radius.
        for v in &right {
            assert!((dist(*v, [480.0, 360.0]) - 60.0).abs() < 1e-3);
        }
        // The arrow's tip swings from +x to +y (down-screen).
        let arrow = arrow_points_for(stage, &p);
        assert!(
            approx(arrow[1], [480.0, 430.0]),
            "arrow tip points down at 90°: {:?}",
            arrow[1]
        );
    }

    #[test]
    fn stamp_buttons_append_one_stroke_each() {
        let mut core = EditorCore::new();
        let params = DrawToolsState {
            sides: 7,
            size_pct: 100,
            rotation_deg: 0,
        };
        let before = core.project.strokes.len();
        stamp_polygon_button(&mut core, &params);
        stamp_star_button(&mut core, &params);
        stamp_arrow_button(&mut core, &params);
        assert_eq!(core.project.strokes.len(), before + 3);
        let strokes = &core.project.strokes[before..];
        assert_eq!(strokes[0].points.len(), 8, "heptagon + closing point");
        assert_eq!(strokes[1].points.len(), 15, "7-point star + closing point");
        for s in strokes {
            assert_eq!(s.frame, core.current);
            assert_eq!(s.widths.len(), s.points.len());
        }
        assert!(core.status.starts_with("Stamped an arrow on frame"));
    }

    #[test]
    fn stepper_handlers_report_the_new_value() {
        let mut core = EditorCore::new();
        let mut params = DrawToolsState::default();
        step_stamp_sides(&mut core, &mut params, 1);
        assert_eq!(params.sides, 6);
        assert_eq!(core.status, "Stamp sides: 6.");
        step_stamp_size(&mut core, &mut params, 1);
        assert_eq!(core.status, "Stamp size: 125%.");
        step_stamp_rotation(&mut core, &mut params, -1);
        assert_eq!(core.status, "Stamp rotation: 345°.");
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
