//! Stage geometry and `Shape` building for the Rune editor shell
//! (Phase C of the GPUI→Rune migration). Declared from `main.rs`
//! only — this is bin-crate code, not part of the engine library.
//!
//! ## Coordinate design
//!
//! The stage is drawn in the canvas element's own layout points
//! (no `Shape::view_box`: view-box stretches coordinates but keeps
//! stroke widths in points, which would break thickness scaling).
//! One [`ViewTransform`] maps stage coordinates to element points
//! and back, using the convention both previous renderers used
//! (rasterizer `export::render_frame_at`'s view closure; GPUI
//! `paint_stage`/`to_stage`):
//!
//! * the camera key's `(x, y)` is the stage point shown at the
//!   centre of the view;
//! * `fit = min(el_w / stage_w, el_h / stage_h)` letterboxes the
//!   stage into the element, and `scale = fit * zoom`;
//! * `to_screen(p) = el_centre + (p - cam) * scale`;
//! * `to_stage(local) = cam + (local - el_centre) / scale` — the
//!   exact inverse, so a pointer point converts back through the
//!   same transform the drawing used.
//!
//! Drawing reads the element size from the previous commit's
//! layout (`Cx::layout_size("stage")` in `main.rs`); drag events
//! carry the element's `w`/`h` themselves, so both paths feed the
//! same function the same inputs. Phase E applies camera
//! *rotation* too, through `stage_camera::RotTransform` (the
//! rasterizer's own convention), and draws the full stage stack:
//! guides (optional), onion skins, drawing strokes, figures,
//! FX particles, text labels, weather — the export order.
//! Phase G adds the shell's selection overlays on top of that
//! stack, threaded through [`StageOverlays`]: the selected
//! stroke's accent highlight (in the stroke slot, under the
//! base strokes) and the selected particle event's anchor
//! gizmo (topmost), per-side onion depths in place of the
//! core's single `onion_frames` count, and independent
//! frustum / safe-frame guide toggles under the master
//! `guides` flag. Roadmap Wave 1 (RM18, P-6c) makes the
//! frustum family frustum-true under those same toggles:
//! the frustum toggle also draws the rule-of-thirds grid
//! INSIDE the framed view, and the safe toggle also draws
//! the framed view's own action/title-safe rects with
//! their labels — the stage-space grid and safe frames
//! remain, honestly describing the stage itself.
//! UI mission Wave 5 adds two things to the stack: a permanent
//! stage frame (the stage rect filled with the project
//! background and outlined in the theme's border token, drawn
//! first — audit S10 D1) and the shell view-navigation layer
//! (a zoom/pan composed over the camera transform, folded in
//! by [`display_transform`] / [`drag_transform`] — audit
//! S10 D4; see the "View navigation" section below).
//! Roadmap Wave 1 (RM16, roadmap P-4) adds the motion-path
//! overlay in the stack's tail: while `core.show_paths`
//! is on, the selected figure's evaluated root trajectory
//! (with a draggable point per frame, wired to
//! `EditorCore::nudge_path_point`) and the selected joint's
//! trajectory are drawn over the frame content — see the
//! "Motion-path overlay" section at [`motion_path_points`].

use std::cell::{Cell, RefCell};

use rune::{Color, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, V2};

/// Convert a `0xRRGGBB` engine colour to a Rune colour.
pub fn hex_color(rgb: u32) -> Option<Color> {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF))
}

/// The camera mapping between stage coordinates and canvas
/// element points (see the module docs for the convention).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    /// Element points per stage unit (`fit * zoom`); always > 0.
    pub scale: f32,
    /// Camera centre in stage coordinates.
    pub cam: [f32; 2],
    /// Canvas element size in points.
    pub size: [f32; 2],
}

/// Build the transform for an element of `el_size` points showing
/// `stage`-sized content under camera `(x, y, zoom)`. A degenerate
/// element or stage size falls back to a 1:1 fit so the scale can
/// never be zero (which would make [`to_stage`] divide by zero).
pub fn view_transform(el_size: [f32; 2], stage: [f32; 2], cam: (f32, f32, f32)) -> ViewTransform {
    let fit = if el_size[0] > 0.0 && el_size[1] > 0.0 && stage[0] > 0.0 && stage[1] > 0.0 {
        (el_size[0] / stage[0]).min(el_size[1] / stage[1])
    } else {
        1.0
    };
    ViewTransform {
        scale: fit * cam.2.max(0.01),
        cam: [cam.0, cam.1],
        size: el_size,
    }
}

/// Stage point → canvas element point.
pub fn to_screen(vt: &ViewTransform, p: V2) -> [f32; 2] {
    [
        vt.size[0] / 2.0 + (p[0] - vt.cam[0]) * vt.scale,
        vt.size[1] / 2.0 + (p[1] - vt.cam[1]) * vt.scale,
    ]
}

/// Canvas element point → stage point (exact inverse of
/// [`to_screen`]).
pub fn to_stage(vt: &ViewTransform, local: [f32; 2]) -> V2 {
    [
        vt.cam[0] + (local[0] - vt.size[0] / 2.0) / vt.scale,
        vt.cam[1] + (local[1] - vt.size[1] / 2.0) / vt.scale,
    ]
}

// ----------------------------------------------------------------
// View navigation (UI mission Wave 5, audit S10 D4)
// ----------------------------------------------------------------

/// Shell-only viewport navigation: a zoom multiplier and a pan
/// offset (in canvas element points) composed OVER the authored
/// camera transform, so a detail can be inspected without
/// editing the film. Until this layer existed, every zoom input
/// (the mouse wheel, the camera overlay's steppers) called
/// `EditorCore::camera_zoom_by`, which upserts a camera key and
/// takes an undo snapshot — scrolling the wheel rewrote the
/// animation's camera track.
///
/// The layer is a pure reparameterisation of the transform the
/// stage already uses, not a second transform. A screen-space
/// similarity (uniform `zoom` about the element centre, then a
/// `pan` translation) folds exactly into the camera parameters
/// of the same `RotTransform`:
///
/// * `zoom' = zoom · view.zoom`
/// * `cam'  = cam − R(θ) · view.pan / (view.zoom · scale)`
///
/// where `R(θ)` is the camera-rotation matrix and `scale` the
/// base transform's `fit · zoom`. Uniform scaling commutes with
/// rotation, and the pan (a screen-space vector) is un-rotated
/// into stage space by the transform's own matrix, so
/// [`display_transform`] (drawing) and [`drag_transform`]
/// (pointer conversion) remain exact inverses of each other —
/// the property drag correctness rests on — at any view state.
/// At the identity view the fold returns the base transform
/// unchanged, bit for bit.
///
/// The state lives in a `thread_local`: the shell's pattern for
/// view-state with no `State` field (the camera panel's
/// clipboard, the clips panel's selection, `main.rs`'s
/// `STAGE_EL_SIZE`). It is session-only — never persisted,
/// never written into the project — and no engine op sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewNav {
    /// View zoom multiplier over the authored camera zoom.
    pub zoom: f32,
    /// View pan in canvas element points: the displayed image
    /// shifts by exactly this vector.
    pub pan: [f32; 2],
}

impl ViewNav {
    /// The identity view: the camera's own framing, exactly.
    pub const IDENTITY: ViewNav = ViewNav { zoom: 1.0, pan: [0.0, 0.0] };

    /// Whether this view shows the camera's own framing.
    fn is_identity(self) -> bool {
        self.zoom == 1.0 && self.pan == [0.0, 0.0]
    }
}

/// View zoom bounds: wide enough to inspect one joint or to
/// take in the whole stage with margin, narrow enough that a
/// runaway wheel gesture cannot lose the picture entirely (the
/// overlay's Fit always restores the camera's framing).
pub const VIEW_ZOOM_MIN: f32 = 0.1;
/// See [`VIEW_ZOOM_MIN`].
pub const VIEW_ZOOM_MAX: f32 = 8.0;

thread_local! {
    static VIEW_NAV: Cell<ViewNav> = const { Cell::new(ViewNav::IDENTITY) };
    /// The stage element size the most recent [`stage_shape`]
    /// call drew at. The camera overlay's Centre action needs
    /// an element size to aim the view with, and the overlay
    /// builder itself receives no layout — the same idea as
    /// `main.rs`'s `STAGE_EL_SIZE`, kept where the drawing
    /// happens.
    static LAST_STAGE_SIZE: Cell<[f32; 2]> = const { Cell::new([0.0, 0.0]) };
}

/// The current shell view navigation state.
pub fn view_nav() -> ViewNav {
    VIEW_NAV.with(|c| c.get())
}

/// Replace the view navigation state (the zoom is clamped to
/// [`VIEW_ZOOM_MIN`]..=[`VIEW_ZOOM_MAX`]).
pub fn set_view_nav(nav: ViewNav) {
    let nav = ViewNav {
        zoom: nav.zoom.clamp(VIEW_ZOOM_MIN, VIEW_ZOOM_MAX),
        pan: nav.pan,
    };
    VIEW_NAV.with(|c| c.set(nav));
}

/// Restore the camera's own framing (the identity view).
pub fn reset_view_nav() {
    set_view_nav(ViewNav::IDENTITY);
}

/// Multiply the view zoom by `factor` about the element centre
/// (the pan is untouched, so the camera centre stays centred).
/// The camera overlay steppers' zoom; the wheel uses
/// [`view_zoom_at`] instead, which keeps the pointed-at stage
/// point under the cursor.
pub fn view_zoom_by(factor: f32) {
    let nav = view_nav();
    set_view_nav(ViewNav {
        zoom: nav.zoom * factor,
        pan: nav.pan,
    });
}

/// The effective zoom factor the stage is actually
/// displayed at: the authored camera zoom at the current
/// frame multiplied by the shell view zoom. This is the
/// single definition every readout must share — the camera
/// overlay's label already shows this product
/// (`camera_panel::view_zoom_pct` rounds it to a whole
/// percentage), while a camera-only figure understates the
/// magnification whenever the view is zoomed (the status
/// strip's pre-Wave-9 readout). Read-only: it writes
/// neither the view state nor the camera track.
pub fn view_zoom_factor(core: &EditorCore) -> f32 {
    core.camera_here().2 * view_nav().zoom
}

/// The stage element size the most recent [`stage_shape`] call
/// drew at (zero before the first frame).
pub fn last_stage_size() -> [f32; 2] {
    LAST_STAGE_SIZE.with(|c| c.get())
}

/// Fold the shell view navigation into a base camera transform
/// (see the section docs above for the algebra). An identity
/// view returns `base` unchanged.
pub fn with_view_nav(
    base: crate::rune_sec::stage_camera::RotTransform,
) -> crate::rune_sec::stage_camera::RotTransform {
    let nav = view_nav();
    if nav.is_identity() {
        return base;
    }
    let theta = base.angle_deg;
    let (sin, cos) = theta.to_radians().sin_cos();
    // R(θ) · pan: the stage_camera rotation matrix (its
    // `rotate_about`, for a free vector) applied to the pan.
    let rp = [
        nav.pan[0] * cos - nav.pan[1] * sin,
        nav.pan[0] * sin + nav.pan[1] * cos,
    ];
    let denom = nav.zoom * base.vt.scale;
    let vt = ViewTransform {
        scale: base.vt.scale * nav.zoom,
        cam: [
            base.vt.cam[0] - rp[0] / denom,
            base.vt.cam[1] - rp[1] / denom,
        ],
        size: base.vt.size,
    };
    crate::rune_sec::stage_camera::RotTransform::new(vt, theta)
}

// ----------------------------------------------------------------
// Stage feedback & precision (Roadmap Wave 1, RM15 — roadmap
// P-3; audit A2's R-A2-1…R-A2-7 cluster)
// ----------------------------------------------------------------
//
// The stage's transform core was already exact; what it lacked
// was FEEDBACK: picking that feels the same at every zoom,
// something under the pointer before the grab, numbers while
// dragging instead of only a recap after release, and any
// precision channel at all (snapping, nudge, coordinates).
// This section is that layer, built as pure functions and
// session state in this module:
//
// * screen-space picking — the pick radius and snap tolerance
//   are ELEMENT-POINT constants converted through the live
//   view scale ([`pick_radius`], [`snap_tolerance`]), so a
//   target is the same size on screen at 0.1× and at 8×
//   (R-A2-1);
// * segment picking — [`segment_hit`] over the same evaluated
//   positions the figure pass draws, so a long segment's
//   middle is grabbable, not dead stage (R-A2-4);
// * the hover model — [`hover_at`] classifies the point under
//   the pointer (joint > segment > sprite > stroke, the press
//   priority with segment slotted in), [`stage_cursor`] turns
//   hover + mode + drag state into the canvas's cursor shape,
//   and the drawing pass at the end of [`stage_shape`] paints
//   the hover highlight (R-A2-2);
// * the snap engine — angle steps for aim/IK drags, centre /
//   edge / ground / grid snapping for root drags, with the
//   marks a HUD needs to SAY a snap engaged (R-A2-5);
// * the drag HUD + pointer readout — [`drag_hud_lines`] reads
//   the live drag straight off the core (the solvers have
//   already written the pose the pointer asked for, so the
//   numbers are the pose's own), drawn beside the pointer by
//   the same final pass; with no drag in flight the pointer's
//   stage coordinates stand in the stage's bottom-left corner
//   (R-A2-3 + the coordinate half of R-A2-6);
// * keyboard nudge — [`nudge_selected`], one snapshot per
//   key-repeat burst, on the drag path's own write patterns
//   (R-A2-7).
//
// The pointer's element-local position reaches this module
// through the [`StagePointer`] probe — a `thread_local` in
// the `VIEW_NAV` pattern, fed by the shell (IA-RM15-1). Until
// the shell feeds it, the probe is empty, the final pass
// draws nothing, and the stage is byte-for-byte the stage
// this section found. The drag-path adoptions that live in
// frozen files (the core hit test's radius, `drag_to`'s snap,
// the key bindings) are filed as IA-RM15-2…IA-RM15-5 in the
// seat findings; everything here is written so those are
// one-line call-site swaps.

/// The stage picking radius in ELEMENT POINTS (R-A2-1): the
/// on-screen size of a joint target, converted to stage units
/// at the pointer path by [`pick_radius`]. 13 pt is the size
/// the old stage-unit constant produced at the fit view (one
/// stage unit ≈ one point there), so the fit view's feel is
/// unchanged — what changes is that zooming no longer shrinks
/// or inflates the target.
pub const PICK_RADIUS_PX: f32 = 13.0;

/// The snap engagement tolerance in ELEMENT POINTS (R-A2-5):
/// how close, ON SCREEN, a dragged point must come to a snap
/// target before the snap takes it.
pub const SNAP_TOLERANCE_PX: f32 = 8.0;

/// Stage units for a point-denominated `px` at view `scale`:
/// the one conversion every screen-space constant goes
/// through. A degenerate scale (no stage drawn yet) reads as
/// 1:1 rather than dividing by zero.
fn stage_units(px: f32, scale: f32) -> f32 {
    if scale > 0.0 {
        px / scale
    } else {
        px
    }
}

/// The joint/segment pick radius in stage units at view
/// `scale` — [`PICK_RADIUS_PX`] on screen, whatever the zoom.
pub fn pick_radius(scale: f32) -> f32 {
    stage_units(PICK_RADIUS_PX, scale)
}

/// The snap tolerance in stage units at view `scale`.
pub fn snap_tolerance(scale: f32) -> f32 {
    stage_units(SNAP_TOLERANCE_PX, scale)
}

/// The pointer's last known place over the stage canvas, in
/// the canvas element's own layout points, with the element
/// size it was measured against. Fed by the shell on pointer
/// move and through the drag phases (IA-RM15-1); read by
/// [`stage_shape`]'s feedback pass and by [`stage_cursor_now`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StagePointer {
    /// Element-local pointer position, layout points.
    pub local: [f32; 2],
    /// Canvas element size when the position was taken.
    pub size: [f32; 2],
}

thread_local! {
    /// The display scale [`stage_shape`] last drew at
    /// (element points per stage unit). Session consumers
    /// that only ever see stage points — the sprite drag's
    /// jitter gate is the first — read it to convert their
    /// screen-space constants, the `LAST_STAGE_SIZE`
    /// precedent ("the size the stage was last drawn at")
    /// applied to scale. 0 = never drawn.
    static LAST_DISPLAY_SCALE: Cell<f32> = const { Cell::new(0.0) };
    /// The pointer probe (see [`StagePointer`]).
    static STAGE_POINTER: Cell<Option<StagePointer>> = const { Cell::new(None) };
    /// The session snap configuration (see [`SnapConfig`]).
    static SNAP_CONFIG: Cell<SnapConfig> = const { Cell::new(SnapConfig::DEFAULT) };
    /// The last nudge's burst stamp (see [`nudge_selected`]).
    static NUDGE_STAMP: Cell<Option<NudgeStamp>> = const { Cell::new(None) };
}

/// The scale the stage last drew at; 1.0 before the first
/// draw, so a screen-space conversion made early is the
/// fit-view conversion rather than a divide by zero.
pub fn last_display_scale() -> f32 {
    let s = LAST_DISPLAY_SCALE.with(|c| c.get());
    if s > 0.0 {
        s
    } else {
        1.0
    }
}

/// Feed the pointer probe: the shell calls this on pointer
/// move and on every drag phase with the event's
/// element-local point and the element's size, and with
/// `None` when the pointer leaves the canvas.
pub fn set_stage_pointer(p: Option<StagePointer>) {
    STAGE_POINTER.with(|c| c.set(p));
}

/// The pointer probe as last fed (see [`set_stage_pointer`]).
pub fn stage_pointer() -> Option<StagePointer> {
    STAGE_POINTER.with(|c| c.get())
}

/// The evaluated joint positions the feedback layer reads:
/// the display frame's, setup-aware — the very positions the
/// figure pass of [`stage_shape`] draws, so a highlight sits
/// on the joint as drawn, not on a neighbouring frame's.
fn feedback_positions(core: &EditorCore, fi: usize) -> Vec<V2> {
    if core.setup_mode {
        core.setup_positions_f(core.display_frame(), fi)
    } else {
        core.positions_f(core.display_frame(), fi)
    }
}

/// Distance from `p` to the segment `a`–`b`, with the
/// projection parameter `t` (0 at `a`, 1 at `b`, clamped):
/// the segment-picking primitive. A degenerate segment
/// (`a == b`) measures to the point itself with `t = 0`.
pub fn dist_to_segment(p: V2, a: V2, b: V2) -> (f32, f32) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 <= 0.0 {
        return (model::dist(p, a), 0.0);
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    (model::dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t]), t)
}

/// The nearest joint to stage point `p` within `radius`
/// stage units, over every figure's drawn positions: the
/// core hit test's shape with the radius as a parameter, so
/// the caller converts it from screen points first
/// ([`pick_radius`]). Nearest wins; ties keep the earlier
/// figure/node, the hit test's own convention.
pub fn joint_hit(core: &EditorCore, p: V2, radius: f32) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for fi in 0..core.project.figures.len() {
        for (ni, joint) in feedback_positions(core, fi).iter().enumerate() {
            let d = model::dist(p, *joint);
            if d <= radius && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                best = Some((fi, ni, d));
            }
        }
    }
    best.map(|(fi, ni, _)| (fi, ni))
}

/// The nearest segment to stage point `p` within `radius`
/// stage units: `(figure, node, distance)` where `node` is
/// the segment's CHILD joint (the segment runs parent →
/// node, the indexing the bend channel uses). A stickman is
/// mostly segment; until now only its joints were pickable
/// (R-A2-4).
pub fn segment_hit(core: &EditorCore, p: V2, radius: f32) -> Option<(usize, usize, f32)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for fi in 0..core.project.figures.len() {
        let pos = feedback_positions(core, fi);
        for (ni, node) in core.project.figures[fi].def.nodes.iter().enumerate() {
            let Some(par) = node.parent else { continue };
            let (Some(&a), Some(&b)) = (pos.get(par), pos.get(ni)) else {
                continue;
            };
            let (d, _) = dist_to_segment(p, a, b);
            if d <= radius && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                best = Some((fi, ni, d));
            }
        }
    }
    best
}

/// What the stage pointer is over, in press priority order
/// (R-A2-2/R-A2-4): a joint beats the segment under it, a
/// segment beats the sprite/stroke layers beneath the
/// figures — the left press's own order (joint → sprite →
/// stroke) with segment slotted where a pose grab belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageHover {
    None,
    Joint(usize, usize),
    Segment(usize, usize),
    Sprite(usize),
    Stroke(usize),
}

/// Classify stage point `p` at view `scale` (see
/// [`StageHover`]): the pick radius is screen-space, so the
/// classification is the same at any zoom.
pub fn hover_at(core: &EditorCore, p: V2, scale: f32) -> StageHover {
    let radius = pick_radius(scale);
    if let Some((fi, ni)) = joint_hit(core, p, radius) {
        return StageHover::Joint(fi, ni);
    }
    if let Some((fi, ni, _)) = segment_hit(core, p, radius) {
        return StageHover::Segment(fi, ni);
    }
    if let Some(si) = crate::stage_sprites::sprite_pick_at(core, p) {
        return StageHover::Sprite(si);
    }
    if let Some(si) = crate::rune_sec::stage_layers::stroke_at_point(core, p, radius) {
        return StageHover::Stroke(si);
    }
    StageHover::None
}

/// The cursor shape the stage canvas wears for this core
/// state and hover (R-A2-2): the drag in flight decides
/// first (grabbing, or move for a camera pan), then the
/// modal tools (camera pans, draw crosses), then the hover
/// target (a pose grab over joints/segments, a move over
/// sprites, a pointer over strokes), else the arrow. Pure:
/// the shell asks per commit and sets the element's cursor
/// (IA-RM15-1).
pub fn stage_cursor(core: &EditorCore, hover: StageHover) -> rune::Cursor {
    use stykanimate_rs::editor::DragKind;
    match core.dragging {
        Some(DragKind::Camera) => return rune::Cursor::Move,
        Some(_) => return rune::Cursor::Grabbing,
        None => {}
    }
    if core.camera_mode {
        return rune::Cursor::Move;
    }
    if core.draw_mode {
        return rune::Cursor::Crosshair;
    }
    match hover {
        StageHover::Joint(..) | StageHover::Segment(..) => rune::Cursor::Grab,
        StageHover::Sprite(_) => rune::Cursor::Move,
        StageHover::Stroke(_) => rune::Cursor::Pointer,
        StageHover::None => rune::Cursor::Default,
    }
}

/// [`stage_cursor`] with the hover classified from the
/// pointer probe: the one call the canvas mount needs
/// (IA-RM15-1). No probe (pointer never seen / already left)
/// classifies as [`StageHover::None`].
pub fn stage_cursor_now(core: &EditorCore, el_size: [f32; 2]) -> rune::Cursor {
    let hover = match stage_pointer() {
        Some(ptr) => {
            let rt = drag_transform(core, el_size);
            hover_at(core, rt.to_stage(ptr.local), rt.vt.scale)
        }
        None => StageHover::None,
    };
    stage_cursor(core, hover)
}

/// The session's snap configuration (R-A2-5): session-only,
/// like the view navigation — a posing aid, not project
/// data, so it takes no undo step and saves nowhere. The
/// panel/settings surface that will one day own these
/// switches writes through [`set_snap_config`]; until then
/// the defaults below are the behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapConfig {
    /// Master switch for every snap in [`snap_drag_point`].
    pub enabled: bool,
    /// Angle snap step in degrees for aim/IK drags (15° —
    /// the Pivot/Stick Nodes convention).
    pub angle_step: f32,
    /// Grid spacing in stage units for root drags, when a
    /// grid is configured; `None` = no grid snap.
    pub grid: Option<f32>,
}

impl SnapConfig {
    pub const DEFAULT: SnapConfig = SnapConfig {
        enabled: true,
        angle_step: 15.0,
        grid: None,
    };
}

/// The session snap configuration (see [`SnapConfig`]).
pub fn snap_config() -> SnapConfig {
    SNAP_CONFIG.with(|c| c.get())
}

/// Replace the session snap configuration.
pub fn set_snap_config(cfg: SnapConfig) {
    SNAP_CONFIG.with(|c| c.set(cfg));
}

/// `angle` rounded to the nearest multiple of `step`
/// degrees; a non-positive step is no snap at all.
pub fn snap_angle_deg(angle: f32, step: f32) -> f32 {
    if step <= 0.0 {
        angle
    } else {
        (angle / step).round() * step
    }
}

/// The aim snap: `p` rotated about `pivot` onto the nearest
/// `step`-degree ray, at `p`'s own distance — the dragged
/// joint keeps its reach and takes the snapped direction,
/// which is exactly what the aim solver will then pose. A
/// point on the pivot has no direction and passes through.
pub fn snap_aim_point(pivot: V2, p: V2, step: f32) -> V2 {
    let d = [p[0] - pivot[0], p[1] - pivot[1]];
    let r = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if r <= 0.0 {
        return p;
    }
    let snapped = snap_angle_deg(d[1].atan2(d[0]).to_degrees(), step).to_radians();
    [pivot[0] + r * snapped.cos(), pivot[1] + r * snapped.sin()]
}

/// Which root-snap targets took a coordinate (R-A2-5): the
/// HUD's vocabulary for saying a snap engaged, and the snap
/// pins' assertion surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RootSnap {
    pub centre_x: bool,
    pub centre_y: bool,
    pub edge: bool,
    pub ground: bool,
    pub grid: bool,
}

/// Snap a root-drag point to the stage's own landmarks:
/// per coordinate, the nearest of {edge, centre, opposite
/// edge} within `tol` stage units wins over the grid (a
/// landmark is a stronger statement than a grid line); the
/// bottom edge is the ground line a pose stands on. With no
/// grid configured and nothing in tolerance, the point
/// passes through untouched.
pub fn snap_root_point(p: V2, stage: [f32; 2], tol: f32, grid: Option<f32>) -> (V2, RootSnap) {
    let mut out = p;
    let mut marks = RootSnap::default();
    let (mut grid_x, mut grid_y) = (false, false);
    if let Some(sp) = grid {
        if sp > 0.0 {
            let gx = (p[0] / sp).round() * sp;
            grid_x = gx != out[0];
            out[0] = gx;
            let gy = (p[1] / sp).round() * sp;
            grid_y = gy != out[1];
            out[1] = gy;
        }
    }
    if stage[0] > 0.0 {
        for (target, is_centre) in [(0.0, false), (stage[0] / 2.0, true), (stage[0], false)] {
            if (out[0] - target).abs() <= tol {
                out[0] = target;
                if is_centre {
                    marks.centre_x = true;
                } else {
                    marks.edge = true;
                }
                grid_x = false;
                break;
            }
        }
    }
    if stage[1] > 0.0 {
        for (target, kind) in [(0.0, 0u8), (stage[1] / 2.0, 1u8), (stage[1], 2u8)] {
            if (out[1] - target).abs() <= tol {
                out[1] = target;
                match kind {
                    1 => marks.centre_y = true,
                    2 => marks.ground = true,
                    _ => marks.edge = true,
                }
                grid_y = false;
                break;
            }
        }
    }
    marks.grid = grid_x || grid_y;
    (out, marks)
}

/// What a snapped drag point engaged (see [`snap_drag_point`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SnapMarks {
    /// An aim/IK point was rotated onto a step ray.
    pub angle: bool,
    /// A root point's landmark/grid engagements.
    pub root: RootSnap,
}

/// Snap the stage point a drag is about to write, per the
/// drag in flight on `core` and the session [`SnapConfig`]:
/// roots take the landmark snap, aim drags snap about the
/// parent joint, IK drags about the chain root (the
/// grandparent joint — the ray the chain reaches along).
/// Camera drags and no-drag pass through. This is the whole
/// drag-path adoption surface: the shell snaps the converted
/// pointer point through here before handing it to the core
/// (IA-RM15-3), so the core's solvers stay snap-free and the
/// undo discipline is untouched.
pub fn snap_drag_point(core: &EditorCore, p: V2, scale: f32) -> (V2, SnapMarks) {
    use stykanimate_rs::editor::DragKind;
    let marks = SnapMarks::default();
    let cfg = snap_config();
    if !cfg.enabled {
        return (p, marks);
    }
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get(fi) else {
        return (p, marks);
    };
    match core.dragging {
        Some(DragKind::Root) => {
            let (q, m) = snap_root_point(p, core.project.stage, snap_tolerance(scale), cfg.grid);
            (q, SnapMarks { angle: false, root: m })
        }
        Some(DragKind::Aim(ni)) | Some(DragKind::SetupAim(ni)) => {
            let pos = feedback_positions(core, fi);
            let pivot = fig
                .def
                .nodes
                .get(ni)
                .and_then(|n| n.parent)
                .and_then(|par| pos.get(par).copied())
                .unwrap_or(p);
            let q = snap_aim_point(pivot, p, cfg.angle_step);
            (q, SnapMarks { angle: q != p, root: RootSnap::default() })
        }
        Some(DragKind::Ik(ni)) | Some(DragKind::SetupIk(ni)) => {
            let pos = feedback_positions(core, fi);
            let pivot = fig
                .def
                .nodes
                .get(ni)
                .and_then(|n| n.parent)
                .and_then(|par| fig.def.nodes.get(par))
                .and_then(|par| par.parent)
                .and_then(|gp| pos.get(gp).copied())
                .unwrap_or(p);
            let q = snap_aim_point(pivot, p, cfg.angle_step);
            (q, SnapMarks { angle: q != p, root: RootSnap::default() })
        }
        _ => (p, marks),
    }
}

/// A segment's direction in degrees, stage convention
/// (y down, `atan2` range −180..=180): the HUD's angle.
fn seg_angle_deg(a: V2, b: V2) -> f32 {
    (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees()
}

/// The angle of node `ni`'s segment on figure `fi`, read off
/// the drawn positions — the number the aim/IK solvers are
/// posing, not a re-derivation from stored angles.
fn aim_angle_deg(core: &EditorCore, fi: usize, ni: usize) -> Option<f32> {
    let fig = core.project.figures.get(fi)?;
    let par = fig.def.nodes.get(ni)?.parent?;
    let pos = feedback_positions(core, fi);
    Some(seg_angle_deg(*pos.get(par)?, *pos.get(ni)?))
}

/// An IK chain's reach as a percentage of its fully
/// extended length (the two segment lengths summed): the
/// HUD's "how straight is this limb" number. `None` when
/// the chain or its lengths are degenerate.
fn ik_reach_pct(core: &EditorCore, fi: usize, ni: usize) -> Option<f32> {
    let fig = core.project.figures.get(fi)?;
    let mid = fig.def.nodes.get(ni)?.parent?;
    let root_i = fig.def.nodes.get(mid)?.parent?;
    let pos = feedback_positions(core, fi);
    let d = model::dist(*pos.get(root_i)?, *pos.get(ni)?);
    let max = fig.def.nodes[mid].length + fig.def.nodes[ni].length;
    if max <= 0.0 {
        return None;
    }
    Some((d / max * 100.0).clamp(0.0, 100.0))
}

/// The live drag HUD's lines (R-A2-3), read off the core
/// while a pose/camera drag is in flight: the drag has
/// already written the pose the pointer asked for, so these
/// are the pose's own live numbers — root coordinates for a
/// root drag, the grabbed segment's angle for an aim, reach
/// for an IK grab, the camera key for a camera pan, the
/// `REST` twins in setup mode. Empty when no core drag is
/// in flight (the sprite drag's line is
/// `stage_sprites::sprite_drag_hud`'s, and the drawing pass
/// concatenates the two). Formatting follows the numeric
/// idiom RM3's kit set for readouts: fixed decimals per
/// quantity (stage coordinates and degrees whole, zoom two
/// places), and every string is plain ASCII capitals,
/// digits and `.,-%` — the canvas text stamp's charset.
pub fn drag_hud_lines(core: &EditorCore) -> Vec<String> {
    use stykanimate_rs::editor::DragKind;
    let Some(kind) = core.dragging else {
        return Vec::new();
    };
    let fi = core.selected_figure;
    if core.project.figures.get(fi).is_none() {
        return Vec::new();
    }
    match kind {
        DragKind::Root => {
            let root = core
                .project
                .frames
                .get(core.current)
                .and_then(|f| f.poses.get(fi))
                .map(|pose| pose.root)
                .unwrap_or([0.0, 0.0]);
            vec![format!("ROOT {:.0}, {:.0}", root[0], root[1])]
        }
        DragKind::Aim(ni) => match aim_angle_deg(core, fi, ni) {
            Some(deg) => vec![format!("ANGLE {:.0} DEG", deg)],
            None => Vec::new(),
        },
        DragKind::Ik(ni) => match ik_reach_pct(core, fi, ni) {
            Some(pct) => vec![format!("IK REACH {:.0}%", pct)],
            None => Vec::new(),
        },
        DragKind::Camera => {
            let (x, y, z) = core.camera_here();
            vec![format!("CAM {:.0}, {:.0}", x, y), format!("ZOOM {:.2}", z)]
        }
        DragKind::SetupAim(ni) => match aim_angle_deg(core, fi, ni) {
            Some(deg) => vec![format!("REST ANGLE {:.0} DEG", deg)],
            None => Vec::new(),
        },
        DragKind::SetupIk(ni) => match ik_reach_pct(core, fi, ni) {
            Some(pct) => vec![format!("REST IK REACH {:.0}%", pct)],
            None => Vec::new(),
        },
    }
}

/// The pointer's stage coordinates as the corner readout
/// shows them: whole stage units, `x, y`.
pub fn pointer_readout(p: V2) -> String {
    format!("{:.0}, {:.0}", p[0], p[1])
}

/// The keyboard nudge steps in stage units (R-A2-7): plain
/// arrow = 1, Shift = 10, Alt = 0.1 — [`nudge_step`] picks.
pub const NUDGE_STEP: f32 = 1.0;
pub const NUDGE_STEP_LARGE: f32 = 10.0;
pub const NUDGE_STEP_FINE: f32 = 0.1;

/// The nudge step for a key press's modifiers (Alt beats
/// Shift: the finer override wins when both are held).
pub fn nudge_step(shift: bool, alt: bool) -> f32 {
    if alt {
        NUDGE_STEP_FINE
    } else if shift {
        NUDGE_STEP_LARGE
    } else {
        NUDGE_STEP
    }
}

/// One nudge burst's stamp: nudges against the same
/// selection, frame and mode within
/// [`NUDGE_BURST_WINDOW_MS`] of the last one share its
/// snapshot, so a held arrow key (OS key-repeat) is ONE
/// undo step, not thirty — the gesture-coalescing rule the
/// drag path gets from its grab snapshot, restated for keys.
#[derive(Clone, Copy)]
struct NudgeStamp {
    at: std::time::Instant,
    fi: usize,
    ni: Option<usize>,
    frame: usize,
    setup: bool,
}

/// The burst window for [`nudge_selected`]'s snapshot
/// coalescing, in milliseconds.
pub const NUDGE_BURST_WINDOW_MS: u128 = 750;

/// Forget the nudge burst stamp: the next nudge snapshots
/// afresh. The shell calls this when the selection or frame
/// changes under it (the stamp also keys on both, so this
/// is belt-and-braces); tests use it to separate bursts.
pub fn reset_nudge_burst() {
    NUDGE_STAMP.with(|c| c.set(None));
}

/// Snapshot for a nudge, coalesced per burst (see
/// [`NudgeStamp`]): the first press of a burst takes the
/// gesture's one snapshot; repeats inside the window ride
/// it.
fn nudge_snapshot(core: &mut EditorCore, fi: usize, ni: Option<usize>, setup: bool) {
    let now = std::time::Instant::now();
    let reuse = NUDGE_STAMP.with(|c| c.get()).map_or(false, |st| {
        st.fi == fi
            && st.ni == ni
            && st.frame == core.current
            && st.setup == setup
            && now.duration_since(st.at).as_millis() <= NUDGE_BURST_WINDOW_MS
    });
    if !reuse {
        core.snapshot();
    }
    NUDGE_STAMP.with(|c| {
        c.set(Some(NudgeStamp {
            at: now,
            fi,
            ni,
            frame: core.current,
            setup,
        }))
    });
}

/// Nudge the selected joint by `(dx, dy)` stage units
/// (R-A2-7): the keyboard twin of a stage drag, on the drag
/// path's own write patterns — a root joint's pose root
/// moves verbatim; any other joint is aimed at its current
/// position plus the delta through `model::aim_node` (so
/// joint limits clamp exactly as a drag's would); in setup
/// mode the same aim writes clamped REST angles through a
/// scratch pose, and a root there refuses with
/// `begin_setup_drag`'s own sentence, because a rest pose
/// has no root to move. One snapshot per key-repeat burst
/// (see [`NudgeStamp`]). Refuses — silently, as a press
/// during playback does — while playing, with no joint
/// selected, or on a stale selection; a locked figure
/// refuses with the drag path's own status sentence.
/// Returns whether a nudge was written. The key binding
/// itself is the shell's (IA-RM15-4).
pub fn nudge_selected(core: &mut EditorCore, dx: f32, dy: f32) -> bool {
    if core.playing || (dx == 0.0 && dy == 0.0) {
        return false;
    }
    let fi = core.selected_figure;
    let Some(ni) = core.selected_node else {
        return false;
    };
    let Some(fig) = core.project.figures.get(fi) else {
        return false;
    };
    if fig.locked {
        core.status = "Figure is locked — unlock it in the figures list.".into();
        return false;
    }
    if core.setup_mode {
        if fig.def.nodes[ni].parent.is_none() {
            core.status = "The rest pose has no root position — roots live in frame poses.".into();
            return false;
        }
        let pos = core.setup_positions(fi);
        let Some(&joint) = pos.get(ni) else {
            return false;
        };
        let target = [joint[0] + dx, joint[1] + dy];
        nudge_snapshot(core, fi, Some(ni), true);
        let root = core.project.frames[core.current].poses[fi].root;
        let mut scratch = core.project.figures[fi].def.rest_pose(root);
        model::aim_node(&core.project.figures[fi].def, &mut scratch, ni, target);
        let fig = &mut core.project.figures[fi];
        for (node, angle) in fig.def.nodes.iter_mut().zip(scratch.angles.iter()) {
            node.rest_angle = node.clamp_angle(*angle);
        }
        core.status = format!("Nudged rest joint {ni} by ({dx:.1}, {dy:.1}).");
        return true;
    }
    let frame = core.current;
    if fig.def.nodes[ni].parent.is_none() {
        nudge_snapshot(core, fi, Some(ni), false);
        let pose = &mut core.project.frames[frame].poses[fi];
        pose.root = [pose.root[0] + dx, pose.root[1] + dy];
        core.status = format!(
            "Nudged figure {} root to ({:.0}, {:.0}).",
            fi + 1,
            pose.root[0],
            pose.root[1]
        );
        return true;
    }
    let pos = core.positions(frame, fi);
    let Some(&joint) = pos.get(ni) else {
        return false;
    };
    let target = [joint[0] + dx, joint[1] + dy];
    nudge_snapshot(core, fi, Some(ni), false);
    let def = &core.project.figures[fi].def;
    let pose = &mut core.project.frames[frame].poses[fi];
    model::aim_node(def, pose, ni, target);
    core.status = format!("Nudged figure {} joint {} by ({dx:.1}, {dy:.1}).", fi + 1, ni);
    true
}

/// The stage's feedback pass (RM15), drawn LAST in
/// [`stage_shape`] — over figures, FX, text and weather,
/// with the selection gizmos: these are editor markers, and
/// a marker the frame content could cover would fail at its
/// job. Reads only the pointer probe and the core, so with
/// the probe empty (the shell has not fed it yet, or the
/// pointer has left) it draws nothing at all.
///
/// * Hover (no drag in flight): the hovered joint gets an
///   enlarged accent octagon, the hovered segment a
///   translucent accent wash, the hovered sprite a
///   full-strength outline, the hovered stroke a white glow
///   along its points — the target announces itself BEFORE
///   the grab (R-A2-2).
/// * A drag in flight: the HUD — a theme-card plate beside
///   the pointer carrying [`drag_hud_lines`] (plus the
///   sprite drag's line) in the canvas text stamp (R-A2-3).
/// * Otherwise: the pointer's stage coordinates in the
///   stage's bottom-left corner (the readout half of
///   R-A2-6's measurement furniture).
fn draw_stage_feedback(
    mut shape: Shape,
    core: &EditorCore,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    el_size: [f32; 2],
    s: f32,
) -> Shape {
    let Some(ptr) = stage_pointer() else {
        return shape;
    };
    if el_size[0] <= 0.0 || el_size[1] <= 0.0 {
        return shape;
    }
    let stage_p = rt.to_stage(ptr.local);
    let accent = hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
    let dragging = core.dragging.is_some() || crate::stage_sprites::sprite_drag_active();
    if !dragging {
        match hover_at(core, stage_p, s) {
            StageHover::Joint(fi, ni) => {
                if let Some(&joint) = feedback_positions(core, fi).get(ni) {
                    shape = shape.fill(
                        ngon(rt.to_screen(joint), 11.0 * s.max(0.6), 8),
                        accent.alpha(0.40),
                    );
                }
            }
            StageHover::Segment(fi, ni) => {
                let pos = feedback_positions(core, fi);
                if let (Some(&a), Some(&b)) = (
                    core.project.figures[fi]
                        .def
                        .nodes
                        .get(ni)
                        .and_then(|n| n.parent)
                        .and_then(|par| pos.get(par)),
                    pos.get(ni),
                ) {
                    shape = shape.line(rt.to_screen(a), rt.to_screen(b), 5.0, LineCap::Round, accent.alpha(0.30));
                }
            }
            StageHover::Sprite(si) => {
                let quads = crate::stage_sprites::sprite_quads(&core.project, core.display_frame());
                if let Some(q) = quads.iter().find(|q| q.sprite == si) {
                    let pts: Vec<[f32; 2]> = q.corners.iter().map(|p| rt.to_screen(*p)).collect();
                    shape = shape.stroke_with(pts, true, 1.5, LineCap::Round, accent.alpha(0.95));
                }
            }
            StageHover::Stroke(si) => {
                if let Some(stroke) = core.project.strokes.get(si) {
                    for w in stroke.points.windows(2) {
                        shape = shape.line(
                            rt.to_screen(w[0]),
                            rt.to_screen(w[1]),
                            3.0,
                            LineCap::Round,
                            Color::WHITE.alpha(0.45),
                        );
                    }
                }
            }
            StageHover::None => {}
        }
    }
    // HUD text geometry: glyph height ≈ 11 element points,
    // expressed in stage units through the live scale; the
    // stamp's cell is a fifth of the glyph height and each
    // character advances four cells (stage_text's own
    // layout constants, restated for the width estimate).
    let glyph = 11.0 / s.max(0.05);
    let cell = glyph / 5.0;
    let text_w = |line: &str| (line.chars().count() as f32 * 4.0 - 1.0).max(0.0) * cell;
    let ink: u32 = if core.light_theme { 0x14161A } else { 0xF2F4F8 };
    let mut lines = drag_hud_lines(core);
    if let Some(line) = crate::stage_sprites::sprite_drag_hud(core) {
        lines.push(line);
    }
    if !lines.is_empty() {
        let theme = crate::rune_sec::app_theme::app_theme(!core.light_theme);
        let line_h = glyph * 1.5;
        let max_w = lines.iter().map(|l| text_w(l)).fold(0.0f32, f32::max);
        let pad = glyph * 0.55;
        // The plate's top-left in stage coordinates: the
        // stage point under a screen offset from the
        // pointer, so the plate rides beside the cursor at
        // any zoom or camera rotation.
        let anchor = rt.to_stage([ptr.local[0] + 16.0, ptr.local[1] + 18.0]);
        let total_h = line_h * lines.len() as f32 - (line_h - glyph);
        let corners = [
            [anchor[0] - pad, anchor[1] - pad],
            [anchor[0] + max_w + pad, anchor[1] - pad],
            [anchor[0] + max_w + pad, anchor[1] + total_h + pad],
            [anchor[0] - pad, anchor[1] + total_h + pad],
        ];
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen.clone(), theme.card.alpha(0.88));
        shape = shape.stroke_with(screen, true, 1.0, LineCap::Round, theme.card_border.alpha(0.9));
        for (i, line) in lines.iter().enumerate() {
            let centre = [
                anchor[0] + text_w(line) / 2.0,
                anchor[1] + glyph / 2.0 + line_h * i as f32,
            ];
            for tp in crate::rune_sec::stage_text::label_boxes_alpha(centre, 0.0, line, glyph, ink, 0.95) {
                let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = tp;
                let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
            }
        }
        return shape;
    }
    if !dragging {
        // The coordinate readout, bottom-left of the
        // ELEMENT (screen-anchored through the inverse
        // transform, like the HUD plate).
        let size = 10.0 / s.max(0.05);
        let cell = size / 5.0;
        let line = pointer_readout(stage_p);
        let w = (line.chars().count() as f32 * 4.0 - 1.0).max(0.0) * cell;
        let base = rt.to_stage([10.0, el_size[1] - 10.0]);
        let centre = [base[0] + w / 2.0, base[1] - size / 2.0];
        for tp in crate::rune_sec::stage_text::label_boxes_alpha(centre, 0.0, &line, size, ink, 0.80) {
            let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = tp;
            let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
            shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
        }
    }
    shape
}

/// The transform the stage DRAWS with: the camera at the
/// display frame (as `RotTransform::for_display`) with the
/// shell view navigation folded in.
pub fn display_transform(
    core: &EditorCore,
    el_size: [f32; 2],
) -> crate::rune_sec::stage_camera::RotTransform {
    with_view_nav(crate::rune_sec::stage_camera::RotTransform::for_display(
        core, el_size,
    ))
}

/// The transform stage POINTER conversion uses: the camera at
/// the current frame (as `RotTransform::for_drag`) with the
/// shell view navigation folded in. Drawing and dragging fold
/// the same view state into the same transform family, so a
/// drag still lands on the grabbed joint at any view zoom.
pub fn drag_transform(
    core: &EditorCore,
    el_size: [f32; 2],
) -> crate::rune_sec::stage_camera::RotTransform {
    with_view_nav(crate::rune_sec::stage_camera::RotTransform::for_drag(
        core, el_size,
    ))
}

/// Wheel zoom: multiply the view zoom by `factor`, keeping the
/// stage point under `cursor` (element points) under the
/// cursor — zoom-to-cursor, so inspecting a detail does not
/// also slide it away. With no laid-out element size there is
/// no cursor geometry to honour, so the zoom falls back to the
/// element centre (pan unchanged).
pub fn view_zoom_at(core: &EditorCore, el_size: [f32; 2], cursor: [f32; 2], factor: f32) {
    let nav = view_nav();
    let new_zoom = (nav.zoom * factor).clamp(VIEW_ZOOM_MIN, VIEW_ZOOM_MAX);
    if new_zoom == nav.zoom || el_size[0] <= 0.0 || el_size[1] <= 0.0 {
        set_view_nav(ViewNav {
            zoom: new_zoom,
            pan: nav.pan,
        });
        return;
    }
    // The stage point under the cursor, through the composed
    // transform the user is actually looking at (display
    // frame), and where the base camera transform puts that
    // point on screen: the new pan must satisfy
    //   centre + new_zoom * (base(anchor) − centre) + pan = cursor.
    let anchor = display_transform(core, el_size).to_stage(cursor);
    let base = crate::rune_sec::stage_camera::RotTransform::for_display(core, el_size);
    let b = base.to_screen(anchor);
    let centre = [el_size[0] / 2.0, el_size[1] / 2.0];
    set_view_nav(ViewNav {
        zoom: new_zoom,
        pan: [
            cursor[0] - centre[0] - new_zoom * (b[0] - centre[0]),
            cursor[1] - centre[1] - new_zoom * (b[1] - centre[1]),
        ],
    });
}

/// Pan the view so `stage_point` sits at the element centre
/// (the camera overlay's Centre action: centre the VIEW on the
/// selected figure's root, where `centre_on_selection` would
/// aim the authored CAMERA). Returns `false` — view untouched —
/// when no stage has been laid out yet, so the caller can fall
/// back to clearing the pan.
pub fn view_centre_on(core: &EditorCore, stage_point: [f32; 2]) -> bool {
    let size = last_stage_size();
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return false;
    }
    let base = crate::rune_sec::stage_camera::RotTransform::for_display(core, size);
    let b = base.to_screen(stage_point);
    let centre = [size[0] / 2.0, size[1] / 2.0];
    let nav = view_nav();
    set_view_nav(ViewNav {
        zoom: nav.zoom,
        pan: [
            nav.zoom * (centre[0] - b[0]),
            nav.zoom * (centre[1] - b[1]),
        ],
    });
    true
}

/// Points of a regular `n`-gon of radius `r` around `centre`
/// (first vertex at the top). Used for joint handles (octagons)
/// and the circle/polygon shape nodes, since `Shape` has no
/// circle primitive.
pub fn ngon(centre: [f32; 2], r: f32, n: usize) -> Vec<[f32; 2]> {
    let n = n.max(3);
    (0..n)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / n as f32 - std::f32::consts::FRAC_PI_2;
            [centre[0] + r * a.cos(), centre[1] + r * a.sin()]
        })
        .collect()
}

/// Onion-skin piece sets around the current frame, mirroring
/// the GPUI stage's colours and falloff: previous frames keep
/// each figure's own colour (alpha `0x4D` for the nearest,
/// `0x26` for older), next frames share one green tint
/// (`0x7BE495`) with the same falloff. Returns `(pieces,
/// side base colour, ghost alpha)` entries in draw order; the
/// base colour is the hex every piece's near colour fell back
/// to (a node's own colour wins over it, per piece).
///
/// Wave 12 (SPEC-ONION): the skins are built from the same
/// `paint::styled_segment_pieces` the main figure pass uses,
/// so a ghost now carries its nodes' taper, gradient and
/// style opacity instead of stroking every segment at one
/// flat 5.0 width — the named behaviour change: ghost width
/// is the per-node thickness at the node's own style. Two
/// standing decisions hold: the bend is always 0.0 (ghosts
/// stay unbended — the pose at a neighbouring frame is the
/// point of the skin, and the rasterizer draws no ghosts to
/// mirror), and the ghost alpha multiplies each piece's own
/// alpha at draw time rather than replacing it.
///
/// The depths are per-side shell state (Phase G,
/// `State::onion_ui`), not the core's single `onion_frames`
/// count: `prev` frames behind and `next` frames ahead are
/// drawn, and a count of 0 draws nothing on that side. The
/// falloff is a function of the step distance only (step 1 is
/// always `0x4D`, steps ≥ 2 always `0x26`), so widening one
/// side never changes the other side's skins.
fn onion_skins(
    core: &EditorCore,
    prev: u8,
    next: u8,
) -> Vec<(Vec<stykanimate_rs::paint::StylePiece>, u32, f32)> {
    let mut out = Vec::new();
    if !core.onion {
        return out;
    }
    let project = &core.project;
    let pieces_at = |frame: usize, fi: usize, base: u32| -> Vec<stykanimate_rs::paint::StylePiece> {
        // Wave 10: a figure need not appear in every frame
        // (its poses slot is simply absent — the engine's
        // presence idiom, `fi < frame.poses.len()`). The
        // onion pass walks neighbouring frames for EVERY
        // figure, so without this guard scrubbing past a
        // figure's entrance with onion skin on panicked in
        // `positions` (index out of bounds).
        if project.frames[frame].poses.len() <= fi {
            return Vec::new();
        }
        let pos = core.positions(frame, fi);
        let def = &project.figures[fi].def;
        let mut pieces = Vec::new();
        for (i, node) in def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                pieces.extend(stykanimate_rs::paint::styled_segment_pieces(
                    pos[p],
                    pos[i],
                    0.0,
                    node.thickness,
                    &node.style,
                    node.color.unwrap_or(base),
                ));
            }
        }
        pieces
    };
    if prev > 0 && core.current > 0 {
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            let base = project.figures[fi].color;
            out.push((pieces_at(core.current - 1, fi, base), base, 0x4D as f32 / 255.0));
        }
        for back in 2..=(prev as usize) {
            if core.current < back {
                break;
            }
            for fi in 0..project.figures.len() {
                if project.figures[fi].hidden {
                    continue;
                }
                let base = project.figures[fi].color;
                out.push((pieces_at(core.current - back, fi, base), base, 0x26 as f32 / 255.0));
            }
        }
    }
    for ahead in 1..=(next as usize) {
        let frame = core.current + ahead;
        if frame >= core.frame_count() {
            break;
        }
        let alpha = (if ahead == 1 { 0x4D } else { 0x26 }) as f32 / 255.0;
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            out.push((pieces_at(frame, fi, 0x7BE495), 0x7BE495, alpha));
        }
    }
    out
}

/// Clip a stage-space polygon to the stage rect
/// `[0, 0]..stage` (Sutherland–Hodgman against the four
/// half-planes). Vertices keep their winding and order;
/// a polygon lying wholly inside comes back
/// vertex-for-vertex identical, and one lying wholly
/// outside comes back empty.
///
/// This exists for FX discs (see [`particle_prim`]): the
/// Rune canvas does not clip shapes, so the intersection
/// has to be materialised as geometry here.
fn clip_polygon_to_stage(poly: &[[f32; 2]], stage: [f32; 2]) -> Vec<[f32; 2]> {
    // One half-plane pass: keep the side of the axis line
    // `p[axis] == bound` that `keep_below` selects,
    // inserting the crossing point wherever an edge
    // straddles the line.
    fn pass(
        poly: Vec<[f32; 2]>,
        axis: usize,
        bound: f32,
        keep_below: bool,
    ) -> Vec<[f32; 2]> {
        // Signed distance to the line, positive inside.
        let dist = |p: [f32; 2]| {
            if keep_below {
                bound - p[axis]
            } else {
                p[axis] - bound
            }
        };
        let mut out = Vec::with_capacity(poly.len() + 1);
        for i in 0..poly.len() {
            let cur = poly[i];
            let prev = poly[(i + poly.len() - 1) % poly.len()];
            let (dc, dp) = (dist(cur), dist(prev));
            if dc >= 0.0 {
                if dp < 0.0 {
                    // Entering: the crossing, then the point.
                    let t = dp / (dp - dc);
                    out.push([
                        prev[0] + (cur[0] - prev[0]) * t,
                        prev[1] + (cur[1] - prev[1]) * t,
                    ]);
                }
                out.push(cur);
            } else if dp >= 0.0 {
                // Leaving: the crossing only.
                let t = dp / (dp - dc);
                out.push([
                    prev[0] + (cur[0] - prev[0]) * t,
                    prev[1] + (cur[1] - prev[1]) * t,
                ]);
            }
        }
        out
    }
    let mut out = poly.to_vec();
    for (axis, bound, keep_below) in [
        (0usize, 0.0, false),
        (0, stage[0], true),
        (1, 0.0, false),
        (1, stage[1], true),
    ] {
        out = pass(out, axis, bound, keep_below);
        if out.is_empty() {
            break;
        }
    }
    out
}

/// The stage rect FX/weather discs clip against, or
/// `None` when the project has no positive stage size —
/// then there is no frame to honour (the stage frame in
/// `stage_shape` is not drawn either) and discs draw
/// whole, as before.
fn disc_clip_stage(stage: [f32; 2]) -> Option<[f32; 2]> {
    (stage[0] > 0.0 && stage[1] > 0.0).then_some(stage)
}

/// Convert one stage-space particle primitive (FX clip mote or
/// weather particle) into the shape being built, through the
/// rotated transform: discs become small polygons (Shape has no
/// circle primitive), lines keep a hairline-plus width.
///
/// `clip` is the stage rect for the FX and weather passes:
/// a disc straddling the stage edge is clipped to the
/// frame before it is drawn (Wave 10, closing the residual
/// `stage_fx`'s module delta 9 left to this layer). Rune's
/// `Shape` has no clip region, so the disc's 10-gon is
/// built in stage space, intersected with the rect by
/// [`clip_polygon_to_stage`], and only then mapped to the
/// screen. A disc lying wholly inside keeps exactly its
/// old outline (the clip returns it unchanged, and the
/// screen-radius floor `max(radius * s, 0.8)` is preserved
/// by flooring the stage radius at `0.8 / s` instead); the
/// only visual change is at a straddled edge, where the
/// polygon now ends flush with the frame instead of
/// painting up to a full radius past it — fog banks up to
/// 100 units, the storm lightning disc up to 300. The
/// clipped arc follows the same 10-gon chords the disc
/// always had, so no new approximation appears; under a
/// rotated camera the polygon's phase rotates with the
/// stage rather than staying screen-upright, which is
/// invisible for a near-circle mote. The gizmo passes
/// pass `None`: selection markers are exempt by design
/// (stage_fx delta 9) so an off-stage anchor stays
/// findable. The `Line` arm ignores `clip` — trail lines
/// arrive already cut to the rect by `stage_fx`.
fn particle_prim(
    shape: Shape,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
    prim: crate::rune_sec::stage_fx::ParticlePrim,
    clip: Option<[f32; 2]>,
) -> Shape {
    use crate::rune_sec::stage_fx::ParticlePrim;
    match prim {
        ParticlePrim::Disc { pos, radius, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            match clip {
                Some(stage) => {
                    // Floor in stage units, so the clipped
                    // path keeps the unclipped path's
                    // minimum screen radius of 0.8.
                    let r = if s > 0.0 { radius.max(0.8 / s) } else { radius };
                    let poly = clip_polygon_to_stage(&ngon(pos, r, 10), stage);
                    if poly.len() < 3 {
                        return shape;
                    }
                    let screen: Vec<[f32; 2]> =
                        poly.iter().map(|p| rt.to_screen(*p)).collect();
                    shape.fill(screen, c)
                }
                None => shape.fill(ngon(rt.to_screen(pos), (radius * s).max(0.8), 10), c),
            }
        }
        ParticlePrim::Line { a, b, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            shape.line(rt.to_screen(a), rt.to_screen(b), (1.5 * s).max(1.0), LineCap::Round, c)
        }
    }
}

/// The shell's selection accent, `0xRRGGBB` — the 2.0 primary
/// accent, and the same value `stage_fx::GIZMO_COLOR` pins for
/// the particle gizmo, so both Phase G selection markers read
/// as one selection language on the stage.
pub const SELECTION_ACCENT: u32 = 0x6D8CFF;

/// Alpha the selected stroke's highlight underlay is drawn at
/// (and the factor its spray dots' carried opacities are
/// multiplied by). The highlight prims are widened copies of
/// the stroke drawn *under* the base stroke (see
/// [`recolor_prims`]); half-strength accent lets the glow read
/// as a halo without hiding the stroke's own colour where the
/// two overlap at the edges. `stage_layers` deliberately
/// leaves this value to the drawing side.
const HIGHLIGHT_ALPHA: f32 = 0.5;

/// Shell-side overlay choices for [`stage_shape`] (Phase G):
/// everything the stage draws that lives in the shell's
/// `State` rather than in the engine core — the guide toggles,
/// the three selection indices, the per-side onion depths,
/// and the 3D stage flag. Bundled into one `Copy` value so
/// the call site names each choice once and future overlays
/// don't grow the signature again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageOverlays {
    /// Master guides switch: the thirds grid, and the gate
    /// for both sub-toggles below (frustum/safe frames never
    /// draw while this is off, whatever they say).
    pub guides: bool,
    /// Camera-frustum outline under `guides` — and, since
    /// RM18 (P-6c), the frustum-true thirds grid inside it.
    pub guides_frustum: bool,
    /// Action/title-safe frames under `guides`: the
    /// stage's own rects, and since RM18 (P-6c) the framed
    /// view's rects with their "Action"/"Title" labels.
    pub guides_safe: bool,
    /// Selected stroke, project-wide index into
    /// `core.project.strokes` (the `delete_stroke_at` /
    /// `highlight_geometry` indexing); `None` — or a stale
    /// index — draws no highlight.
    pub stroke_selected: Option<usize>,
    /// Selected particle event, index into
    /// `core.project.particles` (the `gizmo_geometry`
    /// indexing); `None` — or a stale index — draws no gizmo.
    pub particle_selected: Option<usize>,
    /// Selected FX clip, index into `core.project.fx_clips`
    /// (the `fx_clip_gizmo_geometry` indexing — a different
    /// list from `particle_selected`'s events); `None` — or
    /// a stale index — draws no gizmo.
    pub fx_selected: Option<usize>,
    /// Selected sprite instance, index into
    /// `core.project.sprites` (the Sprites panel's
    /// selection, Wave 13); `None` — or a stale index —
    /// draws no corner marks and keeps the plain outline.
    pub sprite_selected: Option<usize>,
    /// Draw the 3D session's figure on the stage (Phase M):
    /// set by the shell only in the Build3D workspace while
    /// a fig3 session is active. Even with the flag set, the
    /// pass in [`stage_shape`] draws only while `core.fig3`
    /// actually holds a session.
    pub fig3_stage: bool,
    /// Onion skins behind the current frame (0 = none).
    pub onion_prev: u8,
    /// Onion skins ahead of the current frame (0 = none).
    pub onion_next: u8,
}

impl Default for StageOverlays {
    /// Matches the shell's own initial state: guides off,
    /// both sub-toggles on, nothing selected, no 3D stage
    /// pass, and 1/1 onion depths — the depths the core's
    /// historical single `onion_frames` count defaulted to,
    /// so a default overlays value reproduces the
    /// pre-Phase-G stage.
    fn default() -> Self {
        StageOverlays {
            guides: false,
            guides_frustum: true,
            guides_safe: true,
            stroke_selected: None,
            particle_selected: None,
            fx_selected: None,
            sprite_selected: None,
            fig3_stage: false,
            onion_prev: 1,
            onion_next: 1,
        }
    }
}

/// Recolour every prim in a highlight list to `rgb`, replacing
/// only the colour each prim carries — points, width, radius
/// and (for spray dots) the per-dot opacity pass through
/// untouched, so the recoloured list draws exactly where the
/// original did. `stage_layers::highlight_geometry` returns
/// prims in the stroke's own colours and leaves the accent
/// decision to this side; this is that decision, kept pure so
/// it can be pinned by tests.
fn recolor_prims(
    prims: Vec<crate::rune_sec::stage_layers::StrokePrim>,
    rgb: u32,
) -> Vec<crate::rune_sec::stage_layers::StrokePrim> {
    use crate::rune_sec::stage_layers::StrokePrim;
    prims
        .into_iter()
        .map(|prim| match prim {
            StrokePrim::Polyline { pts, width, opacity, .. } => {
                StrokePrim::Polyline { pts, width, color: rgb, opacity }
            }
            StrokePrim::Filled { pts, opacity, .. } => {
                StrokePrim::Filled { pts, color: rgb, opacity }
            }
            StrokePrim::Dots { pts, radius, opacity, .. } => {
                StrokePrim::Dots { pts, radius, color: rgb, opacity }
            }
        })
        .collect()
}

/// Convert one recoloured highlight prim into the shape being
/// built: the base stroke layer's conversions (see
/// [`stage_shape`]) at [`HIGHLIGHT_ALPHA`] — polylines and
/// fills take the flat alpha on the colour (their carried
/// stroke opacity is deliberately NOT applied: the selection
/// glow stays at full half-strength so a faint stroke is
/// still visibly selected — the Phase M section 14 contract),
/// spray dots multiply it into the opacity each dot already
/// carries, per `stage_layers`' contract.
fn highlight_prim(
    shape: Shape,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
    prim: crate::rune_sec::stage_layers::StrokePrim,
) -> Shape {
    use crate::rune_sec::stage_layers::StrokePrim;
    match prim {
        StrokePrim::Polyline { pts, width, color, .. } => {
            let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
            shape.stroke_with(
                screen,
                false,
                (width * s).max(1.0),
                LineCap::Round,
                hex_color(color).unwrap_or(Color::WHITE).alpha(HIGHLIGHT_ALPHA),
            )
        }
        StrokePrim::Filled { pts, color, .. } => {
            let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
            shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(HIGHLIGHT_ALPHA))
        }
        StrokePrim::Dots { pts, radius, color, opacity } => {
            let c = hex_color(color)
                .unwrap_or(Color::WHITE)
                .alpha(opacity * HIGHLIGHT_ALPHA);
            let mut shape = shape;
            for p in pts {
                shape = shape.fill(ngon(rt.to_screen(p), (radius * s).max(0.8), 8), c);
            }
            shape
        }
    }
}

/// Map the 3D session's stage-space prims (`stage_fig3`)
/// into screen-space draw tuples — `(a, b, colour, width)`
/// per prim: the endpoints through the same rotated
/// transform the figure bones are drawn with
/// (`RotTransform::to_screen`, which at angle 0 is
/// [`to_screen`] verbatim), the colour through
/// [`hex_color`], and the width scaled by the view scale
/// exactly as a bone's thickness is (`width * s`, no
/// clamp). Pure so the mapping is testable without a
/// `Shape` readback; an empty prim list maps to an empty
/// list, which is how the stage's fig3 pass draws nothing
/// when the session projects nothing onto the stage.
fn fig3_screen_segments(
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
    prims: Vec<crate::rune_sec::stage_fig3::StageFig3Prim>,
) -> Vec<([f32; 2], [f32; 2], Color, f32)> {
    prims
        .into_iter()
        .map(|prim| {
            (
                rt.to_screen(prim.a),
                rt.to_screen(prim.b),
                hex_color(prim.color).unwrap_or(Color::WHITE),
                prim.width * s,
            )
        })
        .collect()
}

/// The whole stage as one vector `Shape` in canvas-element
/// points: onion skins (under), then every visible figure at the
/// display frame — segments drawn from
/// `paint::styled_segment_pieces` (per-node colour, taper, cap
/// geometry and style opacity per piece; a node with a `color2`
/// gradient draws as blended sub-segments, mirroring the
/// exporters; a node with a nonzero bend at the display frame
/// arcs through `model::bend_mid` as two pieces, again
/// mirroring the exporters), shape-node rings/polygons at the
/// nodes' style opacity, head rings — plus
/// joint handles on the selected figure. Mirrors the GPUI stage's
/// `draw_data`/`paint_stage` decisions (figure opacity fades
/// toward the background; solo/hidden figures skipped via
/// `layers::visible_figures`).
///
/// `overlays` carries the shell-side choices (see
/// [`StageOverlays`]): the guide toggles gate the guide passes
/// below, the onion depths size the skins, and the selection
/// indices add two Phase G markers — the selected stroke's
/// accent highlight in the stroke slot, *under* the base
/// strokes so the stroke stays crisp on top of its own glow
/// (the draw order `stage_layers` prescribes), and the
/// selected particle event's anchor gizmo *last*, over even
/// the weather: a locator marker that weather or text could
/// cover would fail at its only job, and like the joint
/// handles it is small, translucent and selection-only.
/// Phase K adds a third marker in the same last slot: the
/// selected FX clip's gizmo, drawn after the particle
/// gizmo — it locates the clip being edited, so it follows
/// the particle gizmo's rules exactly.
/// Phase M adds one content pass between the figures and
/// the FX: the 3D session's figure on the stage, drawn
/// when `overlays.fig3_stage` is set and a session exists.
pub fn stage_shape(core: &EditorCore, el_size: [f32; 2], overlays: StageOverlays) -> Shape {
    // The overlay Centre action aims the view with the size
    // the stage was last drawn at (see `last_stage_size`).
    LAST_STAGE_SIZE.with(|c| c.set(el_size));
    let project = &core.project;
    let display = core.display_frame();
    // The display transform: the camera at the display frame
    // with the shell view navigation folded in (identity view
    // = the camera's own transform, bit for bit).
    let rt = display_transform(core, el_size);
    let s = rt.vt.scale;
    // The feedback layer's scale record (RM15): session
    // consumers that only see stage points convert their
    // screen-space constants through `last_display_scale`.
    LAST_DISPLAY_SCALE.with(|c| c.set(s));
    let mut shape = Shape::new();

    // The stage frame (UI mission Wave 5, audit S10 D1),
    // drawn first, under every pass: the stage rect filled
    // with the project background and outlined with the
    // theme's border token. The canvas element itself now
    // wears the chrome card colour (see `main.rs`), so the
    // letterbox reads as surround and the frame shows exactly
    // what an export will contain, in every workspace, with
    // guides on or off. Until now the whole element wore the
    // project background and the only stage outline was the
    // guides border — which defaults off — so stage area and
    // letterbox were one flat colour and off-stage content
    // looked identical to on-stage content. (The guides' own
    // border overdrawing this frame when guides are on is the
    // pre-existing rest-state overlap, not a second frame.)
    if project.stage[0] > 0.0 && project.stage[1] > 0.0 {
        let theme = crate::rune_sec::app_theme::app_theme(!core.light_theme);
        let (w, h) = (project.stage[0], project.stage[1]);
        let corners = [
            rt.to_screen([0.0, 0.0]),
            rt.to_screen([w, 0.0]),
            rt.to_screen([w, h]),
            rt.to_screen([0.0, h]),
        ];
        shape = shape.fill(
            corners.to_vec(),
            hex_color(project.background).unwrap_or(theme.card),
        );
        for (a, b) in [
            (corners[0], corners[1]),
            (corners[1], corners[2]),
            (corners[2], corners[3]),
            (corners[3], corners[0]),
        ] {
            shape = shape.line(a, b, 1.0, LineCap::Round, theme.card_border);
        }
    }

    if overlays.guides {
        let guide_color = Color::WHITE.alpha(0.22);
        for prim in crate::rune_sec::stage_camera::guides_geometry(project.stage) {
            let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
        }
        // Camera frustum + safe frames (Phase F stage_camera):
        // what an export frames, and the action/title-safe
        // insets, in the same ink as the thirds grid. Phase G
        // splits them into independent sub-toggles, both still
        // gated by the master `guides` switch above.
        if overlays.guides_frustum {
            for prim in crate::rune_sec::stage_camera::frustum_geometry(core) {
                let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
            }
            // Frustum-true thirds (Roadmap Wave 1, RM18 /
            // P-6c): the rule-of-thirds grid of the FRAMED
            // VIEW, inside the outline above — under any
            // pan/zoom/rotation the stage grid at the top of
            // this pass stops describing the shot, and these
            // lines take over that job. Same ink, same gate.
            for prim in crate::rune_sec::stage_camera::frustum_thirds_geometry(core) {
                let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
            }
        }
        if overlays.guides_safe {
            for prim in crate::rune_sec::stage_camera::safe_frame_geometry(project.stage) {
                let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
            }
            // Frustum-true safe areas (RM18 / P-6c): the
            // action/title-safe rects of the framed view,
            // beside the stage's own above, with their
            // labels laid out by stage_text's label layout
            // (the sprite labels' machinery) at a glyph
            // height relative to the framed view's height,
            // so the tags scale with the shot they annotate.
            for prim in crate::rune_sec::stage_camera::frustum_safe_geometry(core) {
                let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
            }
            if let Some(c) = crate::rune_sec::stage_camera::frustum_corners(core) {
                let frame_h = ((c[3][0] - c[0][0]).powi(2)
                    + (c[3][1] - c[0][1]).powi(2))
                .sqrt();
                let size = frame_h * 0.035;
                if size >= 4.0 {
                    for label in
                        crate::rune_sec::stage_camera::frustum_safe_labels(core, size)
                    {
                        for tp in crate::rune_sec::stage_text::label_boxes_alpha(
                            label.centre,
                            label.angle_deg,
                            label.kind.label(),
                            size,
                            0xFFFFFF,
                            0.85,
                        ) {
                            let crate::rune_sec::stage_text::TextPrim::GlyphBox {
                                corners,
                                color,
                                alpha,
                            } = tp;
                            let screen: Vec<[f32; 2]> =
                                corners.iter().map(|p| rt.to_screen(*p)).collect();
                            shape = shape.fill(
                                screen,
                                hex_color(color).unwrap_or(Color::WHITE).alpha(alpha),
                            );
                        }
                    }
                }
            }
        }
    }

    // Onion skins (Wave 12, SPEC-ONION): styled pieces, drawn
    // per piece at the piece's own width; the colour is the
    // piece's colour with its alpha composed as piece alpha ×
    // ghost falloff (multiplication, never replacement — the
    // hex colours are opaque, so the product is set directly).
    for (pieces, _base, ghost_alpha) in onion_skins(core, overlays.onion_prev, overlays.onion_next) {
        for piece in pieces {
            let color = hex_color(piece.color)
                .unwrap_or(Color::WHITE)
                .alpha(piece.alpha * ghost_alpha);
            shape = shape.line(
                rt.to_screen(piece.a),
                rt.to_screen(piece.b),
                piece.width * s,
                LineCap::Round,
                color,
            );
        }
    }

    // Selected stroke's highlight (Phase G): the widened
    // prims from `highlight_geometry`, recoloured to the
    // selection accent, drawn in the stroke layer's slot
    // BEFORE the base strokes so the glow sits under the
    // stroke it marks. The helper returns an empty vec for a
    // stale index or a stroke on another frame, so this draws
    // nothing then — no index is ever dereferenced here.
    if let Some(idx) = overlays.stroke_selected {
        let prims = crate::rune_sec::stage_layers::highlight_geometry(core, idx);
        for prim in recolor_prims(prims, SELECTION_ACCENT) {
            shape = highlight_prim(shape, &rt, s, prim);
        }
        // The stroke's own vertices as dots (Phase J, §34):
        // same accent, same pass, drawn over the glow so the
        // authored points read as the stroke's handles. The
        // helper shares the highlight's indexing/frame rules,
        // so it is empty exactly when the glow is.
        let dots = crate::rune_sec::stage_layers::stroke_points_geometry(core, idx);
        for prim in recolor_prims(dots, SELECTION_ACCENT) {
            shape = highlight_prim(shape, &rt, s, prim);
        }
    }

    // Drawing strokes under the figures (the GPUI canvas's
    // order): polylines at real width, filled shapes, spray dots.
    for prim in crate::rune_sec::stage_layers::stroke_geometry(core) {
        use crate::rune_sec::stage_layers::StrokePrim;
        match prim {
            StrokePrim::Polyline { pts, width, color, opacity } => {
                let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.stroke_with(
                    screen,
                    false,
                    (width * s).max(1.0),
                    LineCap::Round,
                    hex_color(color).unwrap_or(Color::WHITE).alpha(opacity),
                );
            }
            StrokePrim::Filled { pts, color, opacity } => {
                let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(opacity));
            }
            StrokePrim::Dots { pts, radius, color, opacity } => {
                let c = hex_color(color).unwrap_or(Color::WHITE).alpha(opacity);
                for p in pts {
                    shape = shape.fill(ngon(rt.to_screen(p), (radius * s).max(0.8), 8), c);
                }
            }
        }
    }

    let visible = stykanimate_rs::layers::visible_figures(project, core.solo);
    for (fi, fig) in project.figures.iter().enumerate() {
        if !visible[fi] {
            continue;
        }
        // Wave Y (Y8, P-10): in setup mode the stage
        // draws the rest pose (rooted at the display
        // frame's sampled root) — what you see is what
        // a setup drag edits. The joint handles below
        // derive from this same `pos`.
        let pos = if core.setup_mode {
            core.setup_positions_f(display, fi)
        } else {
            core.positions_f(display, fi)
        };
        // Per-node bends at the display frame (export.rs makes
        // the same per-figure fetch); empty when the project
        // carries no bend data, and every segment then takes
        // the straight path in `paint::styled_segment_pieces`
        // unchanged.
        // Bends apply to this main figure pass only: the onion
        // skins above and the joint handles below stay
        // unbended, exactly as they were before bends reached
        // the stage (the rasterizer draws no ghosts at all, so
        // there is no exporter ghost behaviour to mirror).
        // Wave Y (P-7): with channels present the bends come
        // from the same `apply_channels` evaluation
        // `positions_f` runs — its bend rows are seeded from
        // `bends_at` and overridden by any Bend channel, so a
        // bend channel shows on the stage; the no-channels
        // path is `bends_at` verbatim, as before.
        // Roadmap Wave 1 (RM6, F-5): that evaluation is now
        // the core's one shared `evaluated_frame` — computed
        // once per (revision, frame) for the whole redraw
        // instead of a second full `apply_channels` per
        // figure here; its bend rows are the same values in
        // both branches (seeded from `bends_at`, channel
        // overrides applied).
        // Bends are per-frame pose data: a rest pose
        // draws straight in setup mode (Wave Y, Y8).
        let bends = if core.setup_mode {
            Vec::new()
        } else {
            core.evaluated_frame(display)
                .bends
                .get(fi)
                .cloned()
                .unwrap_or_default()
        };
        // Canvas fade for layer opacity (the rasterizer uses true
        // alpha for segments; the canvas blends toward the bg —
        // the GPUI stage's decision).
        let faded = stykanimate_rs::style::lerp_color(
            project.background,
            fig.color,
            fig.opacity.clamp(0.05, 1.0),
        );
        let base = hex_color(faded).unwrap_or(Color::WHITE);
        let node_color = |node: &model::NodeDef| -> Color {
            node.color
                .map(|hex| hex_color(hex).unwrap_or(base))
                .unwrap_or(base)
        };
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                // Styled pieces from
                // `paint::styled_segment_pieces` (Wave 12): the
                // one segment construction the stage and both
                // exporters now share, so taper widths, cap
                // geometry, the `color2` gradient steps and the
                // style opacity all arrive encoded per piece
                // instead of the stage stroking every segment at
                // uniform thickness with a round cap and no
                // opacity. Gradient pieces blend node colour to
                // `color2`, parent joint to node joint — the
                // exporters' direction and lerp — and a nonzero
                // bend at the display frame arcs through
                // `model::bend_mid` as two pieces, with the
                // indexing export uses (`bends` is per node; the
                // segment to node `i` takes node `i`'s bend).
                // The near hex is exactly the colour the flat
                // path drew (node colour, else the faded figure
                // colour), so the gradient starts where the flat
                // segment did; `color2` is read from the style by
                // paint, raw, as export.rs reads it.
                //
                // Alpha composition: a piece's alpha is the style
                // opacity, and it is MULTIPLIED by the alpha this
                // pass composes — which is 1.0, because this pass
                // composes figure opacity into the colour itself
                // (the `faded` lerp toward the background above;
                // the rasterizer multiplies true alpha instead —
                // export.rs's `style.opacity * fig.opacity` —
                // where the stage keeps the GPUI stage's
                // colour-fade decision). The colours this pass
                // builds are opaque, so the product is the piece
                // alpha itself: applied by multiplication, never
                // added to or replaced by a stage-side constant.
                let bend = bends.get(i).copied().unwrap_or(0.0);
                let pieces = stykanimate_rs::paint::styled_segment_pieces(
                    pos[p],
                    pos[i],
                    bend,
                    node.thickness,
                    &node.style,
                    node.color.unwrap_or(faded),
                );
                for piece in pieces {
                    let color = hex_color(piece.color)
                        .unwrap_or(Color::WHITE)
                        .alpha(piece.alpha);
                    shape = shape.line(
                        rt.to_screen(piece.a),
                        rt.to_screen(piece.b),
                        piece.width * s,
                        LineCap::Round,
                        color,
                    );
                }
            }
        }
        // Shape segments: circle rings and regular polygons
        // centred on the node (rasterizer + GPUI parity).
        // Wave 12: the node's style opacity applies to their
        // colours the same way it applies to the segment
        // pieces above — multiplied into the colour's alpha
        // (the colours `node_color` builds are opaque, so the
        // product lands as the style opacity itself). Until
        // now the shape pass ignored `style.opacity` entirely,
        // like the segment pass did.
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let centre = rt.to_screen(pos[i]);
            let r = node.length / 2.0 * s;
            match node.shape {
                model::Shape::Circle => {
                    shape = shape.stroke_with(
                        ngon(centre, r, 24),
                        true,
                        node.thickness * s,
                        LineCap::Round,
                        node_color(node).alpha(node.style.opacity),
                    );
                }
                model::Shape::Polygon(sides) => {
                    shape = shape.fill(
                        ngon(centre, r, sides as usize),
                        node_color(node).alpha(node.style.opacity),
                    );
                }
                model::Shape::Line => {}
            }
        }
        // Head rings for every figure (both prior renderers draw
        // them; a figure without its head ring is a broken
        // drawing, not a scope choice). The radius is read from
        // the project (Phase L): `Project::head_ring_radius()`
        // returns the model's `HEAD_RING_RADIUS` constant
        // (Phase K) scaled by the project's head-ring scale, so
        // at the default scale the ring is exactly the constant
        // the stage, the rasterizer and the SVG exporter shared.
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if node.head {
                shape = shape.stroke_with(
                    ngon(rt.to_screen(pos[i]), core.project.head_ring_radius() * s, 24),
                    true,
                    5.0 * s,
                    LineCap::Round,
                    base,
                );
            }
        }
        // Joint handles on the selected figure only (the Phase C
        // scope): red octagons, the selected joint amber and
        // larger — the GPUI stage's colours and radii.
        if fi == core.selected_figure {
            for (ni, joint) in pos.iter().enumerate() {
                let selected = core.selected_node == Some(ni);
                let r = if selected { 8.0 } else { 5.0 } * s.max(0.6);
                let color = hex_color(if selected { 0xFFC65D } else { 0xFF5D5D })
                    .unwrap_or(Color::WHITE);
                shape = shape.fill(ngon(rt.to_screen(*joint), r, 8), color);
            }
        }
    }
    // Image media placeholders (Wave 12, A6 §3b): Rune's
    // `Shape` has no bitmap primitive, so the stage cannot
    // draw the PNGs the export composites — the hard limit
    // A6 named R1. Instead, every image item gets a
    // geometry-exact placeholder: the item's own quad from
    // `media::media_items` (the export's placement, evaluated
    // at the same display frame the figure pass used), filled
    // translucently and outlined at the item's alpha (§3a's
    // figure × style product for segment media). The slot is
    // A6's ruling: over the figures, under the 3D session
    // figure and the FX/text/weather passes — a documented
    // divergence from the raster, where media composites over
    // everything. Text items are not drawn here; stage_text
    // owns them later in this same shape.
    let sprite_quads = crate::stage_sprites::sprite_quads(&core.project, display);
    for item in stykanimate_rs::media::media_items(&core.project, display) {
        if let stykanimate_rs::media::MediaItem::Image { corners, alpha, .. } = item {
            // Sprite instances are painted by the sprite
            // pass below from `stage_sprites`' prim stream
            // (Wave 13, A2) — same geometry, plus selection
            // corner marks and name labels — so the generic
            // placeholder skips them here rather than
            // double-painting the quad.
            if sprite_quads.iter().any(|q| q.corners == corners) {
                continue;
            }
            let pts: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
            let accent = hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
            shape = shape.fill(pts.clone(), accent.alpha(0.16 * alpha));
            shape = shape.stroke_with(pts, true, 1.0, LineCap::Round, accent.alpha(0.7 * alpha));
        }
    }
    // Sprite instances (Wave 13, A2 + A6): the prim stream's
    // fills/outlines carry the placeholder family's accent
    // and alpha factors; the panel-selected sprite also gets
    // corner marks and a full-strength outline, and every
    // sprite gets its asset name laid over its quad by
    // `stage_text::sprite_label_boxes` (the fitted in-quad
    // layout — the integrator's reconciliation of A2's label
    // spec with A6's shipped layout: the prim decides WHICH
    // sprites are labelled and with what name; A6's layout
    // places the glyphs). Same slot as the placeholders:
    // over the figures, under the 3D session figure.
    {
        let accent = hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
        for prim in crate::stage_sprites::sprite_prims(&core.project, display, overlays.sprite_selected) {
            use crate::stage_sprites::SpritePrim;
            match prim {
                SpritePrim::Fill { corners, alpha, .. } => {
                    let pts: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
                    shape = shape.fill(pts, accent.alpha(alpha));
                }
                SpritePrim::Outline { corners, width, alpha, .. } => {
                    let pts: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
                    shape = shape.stroke_with(pts, true, width * s, LineCap::Round, accent.alpha(alpha));
                }
                SpritePrim::CornerMark { a, b, width, .. } => {
                    shape = shape.line(rt.to_screen(a), rt.to_screen(b), width * s, LineCap::Round, accent);
                }
                SpritePrim::Label { sprite, content, .. } => {
                    if let Some(q) = sprite_quads.iter().find(|q| q.sprite == sprite) {
                        for tp in crate::rune_sec::stage_text::sprite_label_boxes(q.corners, &content, SELECTION_ACCENT) {
                            let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = tp;
                            let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
                            shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
                        }
                    }
                }
            }
        }
    }

    // The 3D session's figure on the stage (Phase M): the
    // projected prims from `stage_fig3` — stage coordinates,
    // already clipped to the stage rect — drawn over the
    // 2D figures and their onion skins but under the FX,
    // text and weather: the session figure is stage
    // content, not an effect, and the FX/weather passes
    // must still fall over it. Gated twice: the shell sets
    // the overlay flag only in the Build3D workspace, and
    // a fig3 session must actually exist.
    if overlays.fig3_stage && core.fig3.is_some() {
        let prims = crate::rune_sec::stage_fig3::stage_fig3_prims(
            core,
            crate::rune_sec::fig3_preview::Fig3Palette::from_theme(
                &crate::rune_sec::app_theme::app_theme(!core.light_theme),
            ),
        );
        for (a, b, color, width) in fig3_screen_segments(&rt, s, prims) {
            shape = shape.line(a, b, width, LineCap::Round, color);
        }
    }
    // FX particles, text labels, then weather on top — the
    // rasterizer's pass order (export.rs render_frame_at) —
    // and subtitles over all of it (the burn-in's slot),
    // before the selection gizmos below.
    // Both particle passes clip their discs to the stage
    // rect (`particle_prim`'s `clip`): exports have no
    // pixels outside the frame, and the canvas must not
    // paint any there either.
    let disc_clip = disc_clip_stage(project.stage);
    for prim in crate::rune_sec::stage_fx::fx_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim, disc_clip);
    }
    for prim in crate::rune_sec::stage_text::text_geometry(core) {
        let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = prim;
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
    }
    for prim in crate::rune_sec::stage_fx::weather_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim, disc_clip);
    }
    // Subtitles (Wave P, P28): the display frame's live cues
    // over ALL frame content — the raster burn-in composites
    // its cues into the finished frame the same way, and the
    // band layout is the shared contract `stage_text`'s
    // subtitle section documents, so the stage shows what
    // the SVG export contains (the raster burn-in adopts
    // the same layout via IA-P26-1). The prims are stage-space
    // like the text pass's, mapped through the same display
    // transform: the bands stay pinned to the picture under
    // camera and view moves. The selection gizmos still
    // follow — they are editor markers, not frame content,
    // and must stay findable over a subtitle. A frame with
    // no live cue yields no prims and draws as before.
    for prim in crate::rune_sec::stage_text::subtitle_geometry(core) {
        let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = prim;
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
    }
    // Selected particle event's anchor gizmo (Phase G), drawn
    // last — over figures, FX, text and weather — so the
    // marker is always findable (see the `stage_shape` docs).
    // The prims arrive already in the accent at the gizmo's
    // own alpha, so they convert through the ordinary particle
    // path; a stale index yields an empty vec and draws
    // nothing.
    if let Some(idx) = overlays.particle_selected {
        for prim in crate::rune_sec::stage_fx::gizmo_geometry(core, idx) {
            // No disc clip: gizmo markers are exempt by
            // design (stage_fx delta 9) — the marker must
            // stay findable at an off-stage anchor.
            shape = particle_prim(shape, &rt, s, prim, None);
        }
    }
    // Selected FX clip's gizmo (Phase K), after the particle
    // gizmo in the same last slot: the marker for the clip
    // being edited, drawn over the weather for the same
    // reason the particle gizmo is. The prims arrive in the
    // accent at their own alpha, so they convert through the
    // particle gizmo's own path; a stale index yields an
    // empty vec and draws nothing.
    if let Some(idx) = overlays.fx_selected {
        for prim in crate::rune_sec::stage_fx::fx_clip_gizmo_geometry(core, idx) {
            // Unclipped, like the particle gizmo above.
            shape = particle_prim(shape, &rt, s, prim, None);
        }
    }
    // Motion-path overlay (Roadmap Wave 1, RM16 — P-4),
    // the last content pass: the selected figure's evaluated
    // root trajectory with one draggable point per frame,
    // and the selected joint's trajectory beside it (see
    // the motion-path section below). It reads the CORE's
    // own `show_paths` flag — the Wave 7 field whose only
    // consumers until now were the path ops themselves —
    // so it needs no `StageOverlays` field and no shell
    // wiring to appear. Setup mode hides it: the stage
    // there draws rest poses, and the path is not what a
    // setup drag edits.
    if core.show_paths && !core.setup_mode {
        shape = motion_path_prims(shape, core, &rt, s);
    }
    // Stage feedback (Roadmap Wave 1, RM15 — P-3): hover
    // highlight, the live drag HUD and the pointer
    // coordinate readout — after even the motion-path
    // markers, since the HUD must never be covered by the
    // content it annotates. Draws nothing while the
    // pointer probe is empty.
    let shape = draw_stage_feedback(shape, core, &rt, el_size, s);
    shape
}

/// The stage pass for the motion-path overlay: the
/// selected figure's root path — samples stroked in the
/// figure's own colour at half strength, one octagon
/// marker per frame, the current frame's marker amber and
/// larger (the selected-joint ink) — plus the selected
/// joint's samples in translucent white. An in-flight drag
/// substitutes its live displacement into the grabbed
/// point and its two half-frame neighbours (see
/// [`path_drag_preview`]). A layers-hidden figure and an
/// empty path draw nothing. Called by [`stage_shape`] in
/// its tail (RM15's feedback pass follows it), gated
/// there on `core.show_paths && !core.setup_mode`.
fn motion_path_prims(
    mut shape: Shape,
    core: &EditorCore,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
) -> Shape {
    let fi = core.selected_figure;
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return shape;
    }
    let Some(fig) = core.project.figures.get(fi) else {
        return shape;
    };
    let (mut points, mut samples, joint) = path_geometry(core);
    if points.is_empty() {
        return shape;
    }
    // Live drag preview: the grabbed point — and the two
    // half-frame samples beside it, which are interpolations
    // dominated by it — ride the pointer's displacement.
    // The joint line does not follow: it is evaluation, and
    // settles to the dragged truth on release.
    if let Some((frame, delta)) = path_drag_preview(fi) {
        if delta != [0.0, 0.0] {
            if let Some(p) = points.get_mut(frame) {
                p[0] += delta[0];
                p[1] += delta[1];
            }
            if !samples.is_empty() {
                let lo = (2 * frame).saturating_sub(1);
                let hi = (2 * frame + 1).min(samples.len() - 1);
                for p in &mut samples[lo..=hi] {
                    p[0] += delta[0];
                    p[1] += delta[1];
                }
            }
        }
    }
    let base = hex_color(fig.color).unwrap_or(Color::WHITE);
    if samples.len() > 1 {
        let screen: Vec<[f32; 2]> = samples.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.stroke_with(screen, false, 1.5, LineCap::Round, base.alpha(0.55));
    }
    if joint.len() > 1 {
        let screen: Vec<[f32; 2]> = joint.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.stroke_with(screen, false, 1.0, LineCap::Round, Color::WHITE.alpha(0.35));
    }
    let amber = hex_color(0xFFC65D).unwrap_or(Color::WHITE);
    for (f, p) in points.iter().enumerate() {
        let current = f == core.current;
        let r = if current { 6.0 } else { 3.5 } * s.max(0.6);
        let color = if current { amber } else { base };
        shape = shape.fill(ngon(rt.to_screen(*p), r, 8), color);
    }
    shape
}

// ---- Motion-path overlay (Roadmap Wave 1, RM16 — P-4) --------
//
// `EditorCore::show_paths` has existed since Wave 7 beside
// the two ops that edit what it names — `nudge_path_point`
// (move one frame's root) and `smooth_path_roots` (one
// averaging pass) — but nothing ever read the flag: the
// ops' only consumers were the MCP doors and the frame-ops
// panel's Smooth button, and the path they edit was
// invisible. This section is the stage consumer roadmap
// P-4 asked for (its geometry and drag session above and
// below; the pass itself is `motion_path_prims`, called
// from `stage_shape`'s tail):
//
// * the evaluated trajectory of the SELECTED figure's root
//   across every frame — a polyline through half-frame
//   samples, so channel-curved motion reads as a curve and
//   not a chord — with one point marker per frame: the
//   points are the authored data, the samples between them
//   are evaluation only;
// * the selected joint's trajectory alongside it (a plain
//   polyline, no markers): read-only by design — no op
//   moves a joint's path directly; it moves when its root
//   or its channels move;
// * a drag session over the root points: the press seeks
//   the playhead to the point's frame, the drag previews
//   live without writing, and the release lands exactly
//   one `nudge_path_point` — one undo step carrying the
//   op's own status line. The current frame's point is
//   never grabbable here: the pose drag owns the current
//   root, which sits at the same spot, so the pick skips
//   it.
//
// Evaluation goes through the core's public paths only —
// `evaluated_frame` for roots, `positions_f` for joints —
// the same values the figure pass draws. The geometry is
// memoised in a thread local keyed by (project revision,
// figure, node, frame count): RM6's revision is the honest
// key for anything derived from the project, and without
// the memo the stage would re-evaluate every frame on
// every redraw (the core's own evaluation cache holds
// only a handful of frames at a time).

/// How close (stage units) a press must land to a frame's
/// root point to grab it — the stage layers' pick
/// tolerance, in the same units for the same reason
/// (presses arrive in stage coordinates through
/// `drag_transform`).
pub const PATH_PICK_TOLERANCE: f32 = 13.0;

/// The selected figure's root at every integer frame, in
/// stage coordinates: [`EditorCore::evaluated_frame`]'s
/// pose roots, so RootX/RootY graph channels bend this
/// path exactly as they bend the drawn figure (the
/// authored frame roots underneath differ wherever a
/// channel speaks). Empty when the figure index is out of
/// range or the project has no frames.
pub fn motion_path_points(core: &EditorCore, fi: usize) -> Vec<V2> {
    let n = core.frame_count();
    if fi >= core.project.figures.len() || n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|f| core.evaluated_frame(f as f32).poses[fi].root)
        .collect()
}

/// The root path as the overlay strokes it: every integer
/// frame's root with the evaluated midpoint between
/// neighbours interleaved — layout `[f0, f0.5, f1, f1.5,
/// …]`, so frame `f`'s point sits at index `2f` and its
/// half-frame neighbours at `2f−1` / `2f+1` (the drag
/// preview's substitution indices). Channels curve the
/// root between frames (Hermite tangents on RootX/RootY),
/// so the halves keep the drawn path honest on long moves;
/// they are display only — never pickable, never edited.
pub fn motion_path_samples(core: &EditorCore, fi: usize) -> Vec<V2> {
    let n = core.frame_count();
    if fi >= core.project.figures.len() || n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2 * n - 1);
    for f in 0..n {
        out.push(core.evaluated_frame(f as f32).poses[fi].root);
        if f + 1 < n {
            out.push(core.evaluated_frame(f as f32 + 0.5).poses[fi].root);
        }
    }
    out
}

/// The selected joint's trajectory as the overlay strokes
/// it, in the same interleaved layout as
/// [`motion_path_samples`]: [`EditorCore::positions_f`]'s
/// values — the figure pass's own joint positions, channel
/// evaluation included. Empty for an out-of-range figure
/// or node, or an empty project.
pub fn joint_path_samples(core: &EditorCore, fi: usize, ni: usize) -> Vec<V2> {
    let n = core.frame_count();
    let Some(fig) = core.project.figures.get(fi) else {
        return Vec::new();
    };
    if ni >= fig.def.nodes.len() || n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2 * n - 1);
    for f in 0..n {
        out.push(core.positions_f(f as f32, fi)[ni]);
        if f + 1 < n {
            out.push(core.positions_f(f as f32 + 0.5, fi)[ni]);
        }
    }
    out
}

/// The overlay's memoised geometry for one (revision,
/// figure, node, frame count) key: the integer root
/// points, the interleaved root samples, and the
/// interleaved joint samples (empty unless a non-root
/// node is selected — the root's own "joint path" would
/// duplicate the root path). Stored in a thread local
/// beside the view navigation (see the section docs for
/// why the memo exists and why the revision keys it).
struct PathGeometry {
    revision: u64,
    fi: usize,
    node: Option<usize>,
    frames: usize,
    points: Vec<V2>,
    samples: Vec<V2>,
    joint: Vec<V2>,
}

/// One in-flight path-point drag: the frame and figure the
/// press grabbed, where the press landed, and the live
/// pointer position the overlay previews. Nothing is
/// written to the project while it lives — the release
/// converts the total displacement into one
/// `nudge_path_point` — so the record needs no restore
/// half (the sprite drag's record keeps original keys
/// because its moves write early; this session never
/// writes early, and a cancelled gesture therefore has
/// nothing to put back).
#[derive(Clone, Copy)]
struct PathDrag {
    frame: usize,
    fi: usize,
    press: V2,
    at: V2,
}

thread_local! {
    static PATH_GEOMETRY: RefCell<Option<PathGeometry>> = const { RefCell::new(None) };
    static PATH_DRAG: RefCell<Option<PathDrag>> = const { RefCell::new(None) };
}

/// The overlay geometry for the core's current selection,
/// memoised per the section docs: `(integer root points,
/// interleaved root samples, interleaved joint samples)`.
fn path_geometry(core: &EditorCore) -> (Vec<V2>, Vec<V2>, Vec<V2>) {
    let fi = core.selected_figure;
    let node = core.selected_node;
    let frames = core.frame_count();
    let revision = core.revision();
    let hit = PATH_GEOMETRY.with(|c| {
        let cell = c.borrow();
        match cell.as_ref() {
            Some(g)
                if g.revision == revision
                    && g.fi == fi
                    && g.node == node
                    && g.frames == frames =>
            {
                Some((g.points.clone(), g.samples.clone(), g.joint.clone()))
            }
            _ => None,
        }
    });
    if let Some(hit) = hit {
        return hit;
    }
    let points = motion_path_points(core, fi);
    let samples = motion_path_samples(core, fi);
    let joint = match node {
        Some(ni) if ni > 0 => joint_path_samples(core, fi, ni),
        _ => Vec::new(),
    };
    PATH_GEOMETRY.with(|c| {
        *c.borrow_mut() = Some(PathGeometry {
            revision,
            fi,
            node,
            frames,
            points: points.clone(),
            samples: samples.clone(),
            joint: joint.clone(),
        });
    });
    (points, samples, joint)
}

/// The frame whose root point a press at stage point `p`
/// would grab, or `None`: the nearest of the selected
/// figure's frame points within [`PATH_PICK_TOLERANCE`],
/// ties to the earlier frame (the graph panel's
/// nearest-key rule). Pure geometry over the drawn path,
/// behind the drawing's own gates: nothing is pickable
/// while the overlay is off or in setup mode (the pass
/// draws nothing then), nor on a figure the layers hide,
/// and the CURRENT frame's point is excluded — the pose
/// drag owns the current root, which sits at the same
/// spot.
pub fn path_point_at(core: &EditorCore, p: V2) -> Option<usize> {
    if !core.show_paths || core.setup_mode {
        return None;
    }
    let fi = core.selected_figure;
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return None;
    }
    let pts = motion_path_points(core, fi);
    let mut best: Option<(usize, f32)> = None;
    for (f, pt) in pts.iter().enumerate() {
        if f == core.current {
            continue;
        }
        let d = (p[0] - pt[0]).hypot(p[1] - pt[1]);
        if d <= PATH_PICK_TOLERANCE && best.map_or(true, |(_, b)| d < b) {
            best = Some((f, d));
        }
    }
    best.map(|(f, _)| f)
}

/// Stage drag, Down phase for the path overlay: grab the
/// root point at stage point `p` (see [`path_point_at`])
/// and seek the playhead to its frame, so the pose on
/// stage is the one being moved. Returns true when the
/// press was consumed. Refused while playing (the pose
/// drag's own rule), and a stale record from an
/// interrupted gesture is dropped first — it holds no
/// writes, so dropping is all the cleanup it needs. No
/// snapshot: the gesture's single undo step is the
/// release's `nudge_path_point`.
pub fn path_drag_down(core: &mut EditorCore, p: V2) -> bool {
    PATH_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing {
        return false;
    }
    let Some(frame) = path_point_at(core, p) else {
        return false;
    };
    let fi = core.selected_figure;
    PATH_DRAG.with(|c| {
        *c.borrow_mut() = Some(PathDrag {
            frame,
            fi,
            press: p,
            at: p,
        });
    });
    core.goto(frame);
    true
}

/// Stage drag, Move phase: track the pointer for the
/// overlay's live preview. Writes nothing to the project.
/// Returns true while a path drag owns the gesture; a
/// record gone stale mid-gesture (its figure or frame
/// deleted under it) ends the drag instead of writing
/// anywhere.
pub fn path_drag_move(core: &mut EditorCore, p: V2) -> bool {
    let alive = PATH_DRAG.with(|c| {
        let mut cell = c.borrow_mut();
        match cell.as_mut() {
            Some(d) if d.fi < core.project.figures.len() && d.frame < core.frame_count() => {
                d.at = p;
                true
            }
            _ => false,
        }
    });
    if !alive {
        PATH_DRAG.with(|c| *c.borrow_mut() = None);
    }
    alive
}

/// Stage drag, Up phase: settle the gesture. A cancelled
/// gesture just drops the record (nothing was written, so
/// there is nothing to restore and no undo step to
/// spend). Otherwise the press-to-release displacement
/// lands as ONE `nudge_path_point` on the grabbed frame's
/// root — one undo step, the op's own status line. A
/// press that never moved spends nothing: it was a seek,
/// and the seek already happened at Down.
pub fn path_drag_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = PATH_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    let delta = [p[0] - d.press[0], p[1] - d.press[1]];
    if delta != [0.0, 0.0] {
        core.nudge_path_point(d.frame, d.fi, delta);
    }
}

/// The live drag displacement the draw pass previews —
/// `(frame, delta)` — while a path drag is in flight on
/// figure `fi`; `None` otherwise.
fn path_drag_preview(fi: usize) -> Option<(usize, V2)> {
    PATH_DRAG.with(|c| {
        c.borrow().as_ref().and_then(|d| {
            if d.fi == fi {
                Some((d.frame, [d.at[0] - d.press[0], d.at[1] - d.press[1]]))
            } else {
                None
            }
        })
    })
}

/// Right-press selection on the stage (Phase K): select the
/// figure and joint under stage point `p` WITHOUT starting
/// a drag — the selection half of `EditorCore::begin_drag`,
/// with the same guards and the same writes, no more, no
/// less:
///
/// - While `core.playing`, a left-press grab refuses, so
///   this does too: nothing changes, `false`.
/// - A miss (`hit_test` finds no joint within its radius)
///   changes nothing — in particular the current selection
///   is never cleared — and returns `false`.
/// - A hit on a locked figure selects nothing and sets the
///   one status `begin_drag` sets on this path, `false`.
/// - Otherwise the writes are `begin_drag`'s selection
///   writes exactly: `select_figure(fi)` (which also clears
///   `selected_node`), then `selected_node = Some(ni)`.
///   Returns `true`.
///
/// Deliberately absent, because they belong to the drag
/// and not to selection: `begin_drag`'s snapshot (an undo
/// checkpoint for an edit this press never makes) and its
/// `dragging` state / `drag_to` (the pose does not move).
/// The shell wires this to the stage canvas's
/// `on_secondary_press_local`, converting the element-local
/// point through `RotTransform::for_drag` exactly as
/// `stage_drag` converts a drag's point.
pub fn secondary_select(core: &mut EditorCore, p: V2) -> bool {
    secondary_select_with_radius(core, p, 13.0)
}

/// [`secondary_select`] with the pick radius as a
/// parameter (Roadmap Wave 1, IA-RM15-2): the shell
/// passes its screen-space radius so the right-press
/// target matches the left press's at any view zoom.
pub fn secondary_select_with_radius(core: &mut EditorCore, p: V2, radius: f32) -> bool {
    if core.playing {
        return false;
    }
    let Some((fi, ni)) = core.hit_test_with_radius(p, radius) else {
        return false;
    };
    if core.project.figures[fi].locked {
        core.status = "Figure is locked — unlock it in the figures list.".into();
        return false;
    }
    core.select_figure(fi);
    core.selected_node = Some(ni);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> (f32, f32, f32) {
        (400.0, 300.0, 2.0)
    }

    #[test]
    fn camera_centre_maps_to_element_centre() {
        let vt = view_transform([800.0, 600.0], [960.0, 720.0], cam());
        assert_eq!(to_screen(&vt, [400.0, 300.0]), [400.0, 300.0]);
        assert_eq!(to_stage(&vt, [400.0, 300.0]), [400.0, 300.0]);
    }

    #[test]
    fn fit_uses_the_limiting_axis() {
        // Element is wider (relative to the stage) than tall:
        // height limits, fit = 600/720.
        let vt = view_transform([1600.0, 600.0], [960.0, 720.0], (0.0, 0.0, 1.0));
        assert!((vt.scale - 600.0 / 720.0).abs() < 1e-6);
        // Zoom multiplies the fit.
        let vt = view_transform([1600.0, 600.0], [960.0, 720.0], (0.0, 0.0, 3.0));
        assert!((vt.scale - 3.0 * 600.0 / 720.0).abs() < 1e-6);
    }

    #[test]
    fn screen_stage_roundtrip_is_exact() {
        for el_size in [[800.0, 600.0], [1440.0, 900.0], [333.0, 777.0]] {
            for zoom in [0.3, 1.0, 2.5, 4.0] {
                let vt = view_transform(el_size, [960.0, 720.0], (123.0, 456.0, zoom));
                for p in [[0.0, 0.0], [960.0, 720.0], [123.4, 567.8], [-50.0, 1000.0]] {
                    let back = to_stage(&vt, to_screen(&vt, p));
                    assert!(
                        (back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3,
                        "roundtrip drift for {p:?} at zoom {zoom}: {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn degenerate_sizes_never_give_zero_scale() {
        let vt = view_transform([0.0, 0.0], [960.0, 720.0], cam());
        assert!(vt.scale > 0.0);
        let vt = view_transform([800.0, 600.0], [0.0, 0.0], cam());
        assert!(vt.scale > 0.0);
    }

    // ---- View navigation (UI mission Wave 5, audit S10 D4) ----

    #[test]
    fn view_nav_identity_returns_the_base_transform_unchanged() {
        set_view_nav(ViewNav::IDENTITY);
        let core = EditorCore::new();
        let base = crate::rune_sec::stage_camera::RotTransform::for_display(&core, [800.0, 600.0]);
        assert_eq!(with_view_nav(base), base);
        assert_eq!(display_transform(&core, [800.0, 600.0]), base);
        let drag_base = crate::rune_sec::stage_camera::RotTransform::for_drag(&core, [800.0, 600.0]);
        assert_eq!(drag_transform(&core, [800.0, 600.0]), drag_base);
    }

    #[test]
    fn view_nav_fold_keeps_the_screen_stage_roundtrip_exact() {
        // Rotation included: the fold must invert exactly
        // under a turned camera, not only at angle 0.
        let base = crate::rune_sec::stage_camera::RotTransform::from_parts(
            [800.0, 600.0],
            [960.0, 720.0],
            (400.0, 300.0, 2.0),
            30.0,
        );
        set_view_nav(ViewNav {
            zoom: 2.0,
            pan: [13.0, -7.0],
        });
        let folded = with_view_nav(base);
        for p in [[0.0, 0.0], [960.0, 720.0], [123.4, 567.8], [400.0, 300.0]] {
            let back = folded.to_stage(folded.to_screen(p));
            assert!(
                (back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3,
                "folded roundtrip drift for {p:?}: {back:?}"
            );
        }
        reset_view_nav();
    }

    #[test]
    fn view_nav_zooms_about_the_centre_and_pans_in_element_points() {
        let base = crate::rune_sec::stage_camera::RotTransform::from_parts(
            [800.0, 600.0],
            [960.0, 720.0],
            (400.0, 300.0, 2.0),
            0.0,
        );
        set_view_nav(ViewNav {
            zoom: 2.0,
            pan: [13.0, -7.0],
        });
        let folded = with_view_nav(base);
        // The camera centre sits at the element centre under
        // the base transform; the fold keeps it there, shifted
        // by the pan (tolerance form: the fold divides by the
        // fit scale 800/960, which is inexact in binary, so the
        // result lands within 1e-3 of the exact point — the
        // same tolerance the roundtrip test above uses).
        let centre = folded.to_screen([400.0, 300.0]);
        assert!(
            (centre[0] - 413.0).abs() < 1e-3 && (centre[1] - 293.0).abs() < 1e-3,
            "expected [413, 293], got {centre:?}"
        );
        // A point 100 element points right of centre doubles
        // its offset under the 2x view zoom, plus the pan.
        let right = base.to_stage([500.0, 300.0]);
        let mapped = folded.to_screen(right);
        assert!(
            (mapped[0] - 613.0).abs() < 1e-3 && (mapped[1] - 293.0).abs() < 1e-3,
            "expected [613, 293], got {mapped:?}"
        );
        reset_view_nav();
    }

    #[test]
    fn view_zoom_at_keeps_the_cursor_anchor_put() {
        let core = EditorCore::new();
        set_view_nav(ViewNav::IDENTITY);
        let cursor = [600.0, 300.0];
        let before = display_transform(&core, [800.0, 600.0]).to_stage(cursor);
        view_zoom_at(&core, [800.0, 600.0], cursor, 2.0);
        assert_eq!(view_nav().zoom, 2.0);
        let after = display_transform(&core, [800.0, 600.0]).to_stage(cursor);
        assert!(
            (after[0] - before[0]).abs() < 1e-3 && (after[1] - before[1]).abs() < 1e-3,
            "cursor anchor drifted: {before:?} -> {after:?}"
        );
        reset_view_nav();
    }

    #[test]
    fn view_zoom_by_clamps_and_reset_restores_identity() {
        set_view_nav(ViewNav::IDENTITY);
        view_zoom_by(100.0);
        assert_eq!(view_nav().zoom, VIEW_ZOOM_MAX);
        view_zoom_by(0.0001);
        assert_eq!(view_nav().zoom, VIEW_ZOOM_MIN);
        reset_view_nav();
        assert_eq!(view_nav(), ViewNav::IDENTITY);
    }

    #[test]
    fn view_zoom_factor_is_camera_times_view() {
        // The read-only accessor the status strip reads
        // (Wave 9): at the identity view it is exactly the
        // authored camera zoom; a view zoom multiplies it;
        // reading it authors nothing on either side.
        let core = EditorCore::new();
        set_view_nav(ViewNav::IDENTITY);
        let cam = core.camera_here().2;
        assert_eq!(view_zoom_factor(&core), cam);
        view_zoom_by(2.0);
        assert_eq!(view_zoom_factor(&core), cam * 2.0);
        assert_eq!(core.camera_here().2, cam, "the accessor must not author camera keys");
        reset_view_nav();
        assert_eq!(view_zoom_factor(&core), cam);
    }

    #[test]
    fn ngon_has_n_points_on_the_radius() {
        let pts = ngon([10.0, 20.0], 5.0, 8);
        assert_eq!(pts.len(), 8);
        for p in &pts {
            let d = ((p[0] - 10.0).powi(2) + (p[1] - 20.0).powi(2)).sqrt();
            assert!((d - 5.0).abs() < 1e-4, "vertex off radius: {p:?}");
        }
        // First vertex at the top.
        assert!((pts[0][0] - 10.0).abs() < 1e-4 && (pts[0][1] - 15.0).abs() < 1e-4);
        // Fewer than 3 sides clamp to a triangle.
        assert_eq!(ngon([0.0, 0.0], 1.0, 2).len(), 3);
    }

    #[test]
    fn selection_accent_is_the_gizmo_accent() {
        // Both Phase G selection markers share one accent —
        // the value stage_fx pins for the particle gizmo.
        assert_eq!(SELECTION_ACCENT, 0x6D8CFF);
        assert_eq!(SELECTION_ACCENT, crate::rune_sec::stage_fx::GIZMO_COLOR);
    }

    #[test]
    fn overlays_default_matches_the_shells_initial_state() {
        let o = StageOverlays::default();
        assert!(!o.guides);
        assert!(o.guides_frustum && o.guides_safe);
        assert_eq!(o.stroke_selected, None);
        assert_eq!(o.particle_selected, None);
        // 1/1: the depths the core's old single count
        // defaulted to, so the default stage is unchanged.
        assert_eq!((o.onion_prev, o.onion_next), (1, 1));
    }

    #[test]
    fn overlays_default_has_no_fx_clip_selected() {
        // Phase K: the FX gizmo is selection-only, and the
        // default overlays select nothing — the pre-Phase-K
        // stage is unchanged.
        assert_eq!(StageOverlays::default().fx_selected, None);
    }

    #[test]
    fn overlays_default_has_no_fig3_stage() {
        // Phase M: the 3D stage pass is workspace-gated
        // shell state; a default overlays value never asks
        // for it, so the pre-Phase-M stage is unchanged.
        assert!(!StageOverlays::default().fig3_stage);
    }

    #[test]
    fn fig3_screen_segments_map_through_the_view_transform() {
        // The Phase C transform numbers: element 800×600,
        // stage 960×720, camera `cam()` = (400, 300, 2.0).
        // Fit is height-limited (600/720), so the scale is
        // 5/3 and the camera centre lands on the element
        // centre. Angle 0, so the rotated transform is the
        // plain `to_screen` path the bones use.
        let vt = view_transform([800.0, 600.0], [960.0, 720.0], cam());
        let rt = crate::rune_sec::stage_camera::RotTransform::new(vt, 0.0);
        let prim = crate::rune_sec::stage_fig3::StageFig3Prim {
            a: [400.0, 300.0],
            b: [460.0, 300.0],
            color: 0xFF0000,
            width: 3.0,
        };
        let segs = fig3_screen_segments(&rt, vt.scale, vec![prim]);
        assert_eq!(segs.len(), 1);
        let (a, b, color, width) = segs[0];
        // a is the camera centre: exactly the element centre.
        assert_eq!(a, [400.0, 300.0]);
        // b sits 60 stage units right of centre: 60 · 5/3
        // = 100 element points further right, same row.
        assert!((b[0] - 500.0).abs() < 1e-3 && (b[1] - 300.0).abs() < 1e-3);
        // The prim's hex colour, converted like every other
        // pass converts; the width scaled by the view scale
        // as a bone's thickness is: 3.0 · 5/3 = 5.0.
        assert_eq!(color, hex_color(0xFF0000).unwrap());
        assert!((width - 5.0).abs() < 1e-3);
    }

    #[test]
    fn fig3_screen_segments_of_nothing_is_nothing() {
        // The pass's empty case at the mapping level — the
        // flag off, or no session, both reach the draw loop
        // with no prims, and no prims map to no segments.
        let vt = view_transform([800.0, 600.0], [960.0, 720.0], cam());
        let rt = crate::rune_sec::stage_camera::RotTransform::new(vt, 0.0);
        assert!(fig3_screen_segments(&rt, vt.scale, Vec::new()).is_empty());
    }

    /// Shoelace area of a stage-space polygon (absolute).
    fn poly_area(poly: &[[f32; 2]]) -> f32 {
        let mut sum = 0.0;
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            sum += a[0] * b[1] - b[0] * a[1];
        }
        (sum / 2.0).abs()
    }

    #[test]
    fn clip_polygon_inside_the_stage_is_unchanged() {
        // A polygon lying wholly inside comes back
        // vertex-for-vertex: an unstraddled FX disc keeps
        // exactly the outline it had before the clip
        // existed.
        let stage = [960.0, 540.0];
        let square = [[100.0, 100.0], [200.0, 100.0], [200.0, 200.0], [100.0, 200.0]];
        assert_eq!(clip_polygon_to_stage(&square, stage), square.to_vec());
        let disc = ngon([480.0, 270.0], 15.0, 10);
        assert_eq!(clip_polygon_to_stage(&disc, stage), disc);
    }

    #[test]
    fn clip_polygon_outside_the_stage_is_empty() {
        let stage = [960.0, 540.0];
        let square = [[-200.0, 100.0], [-100.0, 100.0], [-100.0, 200.0], [-200.0, 200.0]];
        assert!(clip_polygon_to_stage(&square, stage).is_empty());
    }

    #[test]
    fn clip_polygon_straddling_an_edge_ends_flush_with_it() {
        // Half the square hangs past the left edge: the
        // clipped polygon covers exactly the on-stage
        // half, with every vertex inside the rect and at
        // least one lying ON the boundary the disc used
        // to paint past.
        let stage = [960.0, 540.0];
        let square = [[-50.0, 100.0], [50.0, 100.0], [50.0, 200.0], [-50.0, 200.0]];
        let clipped = clip_polygon_to_stage(&square, stage);
        assert!(clipped.len() >= 3);
        assert!(clipped.iter().all(|p| p[0] >= 0.0 && p[0] <= stage[0] && p[1] >= 0.0 && p[1] <= stage[1]));
        assert!(clipped.iter().any(|p| p[0].abs() < 1e-4), "a vertex sits on the stage edge");
        assert!((poly_area(&clipped) - 5_000.0).abs() < 1e-2, "exactly the on-stage half remains");
    }

    #[test]
    fn clip_disc_straddling_the_corner_stays_in_stage() {
        // The fog-bank case at the corner: a radius-100
        // disc centred ON the corner keeps only its
        // on-stage quarter — no vertex outside the rect,
        // and roughly a quarter of the 10-gon's area.
        let stage = [960.0, 540.0];
        let disc = ngon([0.0, 0.0], 100.0, 10);
        let clipped = clip_polygon_to_stage(&disc, stage);
        assert!(clipped.len() >= 3);
        assert!(clipped.iter().all(|p| p[0] >= -1e-4 && p[0] <= stage[0] + 1e-4 && p[1] >= -1e-4 && p[1] <= stage[1] + 1e-4));
        let quarter = poly_area(&disc) / 4.0;
        assert!((poly_area(&clipped) - quarter).abs() / quarter < 0.02);
    }

    #[test]
    fn clip_storm_disc_covers_only_the_stage() {
        // The worst documented overshoot: the storm
        // lightning disc, radius 300 around the centre of
        // a 960x540 stage, reaches 30 units past the top
        // and bottom edges. Clipped, every vertex is in
        // the rect and the painted area is real but never
        // more than the stage itself.
        let stage = [960.0, 540.0];
        let disc = ngon([480.0, 270.0], 300.0, 10);
        let clipped = clip_polygon_to_stage(&disc, stage);
        assert!(clipped.iter().all(|p| p[0] >= -1e-4 && p[0] <= stage[0] + 1e-4 && p[1] >= -1e-4 && p[1] <= stage[1] + 1e-4));
        let area = poly_area(&clipped);
        assert!(area <= stage[0] * stage[1]);
        assert!(area > 0.4 * stage[0] * stage[1], "the flash still fills the stage");
    }

    #[test]
    fn disc_clip_stage_rejects_a_degenerate_stage() {
        assert_eq!(disc_clip_stage([960.0, 540.0]), Some([960.0, 540.0]));
        assert_eq!(disc_clip_stage([0.0, 540.0]), None);
        assert_eq!(disc_clip_stage([960.0, -1.0]), None);
    }

    #[test]
    fn stage_shape_builds_with_an_fx_selection() {
        // The demo project has no FX clips, so the selected
        // index is stale: the gizmo fn yields no prims and
        // the stage builds exactly as without the overlay.
        let core = EditorCore::new();
        let overlays = StageOverlays { fx_selected: Some(0), ..StageOverlays::default() };
        let _shape = stage_shape(&core, [800.0, 600.0], overlays);
    }

    #[test]
    fn secondary_select_hit_selects_figure_and_joint() {
        let mut core = EditorCore::new();
        // A second figure, so the press proves it moves the
        // selection ACROSS figures, not just within one.
        core.add_figure();
        core.select_figure(0);
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, None);
        // Press exactly on figure 1's root joint: that joint
        // sits at distance zero, so it is what `hit_test`
        // resolves, and it is what must end up selected.
        let p = core.positions(core.current, 1)[0];
        let (fi, ni) = core.hit_test(p).expect("a joint at the pressed point");
        assert_eq!(fi, 1, "the press lands on the added figure");
        let depth = core.undo_depth();
        assert!(secondary_select(&mut core, p));
        assert_eq!(core.selected_figure, fi);
        assert_eq!(core.selected_node, Some(ni));
        // Selection is not an edit: unlike `begin_drag`,
        // no undo step was taken for it.
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn secondary_select_miss_changes_nothing() {
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        let before = (
            core.selected_figure,
            core.selected_node,
            core.status.clone(),
            core.undo_depth(),
        );
        assert!(!secondary_select(&mut core, [-10_000.0, -10_000.0]));
        // Never clears: figure, joint, status and undo
        // depth are all exactly as before the miss.
        assert_eq!(
            (core.selected_figure, core.selected_node, core.status.clone(), core.undo_depth()),
            before
        );
    }

    #[test]
    fn secondary_select_locked_figure_sets_begin_drags_status_only() {
        let mut core = EditorCore::new();
        core.project.figures[0].locked = true;
        let p = core.positions(core.current, 0)[0];
        assert!(!secondary_select(&mut core, p));
        // The one status `begin_drag` sets on this path,
        // byte-identical — and no selection was made.
        assert_eq!(core.status, "Figure is locked — unlock it in the figures list.");
        assert_eq!(core.selected_node, None);
    }

    #[test]
    fn secondary_select_while_playing_is_a_no_op() {
        // `begin_drag` refuses while playing; the
        // right-press mirrors it.
        let mut core = EditorCore::new();
        core.playing = true;
        let p = core.positions(core.current, 0)[0];
        assert!(!secondary_select(&mut core, p));
        assert_eq!(core.selected_node, None);
    }

    #[test]
    fn recolor_prims_replaces_only_the_colour() {
        use crate::rune_sec::stage_layers::StrokePrim;
        let prims = vec![
            StrokePrim::Polyline { pts: vec![[1.0, 2.0], [3.0, 4.0]], width: 7.0, color: 0x111111, opacity: 0.6 },
            StrokePrim::Filled { pts: vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], color: 0x222222, opacity: 0.3 },
            StrokePrim::Dots { pts: vec![[9.0, 9.0]], radius: 3.0, color: 0x333333, opacity: 0.4 },
        ];
        let out = recolor_prims(prims, SELECTION_ACCENT);
        assert_eq!(
            out,
            vec![
                StrokePrim::Polyline {
                    pts: vec![[1.0, 2.0], [3.0, 4.0]],
                    width: 7.0,
                    color: SELECTION_ACCENT,
                    opacity: 0.6,
                },
                StrokePrim::Filled {
                    pts: vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]],
                    color: SELECTION_ACCENT,
                    opacity: 0.3,
                },
                // The dots' opacity is NOT scaled here: the
                // HIGHLIGHT_ALPHA factor applies at conversion
                // time (highlight_prim), not in the recolour.
                StrokePrim::Dots {
                    pts: vec![[9.0, 9.0]],
                    radius: 3.0,
                    color: SELECTION_ACCENT,
                    opacity: 0.4,
                },
            ]
        );
        // An empty highlight (stale selection) stays empty.
        assert!(recolor_prims(Vec::new(), SELECTION_ACCENT).is_empty());
    }

    /// Demo-project core (1 figure, 5 frames) parked
    /// mid-timeline with onion skins on.
    fn onion_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.onion = true;
        core.current = 2;
        core
    }

    #[test]
    fn onion_depths_draw_each_side_independently() {
        let core = onion_core();
        let near = 0x4D as f32 / 255.0;
        let far = 0x26 as f32 / 255.0;
        let alphas = |prev: u8, next: u8| -> Vec<f32> {
            onion_skins(&core, prev, next).iter().map(|e| e.2).collect()
        };
        // Symmetric 1/1: one skin each side, both nearest.
        assert_eq!(alphas(1, 1), vec![near, near]);
        // Deeper past only: nearest + one older, no future.
        assert_eq!(alphas(2, 0), vec![near, far]);
        // Deeper future only: no past, nearest + one older.
        assert_eq!(alphas(0, 2), vec![near, far]);
        // Both deep: draw order is past (nearest→older),
        // then future — with the falloff following distance.
        assert_eq!(alphas(2, 2), vec![near, far, near, far]);
        // Zero on both sides draws nothing at all.
        assert!(onion_skins(&core, 0, 0).is_empty());
    }

    #[test]
    fn onion_skins_keep_their_colours_and_switch() {
        let mut core = onion_core();
        let skins = onion_skins(&core, 1, 1);
        assert_eq!(skins.len(), 2);
        // Past keeps the demo figure's own colour; the
        // future skin is the shared green.
        assert_eq!(skins[0].1, 0xE8E8EC);
        assert_eq!(skins[1].1, 0x7BE495);
        // Every skin carries real segments.
        assert!(skins.iter().all(|e| !e.0.is_empty()));
        // The master switch still wins over the depths.
        core.onion = false;
        assert!(onion_skins(&core, 3, 3).is_empty());
        // At the first frame there is no past to draw, however
        // deep the prev count is.
        core.onion = true;
        core.current = 0;
        let skins = onion_skins(&core, 3, 1);
        assert_eq!(skins.len(), 1);
        assert_eq!(skins[0].1, 0x7BE495);
    }

    /// The pre-Wave-12 flat segment as a style: taper 1.0,
    /// round cap, no `color2`, opacity 1.0 — `NodeStyle`'s
    /// own default, shared by the styled-pieces tests below.
    fn flat_style() -> model::NodeStyle {
        model::NodeStyle::default()
    }

    #[test]
    fn styled_segment_pieces_straight_flat_is_one_piece() {
        // Ported from the retired local
        // `segment_pieces_straight_flat_is_one_line`: one
        // piece spanning the whole segment in the flat
        // colour — now with the piece's width and alpha
        // pinned too (the full thickness; the default
        // style's opacity, 1.0).
        let pieces = stykanimate_rs::paint::styled_segment_pieces(
            [0.0, 0.0],
            [80.0, 40.0],
            0.0,
            6.0,
            &flat_style(),
            0x123456,
        );
        assert_eq!(pieces.len(), 1);
        assert_eq!((pieces[0].a, pieces[0].b), ([0.0, 0.0], [80.0, 40.0]));
        assert_eq!(pieces[0].color, 0x123456);
        assert_eq!(pieces[0].width, 6.0);
        assert_eq!(pieces[0].alpha, 1.0);
    }

    #[test]
    fn styled_segment_pieces_zero_bend_matches_the_straight_construction() {
        // Ported from the retired local
        // `segment_pieces_zero_bend_matches_the_straight_construction`,
        // re-derived at Wave 12 integration against the shared
        // generator's arithmetic (the port's first draft kept
        // the old local construction's 6 midpoint steps; the
        // generator subdivides a varying span into 4 pieces
        // sampled at their START joints — the convention
        // export.rs's byte-identical straight path requires).
        // Gradient pieces coloured at the k / 4 start samples
        // along the straight segment (the exporters' own
        // `style::lerp_color`, called directly here), endpoints
        // walking k/4 to (k+1)/4; the first piece carries the
        // pure near colour. A zero bend must reproduce it
        // exactly, so bend-less and all-zero projects draw not
        // one line differently.
        let (a, b) = ([10.0, 20.0], [110.0, 60.0]);
        let (near, far) = (0x102030u32, 0xE0D0C0u32);
        let style = model::NodeStyle { color2: Some(far), ..flat_style() };
        let pieces =
            stykanimate_rs::paint::styled_segment_pieces(a, b, 0.0, 6.0, &style, near);
        assert_eq!(pieces.len(), 4);
        let n = 4.0f32;
        for (k, piece) in pieces.iter().enumerate() {
            let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            assert_eq!(piece.a, at(k as f32 / n));
            assert_eq!(piece.b, at((k + 1) as f32 / n));
            assert_eq!(
                piece.color,
                stykanimate_rs::style::lerp_color(near, far, k as f32 / n)
            );
            assert_eq!(piece.width, 6.0);
            assert_eq!(piece.alpha, 1.0);
        }
    }

    #[test]
    fn styled_segment_pieces_bent_flat_splits_at_bend_mid() {
        // Ported from the retired local
        // `segment_pieces_bent_flat_splits_at_bend_mid`.
        let (a, b) = ([0.0, 0.0], [80.0, 40.0]);
        let bend = 0.6;
        let pieces = stykanimate_rs::paint::styled_segment_pieces(
            a,
            b,
            bend,
            6.0,
            &flat_style(),
            0x123456,
        );
        // Four pieces where the straight segment drew one
        // (Wave Y, P-7: the bend arc's quarter points, not
        // the old two-half split), all in the flat colour,
        // meeting exactly at the quarter points bend_mid
        // itself computes.
        assert_eq!(pieces.len(), 4);
        let mid = model::bend_mid(a, b, bend);
        let q1 = model::bend_mid(a, mid, bend * 0.5);
        let q3 = model::bend_mid(mid, b, bend * 0.5);
        assert_eq!((pieces[0].a, pieces[0].b), (a, q1));
        assert_eq!((pieces[1].a, pieces[1].b), (q1, mid));
        assert_eq!((pieces[2].a, pieces[2].b), (mid, q3));
        assert_eq!((pieces[3].a, pieces[3].b), (q3, b));
        assert!(pieces.iter().all(|piece| piece.color == 0x123456));
        // The joint really leaves the straight midpoint —
        // bend_mid's whole point for a nonzero bend.
        assert_ne!(mid, [40.0, 20.0]);
    }

    #[test]
    fn styled_segment_pieces_bent_gradient_splits_each_half() {
        // Ported from the retired local
        // `segment_pieces_bent_gradient_splits_each_half`,
        // re-derived at Wave 12 integration against the shared
        // generator's arithmetic (4 start-sampled pieces per
        // varying span; the width/colour parameter runs
        // continuously across the bend split — it does NOT
        // restart on the second half).
        let (a, b) = ([0.0, 0.0], [90.0, 0.0]);
        let (near, far) = (0x000000u32, 0xFFFFFFu32);
        let style = model::NodeStyle { color2: Some(far), ..flat_style() };
        // Straight gradient: 4 pieces. Bent (Wave Y, P-7):
        // four arc spans at the quarter points, each with
        // its own 4 sub-steps — 16 pieces.
        let straight =
            stykanimate_rs::paint::styled_segment_pieces(a, b, 0.0, 6.0, &style, near);
        assert_eq!(straight.len(), 4);
        let bent =
            stykanimate_rs::paint::styled_segment_pieces(a, b, 0.5, 6.0, &style, near);
        assert_eq!(bent.len(), 16);
        // The spans meet exactly at the bend midpoint: it
        // is the boundary between pieces 7 and 8.
        let mid = model::bend_mid(a, b, 0.5);
        assert_eq!(bent[7].b, mid);
        assert_eq!(bent[8].a, mid);
        // Continuous parameter, start samples: the third
        // span's first piece starts AT the split, so it
        // carries the whole-segment lerp at t = 0.5 exactly;
        // the second span's last piece starts at
        // t = 0.25 + 3/16 = 0.4375 (its span runs
        // 1/4 → 1/2 in 4 steps). The blends are
        // `style::lerp_color` itself, as the exporters call
        // it.
        let mid_hex = stykanimate_rs::style::lerp_color(near, far, 0.5);
        assert_eq!(bent[8].color, mid_hex);
        let expect_first = stykanimate_rs::style::lerp_color(near, far, 0.4375);
        assert_eq!(bent[7].color, expect_first);
    }

    #[test]
    fn styled_segment_pieces_encode_taper_cap_and_opacity() {
        // The Wave 12 point of the adoption: what the old
        // local construction could not express at all.
        let (a, b) = ([0.0, 0.0], [80.0, 0.0]);
        // Taper: the widths step down from the full
        // thickness at the near end toward thickness *
        // taper at the far end, and every piece carries the
        // style opacity as its alpha.
        let style = model::NodeStyle { taper: 0.25, opacity: 0.5, ..flat_style() };
        let pieces =
            stykanimate_rs::paint::styled_segment_pieces(a, b, 0.0, 8.0, &style, 0x123456);
        assert!(pieces.len() > 1, "a tapered segment needs steps to draw");
        assert_eq!(pieces[0].width, 8.0);
        for w in pieces.windows(2) {
            assert!(w[0].width > w[1].width, "widths must fall along the taper");
        }
        let last = pieces[pieces.len() - 1].width;
        assert!(
            last < 8.0 && last >= 8.0 * 0.25 - 1e-4,
            "far width approaches thickness * taper, got {last}"
        );
        assert!(pieces.iter().all(|piece| piece.alpha == 0.5));
        assert!(pieces.iter().all(|piece| piece.color == 0x123456));
        // Point cap: the far width is forced to zero
        // whatever the taper says — export.rs's StyledSeg
        // rule (`half_far = 0.0` for a point cap). Pieces
        // carry START samples, so the terminal 0 belongs to
        // the far joint itself and no piece carries it: the
        // last of the 4 pieces starts at t = 3/4 and carries
        // thickness * (1 - 3/4) = thickness / 4. (The port's
        // first draft asserted 0.0 — the old local
        // construction's terminal-piece convention, which
        // the shared generator does not share.)
        let style = model::NodeStyle { cap: model::CapKind::Point, ..flat_style() };
        let pieces =
            stykanimate_rs::paint::styled_segment_pieces(a, b, 0.0, 8.0, &style, 0x123456);
        assert_eq!(pieces[pieces.len() - 1].width, 8.0 / 4.0);
    }

    #[test]
    fn core_bend_data_absent_and_all_zero_draw_straight() {
        let mut core = EditorCore::new();
        core.current = 0;
        let display = core.display_frame();
        // Absent: no bend data anywhere → bends_at is empty,
        // so every segment's bend reads 0.0.
        assert!(core.project.bends_at(display, 0).is_empty());
        // All-zero data collapses to the same empty report,
        // so the two projects feed the segment pass identical
        // (zero) bends — the count equality the straight path
        // rests on.
        let nodes = core.project.figures[0].def.nodes.len();
        for frame in core.project.frames.iter_mut() {
            frame.bends = vec![vec![0.0; nodes]];
        }
        assert!(core.project.bends_at(display, 0).is_empty());
        // One bent node: exactly its segment splits, through
        // bend_mid of its evaluated endpoints.
        let (ni, parent) = core.project.figures[0]
            .def
            .nodes
            .iter()
            .enumerate()
            .find_map(|(i, n)| n.parent.map(|p| (i, p)))
            .expect("the demo figure has segments");
        for frame in core.project.frames.iter_mut() {
            frame.bends[0][ni] = 0.5;
        }
        let bends = core.project.bends_at(display, 0);
        assert_eq!(bends.len(), nodes);
        assert_eq!(bends[ni], 0.5);
        let pos = core.positions_f(display, 0);
        let node = &core.project.figures[0].def.nodes[ni];
        let pieces = stykanimate_rs::paint::styled_segment_pieces(
            pos[parent],
            pos[ni],
            bends.get(ni).copied().unwrap_or(0.0),
            node.thickness,
            &node.style,
            0xE8E8EC,
        );
        // Wave Y (P-7): the bent arc is four pieces; bend_mid
        // is the middle boundary (pieces[1] -> pieces[2]).
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[1].b, model::bend_mid(pos[parent], pos[ni], 0.5));
        assert_eq!(pieces[2].a, model::bend_mid(pos[parent], pos[ni], 0.5));
    }

    /// Wave P (P18) — the stage half of the one SETTLED
    /// zero-length cell (board contract 4; the full matrix
    /// is in ~/workspace/wave-p/findings/p18.md). A
    /// zero-length segment in the plain style (the default
    /// `NodeStyle`: round cap, taper 1.0, no `color2`)
    /// paints a disc of radius thickness/2 on all three
    /// surfaces. The stage's share of that agreement is
    /// built here: the shared generator yields ONE piece
    /// for coincident endpoints, carrying the full
    /// thickness and the near colour — no subdivision (the
    /// style does not vary) and no square-cap extension
    /// (the extension is length-gated in the generator).
    #[test]
    fn zero_length_flat_segment_is_one_coincident_piece() {
        let p = [40.0, 30.0];
        let pieces = stykanimate_rs::paint::styled_segment_pieces(
            p,
            p,
            0.0,
            12.0,
            &flat_style(),
            0xFF0000,
        );
        assert_eq!(pieces.len(), 1);
        assert_eq!((pieces[0].a, pieces[0].b), (p, p));
        assert_eq!(pieces[0].width, 12.0);
        assert_eq!(pieces[0].color, 0xFF0000);
        assert_eq!(pieces[0].alpha, 1.0);
    }

    /// Wave P (P18) — the stage DRAW half of the same
    /// settled cell: `stage_shape` turns that one piece
    /// into one stroke op whose endpoints coincide, at the
    /// full thickness, with a round cap — and a zero-length
    /// round-capped stroke is a capsule of zero length,
    /// i.e. the disc the exporters paint (the rune painter
    /// emits one KIND_SEGMENT capsule per segment; the
    /// raster pin and the SVG pin for this cell live in
    /// tests/wave_p_zero_length.rs). The shape's ops are
    /// crate-private to rune, so the assertion reads the
    /// `Debug` form and scans every stroke op for the
    /// coincident one at width 12 — transform-proof,
    /// because both endpoints pass through the same
    /// `to_screen`, so they stay bit-equal whatever the
    /// view transform is.
    #[test]
    fn stage_shape_draws_zero_length_segment_as_coincident_round_stroke() {
        reset_view_nav();
        let mut core = EditorCore::new();
        let node = |name: &str, parent: Option<usize>, thickness: f32| model::NodeDef {
            min_angle: None,
            max_angle: None,
            name: name.into(),
            parent,
            length: 0.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness,
            color: Some(0xFF0000),
            ik_end: false,
            shape: model::Shape::Line,
            style: flat_style(),
            media: None,
        };
        let mut project = model::Project::new_demo();
        project.stage = [200.0, 200.0];
        project.background = 0x000000;
        project.camera.clear();
        project.camera_follow = None;
        project.strokes.clear();
        project.figures = vec![model::FigureInstance {
            name: "zero".into(),
            color: 0xFF0000,
            def: model::FigureDef {
                nodes: vec![node("root", None, 5.0), node("dot", Some(0), 12.0)],
            },
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        }];
        project.frames = vec![model::Frame {
            poses: vec![model::Pose {
                root: [100.0, 100.0],
                angles: vec![0.0, 0.0],
            }],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        core.project = project;
        let shape = stage_shape(&core, [200.0, 200.0], StageOverlays::default());
        let dbg = format!("{shape:?}");
        let mut rest = dbg.as_str();
        let mut found = false;
        while let Some(idx) = rest.find("Stroke { points: [[") {
            let seg = &rest[idx..];
            if let Some(end) = seg.find("]]") {
                let pts = &seg["Stroke { points: [[".len()..end];
                let nums: Vec<f32> = pts
                    .split(|c: char| c == ',' || c == '[' || c == ']')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().parse().expect("a point coordinate"))
                    .collect();
                if nums.len() == 4
                    && nums[0] == nums[2]
                    && nums[1] == nums[3]
                    && seg[end..].starts_with("]], closed: false, width: 12.0, cap: Round")
                {
                    found = true;
                    break;
                }
            }
            rest = &rest[idx + 1..];
        }
        assert!(
            found,
            "the zero-length segment must reach the stage as one coincident \
             round-capped stroke at full thickness:\n{dbg}"
        );
        reset_view_nav();
    }

    /// Wave P (P28) — the stage DRAW half of the subtitle
    /// feature: `stage_shape` turns
    /// `stage_text::subtitle_geometry`'s prims into fill
    /// ops over the frame content. The pin counts fill ops
    /// in the shape's `Debug` form (the P18 idiom above):
    /// a frame with no live cue adds NO fills over the
    /// cue-free stage (those frames draw exactly as
    /// before), and a live cue adds exactly one fill per
    /// geometry prim — plates and glyphs alike — so the
    /// wiring cannot silently drop (or double) the
    /// subtitle pass. The geometry itself (bands, colours,
    /// stacking) is pinned in `stage_text`'s tests; this
    /// pin proves the prims reach the stage.
    // ---- Motion-path overlay (Roadmap Wave 1, RM16 — P-4) ----

    /// A demo core whose figure-0 roots march along y=100 in
    /// 40-unit steps (the wave7_smooth fixture style: roots
    /// written directly, no channels, so the evaluated path
    /// is the authored path).
    fn path_core() -> EditorCore {
        let mut core = EditorCore::new();
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [40.0 * i as f32, 100.0];
        }
        core
    }

    fn approx2(a: V2, b: V2) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    #[test]
    fn motion_path_points_follow_the_authored_roots() {
        let core = path_core();
        let pts = motion_path_points(&core, 0);
        assert_eq!(pts.len(), core.frame_count());
        for (f, p) in pts.iter().enumerate() {
            assert!(approx2(*p, [40.0 * f as f32, 100.0]), "frame {f}: {p:?}");
        }
        // Out-of-range figure: an empty path, never a panic.
        assert!(motion_path_points(&core, 99).is_empty());
        assert!(motion_path_samples(&core, 99).is_empty());
    }

    #[test]
    fn motion_path_samples_interleave_half_frame_evaluations() {
        let core = path_core();
        let samples = motion_path_samples(&core, 0);
        let n = core.frame_count();
        assert_eq!(samples.len(), 2 * n - 1);
        // Integer slots are the authored points themselves.
        let pts = motion_path_points(&core, 0);
        for f in 0..n {
            assert!(approx2(samples[2 * f], pts[f]), "frame {f} slot");
        }
        // The half-frame slot between frames 0 and 1 is the
        // evaluated midpoint of the (channel-free, linearly
        // interpolated) move: x = 20, between 0 and 40.
        assert!(approx2(samples[1], [20.0, 100.0]), "midpoint: {:?}", samples[1]);
    }

    #[test]
    fn joint_path_samples_track_the_selected_joint() {
        let core = path_core();
        let samples = joint_path_samples(&core, 0, 1);
        assert_eq!(samples.len(), 2 * core.frame_count() - 1);
        // The first sample is the joint's own frame-0
        // position, through the same evaluation the figure
        // pass draws.
        assert!(approx2(samples[0], core.positions(0, 0)[1]));
        // Out-of-range node / figure: empty, never a panic.
        assert!(joint_path_samples(&core, 0, 999).is_empty());
        assert!(joint_path_samples(&core, 99, 1).is_empty());
    }

    #[test]
    fn path_point_at_picks_the_nearest_non_current_point() {
        let mut core = path_core();
        core.show_paths = true;
        core.goto(2);
        // Exactly on frame 3's point: frame 3.
        assert_eq!(path_point_at(&core, [120.0, 100.0]), Some(3));
        // Exactly on the CURRENT frame's point: excluded —
        // the pose drag owns the current root — and every
        // other point is 40 units away, outside tolerance.
        assert_eq!(path_point_at(&core, [80.0, 100.0]), None);
        // Between two points, the nearer one wins.
        assert_eq!(path_point_at(&core, [112.0, 100.0]), Some(3));
        assert_eq!(path_point_at(&core, [45.0, 100.0]), Some(1));
        // Far from every point: nothing.
        assert_eq!(path_point_at(&core, [500.0, 400.0]), None);
        // Overlay off: nothing is pickable, however close.
        core.show_paths = false;
        assert_eq!(path_point_at(&core, [120.0, 100.0]), None);
        // Setup mode draws no path and picks none either.
        core.show_paths = true;
        core.setup_mode = true;
        assert_eq!(path_point_at(&core, [120.0, 100.0]), None);
    }

    #[test]
    fn path_drag_writes_one_nudge_on_release() {
        let mut core = path_core();
        core.show_paths = true;
        let depth = core.undo_depth();
        // Press on frame 1's point: consumed, and the
        // playhead seeks to the grabbed frame.
        assert!(path_drag_down(&mut core, [40.0, 100.0]));
        assert_eq!(core.current, 1);
        // Moves write nothing yet.
        assert!(path_drag_move(&mut core, [55.0, 95.0]));
        assert!(approx2(core.project.frames[1].poses[0].root, [40.0, 100.0]));
        assert_eq!(core.undo_depth(), depth);
        // The release lands the total displacement as one
        // nudge: exactly one undo step, only frame 1 moved.
        path_drag_up(&mut core, [55.0, 95.0], false);
        assert!(approx2(core.project.frames[1].poses[0].root, [55.0, 95.0]));
        assert!(approx2(core.project.frames[0].poses[0].root, [0.0, 100.0]));
        assert!(approx2(core.project.frames[2].poses[0].root, [80.0, 100.0]));
        assert_eq!(core.undo_depth(), depth + 1, "one gesture, one undo step");
        assert!(core.status.contains("Nudged path point"), "{}", core.status);
        // The gesture is over: a stray move owns nothing.
        assert!(!path_drag_move(&mut core, [0.0, 0.0]));
    }

    #[test]
    fn path_drag_cancel_and_plain_click_write_nothing() {
        let mut core = path_core();
        core.show_paths = true;
        let depth = core.undo_depth();
        // Cancelled mid-drag: the root never moved (moves
        // don't write) and no undo step was spent.
        assert!(path_drag_down(&mut core, [40.0, 100.0]));
        assert!(path_drag_move(&mut core, [90.0, 130.0]));
        path_drag_up(&mut core, [90.0, 130.0], true);
        assert!(approx2(core.project.frames[1].poses[0].root, [40.0, 100.0]));
        assert_eq!(core.undo_depth(), depth);
        // A press released where it landed is a seek, not
        // an edit: it moved the playhead at Down and spends
        // no undo step at Up.
        assert!(path_drag_down(&mut core, [120.0, 100.0]));
        assert_eq!(core.current, 3);
        path_drag_up(&mut core, [120.0, 100.0], false);
        assert!(approx2(core.project.frames[3].poses[0].root, [120.0, 100.0]));
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn path_drag_refused_while_playing() {
        let mut core = path_core();
        core.show_paths = true;
        core.playing = true;
        assert!(!path_drag_down(&mut core, [40.0, 100.0]));
        assert!(!path_drag_move(&mut core, [50.0, 100.0]));
    }

    #[test]
    fn stage_shape_draws_one_point_marker_per_frame_only_when_on() {
        reset_view_nav();
        let fill_count = |core: &EditorCore| {
            format!("{:?}", stage_shape(core, [800.0, 600.0], StageOverlays::default()))
                .matches("Fill { points:")
                .count()
        };
        let mut core = path_core();
        let off = fill_count(&core);
        core.show_paths = true;
        let on_shape = format!(
            "{:?}",
            stage_shape(&core, [800.0, 600.0], StageOverlays::default())
        );
        let on = on_shape.matches("Fill { points:").count();
        assert_eq!(
            on,
            off + core.frame_count(),
            "one marker fill per frame, nothing else filled"
        );
        // The polylines are in the same shape (the debug
        // string differs beyond the added fills).
        assert_ne!(
            on_shape,
            format!(
                "{:?}",
                stage_shape(&path_core(), [800.0, 600.0], StageOverlays::default())
            )
        );
        // Setup mode hides the pass entirely.
        core.setup_mode = true;
        assert_eq!(fill_count(&core), off);
        reset_view_nav();
    }

    #[test]
    fn stage_shape_paints_live_subtitle_prims_and_nothing_otherwise() {
        reset_view_nav();
        let fill_ops = |core: &EditorCore| {
            format!("{:?}", stage_shape(core, [640.0, 360.0], StageOverlays::default()))
                .matches("Fill { points:")
                .count()
        };
        let core_with = |cue: Option<stykanimate_rs::subtitles::SubtitleCue>| {
            let mut core = EditorCore::new();
            core.project.stage = [640.0, 360.0];
            core.project.subtitle_style = stykanimate_rs::subtitles::SubtitleStyle {
                position: stykanimate_rs::subtitles::SubtitlePosition::Bottom,
                font_scale: 1.0,
                text_color: 0xFFEE00,
                box_color: 0x112233,
                font: Default::default(),
            };
            if let Some(c) = cue {
                core.project.subtitles.push(c);
            }
            core
        };
        let cue = |start: usize, end: usize| stykanimate_rs::subtitles::SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: "HI".into(),
            speaker: None,
        };
        let none = core_with(None);
        let live = core_with(Some(cue(0, 10)));
        let later = core_with(Some(cue(5, 9)));
        // "HI" is 20 lit pixels (H = 11, I = 9), each
        // preceded by its backing plate: 40 prims.
        let prims = crate::rune_sec::stage_text::subtitle_geometry(&live).len();
        assert_eq!(prims, 40, "the live cue's prim stream");
        assert_eq!(
            fill_ops(&later),
            fill_ops(&none),
            "a cue outside the display frame adds no fills"
        );
        assert_eq!(
            fill_ops(&live),
            fill_ops(&none) + prims,
            "every subtitle prim reaches the stage as one fill"
        );
        reset_view_nav();
    }

    // --------------------------------------------------------
    // Stage feedback & precision (RM15 — P-3)
    // --------------------------------------------------------

    /// The first node of figure 0 that HAS a parent, and
    /// that parent: the demo figure's first real segment.
    fn first_segment(core: &EditorCore) -> (usize, usize) {
        core.project.figures[0]
            .def
            .nodes
            .iter()
            .enumerate()
            .find_map(|(ni, n)| n.parent.map(|par| (ni, par)))
            .expect("the demo figure has a segment")
    }

    #[test]
    fn pick_radius_is_a_screen_constant_at_every_zoom() {
        // 13 pt on screen at 1×, 0.1× and 8× alike: the
        // stage-unit radius divides by the scale, so the
        // product (what the user sees) never changes.
        for scale in [0.1f32, 0.5, 1.0, 2.0, 8.0] {
            assert!((pick_radius(scale) * scale - PICK_RADIUS_PX).abs() < 1e-4, "scale {scale}");
        }
        assert_eq!(pick_radius(1.0), 13.0, "the fit view keeps the old constant");
        assert_eq!(pick_radius(0.0), PICK_RADIUS_PX, "degenerate scale reads 1:1");
        assert_eq!(snap_tolerance(2.0), SNAP_TOLERANCE_PX / 2.0);
    }

    #[test]
    fn dist_to_segment_measures_and_clamps() {
        let (d, t) = dist_to_segment([5.0, 5.0], [0.0, 0.0], [10.0, 0.0]);
        assert!((d - 5.0).abs() < 1e-4 && (t - 0.5).abs() < 1e-4);
        let (d, t) = dist_to_segment([-3.0, 0.0], [0.0, 0.0], [10.0, 0.0]);
        assert!((d - 3.0).abs() < 1e-4 && t == 0.0, "past the end clamps to a");
        let (d, t) = dist_to_segment([2.0, 5.0], [2.0, 2.0], [2.0, 2.0]);
        assert!((d - 3.0).abs() < 1e-4 && t == 0.0, "degenerate segment measures to the point");
    }

    #[test]
    fn segment_and_joint_hits_read_the_drawn_positions() {
        let core = EditorCore::new();
        let pos = core.positions_f(0.0, 0);
        assert_eq!(joint_hit(&core, pos[0], pick_radius(1.0)), Some((0, 0)));
        // The longest segment's midpoint: distance ~0 from
        // its own segment, and a segment hit names the
        // segment's CHILD node.
        let (ni, par) = core.project.figures[0]
            .def
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(ni, n)| n.parent.map(|par| (ni, par)))
            .max_by(|a, b| {
                stykanimate_rs::model::dist(pos[a.1], pos[a.0])
                    .partial_cmp(&stykanimate_rs::model::dist(pos[b.1], pos[b.0]))
                    .unwrap()
            })
            .expect("the demo figure has segments");
        let mid = [
            (pos[par][0] + pos[ni][0]) / 2.0,
            (pos[par][1] + pos[ni][1]) / 2.0,
        ];
        let (fi, hit_ni, d) = segment_hit(&core, mid, pick_radius(1.0)).expect("the midpoint hits");
        assert_eq!((fi, hit_ni), (0, ni));
        assert!(d < 1.0, "the midpoint lies on its segment (d = {d})");
        // Beyond the radius from everything: no hit.
        assert!(segment_hit(&core, [-5000.0, -5000.0], pick_radius(1.0)).is_none());
        assert_eq!(joint_hit(&core, [-5000.0, -5000.0], pick_radius(1.0)), None);
    }

    #[test]
    fn hover_priority_is_joint_then_segment_then_nothing() {
        let core = EditorCore::new();
        let pos = core.positions_f(0.0, 0);
        assert_eq!(hover_at(&core, pos[0], 1.0), StageHover::Joint(0, 0));
        assert_eq!(hover_at(&core, [-5000.0, -5000.0], 1.0), StageHover::None);
        // Zooming out must not shrink the target away: at
        // 0.1× the same joint is still the hover at its own
        // position (the radius grew in stage units).
        assert_eq!(hover_at(&core, pos[0], 0.1), StageHover::Joint(0, 0));
    }

    #[test]
    fn cursor_follows_drag_mode_and_hover() {
        use stykanimate_rs::editor::DragKind;
        let mut core = EditorCore::new();
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Default);
        assert_eq!(stage_cursor(&core, StageHover::Joint(0, 0)), rune::Cursor::Grab);
        assert_eq!(stage_cursor(&core, StageHover::Segment(0, 1)), rune::Cursor::Grab);
        assert_eq!(stage_cursor(&core, StageHover::Sprite(0)), rune::Cursor::Move);
        assert_eq!(stage_cursor(&core, StageHover::Stroke(0)), rune::Cursor::Pointer);
        core.camera_mode = true;
        assert_eq!(stage_cursor(&core, StageHover::Joint(0, 0)), rune::Cursor::Move);
        core.camera_mode = false;
        core.draw_mode = true;
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Crosshair);
        core.draw_mode = false;
        core.dragging = Some(DragKind::Aim(1));
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Grabbing);
        core.dragging = Some(DragKind::Camera);
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Move);
        // No probe fed: the mount-level helper sees modes only.
        set_stage_pointer(None);
        core.dragging = None;
        assert_eq!(stage_cursor_now(&core, [800.0, 600.0]), rune::Cursor::Default);
    }

    #[test]
    fn snap_angle_rounds_to_the_step() {
        assert_eq!(snap_angle_deg(7.0, 15.0), 0.0);
        assert_eq!(snap_angle_deg(8.0, 15.0), 15.0);
        assert_eq!(snap_angle_deg(-22.0, 15.0), -15.0);
        assert_eq!(snap_angle_deg(44.0, 15.0), 45.0);
        assert_eq!(snap_angle_deg(33.0, 0.0), 33.0, "a zero step is no snap");
    }

    #[test]
    fn snap_aim_point_rotates_onto_the_step_ray_at_the_same_reach() {
        let pivot = [100.0, 100.0];
        let a10 = 10.0f32.to_radians();
        let p = [pivot[0] + 10.0 * a10.cos(), pivot[1] + 10.0 * a10.sin()];
        let q = snap_aim_point(pivot, p, 15.0);
        let a15 = 15.0f32.to_radians();
        assert!((q[0] - (pivot[0] + 10.0 * a15.cos())).abs() < 1e-3, "x: {q:?}");
        assert!((q[1] - (pivot[1] + 10.0 * a15.sin())).abs() < 1e-3, "y: {q:?}");
        assert_eq!(snap_aim_point(pivot, pivot, 15.0), pivot, "on the pivot: no direction");
    }

    #[test]
    fn snap_root_point_takes_landmarks_over_grid() {
        let stage = [960.0, 540.0];
        let (q, m) = snap_root_point([478.0, 200.0], stage, 6.0, None);
        assert_eq!(q[0], 480.0);
        assert!(m.centre_x && !m.grid);
        let (q, m) = snap_root_point([300.0, 536.0], stage, 6.0, None);
        assert_eq!(q[1], 540.0);
        assert!(m.ground, "the bottom edge is the ground line");
        let (q, m) = snap_root_point([4.0, 200.0], stage, 6.0, None);
        assert_eq!(q[0], 0.0);
        assert!(m.edge);
        let (q, m) = snap_root_point([300.0, 200.0], stage, 6.0, None);
        assert_eq!(q, [300.0, 200.0]);
        assert_eq!(m, RootSnap::default(), "nothing in tolerance: untouched, unmarked");
        // Grid alone: rounds to the spacing, marked as grid.
        let (q, m) = snap_root_point([312.0, 200.0], stage, 6.0, Some(50.0));
        assert_eq!(q, [300.0, 200.0]);
        assert!(m.grid);
        // Landmark beats grid on its coordinate: 478 grids
        // to 475 but centres to 480, and the grid mark for
        // x is spent.
        let (q, m) = snap_root_point([478.0, 200.0], stage, 6.0, Some(25.0));
        assert_eq!(q[0], 480.0);
        assert!(m.centre_x);
    }

    #[test]
    fn snap_drag_point_snaps_aims_and_honours_the_master_switch() {
        use stykanimate_rs::editor::DragKind;
        let mut core = EditorCore::new();
        let (ni, par) = first_segment(&core);
        let pos = core.positions(0, 0);
        let pivot = pos[par];
        let a10 = 10.0f32.to_radians();
        let p = [pivot[0] + 50.0 * a10.cos(), pivot[1] + 50.0 * a10.sin()];
        core.dragging = Some(DragKind::Aim(ni));
        set_snap_config(SnapConfig::DEFAULT);
        let (q, marks) = snap_drag_point(&core, p, 1.0);
        assert!(marks.angle, "the aim point rotated onto the 15-degree ray");
        let a15 = 15.0f32.to_radians();
        assert!((q[0] - (pivot[0] + 50.0 * a15.cos())).abs() < 1e-2, "x: {q:?}");
        assert!((q[1] - (pivot[1] + 50.0 * a15.sin())).abs() < 1e-2, "y: {q:?}");
        set_snap_config(SnapConfig { enabled: false, ..SnapConfig::DEFAULT });
        let (q, marks) = snap_drag_point(&core, p, 1.0);
        assert_eq!(q, p, "the master switch off passes the point through");
        assert_eq!(marks, SnapMarks::default());
        set_snap_config(SnapConfig::DEFAULT);
        // No drag in flight: nothing to snap.
        core.dragging = None;
        let (q, _) = snap_drag_point(&core, p, 1.0);
        assert_eq!(q, p);
    }

    #[test]
    fn drag_hud_lines_read_the_live_drag() {
        use stykanimate_rs::editor::DragKind;
        let mut core = EditorCore::new();
        assert!(drag_hud_lines(&core).is_empty(), "no drag, no HUD");
        let root = core.project.frames[0].poses[0].root;
        core.dragging = Some(DragKind::Root);
        assert_eq!(
            drag_hud_lines(&core),
            vec![format!("ROOT {:.0}, {:.0}", root[0], root[1])]
        );
        let (ni, _) = first_segment(&core);
        core.dragging = Some(DragKind::Aim(ni));
        let lines = drag_hud_lines(&core);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("ANGLE ") && lines[0].ends_with(" DEG"), "{lines:?}");
        core.dragging = Some(DragKind::Camera);
        let lines = drag_hud_lines(&core);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("CAM ") && lines[1].starts_with("ZOOM "), "{lines:?}");
        assert_eq!(pointer_readout([480.4, 519.6]), "480, 520");
    }

    #[test]
    fn nudge_root_moves_the_pose_root_with_one_snapshot_per_burst() {
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        reset_nudge_burst();
        let before = core.project.frames[0].poses[0].root;
        let depth = core.undo_depth();
        assert!(nudge_selected(&mut core, 5.0, -3.0));
        assert_eq!(
            core.project.frames[0].poses[0].root,
            [before[0] + 5.0, before[1] - 3.0]
        );
        assert_eq!(core.undo_depth(), depth + 1, "the burst's one snapshot");
        // Key-repeat inside the window rides the same snapshot.
        assert!(nudge_selected(&mut core, 1.0, 0.0));
        assert!(nudge_selected(&mut core, 1.0, 0.0));
        assert_eq!(core.undo_depth(), depth + 1, "repeats coalesce");
        // A fresh burst (the shell resets on selection/frame
        // change; tests do it by hand) snapshots again.
        reset_nudge_burst();
        assert!(nudge_selected(&mut core, 1.0, 0.0));
        assert_eq!(core.undo_depth(), depth + 2);
    }

    #[test]
    fn nudge_aim_keeps_the_segment_length_and_refuses_cleanly() {
        let mut core = EditorCore::new();
        let (ni, par) = first_segment(&core);
        core.selected_node = Some(ni);
        reset_nudge_burst();
        let pos = core.positions(0, 0);
        let len_before = stykanimate_rs::model::dist(pos[par], pos[ni]);
        assert!(nudge_selected(&mut core, 30.0, 0.0));
        let after = core.positions(0, 0);
        assert!(
            (stykanimate_rs::model::dist(after[par], after[ni]) - len_before).abs() < 1e-3,
            "aiming rotates the segment; it never stretches it"
        );
        assert!(after[ni] != pos[ni], "the joint actually moved");
        // No selection: nothing.
        core.selected_node = None;
        assert!(!nudge_selected(&mut core, 1.0, 0.0));
        // Playing: nothing (a press during playback refuses
        // the same way).
        core.selected_node = Some(ni);
        core.playing = true;
        let frozen = core.positions(0, 0);
        assert!(!nudge_selected(&mut core, 1.0, 0.0));
        assert_eq!(core.positions(0, 0), frozen);
        // The step idiom: Alt fine, Shift large, plain 1.
        assert_eq!(nudge_step(false, false), NUDGE_STEP);
        assert_eq!(nudge_step(true, false), NUDGE_STEP_LARGE);
        assert_eq!(nudge_step(true, true), NUDGE_STEP_FINE);
    }

    #[test]
    fn feedback_pass_draws_nothing_without_a_probe_and_paints_with_one() {
        reset_view_nav();
        let mut core = EditorCore::new();
        core.project.stage = [640.0, 360.0];
        let fills = |core: &EditorCore| {
            format!("{:?}", stage_shape(core, [640.0, 360.0], StageOverlays::default()))
                .matches("Fill { points:")
                .count()
        };
        set_stage_pointer(None);
        let base = fills(&core);
        // Probe over joint 0: the hover octagon plus the
        // coordinate readout's glyph fills appear.
        let joint = core.positions_f(0.0, 0)[0];
        let rt = display_transform(&core, [640.0, 360.0]);
        set_stage_pointer(Some(StagePointer {
            local: rt.to_screen(joint),
            size: [640.0, 360.0],
        }));
        assert!(fills(&core) > base, "hover + readout paint over the bare stage");
        // Dragging: the HUD plate and its glyph lines paint.
        core.dragging = Some(stykanimate_rs::editor::DragKind::Root);
        assert!(fills(&core) > base, "the HUD paints over the bare stage");
        set_stage_pointer(None);
    }
}
