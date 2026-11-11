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
//! Roadmap Wave 2 (W2-33, wiring RM18's IA-RM18-3) adds
//! the two composition variants RM18 built but left
//! undrawn — the golden-ratio grid and the diagonals —
//! as their own default-off overlay flags inside the
//! frustum gate.
//! Roadmap Wave 2 (W2-37) deepens the onion pass: per-figure
//! depth and tint overrides ([`OnionFigureOverride`]) refine
//! the per-side global depths figure by figure, fed through
//! a thread-local in the `VIEW_NAV` pattern; with no
//! overrides fed, the pass is entry-for-entry what it was.
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
//! Roadmap Wave 2 (W2-01) adds marquee multi-selection in
//! the same tail: a left-hold drag on empty stage draws a
//! rubber band and selects every figure it touches into a
//! figure set (drawn as accent bounding outlines), and a
//! press on a member of a multi-figure set drags the whole
//! set by its roots for exactly one undo step — see the
//! "Marquee multi-selection" section at [`figures_in_marquee`].
//! Roadmap Wave 2 (W2-04) completes the view-navigation
//! layer Wave 5 began: a pan gesture (middle-button drag,
//! or Space held + left drag — Rune's `Drag` carries no
//! button or modifier, so the shell routes by
//! `Tx::button()` and a key-tracked Space state; see
//! [`view_pan_down`]) that writes only the [`ViewNav`]
//! pan, and a navigator minimap in the [`navigator`]
//! submodule — the whole scene in miniature with the
//! viewport quad, click/drag to jump the view.
//! Roadmap Wave 2 (W2-06, building RM22's IA-RM22-4) adds
//! the stage sprite BITMAP layer: placed sprite instances
//! render as real pixel elements (decoded artwork through
//! `media::asset_pixels`, flips baked as mirrored variants)
//! in an overlay element layer — see [`stage_sprite_layers`]
//! — and the canvas's translucent placeholder quad for
//! sprites (the Fill/Outline prims) retires in the same
//! change; the selection corner marks, transform handles
//! and name labels stay canvas-drawn over the artwork.
//! Segment media is NOT part of the layer (RM22's spike
//! verdict: its skewed/bent/cropped quads have no element
//! counterpart) and keeps its canvas placeholder.
//! Roadmap Wave 2 (W2-05) adds rulers + stored guides:
//! user-placed guide lines persisted in the project
//! (`Project::stored_guides`, world-space stage
//! coordinates, edited through the core's stored-guide
//! ops) drawn in the guides pass under their own overlay
//! flag, graduated rulers along the stage's top/left
//! edges in the same pass, and a drag session
//! (drag off a ruler to create, drag a line to move it,
//! drag it back to its ruler to delete) — see the
//! "Stored guides & rulers" section at
//! [`stored_guide_lines`].
//! Roadmap Wave 2 (W2-27) adds joint-limit arcs: for the
//! selected node of the selected figure, the stage draws
//! the node's joint-limit partition (`model::limit_spans`
//! — the same arc `NodeDef::clamp_angle` enforces) at the
//! node's joint: the allowed span solid, the forbidden
//! remainder dashed, spokes at the bounds and a tick at
//! the parent's straight-ahead direction. Display only —
//! nothing here clamps or writes. The pass rides its own
//! opt-in overlay flag (`StageOverlays::limit_arcs`,
//! default off; the segment inspector's limit rows carry
//! the toggle) — see [`limit_arc_prims`].

use std::cell::{Cell, RefCell};

use rune::{col, el, Color, El, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, V2};

/// The stage navigator minimap (Roadmap Wave 2, W2-04):
/// whole-scene thumbnail + viewport quad in the canvas's
/// top-left corner, click/drag to jump the view. A
/// submodule so its session and geometry stay out of
/// this file's sections; it reads this module's view
/// state and drawing helpers through `super::`.
pub mod navigator;

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
    /// The character move arm (W2-02; see
    /// [`note_character_selected`]): the one figure the
    /// arrow keys currently move as a whole character
    /// (0 or 1 entries; a SET of figures is W2-01's
    /// `FIGURE_SET`, read through
    /// [`character_move_targets`]).
    static CHARACTER_ARM: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    /// The previous selection/frame/swap observation of
    /// [`sync_arrow_selection`], the shell's poll watcher.
    static ARROW_WATCH: Cell<Option<ArrowWatch>> = const { Cell::new(None) };
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
    // Roadmap Wave 2 (W2-04): a view pan in flight is a
    // grab in progress whatever the core is doing, and a
    // held Space is the pan hand waiting for its drag —
    // both read before the core's own drag/mode states,
    // the same order the routing gives them.
    if view_pan_active() {
        return rune::Cursor::Grabbing;
    }
    // Roadmap Wave 2 (W2-08): a bend drag in flight is a
    // grab in progress, like the core's own drags below.
    if bend_drag_active() {
        return rune::Cursor::Grabbing;
    }
    if space_pan_held() {
        return rune::Cursor::Grab;
    }
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
/// `stage_sprites::sprite_drag_hud`'s, and the session
/// drags' lines are the W2-07 builders below — the drawing
/// pass concatenates them all). Formatting follows the numeric
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

// ----------------------------------------------------------------
// Mode-aware drag HUD (Roadmap Wave 2, W2-07 — IA-RM22-5)
// ----------------------------------------------------------------
//
// RM15's HUD ([`drag_hud_lines`], above) reads the CORE
// drags; the stage has grown drag kinds since — the
// marquee and its group move (W2-01), the transform
// gizmo (W2-03), the motion-path drag (RM16), the sprite
// handle drags (RM22 — their builder is
// `stage_sprites::sprite_drag_hud`, mode-aware since
// this seat) and the bend drag (W2-08 — landed in this
// tree in parallel; its builder is its own
// [`bend_drag_hud`], see the note at this section's
// end). The builders below are the content half for
// those modes: pure functions from a session's live
// state to its HUD line(s), in [`drag_hud_lines`]' house
// format — plain ASCII capitals, digits and `.,-%+`, the
// canvas stamp's charset (the stamp has no degree sign
// and no multiplication sign, so degrees spell DEG and
// dimensions spell X — IA-RM22-5's `°`/`×` strings in
// stamp form). Positions print absolute (`{:.0}`);
// DELTAS print signed (`{:+.0}`) — a move's sign is its
// information. The feedback pass concatenates every
// builder's lines; each returns nothing while its
// gesture is not live, so the plate hides cleanly when
// no drag is in flight.

/// The marquee group move's HUD line (W2-01's group
/// drag): how many figures the grab took and the live
/// displacement they will settle by. `None` when no
/// group drag is in flight.
pub fn group_drag_hud() -> Option<String> {
    let delta = group_drag_delta()?;
    let n = group_drag_count()?;
    Some(format!(
        "MOVE {} FIGURES {:+.0}, {:+.0}",
        n, delta[0], delta[1]
    ))
}

/// The motion-path drag's HUD line (RM16's session): the
/// frame whose root point is grabbed — numbered as the
/// UI numbers frames, from 1 — and the live displacement
/// the release will nudge that frame's root by. Reads
/// the session through the same preview the draw pass
/// offsets the path by, for the selected figure. `None`
/// when no path drag is live on it.
pub fn path_drag_hud(core: &EditorCore) -> Option<String> {
    let (frame, delta) = path_drag_preview(core.selected_figure)?;
    Some(format!(
        "PATH FRAME {} {:+.0}, {:+.0}",
        frame + 1,
        delta[0],
        delta[1]
    ))
}

/// The marquee rubber band's HUD lines while it is being
/// drawn: the rectangle's live size, and — once it
/// covers any figure — how many it would select. A span
/// still inside the click slop on BOTH axes is a click
/// in progress, not a marquee (`marquee_up`'s own test),
/// and shows nothing.
pub fn marquee_hud_lines(core: &EditorCore) -> Vec<String> {
    let Some(rect) = marquee_live_rect() else {
        return Vec::new();
    };
    let w = rect[2] - rect[0];
    let h = rect[3] - rect[1];
    if w < MARQUEE_CLICK_SLOP && h < MARQUEE_CLICK_SLOP {
        return Vec::new();
    }
    let mut lines = vec![format!("MARQUEE {:.0} X {:.0}", w, h)];
    let n = figures_in_marquee(core, rect).len();
    if n > 0 {
        lines.push(format!("FIGURES {}", n));
    }
    lines
}

/// The transform gizmo's HUD line (W2-03), as a pure
/// formatter over the session's preview values: a move
/// drag's line is its live displacement; a rotate drag's
/// is the SNAPPED angle the release would settle (the
/// preview's `angle_deg` — the HUD says what the settle
/// writes). The builder takes primitives because
/// `stage_gizmo` is not yet registered in the shell (its
/// registration is IA-W2-03-1); the feedback pass's call
/// site rides IA-W2-07-1.
pub fn gizmo_hud_line(rotate: bool, delta: V2, angle_deg: f32) -> String {
    if rotate {
        format!("ROTATE {:+.0} DEG", angle_deg)
    } else {
        format!("MOVE {:+.0}, {:+.0}", delta[0], delta[1])
    }
}

// The bend mode's line is W2-08's own builder
// (`bend_drag_hud`, in the bend section below): the
// bend drag landed in this same tree while this seat
// was in flight, carrying its session preview and its
// `BEND n DEG` line, and the feedback pass pushes it
// after the lines above. This seat's bend slot was
// designed independently from RM15/RM22 text to the
// same shape (the preview bend in whole degrees); the
// duplicate builder was withdrawn rather than fork the
// format or collide on the name — one bend line, one
// spelling.

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
    /// The figure-set key for a set move ([`nudge_figures`]):
    /// its stamp carries `fi == usize::MAX` and the set's
    /// hash here; single-target stamps carry 0.
    set: u64,
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
fn nudge_snapshot(core: &mut EditorCore, fi: usize, ni: Option<usize>, setup: bool, label: &str) {
    let now = std::time::Instant::now();
    let reuse = NUDGE_STAMP.with(|c| c.get()).map_or(false, |st| {
        st.fi == fi
            && st.ni == ni
            && st.frame == core.current
            && st.setup == setup
            && now.duration_since(st.at).as_millis() <= NUDGE_BURST_WINDOW_MS
    });
    if !reuse {
        core.snapshot_labeled(label);
    }
    NUDGE_STAMP.with(|c| {
        c.set(Some(NudgeStamp {
            at: now,
            fi,
            ni,
            frame: core.current,
            setup,
            set: 0,
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
/// itself is the shell's (IA-RM15-4, landed composed with
/// IA-RM26-1): the shell calls [`nudge_arrows`], which
/// resolves the user-settled rule (W2-02) across this op,
/// [`nudge_figure`] and [`nudge_figures`].
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
        nudge_snapshot(core, fi, Some(ni), true, "Nudge rest joint");
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
        nudge_snapshot(core, fi, Some(ni), false, "Nudge figure");
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
    nudge_snapshot(core, fi, Some(ni), false, "Nudge joint");
    let def = &core.project.figures[fi].def;
    let pose = &mut core.project.frames[frame].poses[fi];
    model::aim_node(def, pose, ni, target);
    core.status = format!("Nudged figure {} joint {} by ({dx:.1}, {dy:.1}).", fi + 1, ni);
    true
}

// ----------------------------------------------------------------
// Arrow-key character movement (W2-02): the user-settled rule
// (2026-11-10) — the arrows MOVE the selected character/joint
// through this seat's nudge family; they step frames ONLY when
// nothing is selected. The Wave 1 wiring (IA-RM26-1 item 4,
// subsuming IA-RM15-4) landed the joint half: with
// `selected_node` set the arrows call [`nudge_selected`], else
// Left/Right step frames. The character half was missing:
// `EditorCore::select_figure` — the figures/layers/dopesheet
// row press and the MCP select tool — CLEARS the node, so a
// selected character fell through to frame-stepping. This
// section is the missing half: a session "character move arm"
// naming the figure the arrows move whole, the root-move
// ops behind it, the one resolution entry the shell calls,
// and the poll watcher that keeps the arm honest. All state
// is thread-local session state, the module's standing
// pattern (the pointer probe, the nudge stamp): nothing here
// is project data, nothing serialises, nothing undoes.
//
// Composition with W2-01's marquee (the "Marquee
// multi-selection" section below): the marquee's figure set
// is W2-01's store ([`figure_set`]); this section keeps no
// second set. [`character_move_targets`] reads the marquee
// set first (a committed marquee whose primary figure the
// core's selection follows — `marquee_up` selects the
// set's topmost figure — is the character selection), then
// this section's single-character arm. The set move itself
// is [`nudge_figures`], the same one-snapshot-per-gesture
// engine W2-01's group drag lands by hand.
// ----------------------------------------------------------------

/// Which route [`nudge_arrows`] took for one arrow press —
/// the shell ignores it (it claims the key on
/// [`arrows_move_selection`] alone); tests and future
/// surfaces (W2-01's marquee wiring) read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowMove {
    /// A joint was selected: [`nudge_selected`] ran (its
    /// own refusals — playing, locked, stale — still apply;
    /// the route is the selection's, never a frame step).
    Joint,
    /// One armed character: [`nudge_figure`] ran.
    Character,
    /// A set of characters — W2-01's marquee set, read
    /// through [`character_move_targets`]: [`nudge_figures`]
    /// ran; the payload is the set's size.
    Characters(usize),
    /// Nothing selected and nothing armed: the caller
    /// steps frames.
    None,
}

/// Arm one character for arrow movement: the shell's
/// character-selection acts call this (the row presses via
/// IA-W2-02-2; [`sync_arrow_selection`] for every other
/// path). The arm is a single character by design — a SET
/// of characters is W2-01's marquee store ([`figure_set`]),
/// which [`character_move_targets`] reads ahead of the
/// arm, so noting a marquee member here never competes
/// with the set: while the set is live and holds the
/// core's primary figure, the set is the arrow target.
pub fn note_character_selected(fi: usize) {
    CHARACTER_ARM.with(|c| *c.borrow_mut() = vec![fi]);
}

/// Stand the character move arm down: the arrows step
/// frames again (until the next character selection).
pub fn release_character_arm() {
    CHARACTER_ARM.with(|c| c.borrow_mut().clear());
}

/// The armed character: 0 or 1 entries (empty = disarmed).
pub fn armed_characters() -> Vec<usize> {
    CHARACTER_ARM.with(|c| c.borrow().clone())
}

/// The characters the arrows can move now, in resolution
/// order. Empty while a joint is selected (the joint owns
/// the arrows). Otherwise:
///
/// 1. W2-01's marquee set ([`figure_set`]), filtered to
///    live figures, when it holds the core's primary
///    figure — a committed marquee IS the character
///    selection, and `marquee_up` already made its
///    topmost figure the core's selected figure. A set
///    that does not hold the primary is stale next to a
///    newer single selection and yields to the arm.
/// 2. This section's arm, when it names the core's
///    primary figure — the staleness guard that retires
///    the arm on its own when the selection has moved on
///    by a path the watcher did not see (a deleted
///    figure, a clamped selection).
pub fn character_move_targets(core: &EditorCore) -> Vec<usize> {
    if core.selected_node.is_some() {
        return Vec::new();
    }
    let live = |fi: &usize| *fi < core.project.figures.len();
    let set: Vec<usize> = figure_set(core).into_iter().filter(live).collect();
    if set.contains(&core.selected_figure) {
        return set;
    }
    let arm = armed_characters();
    if arm.contains(&core.selected_figure) {
        return arm.into_iter().filter(live).collect();
    }
    Vec::new()
}

/// The shell's arrow gate, the user-settled rule in one
/// predicate: true when a joint is selected OR a character
/// (set) is armed — the arrows then move the selection and
/// must NOT step frames; false only when nothing is
/// selected, the one case where arrows step frames.
pub fn arrows_move_selection(core: &EditorCore) -> bool {
    core.selected_node.is_some() || !character_move_targets(core).is_empty()
}

/// Move figure `fi`'s root at the current frame by
/// `(dx, dy)` — the whole-character twin of
/// [`nudge_selected`]'s root branch, for a character
/// selected without a joint (every `select_figure` path).
/// Same discipline: burst-coalesced snapshot (keyed with
/// `ni == None`, so a character burst never shares a step
/// with a joint burst), the drag path's locked sentence,
/// silent refusal while playing or on a stale index. In
/// setup mode a whole-character move has no meaning — a
/// rest pose has no root — so it refuses with
/// `begin_setup_drag`'s own sentence, as the root-joint
/// nudge does there.
pub fn nudge_figure(core: &mut EditorCore, fi: usize, dx: f32, dy: f32) -> bool {
    if core.playing || (dx == 0.0 && dy == 0.0) {
        return false;
    }
    let Some(fig) = core.project.figures.get(fi) else {
        return false;
    };
    if fig.locked {
        core.status = "Figure is locked — unlock it in the figures list.".into();
        return false;
    }
    if core.setup_mode {
        core.status = "The rest pose has no root position — roots live in frame poses.".into();
        return false;
    }
    nudge_snapshot(core, fi, None, false, "Nudge figure");
    write_root_move(core, fi, dx, dy);
    true
}

/// Move every movable figure in `figs` by `(dx, dy)` at
/// the current frame — the set form behind a marquee
/// selection (W2-01). ONE snapshot per burst for the whole
/// set (the stamp keys on the set's hash, never per
/// figure), locked members skipped and named in the
/// status, stale indices dropped, input order and
/// duplicates irrelevant. Returns how many figures moved.
/// Refusals mirror [`nudge_figure`]: silent while playing;
/// setup mode refuses with the rest-pose sentence; a set
/// whose every member is locked refuses with the locked
/// sentence and takes no snapshot.
pub fn nudge_figures(core: &mut EditorCore, figs: &[usize], dx: f32, dy: f32) -> usize {
    if core.playing || (dx == 0.0 && dy == 0.0) {
        return 0;
    }
    let mut targets: Vec<usize> = figs
        .iter()
        .copied()
        .filter(|&fi| fi < core.project.figures.len())
        .collect();
    targets.sort_unstable();
    targets.dedup();
    if targets.is_empty() {
        return 0;
    }
    if core.setup_mode {
        core.status = "The rest pose has no root position — roots live in frame poses.".into();
        return 0;
    }
    let (movable, locked): (Vec<usize>, Vec<usize>) = targets
        .into_iter()
        .partition(|&fi| !core.project.figures[fi].locked);
    if movable.is_empty() {
        core.status = "Figure is locked — unlock it in the figures list.".into();
        return 0;
    }
    nudge_snapshot_set(core, &movable);
    let frame = core.current;
    for &fi in &movable {
        let pose = &mut core.project.frames[frame].poses[fi];
        pose.root = [pose.root[0] + dx, pose.root[1] + dy];
    }
    let n = movable.len();
    core.status = if n == 1 {
        let fi = movable[0];
        if locked.is_empty() {
            let (x, y) = {
                let p = &core.project.frames[frame].poses[fi];
                (p.root[0], p.root[1])
            };
            root_move_status(fi, x, y)
        } else {
            format!(
                "Nudged figure {} by ({dx:.1}, {dy:.1}); {} locked, skipped.",
                fi + 1,
                locked.len()
            )
        }
    } else if locked.is_empty() {
        format!("Nudged {n} figures by ({dx:.1}, {dy:.1}).")
    } else {
        format!(
            "Nudged {n} figures by ({dx:.1}, {dy:.1}); {} locked, skipped.",
            locked.len()
        )
    };
    n
}

/// The root write behind [`nudge_figure`]: move the
/// pose root verbatim and post the root sentence
/// ([`root_move_status`]).
fn write_root_move(core: &mut EditorCore, fi: usize, dx: f32, dy: f32) {
    let frame = core.current;
    let pose = &mut core.project.frames[frame].poses[fi];
    pose.root = [pose.root[0] + dx, pose.root[1] + dy];
    let (x, y) = (pose.root[0], pose.root[1]);
    core.status = root_move_status(fi, x, y);
}

/// The root-move status sentence, one wording for the
/// joint path's twin and the character ops: it names the
/// figure and where its root now sits.
fn root_move_status(fi: usize, x: f32, y: f32) -> String {
    format!("Nudged figure {} root to ({x:.0}, {y:.0}).", fi + 1)
}

/// The set form of [`nudge_snapshot`]: one snapshot per
/// burst for the whole set, keyed on the set's hash with
/// `fi == usize::MAX` marking the stamp as a set stamp.
fn nudge_snapshot_set(core: &mut EditorCore, figs: &[usize]) {
    let now = std::time::Instant::now();
    let key = figure_set_key(figs);
    let reuse = NUDGE_STAMP.with(|c| c.get()).map_or(false, |st| {
        st.fi == usize::MAX
            && st.set == key
            && st.frame == core.current
            && !st.setup
            && now.duration_since(st.at).as_millis() <= NUDGE_BURST_WINDOW_MS
    });
    if !reuse {
        core.snapshot_labeled("Nudge figures");
    }
    NUDGE_STAMP.with(|c| {
        c.set(Some(NudgeStamp {
            at: now,
            fi: usize::MAX,
            ni: None,
            frame: core.current,
            setup: false,
            set: key,
        }))
    });
}

/// A deterministic key for a sorted figure set (FNV-1a
/// over the indices): two bursts over the same set share
/// a stamp; any membership change starts a new one.
fn figure_set_key(figs: &[usize]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &fi in figs {
        for b in fi.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Resolve one arrow press under the user-settled rule:
/// a selected joint nudges ([`nudge_selected`]); else an
/// armed character or set moves whole ([`nudge_figure`] /
/// [`nudge_figures`]); else nothing — the caller steps
/// frames. The shell claims the key whenever
/// [`arrows_move_selection`] holds, so a refused write
/// (locked, playing) never falls through to a frame step:
/// frames step ONLY when nothing is selected.
pub fn nudge_arrows(core: &mut EditorCore, dx: f32, dy: f32) -> ArrowMove {
    if core.selected_node.is_some() {
        nudge_selected(core, dx, dy);
        return ArrowMove::Joint;
    }
    let targets = character_move_targets(core);
    match targets.len() {
        0 => ArrowMove::None,
        1 => {
            nudge_figure(core, targets[0], dx, dy);
            ArrowMove::Character
        }
        n => {
            nudge_figures(core, &targets, dx, dy);
            ArrowMove::Characters(n)
        }
    }
}

/// One [`sync_arrow_selection`] observation: the core's
/// selection, frame and project-swap counter at a poll.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ArrowWatch {
    fi: usize,
    ni: Option<usize>,
    frame: usize,
    swaps: usize,
}

/// The shell's per-tick arrow-selection watcher (the poll
/// calls it via IA-W2-02-2; every rule lives here). It
/// maintains the character move arm and the nudge burst
/// hygiene IA-RM15-4 asked the shell for and the Wave 1
/// integration did not land:
///
/// - A project swap releases the arm and the burst — the
///   indices the arm names belong to the old project.
/// - A frame change by any other means (timeline, goto,
///   keyframe jump, playback) releases the arm and the
///   burst: the user is navigating frames, so the arrows
///   are frame keys again. (Arrows themselves never change
///   the frame while armed — they nudge.)
/// - A selection change resets the burst; a change that
///   lands on a BARE character (the figure changed, or the
///   node was cleared on the same figure —
///   `select_figure`'s signature) arms that character.
///   Landing on a joint arms nothing: the joint rule owns
///   the arrows while a joint is selected.
///
/// The first observation only baselines: a freshly started
/// shell has selected nothing, and its arrows step frames.
pub fn sync_arrow_selection(core: &EditorCore) {
    let cur = ArrowWatch {
        fi: core.selected_figure,
        ni: core.selected_node,
        frame: core.current,
        swaps: core.project_swaps(),
    };
    let prev = ARROW_WATCH.with(|c| c.get());
    ARROW_WATCH.with(|c| c.set(Some(cur)));
    let Some(prev) = prev else {
        return;
    };
    if cur.swaps != prev.swaps {
        release_character_arm();
        reset_nudge_burst();
        return;
    }
    if cur.frame != prev.frame {
        release_character_arm();
        reset_nudge_burst();
    }
    if cur.fi != prev.fi || cur.ni != prev.ni {
        reset_nudge_burst();
        if cur.ni.is_none() && (cur.fi != prev.fi || prev.ni.is_some()) {
            note_character_selected(cur.fi);
        }
    }
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
///   sprite drag's line and the session modes' lines —
///   group move, path, marquee; W2-07) in the canvas text
///   stamp (R-A2-3).
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
    let dragging = core.dragging.is_some()
        || crate::stage_sprites::sprite_drag_active()
        || bend_drag_active()
        || group_drag_delta().is_some()
        || path_drag_active()
        || marquee_live_rect().is_some();
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
                    // Roadmap Wave 2 (W2-08): the bend
                    // grab's handle — a dot at the arc
                    // midpoint, the point a mid-segment
                    // press drags (see `bend_hit`). Setup
                    // mode draws straight and bends
                    // nothing, so it marks nothing.
                    if !core.setup_mode {
                        let shown = core
                            .evaluated_frame(core.display_frame())
                            .bends
                            .get(fi)
                            .and_then(|row| row.get(ni))
                            .copied()
                            .unwrap_or(0.0);
                        let mid = model::bend_mid(a, b, shown);
                        shape = shape.fill(
                            ngon(rt.to_screen(mid), 4.0 * s.max(0.6), 8),
                            accent.alpha(0.85),
                        );
                    }
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
    // Roadmap Wave 2 (W2-08): while a bend drag is live,
    // its handle rides the arc midpoint at the preview
    // bend — the figure pass already draws the segment
    // itself at that bend (see `bend_preview_apply`);
    // this dot is the grab point the pointer is holding.
    if let Some((fi, ni, bend)) = bend_drag_preview() {
        let pos = feedback_positions(core, fi);
        if let (Some(&a), Some(&b)) = (
            core.project
                .figures
                .get(fi)
                .and_then(|f| f.def.nodes.get(ni))
                .and_then(|n| n.parent)
                .and_then(|par| pos.get(par)),
            pos.get(ni),
        ) {
            let mid = model::bend_mid(a, b, bend);
            shape = shape.fill(
                ngon(rt.to_screen(mid), 6.0 * s.max(0.6), 8),
                accent.alpha(0.90),
            );
        }
    }
    let mut lines = drag_hud_lines(core);
    if let Some(line) = crate::stage_sprites::sprite_drag_hud(core) {
        lines.push(line);
    }
    // The session modes' lines (W2-07). The stage's
    // routing makes the sessions mutually exclusive in
    // practice, so at most one of these fires; the order
    // is fixed anyway so the plate is deterministic.
    if let Some(line) = group_drag_hud() {
        lines.push(line);
    }
    if let Some(line) = path_drag_hud(core) {
        lines.push(line);
    }
    lines.extend(marquee_hud_lines(core));
    if let Some(line) = bend_drag_hud() {
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

// ----------------------------------------------------------------
// View pan gesture + Space-held state (Roadmap Wave 2, W2-04)
// ----------------------------------------------------------------
//
// The view layer above could zoom (wheel, steppers) and
// centre (the overlay action), but the user could not
// simply GRAB the view and pull it. This section is the
// grab: a drag that writes only [`ViewNav::pan`], in
// element points, by exactly the pointer's element-point
// travel — a pan is a screen-space translation, so the
// folded transform shifts the picture by the drag delta
// with no stage conversion anywhere (the `with_view_nav`
// algebra: pan is already in screen space).
//
// The gesture has two doors, chosen from Rune's ACTUAL
// event model (verified in the pinned Rune sources, not
// assumed):
//
// * Middle-button drag. `rune::Drag` carries NO button
//   field — a drag looks identical whichever button
//   drives it. The button reaches the handler through
//   the element's opt-in: `El::pointer_buttons` admits
//   buttons besides the primary to `on_drag`, and
//   `Tx::button()` says which one drove it (Rune's own
//   docs name this exact use: "A middle-button drag
//   pans a canvas this way"). The shell's mount opts in
//   and routes by the button (IA-W2-04-1).
// * Space held + left drag. A `Drag` carries no
//   modifier state either (RM5's standing finding), so
//   Space is tracked as shell key state: the app-level
//   `on_key` sees the press, `El::on_key_up` on the root
//   sees the release — Rune's own docs describe this
//   pattern for exactly this feature ("A canvas that
//   pans while Space is held starts on Space's press in
//   `on_key` and stops here"). The two setters below are
//   the whole surface the shell needs (IA-W2-04-3).
//
// Space's standing job is play/pause, and it keeps it —
// as a TAP. The press no longer toggles: it arms the
// pan state, and the release toggles play only when no
// pan gesture consumed the hold ([`space_pan_release`]
// returns that verdict). A side benefit: OS key-repeat
// on a held Space used to re-toggle playback on every
// repeat; repeats now land on an already-armed state
// and do nothing.
//
// Precedence (the full table is in the W2-04 findings):
// the pan outranks every content gesture — it is view
// navigation, available in draw, camera, setup and
// playback like the wheel zoom, and it writes nothing
// to the project, so no content gesture can be harmed
// by losing a press to it. An ordinary left press is
// unaffected: marquee (left-drag on empty stage) and
// the path drag keep their priority within the left
// button's own chain, which the pan never joins unless
// Space is held.

/// Which door opened a view pan gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanSource {
    /// Middle-button drag (the element opted in via
    /// `El::pointer_buttons`; the shell routes by
    /// `Tx::button()`).
    Middle,
    /// Left drag while Space is held (the shell routes
    /// by [`space_pan_held`]).
    Space,
}

/// A view pan gesture in flight: the press point and the
/// view state it started from (a cancelled gesture
/// restores it — Rune's `Drag::cancelled` doctrine,
/// "put back whatever the drag moved"; what this drag
/// moves is the view).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ViewPanDrag {
    start_local: [f32; 2],
    start_nav: ViewNav,
    source: PanSource,
}

thread_local! {
    static VIEW_PAN_DRAG: Cell<Option<ViewPanDrag>> = const { Cell::new(None) };
    /// Space is currently held (its press armed the pan
    /// state and its release has not arrived).
    static SPACE_HELD: Cell<bool> = const { Cell::new(false) };
    /// The current Space hold has not been consumed by
    /// a pan gesture, so its release is still a tap —
    /// the play/pause toggle.
    static SPACE_TAP_LIVE: Cell<bool> = const { Cell::new(false) };
}

/// Record a Space press (shell `on_key`, IA-W2-04-3).
/// A press while already held is an OS key-repeat and
/// changes nothing.
pub fn space_pan_press() {
    if SPACE_HELD.with(|c| c.get()) {
        return;
    }
    SPACE_HELD.with(|c| c.set(true));
    SPACE_TAP_LIVE.with(|c| c.set(true));
}

/// Whether Space is currently held (the stage drag's
/// routing test for the pan's second door, and the
/// cursor's).
pub fn space_pan_held() -> bool {
    SPACE_HELD.with(|c| c.get())
}

/// Record a Space release (shell `on_key_up`,
/// IA-W2-04-3). Returns true when the hold was a TAP —
/// no pan gesture consumed it — so the shell runs the
/// tap's standing action (play/pause) itself. A release
/// with no matching press (the press went to a text
/// field, which bails before arming) returns false.
pub fn space_pan_release() -> bool {
    let held = SPACE_HELD.with(|c| c.get());
    SPACE_HELD.with(|c| c.set(false));
    let tap = held && SPACE_TAP_LIVE.with(|c| c.get());
    SPACE_TAP_LIVE.with(|c| c.set(false));
    tap
}

/// Stage drag, Down phase for the view pan: record the
/// press point (element points) and the view it starts
/// from. Returns true when the gesture is a pan — for
/// [`PanSource::Space`] only while Space is held (the
/// shell's routing asks first either way; the guard
/// keeps a stray call honest). A stale record is
/// dropped first, the session discipline every stage
/// gesture keeps. The pan itself applies from the
/// first Move; a press with no travel changes nothing.
pub fn view_pan_down(local: [f32; 2], source: PanSource) -> bool {
    VIEW_PAN_DRAG.with(|c| c.set(None));
    if source == PanSource::Space {
        if !space_pan_held() {
            return false;
        }
        // The hold is spent on a pan now: its release
        // is no longer a play/pause tap.
        SPACE_TAP_LIVE.with(|c| c.set(false));
    }
    VIEW_PAN_DRAG.with(|c| {
        c.set(Some(ViewPanDrag {
            start_local: local,
            start_nav: view_nav(),
            source,
        }))
    });
    true
}

/// Whether a view pan gesture owns the drag right now.
pub fn view_pan_active() -> bool {
    VIEW_PAN_DRAG.with(|c| c.get()).is_some()
}

/// The door the in-flight pan came through, if any.
pub fn view_pan_source() -> Option<PanSource> {
    VIEW_PAN_DRAG.with(|c| c.get()).map(|d| d.source)
}

/// Stage drag, Move phase for the view pan: the view's
/// pan becomes the gesture's start pan plus the
/// pointer's element-point travel since the press, so
/// the picture tracks the pointer one-to-one (grab the
/// world and pull). The zoom is the gesture's start
/// zoom throughout — a wheel turn mid-pan is not a
/// thing the shell can deliver to the same gesture.
/// Writes view state only. Returns true when the
/// session owns the gesture.
pub fn view_pan_move(local: [f32; 2]) -> bool {
    let Some(drag) = VIEW_PAN_DRAG.with(|c| c.get()) else {
        return false;
    };
    set_view_nav(ViewNav {
        zoom: drag.start_nav.zoom,
        pan: [
            drag.start_nav.pan[0] + (local[0] - drag.start_local[0]),
            drag.start_nav.pan[1] + (local[1] - drag.start_local[1]),
        ],
    });
    true
}

/// Stage drag, Up phase for the view pan: end the
/// gesture. A cancelled gesture (Escape, focus loss,
/// the pointer taken away) restores the view the press
/// started from — this drag DID move something (the
/// view), and Rune's cancel doctrine is to put it back;
/// the restore is view state only, so it costs no undo
/// step and touches no project data. Returns true when
/// a gesture was in flight.
pub fn view_pan_up(cancelled: bool) -> bool {
    let Some(drag) = VIEW_PAN_DRAG.with(|c| c.get()) else {
        return false;
    };
    VIEW_PAN_DRAG.with(|c| c.set(None));
    if cancelled {
        set_view_nav(drag.start_nav);
    }
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
///
/// Roadmap Wave 2 (W2-37) generalises the walk to per-figure
/// overrides ([`OnionFigureOverride`]): the implementation
/// lives in `onion_skins_with_overrides` below, and
/// `onion_skins` keeps this entry as its empty-overrides
/// delegation, so the behaviour described above is the
/// no-override behaviour every figure inherits.

/// The renderer's own future-frame onion tint, `0xRRGGBB` —
/// the value `onion_controls::FUTURE_ONION_RGB` mirrors (its
/// test pins the two equal). Named here so the per-figure
/// override path below reads as a default rather than a
/// second literal.
pub const FUTURE_ONION_RGB: u32 = 0x7BE495;

/// A per-figure onion-skin override (Roadmap Wave 2, W2-37):
/// the "depth" half of the onion model grows from one global
/// per-side pair to a per-figure refinement, exactly the way
/// Phase G grew the core's single count into per-side counts.
/// Every field is optional — `None` inherits, so a figure
/// with no override (or an all-`None` override, which the
/// panel normalises away) renders exactly as it did before
/// this type existed.
///
/// * `prev` / `next` — this figure's own per-side depths,
///   replacing the shell's global counts for this figure
///   only. Same `0..=3` range (the panel's `step_depth`
///   clamps its writes); `Some(0)` hides this figure on
///   that side while the others still draw.
/// * `past_tint` — replaces the past side's base colour
///   (the figure's own colour) for this figure: every past
///   ghost piece falls back to it (a node's own colour
///   still wins per piece, as it does over the base today).
/// * `future_tint` — replaces the future side's shared
///   green ([`FUTURE_ONION_RGB`]) for this figure.
///
/// Tints are per DIRECTION, not one per figure, because the
/// model being refined is directional: past and future have
/// never shared a tint rule (own colour vs shared green), so
/// a single per-figure tint would have to break one side's
/// rule or the other's. Depth and tint override
/// independently — a figure may pin only a deeper past, or
/// only a magenta future.
///
/// The override lives in shell session state
/// (`State::onion_ui`, owned by `onion_controls`) — the same
/// home, and the same discipline, as the global depths it
/// refines: no snapshot, no core write, nothing persisted.
/// The shell feeds the panel's vec to the stage through
/// [`set_onion_figure_overrides`] every frame (the
/// `VIEW_NAV` thread-local pattern); the type itself lives
/// here, beside the renderer that consumes it, the way
/// `hex_color` already crosses the same boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OnionFigureOverride {
    /// This figure's past depth, replacing the global
    /// `prev` for this figure only.
    pub prev: Option<u8>,
    /// This figure's future depth, replacing the global
    /// `next` for this figure only.
    pub next: Option<u8>,
    /// This figure's past base colour, replacing its own
    /// colour as the past ghosts' fallback.
    pub past_tint: Option<u32>,
    /// This figure's future base colour, replacing the
    /// shared green as the future ghosts' fallback.
    pub future_tint: Option<u32>,
}

impl OnionFigureOverride {
    /// Whether every field inherits — the panel stores
    /// `None`, never an all-`None` override, so this is the
    /// normalisation test for its writes.
    pub fn is_empty(&self) -> bool {
        self.prev.is_none()
            && self.next.is_none()
            && self.past_tint.is_none()
            && self.future_tint.is_none()
    }

    /// This figure's past depth under global `prev`.
    pub fn prev_depth(&self, global: u8) -> u8 {
        self.prev.unwrap_or(global)
    }

    /// This figure's future depth under global `next`.
    pub fn next_depth(&self, global: u8) -> u8 {
        self.next.unwrap_or(global)
    }

    /// This figure's past base colour: the tint override,
    /// or the figure's own colour — the pre-W2-37 rule.
    pub fn past_base(&self, figure_color: u32) -> u32 {
        self.past_tint.unwrap_or(figure_color)
    }

    /// This figure's future base colour: the tint override,
    /// or the shared green — the pre-W2-37 rule.
    pub fn future_base(&self) -> u32 {
        self.future_tint.unwrap_or(FUTURE_ONION_RGB)
    }
}

/// The override in force for figure `fi`: the stored one,
/// or the all-inheriting default when the vec is short or
/// the slot is empty. Stale slots past the figure list are
/// never read (every caller iterates the live figures), so
/// a vec that outlives a figure deletion is inert.
fn figure_override(overrides: &[Option<OnionFigureOverride>], fi: usize) -> OnionFigureOverride {
    overrides.get(fi).copied().flatten().unwrap_or_default()
}

thread_local! {
    /// The shell's per-figure onion overrides for the next
    /// [`stage_shape`] call, fed by `main.rs` from
    /// `State::onion_ui` (W2-37; the feed line is IA-W2-37-1).
    /// Empty — the default, and the state of every caller
    /// that never feeds it — renders the pre-W2-37 stage
    /// exactly. The `VIEW_NAV` pattern: session-only view
    /// state with no `State` field of its own in this
    /// module, never persisted, never seen by the engine.
    static ONION_FIGURE_OVERRIDES: std::cell::RefCell<Vec<Option<OnionFigureOverride>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The per-figure onion overrides the stage will read on
/// its next draw (see [`ONION_FIGURE_OVERRIDES`]).
pub fn onion_figure_overrides() -> Vec<Option<OnionFigureOverride>> {
    ONION_FIGURE_OVERRIDES.with(|c| c.borrow().clone())
}

/// Replace the per-figure onion overrides (the shell's
/// per-frame feed; tests use it too, and must clear after
/// themselves with [`clear_onion_figure_overrides`]).
pub fn set_onion_figure_overrides(overrides: Vec<Option<OnionFigureOverride>>) {
    ONION_FIGURE_OVERRIDES.with(|c| *c.borrow_mut() = overrides);
}

/// Drop every per-figure onion override the stage holds —
/// back to the pre-W2-37 global-only render.
pub fn clear_onion_figure_overrides() {
    ONION_FIGURE_OVERRIDES.with(|c| c.borrow_mut().clear());
}

/// [`onion_skins`] under per-figure overrides (W2-37): the
/// global depths and tints are the default every figure
/// inherits until its override says otherwise. An empty (or
/// all-`None`) `overrides` reproduces `onion_skins` entry
/// for entry — the draw order is unchanged (past rings,
/// nearest first, figures in index order inside each ring;
/// then future rings likewise), each ring simply runs to
/// the deepest depth any figure asks for on that side and
/// skips the figures whose own depth ends sooner. Ghost
/// alpha stays a function of the step distance only, so a
/// figure's nearer ghosts never change when another
/// figure's override deepens its own.
fn onion_skins_with_overrides(
    core: &EditorCore,
    prev: u8,
    next: u8,
    overrides: &[Option<OnionFigureOverride>],
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
    // Past side (W2-37): each figure's rings run to its own
    // effective depth — the override's, else the global
    // `prev`. The nearest ring is its own loop, as it was
    // when one count drove every figure; the deeper rings
    // run to the deepest depth any figure asks for and skip
    // the figures whose own depth has ended.
    if core.current > 0 {
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            let ov = figure_override(overrides, fi);
            if ov.prev_depth(prev) == 0 {
                continue;
            }
            let base = ov.past_base(project.figures[fi].color);
            out.push((pieces_at(core.current - 1, fi, base), base, 0x4D as f32 / 255.0));
        }
        let deepest = (0..project.figures.len())
            .map(|fi| figure_override(overrides, fi).prev_depth(prev))
            .max()
            .unwrap_or(0);
        for back in 2..=(deepest as usize) {
            if core.current < back {
                break;
            }
            for fi in 0..project.figures.len() {
                if project.figures[fi].hidden {
                    continue;
                }
                let ov = figure_override(overrides, fi);
                if (back as u8) > ov.prev_depth(prev) {
                    continue;
                }
                let base = ov.past_base(project.figures[fi].color);
                out.push((pieces_at(core.current - back, fi, base), base, 0x26 as f32 / 255.0));
            }
        }
    }
    // Future side (W2-37): the same per-figure reading over
    // `next`, with each figure's own base — its tint
    // override, else the shared green.
    let deepest_next = (0..project.figures.len())
        .map(|fi| figure_override(overrides, fi).next_depth(next))
        .max()
        .unwrap_or(0);
    for ahead in 1..=(deepest_next as usize) {
        let frame = core.current + ahead;
        if frame >= core.frame_count() {
            break;
        }
        let alpha = (if ahead == 1 { 0x4D } else { 0x26 }) as f32 / 255.0;
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            let ov = figure_override(overrides, fi);
            if (ahead as u8) > ov.next_depth(next) {
                continue;
            }
            let base = ov.future_base();
            out.push((pieces_at(frame, fi, base), base, alpha));
        }
    }
    out
}

/// Onion-skin piece sets with no per-figure overrides in
/// force — the pre-W2-37 entry, kept as the delegation the
/// shell's older callers and tests read through. Exactly
/// [`onion_skins_with_overrides`] with an empty override
/// list, so the two can never drift apart.
fn onion_skins(
    core: &EditorCore,
    prev: u8,
    next: u8,
) -> Vec<(Vec<stykanimate_rs::paint::StylePiece>, u32, f32)> {
    onion_skins_with_overrides(core, prev, next, &[])
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

/// Joint-limit arc ink (Roadmap Wave 2, W2-27): the
/// allowed span's green — the hue the future onion tint
/// and the segment palette already use for "go" — and
/// the forbidden remainder's red, the joint handles'
/// unselected red. Named separately from the onion
/// const: the two features share a hue, not a meaning,
/// and either may retune without moving the other.
pub const LIMIT_ARC_ALLOWED_RGB: u32 = 0x7BE495;
/// See [`LIMIT_ARC_ALLOWED_RGB`].
pub const LIMIT_ARC_FORBIDDEN_RGB: u32 = 0xFF5D5D;

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
    /// Golden-ratio grid inside the framed view under
    /// `guides` + `guides_frustum` (Roadmap Wave 2, W2-33,
    /// wiring IA-RM18-3): RM18 built and pinned the
    /// geometry in `stage_camera`; this flag is its
    /// toggle. Default OFF — an opt-in composition
    /// variant, never a default (the IA's default-flip
    /// question was explicitly not taken).
    pub guides_golden: bool,
    /// The framed view's two diagonals under `guides` +
    /// `guides_frustum` (W2-33, IA-RM18-3) — the diagonal
    /// companion to `guides_golden`, same default-off
    /// footing.
    pub guides_diagonals: bool,
    /// Stored guides under NO other gate (Roadmap Wave 2,
    /// W2-05): the project's user-placed guide lines
    /// (`Project::stored_guides`) are authored content,
    /// not composition aids, so they do not hide behind
    /// the master `guides` switch the way the derived
    /// families do — a user who placed a line expects it
    /// on stage. This flag is the family's own off switch
    /// (and the drag session's routing gate: the shell
    /// consults the session only while this is on, so a
    /// hidden guide can never be grabbed invisibly).
    pub guides_stored: bool,
    /// Rulers under NO other gate (W2-05): graduated
    /// stage-coordinate rulers along the stage's top and
    /// left edges — the surface stored guides are dragged
    /// from, useful on their own as a coordinate readout.
    /// Deliberately independent of `guides_stored`: the
    /// rulers may be hidden while guides show (the lines
    /// stay, their source chrome goes), and the drag
    /// session prices that case in its own docs (no ruler
    /// ⇒ no create-by-drag and no drag-to-delete; moving
    /// an existing guide still works).
    pub rulers: bool,
    /// Joint-limit arcs for the selected node (Roadmap
    /// Wave 2, W2-27): the selected node's limit partition
    /// drawn at its joint (see [`limit_arc_prims`]). A
    /// selection annotation, not a composition guide, so
    /// the master `guides` switch does NOT gate it — it
    /// carries its own opt-in flag, default OFF, toggled
    /// from the segment inspector's limit rows (where the
    /// numbers it draws live). With the flag off — or a
    /// selected node without limits — the stage draws
    /// exactly what it drew before this seat.
    pub limit_arcs: bool,
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
    /// both sub-toggles on, the golden/diagonal variants
    /// off (W2-33: opt-in, default-off by design), stored
    /// guides ON (W2-05: authored lines show unless their
    /// own toggle says otherwise — and a fresh project has
    /// none, so the default stage is still unchanged),
    /// rulers off (opt-in chrome), nothing selected, no
    /// 3D stage pass, and 1/1 onion depths — the depths
    /// the core's historical single `onion_frames` count
    /// defaulted to, so a default overlays value
    /// reproduces the pre-Phase-G stage.
    fn default() -> Self {
        StageOverlays {
            guides: false,
            guides_frustum: true,
            guides_safe: true,
            guides_golden: false,
            guides_diagonals: false,
            guides_stored: true,
            rulers: false,
            limit_arcs: false,
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

// ----------------------------------------------------------------
// Joint-limit arcs (Roadmap Wave 2, W2-27)
// ----------------------------------------------------------------
//
// For the selected node, the stage can draw the node's
// joint-limit partition at the node's joint: the directions
// the node's segment may point, measured from its parent's
// direction. The partition itself is the model's
// (`model::limit_spans` — derived from the same bounds
// `NodeDef::clamp_angle` enforces, so the drawing can never
// disagree with the solvers); this section only turns spans
// into stage-space line pieces. DISPLAY ONLY: nothing here
// clamps, snaps or writes — the set-rest clamp question
// (IA-AA9-1) is a standing user decision and is not built.

/// Which part of a limit-arc drawing one segment is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LimitArcKind {
    /// A piece of an allowed span: solid, full ink.
    Allowed,
    /// A piece of a forbidden span: one dash of the
    /// dashed rendering (the gaps are simply absent
    /// pieces, so the remainder reads as forbidden even
    /// where colour is hard to tell apart).
    Forbidden,
    /// A boundary spoke: joint to arc at one bound.
    Spoke,
    /// The straight-ahead tick: a short radial mark at
    /// relative angle 0 — the parent's own direction, the
    /// reference every limit is measured from.
    Straight,
}

/// One straight stage-space piece of a limit-arc drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitArcSeg {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub kind: LimitArcKind,
}

/// A node's limit-arc drawing in stage space: the pieces,
/// plus the arc points at the bounds (the spoke ends,
/// which the caller also dots so a bound reads at a
/// glance — and so a pinned joint, whose allowed span is
/// a single direction, still marks its one legal point).
#[derive(Clone, Debug, PartialEq)]
pub struct LimitArcPrim {
    pub segs: Vec<LimitArcSeg>,
    pub bound_points: Vec<[f32; 2]>,
}

/// Angular step of one allowed-arc piece, degrees: at the
/// radii involved (a segment's own length) a 4° chord is
/// visually smooth, and a full-circle arc costs 90 pieces.
const LIMIT_ARC_STEP_DEG: f32 = 4.0;
/// Forbidden-arc dash pattern, degrees: 5° drawn, then a
/// 3.5° gap.
const LIMIT_ARC_DASH_DEG: f32 = 5.0;
const LIMIT_ARC_GAP_DEG: f32 = 3.5;
/// Smallest arc radius drawn, stage units: a zero-length
/// segment still gets a readable annotation.
const LIMIT_ARC_MIN_RADIUS: f32 = 16.0;

/// The limit-arc drawing for one node, in stage space.
///
/// `joint` is the node's joint — its PARENT's evaluated
/// position, the point the segment pivots around;
/// `parent_dir_deg` is the parent segment's evaluated
/// direction in the model's angle convention (0° down,
/// positive clockwise — the root's direction is 0 by the
/// model's own FK convention, so a root child's limits
/// measure from straight down); `radius` is the segment's
/// evaluated length, so the arc is the locus of the node's
/// own position. The bounds are the node's `min_angle` /
/// `max_angle`. Returns `None` when the node carries no
/// limits — the caller then draws nothing, byte-identical
/// to the stage without this pass.
pub fn limit_arc_prims(
    joint: [f32; 2],
    parent_dir_deg: f32,
    min_angle: Option<f32>,
    max_angle: Option<f32>,
    radius: f32,
) -> Option<LimitArcPrim> {
    let spans = model::limit_spans(min_angle, max_angle)?;
    let radius = radius.max(LIMIT_ARC_MIN_RADIUS);
    let at = |rel_deg: f32, r: f32| -> [f32; 2] {
        let d = model::dir(parent_dir_deg + rel_deg);
        [joint[0] + d[0] * r, joint[1] + d[1] * r]
    };
    let mut segs: Vec<LimitArcSeg> = Vec::new();
    // Allowed spans: solid pieces at most one step wide.
    for &(a, b) in &spans.allowed {
        let mut t = a;
        while t < b {
            let next = (t + LIMIT_ARC_STEP_DEG).min(b);
            segs.push(LimitArcSeg { a: at(t, radius), b: at(next, radius), kind: LimitArcKind::Allowed });
            t = next;
        }
    }
    // Forbidden spans: dashes — a drawn dash, then an
    // absent gap, until the span ends.
    for &(a, b) in &spans.forbidden {
        let mut t = a;
        while t < b {
            let dash_end = (t + LIMIT_ARC_DASH_DEG).min(b);
            segs.push(LimitArcSeg { a: at(t, radius), b: at(dash_end, radius), kind: LimitArcKind::Forbidden });
            t += LIMIT_ARC_DASH_DEG + LIMIT_ARC_GAP_DEG;
        }
    }
    // Boundary spokes + bound dots at each stored bound,
    // normalised the way the spans were; two bounds that
    // normalise to the same direction (a pinned joint, or
    // the ±180 pair — one direction, canonicalised here
    // because `dir(180°)` and `dir(-180°)` differ by a
    // float hair) share one spoke.
    let mut bound_points: Vec<[f32; 2]> = Vec::new();
    let mut bound_rels: Vec<f32> = Vec::new();
    for bound in [min_angle, max_angle].into_iter().flatten() {
        if !bound.is_finite() {
            continue;
        }
        let rel = match model::norm_deg(bound) {
            r if r == -180.0 => 180.0,
            r => r,
        };
        if bound_rels.contains(&rel) {
            continue;
        }
        bound_rels.push(rel);
        let end = at(rel, radius);
        segs.push(LimitArcSeg { a: joint, b: end, kind: LimitArcKind::Spoke });
        bound_points.push(end);
    }
    // The straight-ahead tick at relative 0°, crossing
    // the arc so it reads at any radius.
    segs.push(LimitArcSeg {
        a: at(0.0, radius * 0.84),
        b: at(0.0, radius * 1.16),
        kind: LimitArcKind::Straight,
    });
    Some(LimitArcPrim { segs, bound_points })
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
            // Golden-ratio + diagonal variants (Roadmap
            // Wave 2, W2-33, wiring IA-RM18-3): RM18 built
            // and pinned this geometry in stage_camera but
            // left it undrawn for want of toggles; the
            // toggles are these two overlay flags, each
            // default-off, drawn here inside the frustum
            // gate in the same ink as the rest of the
            // framed-view family.
            if overlays.guides_golden {
                for prim in crate::rune_sec::stage_camera::frustum_golden_geometry(core) {
                    let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                    shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
                }
            }
            if overlays.guides_diagonals {
                for prim in crate::rune_sec::stage_camera::frustum_diagonals_geometry(core) {
                    let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                    shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
                }
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

    // Stored guides (Roadmap Wave 2, W2-05): the project's
    // user-placed lines, drawn here — right after the
    // derived composition families, in the guides pass's
    // own territory — but under their OWN overlay flag and
    // in their own ink: they are authored content, so the
    // master `guides` switch does not gate them (see
    // `StageOverlays::guides_stored`). The geometry (with
    // any live drag preview folded in) is
    // [`stored_guide_lines`]; each line carries its drag
    // handle on its ruler edge. Same 1.0 hairline width as
    // every guide on this stage.
    if overlays.guides_stored {
        let ink = hex_color(STORED_GUIDE_COLOR)
            .unwrap_or(Color::WHITE)
            .alpha(0.7);
        for (a, b, handle) in stored_guide_lines(core) {
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, ink);
            shape = shape.fill(ngon(rt.to_screen(handle), 5.0, 4), ink);
        }
    }

    // Rulers (W2-05): graduated stage-coordinate bands
    // along the stage's top and left edges, drawn in stage
    // space so the camera transform (rotation included)
    // and the shell view navigation carry them exactly
    // like the guides — a ruler is a graduated stage
    // border, not window chrome. The band is a
    // screen-constant thickness expressed in stage units
    // ([`ruler_band`]), ticks are [`ruler_ticks`], labels
    // ride the stage bitmap label machinery (digits are
    // in its font) at a screen-constant height.
    if overlays.rulers {
        let (w, h) = (project.stage[0], project.stage[1]);
        if w > 0.0 && h > 0.0 {
            let theme = crate::rune_sec::app_theme::app_theme(!core.light_theme);
            let band = ruler_band(s);
            // Band plates: above the top edge, left of
            // the left edge.
            for rect in [
                [[0.0, -band], [w, -band], [w, 0.0], [0.0, 0.0]],
                [[-band, 0.0], [0.0, 0.0], [0.0, h], [-band, h]],
            ] {
                let screen: Vec<[f32; 2]> =
                    rect.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.fill(screen.clone(), theme.card);
                for i in 0..4 {
                    shape = shape.line(
                        screen[i],
                        screen[(i + 1) % 4],
                        1.0,
                        LineCap::Round,
                        theme.card_border,
                    );
                }
            }
            let tick_ink = Color::WHITE.alpha(0.4);
            for tick in ruler_ticks(project.stage, s) {
                let (a, b) = ruler_tick_line(&tick, band);
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, tick_ink);
                if let Some(text) = &tick.label {
                    let (centre, angle) = ruler_label_placement(&tick, band);
                    for tp in crate::rune_sec::stage_text::label_boxes_alpha(
                        centre,
                        angle,
                        text,
                        ruler_label_size(s),
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

    // Onion skins (Wave 12, SPEC-ONION): styled pieces, drawn
    // per piece at the piece's own width; the colour is the
    // piece's colour with its alpha composed as piece alpha ×
    // ghost falloff (multiplication, never replacement — the
    // hex colours are opaque, so the product is set directly).
    for (pieces, _base, ghost_alpha) in onion_skins_with_overrides(
        core,
        overlays.onion_prev,
        overlays.onion_next,
        &onion_figure_overrides(),
    ) {
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
        let bends = bend_preview_apply(
            fi,
            display,
            if core.setup_mode {
                Vec::new()
            } else {
                core.evaluated_frame(display)
                    .bends
                    .get(fi)
                    .cloned()
                    .unwrap_or_default()
            },
        );
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
        // Joint-limit arcs (Roadmap Wave 2, W2-27): the
        // selected node's limit partition at its joint,
        // under its own opt-in overlay flag — drawn here,
        // inside the figure pass, because `pos` is the
        // evaluated geometry the arc must agree with (the
        // parent's DRAWN direction, channels and pose scale
        // included), and drawn BEFORE the joint handles so
        // the handles stay crisp on top. Display only: the
        // partition is `model::limit_spans`, the same arc
        // `clamp_angle` enforces; the flag off, no
        // selection, or a node without limits adds nothing
        // to the shape.
        if overlays.limit_arcs && fi == core.selected_figure {
            let arc = core.selected_node.and_then(|ni| {
                let node = fig.def.nodes.get(ni)?;
                let p = node.parent?;
                // The parent's drawn direction: from its
                // own parent's evaluated position, or the
                // model's root direction (0°, straight
                // down) when the parent IS the root —
                // `evaluate` roots every chain at absolute
                // angle 0, so a root child's limits measure
                // from straight down.
                let parent_dir = match fig.def.nodes[p].parent {
                    Some(gp) => model::angle_of([
                        pos[p][0] - pos[gp][0],
                        pos[p][1] - pos[gp][1],
                    ]),
                    None => 0.0,
                };
                let seg_len = ((pos[ni][0] - pos[p][0]).powi(2)
                    + (pos[ni][1] - pos[p][1]).powi(2))
                .sqrt();
                limit_arc_prims(pos[p], parent_dir, node.min_angle, node.max_angle, seg_len)
            });
            if let Some(prim) = arc {
                let allowed = hex_color(LIMIT_ARC_ALLOWED_RGB).unwrap_or(Color::WHITE);
                let forbidden = hex_color(LIMIT_ARC_FORBIDDEN_RGB).unwrap_or(Color::WHITE);
                for seg in &prim.segs {
                    let (width, color) = match seg.kind {
                        LimitArcKind::Allowed => ((2.0 * s).max(1.0), allowed.alpha(0.95)),
                        LimitArcKind::Forbidden => ((1.5 * s).max(1.0), forbidden.alpha(0.6)),
                        LimitArcKind::Spoke => (1.0, allowed.alpha(0.55)),
                        LimitArcKind::Straight => (1.0, Color::WHITE.alpha(0.45)),
                    };
                    shape = shape.line(rt.to_screen(seg.a), rt.to_screen(seg.b), width, LineCap::Round, color);
                }
                for pt in &prim.bound_points {
                    shape = shape.fill(ngon(rt.to_screen(*pt), 2.5 * s.max(0.6), 8), allowed.alpha(0.95));
                }
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
    // Sprite instances (Wave 13, A2 + A6): the panel-selected
    // sprite gets corner marks and transform handles, and
    // every sprite gets its asset name laid over its quad by
    // `stage_text::sprite_label_boxes` (the fitted in-quad
    // layout — the integrator's reconciliation of A2's label
    // spec with A6's shipped layout: the prim decides WHICH
    // sprites are labelled and with what name; A6's layout
    // places the glyphs). Same slot as the placeholders:
    // over the figures, under the 3D session figure.
    // Roadmap Wave 2 (W2-06, IA-RM22-4): the placeholder
    // quad itself — the Fill/Outline prims, the translucent
    // accent stand-in for artwork the canvas could not
    // draw — RETIRES here: placed sprites now render their
    // real bitmaps in the element layer
    // (`stage_sprite_layers`, mounted over this canvas by
    // the shell), transformed identically to the export
    // composite. The prim stream still produces Fill/Outline
    // (stage_sprites' contract, pinned there); this consumer
    // no longer paints them. The marks, handles and labels
    // below are selection/identification chrome, not
    // placeholder, and stay canvas-drawn over the artwork.
    {
        let accent = hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
        for prim in crate::stage_sprites::sprite_prims(&core.project, display, overlays.sprite_selected) {
            use crate::stage_sprites::SpritePrim;
            match prim {
                SpritePrim::Fill { .. } | SpritePrim::Outline { .. } => {
                    // Retired (W2-06): the bitmap element
                    // layer paints the sprite's real artwork;
                    // see the block comment above.
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
    // Roadmap Wave 2 (W2-45, IA-RM2-4): this loop paints
    // only the BITMAP-routed bands — under the Shaped font
    // style a band whose line resolves through RM2's font
    // engine is claimed by the element layer
    // (`stage_text::stage_subtitle_layers`, mounted over
    // this canvas by the shell) and `subtitle_geometry`
    // omits it here, so the two surfaces partition the
    // bands and never double-draw. Under the Pixel style
    // this loop paints the whole stream, exactly as before.
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
    // Marquee multi-selection (Roadmap Wave 2, W2-01):
    // the figure set's bounding outlines (offset by a
    // group drag's live displacement while one is in
    // flight) and the rubber band of a marquee drag in
    // flight — over the content passes, under the
    // feedback pass, whose HUD annotates the same stage.
    // Both halves read session thread-locals only, so an
    // idle stage draws exactly what it drew before.
    let shape = draw_marquee_selection(shape, core, &rt, s);
    // Transform gizmo (Roadmap Wave 2, W2-03): the
    // selected figure's / marquee set's frame with its
    // move grip and rotate knob — over the marquee
    // outlines it frames, under the feedback pass,
    // whose HUD annotates the same stage. Draws
    // nothing without a target.
    let shape = crate::stage_gizmo::draw_gizmo(shape, core, &rt, s);
    // Navigator minimap (Roadmap Wave 2, W2-04): the
    // whole scene in miniature with the viewport quad,
    // pinned to the canvas's top-left corner — chrome
    // over the content passes, under the feedback pass
    // whose HUD annotates the same stage. Screen-space:
    // it takes the element size, not the stage transform.
    // Hides itself on small elements (see the module).
    let shape = navigator::draw_navigator(shape, core, el_size);
    // Stage feedback (Roadmap Wave 1, RM15 — P-3): hover
    // highlight, the live drag HUD and the pointer
    // coordinate readout — after even the motion-path
    // markers, since the HUD must never be covered by the
    // content it annotates. Draws nothing while the
    // pointer probe is empty.
    let shape = draw_stage_feedback(shape, core, &rt, el_size, s);
    shape
}

// ----------------------------------------------------------------
// Stage sprite bitmap layer (Roadmap Wave 2, W2-06 — RM22's
// IA-RM22-4, on the RM22 spike's verdict)
// ----------------------------------------------------------------
//
// The canvas cannot paint bitmaps (Rune's `Shape` has no image
// primitive — RM22's spike, findings rm22 §1), but the element
// tree can: `El::pixels` paints decoded pixels into the
// element's rect, `El::rotate` turns the element about
// `El::origin`, and `El::opacity` blends it. A sprite
// instance's quad is exactly an axis-aligned rect (the asset's
// scaled pixel size) rotated about the key's pivot — every
// term has an element counterpart, so a placed sprite renders
// its REAL artwork on stage, transformed identically to the
// export composite. Segment media has no such counterpart
// (skew, bend strips, source crops) and is NOT mounted here —
// it keeps its canvas placeholder in `stage_shape`, per the
// spike. Text instances are glyphs, not bitmaps, and stay
// with stage_text.
//
// The placement algebra (all in canvas element points — the
// space `RotTransform::to_screen` produces, which the mounted
// layer shares with the canvas: the layer is pinned to the
// stage wrap's box, and the wrap's origin is the canvas's):
//
// * size    = (asset_w · key.scale, asset_h · key.eff_scale_y())
//             · rt.vt.scale — the media pass's quad extents,
//             in points.
// * origin  = the pivot's position inside the element rect,
//             in size fractions. Flips are baked into a
//             mirrored pixel variant (the element path has no
//             mirror), so a flipped axis reads its pivot from
//             the other side: (1 − pivot).
// * the origin point is the one point rotation leaves fixed,
//             so the element's layout box is placed at
//             rt.to_screen(key position) − origin · size, and
//             its rotation is (key.angle − camera angle): the
//             canvas maps a stage offset about the anchor
//             through R(−θ) (RotTransform::to_screen turns by
//             −θ about the view centre), the media pass builds
//             the quad by turning the rect by the key angle φ,
//             and Rune's `Turn` matrix is the same y-down
//             rotation — R(−θ)·R(φ) = R(φ−θ), so one element
//             rotation about the anchor reproduces the quad
//             corner-for-corner (pinned in the tests below).
// * opacity = the quad's alpha (the sampled key opacity,
//             already clamped by the media pass — the same
//             value the retired placeholder factored).
//
// Z-order: `stage_sprite_layers` yields ONE container for the
// shell to mount inside the stage wrap directly after the
// canvas child — over the canvas and under the toast stack
// and camera pill, which mount after it; sprites keep the
// media pass's painter's order among themselves. Within the
// canvas's own stack, the marks/handles/labels the sprite
// pass still draws sit in the canvas's sprite slot (over the
// figures, under the 3D figure and the FX/text/weather
// passes), while the bitmaps ride the element layer over the
// whole canvas — a documented divergence from the canvas
// slot order, of the same class A6 recorded for the
// placeholder slot vs the raster; the spike's design fixes
// the layer above the figure canvas and below the handle
// prims' element-level peers.
//
// The elements carry no handlers, so they never shadow the
// canvas's drag/press routing (Rune hit testing reaches
// elements with handlers; the toast stack already overlays
// the canvas on the same terms). `instant_changes` on the
// container and on each sprite lands position, rotation and
// opacity at once instead of springing them — the canvas
// draws the same transforms immediately, and a springing
// bitmap would lag the artwork behind its own corner marks
// during playback. The container clips its children to the
// stage box (`clip_rounded(0.0)`), so a sprite hanging off
// the view's edge cannot bleed over the neighbouring panels
// — the canvas's own prims are bounded by the canvas element
// the same way.

/// Where one sprite's bitmap element goes: everything the
/// element build needs, computed from the sprite's quad, its
/// sampled key and the display transform (see the section
/// docs for the algebra).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteBitmapPlacement {
    /// Index into `Project.sprites` (the element key's suffix).
    pub sprite: usize,
    /// Index into `Project.assets` (the bitmap's source).
    pub asset: usize,
    /// The sampled key's flips — baked into the pixel
    /// variant, and already accounted for in `origin`.
    pub flip_x: bool,
    /// See `flip_x`.
    pub flip_y: bool,
    /// Layout-box top-left in canvas element points (the
    /// box's position before its rotation about `origin`).
    pub top_left: [f32; 2],
    /// Element size in points: the asset's scaled pixel
    /// size under the display transform's scale.
    pub size: [f32; 2],
    /// Rotation pivot in size fractions: the key's pivot,
    /// read from the far side on a flipped axis.
    pub origin: [f32; 2],
    /// Element rotation in radians (clockwise, y-down):
    /// key angle minus camera angle.
    pub rotate_rad: f32,
    /// The quad's alpha: the sampled key opacity, clamped
    /// by the media pass.
    pub alpha: f32,
}

/// The placement of one sprite quad under a display
/// transform — the pure algebra of the section docs,
/// separated from the element build so the tests can pin
/// the presented corners against the canvas transform
/// directly. `asset_wh` is the asset's decoded pixel size.
pub fn sprite_bitmap_placement(
    quad: &crate::stage_sprites::SpriteQuad,
    key: &model::InstanceKey,
    asset_wh: (u32, u32),
    rt: &crate::rune_sec::stage_camera::RotTransform,
) -> SpriteBitmapPlacement {
    let s = rt.vt.scale;
    let size = [
        asset_wh.0 as f32 * key.scale * s,
        asset_wh.1 as f32 * key.eff_scale_y() * s,
    ];
    let origin = [
        if key.flip_x { 1.0 - key.pivot_x } else { key.pivot_x },
        if key.flip_y { 1.0 - key.pivot_y } else { key.pivot_y },
    ];
    let anchor = rt.to_screen([key.x, key.y]);
    SpriteBitmapPlacement {
        sprite: quad.sprite,
        asset: quad.asset,
        flip_x: key.flip_x,
        flip_y: key.flip_y,
        top_left: [
            anchor[0] - origin[0] * size[0],
            anchor[1] - origin[1] * size[1],
        ],
        size,
        origin,
        rotate_rad: (key.angle - rt.angle_deg).to_radians(),
        alpha: quad.alpha,
    }
}

/// Every placed sprite's placement at the core's display
/// frame, in the media pass's painter's order (the order
/// `stage_sprites::sprite_quads` returns). A quad whose key
/// no longer samples, whose asset no longer decodes, or
/// whose placement is not finite is skipped — the layer
/// shows the sprites it can place exactly, never an
/// approximation.
pub fn sprite_bitmap_placements(
    core: &EditorCore,
    el_size: [f32; 2],
) -> Vec<SpriteBitmapPlacement> {
    let rt = display_transform(core, el_size);
    let display = core.display_frame();
    let mut out = Vec::new();
    for quad in crate::stage_sprites::sprite_quads(&core.project, display) {
        let Some(inst) = core.project.sprites.get(quad.sprite) else {
            continue;
        };
        let Some(key) = model::InstanceKey::sample(&inst.keys, display) else {
            continue;
        };
        let Some(asset_wh) = stykanimate_rs::media::png_dims(&core.project, quad.asset) else {
            continue;
        };
        let p = sprite_bitmap_placement(&quad, &key, asset_wh, &rt);
        if p.top_left.iter().chain(p.size.iter()).all(|v| v.is_finite())
            && p.size[0] > 0.0
            && p.size[1] > 0.0
        {
            out.push(p);
        }
    }
    out
}

thread_local! {
    /// Baked sprite bitmaps, one per (asset, flip pair) —
    /// the element layer's counterpart of the Asset Manager
    /// thumbnails' cache, with the same validity discipline:
    /// an entry is served while the asset at that index
    /// still stores the same byte length, and re-baked when
    /// that changes. `None` entries are cached too (an
    /// undecodable asset is re-asked only when its bytes
    /// change). The unflipped variant and each flipped
    /// variant are separate entries — flips are baked
    /// mirrored copies, per the spike (the element path has
    /// no mirror) — and a flip is part of the sampled key,
    /// so a track that flips mid-animation swaps variants
    /// rather than re-baking per frame.
    static SPRITE_BITMAPS: RefCell<
        std::collections::HashMap<(usize, bool, bool, usize), Option<rune::Image>>,
    > = RefCell::new(std::collections::HashMap::new());
}

/// The baked Rune image for asset `asset` under the flip
/// pair, at the asset's native resolution (the renderer
/// scales it into the element's rect — no resize is baked).
/// `None` when the asset is missing or does not decode: a
/// sprite without a bitmap is not mounted, never a
/// placeholder.
fn sprite_bitmap(
    project: &model::Project,
    asset: usize,
    flip_x: bool,
    flip_y: bool,
) -> Option<rune::Image> {
    let stored = project.assets.get(asset)?;
    let cache_key = (asset, flip_x, flip_y, stored.png.len());
    if let Some(hit) = SPRITE_BITMAPS.with(|c| c.borrow().get(&cache_key).cloned()) {
        return hit;
    }
    let baked = bake_sprite_bitmap(project, asset, flip_x, flip_y);
    SPRITE_BITMAPS.with(|c| {
        c.borrow_mut().insert(cache_key, baked.clone());
    });
    baked
}

/// Bake one variant: the shared decode
/// (`media::asset_pixels`, IA-RM22-1), mirrored with the
/// `image` crate's flips when the key asks — a mirrored
/// copy of the same decode the exporter samples with
/// flipped coordinates, so the two surfaces show the same
/// picture. The Rune image is made through a throwaway
/// `ImageCache` whose decode is waited out, so the
/// returned handle owns its pixels after the cache drops
/// (the Home thumbnails' recipe).
fn bake_sprite_bitmap(
    project: &model::Project,
    asset: usize,
    flip_x: bool,
    flip_y: bool,
) -> Option<rune::Image> {
    let (w, h, rgba) = stykanimate_rs::media::asset_pixels(project, asset)?;
    if w == 0 || h == 0 {
        return None;
    }
    let raw = if !flip_x && !flip_y {
        (*rgba).clone().into_raw()
    } else {
        let mut img = (*rgba).clone();
        if flip_x {
            img = image::imageops::flip_horizontal(&img);
        }
        if flip_y {
            img = image::imageops::flip_vertical(&img);
        }
        img.into_raw()
    };
    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
    let image = cache.rgba(w, h, raw);
    image.wait().ok()?;
    Some(image)
}

/// The stage sprite bitmap layer as a mount list for the
/// shell: ONE container element when at least one sprite is
/// placed (and decodable) at the display frame, ZERO
/// elements otherwise — so a sprite-less frame's tree is
/// exactly the tree it has always been. The container is
/// pinned to the stage wrap's box (its outermost placement
/// call, the toast stack's discipline), clips its children
/// to that box, and carries the counted key
/// `stage-sprite-layer`; each sprite element carries the
/// variable key family `stage-sprite-{sprite}` (outside the
/// key counter, the asset-row precedent). Mounting — one
/// `.children(...)` line in main.rs, between the canvas
/// child and the toast child of the stage wrap — is
/// IA-W2-06-1.
pub fn stage_sprite_layers(core: &EditorCore, el_size: [f32; 2]) -> Vec<El> {
    let placements = sprite_bitmap_placements(core, el_size);
    if placements.is_empty() {
        return Vec::new();
    }
    let mut layer = col()
        .key("stage-sprite-layer")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .clip_rounded(0.0)
        .instant_changes();
    let mut mounted = 0;
    for p in &placements {
        let Some(image) = sprite_bitmap(&core.project, p.asset, p.flip_x, p.flip_y) else {
            continue;
        };
        layer = layer.child(
            el()
                .key(format!("stage-sprite-{}", p.sprite))
                .absolute(p.top_left[0], p.top_left[1])
                .w(p.size[0])
                .h(p.size[1])
                .origin(p.origin[0], p.origin[1])
                .rotate(p.rotate_rad)
                .opacity(p.alpha)
                .instant_changes()
                .pixels(image.pixels()),
        );
        mounted += 1;
    }
    if mounted == 0 {
        return Vec::new();
    }
    vec![layer]
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

/// Whether a path drag is currently in flight (the
/// feedback pass's drag test, W2-07: the session owns the
/// gesture, so hover and the corner readout stand down
/// while it lives — RM15's "hover is no-drag feedback"
/// rule, extended to the session drags).
pub fn path_drag_active() -> bool {
    PATH_DRAG.with(|c| c.borrow().is_some())
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

// ---- Motion-path point insert/delete (Roadmap Wave 2, W2-36) ----
//
// RM16 made the path's points draggable; this section adds
// and removes points. The model is the one the drag already
// writes: a point is a frame's authored root for the figure
// (dense frames — there is no sparse key store), so the two
// gestures are the dense equivalents of a curve editor's
// insert/delete:
//
// * INSERT — a double-press on a SEGMENT (Rune's `Drag`
//   carries the click count, so the shell can tell the
//   second press of a pair from the first) splits it:
//   [`EditorCore::insert_path_point`] lands a new frame at
//   the split whose root for this figure is the pressed
//   point projected onto the segment — the path does not
//   move, it gains a point, and the point is immediately
//   the current frame's (the pose drag owns it from
//   there). The insert fires on the double-press's Down:
//   it is a press, not a drag, so there is no session and
//   nothing to preview — the gesture's Move/Up phases find
//   no record and pass through, exactly as a consumed
//   path-point press's do. The first press of the pair was
//   an ordinary click (a seek if it landed on a point).
// * DELETE — the selected point is the CURRENT frame's
//   point for the selected figure (a path press already
//   seeks the playhead onto a point: that is how a point
//   is selected), and the doors are the frame-ops panel's
//   path row and the Delete key, both filed as IAs beside
//   this section's core door, [`delete_selected_path_point`].
//   The op re-interpolates: the root becomes the midpoint
//   of its neighbours, so the path runs straight through
//   where the point was. Endpoints and already-straight
//   points refuse in the op; the read-side guard
//   ([`selected_path_point_deletable`]) mirrors the op so
//   the panel button is live exactly when a press lands.
//
// Both ops take exactly one labelled snapshot on success
// and none on refusal, through the core — this section
// adds no writes of its own.

/// The path segment a press at stage point `p` falls on:
/// `(segment's first frame, t along it, the pressed point
/// projected onto it)`. Segments join the integer frame
/// points (the markers) — the half-frame samples between
/// them are display only, exactly as they are unpickable
/// in [`path_point_at`]. Behind the drawing's own gates
/// (overlay on, not setup mode, figure not layers-hidden).
/// A press whose nearest point on the winning segment
/// sits within the segment's end 2% is a POINT press, not
/// a split, and returns `None` — as does any press farther
/// than [`PATH_PICK_TOLERANCE`] from every segment.
pub fn path_segment_at(core: &EditorCore, p: V2) -> Option<(usize, f32, V2)> {
    if !core.show_paths || core.setup_mode {
        return None;
    }
    let fi = core.selected_figure;
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return None;
    }
    let pts = motion_path_points(core, fi);
    let mut best: Option<(usize, f32, V2, f32)> = None;
    for f in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[f], pts[f + 1]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let len2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if len2 > 0.0 {
            (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if d <= PATH_PICK_TOLERANCE && best.map_or(true, |(_, _, _, bd)| d < bd) {
            best = Some((f, t, q, d));
        }
    }
    let (f, t, q, _) = best?;
    if !(0.02..=0.98).contains(&t) {
        return None;
    }
    Some((f, t, q))
}

/// Stage drag, Down phase for a path-point insert: the
/// second press of a double-press (`double` — the shell
/// passes `Drag::clicks >= 2`) on a path segment splits
/// it, landing one [`EditorCore::insert_path_point`].
/// Returns true when the press was consumed. A single
/// press is never this gesture (it belongs to the path
/// drag / pose drag routing below), a stale path-drag
/// record is dropped first as in [`path_drag_down`], and
/// playback refuses (the path drag's own rule). No session
/// is started: the insert is complete on the press.
pub fn path_insert_down(core: &mut EditorCore, p: V2, double: bool) -> bool {
    if !double {
        return false;
    }
    PATH_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing {
        return false;
    }
    let Some((after, t, point)) = path_segment_at(core, p) else {
        return false;
    };
    let fi = core.selected_figure;
    core.insert_path_point(after, fi, t, point)
}

/// Whether the selected path point — the current frame's
/// point for the selected figure — can be deleted right
/// now: the drawing's gates plus
/// [`EditorCore::delete_path_point`]'s success condition,
/// read-side (the op's own checks still run on the press).
/// This is the frame-ops button's live/inert guard and the
/// Delete key's claim test, so a door is never live for a
/// press that would only refuse.
pub fn selected_path_point_deletable(core: &EditorCore) -> bool {
    if !core.show_paths || core.setup_mode {
        return false;
    }
    let fi = core.selected_figure;
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return false;
    }
    let Some(mid) = core.path_point_reinterpolation(core.current, fi) else {
        return false;
    };
    let Some(cur) = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .map(|pose| pose.root)
    else {
        return false;
    };
    (cur[0] - mid[0]).hypot(cur[1] - mid[1]) > EditorCore::PATH_POINT_STRAIGHT_EPS
}

/// The delete door the panel button and the Delete key
/// share: delete the selected path point (see the section
/// docs) through [`EditorCore::delete_path_point`] —
/// one labelled snapshot when it lands. Behind the same
/// gates as [`selected_path_point_deletable`]; a gated or
/// refusing press writes nothing and leaves the status
/// to the op (a gated press sets no status at all — the
/// doors render inert then, so it should not be reached).
pub fn delete_selected_path_point(core: &mut EditorCore) -> bool {
    if !core.show_paths || core.setup_mode {
        return false;
    }
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(core.selected_figure).copied().unwrap_or(false) {
        return false;
    }
    let fi = core.selected_figure;
    let frame = core.current;
    core.delete_path_point(frame, fi)
}

// ----------------------------------------------------------------
// Bend drag (Roadmap Wave 2, W2-08 — IA-RM15-5 remainder,
// R-A2-11)
// ----------------------------------------------------------------
//
// RM15 built segment picking and filed the segment press
// (IA-RM15-5, landed: a segment grab starts the child
// joint's Aim drag through `begin_drag_at`). Its own
// "not built" list named the remainder: "bend drag needs
// IA-RM15-5's segment press first (a mid-segment grab is
// its gesture)". This section is that gesture.
//
// THE HIT TARGET (distinct from the joint drag by
// construction): a joint press is taken by the core's hit
// test before this session is ever consulted, and a press
// near a segment's ENDS keeps the landed IA-RM15-5
// behaviour (the Aim drag). The bend grab owns the
// segment's MIDDLE: the central band of the chord
// ([`BEND_GRAB_T_MIN`]..=[`BEND_GRAB_T_MAX`]) plus the
// ARC MIDPOINT handle — the point `model::bend_mid`
// draws the bulge through, which for a bent segment sits
// OFF the chord, so a chord-distance test alone would
// miss the very point the user sees and reaches for.
//
// THE GEOMETRY is the exact inverse of the renderer's:
// `bend_mid` pushes the chord midpoint along the left
// normal by (len/2)·tan(θ/4), so the pointer's signed
// offset along that same normal gives θ =
// 4·atan(off/(len/2)) — [`bend_from_pointer`]. The value
// clamps to [`BEND_LIMIT`], the panel/MCP clamp
// (`tool_set_bend` clamps ±0.7; `cycle_bend`'s steps top
// out at ±0.7), and honours the session snap engine
// (RM15's `SnapConfig`: when enabled, the bend's degrees
// round to the angle step, as an aim drag's direction
// does — the settings surface that will own the switch
// is IA-RM15-7's remainder).
//
// THE SESSION mirrors the path drag's discipline (and
// the gizmo's): Down records and selects, Move updates
// only the session's preview value — NOTHING is written
// while the gesture lives (the figure pass applies the
// preview through [`bend_preview_apply`], so the arc the
// user sees is the renderer's own construction at the
// preview bend) — and Up settles as exactly ONE
// `snapshot_labeled("Bend segment")` plus the one row
// write, in `tool_set_bend`'s own resize-and-store
// shape, on the GRAB frame. A release that travelled
// nowhere or lands back on the starting bend spends
// nothing; a cancelled gesture drops the record (no
// snapshot was taken at Down, so there is nothing to
// roll back — cleaner than the core drags, which must
// undo their Down snapshot). Locked figures refuse at
// Down WITHOUT a status write: the shell's fall-through
// reaches the Aim arm, whose `begin_drag_at` posts the
// standing locked sentence, exactly as a locked segment
// press reads today. Setup mode refuses (bends are
// frame data; a rest pose draws straight), playback
// refuses (the pose drag's own rule).
//
// The settle writes the STORED row at the grab frame,
// like every bend author (`cycle_bend`, the steppers,
// MCP `set_bend`). Where a Bend graph channel targets
// the node, the channel still wins on display over the
// stored row — the standing caveat of all bend
// authoring, unchanged here.
//
// Routing (main.rs) is IA-W2-08-1: the shell consults
// this session in the press stack's joint-miss branch,
// after the sprite-handle pick and before the
// segment→Aim arm. No editor.rs change is needed: the
// extraction this seat's board line anticipated
// (`begin_drag_at`) already landed with IA-RM15-5.

/// The bend drag's clamp, radians: the panel/MCP bend
/// limit (±0.7 — `tool_set_bend`'s clamp and
/// `cycle_bend`'s outermost steps).
pub const BEND_LIMIT: f32 = 0.7;

/// The chord band a press must land in to read as a
/// bend grab (projection parameter `t` along the
/// segment, 0 at the parent joint, 1 at the child): the
/// segment's central 40%. Outside it, a segment press is
/// the Aim grab IA-RM15-5 landed.
pub const BEND_GRAB_T_MIN: f32 = 0.30;
/// See [`BEND_GRAB_T_MIN`].
pub const BEND_GRAB_T_MAX: f32 = 0.70;

/// How far, in stage units, the pointer must travel
/// from the press point before a bend release reads as
/// a drag rather than a click on the segment (a click
/// selects; it does not bend).
pub const BEND_DRAG_SLOP: f32 = 2.0;

/// A settled bend within this many radians of the
/// starting bend spends no undo step: the gesture
/// travelled but landed back where the segment already
/// was (a drag along the chord, or out and back).
pub const BEND_SETTLE_EPSILON: f32 = 0.005;

/// The bend a pointer at stage point `p` asks of the
/// segment `a`→`b`: the exact inverse of
/// `model::bend_mid` (see the section docs), clamped to
/// [`BEND_LIMIT`] and snapped through the session snap
/// engine. A degenerate segment asks nothing (0.0).
pub fn bend_from_pointer(a: V2, b: V2, p: V2) -> f32 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if len < 0.001 {
        return 0.0;
    }
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    // bend_mid's left normal, restated.
    let n = [-d[1] / len, d[0] / len];
    let off = (p[0] - mid[0]) * n[0] + (p[1] - mid[1]) * n[1];
    let mut bend = 4.0 * (off / (len / 2.0)).atan();
    bend = bend.clamp(-BEND_LIMIT, BEND_LIMIT);
    let cfg = snap_config();
    if cfg.enabled {
        bend = snap_angle_deg(bend.to_degrees(), cfg.angle_step).to_radians();
        bend = bend.clamp(-BEND_LIMIT, BEND_LIMIT);
    }
    bend
}

/// The segment a bend press at stage point `p` would
/// grab, or `None`: `(figure, node, score)` with `node`
/// the segment's CHILD joint (the bend channel's
/// indexing, [`segment_hit`]'s convention). A segment
/// qualifies when the point is within `radius` of its
/// arc midpoint (the handle the bulge draws through —
/// [`model::bend_mid`] at the DISPLAY bend, channels
/// included, since that is the arc on stage) or within
/// `radius` of its chord with the projection inside the
/// grab band ([`BEND_GRAB_T_MIN`]..=[`BEND_GRAB_T_MAX`]).
/// Nearest score wins; ties keep the earlier figure /
/// node, the hit tests' own convention. Setup mode
/// picks nothing: a rest pose draws straight and bends
/// are frame data.
pub fn bend_hit(core: &EditorCore, p: V2, radius: f32) -> Option<(usize, usize, f32)> {
    if core.setup_mode {
        return None;
    }
    let eval = core.evaluated_frame(core.display_frame());
    let mut best: Option<(usize, usize, f32)> = None;
    for fi in 0..core.project.figures.len() {
        let pos = feedback_positions(core, fi);
        for (ni, node) in core.project.figures[fi].def.nodes.iter().enumerate() {
            let Some(par) = node.parent else { continue };
            let (Some(&a), Some(&b)) = (pos.get(par), pos.get(ni)) else {
                continue;
            };
            let shown = eval
                .bends
                .get(fi)
                .and_then(|row| row.get(ni))
                .copied()
                .unwrap_or(0.0);
            let d_mid = model::dist(p, model::bend_mid(a, b, shown));
            let (d_chord, t) = dist_to_segment(p, a, b);
            let score = if d_mid <= radius {
                d_mid
            } else if d_chord <= radius && (BEND_GRAB_T_MIN..=BEND_GRAB_T_MAX).contains(&t) {
                d_chord
            } else {
                continue;
            };
            if best.map(|(_, _, bs)| score < bs).unwrap_or(true) {
                best = Some((fi, ni, score));
            }
        }
    }
    best
}

/// One in-flight bend drag: the grabbed segment, the
/// frame the settle will write, the segment's endpoints
/// as drawn at Down (poses are never written during the
/// gesture, so they stay the segment's geometry
/// throughout), the stored bend at Down, and the live
/// preview bend the draw pass renders.
#[derive(Clone, Copy)]
struct BendDrag {
    fi: usize,
    ni: usize,
    frame: usize,
    a: V2,
    b: V2,
    press: V2,
    start: f32,
    bend: f32,
}

thread_local! {
    static BEND_DRAG: RefCell<Option<BendDrag>> = const { RefCell::new(None) };
}

/// Whether a bend drag currently owns the stage
/// gesture (the cursor and feedback passes ask).
pub fn bend_drag_active() -> bool {
    BEND_DRAG.with(|c| c.borrow().is_some())
}

/// The live bend session's `(figure, node, preview
/// bend)` for the draw passes; `None` when no bend drag
/// is in flight.
pub fn bend_drag_preview() -> Option<(usize, usize, f32)> {
    BEND_DRAG.with(|c| c.borrow().map(|d| (d.fi, d.ni, d.bend)))
}

/// Apply the bend drag's preview to one figure's bend
/// row for the figure pass: while a bend drag is live
/// on this figure and the display frame is the grab
/// frame, the grabbed node's drawn bend is the
/// session's preview, not the evaluated row's. Every
/// other row — and every row the moment the gesture
/// ends — passes through untouched, so a stage that
/// never bend-drags draws byte-for-byte as before.
fn bend_preview_apply(fi: usize, display: f32, mut bends: Vec<f32>) -> Vec<f32> {
    let live = BEND_DRAG.with(|c| {
        c.borrow().and_then(|d| {
            if d.fi == fi && display == d.frame as f32 {
                Some((d.ni, d.bend))
            } else {
                None
            }
        })
    });
    if let Some((ni, bend)) = live {
        if bends.len() <= ni {
            bends.resize(ni + 1, 0.0);
        }
        bends[ni] = bend;
    }
    bends
}

/// Stage drag, Down phase for the bend grab: arm a bend
/// drag on the segment at stage point `p` (see
/// [`bend_hit`]) and select it, so the inspector and
/// dopesheet show the segment being bent. Returns true
/// when the press was consumed. Refused while playing,
/// in setup mode, or while a core drag already owns the
/// gesture; a locked figure refuses silently (the
/// shell's fall-through posts the standing refusal — see
/// the section docs). A stale record from an
/// interrupted gesture is dropped first — it holds no
/// writes, so dropping is all the cleanup it needs. No
/// snapshot: the gesture's single undo step is the
/// release's settle.
pub fn bend_drag_down(core: &mut EditorCore, p: V2, radius: f32) -> bool {
    BEND_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing || core.setup_mode || core.dragging.is_some() {
        return false;
    }
    let Some((fi, ni, _)) = bend_hit(core, p, radius) else {
        return false;
    };
    if core.project.figures[fi].locked {
        return false;
    }
    let frame = core.current;
    let start = core
        .project
        .frames
        .get(frame)
        .and_then(|f| f.bends.get(fi))
        .and_then(|row| row.get(ni))
        .copied()
        .unwrap_or(0.0);
    let pos = feedback_positions(core, fi);
    let par = core.project.figures[fi].def.nodes[ni].parent;
    let (Some(&a), Some(&b)) = (par.and_then(|par| pos.get(par)), pos.get(ni)) else {
        return false;
    };
    let shown = core
        .evaluated_frame(core.display_frame())
        .bends
        .get(fi)
        .and_then(|row| row.get(ni))
        .copied()
        .unwrap_or(start);
    core.selected_figure = fi;
    core.selected_node = Some(ni);
    BEND_DRAG.with(|c| {
        *c.borrow_mut() = Some(BendDrag {
            fi,
            ni,
            frame,
            a,
            b,
            press: p,
            start,
            bend: shown,
        });
    });
    true
}

/// Stage drag, Move phase: track the pointer into the
/// session's preview bend. Writes nothing to the
/// project — the figure pass renders the preview (see
/// [`bend_preview_apply`]). Returns true while a bend
/// drag owns the gesture; a record gone stale
/// mid-gesture (its figure deleted under it) ends the
/// drag instead of writing anywhere.
pub fn bend_drag_move(core: &mut EditorCore, p: V2) -> bool {
    let alive = BEND_DRAG.with(|c| {
        let mut cell = c.borrow_mut();
        match cell.as_mut() {
            Some(d) if d.fi < core.project.figures.len() && d.frame < core.frame_count() => {
                d.bend = bend_from_pointer(d.a, d.b, p);
                true
            }
            _ => false,
        }
    });
    if !alive {
        BEND_DRAG.with(|c| *c.borrow_mut() = None);
    }
    alive
}

/// Stage drag, Up phase: settle the gesture. A
/// cancelled gesture just drops the record (nothing was
/// written and no snapshot was taken, so there is
/// nothing to restore and no undo step to spend).
/// Otherwise, a release that travelled past
/// [`BEND_DRAG_SLOP`] and lands a bend genuinely
/// different from the starting one ([`BEND_SETTLE_EPSILON`])
/// lands as exactly ONE `snapshot_labeled("Bend
/// segment")` plus the one stored-row write on the grab
/// frame — `tool_set_bend`'s resize-and-store shape and
/// `cycle_bend`'s status sentence. Anything else spends
/// nothing: it was a click, or a drag that came home.
pub fn bend_drag_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = BEND_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    if d.fi >= core.project.figures.len() || d.frame >= core.project.frames.len() {
        return;
    }
    if core.project.figures[d.fi]
        .def
        .nodes
        .get(d.ni)
        .and_then(|n| n.parent)
        .is_none()
    {
        return;
    }
    if core.project.figures[d.fi].locked {
        core.status = "Figure is locked — unlock it in the figures list.".into();
        return;
    }
    let settled = bend_from_pointer(d.a, d.b, p);
    if model::dist(p, d.press) <= BEND_DRAG_SLOP
        || (settled - d.start).abs() <= BEND_SETTLE_EPSILON
    {
        return;
    }
    core.snapshot_labeled("Bend segment");
    let nodes = core.project.figures[d.fi].def.nodes.len();
    let frame = &mut core.project.frames[d.frame];
    while frame.bends.len() <= d.fi {
        frame.bends.push(Vec::new());
    }
    if frame.bends[d.fi].len() < nodes {
        frame.bends[d.fi].resize(nodes, 0.0);
    }
    frame.bends[d.fi][d.ni] = settled;
    core.status = format!(
        "Bend: figure {} node {} \u{2192} {:.0}\u{00b0}.",
        d.fi + 1,
        d.ni,
        settled.to_degrees()
    );
}

/// The drag HUD's bend line while a bend drag is live
/// (`BEND -15 DEG`): the preview bend in whole degrees,
/// in the HUD stamp's ASCII vocabulary, mirroring
/// `stage_sprites::sprite_drag_hud`'s adoption shape.
pub fn bend_drag_hud() -> Option<String> {
    bend_drag_preview().map(|(_, _, bend)| format!("BEND {:.0} DEG", bend.to_degrees()))
}

// ----------------------------------------------------------------
// Marquee multi-selection (Roadmap Wave 2, W2-01)
// ----------------------------------------------------------------
//
// The user's verbatim ask (2026-11-10): "i can select
// character by left mouse holding get square" — hold the
// left button on empty stage, drag a square, and every
// character the square touches becomes the selection.
// Until this section the stage had point selection only
// (a press picks one figure/joint; RM5's marquee lives in
// the graph editor, over keys, not figures).
//
// Three pieces, all session-side like the motion-path
// session above (whose shape this section deliberately
// mirrors):
//
// * The SET: the figures the last marquee committed,
//   held in a thread local — shell session state, the
//   `VIEW_NAV` / [`StagePointer`] precedent, NOT project
//   data: it is never serialised, never undoable (a
//   selection is not an edit), and every consumer
//   re-validates indices against the live project, so a
//   stale index (a figure deleted under the set) simply
//   drops out of the geometry instead of panicking —
//   the `StageOverlays` stale-index discipline. Promoting
//   the set into `EditorCore` for panel/MCP consumers is
//   IA-W2-01-1; the storage swap is behind the four
//   accessors below and changes no signature here.
//
// * The MARQUEE session: Down records the anchor, Move
//   tracks the pointer, and NOTHING is written while the
//   gesture lives — the release resolves the rectangle
//   against the drawn figures ([`figures_in_marquee`])
//   and replaces the set. A release inside the click slop
//   is a click, not a selection: it CLEARS the set (the
//   empty-stage click's standing job). A cancelled
//   gesture drops the record and changes nothing.
//
// * The GROUP DRAG: a press on a joint of a figure that
//   is in a set of two or more grabs the SET, not the
//   figure. Moves write nothing (the draw pass previews
//   the set's outlines at the live displacement, the
//   path drag's preview discipline); the release lands
//   the whole move as exactly ONE `snapshot_labeled`
//   plus one root write per movable figure — one undo
//   step per gesture, the rule every stage gesture
//   obeys. Locked figures in the set stay put and are
//   named in the recap; a zero-displacement release
//   spends nothing.
//
// Non-interference, by construction: the sessions arm
// only through the shell's routing (IA-W2-01-2), which
// consults them exactly where the press stack has
// already missed everything else (marquee) or before
// the single-figure drag on a set member (group drag).
// RM15's picking (`joint_hit` / `segment_hit` at the
// screen-space radius), AC6's right-press stack (a
// separate event entirely — untouched here) and RM16's
// path session (consulted earlier in the same Down
// chain, and mutually exclusive with these by the
// `dragging` guard) all behave exactly as before when
// the set is empty and no session is live, which is
// every stage that never marquees.

/// The largest rectangle, in stage units on BOTH axes, a
/// marquee release may span and still read as a click
/// rather than a selection: a press-and-release with no
/// real travel. Stage units, not screen points — the
/// sessions in this module only ever see stage points
/// (the path session's `PATH_PICK_TOLERANCE` precedent);
/// at the fit view one stage unit is ≈ one point, so
/// this is a ≈4 pt slop where the stage is read largest.
pub const MARQUEE_CLICK_SLOP: f32 = 4.0;

/// One in-flight marquee drag: where the press landed
/// and where the pointer is now. The rectangle is the
/// normalised span of the two (see [`marquee_rect`]).
#[derive(Clone, Copy)]
struct MarqueeDrag {
    anchor: V2,
    at: V2,
}

/// One in-flight group drag: where the press landed,
/// where the pointer is now, and the set it grabbed
/// (snapshotted at Down, so a mid-gesture set change
/// cannot retarget the gesture).
#[derive(Clone)]
struct GroupDrag {
    press: V2,
    at: V2,
    figures: Vec<usize>,
}

thread_local! {
    /// The in-flight marquee drag, if any.
    static MARQUEE_DRAG: RefCell<Option<MarqueeDrag>> = const { RefCell::new(None) };
    /// The in-flight group drag, if any.
    static GROUP_DRAG: RefCell<Option<GroupDrag>> = const { RefCell::new(None) };
}

/// The current marquee figure set: sorted figure
/// indices, possibly carrying stale entries a consumer
/// must re-validate (the geometry below does).
/// (Roadmap Wave 2, IA-W2-01-1: the set lives in
/// `EditorCore::selected_figures` now, so panels, MCP
/// and the shell read ONE set; these accessors are the
/// marquee section's doors onto it.)
pub fn figure_set(core: &EditorCore) -> Vec<usize> {
    core.selected_figures.clone()
}

/// Replace the marquee figure set (sorted and
/// deduplicated here, so every writer gets the model's
/// one canonical form).
pub fn set_figure_set(core: &mut EditorCore, mut figures: Vec<usize>) {
    figures.sort_unstable();
    figures.dedup();
    core.selected_figures = figures;
}

/// Empty the marquee figure set.
pub fn clear_figure_set(core: &mut EditorCore) {
    core.selected_figures.clear();
}

/// Whether figure `fi` is in the marquee set.
pub fn figure_set_contains(core: &EditorCore, fi: usize) -> bool {
    core.selected_figures.contains(&fi)
}

/// The marquee rectangle spanned by two opposite
/// corners, normalised to `[min_x, min_y, max_x, max_y]`
/// — every drag direction yields the same rectangle.
pub fn marquee_rect(a: V2, b: V2) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

/// Whether stage point `p` lies inside marquee
/// rectangle `rect` (`[min_x, min_y, max_x, max_y]`),
/// edges inclusive: a joint exactly on the square's
/// border is inside the square.
pub fn marquee_rect_contains(rect: [f32; 4], p: V2) -> bool {
    p[0] >= rect[0] && p[0] <= rect[2] && p[1] >= rect[1] && p[1] <= rect[3]
}

/// Whether segment `a`–`b` touches marquee rectangle
/// `rect`: either endpoint inside, or the segment
/// crossing any of the rectangle's four edges. A long
/// limb spanning the square selects its figure even
/// when both joints land outside it.
pub fn segment_intersects_marquee_rect(a: V2, b: V2, rect: [f32; 4]) -> bool {
    if marquee_rect_contains(rect, a) || marquee_rect_contains(rect, b) {
        return true;
    }
    let corners = [
        [rect[0], rect[1]],
        [rect[2], rect[1]],
        [rect[2], rect[3]],
        [rect[0], rect[3]],
    ];
    for i in 0..4 {
        if segments_cross(a, b, corners[i], corners[(i + 1) % 4]) {
            return true;
        }
    }
    false
}

/// Whether segments `a`–`b` and `c`–`d` intersect,
/// touching and collinear overlap included (a limb
/// lying along the square's edge touches it).
fn segments_cross(a: V2, b: V2, c: V2, d: V2) -> bool {
    fn cross(o: V2, p: V2, q: V2) -> f32 {
        (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
    }
    fn on_segment(p: V2, q: V2, r: V2) -> bool {
        // q collinear with p–r: inside the bounding box?
        q[0] >= p[0].min(r[0])
            && q[0] <= p[0].max(r[0])
            && q[1] >= p[1].min(r[1])
            && q[1] <= p[1].max(r[1])
    }
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return true;
    }
    (d1 == 0.0 && on_segment(c, a, d))
        || (d2 == 0.0 && on_segment(c, b, d))
        || (d3 == 0.0 && on_segment(a, c, b))
        || (d4 == 0.0 && on_segment(a, d, b))
}

/// The figures a marquee over `rect` selects, in figure
/// order: every figure the layers show (hidden or
/// solo'd-away figures are not on stage and cannot be
/// grabbed — `path_point_at`'s visibility rule) whose
/// DRAWN positions touch the rectangle — any joint
/// inside it, or any segment crossing it. The positions
/// are the display frame's, setup-aware via
/// [`feedback_positions`], so the set is exactly the
/// characters the square was drawn around. Locked
/// figures ARE selected (selection is not an edit; the
/// group drag is where locking bites).
pub fn figures_in_marquee(core: &EditorCore, rect: [f32; 4]) -> Vec<usize> {
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    let mut out = Vec::new();
    for fi in 0..core.project.figures.len() {
        if !visible.get(fi).copied().unwrap_or(false) {
            continue;
        }
        let pos = feedback_positions(core, fi);
        let joint_in = pos.iter().any(|&j| marquee_rect_contains(rect, j));
        let segment_in = !joint_in
            && core.project.figures[fi]
                .def
                .nodes
                .iter()
                .enumerate()
                .any(|(ni, node)| {
                    let Some(par) = node.parent else { return false };
                    match (pos.get(par), pos.get(ni)) {
                        (Some(&a), Some(&b)) => {
                            segment_intersects_marquee_rect(a, b, rect)
                        }
                        _ => false,
                    }
                });
        if joint_in || segment_in {
            out.push(fi);
        }
    }
    out
}

/// Figure `fi`'s drawn bounding box on the stage
/// (`[min_x, min_y, max_x, max_y]` over its display
/// positions): the set outline's geometry. `None` for a
/// stale index, a layers-hidden figure, or a figure
/// with no positions — draws nothing then, the overlay
/// stale-index discipline.
pub fn figure_stage_bbox(core: &EditorCore, fi: usize) -> Option<[f32; 4]> {
    if fi >= core.project.figures.len() {
        return None;
    }
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return None;
    }
    let pos = feedback_positions(core, fi);
    let mut it = pos.iter();
    let first = *it.next()?;
    let mut rect = [first[0], first[1], first[0], first[1]];
    for p in it {
        rect[0] = rect[0].min(p[0]);
        rect[1] = rect[1].min(p[1]);
        rect[2] = rect[2].max(p[0]);
        rect[3] = rect[3].max(p[1]);
    }
    Some(rect)
}

/// Stage drag, Down phase for the marquee: record the
/// anchor at stage point `p`. Returns true when the
/// press starts a marquee. The shell routes a press
/// here only once its stack has missed everything else
/// (IA-W2-01-2), so the guards are the gesture's own:
/// refused while playing (every stage press path's
/// rule), in draw/camera mode (those presses belong to
/// their modes), or while a pose drag already owns the
/// gesture. A stale record from an interrupted gesture
/// is dropped first — it holds no writes. No snapshot:
/// a selection is not an edit.
pub fn marquee_down(core: &mut EditorCore, p: V2) -> bool {
    MARQUEE_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing || core.draw_mode || core.camera_mode || core.dragging.is_some() {
        return false;
    }
    MARQUEE_DRAG.with(|c| *c.borrow_mut() = Some(MarqueeDrag { anchor: p, at: p }));
    true
}

/// Stage drag, Move phase for the marquee: track the
/// pointer for the rubber band. Writes nothing. Returns
/// true while a marquee drag owns the gesture.
pub fn marquee_move(p: V2) -> bool {
    MARQUEE_DRAG.with(|c| match c.borrow_mut().as_mut() {
        Some(d) => {
            d.at = p;
            true
        }
        None => false,
    })
}

/// The marquee rectangle a live marquee drag currently
/// spans — the rubber band the draw pass paints — or
/// `None` when no marquee is in flight.
pub fn marquee_live_rect() -> Option<[f32; 4]> {
    MARQUEE_DRAG.with(|c| c.borrow().as_ref().map(|d| marquee_rect(d.anchor, d.at)))
}

/// Stage drag, Up phase for the marquee: settle the
/// gesture. A cancelled gesture drops the record and
/// changes nothing. A release inside the click slop
/// ([`MARQUEE_CLICK_SLOP`] on both axes) was a click on
/// empty stage: it clears the set and nothing else. A
/// real drag REPLACES the set with the rectangle's
/// figures ([`figures_in_marquee`]); a non-empty result
/// also makes its topmost figure (highest index — the
/// one drawn last, on top) the core's selected figure,
/// so the panels that read the single selection follow
/// the marquee, and posts the count to the status line.
/// An empty result clears the set silently, the
/// empty-stage click's standing silence.
pub fn marquee_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = MARQUEE_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    let rect = marquee_rect(d.anchor, p);
    let is_click = (rect[2] - rect[0]) < MARQUEE_CLICK_SLOP
        && (rect[3] - rect[1]) < MARQUEE_CLICK_SLOP;
    if is_click {
        clear_figure_set(core);
        return;
    }
    let found = figures_in_marquee(core, rect);
    if found.is_empty() {
        clear_figure_set(core);
        return;
    }
    let topmost = *found.last().unwrap();
    set_figure_set(core, found.clone());
    core.select_figure(topmost);
    core.status = if found.len() == 1 {
        "Selected 1 figure.".to_string()
    } else {
        format!("Selected {} figures.", found.len())
    };
}

/// Stage drag, Down phase for the group drag: grab the
/// whole marquee set through a press on one of its
/// members. Returns true when the press was consumed.
/// The press must land on a joint (RM15's [`joint_hit`]
/// at the caller's screen-space radius — the pose
/// press's own target) of a figure IN the set, the set
/// must hold at least two figures, and the gesture
/// guards are the pose drag's: refused while playing,
/// in setup mode (rest poses have no roots to move —
/// the nudge's own sentence), in draw/camera mode, or
/// while another drag owns the gesture. On a grab the
/// core's single selection follows the grabbed figure
/// (the pose press's selection writes); no snapshot is
/// taken — the gesture's one undo step is the release's.
pub fn group_drag_down(core: &mut EditorCore, p: V2, radius: f32) -> bool {
    GROUP_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing
        || core.setup_mode
        || core.draw_mode
        || core.camera_mode
        || core.dragging.is_some()
    {
        return false;
    }
    let set = figure_set(core);
    if set.len() < 2 {
        return false;
    }
    let Some((fi, ni)) = joint_hit(core, p, radius) else {
        return false;
    };
    if !set.contains(&fi) {
        return false;
    }
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    if !visible.get(fi).copied().unwrap_or(false) {
        return false;
    }
    core.selected_figure = fi;
    core.selected_node = Some(ni);
    GROUP_DRAG.with(|c| {
        *c.borrow_mut() = Some(GroupDrag {
            press: p,
            at: p,
            figures: set,
        })
    });
    true
}

/// Stage drag, Move phase for the group drag: track
/// the pointer for the draw pass's preview. Writes
/// nothing to the project. Returns true while a group
/// drag owns the gesture; a record whose figures have
/// all been deleted under it ends the drag instead of
/// writing anywhere.
pub fn group_drag_move(core: &mut EditorCore, p: V2) -> bool {
    let alive = GROUP_DRAG.with(|c| {
        let mut cell = c.borrow_mut();
        match cell.as_mut() {
            Some(d) if d.figures.iter().any(|&fi| fi < core.project.figures.len()) => {
                d.at = p;
                true
            }
            _ => false,
        }
    });
    if !alive {
        GROUP_DRAG.with(|c| *c.borrow_mut() = None);
    }
    alive
}

/// How many figures the in-flight group drag grabbed —
/// the set as it stood at Down (the record's snapshot,
/// so a mid-gesture set change cannot rewrite the
/// count). `None` when no group drag is in flight. The
/// HUD's count (W2-07, [`group_drag_hud`]): the grabbed
/// set, locked members included — locking bites at the
/// settle, and the draw pass previews the whole set's
/// outlines on the same terms.
pub fn group_drag_count() -> Option<usize> {
    GROUP_DRAG.with(|c| c.borrow().as_ref().map(|d| d.figures.len()))
}

/// The live displacement a group drag previews —
/// `Some(delta)` while one is in flight, `None`
/// otherwise. The draw pass offsets the set's outlines
/// by it; the figures themselves settle at release.
pub fn group_drag_delta() -> Option<V2> {
    GROUP_DRAG.with(|c| {
        c.borrow()
            .as_ref()
            .map(|d| [d.at[0] - d.press[0], d.at[1] - d.press[1]])
    })
}

/// Stage drag, Up phase for the group drag: settle the
/// gesture. A cancelled gesture drops the record —
/// nothing was written, so there is nothing to restore
/// and no undo step to spend. A zero-displacement
/// release spends nothing either: it was a click on a
/// set member, and its selection writes already
/// happened at Down. Otherwise the move lands as
/// exactly ONE undo step: one `snapshot_labeled` ("Move
/// figures"), then each movable figure's root at the
/// current frame shifts by the press-to-release delta
/// (the pose the user dragged to, written once — not a
/// per-frame chase). Locked figures stay put and are
/// counted in the recap; a set whose figures are ALL
/// locked (or gone) takes no snapshot and says so.
pub fn group_drag_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = GROUP_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    let delta = [p[0] - d.press[0], p[1] - d.press[1]];
    if delta == [0.0, 0.0] {
        return;
    }
    let frame = core.current;
    let movable: Vec<usize> = d
        .figures
        .iter()
        .copied()
        .filter(|&fi| {
            fi < core.project.figures.len() && !core.project.figures[fi].locked
        })
        .collect();
    let skipped = d.figures.len() - movable.len();
    if movable.is_empty() {
        core.status =
            "Selected figures are locked — unlock them in the figures list.".into();
        return;
    }
    core.snapshot_labeled("Move figures");
    for fi in &movable {
        if let Some(pose) = core
            .project
            .frames
            .get_mut(frame)
            .and_then(|f| f.poses.get_mut(*fi))
        {
            pose.root = [pose.root[0] + delta[0], pose.root[1] + delta[1]];
        }
    }
    core.status = if skipped == 0 {
        format!("Moved {} figures at frame {}.", movable.len(), frame + 1)
    } else if skipped == 1 {
        format!(
            "Moved {} figures at frame {}; 1 locked figure stayed.",
            movable.len(),
            frame + 1
        )
    } else {
        format!(
            "Moved {} figures at frame {}; {} locked figures stayed.",
            movable.len(),
            frame + 1,
            skipped
        )
    };
}

/// The stage pass for marquee multi-selection, called
/// by [`stage_shape`] in its tail (after the
/// motion-path pass, before RM15's feedback pass):
/// every figure in the set gets a bounding outline in
/// the selection accent, padded a screen-constant
/// margin so the outline hugs the figure at any zoom;
/// while a group drag is in flight the outlines ride
/// at the drag's live displacement (the figures
/// themselves settle at release — the path drag's
/// preview discipline); while a marquee drag is in
/// flight its rubber band is drawn over the set — a
/// translucent accent fill inside a solid accent
/// outline, its corners converted through the display
/// transform so the band is true under camera rotation.
/// An empty set with no live marquee draws nothing at
/// all — the idle stage is byte-for-byte the stage
/// before this section existed.
fn draw_marquee_selection(
    mut shape: Shape,
    core: &EditorCore,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
) -> Shape {
    let accent = hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
    let delta = group_drag_delta().unwrap_or([0.0, 0.0]);
    let pad = 6.0 / s.max(0.05);
    for fi in figure_set(core) {
        let Some(bb) = figure_stage_bbox(core, fi) else {
            continue;
        };
        let corners = [
            [bb[0] - pad + delta[0], bb[1] - pad + delta[1]],
            [bb[2] + pad + delta[0], bb[1] - pad + delta[1]],
            [bb[2] + pad + delta[0], bb[3] + pad + delta[1]],
            [bb[0] - pad + delta[0], bb[3] + pad + delta[1]],
        ];
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.stroke_with(screen, true, 1.5, LineCap::Round, accent.alpha(0.9));
    }
    if let Some(rect) = marquee_live_rect() {
        let corners = [
            [rect[0], rect[1]],
            [rect[2], rect[1]],
            [rect[2], rect[3]],
            [rect[0], rect[3]],
        ];
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen.clone(), accent.alpha(0.10));
        shape = shape.stroke_with(screen, true, 1.0, LineCap::Round, accent.alpha(0.95));
    }
    shape
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

// ----------------------------------------------------------------
// Stored guides & rulers (Roadmap Wave 2, W2-05)
// ----------------------------------------------------------------
//
// The composition guides above are DERIVED (from the stage
// or the camera) and live nowhere; this section is their
// authored counterpart. The schema is project data —
// `model::StoredGuide` + `Project::stored_guides`,
// serde-defaulted, no format bump — edited through the
// core's stored-guide ops (`stored_guide_add/move/delete/
// clear`, one undo step each). This section is the stage
// half: the drawing geometry (stored lines + ruler ticks)
// and the drag session that creates, moves and deletes
// guides by hand.
//
// WORLD SPACE, end to end. A guide's position is a stage
// coordinate; the lines and rulers below are emitted in
// stage coordinates and mapped by `stage_shape` through
// `display_transform` — the camera transform with W2-04's
// view pan/zoom folded in — and the session receives stage
// points converted by the shell through `drag_transform`,
// the same fold's inverse. So a guide stays glued to its
// stage coordinate under camera moves, view pans and
// rotation alike; nothing in this section ever sees a
// screen coordinate except the screen-CONSTANT sizes
// (band thickness, label height, pick radius), which are
// converted through the view scale by `stage_units` like
// every RM15 constant.
//
// THE RULERS are graduated stage borders: the top ruler
// measures stage x along the stage's top edge, the left
// ruler measures stage y along its left edge, each in a
// band OUTSIDE the stage rect whose thickness is
// [`RULER_THICKNESS_PX`] on screen at any zoom. Drawing
// them in stage space (rather than as fixed window
// chrome) is what makes them exact under camera rotation
// — a screen-fixed ruler cannot label a rotated axis —
// at the honest price that a ruler leaves the screen
// when its stage edge does (recorded in the findings).
//
// THE SESSION mirrors the marquee/path discipline: Down
// records, Move writes nothing (the draw pass previews
// through [`guide_drag_preview`]), Up settles through
// exactly ONE core op — or none: a cancelled gesture, a
// create released back inside its ruler, and a move that
// lands on the guide's own coordinate all spend nothing
// (the last by the op's own no-change rule). The three
// doors:
//
// * CREATE — press inside a ruler band (top ruler → a
//   horizontal guide, left ruler → a vertical one) and
//   drag onto the stage; the release adds the guide at
//   the release coordinate. A press in the corner square
//   where the bands meet is ambiguous and refused.
// * MOVE — press within the pick radius of an existing
//   guide's line (the shell routes here only while the
//   stored-guides overlay is on, so a hidden guide can
//   never be grabbed invisibly); the release moves it.
// * DELETE — a move released back beyond its ruler's
//   outer region (past the stage edge, into the band and
//   beyond) deletes the guide instead: the classic
//   drag-back-to-the-ruler gesture, and the only delete
//   the stage itself offers (the op also exists for
//   panel/MCP callers — see the findings' IAs).
//
// If the rulers overlay is off, the shell still routes
// existing-guide presses here (MOVE works), but no press
// can land in a band that is not drawn, so CREATE and
// DELETE-by-drag are unreachable — [`guide_down`] takes
// the band thickness as a parameter and the shell passes
// 0 when rulers are hidden, which closes both ruler
// doors while leaving the line hit test intact.
//
// Mode guards are the marquee's: refused while playing,
// in draw mode, in camera mode (those presses belong to
// their modes), or while a pose drag owns the gesture.
// Setup mode is ALLOWED — rigging against a guide is a
// primary use, and guides are frame-independent (they
// live on the stage, not on a frame), so nothing about
// the setup pose can conflict with them.

/// The ink stored guides draw in: a cyan distinct from
/// the composition guides' white hairlines and from the
/// selection accent (`SELECTION_ACCENT` is taken — it
/// means "selected", and a stored guide is not a
/// selection). Alpha is applied at the draw site.
pub const STORED_GUIDE_COLOR: u32 = 0x2FB9D8;

/// Ruler band thickness in ELEMENT POINTS: the bands are
/// screen-constant chrome expressed in stage units (see
/// [`ruler_band`]), so the ruler keeps its size at any
/// view zoom — the pick radius's doctrine.
pub const RULER_THICKNESS_PX: f32 = 18.0;

/// Ruler tick-label glyph height in ELEMENT POINTS,
/// converted like the band (see [`ruler_label_size`]).
pub const RULER_LABEL_PX: f32 = 9.0;

/// Minimum on-screen spacing between major ruler ticks,
/// in element points: the step ladder picks the smallest
/// step that keeps majors at least this far apart, so
/// labels never crowd at any zoom.
pub const RULER_MAJOR_MIN_PX: f32 = 56.0;

/// The ruler band thickness in stage units at view
/// `scale` ([`RULER_THICKNESS_PX`] on screen).
pub fn ruler_band(scale: f32) -> f32 {
    stage_units(RULER_THICKNESS_PX, scale)
}

/// The ruler label glyph height in stage units at view
/// `scale` ([`RULER_LABEL_PX`] on screen).
pub fn ruler_label_size(scale: f32) -> f32 {
    stage_units(RULER_LABEL_PX, scale)
}

/// The major-tick step for a ruler at view `scale`, in
/// stage units: the smallest value of the 1/2/5 ladder
/// (1, 2, 5, 10, 20, 50, …) whose on-screen spacing
/// reaches [`RULER_MAJOR_MIN_PX`]. Steps are whole stage
/// units at every zoom — tick labels are integers, which
/// the stage bitmap font draws exactly (it has digits
/// and `-`, no decimal point worth trusting at 9 pt).
pub fn ruler_step(scale: f32) -> f32 {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    for exp in 0..=6u32 {
        for m in [1.0f32, 2.0, 5.0] {
            let step = m * 10f32.powi(exp as i32);
            if step * scale >= RULER_MAJOR_MIN_PX {
                return step;
            }
        }
    }
    5_000_000.0
}

/// One ruler tick, in stage coordinates: a minor or major
/// division on one ruler. `along_x` selects the ruler —
/// true is the TOP ruler (it measures stage x; its ticks
/// are vertical lines at `pos`), false is the LEFT ruler
/// (measures stage y; horizontal ticks at `pos`). Major
/// ticks carry their coordinate as `label`; minors carry
/// none. Endpoints and label placement are derived from
/// the tick + band at draw time ([`ruler_tick_line`],
/// [`ruler_label_placement`]) so this list stays the pure
/// measuring result.
#[derive(Clone, Debug, PartialEq)]
pub struct RulerTick {
    /// Stage coordinate the tick sits at.
    pub pos: f32,
    /// True: top ruler (measures x). False: left (y).
    pub along_x: bool,
    /// A labelled major division (vs a minor fifth).
    pub major: bool,
    /// The coordinate text for a major tick.
    pub label: Option<String>,
}

/// All ticks for both rulers of a `stage`-sized stage at
/// view `scale`: majors at every [`ruler_step`] multiple
/// from 0 to the stage extent (inclusive), minors at the
/// four fifth-divisions between consecutive majors (the
/// step is always a 1/2/5 value, so fifths are exact).
/// Both rulers are emitted (top first, then left), each
/// ascending. A non-positive stage axis yields no ticks
/// for its ruler; a degenerate stage yields none at all.
pub fn ruler_ticks(stage: [f32; 2], scale: f32) -> Vec<RulerTick> {
    let step = ruler_step(scale);
    let minor = step / 5.0;
    let mut out = Vec::new();
    for (along_x, extent) in [(true, stage[0]), (false, stage[1])] {
        if !(extent > 0.0) {
            continue;
        }
        // k counts MINOR divisions; every fifth is a major.
        // The loop is bounded by the extent, and the extent
        // by the stage — a hand-edited absurd stage could
        // still ask for many ticks, so the count is capped
        // where the labels would be unreadable anyway.
        let mut k: u32 = 0;
        loop {
            let pos = k as f32 * minor;
            if pos > extent || k > 100_000 {
                break;
            }
            let major = k % 5 == 0;
            out.push(RulerTick {
                pos,
                along_x,
                major,
                label: if major {
                    Some(format!("{}", pos.round() as i64))
                } else {
                    None
                },
            });
            k += 1;
        }
    }
    out
}

/// The stage-space endpoints of one tick's line, in a
/// band of `band` stage units: ticks are anchored at the
/// stage edge (the measuring edge) and reach into the
/// band — majors 45% of the band, minors 25% — leaving
/// the band's outer part to the labels.
pub fn ruler_tick_line(tick: &RulerTick, band: f32) -> (V2, V2) {
    let reach = band * if tick.major { 0.45 } else { 0.25 };
    if tick.along_x {
        ([tick.pos, 0.0], [tick.pos, -reach])
    } else {
        ([0.0, tick.pos], [-reach, tick.pos])
    }
}

/// Where a major tick's label centres, and at what angle:
/// in the band's outer part, centred on the tick. Top
/// labels run along the stage (angle 0); left labels run
/// bottom-to-top (angle −90 in y-down stage space), the
/// classic left-ruler orientation, so a long coordinate
/// extends along the ruler instead of out of its band.
/// Both rotate with the stage under the camera transform,
/// like every stage-space label (the frustum-safe labels'
/// precedent).
pub fn ruler_label_placement(tick: &RulerTick, band: f32) -> (V2, f32) {
    if tick.along_x {
        ([tick.pos, -band * 0.75], 0.0)
    } else {
        ([-band * 0.75, tick.pos], -90.0)
    }
}

/// The stored-guide lines the stage draws, as
/// `(a, b, handle)` stage-space triples — every stored
/// guide's [`model::StoredGuide::endpoints`] + handle,
/// with the live drag preview folded in (the draw pass's
/// one session read, the marquee band's precedent): a
/// MOVE preview replaces its guide's stored position, a
/// CREATE preview appends its not-yet-a-guide line. A
/// guide whose stored position is non-finite draws
/// nothing (the model's inert-read rule), preview or not.
pub fn stored_guide_lines(core: &EditorCore) -> Vec<(V2, V2, V2)> {
    let stage = core.project.stage;
    let preview = guide_drag_preview();
    let mut out = Vec::new();
    for (i, g) in core.project.stored_guides.iter().enumerate() {
        let g = match preview {
            Some((_, pos, Some(idx))) if idx == i => model::StoredGuide {
                axis: g.axis,
                position: pos,
            },
            _ => *g,
        };
        if let (Some((a, b)), Some(handle)) = (g.endpoints(stage), g.handle(stage)) {
            out.push((a, b, handle));
        }
    }
    if let Some((axis, pos, None)) = preview {
        let g = model::StoredGuide {
            axis,
            position: pos,
        };
        if let (Some((a, b)), Some(handle)) = (g.endpoints(stage), g.handle(stage)) {
            out.push((a, b, handle));
        }
    }
    out
}

/// What an in-flight guide drag is doing, snapshotted at
/// Down so a mid-gesture list change cannot retarget it
/// (the group drag's rule — indices are re-validated at
/// settle, and a vanished index settles as a refusal from
/// the op, never a panic).
#[derive(Clone, Copy)]
enum GuideDragKind {
    /// Dragging a new guide off a ruler: not yet project
    /// data; the release creates it (or doesn't).
    Create { axis: model::GuideAxis },
    /// Dragging stored guide `index` (creation-order).
    Move { index: usize, axis: model::GuideAxis },
}

/// One in-flight guide drag: its kind, the live position
/// (stage coordinate on the guide's perpendicular axis),
/// and the ruler band thickness in stage units measured
/// at Down — the delete door is priced against the band
/// the user actually saw when they grabbed the guide,
/// even if the view zooms mid-gesture.
#[derive(Clone, Copy)]
struct GuideDrag {
    kind: GuideDragKind,
    at: f32,
    band: f32,
}

thread_local! {
    /// The in-flight guide drag, if any.
    static GUIDE_DRAG: RefCell<Option<GuideDrag>> = const { RefCell::new(None) };
}

/// The live preview of an in-flight guide drag:
/// `(axis, position, Some(index))` for a move of stored
/// guide `index`, `(axis, position, None)` for a create,
/// `None` when no guide drag is live. The draw pass's
/// fold-in point ([`stored_guide_lines`]).
pub fn guide_drag_preview() -> Option<(model::GuideAxis, f32, Option<usize>)> {
    GUIDE_DRAG.with(|c| {
        c.borrow().as_ref().map(|d| match d.kind {
            GuideDragKind::Create { axis } => (axis, d.at, None),
            GuideDragKind::Move { index, axis } => (axis, d.at, Some(index)),
        })
    })
}

/// Whether a guide drag currently owns the stage gesture.
pub fn guide_drag_active() -> bool {
    GUIDE_DRAG.with(|c| c.borrow().is_some())
}

/// Stage drag, Down phase for stored guides + rulers:
/// start a CREATE (press inside a ruler band) or a MOVE
/// (press on an existing guide's line). `p` is the press
/// in stage coordinates; `scale` is the view scale the
/// shell converted it at (its screen-space constants —
/// band thickness, pick radius — are priced from it, the
/// group drag's radius-parameter precedent). `band_px`
/// carries the rulers overlay's visibility: the shell
/// passes [`RULER_THICKNESS_PX`] while rulers show and
/// 0.0 while they are hidden, which closes the CREATE
/// door and (via the stored band) the DELETE door while
/// leaving MOVE reachable. Returns true when the press
/// started a guide gesture.
///
/// The mode guards are the marquee's: refused while
/// playing, in draw/camera mode, or while a pose drag
/// owns the gesture (setup mode is allowed — see the
/// section docs). A stale record from an interrupted
/// gesture is dropped first — it holds no writes. No
/// snapshot: nothing is written until the release.
pub fn guide_down(core: &mut EditorCore, p: V2, scale: f32, band_px: f32) -> bool {
    GUIDE_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing || core.draw_mode || core.camera_mode || core.dragging.is_some() {
        return false;
    }
    let (w, h) = (core.project.stage[0], core.project.stage[1]);
    if !(w > 0.0 && h > 0.0) {
        return false;
    }
    let band = stage_units(band_px, scale);
    if band > 0.0 {
        let in_top = p[1] >= -band && p[1] < 0.0 && p[0] >= 0.0 && p[0] <= w;
        let in_left = p[0] >= -band && p[0] < 0.0 && p[1] >= 0.0 && p[1] <= h;
        // The corner square satisfies neither test (each
        // band claims only its own edge's span), so an
        // ambiguous corner press falls through — it can
        // still hit a guide line or the marquee below.
        let create_axis = if in_top {
            Some(model::GuideAxis::Horizontal)
        } else if in_left {
            Some(model::GuideAxis::Vertical)
        } else {
            None
        };
        if let Some(axis) = create_axis {
            GUIDE_DRAG.with(|c| {
                *c.borrow_mut() = Some(GuideDrag {
                    kind: GuideDragKind::Create { axis },
                    at: axis.coordinate(p),
                    band,
                })
            });
            return true;
        }
    }
    // MOVE door: the nearest stored guide within the
    // screen-space pick radius (the same 13 pt the joints
    // get — a guide line is at least as grabbable as a
    // joint).
    if let Some(index) = core
        .project
        .nearest_stored_guide(p, pick_radius(scale))
    {
        let axis = core.project.stored_guides[index].axis;
        GUIDE_DRAG.with(|c| {
            *c.borrow_mut() = Some(GuideDrag {
                kind: GuideDragKind::Move { index, axis },
                at: axis.coordinate(p),
                band,
            })
        });
        return true;
    }
    false
}

/// Stage drag, Move phase for a guide drag: track the
/// pointer's coordinate on the guide's axis for the
/// preview. Writes nothing. Returns true while a guide
/// drag owns the gesture.
pub fn guide_move(p: V2) -> bool {
    GUIDE_DRAG.with(|c| match c.borrow_mut().as_mut() {
        Some(d) => {
            let axis = match d.kind {
                GuideDragKind::Create { axis } => axis,
                GuideDragKind::Move { axis, .. } => axis,
            };
            d.at = axis.coordinate(p);
            true
        }
        None => false,
    })
}

/// Stage drag, Up phase for a guide drag: settle the
/// gesture through the core's stored-guide ops — exactly
/// one op on a real settle, none otherwise:
///
/// * cancelled ⇒ drop the record, change nothing;
/// * CREATE released while still inside its ruler band
///   (the release coordinate never crossed the stage
///   edge) ⇒ the user never dragged the guide out —
///   nothing is created;
/// * CREATE released on the stage ⇒ `stored_guide_add`
///   at the release coordinate (one undo step);
/// * MOVE released beyond its ruler (coordinate past
///   the stage edge by more than the band measured at
///   Down) ⇒ `stored_guide_delete` (one undo step) —
///   with the rulers hidden the stored band is 0 and
///   ANY release past the edge would qualify, so the
///   delete door is open only when `band > 0`, the
///   Down-time record of a ruler the user could see;
/// * any other MOVE release ⇒ `stored_guide_move` (one
///   undo step; a release on the guide's own coordinate
///   is the op's reported no-op and spends nothing).
pub fn guide_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = GUIDE_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    match d.kind {
        GuideDragKind::Create { axis } => {
            let pos = axis.coordinate(p);
            // Still inside the band (the perpendicular
            // coordinate never crossed the stage edge
            // onto the stage): nothing was dragged out.
            let crossed = if axis.is_vertical() {
                p[0] >= 0.0
            } else {
                p[1] >= 0.0
            };
            if crossed {
                core.stored_guide_add(axis, pos);
            }
        }
        GuideDragKind::Move { index, axis } => {
            let pos = axis.coordinate(p);
            let beyond_ruler = if axis.is_vertical() {
                p[0] < -d.band
            } else {
                p[1] < -d.band
            };
            if d.band > 0.0 && beyond_ruler {
                core.stored_guide_delete(index);
            } else {
                core.stored_guide_move(index, pos);
            }
        }
    }
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
    fn view_pan_drags_the_picture_by_the_pointers_own_travel() {
        // Roadmap Wave 2 (W2-04): a pan writes only the
        // ViewNav pan, by exactly the press-to-pointer
        // element-point travel, from the gesture's start
        // state on every Move (no accumulation drift),
        // and leaves the zoom where the gesture found it.
        set_view_nav(ViewNav {
            zoom: 1.5,
            pan: [10.0, 20.0],
        });
        assert!(view_pan_down([100.0, 100.0], PanSource::Middle));
        assert!(view_pan_active());
        assert_eq!(view_pan_source(), Some(PanSource::Middle));
        assert!(view_pan_move([130.0, 85.0]));
        assert_eq!(
            view_nav(),
            ViewNav {
                zoom: 1.5,
                pan: [40.0, 5.0]
            }
        );
        assert!(view_pan_move([90.0, 120.0]));
        assert_eq!(view_nav().pan, [0.0, 40.0]);
        assert!(view_pan_up(false));
        assert!(!view_pan_active());
        assert_eq!(view_nav().pan, [0.0, 40.0], "a clean release keeps the pan");
        assert!(!view_pan_move([0.0, 0.0]), "no gesture, no move");
        reset_view_nav();
    }

    #[test]
    fn view_pan_cancel_restores_the_start_view() {
        set_view_nav(ViewNav {
            zoom: 1.0,
            pan: [5.0, 5.0],
        });
        assert!(view_pan_down([0.0, 0.0], PanSource::Middle));
        assert!(view_pan_move([50.0, 50.0]));
        assert_eq!(view_nav().pan, [55.0, 55.0]);
        assert!(view_pan_up(true));
        assert_eq!(
            view_nav(),
            ViewNav {
                zoom: 1.0,
                pan: [5.0, 5.0]
            },
            "a cancelled pan puts the view back"
        );
        assert!(!view_pan_up(false), "the gesture is over");
        reset_view_nav();
    }

    #[test]
    fn space_hold_arms_the_pan_and_a_consumed_hold_is_no_tap() {
        assert!(!space_pan_held());
        // The Space door refuses while Space is not held.
        assert!(!view_pan_down([10.0, 10.0], PanSource::Space));
        space_pan_press();
        assert!(space_pan_held());
        // An OS key-repeat press changes nothing.
        space_pan_press();
        assert!(space_pan_held());
        assert!(view_pan_down([10.0, 10.0], PanSource::Space));
        assert_eq!(view_pan_source(), Some(PanSource::Space));
        assert!(view_pan_move([30.0, 10.0]));
        assert!(view_pan_up(false));
        // The hold was spent on the pan: its release must
        // NOT toggle play.
        assert!(!space_pan_release());
        assert!(!space_pan_held());
        reset_view_nav();
    }

    #[test]
    fn space_tap_stays_a_tap_until_release_and_only_once() {
        // A release with no press (the press went to a
        // text field) is not a tap.
        assert!(!space_pan_release());
        space_pan_press();
        assert!(space_pan_release(), "an unconsumed hold releases as a tap");
        assert!(!space_pan_release(), "and only once");
        // A middle-button pan does not consume the hold:
        // only the Space door spends the tap.
        space_pan_press();
        assert!(view_pan_down([0.0, 0.0], PanSource::Middle));
        assert!(view_pan_up(false));
        assert!(space_pan_release());
        reset_view_nav();
    }

    #[test]
    fn stage_cursor_shows_the_pan_hand_over_the_core_states() {
        let core = EditorCore::new();
        space_pan_press();
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Grab);
        assert!(view_pan_down([0.0, 0.0], PanSource::Space));
        assert_eq!(stage_cursor(&core, StageHover::None), rune::Cursor::Grabbing);
        assert!(view_pan_up(false));
        assert!(!space_pan_release());
        reset_view_nav();
    }

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
        // W2-33: the golden/diagonal variants are opt-in —
        // default OFF (IA-RM18-3's default flip was not
        // taken), so a default stage draws neither.
        assert!(!o.guides_golden && !o.guides_diagonals);
        // W2-27: limit arcs are likewise opt-in — default
        // OFF, so a default stage draws none.
        assert!(!o.limit_arcs);
        assert_eq!(o.stroke_selected, None);
        assert_eq!(o.particle_selected, None);
        // 1/1: the depths the core's old single count
        // defaulted to, so the default stage is unchanged.
        assert_eq!((o.onion_prev, o.onion_next), (1, 1));
    }

    #[test]
    fn stage_shape_draws_golden_and_diagonals_only_when_toggled() {
        // W2-33 (IA-RM18-3): the golden/diagonal geometry
        // RM18 pinned reaches the stage only through its
        // overlay flags, inside BOTH gates (master
        // `guides`, then `guides_frustum`). Off must be
        // byte-identical to the pre-W2-33 stage; on must
        // add prims (Debug length is the marker-free
        // witness — the subtitle-prim test above counts
        // `Fill {` the same comparative way).
        reset_view_nav();
        let draw = |overlays: StageOverlays| {
            let mut core = EditorCore::new();
            core.project.stage = [640.0, 360.0];
            format!("{:?}", stage_shape(&core, [640.0, 360.0], overlays))
        };
        let on = |golden: bool, diagonals: bool| StageOverlays {
            guides: true,
            guides_golden: golden,
            guides_diagonals: diagonals,
            ..StageOverlays::default()
        };
        let base = draw(on(false, false));
        let golden = draw(on(true, false));
        let both = draw(on(true, true));
        assert!(golden.len() > base.len(), "golden on adds its 4 lines");
        assert!(both.len() > golden.len(), "diagonals on adds its 2 lines");
        // Master gate: with `guides` off the flags are
        // inert — byte-identical output.
        let gated = StageOverlays {
            guides: false,
            guides_golden: true,
            guides_diagonals: true,
            ..StageOverlays::default()
        };
        assert_eq!(draw(gated), draw(StageOverlays::default()));
        // Frustum gate: the variants live inside the
        // framed-view family, so `guides_frustum` off
        // silences them too — byte-identical again.
        let no_frustum = StageOverlays {
            guides: true,
            guides_frustum: false,
            guides_golden: true,
            guides_diagonals: true,
            ..StageOverlays::default()
        };
        let no_frustum_plain = StageOverlays {
            guides: true,
            guides_frustum: false,
            ..StageOverlays::default()
        };
        assert_eq!(draw(no_frustum), draw(no_frustum_plain));
        reset_view_nav();
    }

    #[test]
    fn limit_arc_prims_draw_the_elbow_partition() {
        // Roadmap Wave 2 (W2-27): elbow limits (-10, +160)
        // about a downward parent (parent_dir 0 = straight
        // down) at radius 40, joint at (100, 100).
        let prim = limit_arc_prims([100.0, 100.0], 0.0, Some(-10.0), Some(160.0), 40.0)
            .expect("limits are set");
        let at = |rel: f32| {
            let d = model::dir(rel);
            [100.0 + d[0] * 40.0, 100.0 + d[1] * 40.0]
        };
        // The bound dots sit exactly on the bounds, and
        // each spoke runs from the joint to its bound.
        assert_eq!(prim.bound_points, vec![at(-10.0), at(160.0)]);
        let spokes: Vec<&LimitArcSeg> =
            prim.segs.iter().filter(|s| s.kind == LimitArcKind::Spoke).collect();
        assert_eq!(spokes.len(), 2);
        assert!(spokes.iter().all(|s| s.a == [100.0, 100.0]));
        // Every allowed piece lies on the arc (both ends at
        // the radius) and the chain starts at the min bound.
        let allowed: Vec<&LimitArcSeg> =
            prim.segs.iter().filter(|s| s.kind == LimitArcKind::Allowed).collect();
        assert!(!allowed.is_empty());
        assert_eq!(allowed[0].a, at(-10.0));
        assert_eq!(allowed[allowed.len() - 1].b, at(160.0));
        for s in &allowed {
            assert!((model::dist(s.a, [100.0, 100.0]) - 40.0).abs() < 0.01);
            assert!((model::dist(s.b, [100.0, 100.0]) - 40.0).abs() < 0.01);
        }
        // The forbidden remainder is DASHED: its drawn
        // sweep (190° worth of span) is strictly less than
        // the span — the gaps are real.
        let forbidden: Vec<&LimitArcSeg> =
            prim.segs.iter().filter(|s| s.kind == LimitArcKind::Forbidden).collect();
        assert!(!forbidden.is_empty());
        let drawn: f32 = forbidden
            .iter()
            .map(|s| {
                model::norm_deg(
                    model::angle_of([s.b[0] - 100.0, s.b[1] - 100.0])
                        - model::angle_of([s.a[0] - 100.0, s.a[1] - 100.0]),
                )
                .abs()
            })
            .sum();
        assert!(drawn > 100.0 && drawn < 190.0, "dashed sweep {drawn}");
        // Exactly one straight-ahead tick, crossing the
        // arc along the parent's direction (straight down:
        // x fixed, y straddling joint + radius).
        let straight: Vec<&LimitArcSeg> =
            prim.segs.iter().filter(|s| s.kind == LimitArcKind::Straight).collect();
        assert_eq!(straight.len(), 1);
        assert_eq!(straight[0].a[0], 100.0);
        assert!(straight[0].a[1] < 140.0 && straight[0].b[1] > 140.0);
    }

    #[test]
    fn limit_arc_prims_wrap_pin_and_floor() {
        // Wrap arc (170, -170): the allowed pieces hug the
        // ±180 seam (far side from the parent direction),
        // never the front.
        let prim = limit_arc_prims([0.0, 0.0], 0.0, Some(170.0), Some(-170.0), 30.0)
            .expect("limits are set");
        for s in prim.segs.iter().filter(|s| s.kind == LimitArcKind::Allowed) {
            let mid = [(s.a[0] + s.b[0]) / 2.0, (s.a[1] + s.b[1]) / 2.0];
            assert!(mid[1] < 0.0, "wrap arc stays behind (up-screen), got {mid:?}");
        }
        assert_eq!(prim.bound_points.len(), 2);
        // Pinned joint (45, 45): no allowed PIECES at all —
        // the one legal direction is carried by a single
        // shared spoke + dot.
        let prim = limit_arc_prims([0.0, 0.0], 0.0, Some(45.0), Some(45.0), 30.0)
            .expect("limits are set");
        assert!(prim.segs.iter().all(|s| s.kind != LimitArcKind::Allowed));
        assert_eq!(prim.bound_points.len(), 1);
        assert_eq!(
            prim.segs.iter().filter(|s| s.kind == LimitArcKind::Spoke).count(),
            1
        );
        // The ±180 pair is one direction, not two spokes.
        let prim = limit_arc_prims([0.0, 0.0], 0.0, Some(-180.0), Some(180.0), 30.0)
            .expect("limits are set");
        assert_eq!(prim.bound_points.len(), 1);
        assert!(prim.segs.iter().all(|s| s.kind != LimitArcKind::Forbidden));
        // Radius floor: a zero-length segment still gets a
        // readable annotation at the floor radius.
        let prim = limit_arc_prims([0.0, 0.0], 0.0, Some(0.0), Some(90.0), 0.0)
            .expect("limits are set");
        assert!(
            (model::dist(prim.bound_points[0], [0.0, 0.0]) - LIMIT_ARC_MIN_RADIUS).abs() < 0.01
        );
        // No limits: no drawing.
        assert!(limit_arc_prims([0.0, 0.0], 0.0, None, None, 30.0).is_none());
    }

    #[test]
    fn stage_shape_draws_limit_arcs_only_when_toggled_and_limited() {
        // Roadmap Wave 2 (W2-27): the three-way pin — the
        // flag off draws nothing whatever the limits; the
        // flag on draws nothing for a node WITHOUT limits
        // (byte-identical to flag off: the brief's pin);
        // flag on + limits draws strictly more.
        reset_view_nav();
        let core_with = |limited: bool| {
            let mut core = EditorCore::new();
            core.project.stage = [640.0, 360.0];
            // Select figure 0's first non-root node (found
            // at runtime, the bend pins' way) and give it
            // elbow limits — or clear every limit for the
            // unlimited leg.
            let ni = core.project.figures[0]
                .def
                .nodes
                .iter()
                .position(|n| n.parent.is_some())
                .expect("demo figure has a non-root node");
            for node in core.project.figures[0].def.nodes.iter_mut() {
                node.min_angle = None;
                node.max_angle = None;
            }
            if limited {
                core.project.figures[0].def.nodes[ni].min_angle = Some(-10.0);
                core.project.figures[0].def.nodes[ni].max_angle = Some(160.0);
            }
            core.selected_node = Some(ni);
            core
        };
        let draw = |core: &EditorCore, on: bool| {
            let overlays = StageOverlays { limit_arcs: on, ..StageOverlays::default() };
            format!("{:?}", stage_shape(core, [640.0, 360.0], overlays))
        };
        let limited = core_with(true);
        let unlimited = core_with(false);
        assert_eq!(draw(&limited, false), draw(&unlimited, false), "flag off: limits alone draw nothing");
        assert_eq!(draw(&unlimited, true), draw(&unlimited, false), "flag on: an unlimited node draws nothing extra");
        assert!(draw(&limited, true).len() > draw(&limited, false).len(), "flag on + limits draws the arc");
        reset_view_nav();
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

    // --------------------------------------------------------
    // Per-figure onion overrides (W2-37)
    // --------------------------------------------------------

    /// Two-figure core parked mid-timeline (5 frames,
    /// current = 2, so both sides have neighbours at every
    /// depth up to 2): figure 0 keeps the demo colour
    /// 0xE8E8EC; figure 1 is recoloured so the two figures'
    /// skins are distinguishable by their base alone.
    fn onion_two_figure_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.add_figure();
        core.project.figures[1].color = 0x112233;
        core.onion = true;
        core.current = 2;
        core
    }

    /// The full skin stream as comparable data: every
    /// entry's pieces verbatim (StylePiece is PartialEq) —
    /// the byte-identity comparisons below compare pieces,
    /// bases and alphas, not counts.
    fn skin_stream(
        core: &EditorCore,
        prev: u8,
        next: u8,
        overrides: &[Option<OnionFigureOverride>],
    ) -> Vec<(Vec<stykanimate_rs::paint::StylePiece>, u32, f32)> {
        onion_skins_with_overrides(core, prev, next, overrides)
    }

    #[test]
    fn onion_empty_overrides_reproduce_the_global_render() {
        // The W2-37 contract: with no override in force the
        // generalised walk IS the old walk, entry for entry
        // and piece for piece, at every depth pair — checked
        // on a two-figure core so a per-figure mix-up would
        // show. Both spellings of "no override" count: an
        // empty vec and a vec of empty slots.
        let core = onion_two_figure_core();
        for (prev, next) in [(0, 0), (1, 1), (2, 0), (0, 2), (3, 3), (2, 1)] {
            let plain = onion_skins(&core, prev, next);
            assert_eq!(skin_stream(&core, prev, next, &[]), plain, "empty vec at {prev}/{next}");
            assert_eq!(
                skin_stream(&core, prev, next, &[None, None]),
                plain,
                "empty slots at {prev}/{next}"
            );
            assert_eq!(
                skin_stream(
                    &core,
                    prev,
                    next,
                    &[Some(OnionFigureOverride::default()), Some(OnionFigureOverride::default())]
                ),
                plain,
                "all-inheriting overrides at {prev}/{next}"
            );
        }
    }

    #[test]
    fn onion_override_depths_are_per_figure() {
        let core = onion_two_figure_core();
        let near = 0x4D as f32 / 255.0;
        let far = 0x26 as f32 / 255.0;
        let bases = |skins: &[(Vec<stykanimate_rs::paint::StylePiece>, u32, f32)]| -> Vec<(u32, f32)> {
            skins.iter().map(|e| (e.1, e.2)).collect()
        };
        // Global 1/1, figure 1 hidden on the past side only:
        // past draws figure 0 alone; the future still draws
        // both, in figure order: past-fig0, future-fig0,
        // future-fig1 — figure 0's future ring is the
        // shared green, not a second past ring.
        let ov = [None, Some(OnionFigureOverride { prev: Some(0), ..Default::default() })];
        assert_eq!(
            bases(&skin_stream(&core, 1, 1, &ov)),
            vec![(0xE8E8EC, near), (0x7BE495, near), (0x7BE495, near)]
        );
        // Global 1/0, figure 0 deepened to 2: figure 0 gains
        // the far past ring, figure 1 stays at the global 1.
        let ov = [Some(OnionFigureOverride { prev: Some(2), ..Default::default() }), None];
        assert_eq!(
            bases(&skin_stream(&core, 1, 0, &ov)),
            vec![(0xE8E8EC, near), (0x112233, near), (0xE8E8EC, far)]
        );
        // Global 0/0 with one figure overridden on both
        // sides: only that figure draws, at its own depths.
        let ov = [
            None,
            Some(OnionFigureOverride { prev: Some(1), next: Some(2), ..Default::default() }),
        ];
        assert_eq!(
            bases(&skin_stream(&core, 0, 0, &ov)),
            vec![(0x112233, near), (0x7BE495, near), (0x7BE495, far)]
        );
        // A stale override past the figure list is inert:
        // slots are read per live figure, never by position
        // in some absolute list.
        let ov = [
            None,
            None,
            Some(OnionFigureOverride { prev: Some(3), next: Some(3), ..Default::default() }),
        ];
        assert_eq!(skin_stream(&core, 1, 1, &ov), onion_skins(&core, 1, 1));
    }

    #[test]
    fn onion_override_tints_recolour_per_direction() {
        let core = onion_two_figure_core();
        // Figure 1: magenta past, blue future. Figure 0
        // keeps the defaults on both sides.
        let ov = [
            None,
            Some(OnionFigureOverride {
                past_tint: Some(0xFF00AA),
                future_tint: Some(0x2244FF),
                ..Default::default()
            }),
        ];
        let skins = skin_stream(&core, 1, 1, &ov);
        let bases: Vec<u32> = skins.iter().map(|e| e.1).collect();
        assert_eq!(bases, vec![0xE8E8EC, 0xFF00AA, 0x7BE495, 0x2244FF]);
        // The tint is the pieces' fallback base: figure 1's
        // past pieces carry the tint (the demo figure's
        // nodes carry no colour of their own to win over it).
        assert!(skins[1].0.iter().all(|p| p.color == 0xFF00AA));
        assert!(skins[3].0.iter().all(|p| p.color == 0x2244FF));
        // A tint alone changes no geometry: same pieces,
        // recoloured — the untinted stream's spans are this
        // stream's spans.
        let plain = onion_skins(&core, 1, 1);
        assert_eq!(skins.len(), plain.len());
        for (tinted, plain) in skins.iter().zip(plain.iter()) {
            assert_eq!(tinted.0.len(), plain.0.len());
            for (tp, pp) in tinted.0.iter().zip(plain.0.iter()) {
                assert_eq!((tp.a, tp.b, tp.width), (pp.a, pp.b, pp.width));
            }
            assert_eq!(tinted.2, plain.2, "falloff is distance-only, tint or not");
        }
    }

    #[test]
    fn onion_override_store_round_trips_and_clears() {
        clear_onion_figure_overrides();
        assert!(onion_figure_overrides().is_empty());
        let ov = vec![
            None,
            Some(OnionFigureOverride { next: Some(3), ..Default::default() }),
        ];
        set_onion_figure_overrides(ov.clone());
        assert_eq!(onion_figure_overrides(), ov);
        clear_onion_figure_overrides();
        assert!(onion_figure_overrides().is_empty());
    }

    #[test]
    fn onion_override_resolution_defaults() {
        let none = OnionFigureOverride::default();
        assert!(none.is_empty());
        assert_eq!(none.prev_depth(2), 2);
        assert_eq!(none.next_depth(3), 3);
        assert_eq!(none.past_base(0xABCDEF), 0xABCDEF);
        assert_eq!(none.future_base(), FUTURE_ONION_RGB);
        let set = OnionFigureOverride {
            prev: Some(0),
            next: Some(1),
            past_tint: Some(0x101010),
            future_tint: Some(0x202020),
        };
        assert!(!set.is_empty());
        assert_eq!(set.prev_depth(2), 0, "Some(0) is a real depth, not inherit");
        assert_eq!(set.next_depth(3), 1);
        assert_eq!(set.past_base(0xABCDEF), 0x101010);
        assert_eq!(set.future_base(), 0x202020);
        // The stage's future green and the panel's mirrored
        // constant stay the one value.
        assert_eq!(FUTURE_ONION_RGB, 0x7BE495);
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
                sx: 1.0,
                sy: 1.0,
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
        // `poses_at` interpolates fractional frames only
        // while tween playback is on (model.rs), so the
        // half-frame samples below need it on.
        core.project.tween_playback = true;
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

    // ---- Motion-path point insert/delete (W2-36) ----

    #[test]
    fn path_segment_at_finds_the_segment_fraction_and_point() {
        let mut core = path_core();
        core.show_paths = true;
        // Mid-segment press between frames 1 and 2: the
        // segment, its halfway fraction, and the pressed
        // point projected onto it.
        let (f, t, q) = path_segment_at(&core, [60.0, 100.0]).expect("segment hit");
        assert_eq!(f, 1);
        assert!((t - 0.5).abs() < 1e-3, "t = {t}");
        assert!(approx2(q, [60.0, 100.0]));
        // A press just off the segment projects onto it.
        let (_, t2, q2) = path_segment_at(&core, [52.0, 108.0]).expect("near segment");
        assert!((t2 - 0.3).abs() < 1e-3, "t = {t2}");
        assert!(approx2(q2, [52.0, 100.0]), "projected: {q2:?}");
        // A press ON a point is not a split: its fraction
        // sits at a segment end.
        assert_eq!(path_segment_at(&core, [40.0, 100.0]), None);
        // Far from the path: nothing.
        assert_eq!(path_segment_at(&core, [60.0, 300.0]), None);
        // Overlay off: nothing is pickable, however close.
        core.show_paths = false;
        assert_eq!(path_segment_at(&core, [60.0, 100.0]), None);
    }

    #[test]
    fn path_insert_down_splits_only_on_a_double_press() {
        let mut core = path_core();
        core.show_paths = true;
        let depth = core.undo_depth();
        let frames = core.frame_count();
        // A single press on a segment is not this gesture —
        // it belongs to the ordinary routing and inserts
        // nothing.
        assert!(!path_insert_down(&mut core, [60.0, 100.0], false));
        assert_eq!(core.frame_count(), frames);
        assert_eq!(core.undo_depth(), depth);
        // The double-press splits the segment: one new
        // frame, exactly one undo step, the playhead on the
        // new point, its neighbours untouched.
        assert!(path_insert_down(&mut core, [60.0, 100.0], true));
        assert_eq!(core.frame_count(), frames + 1);
        assert_eq!(core.undo_depth(), depth + 1, "one insert, one undo step");
        assert_eq!(core.current, 2);
        assert!(approx2(core.project.frames[2].poses[0].root, [60.0, 100.0]));
        assert!(approx2(core.project.frames[1].poses[0].root, [40.0, 100.0]));
        assert!(approx2(core.project.frames[3].poses[0].root, [80.0, 100.0]));
        assert!(core.status.contains("Inserted path point"), "{}", core.status);
        // Playback refuses the gesture, as it refuses the
        // path drag.
        core.playing = true;
        assert!(!path_insert_down(&mut core, [100.0, 100.0], true));
        assert_eq!(core.frame_count(), frames + 1);
    }

    #[test]
    fn delete_selected_path_point_straightens_and_then_refuses() {
        let mut core = path_core();
        core.show_paths = true;
        // A spike on frame 2's point; seeking to its frame
        // is what selects it (the path press's own rule).
        core.project.frames[2].poses[0].root = [80.0, 200.0];
        core.goto(2);
        let depth = core.undo_depth();
        assert!(selected_path_point_deletable(&core), "the spike is deletable");
        assert!(delete_selected_path_point(&mut core));
        assert!(approx2(core.project.frames[2].poses[0].root, [80.0, 100.0]));
        assert_eq!(core.undo_depth(), depth + 1, "one delete, one undo step");
        // Straight now: the guard closes and the door
        // refuses without spending.
        assert!(!selected_path_point_deletable(&core));
        assert!(!delete_selected_path_point(&mut core));
        assert_eq!(core.undo_depth(), depth + 1);
        // The first frame's point anchors the path: never
        // deletable.
        core.goto(0);
        assert!(!selected_path_point_deletable(&core));
        assert!(!delete_selected_path_point(&mut core));
        // Overlay off closes the door even on a spike.
        core.project.frames[2].poses[0].root = [80.0, 200.0];
        core.goto(2);
        core.show_paths = false;
        assert!(!selected_path_point_deletable(&core));
        assert!(!delete_selected_path_point(&mut core));
        assert!(approx2(core.project.frames[2].poses[0].root, [80.0, 200.0]));
    }

    // ----------------------------------------------------
    // Bend drag (W2-08)
    // ----------------------------------------------------

    /// A core with the snap engine off (unless a test
    /// turns it on): the geometry pins read raw values.
    fn bend_test_core() -> EditorCore {
        set_snap_config(SnapConfig {
            enabled: false,
            ..SnapConfig::DEFAULT
        });
        EditorCore::new()
    }

    /// Figure 0's longest segment at the display frame:
    /// `(child node, parent point, child point)`, derived
    /// from the demo at runtime like RM15's pins.
    fn bend_test_segment(core: &EditorCore) -> (usize, V2, V2) {
        let pos = feedback_positions(core, 0);
        let mut best: Option<(usize, V2, V2, f32)> = None;
        for (ni, node) in core.project.figures[0].def.nodes.iter().enumerate() {
            let Some(par) = node.parent else { continue };
            let (a, b) = (pos[par], pos[ni]);
            let len = model::dist(a, b);
            if best.map(|(_, _, _, bl)| len > bl).unwrap_or(true) {
                best = Some((ni, a, b, len));
            }
        }
        let (ni, a, b, _) = best.expect("figure 0 has a segment");
        (ni, a, b)
    }

    #[test]
    fn bend_from_pointer_inverts_bend_mid_and_clamps() {
        set_snap_config(SnapConfig {
            enabled: false,
            ..SnapConfig::DEFAULT
        });
        let (a, b) = ([0.0, 0.0], [120.0, 0.0]);
        for want in [0.35, -0.5, 0.7, -0.7, 0.0] {
            let p = model::bend_mid(a, b, want);
            let got = bend_from_pointer(a, b, p);
            assert!((got - want).abs() < 1e-4, "bend {want}: got {got}");
        }
        // The chord midpoint itself is no bend, and a
        // point dragged far off the segment saturates at
        // the panel clamp, both ways.
        assert_eq!(bend_from_pointer(a, b, [60.0, 0.0]), 0.0);
        assert_eq!(bend_from_pointer(a, b, [60.0, 10_000.0]), BEND_LIMIT);
        assert_eq!(bend_from_pointer(a, b, [60.0, -10_000.0]), -BEND_LIMIT);
        // A degenerate segment asks nothing.
        assert_eq!(bend_from_pointer([5.0, 5.0], [5.0, 5.0], [9.0, 9.0]), 0.0);
    }

    #[test]
    fn bend_from_pointer_honours_the_snap_engine() {
        let (a, b) = ([0.0, 0.0], [120.0, 0.0]);
        let p = model::bend_mid(a, b, 0.35); // ≈ 20.05 degrees
        set_snap_config(SnapConfig::DEFAULT); // enabled, 15-degree step
        let snapped = bend_from_pointer(a, b, p);
        assert!(
            (snapped - 15f32.to_radians()).abs() < 1e-4,
            "snapped {snapped}"
        );
        set_snap_config(SnapConfig {
            enabled: false,
            ..SnapConfig::DEFAULT
        });
        let raw = bend_from_pointer(a, b, p);
        assert!((raw - 0.35).abs() < 1e-4, "raw {raw}");
    }

    #[test]
    fn bend_hit_owns_the_middle_not_the_ends() {
        let mut core = bend_test_core();
        let (ni, a, b) = bend_test_segment(&core);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        // The chord midpoint is the bend grab.
        assert_eq!(
            bend_hit(&core, mid, 13.0).map(|(fi, n, _)| (fi, n)),
            Some((0, ni))
        );
        // Near the child end the press belongs to the Aim
        // grab (IA-RM15-5): this segment does not claim it.
        let near_end = [
            a[0] + (b[0] - a[0]) * 0.05,
            a[1] + (b[1] - a[1]) * 0.05,
        ];
        assert_ne!(
            bend_hit(&core, near_end, 13.0).map(|(fi, n, _)| (fi, n)),
            Some((0, ni))
        );
        // Far from every segment: nothing.
        assert_eq!(bend_hit(&core, [5000.0, 5000.0], 13.0), None);
        // Setup mode draws straight and picks no bend.
        core.setup_mode = true;
        assert_eq!(bend_hit(&core, mid, 13.0), None);
    }

    #[test]
    fn bend_hit_finds_the_arc_midpoint_off_the_chord() {
        let mut core = bend_test_core();
        let (ni, a, b) = bend_test_segment(&core);
        // Author a 0.7 bend through the house op.
        core.selected_figure = 0;
        core.selected_node = Some(ni);
        core.cycle_bend();
        core.cycle_bend();
        let stored = core.project.frames[0].bends[0][ni];
        assert!((stored - 0.7).abs() < 1e-6, "cycle lands on 0.7: {stored}");
        let mid = model::bend_mid(a, b, 0.7);
        let (d_chord, _) = dist_to_segment(mid, a, b);
        // At a radius tighter than the arc's off-chord
        // distance, only the midpoint rule can catch the
        // press — the chord itself is out of reach.
        let radius = d_chord / 2.0;
        assert!(radius > 0.5, "the longest segment arcs visibly: off {d_chord}");
        assert_eq!(
            bend_hit(&core, mid, radius).map(|(fi, n, _)| (fi, n)),
            Some((0, ni))
        );
    }

    #[test]
    fn bend_drag_settles_as_one_labelled_step() {
        let mut core = bend_test_core();
        let (ni, a, b) = bend_test_segment(&core);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let target = model::bend_mid(a, b, 0.7);
        let depth = core.undo_depth();
        assert!(bend_drag_down(&mut core, mid, 13.0));
        assert_eq!(core.selected_node, Some(ni));
        // Moves write nothing — no row, no undo step —
        // but the preview tracks the pointer, and the
        // draw row bends without the project doing so.
        assert!(bend_drag_move(&mut core, target));
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.project.frames[0].bends.is_empty()
                || core.project.frames[0].bends[0][ni] == 0.0
        );
        let (_, _, preview) = bend_drag_preview().expect("session live");
        assert!((preview - 0.7).abs() < 1e-3, "preview {preview}");
        let row = bend_preview_apply(0, 0.0, Vec::new());
        assert!((row[ni] - 0.7).abs() < 1e-3, "preview row {row:?}");
        // The release lands exactly one labelled step.
        bend_drag_up(&mut core, target, false);
        assert_eq!(core.undo_depth(), depth + 1, "one gesture, one undo step");
        assert_eq!(core.undo_label(), Some("Bend segment"));
        let stored = core.project.frames[0].bends[0][ni];
        assert!((stored - 0.7).abs() < 1e-3, "stored {stored}");
        assert!(core.status.contains("Bend: figure 1"), "{}", core.status);
        // One undo restores the straight segment.
        core.undo();
        assert!(
            core.project.frames[0].bends.is_empty()
                || core.project.frames[0].bends[0][ni] == 0.0
        );
    }

    #[test]
    fn bend_drag_click_and_cancel_spend_nothing() {
        let mut core = bend_test_core();
        let (ni, a, b) = bend_test_segment(&core);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let depth = core.undo_depth();
        // A click: press and release at the same point —
        // a selection, not a bend.
        assert!(bend_drag_down(&mut core, mid, 13.0));
        bend_drag_up(&mut core, mid, false);
        assert_eq!(core.undo_depth(), depth);
        assert!(!bend_drag_active());
        // A cancelled drag: the preview lived, nothing
        // landed, nothing is owed.
        let target = model::bend_mid(a, b, 0.7);
        assert!(bend_drag_down(&mut core, mid, 13.0));
        assert!(bend_drag_move(&mut core, target));
        bend_drag_up(&mut core, target, true);
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.project.frames[0].bends.is_empty()
                || core.project.frames[0].bends[0][ni] == 0.0
        );
        assert!(!bend_drag_active());
    }

    #[test]
    fn bend_drag_refusals_are_honest() {
        let mut core = bend_test_core();
        let (_ni, a, b) = bend_test_segment(&core);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        // Playing.
        core.playing = true;
        assert!(!bend_drag_down(&mut core, mid, 13.0));
        core.playing = false;
        // Setup mode.
        core.setup_mode = true;
        assert!(!bend_drag_down(&mut core, mid, 13.0));
        core.setup_mode = false;
        // Locked: refuses, spends nothing, and writes no
        // status of its own — the shell's fall-through
        // posts the Aim arm's standing refusal.
        core.project.figures[0].locked = true;
        let status = core.status.clone();
        let depth = core.undo_depth();
        assert!(!bend_drag_down(&mut core, mid, 13.0));
        assert_eq!(core.status, status);
        assert_eq!(core.undo_depth(), depth);
        assert!(!bend_drag_active());
        core.project.figures[0].locked = false;
        // A press far from every segment arms nothing.
        assert!(!bend_drag_down(&mut core, [5000.0, 5000.0], 13.0));
    }

    #[test]
    fn bend_drag_hud_names_the_preview_bend() {
        let mut core = bend_test_core();
        let (_ni, a, b) = bend_test_segment(&core);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        assert_eq!(bend_drag_hud(), None);
        assert!(bend_drag_down(&mut core, mid, 13.0));
        assert!(bend_drag_move(&mut core, model::bend_mid(a, b, -0.35)));
        assert_eq!(bend_drag_hud().as_deref(), Some("BEND -20 DEG"));
        bend_drag_up(&mut core, model::bend_mid(a, b, -0.35), true);
        assert_eq!(bend_drag_hud(), None);
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
                // Roadmap Wave 2 (W2-45): the Pixel style,
                // pinned explicitly — this pin covers the
                // BITMAP prim wiring (under the default
                // Shaped style a resolvable line leaves the
                // prim stream for the element layer; that
                // partition is pinned in stage_text).
                font: stykanimate_rs::subtitles::SubtitleFont::Pixel,
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
    // Stage sprite bitmap layer (Roadmap Wave 2, W2-06)
    // --------------------------------------------------------

    /// An encoder-made solid PNG (the scenes.rs fixture
    /// recipe): the only honest way to plant an asset whose
    /// bytes decode.
    fn solid_sprite_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 200, 0, 255]));
        let mut buf = Vec::new();
        let _ = image::DynamicImage::ImageRgba8(img).write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        );
        buf
    }

    fn sprite_key() -> model::InstanceKey {
        model::InstanceKey {
            frame: 0.0,
            x: 300.0,
            y: 250.0,
            scale: 2.0,
            angle: -12.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    /// The media pass's sprite corner formula, replicated
    /// from `media::media_items` (the quad the export
    /// composite draws): corners in [c(0,0), c(1,0),
    /// c(1,1), c(0,1)] order.
    fn media_corners(key: &model::InstanceKey, w: f32, h: f32) -> [V2; 4] {
        let rad = key.angle.to_radians();
        let u = [rad.cos(), rad.sin()];
        let v = [-rad.sin(), rad.cos()];
        let wu = w * key.scale;
        let hv = h * key.eff_scale_y();
        let fx = if key.flip_x { -1.0 } else { 1.0 };
        let fy = if key.flip_y { -1.0 } else { 1.0 };
        let corner = |i: f32, j: f32| {
            [
                key.x + u[0] * (fx * (i - key.pivot_x) * wu) + v[0] * (fy * (j - key.pivot_y) * hv),
                key.y + u[1] * (fx * (i - key.pivot_x) * wu) + v[1] * (fy * (j - key.pivot_y) * hv),
            ]
        };
        [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)]
    }

    /// Where the element build presents a local point of the
    /// sprite's rect: the layout box placed at `top_left`,
    /// turned by `rotate_rad` about the origin point — Rune's
    /// `Turn::about` matrix (y-down, from the pinned crate).
    fn presented(p: &SpriteBitmapPlacement, local: [f32; 2]) -> [f32; 2] {
        let o = [p.origin[0] * p.size[0], p.origin[1] * p.size[1]];
        let d = [local[0] - o[0], local[1] - o[1]];
        let (sn, c) = p.rotate_rad.sin_cos();
        [
            p.top_left[0] + o[0] + d[0] * c - d[1] * sn,
            p.top_left[1] + o[1] + d[0] * sn + d[1] * c,
        ]
    }

    #[test]
    fn sprite_bitmap_placement_reproduces_the_canvas_quad() {
        // (angle, flip_x, flip_y, pivot_x, pivot_y,
        //  camera angle, scale_y) — plain, flipped, off-centre
        // pivots, a rotated camera, a following and an
        // explicit second-axis scale.
        let cases: [(f32, bool, bool, f32, f32, f32, f32); 5] = [
            (-12.0, false, false, 0.5, 0.5, 0.0, 0.0),
            (30.0, true, false, 0.25, 0.75, 0.0, 0.0),
            (-10.0, false, true, 0.5, 0.5, 20.0, 0.0),
            (45.0, true, true, 0.0, 1.0, -35.0, 3.0),
            (90.0, false, false, 1.0, 0.0, 15.0, 0.0),
        ];
        for (angle, flip_x, flip_y, pivot_x, pivot_y, cam_angle, scale_y) in cases {
            let key = model::InstanceKey {
                angle,
                flip_x,
                flip_y,
                pivot_x,
                pivot_y,
                scale_y,
                opacity: 0.8,
                ..sprite_key()
            };
            let corners = media_corners(&key, 48.0, 24.0);
            let quad = crate::stage_sprites::SpriteQuad {
                sprite: 0,
                asset: 0,
                name: "badge.png".to_string(),
                corners,
                centre: [
                    (corners[0][0] + corners[2][0]) / 2.0,
                    (corners[0][1] + corners[2][1]) / 2.0,
                ],
                alpha: 0.8,
            };
            let rt = crate::rune_sec::stage_camera::RotTransform::from_parts(
                [800.0, 600.0],
                [640.0, 360.0],
                (320.0, 180.0, 1.0),
                cam_angle,
            );
            let p = sprite_bitmap_placement(&quad, &key, (48, 24), &rt);
            assert_eq!(p.alpha, 0.8, "the quad's alpha carries through");
            // The element's local corner for the ORIGINAL
            // image corner (i, j) sits at (i, j) of the baked
            // variant — mirrored on a flipped axis.
            for (k, (i, j)) in
                [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].into_iter().enumerate()
            {
                let local = [
                    (if flip_x { 1.0 - i } else { i }) * p.size[0],
                    (if flip_y { 1.0 - j } else { j }) * p.size[1],
                ];
                let want = rt.to_screen(corners[k]);
                let got = presented(&p, local);
                assert!(
                    (got[0] - want[0]).abs() < 1e-3 && (got[1] - want[1]).abs() < 1e-3,
                    "angle {angle} flips ({flip_x}, {flip_y}) camera {cam_angle} corner {k}: \
                     presented {got:?}, canvas {want:?}"
                );
            }
        }
    }

    #[test]
    fn sprite_bitmap_layers_mount_placed_sprites_only() {
        reset_view_nav();
        let plain = EditorCore::new();
        assert!(
            sprite_bitmap_placements(&plain, [800.0, 600.0]).is_empty(),
            "the demo project places no sprites"
        );
        assert!(
            stage_sprite_layers(&plain, [800.0, 600.0]).is_empty(),
            "no placements, no layer — the tree is unchanged"
        );

        let mut core = EditorCore::new();
        core.project.assets = vec![model::ImageAsset {
            name: "badge.png".to_string(),
            png: solid_sprite_png(48, 24),
        }];
        core.project.sprites = vec![model::SpriteInstance {
            asset: 0,
            keys: vec![sprite_key()],
        }];
        let mut flipped = sprite_key();
        flipped.x = 500.0;
        flipped.flip_x = true;
        flipped.pivot_x = 0.25;
        core.project.sprites.push(model::SpriteInstance {
            asset: 0,
            keys: vec![flipped],
        });

        let rt = display_transform(&core, [800.0, 600.0]);
        let placements = sprite_bitmap_placements(&core, [800.0, 600.0]);
        assert_eq!(placements.len(), 2, "both instances place");
        assert_eq!(
            (placements[0].sprite, placements[1].sprite),
            (0, 1),
            "painter's order"
        );
        assert_eq!(
            placements[0].size,
            [48.0 * 2.0 * rt.vt.scale, 24.0 * 2.0 * rt.vt.scale],
            "asset pixels × key scale × view scale"
        );
        assert_eq!(
            placements[1].origin[0], 0.75,
            "a flipped axis reads its pivot from the far side"
        );
        assert!(
            (placements[0].rotate_rad - (-12.0 - rt.angle_deg).to_radians()).abs() < 1e-6,
            "element rotation is key angle minus camera angle"
        );
        assert_eq!(
            stage_sprite_layers(&core, [800.0, 600.0]).len(),
            1,
            "exactly one layer container mounts"
        );
        reset_view_nav();
    }

    #[test]
    fn sprite_bitmap_bakes_the_four_flip_variants() {
        let mut core = EditorCore::new();
        core.project.assets = vec![model::ImageAsset {
            name: "v.png".to_string(),
            png: solid_sprite_png(20, 10),
        }];
        for (flip_x, flip_y) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let img = sprite_bitmap(&core.project, 0, flip_x, flip_y)
                .expect("each flip variant bakes");
            assert_eq!(img.size(), Some([20, 10]), "native resolution, no resize");
        }
        assert!(
            sprite_bitmap(&core.project, 9, false, false).is_none(),
            "a missing asset bakes nothing"
        );
    }

    #[test]
    fn stage_shape_no_longer_paints_the_sprite_placeholder() {
        reset_view_nav();
        let size = [800.0, 600.0];
        let fills = |core: &EditorCore| {
            format!("{:?}", stage_shape(core, size, StageOverlays::default()))
                .matches("Fill { points:")
                .count()
        };
        let mut with = EditorCore::new();
        with.project.assets = vec![model::ImageAsset {
            name: "badge.png".to_string(),
            png: solid_sprite_png(48, 24),
        }];
        with.project.sprites = vec![model::SpriteInstance {
            asset: 0,
            keys: vec![sprite_key()],
        }];
        let without = EditorCore::new();
        let quads = crate::stage_sprites::sprite_quads(&with.project, with.display_frame());
        assert_eq!(quads.len(), 1, "the planted sprite places one quad");
        // The label's glyph boxes are the ONLY fills the
        // sprite still adds to the canvas: the placeholder
        // fill retired with the bitmap layer (W2-06), and
        // the corner marks (a selected sprite's) are lines.
        let label_fills = crate::rune_sec::stage_text::sprite_label_boxes(
            quads[0].corners,
            "badge.png",
            SELECTION_ACCENT,
        )
        .len();
        assert!(label_fills > 0, "the label lays out glyph boxes");
        assert_eq!(
            fills(&with),
            fills(&without) + label_fills,
            "only the name label's glyph boxes remain — no placeholder fill"
        );
        // And the quad's own screen points are painted by no
        // canvas prim any more (the bitmap element shows the
        // artwork at exactly these corners instead).
        let rt = display_transform(&with, size);
        let pts: Vec<[f32; 2]> = quads[0].corners.iter().map(|p| rt.to_screen(*p)).collect();
        let debug = format!("{:?}", stage_shape(&with, size, StageOverlays::default()));
        assert!(
            debug.contains("Fill { points:"),
            "the counting method sees the stage's fills"
        );
        assert!(
            !debug.contains(&format!("{:?}", pts)),
            "the sprite quad's points are not painted on the canvas"
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
    fn group_drag_hud_names_the_set_and_its_delta() {
        // W2-07: while a group drag is live the HUD names
        // the grabbed set and the displacement it will
        // settle by — signed, because a move's sign is its
        // information.

        assert_eq!(group_drag_hud(), None, "no group drag, no line");
        let mut core = marquee_core();
        set_figure_set(&mut core, vec![0, 1]);
        let root0 = core.project.frames[0].poses[0].root;
        assert!(group_drag_down(&mut core, root0, 13.0));
        assert!(group_drag_move(&mut core, [root0[0] + 30.0, root0[1] - 10.0]));
        assert_eq!(
            group_drag_hud(),
            Some("MOVE 2 FIGURES +30, -10".to_string())
        );
        group_drag_up(&mut core, [root0[0] + 30.0, root0[1] - 10.0], true);
        assert_eq!(group_drag_hud(), None, "the drag is over");
        clear_figure_set(&mut core);
    }

    #[test]
    fn path_drag_hud_names_the_frame_and_its_delta() {
        // W2-07: the path drag's line names the grabbed
        // frame (numbered from 1, as the UI numbers
        // frames) and the live displacement the release
        // will nudge its root by.
        let mut core = path_core();
        core.show_paths = true;
        assert_eq!(path_drag_hud(&core), None, "no path drag, no line");
        assert!(path_drag_down(&mut core, [40.0, 100.0]));
        assert!(path_drag_move(&mut core, [55.0, 95.0]));
        assert_eq!(
            path_drag_hud(&core),
            Some("PATH FRAME 2 +15, -5".to_string())
        );
        path_drag_up(&mut core, [55.0, 95.0], true);
        assert_eq!(path_drag_hud(&core), None, "the drag is over");
    }

    #[test]
    fn marquee_hud_lines_track_the_band() {
        // W2-07: a band still inside the click slop shows
        // nothing (it is a click in progress); past it the
        // HUD names the rectangle's size and how many
        // figures it would select.

        let mut core = marquee_core();
        assert!(marquee_down(&mut core, [1500.0, 1500.0]));
        assert!(marquee_move([1501.0, 1502.0]));
        assert!(marquee_hud_lines(&core).is_empty(), "inside the slop");
        marquee_up(&mut core, [1501.0, 1502.0], true);
        let bb = figure_stage_bbox(&core, 0).unwrap();
        assert!(marquee_down(&mut core, [bb[2] + 30.0, bb[3] + 30.0]));
        assert!(marquee_move([bb[0] - 30.0, bb[1] - 30.0]));
        let rect = marquee_live_rect().unwrap();
        assert_eq!(
            marquee_hud_lines(&core),
            vec![
                format!(
                    "MARQUEE {:.0} X {:.0}",
                    rect[2] - rect[0],
                    rect[3] - rect[1]
                ),
                "FIGURES 1".to_string(),
            ]
        );
        marquee_up(&mut core, [bb[0] - 30.0, bb[1] - 30.0], true);
        assert!(marquee_hud_lines(&core).is_empty(), "the band is gone");
        clear_figure_set(&mut core);
    }

    #[test]
    fn gizmo_hud_line_spells_move_and_rotate() {
        // W2-07: the gizmo's pure formatter — a move's
        // signed displacement, a rotate's snapped angle.
        // (The session call site is IA-W2-07-1: it needs
        // stage_gizmo registered, IA-W2-03-1.)
        assert_eq!(gizmo_hud_line(false, [15.0, -5.0], 0.0), "MOVE +15, -5");
        assert_eq!(gizmo_hud_line(true, [0.0, 0.0], -30.0), "ROTATE -30 DEG");
        assert_eq!(gizmo_hud_line(true, [0.0, 0.0], 45.0), "ROTATE +45 DEG");
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
    fn nudge_figure_moves_the_whole_character_and_coalesces() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        reset_nudge_burst();
        let before = core.project.frames[0].poses[0].root;
        let depth = core.undo_depth();
        assert!(nudge_figure(&mut core, 0, 5.0, -3.0));
        assert_eq!(
            core.project.frames[0].poses[0].root,
            [before[0] + 5.0, before[1] - 3.0]
        );
        assert_eq!(
            core.status,
            format!(
                "Nudged figure 1 root to ({:.0}, {:.0}).",
                before[0] + 5.0,
                before[1] - 3.0
            )
        );
        assert_eq!(core.undo_depth(), depth + 1, "the burst's one snapshot");
        // Key-repeat inside the window rides the same snapshot.
        assert!(nudge_figure(&mut core, 0, 1.0, 0.0));
        assert!(nudge_figure(&mut core, 0, 1.0, 0.0));
        assert_eq!(core.undo_depth(), depth + 1, "repeats coalesce");
        // A character burst and a joint burst never share a
        // step: their stamps key differently (ni None vs Some).
        core.selected_node = Some(0);
        assert!(nudge_selected(&mut core, 1.0, 0.0));
        assert_eq!(core.undo_depth(), depth + 2, "the joint burst snapshots afresh");
    }

    #[test]
    fn nudge_figure_refuses_locked_playing_setup_and_stale() {
        let mut core = EditorCore::new();
        reset_nudge_burst();
        let before = core.project.frames[0].poses[0].root;
        let depth = core.undo_depth();
        // Stale index: silent, nothing spent.
        assert!(!nudge_figure(&mut core, 99, 1.0, 0.0));
        // Locked: the drag path's own sentence, nothing spent.
        core.project.figures[0].locked = true;
        assert!(!nudge_figure(&mut core, 0, 1.0, 0.0));
        assert_eq!(core.status, "Figure is locked — unlock it in the figures list.");
        core.project.figures[0].locked = false;
        // Playing: silent, as a press during playback is.
        core.playing = true;
        assert!(!nudge_figure(&mut core, 0, 1.0, 0.0));
        core.playing = false;
        // Setup mode: a rest pose has no root to move.
        core.setup_mode = true;
        assert!(!nudge_figure(&mut core, 0, 1.0, 0.0));
        assert_eq!(
            core.status,
            "The rest pose has no root position — roots live in frame poses."
        );
        core.setup_mode = false;
        assert_eq!(core.project.frames[0].poses[0].root, before);
        assert_eq!(core.undo_depth(), depth, "refusals spend no snapshot");
    }

    #[test]
    fn character_arm_turns_arrows_to_the_character_until_released() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        release_character_arm();
        clear_figure_set(&mut core);
        reset_nudge_burst();
        // Nothing selected: the arrows are frame keys.
        assert!(!arrows_move_selection(&core));
        assert_eq!(nudge_arrows(&mut core, 1.0, 0.0), ArrowMove::None);
        // Arm the character (a row press / the watcher does
        // this): the arrows now move it whole.
        note_character_selected(0);
        assert!(arrows_move_selection(&core));
        let before = core.project.frames[0].poses[0].root;
        assert_eq!(nudge_arrows(&mut core, 2.0, 0.0), ArrowMove::Character);
        assert_eq!(core.project.frames[0].poses[0].root, [before[0] + 2.0, before[1]]);
        // Released: frame keys again, and no write happens.
        release_character_arm();
        assert!(!arrows_move_selection(&core));
        assert_eq!(nudge_arrows(&mut core, 1.0, 0.0), ArrowMove::None);
        assert_eq!(core.project.frames[0].poses[0].root, [before[0] + 2.0, before[1]]);
        // An arm naming a figure the core is not showing is
        // stale next to the primary selection: no capture.
        note_character_selected(1);
        assert!(!arrows_move_selection(&core), "selected_figure is 0; the arm names 1");
        release_character_arm();
    }

    #[test]
    fn nudge_arrows_prefers_the_joint_when_one_is_selected() {
        let mut core = EditorCore::new();
        let (ni, _) = first_segment(&core);
        release_character_arm();
        clear_figure_set(&mut core);
        reset_nudge_burst();
        note_character_selected(0);
        core.selected_node = Some(ni);
        let root_before = core.project.frames[0].poses[0].root;
        let joint_before = core.positions(0, 0)[ni];
        assert_eq!(nudge_arrows(&mut core, 30.0, 0.0), ArrowMove::Joint);
        assert_eq!(
            core.project.frames[0].poses[0].root, root_before,
            "a joint nudge aims; it never drags the root"
        );
        assert!(core.positions(0, 0)[ni] != joint_before, "the joint moved");
        release_character_arm();
    }

    #[test]
    fn nudge_figures_moves_the_set_with_one_snapshot() {
        let mut core = EditorCore::new();
        core.add_figure();
        core.select_figure(0);
        reset_nudge_burst();
        let r0 = core.project.frames[0].poses[0].root;
        let r1 = core.project.frames[0].poses[1].root;
        let depth = core.undo_depth();
        // Order and duplicates in the input are irrelevant.
        assert_eq!(nudge_figures(&mut core, &[1, 0, 0], 4.0, 2.0), 2);
        assert_eq!(core.project.frames[0].poses[0].root, [r0[0] + 4.0, r0[1] + 2.0]);
        assert_eq!(core.project.frames[0].poses[1].root, [r1[0] + 4.0, r1[1] + 2.0]);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the whole set");
        assert_eq!(nudge_figures(&mut core, &[0, 1], 1.0, 0.0), 2);
        assert_eq!(core.undo_depth(), depth + 1, "the set's burst coalesces");
        // A locked member stays put and is named; the rest
        // still move — under a new burst, because the
        // movable set (the stamp's key) changed.
        core.project.figures[1].locked = true;
        let depth = core.undo_depth();
        assert_eq!(nudge_figures(&mut core, &[0, 1], 1.0, 0.0), 1);
        assert_eq!(
            core.project.frames[0].poses[1].root,
            [r1[0] + 5.0, r1[1] + 2.0],
            "the locked figure did not move in this call"
        );
        assert!(core.status.contains("locked, skipped"), "{}", core.status);
        assert_eq!(core.undo_depth(), depth + 1);
        // Every member locked: refusal, no snapshot.
        core.project.figures[0].locked = true;
        let depth = core.undo_depth();
        assert_eq!(nudge_figures(&mut core, &[0, 1], 1.0, 0.0), 0);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn marquee_set_is_the_arrow_target_when_it_holds_the_primary() {
        let mut core = EditorCore::new();
        core.add_figure();
        release_character_arm();
        clear_figure_set(&mut core);
        reset_nudge_burst();
        // A committed marquee (W2-01): the set holds both
        // figures and its topmost is the core's selection —
        // exactly marquee_up's writes.
        set_figure_set(&mut core, vec![0, 1]);
        core.select_figure(1);
        assert!(arrows_move_selection(&core));
        let r0 = core.project.frames[0].poses[0].root;
        let r1 = core.project.frames[0].poses[1].root;
        assert_eq!(nudge_arrows(&mut core, 3.0, 0.0), ArrowMove::Characters(2));
        assert_eq!(core.project.frames[0].poses[0].root, [r0[0] + 3.0, r0[1]]);
        assert_eq!(core.project.frames[0].poses[1].root, [r1[0] + 3.0, r1[1]]);
        // A set that does not hold the primary yields: the
        // core's selection has moved on to another figure.
        core.select_figure(0);
        set_figure_set(&mut core, vec![1]);
        assert!(!arrows_move_selection(&core));
        assert_eq!(nudge_arrows(&mut core, 1.0, 0.0), ArrowMove::None);
        clear_figure_set(&mut core);
    }

    #[test]
    fn sync_arrow_selection_arms_and_releases_with_the_shells_acts() {
        let mut core = EditorCore::new();
        release_character_arm();
        clear_figure_set(&mut core);
        // Baseline: the first sighting only records; a fresh
        // shell has selected nothing.
        core.selected_node = Some(0);
        sync_arrow_selection(&core);
        assert!(armed_characters().is_empty());
        // select_figure's signature — same figure, node
        // cleared — arms the character.
        core.select_figure(0);
        sync_arrow_selection(&core);
        assert_eq!(armed_characters(), vec![0]);
        assert!(arrows_move_selection(&core));
        // A frame change by other means releases the arm:
        // the arrows are frame keys again.
        if core.frame_count() < 2 {
            core.add_frame();
        }
        core.goto(1);
        sync_arrow_selection(&core);
        assert!(armed_characters().is_empty());
        assert!(!arrows_move_selection(&core));
        // Switching to another bare figure arms that one.
        core.add_figure();
        core.select_figure(1);
        sync_arrow_selection(&core);
        assert_eq!(armed_characters(), vec![1]);
        release_character_arm();
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

    // ---- Marquee multi-selection (Roadmap Wave 2, W2-01) ----

    /// Three figures rooted far apart on every frame, so
    /// each owns an exclusive patch of stage: fig0
    /// [80, 90], fig1 [700, 90], fig2 [80, 600].
    fn marquee_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.add_figure();
        core.add_figure();
        let roots = [[80.0, 90.0], [700.0, 90.0], [80.0, 600.0]];
        for frame in core.project.frames.iter_mut() {
            for (fi, r) in roots.iter().enumerate() {
                frame.poses[fi].root = *r;
            }
        }
        core
    }

    #[test]
    fn marquee_rect_normalises_every_drag_direction() {
        let want = [10.0, 20.0, 110.0, 220.0];
        assert_eq!(marquee_rect([10.0, 20.0], [110.0, 220.0]), want);
        assert_eq!(marquee_rect([110.0, 220.0], [10.0, 20.0]), want);
        assert_eq!(marquee_rect([110.0, 20.0], [10.0, 220.0]), want);
        assert_eq!(marquee_rect([10.0, 220.0], [110.0, 20.0]), want);
        // Edges are inclusive: a joint ON the border is inside.
        assert!(marquee_rect_contains(want, [10.0, 20.0]));
        assert!(marquee_rect_contains(want, [110.0, 220.0]));
        assert!(!marquee_rect_contains(want, [110.5, 100.0]));
    }

    #[test]
    fn segment_intersects_marquee_rect_by_crossing_touch_and_miss() {
        let rect = [100.0, 100.0, 200.0, 200.0];
        // Straight through, both endpoints outside.
        assert!(segment_intersects_marquee_rect([0.0, 150.0], [300.0, 150.0], rect));
        // One endpoint inside.
        assert!(segment_intersects_marquee_rect([150.0, 150.0], [400.0, 400.0], rect));
        // Lying along the top edge (collinear touch).
        assert!(segment_intersects_marquee_rect([50.0, 100.0], [250.0, 100.0], rect));
        // Parallel miss above the square.
        assert!(!segment_intersects_marquee_rect([0.0, 50.0], [300.0, 50.0], rect));
        // Diagonal miss past the corner.
        assert!(!segment_intersects_marquee_rect([210.0, 0.0], [300.0, 90.0], rect));
    }

    #[test]
    fn figures_in_marquee_selects_by_joint_and_by_crossing_segment() {
        let core = marquee_core();
        // A rectangle over figure 0's own bbox selects it alone.
        let bb = figure_stage_bbox(&core, 0).expect("figure 0 has a bbox");
        assert_eq!(figures_in_marquee(&core, bb), vec![0]);
        // A rectangle over empty stage selects nothing.
        assert!(figures_in_marquee(&core, [900.0, 900.0, 1000.0, 1000.0]).is_empty());
        // A small square around figure 0's longest segment's
        // midpoint — neither endpoint inside (checked, not
        // assumed) — still selects it, through the segment leg.
        let pos = core.positions(0, 0);
        let fig = &core.project.figures[0];
        let (a, b) = fig
            .def
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(ni, n)| n.parent.map(|par| (pos[par], pos[ni])))
            .max_by(|x, y| {
                model::dist(x.0, x.1)
                    .partial_cmp(&model::dist(y.0, y.1))
                    .unwrap()
            })
            .expect("figure 0 has segments");
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let half = (model::dist(a, b) * 0.15).min(5.0);
        let rect = [mid[0] - half, mid[1] - half, mid[0] + half, mid[1] + half];
        assert!(!marquee_rect_contains(rect, a));
        assert!(!marquee_rect_contains(rect, b));
        assert_eq!(figures_in_marquee(&core, rect), vec![0]);
    }

    #[test]
    fn figures_in_marquee_skips_layers_hidden_figures() {
        let mut core = marquee_core();
        core.project.figures[1].hidden = true;
        let everywhere = [-1000.0, -1000.0, 3000.0, 3000.0];
        assert_eq!(figures_in_marquee(&core, everywhere), vec![0, 2]);
        // A hidden figure has no stage bbox either.
        assert!(figure_stage_bbox(&core, 1).is_none());
        assert!(figure_stage_bbox(&core, 99).is_none());
    }

    #[test]
    fn marquee_session_commits_replaces_and_clears() {

        let mut core = marquee_core();
        let bb = figure_stage_bbox(&core, 0).unwrap();
        // Drag a square around figure 0 (either direction).
        assert!(marquee_down(&mut core, [bb[2] + 30.0, bb[3] + 30.0]));
        assert!(marquee_move([bb[0] - 30.0, bb[1] - 30.0]));
        assert_eq!(
            marquee_live_rect(),
            Some(marquee_rect([bb[2] + 30.0, bb[3] + 30.0], [bb[0] - 30.0, bb[1] - 30.0]))
        );
        marquee_up(&mut core, [bb[0] - 30.0, bb[1] - 30.0], false);
        assert_eq!(figure_set(&core), vec![0]);
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.status, "Selected 1 figure.");
        assert!(marquee_live_rect().is_none());
        // A click on empty stage (inside the slop) clears the set.
        assert!(marquee_down(&mut core, [1500.0, 1500.0]));
        marquee_up(&mut core, [1501.0, 1502.0], false);
        assert!(figure_set(&core).is_empty());
        // A cancelled drag changes nothing.
        set_figure_set(&mut core, vec![0, 1]);
        assert!(marquee_down(&mut core, [0.0, 0.0]));
        assert!(marquee_move([400.0, 400.0]));
        marquee_up(&mut core, [400.0, 400.0], true);
        assert_eq!(figure_set(&core), vec![0, 1]);
        // Playing refuses the gesture outright.
        core.playing = true;
        assert!(!marquee_down(&mut core, [0.0, 0.0]));
        clear_figure_set(&mut core);
    }

    #[test]
    fn group_drag_moves_the_set_for_exactly_one_undo_step() {

        let mut core = marquee_core();
        set_figure_set(&mut core, vec![0, 1]);
        let root0 = core.project.frames[0].poses[0].root;
        let root1 = core.project.frames[0].poses[1].root;
        let root2 = core.project.frames[0].poses[2].root;
        let depth = core.undo_depth();
        // Press on figure 0's root joint: the SET grabs.
        assert!(group_drag_down(&mut core, root0, 13.0));
        assert!(group_drag_move(&mut core, [root0[0] + 30.0, root0[1] + 10.0]));
        assert_eq!(group_drag_delta(), Some([30.0, 10.0]));
        // Nothing written yet — the move is preview-only.
        assert_eq!(core.project.frames[0].poses[0].root, root0);
        group_drag_up(&mut core, [root0[0] + 30.0, root0[1] + 10.0], false);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per gesture");
        assert_eq!(
            core.project.frames[0].poses[0].root,
            [root0[0] + 30.0, root0[1] + 10.0]
        );
        assert_eq!(
            core.project.frames[0].poses[1].root,
            [root1[0] + 30.0, root1[1] + 10.0]
        );
        // Figure 2 is not in the set: untouched.
        assert_eq!(core.project.frames[0].poses[2].root, root2);
        assert_eq!(core.status, "Moved 2 figures at frame 1.");
        assert!(group_drag_delta().is_none());
        clear_figure_set(&mut core);
    }

    #[test]
    fn group_drag_guards_and_zero_spend_releases() {

        let mut core = marquee_core();
        let root0 = core.project.frames[0].poses[0].root;
        let root2 = core.project.frames[0].poses[2].root;
        // A single-figure "set" is not a group: the pose
        // drag owns the press.
        set_figure_set(&mut core, vec![0]);
        assert!(!group_drag_down(&mut core, root0, 13.0));
        // A press on a figure outside the set is not a group grab.
        set_figure_set(&mut core, vec![0, 1]);
        assert!(!group_drag_down(&mut core, root2, 13.0));
        // Zero-displacement release: no snapshot, no write.
        let depth = core.undo_depth();
        assert!(group_drag_down(&mut core, root0, 13.0));
        group_drag_up(&mut core, root0, false);
        assert_eq!(core.undo_depth(), depth);
        // Cancelled release: no snapshot, no write.
        assert!(group_drag_down(&mut core, root0, 13.0));
        assert!(group_drag_move(&mut core, [root0[0] + 50.0, root0[1]]));
        group_drag_up(&mut core, [root0[0] + 50.0, root0[1]], true);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.frames[0].poses[0].root, root0);
        clear_figure_set(&mut core);
    }

    #[test]
    fn group_drag_skips_locked_figures_and_names_them() {

        let mut core = marquee_core();
        core.project.figures[1].locked = true;
        set_figure_set(&mut core, vec![0, 1]);
        let root0 = core.project.frames[0].poses[0].root;
        let root1 = core.project.frames[0].poses[1].root;
        assert!(group_drag_down(&mut core, root0, 13.0));
        group_drag_up(&mut core, [root0[0] + 20.0, root0[1]], false);
        assert_eq!(
            core.project.frames[0].poses[0].root,
            [root0[0] + 20.0, root0[1]]
        );
        assert_eq!(core.project.frames[0].poses[1].root, root1, "locked stays");
        assert_eq!(
            core.status,
            "Moved 1 figures at frame 1; 1 locked figure stayed."
        );
        clear_figure_set(&mut core);
    }

    #[test]
    fn marquee_pass_paints_band_and_outlines_only_when_live() {
        reset_view_nav();

        let mut core = marquee_core();
        set_stage_pointer(None);
        let count = |core: &EditorCore, token: &str| {
            format!("{:?}", stage_shape(core, [800.0, 600.0], StageOverlays::default()))
                .matches(token)
                .count()
        };
        let base_fills = count(&core, "Fill { points:");
        let base_strokes = count(&core, "Stroke { points:");
        // A live marquee adds exactly its band fill (+ outline stroke).
        assert!(marquee_down(&mut core, [10.0, 10.0]));
        assert!(marquee_move([300.0, 300.0]));
        assert_eq!(count(&core, "Fill { points:"), base_fills + 1);
        assert_eq!(count(&core, "Stroke { points:"), base_strokes + 1);
        marquee_up(&mut core, [300.0, 300.0], true);
        assert_eq!(count(&core, "Fill { points:"), base_fills);
        // A two-figure set adds exactly two outline strokes, no fills.
        set_figure_set(&mut core, vec![0, 1]);
        assert_eq!(count(&core, "Stroke { points:"), base_strokes + 2);
        assert_eq!(count(&core, "Fill { points:"), base_fills);
        clear_figure_set(&mut core);
        assert_eq!(count(&core, "Stroke { points:"), base_strokes);
        reset_view_nav();
    }

    #[test]
    fn figure_set_accessors_keep_the_canonical_form() {
        let mut core = marquee_core();
        set_figure_set(&mut core, vec![2, 0, 2, 1]);
        assert_eq!(figure_set(&core), vec![0, 1, 2], "sorted + deduplicated");
        assert!(figure_set_contains(&core, 1));
        assert!(!figure_set_contains(&core, 7));
        clear_figure_set(&mut core);
        assert!(figure_set(&core).is_empty());
    }

    // ---- Stored guides & rulers (Roadmap Wave 2, W2-05) ----

    #[test]
    fn overlays_default_shows_stored_guides_and_hides_rulers() {
        // W2-05: stored guides are authored content — ON
        // by default (a fresh project has none, so the
        // default stage is unchanged); rulers are opt-in
        // chrome — OFF.
        let o = StageOverlays::default();
        assert!(o.guides_stored);
        assert!(!o.rulers);
    }

    #[test]
    fn ruler_step_climbs_the_1_2_5_ladder_by_screen_spacing() {
        // At the fit view (scale 1) majors sit 100 apart
        // (50 would be only 50 pt < the 56 pt floor).
        assert_eq!(ruler_step(1.0), 100.0);
        // Zoomed in 2x, 50 units is 100 pt: step halves.
        assert_eq!(ruler_step(2.0), 50.0);
        // Zoomed out, the ladder climbs.
        assert_eq!(ruler_step(0.5), 200.0);
        assert_eq!(ruler_step(0.1), 1000.0);
        // Deep zoom: whole-unit steps still label exactly.
        assert_eq!(ruler_step(60.0), 1.0);
        // A degenerate scale reads as 1:1, never zero.
        assert_eq!(ruler_step(0.0), 100.0);
    }

    #[test]
    fn ruler_ticks_major_every_fifth_with_integer_labels() {
        // Stage 960x720 at scale 1: step 100, minors at 20.
        let ticks = ruler_ticks([960.0, 720.0], 1.0);
        let top: Vec<&RulerTick> = ticks.iter().filter(|t| t.along_x).collect();
        let left: Vec<&RulerTick> = ticks.iter().filter(|t| !t.along_x).collect();
        // Top: minors k=0..=48 (960/20), majors every fifth.
        assert_eq!(top.len(), 49);
        assert_eq!(left.len(), 37);
        let majors: Vec<&RulerTick> = top.iter().filter(|t| t.major).copied().collect();
        assert_eq!(majors.len(), 10, "0..=900 at step 100");
        assert_eq!(majors[0].label.as_deref(), Some("0"));
        assert_eq!(majors[3].label.as_deref(), Some("300"));
        assert_eq!(majors[9].pos, 900.0);
        // Minors carry no label and sit on the fifths.
        let minor = top[1];
        assert!(!minor.major && minor.label.is_none());
        assert_eq!(minor.pos, 20.0);
        // Left ruler majors label stage y the same way.
        let left_majors: Vec<&RulerTick> =
            left.iter().filter(|t| t.major).copied().collect();
        assert_eq!(left_majors.len(), 8, "0..=700 at step 100");
        assert_eq!(left_majors[7].label.as_deref(), Some("700"));
        // A degenerate stage measures nothing.
        assert!(ruler_ticks([0.0, 0.0], 1.0).is_empty());
    }

    #[test]
    fn ruler_tick_lines_anchor_at_the_stage_edge() {
        let band = ruler_band(1.0);
        assert_eq!(band, RULER_THICKNESS_PX);
        let major = RulerTick {
            pos: 300.0,
            along_x: true,
            major: true,
            label: Some("300".into()),
        };
        assert_eq!(
            ruler_tick_line(&major, band),
            ([300.0, 0.0], [300.0, -band * 0.45]),
            "major reaches 45% into the band from the edge"
        );
        let minor = RulerTick { major: false, label: None, ..major.clone() };
        assert_eq!(
            ruler_tick_line(&minor, band),
            ([300.0, 0.0], [300.0, -band * 0.25])
        );
        let left = RulerTick { along_x: false, ..major.clone() };
        assert_eq!(
            ruler_tick_line(&left, band),
            ([0.0, 300.0], [-band * 0.45, 300.0])
        );
        // Labels centre in the band's outer part; left
        // labels run bottom-to-top.
        assert_eq!(ruler_label_placement(&major, band), ([300.0, -band * 0.75], 0.0));
        assert_eq!(
            ruler_label_placement(&left, band),
            ([-band * 0.75, 300.0], -90.0)
        );
    }

    #[test]
    fn stored_guide_lines_fold_the_live_preview() {
        use stykanimate_rs::model::GuideAxis;
        let mut core = EditorCore::new();
        assert!(stored_guide_lines(&core).is_empty(), "no guides, no lines");
        core.stored_guide_add(GuideAxis::Vertical, 480.0);
        assert_eq!(
            stored_guide_lines(&core),
            vec![([480.0, 0.0], [480.0, 720.0], [480.0, 0.0])],
            "line spans the stage; handle on the top edge"
        );
        // A MOVE preview replaces the stored position.
        assert!(guide_down(&mut core, [480.0, 300.0], 1.0, RULER_THICKNESS_PX));
        assert!(guide_move([620.0, 300.0]));
        assert_eq!(
            stored_guide_lines(&core)[0].0,
            [620.0, 0.0],
            "the drawn line follows the drag, not the data"
        );
        guide_up(&mut core, [620.0, 300.0], true);
        assert_eq!(stored_guide_lines(&core)[0].0, [480.0, 0.0], "cancel restores");
        // A CREATE preview appends a line that is not yet data.
        assert!(guide_down(&mut core, [-9.0, 200.0], 1.0, RULER_THICKNESS_PX));
        assert_eq!(stored_guide_lines(&core).len(), 2);
        assert_eq!(core.stored_guide_count(), 1, "preview writes nothing");
        guide_up(&mut core, [-9.0, 200.0], true);
        assert_eq!(stored_guide_lines(&core).len(), 1);
    }

    #[test]
    fn guide_create_drags_off_either_ruler_and_lands_one_op() {
        use stykanimate_rs::model::GuideAxis;
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        // Top ruler → a HORIZONTAL guide at the release y.
        assert!(guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        assert_eq!(
            guide_drag_preview(),
            Some((GuideAxis::Horizontal, -9.0, None))
        );
        assert!(guide_move([300.0, 250.0]));
        guide_up(&mut core, [300.0, 250.0], false);
        assert!(!guide_drag_active());
        assert_eq!(core.stored_guide_count(), 1);
        assert_eq!(
            core.stored_guides()[0],
            stykanimate_rs::model::StoredGuide {
                axis: GuideAxis::Horizontal,
                position: 250.0,
            }
        );
        assert_eq!(core.undo_depth(), depth + 1, "exactly one op for the gesture");
        // Left ruler → a VERTICAL guide at the release x.
        assert!(guide_down(&mut core, [-9.0, 300.0], 1.0, RULER_THICKNESS_PX));
        guide_up(&mut core, [200.0, 300.0], false);
        assert_eq!(core.stored_guides()[1].axis, GuideAxis::Vertical);
        assert_eq!(core.stored_guides()[1].position, 200.0);
        assert_eq!(core.undo_depth(), depth + 2);
    }

    #[test]
    fn guide_create_released_inside_the_ruler_creates_nothing() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        // Pressed in the top band, released still in it:
        // the guide was never dragged onto the stage.
        assert!(guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        guide_up(&mut core, [300.0, -5.0], false);
        assert_eq!(core.stored_guide_count(), 0);
        assert_eq!(core.undo_depth(), depth);
        // The corner square starts nothing.
        assert!(!guide_down(&mut core, [-9.0, -9.0], 1.0, RULER_THICKNESS_PX));
        assert!(!guide_drag_active());
        // A cancelled create spends nothing either.
        assert!(guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        guide_up(&mut core, [300.0, 250.0], true);
        assert_eq!(core.stored_guide_count(), 0);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn guide_move_settles_once_and_dragging_back_to_the_ruler_deletes() {
        use stykanimate_rs::model::GuideAxis;
        let mut core = EditorCore::new();
        core.stored_guide_add(GuideAxis::Vertical, 480.0);
        let depth = core.undo_depth();
        // Grab the line (within the pick radius), move it.
        assert!(guide_down(&mut core, [485.0, 300.0], 1.0, RULER_THICKNESS_PX));
        assert!(guide_move([600.0, 300.0]));
        guide_up(&mut core, [600.0, 300.0], false);
        assert_eq!(core.stored_guides()[0].position, 600.0);
        assert_eq!(core.undo_depth(), depth + 1);
        // A release on the guide's own coordinate is free
        // (the op's no-change rule): a click on a guide
        // costs no undo step.
        assert!(guide_down(&mut core, [600.0, 300.0], 1.0, RULER_THICKNESS_PX));
        guide_up(&mut core, [600.0, 300.0], false);
        assert_eq!(core.undo_depth(), depth + 1, "zero move spends nothing");
        // Drag it back beyond the left ruler: deleted.
        assert!(guide_down(&mut core, [600.0, 300.0], 1.0, RULER_THICKNESS_PX));
        guide_up(&mut core, [-30.0, 300.0], false);
        assert_eq!(core.stored_guide_count(), 0);
        assert_eq!(core.undo_depth(), depth + 2, "the delete is one op");
    }

    #[test]
    fn guide_delete_door_closes_when_the_rulers_are_hidden() {
        use stykanimate_rs::model::GuideAxis;
        let mut core = EditorCore::new();
        core.stored_guide_add(GuideAxis::Vertical, 480.0);
        // band_px = 0 (rulers hidden): CREATE is closed…
        assert!(!guide_down(&mut core, [-9.0, 300.0], 1.0, 0.0));
        // …but the line itself is still grabbable, and a
        // release past the stage edge MOVES the guide
        // there instead of deleting it unseen.
        assert!(guide_down(&mut core, [480.0, 300.0], 1.0, 0.0));
        guide_up(&mut core, [-30.0, 300.0], false);
        assert_eq!(core.stored_guide_count(), 1, "no invisible delete");
        assert_eq!(core.stored_guides()[0].position, -30.0);
    }

    #[test]
    fn guide_session_refuses_modes_and_playback() {
        let mut core = EditorCore::new();
        core.playing = true;
        assert!(!guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        core.playing = false;
        core.draw_mode = true;
        assert!(!guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        core.draw_mode = false;
        core.camera_mode = true;
        assert!(!guide_down(&mut core, [300.0, -9.0], 1.0, RULER_THICKNESS_PX));
        core.camera_mode = false;
        // A press nowhere near a ruler or a guide starts
        // nothing (it falls through to the marquee chain).
        assert!(!guide_down(&mut core, [300.0, 300.0], 1.0, RULER_THICKNESS_PX));
        assert!(!guide_drag_active());
    }

    #[test]
    fn stage_shape_draws_stored_guides_and_rulers_only_when_toggled() {
        use stykanimate_rs::model::GuideAxis;
        reset_view_nav();
        let draw = |core: &EditorCore, overlays: StageOverlays| {
            format!("{:?}", stage_shape(core, [640.0, 360.0], overlays))
        };
        let mut core = EditorCore::new();
        core.project.stage = [640.0, 360.0];
        // No guides stored: the stored flag is inert —
        // byte-identical either way (the default stage is
        // unchanged by this seat).
        let plain = draw(&core, StageOverlays::default());
        let no_stored = draw(
            &core,
            StageOverlays { guides_stored: false, ..StageOverlays::default() },
        );
        assert_eq!(plain, no_stored);
        // One stored guide: on adds its line + handle,
        // off is byte-identical to the guideless stage.
        core.stored_guide_add(GuideAxis::Vertical, 320.0);
        let with_guide = draw(&core, StageOverlays::default());
        assert!(with_guide.len() > plain.len(), "the line + handle draw");
        assert_eq!(
            draw(&core, StageOverlays { guides_stored: false, ..StageOverlays::default() }),
            plain,
        );
        // Rulers off ⇒ nothing; on ⇒ strictly more prims
        // (plates, ticks, labels), stored flag irrelevant.
        let rulers_on = draw(
            &core,
            StageOverlays { rulers: true, guides_stored: false, ..StageOverlays::default() },
        );
        assert!(rulers_on.len() > plain.len(), "the ruler bands draw");
        reset_view_nav();
    }
}
