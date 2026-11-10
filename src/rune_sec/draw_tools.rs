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
//!   shape they stamp — "square" / "circle" are the curated
//!   set's stand-ins for the Rect / Ellipse outlines (the
//!   set has no rectangle or ellipse glyph of its own),
//!   "draw-polygon" and "star" are exact. One button stays
//!   text-only because the curated icon set has no honest
//!   glyph for it: Mirror strokes (no flip/mirror glyph in
//!   the set — verified against `ICON_NAMES`, Wave 10).
//!   Wave 10 also corrected two glyphs whose earlier
//!   excuses had gone stale: the Arrow stamp claimed
//!   "no plain arrow glyph", but the set carries
//!   "arrow-right" (icon_data.rs) and the stamp at the
//!   default rotation points right, so it renders through
//!   `icon_btn` now; and the Line stamp's generic "shapes"
//!   cluster (a triangle, a square and a circle — none of
//!   them the straight segment the button stamps) became
//!   "minus", the single straight stroke the set offers
//!   (it has no diagonal-line glyph). The stamp parameter
//!   steppers stay text too — a − / value / + readout row
//!   reads better without glyphs.
//! * **Drawing readout (Phase I).** A muted line under the
//!   "Draw" header summarises the whole project's drawing —
//!   stroke and point counts plus the summed polyline length
//!   ([`drawing_stats`]) — with a spray count appended only
//!   when spray strokes exist.
//! * **Brush smoothing readout (Phase J).** Smoothing is a
//!   field of the brush *preset* (`paint::Brush::smoothing`,
//!   paint.rs:313 — the Chaikin iterations applied when a
//!   stroke is finished), not writable core state: the
//!   library is a fixed table that `paint::brush_library()`
//!   (paint.rs:335) constructs fresh on every call, the core
//!   stores only `brush_index` (editor.rs:73), and
//!   `EditorCore::active_brush` (editor.rs:3094) returns a
//!   copy out of a fresh library. A stepper here would write
//!   into a throwaway copy and change nothing, so the panel
//!   shows the active preset's smoothing as a readout
//!   ([`smoothing_readout`], key `draw-smooth-readout`)
//!   instead. Changing smoothing means choosing another
//!   preset — the brush row above does that (and the stroke
//!   inspector's Smoothing steppers work the same way, by
//!   stepping the selection through the library). The value
//!   is consulted by `EditorCore::end_draw` (editor.rs:2867)
//!   at the moment each stroke is finished, so it governs
//!   strokes drawn afterwards; strokes already on the canvas
//!   are never re-smoothed by a selection change (the Smooth
//!   button is the post-hoc path).
//! * **Brush width readout (Phase K).** Width has exactly
//!   the same shape as smoothing: a field of the brush
//!   *preset* (`paint::Brush::width`, paint.rs:307 — "base
//!   stroke width"), not writable core state — the core
//!   stores only `brush_index`, and `active_brush`
//!   (editor.rs:3094) returns a copy out of the freshly
//!   built library, so a width stepper would write into a
//!   throwaway copy and change nothing. The panel shows
//!   the active preset's width as a readout
//!   ([`width_readout`], key `draw-width-readout`) directly
//!   under the smoothing one; changing width means
//!   choosing another preset in the brush row. The value
//!   is real, not decorative: `begin_draw` seeds each new
//!   stroke's `width` and per-point `widths` from the
//!   active brush (editor.rs:2769), and the eraser's
//!   radius is twice it (editor.rs:3101). Every preset in
//!   the library carries a positive whole-number width —
//!   2.0 (Pencil, paint.rs:346) up to 20.0 (Broad Brush,
//!   paint.rs:430) — so [`width_label`]'s fractional and
//!   fallback arms exist only so a future preset can
//!   never make the readout print nonsense.
//! * **Brush scatter readout (Wave 10; corrected Wave 11).**
//!   Scatter has the same shape as smoothing and width — a
//!   field of the brush *preset* (`paint::Brush::scatter`,
//!   paint.rs:327) — so the panel shows it as a readout
//!   ([`scatter_readout`], key `draw-scatter-readout`) and
//!   changing it means choosing another preset (Spray, the
//!   only scattered one, radius 12). What the value DOES
//!   depends on the path that records the stroke, and the
//!   Wave 10 text here predates the wiring it asked for:
//!   a hand-drawn stroke with a scatter brush records a
//!   `SprayMark` carrying the radius (`EditorCore::begin_draw`,
//!   editor.rs:2791), which the renderers expand through
//!   `paint::spray_dots`; a stamp records the brush's
//!   scatter on the stroke itself (`Stroke::scatter`,
//!   written by [`push_stamp_stroke`] below), which the
//!   renderers expand through `paint::stroke_dabs`
//!   (paint.rs:646). MCP's `draw_stroke` takes scatter as
//!   an explicit argument (default 0) and never consults
//!   the active brush for it — so this readout describes
//!   what the PANEL's drawing and stamps will do, not
//!   what an MCP client gets by default (Wave 11 audit).
//! * **Panel card (Phase N).** The section is a kit panel
//!   card now, not a loose column of rows (P16: "a row
//!   pretending to be a panel"): the root is
//!   `design::panel_col` under the existing `draw-tools`
//!   key, the "Draw" header is the kit `panel_title`
//!   (pen-nib glyph; the active brush's name as meta) over
//!   a hairline divider, the Brushes / Shapes / Stamp
//!   blocks sit under kit `section_label`s, the brush
//!   presets are kit `chip_btn`s, and the stamp steppers
//!   follow the charter order (label, −, `stepper_value`,
//!   +). The smoothing and width lines stay readouts —
//!   the Phase J/K reasoning above is untouched. Keys and
//!   handlers are unchanged throughout.
//! * **Wave 9 budget pass.** Every row re-measured at real
//!   font metrics against the 276 pt content budget. The
//!   three `flex_wrap` rows (tools, brushes, shapes) fit
//!   by construction — their widest single control is
//!   117.9 pt, so wrapping reflows them at any width —
//!   but the unwrapped stamp-parameter rows did not: no
//!   pairing of the Sides / Size / Rot groups fits 276
//!   (294.5 / 289 / 281.5), so each group now has its own
//!   row (see the builder comment). The tools block also
//!   gained the kit `section_label` its sibling blocks
//!   already had ("Tools" — it was the one unlabelled
//!   group in the panel). Selected-brush visibility was
//!   audited, not changed: the active preset's chip
//!   renders in the accent fill against the inactive
//!   card fill, the panel header's meta names it, and
//!   the cycle button's label names it — three cues.
//! * **Wave 10 clamp pass.** The Sides and Size steppers
//!   rendered live at their clamps, where a press only
//!   re-reported the unchanged value as if it had
//!   stepped. They now follow the Wave 9 inert-at-clamp
//!   discipline (the playback / onion form): at the bound
//!   the button is the handler-free `btn_inert` under the
//!   same key ([`stamp_step_btn`] / [`stamp_step_is_noop`]),
//!   live again the moment the value leaves the bound.
//!   Rotation is exempt by construction — it wraps, so
//!   every press changes the value. The step handlers
//!   themselves are unchanged (they still clamp), the
//!   keys are unchanged, and the rows measure exactly as
//!   before (`btn_inert` is `btn`'s box).

use rune::tok::{type_, Theme};
use rune::{row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::paint::ShapeKind;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{chip_btn, icon_btn};
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

/// The active brush preset's smoothing, in Chaikin passes
/// (`paint::Brush::smoothing` via `EditorCore::active_brush`).
/// Read-only by construction — see the module docs for why
/// no setter exists.
pub fn active_brush_smoothing(core: &EditorCore) -> usize {
    core.active_brush().smoothing
}

/// Format a smoothing value as the readout's text: 0 is
/// "off", 1 is a single pass, anything higher counts its
/// passes. The argument is an `f32` because the shell's
/// brush-parameter machinery treats smoothing as one (the
/// stroke inspector's `Param::value` returns it as `f32`);
/// the value is rounded to whole passes and clamped at
/// zero, so a negative or non-finite input reads as off
/// rather than formatting nonsense.
pub fn smoothing_label(v: f32) -> String {
    let passes = v.round().max(0.0) as usize;
    match passes {
        0 => "Smoothing: off".to_string(),
        1 => "Smoothing: 1 pass".to_string(),
        n => format!("Smoothing: {n} passes"),
    }
}

/// The smoothing readout line for the core's active brush:
/// [`smoothing_label`] of [`active_brush_smoothing`].
pub fn smoothing_readout(core: &EditorCore) -> String {
    smoothing_label(active_brush_smoothing(core) as f32)
}

/// The active brush preset's base stroke width, in stage
/// units (`paint::Brush::width` via
/// `EditorCore::active_brush`). Read-only by construction
/// — see the module docs for why no setter exists.
pub fn active_brush_width(core: &EditorCore) -> f32 {
    core.active_brush().width
}

/// Format a brush width as the readout's text: whole
/// numbers print without decimals ("Width: 3 px") and a
/// fractional width keeps one decimal ("Width: 2.5 px").
/// Every preset in `paint::brush_library` carries a
/// positive whole-number width today, so the fractional
/// arm serves a future finer preset; the fallback —
/// "Width: —" for a non-finite or non-positive input —
/// exists so the readout can never print NaN or a
/// nonsense size. Neither arm is reachable from the
/// library as it stands.
pub fn width_label(w: f32) -> String {
    if !w.is_finite() || w <= 0.0 {
        return "Width: —".to_string();
    }
    if w.fract() == 0.0 {
        format!("Width: {w:.0} px")
    } else {
        format!("Width: {w:.1} px")
    }
}

/// The width readout line for the core's active brush:
/// [`width_label`] of [`active_brush_width`].
pub fn width_readout(core: &EditorCore) -> String {
    width_label(active_brush_width(core))
}

/// The active brush preset's scatter radius, in stage
/// units (`paint::Brush::scatter` via
/// `EditorCore::active_brush`). Read-only by construction,
/// exactly like smoothing and width: the PRESET's value is
/// a fixed library table entry, so there is nothing a
/// panel control could write. Where the value lands
/// depends on the recording path — `EditorCore::begin_draw`
/// copies it onto the stroke's `SprayMark` (and keeps the
/// stroke's own `scatter` at 0.0), while stamps record it
/// on the stroke itself (`Stroke::scatter`, see
/// [`push_stamp_stroke`]).
pub fn active_brush_scatter(core: &EditorCore) -> f32 {
    core.active_brush().scatter
}

/// Format a scatter radius as the readout's text: zero
/// (or negative, or NaN — the engine's own test in
/// `begin_draw` is `scatter > 0.0`, so all three mean the
/// brush lands its marks on the recorded path) is "off";
/// a positive radius counts stage units, whole numbers
/// without decimals and fractional radii with one, the
/// [`width_label`] convention. Every preset today is
/// whole (Spray's 12), so the fractional arm serves a
/// future finer preset.
pub fn scatter_label(v: f32) -> String {
    if !(v > 0.0) {
        return "Scatter: off".to_string();
    }
    if v.fract() == 0.0 {
        format!("Scatter: {v:.0} units")
    } else {
        format!("Scatter: {v:.1} units")
    }
}

/// The scatter readout line for the core's active brush:
/// [`scatter_label`] of [`active_brush_scatter`].
pub fn scatter_readout(core: &EditorCore) -> String {
    scatter_label(active_brush_scatter(core))
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
        // The editor's stamp records the active brush's opacity
        // (editor.rs `recorded_stroke_opacity`: NaN -> 1.0, else
        // clamped to [0, 1]); stamps are never spray strokes, so
        // no mark owns this stroke's alpha.
        opacity: if brush.opacity.is_nan() {
            1.0
        } else {
            brush.opacity.clamp(0.0, 1.0)
        },
        // Mirrors the editor's stamp: never a spray
        // stroke, so the brush's scatter lives on the
        // stroke itself.
        scatter: brush.scatter,
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

/// What the project's drawing holds, for the readout line:
/// the stroke and point counts and the summed polyline
/// length over **every** stroke in the project (all frames —
/// the line summarises the drawing as a whole, not just the
/// frame in view). `spray_strokes` counts the strokes that
/// carry a spray mark (`Stroke::spray`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawingStats {
    /// Strokes in the project.
    pub strokes: usize,
    /// Points across all strokes.
    pub points: usize,
    /// Summed polyline length, in stage units.
    pub total_length: f32,
    /// Strokes carrying a `SprayMark`.
    pub spray_strokes: usize,
}

/// Count [`DrawingStats`] over `core.project.strokes`. Pure
/// read — no snapshot, no status change. A stroke with fewer
/// than two points adds nothing to the length.
pub fn drawing_stats(core: &EditorCore) -> DrawingStats {
    let mut stats = DrawingStats {
        strokes: 0,
        points: 0,
        total_length: 0.0,
        spray_strokes: 0,
    };
    for stroke in &core.project.strokes {
        stats.strokes += 1;
        stats.points += stroke.points.len();
        if stroke.spray.is_some() {
            stats.spray_strokes += 1;
        }
        for pair in stroke.points.windows(2) {
            let dx = pair[1][0] - pair[0][0];
            let dy = pair[1][1] - pair[0][1];
            stats.total_length += (dx * dx + dy * dy).sqrt();
        }
    }
    stats
}

/// The readout line for [`drawing_stats`]: the counts and
/// the length rounded to whole stage units, with the spray
/// count appended only when spray strokes exist — a plain
/// drawing's line stays three figures long.
pub fn drawing_stats_line(stats: &DrawingStats) -> String {
    let mut line = format!(
        "{} strokes · {} points · {:.0} long",
        stats.strokes, stats.points, stats.total_length
    );
    if stats.spray_strokes > 0 {
        line += &format!(" · {} spray", stats.spray_strokes);
    }
    line
}

/// Whether stepping `value` by `delta` inside
/// `[min, max]` would write back the identical value — a
/// press that cannot change anything (the onion section's
/// `step_is_noop` predicate, generalised to this panel's
/// `i32` parameters). False for an out-of-range stored
/// value: either press pulls it back inside the range,
/// which is a real change.
pub fn stamp_step_is_noop(value: i32, delta: i32, min: i32, max: i32) -> bool {
    (value + delta).clamp(min, max) == value
}

/// One stamp stepper button: the live `btn` while the
/// step would change the parameter, the handler-free
/// `btn_inert` (same key, same box) when it can only
/// no-op at the clamp — the Wave 9 inert-at-clamp
/// discipline the playback and onion steppers already
/// follow, so a press at the limit cannot re-report the
/// unchanged value as if it had stepped. The Rotation
/// steppers never use this: rotation wraps, so every
/// press changes the value.
fn stamp_step_btn(
    key: &str,
    label: &str,
    noop: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    // Wave P: delegates to the kit's `design::step_btn`
    // (the inert-at-clamp split above is its Noop form).
    let form = if noop {
        design::StepForm::Noop
    } else {
        design::StepForm::Live
    };
    design::step_btn(key, label, form, t, f)
}

/// The Draw toolbar panel (Phase N: the kit card shell —
/// see the module docs): a header row holding the kit
/// title (the existing "Draw" text, the active brush's
/// name as meta) over a hairline divider, then the drawing
/// readout line and the control blocks under kit section
/// labels — Tools (mode, eraser, brush cycle, mirror,
/// simplify, smooth, clear), Brushes (one chip per preset,
/// the active one highlighted) with the active preset's
/// smoothing, width and scatter readouts under it, Shapes (Line /
/// Rect / Ellipse stamps plus the Polygon / Star / Arrow
/// stamps), and Stamp (the parameter steppers: Sides /
/// Size / Rotation). Active states read from the core's
/// own fields (`draw_mode`, `draw_tool_is_eraser`,
/// `brush_index` via [`active_brush_index`]); the stamp
/// parameters read from the panel-local [`DrawToolsState`].
pub fn draw_tools(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let active_brush = active_brush_index(core);

    let tools = row()
        .key("draw-tools-row")
        .gap(8.0)
        .center_y()
        // UI-mission overflow repair (audit s11, D-2):
        // the seven controls measure 690 pt against the
        // 316 pt column, leaving Simplify, Smooth and
        // Clear drawing past the window. Wrapping packs
        // them onto three lines (266 / 290 / Clear alone).
        .flex_wrap()
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
            // The stroke panel's press, not the bare core
            // op: it remaps `stroke_selected` over the
            // removed indices, so the selection cannot
            // silently land on a different live stroke
            // (Wave 9 integration, B5's flag — the header
            // Clear in stroke_inspector got the same fix).
            crate::rune_sec::stroke_inspector::clear_frame_strokes_press(s)
        }));

    // UI-mission overflow repair (audit s11, D-1): the
    // ten preset chips measure 879 pt against the
    // 316 pt column — six of them sat off-canvas behind
    // a hidden horizontal scroll. Wrapping packs them
    // onto three lines (299 / 277 / 287), all pressable
    // at rest; the `.scroll_x()` mitigation goes with
    // it (a scroll container does not wrap against its
    // own content width).
    let mut brushes = row()
        .key("draw-brushes")
        .gap(8.0)
        .center_y()
        .flex_wrap();
    for (i, name) in brush_names().iter().enumerate() {
        brushes = brushes.child(chip_btn(
            &brush_key(i),
            Some(brush_icon(name)),
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
        // UI-mission overflow repair (audit s11, D-3):
        // the six stamps measure 448 pt against the
        // 316 pt column — the Arrow stamp sat entirely
        // past the window. Wrapping packs them onto two
        // lines (313 / 127).
        .flex_wrap()
        .child(icon_btn("draw-shape-line", "minus", "Line", false, t, |s| {
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
        .child(icon_btn("stamp-arrow", "arrow-right", "Arrow", false, t, |s| {
            stamp_arrow_button(&mut s.core, &s.draw_tools)
        }));

    // Stamp parameter steppers (Phase F; Phase N: charter
    // order per group — label, −, kit `stepper_value`, + —
    // replacing the Phase F combined "Sides 5" readouts).
    // The labels are plain LABEL text rather than
    // growing: a grow on each would spread the groups
    // across the row instead of labelling them. Sides
    // drives the polygon and the star alike.
    // UI-mission overflow repair (audit s11, D-3): the
    // three groups together measure 436 pt against the
    // 316 pt column — the rotation + button sat entirely
    // past the window. They were split explicitly into
    // rows rather than wrapped, so a stepper group never
    // splits across lines mid-group.
    // Wave 9 budget re-measurement (rendered widths from
    // the headless tree dump, real font metrics): against
    // the stricter 276 pt content budget NO pairing of
    // the groups fits — Sides+Size 294.5, Sides+Rot 289,
    // Size+Rot 281.5 — so each group now sits on its own
    // row: Sides 147, Size 139.5, Rot 134, each with
    // more than 129 pt to spare. The `draw-stamp-params`
    // key stays on row 1 (Sides), `draw-stamp-params-2`
    // moves to the Size row, and Rot takes the additive
    // key `draw-stamp-params-3`. Every stepper key and
    // handler is unchanged.
    let params = &s.draw_tools;
    let stamp_params = row()
        .key("draw-stamp-params")
        .gap(8.0)
        .center_y()
        .child(text("Sides").font(type_::LABEL).color(t.text_secondary))
        .child(stamp_step_btn(
            "draw-sides-dn",
            "−",
            stamp_step_is_noop(params.sides, -1, SIDES_MIN, SIDES_MAX),
            t,
            |s| step_stamp_sides(&mut s.core, &mut s.draw_tools, -1),
        ))
        .child(design::stepper_value(&format!("{}", params.sides), t))
        .child(stamp_step_btn(
            "draw-sides-up",
            "+",
            stamp_step_is_noop(params.sides, 1, SIDES_MIN, SIDES_MAX),
            t,
            |s| step_stamp_sides(&mut s.core, &mut s.draw_tools, 1),
        ));
    let stamp_params_2 = row()
        .key("draw-stamp-params-2")
        .gap(8.0)
        .center_y()
        .child(text("Size").font(type_::LABEL).color(t.text_secondary))
        .child(stamp_step_btn(
            "draw-size-dn",
            "−",
            stamp_step_is_noop(params.size_pct, -SIZE_STEP_PCT, SIZE_MIN_PCT, SIZE_MAX_PCT),
            t,
            |s| step_stamp_size(&mut s.core, &mut s.draw_tools, -1),
        ))
        .child(design::stepper_value(&format!("{}%", params.size_pct), t))
        .child(stamp_step_btn(
            "draw-size-up",
            "+",
            stamp_step_is_noop(params.size_pct, SIZE_STEP_PCT, SIZE_MIN_PCT, SIZE_MAX_PCT),
            t,
            |s| step_stamp_size(&mut s.core, &mut s.draw_tools, 1),
        ));
    // The Rotation steppers keep the plain live `btn`:
    // rotation wraps (0° − 15° = 345°), so no press can
    // no-op and there is no clamp to go inert at.
    let stamp_params_3 = row()
        .key("draw-stamp-params-3")
        .gap(8.0)
        .center_y()
        .child(text("Rot").font(type_::LABEL).color(t.text_secondary))
        .child(btn("draw-rot-dn", "−", false, t, |s| {
            step_stamp_rotation(&mut s.core, &mut s.draw_tools, -1)
        }))
        .child(design::stepper_value(
            &format!("{}°", params.rotation_deg),
            t,
        ))
        .child(btn("draw-rot-up", "+", false, t, |s| {
            step_stamp_rotation(&mut s.core, &mut s.draw_tools, 1)
        }));

    design::panel_col("draw-tools", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("pen-nib"),
                    "Draw",
                    Some(core.active_brush().name),
                    t,
                )),
        )
        .child(design::divider(t))
        .child(
            text(drawing_stats_line(&drawing_stats(core)))
                .key("draw-stats")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(design::section_label("Tools", t))
        .child(tools)
        .child(design::section_label("Brushes", t))
        .child(brushes.w_full())
        .child(
            text(smoothing_readout(core))
                .key("draw-smooth-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(width_readout(core))
                .key("draw-width-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(scatter_readout(core))
                .key("draw-scatter-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(design::section_label("Shapes", t))
        .child(shapes)
        .child(design::section_label("Stamp", t))
        .child(stamp_params)
        .child(stamp_params_2)
        .child(stamp_params_3)
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
    fn stamp_step_is_noop_only_at_the_clamp_in_the_step_direction() {
        // Sides: at the minimum, stepping down cannot
        // change the value; stepping up can. Mid-range,
        // both directions move.
        assert!(stamp_step_is_noop(SIDES_MIN, -1, SIDES_MIN, SIDES_MAX));
        assert!(!stamp_step_is_noop(SIDES_MIN, 1, SIDES_MIN, SIDES_MAX));
        assert!(stamp_step_is_noop(SIDES_MAX, 1, SIDES_MIN, SIDES_MAX));
        assert!(!stamp_step_is_noop(SIDES_MAX, -1, SIDES_MIN, SIDES_MAX));
        assert!(!stamp_step_is_noop(5, -1, SIDES_MIN, SIDES_MAX));
        assert!(!stamp_step_is_noop(5, 1, SIDES_MIN, SIDES_MAX));
        // Size steps move in SIZE_STEP_PCT increments.
        assert!(stamp_step_is_noop(
            SIZE_MIN_PCT,
            -SIZE_STEP_PCT,
            SIZE_MIN_PCT,
            SIZE_MAX_PCT
        ));
        assert!(stamp_step_is_noop(
            SIZE_MAX_PCT,
            SIZE_STEP_PCT,
            SIZE_MIN_PCT,
            SIZE_MAX_PCT
        ));
        assert!(!stamp_step_is_noop(
            100,
            SIZE_STEP_PCT,
            SIZE_MIN_PCT,
            SIZE_MAX_PCT
        ));
        // An out-of-range stored value is pulled back
        // inside the range by either press — a real
        // change, so neither direction is a no-op (the
        // onion predicate's documented semantics).
        assert!(!stamp_step_is_noop(2, -1, SIDES_MIN, SIDES_MAX));
        assert!(!stamp_step_is_noop(2, 1, SIDES_MIN, SIDES_MAX));
    }

    #[test]
    fn panel_builds_with_stamp_params_at_their_clamps() {
        // Both inert branches of every clampable stepper
        // (and the live branches between them) build
        // without panicking — the inert form is the same
        // key in the same box, only handler-free.
        let mut state = crate::init();
        let t = rune::tok::theme(true);
        state.draw_tools = DrawToolsState {
            sides: SIDES_MIN,
            size_pct: SIZE_MIN_PCT,
            rotation_deg: 0,
        };
        let _panel = draw_tools(&state, &t);
        state.draw_tools = DrawToolsState {
            sides: SIDES_MAX,
            size_pct: SIZE_MAX_PCT,
            rotation_deg: 345,
        };
        let _panel = draw_tools(&state, &t);
    }

    // ---- Drawing readout (Phase I) ----

    #[test]
    fn drawing_stats_of_an_empty_project_are_all_zero() {
        let mut core = EditorCore::new();
        // The demo project starts stroke-free in practice;
        // clear anyway so the premise does not depend on it.
        core.project.strokes.clear();
        let stats = drawing_stats(&core);
        assert_eq!(
            stats,
            DrawingStats {
                strokes: 0,
                points: 0,
                total_length: 0.0,
                spray_strokes: 0,
            }
        );
        assert_eq!(drawing_stats_line(&stats), "0 strokes · 0 points · 0 long");
    }

    #[test]
    fn drawing_stats_count_strokes_points_and_polyline_length() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        // Draw one stroke through the core's own draw ops:
        // (0,0) -> (30,0) -> (30,40), two legs of 30 and 40.
        // The default Pen brush (smoothing 0, round nib)
        // leaves the recorded path exactly as drawn.
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.draw_to([30.0, 40.0]);
        core.end_draw();
        let stats = drawing_stats(&core);
        assert_eq!(stats.strokes, 1);
        assert_eq!(stats.points, 3);
        assert_eq!(stats.total_length, 70.0);
        assert_eq!(stats.spray_strokes, 0);
        // A stamped shape adds its own points and length:
        // the Line stamp runs the shape box diagonal,
        // (410,310) -> (550,410) on the demo stage.
        draw_shape_button(&mut core, ShapeKind::Line);
        let stats = drawing_stats(&core);
        assert_eq!(stats.strokes, 2);
        assert_eq!(stats.points, 5);
        let diagonal = (140.0f32.powi(2) + 100.0f32.powi(2)).sqrt();
        assert!((stats.total_length - (70.0 + diagonal)).abs() < 1e-3);
    }

    #[test]
    fn drawing_stats_count_spray_strokes_drawn_with_the_spray_brush() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        // The Spray preset (index 9, scatter 12) makes
        // begin_draw record a SprayMark on the stroke.
        select_brush(&mut core, 9);
        core.begin_draw([100.0, 100.0]);
        core.draw_to([140.0, 100.0]);
        core.end_draw();
        // A plain Pen stroke beside it stays unmarked.
        select_brush(&mut core, 0);
        core.begin_draw([0.0, 0.0]);
        core.draw_to([0.0, 50.0]);
        core.end_draw();
        let stats = drawing_stats(&core);
        assert_eq!(stats.strokes, 2);
        assert_eq!(stats.spray_strokes, 1);
        assert_eq!(stats.points, 4);
        assert_eq!(stats.total_length, 90.0);
        assert_eq!(
            drawing_stats_line(&stats),
            "2 strokes · 4 points · 90 long · 1 spray"
        );
    }

    #[test]
    fn drawing_stats_line_rounds_length_and_omits_an_empty_spray_count() {
        let plain = DrawingStats {
            strokes: 3,
            points: 12,
            total_length: 172.6,
            spray_strokes: 0,
        };
        assert_eq!(drawing_stats_line(&plain), "3 strokes · 12 points · 173 long");
        let sprayed = DrawingStats {
            spray_strokes: 2,
            ..plain
        };
        assert_eq!(
            drawing_stats_line(&sprayed),
            "3 strokes · 12 points · 173 long · 2 spray"
        );
    }

    // ---- Brush smoothing readout (Phase J) ----

    #[test]
    fn smoothing_label_formats_off_single_and_multiple_passes() {
        assert_eq!(smoothing_label(0.0), "Smoothing: off");
        assert_eq!(smoothing_label(1.0), "Smoothing: 1 pass");
        assert_eq!(smoothing_label(2.0), "Smoothing: 2 passes");
        assert_eq!(smoothing_label(3.0), "Smoothing: 3 passes");
        // Fractional inputs round to whole passes; negative
        // and non-finite inputs clamp to off.
        assert_eq!(smoothing_label(1.4), "Smoothing: 1 pass");
        assert_eq!(smoothing_label(-2.0), "Smoothing: off");
        assert_eq!(smoothing_label(f32::NAN), "Smoothing: off");
    }

    #[test]
    fn active_brush_smoothing_follows_the_selected_preset() {
        let mut core = EditorCore::new();
        // The default Pen preset records the path exactly.
        assert_eq!(active_brush_smoothing(&core), 0);
        assert_eq!(smoothing_readout(&core), "Smoothing: off");
        // Library order (paint::brush_library): Marker is
        // index 2 with 1 pass, Paint Brush index 5 with 2,
        // Broad Brush index 8 with the library maximum of 3,
        // and Spray (index 9) is back to none.
        select_brush(&mut core, 2);
        assert_eq!(active_brush_smoothing(&core), 1);
        assert_eq!(smoothing_readout(&core), "Smoothing: 1 pass");
        select_brush(&mut core, 5);
        assert_eq!(active_brush_smoothing(&core), 2);
        assert_eq!(smoothing_readout(&core), "Smoothing: 2 passes");
        select_brush(&mut core, 8);
        assert_eq!(active_brush_smoothing(&core), 3);
        assert_eq!(smoothing_readout(&core), "Smoothing: 3 passes");
        select_brush(&mut core, 9);
        assert_eq!(active_brush_smoothing(&core), 0);
    }

    #[test]
    fn every_library_preset_has_a_bounded_smoothing_value() {
        let lib = stykanimate_rs::paint::brush_library();
        assert_eq!(lib.len(), brush_names().len());
        let mut seen = std::collections::BTreeSet::new();
        for b in &lib {
            // The bound paint.rs's own tests assert: no
            // preset smooths more than 3 passes.
            assert!(b.smoothing <= 3, "{} smoothing is bounded", b.name);
            seen.insert(b.smoothing);
            assert!(!smoothing_label(b.smoothing as f32).is_empty());
        }
        // The library exercises every level the readout can
        // name, so each label arm is reachable from a preset.
        assert_eq!(seen, std::collections::BTreeSet::from([0, 1, 2, 3]));
    }

    #[test]
    fn panel_builds_for_every_brush_preset() {
        // The established shell-test fixture (command_palette
        // and audio_panel tests build states the same way):
        // init a real State and a real theme, then build the
        // section with each preset active in turn — and once
        // more with the eraser tool active, since the preset
        // also sizes the eraser.
        let mut state = crate::init();
        let t = rune::tok::theme(true);
        for i in 0..brush_names().len() {
            select_brush(&mut state.core, i);
            assert_eq!(
                smoothing_readout(&state.core),
                smoothing_label(active_brush_smoothing(&state.core) as f32)
            );
            assert_eq!(
                width_readout(&state.core),
                width_label(active_brush_width(&state.core))
            );
            assert_eq!(
                scatter_readout(&state.core),
                scatter_label(active_brush_scatter(&state.core))
            );
            let _panel = draw_tools(&state, &t);
        }
        state.core.toggle_draw_tool();
        assert!(state.core.draw_tool_is_eraser());
        let _panel = draw_tools(&state, &t);
    }

    // ---- Brush width readout (Phase K) ----

    #[test]
    fn width_label_formats_whole_and_fractional_widths() {
        // Whole numbers print without decimals.
        assert_eq!(width_label(3.0), "Width: 3 px");
        assert_eq!(width_label(20.0), "Width: 20 px");
        // A fractional width keeps one decimal. No preset
        // in the library is fractional today — this arm is
        // for a future finer preset.
        assert_eq!(width_label(2.5), "Width: 2.5 px");
        assert_eq!(width_label(0.5), "Width: 0.5 px");
        // A non-finite or non-positive width can never
        // come out of brush_library(), and the label
        // refuses to print one if it ever did.
        assert_eq!(width_label(0.0), "Width: —");
        assert_eq!(width_label(-4.0), "Width: —");
        assert_eq!(width_label(f32::NAN), "Width: —");
        assert_eq!(width_label(f32::INFINITY), "Width: —");
    }

    #[test]
    fn active_brush_width_follows_the_selected_preset() {
        let mut core = EditorCore::new();
        // The default Pen preset (paint.rs:337) is 4.0 wide.
        assert_eq!(active_brush_width(&core), 4.0);
        assert_eq!(width_readout(&core), "Width: 4 px");
        // Widths verified against the library table in
        // paint.rs: Pencil (index 1, paint.rs:346) is the
        // narrowest at 2.0, Paint Brush (index 5,
        // paint.rs:382) is 12.0, and Broad Brush (index 8,
        // paint.rs:430) is the widest at 20.0.
        select_brush(&mut core, 1);
        assert_eq!(active_brush_width(&core), 2.0);
        assert_eq!(width_readout(&core), "Width: 2 px");
        select_brush(&mut core, 5);
        assert_eq!(active_brush_width(&core), 12.0);
        assert_eq!(width_readout(&core), "Width: 12 px");
        select_brush(&mut core, 8);
        assert_eq!(active_brush_width(&core), 20.0);
        assert_eq!(width_readout(&core), "Width: 20 px");
    }

    #[test]
    fn every_library_preset_has_a_positive_whole_width() {
        // The readout's contract with the library: every
        // preset's width is finite, positive and whole, so
        // the label's fractional and fallback arms stay
        // unreachable from real presets (those arms are
        // pinned directly by the formatting test above).
        let lib = stykanimate_rs::paint::brush_library();
        assert_eq!(lib.len(), brush_names().len());
        for b in &lib {
            assert!(
                b.width.is_finite() && b.width > 0.0,
                "{} width is positive",
                b.name
            );
            assert_eq!(b.width.fract(), 0.0, "{} width is whole", b.name);
            assert_eq!(width_label(b.width), format!("Width: {:.0} px", b.width));
        }
    }

    // ---- Brush scatter readout (Wave 10) ----

    #[test]
    fn scatter_label_formats_off_and_positive_radii() {
        // Zero, negative and NaN are all "off" — the
        // engine's own consumer test in `begin_draw` is
        // `scatter > 0.0`, so the label's off condition
        // matches the behaviour exactly.
        assert_eq!(scatter_label(0.0), "Scatter: off");
        assert_eq!(scatter_label(-3.0), "Scatter: off");
        assert_eq!(scatter_label(f32::NAN), "Scatter: off");
        // Whole radii print without decimals (Spray's 12
        // is the library's only positive value today);
        // a fractional radius keeps one decimal, the
        // width label's convention.
        assert_eq!(scatter_label(12.0), "Scatter: 12 units");
        assert_eq!(scatter_label(2.5), "Scatter: 2.5 units");
    }

    #[test]
    fn active_brush_scatter_follows_the_selected_preset() {
        let mut core = EditorCore::new();
        // The default Pen preset lands marks on the path.
        assert_eq!(active_brush_scatter(&core), 0.0);
        assert_eq!(scatter_readout(&core), "Scatter: off");
        // Spray (index 9) is the library's only scattered
        // preset: radius 12, the value begin_draw copies
        // onto the stroke's SprayMark.
        select_brush(&mut core, 9);
        assert_eq!(active_brush_scatter(&core), 12.0);
        assert_eq!(scatter_readout(&core), "Scatter: 12 units");
        // Every other preset reads off — the readout can
        // never claim scatter the engine would not apply.
        for i in 0..9 {
            select_brush(&mut core, i);
            assert_eq!(active_brush_scatter(&core), 0.0, "preset {i} is unscattered");
            assert_eq!(scatter_readout(&core), "Scatter: off");
        }
    }

    #[test]
    fn stamp_records_the_active_brushs_scatter_and_opacity() {
        // Wave 11 parity pin: a stamp is never a spray
        // stroke, so the brush's scatter lives on the
        // stroke itself (`Stroke::scatter`, rendered via
        // paint::stroke_dabs) and its opacity is the
        // brush's, clamped. With Spray active (scatter
        // 12, opacity 0.5 — paint.rs:450) the stamped
        // stroke must carry both; this is the behaviour
        // the scatter readout's docs describe, and the
        // exact opposite of MCP draw_stroke's defaults
        // (scatter 0, opacity 1.0, active brush ignored).
        let mut core = EditorCore::new();
        select_brush(&mut core, 9);
        let params = DrawToolsState::default();
        stamp_polygon_button(&mut core, &params);
        let stroke = core.project.strokes.last().unwrap();
        assert!(stroke.spray.is_none(), "stamps never carry a spray mark");
        assert_eq!(stroke.scatter, 12.0);
        assert_eq!(stroke.opacity, 0.5);
        assert_eq!(stroke.width, 14.0, "the Spray preset's width");
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
