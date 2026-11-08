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
//! `guides` flag.
//! UI mission Wave 5 adds two things to the stack: a permanent
//! stage frame (the stage rect filled with the project
//! background and outlined in the theme's border token, drawn
//! first — audit S10 D1) and the shell view-navigation layer
//! (a zoom/pan composed over the camera transform, folded in
//! by [`display_transform`] / [`drag_transform`] — audit
//! S10 D4; see the "View navigation" section below).

use std::cell::Cell;

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

/// Onion-skin segment sets around the current frame, mirroring
/// the GPUI stage's colours and falloff: previous frames keep
/// each figure's own colour (alpha `0x4D` for the nearest,
/// `0x26` for older), next frames share one green tint
/// (`0x7BE495`) with the same falloff. Returns `(segments,
/// colour, alpha)` entries in draw order.
///
/// The depths are per-side shell state (Phase G,
/// `State::onion_ui`), not the core's single `onion_frames`
/// count: `prev` frames behind and `next` frames ahead are
/// drawn, and a count of 0 draws nothing on that side. The
/// falloff is a function of the step distance only (step 1 is
/// always `0x4D`, steps ≥ 2 always `0x26`), so widening one
/// side never changes the other side's skins.
fn onion_skins(core: &EditorCore, prev: u8, next: u8) -> Vec<(Vec<(V2, V2)>, u32, f32)> {
    let mut out = Vec::new();
    if !core.onion {
        return out;
    }
    let project = &core.project;
    let segs_at = |frame: usize, fi: usize| -> Vec<(V2, V2)> {
        let pos = core.positions(frame, fi);
        let def = &project.figures[fi].def;
        def.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.parent.map(|p| (pos[p], pos[i])))
            .collect()
    };
    if prev > 0 && core.current > 0 {
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            out.push((segs_at(core.current - 1, fi), project.figures[fi].color, 0x4D as f32 / 255.0));
        }
        for back in 2..=(prev as usize) {
            if core.current < back {
                break;
            }
            for fi in 0..project.figures.len() {
                if project.figures[fi].hidden {
                    continue;
                }
                out.push((segs_at(core.current - back, fi), project.figures[fi].color, 0x26 as f32 / 255.0));
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
            out.push((segs_at(frame, fi), 0x7BE495, alpha));
        }
    }
    out
}

/// Convert one stage-space particle primitive (FX clip mote or
/// weather particle) into the shape being built, through the
/// rotated transform: discs become small polygons (Shape has no
/// circle primitive), lines keep a hairline-plus width.
fn particle_prim(
    shape: Shape,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
    prim: crate::rune_sec::stage_fx::ParticlePrim,
) -> Shape {
    use crate::rune_sec::stage_fx::ParticlePrim;
    match prim {
        ParticlePrim::Disc { pos, radius, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            shape.fill(ngon(rt.to_screen(pos), (radius * s).max(0.8), 10), c)
        }
        ParticlePrim::Line { a, b, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            shape.line(rt.to_screen(a), rt.to_screen(b), (1.5 * s).max(1.0), LineCap::Round, c)
        }
    }
}

/// Sub-segments a gradient segment is split into on the canvas.
/// The exporters blend continuously (per pixel / SVG gradient);
/// the canvas has no gradient stroke, so it approximates with
/// this many flat-coloured pieces — enough that the steps are
/// hard to see at stage zoom, cheap enough to rebuild per frame.
const GRADIENT_STEPS: usize = 6;

/// Colour at parameter `t` along a gradient segment, in the
/// exporters' convention (export.rs `StyledSeg`, svg.rs
/// `linearGradient`): `t = 0` at the parent joint is the node's
/// own colour, `t = 1` at the node's joint is its `color2`,
/// blended per channel with `style::lerp_color` — the exact
/// function both exporters call (export.rs:102, style.rs:25).
fn gradient_color(near: u32, far: u32, t: f32) -> u32 {
    stykanimate_rs::style::lerp_color(near, far, t)
}

/// The per-piece colours for a segment from hex `near` to the
/// node's `color2`, or `None` when the node has no `color2` —
/// the caller then keeps the single flat line, unchanged.
/// Each piece is coloured at its midpoint (`t = (k + 0.5) / n`),
/// the sample that minimises the worst error against the
/// exporters' continuous blend.
fn gradient_steps(near: u32, color2: Option<u32>) -> Option<Vec<u32>> {
    let far = color2?;
    Some(
        (0..GRADIENT_STEPS)
            .map(|k| gradient_color(near, far, (k as f32 + 0.5) / GRADIENT_STEPS as f32))
            .collect(),
    )
}

/// Point at parameter `t` along the stage-space segment `a`→`b`.
fn point_at(a: V2, b: V2, t: f32) -> V2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The line pieces one figure segment draws as, in stage
/// space: `(from, to, colour)` triples the caller strokes at
/// the node's thickness. This is the stage's copy of the
/// rasterizer's segment construction (export.rs
/// `render_frame_at`, the `StyledSeg` split):
///
/// * `bend == 0.0` — the pre-bend construction, unchanged:
///   one flat line, or [`GRADIENT_STEPS`] gradient pieces
///   along the straight segment when the node has a `color2`.
/// * `bend != 0.0` — the segment arcs through
///   `model::bend_mid(a, b, bend)` and draws as two pieces
///   meeting there, exactly as export splits its segment.
///   A gradient segment splits EACH piece again into
///   [`GRADIENT_STEPS`] sub-steps along that piece, coloured
///   over that piece's own colour range — export's first
///   piece runs `near` → the t = 0.5 mid colour, its second
///   the mid colour → `color2` — so the blend stays
///   continuous across the arc joint.
///
/// `near` is the hex the flat path starts from (node colour,
/// else the faded figure colour) and `flat` that same colour
/// as a `Color` (the caller's `node_color` result); the
/// gradient derives from the hex pair, as the exporters
/// derive theirs.
fn segment_pieces(
    a: V2,
    b: V2,
    bend: f32,
    near: u32,
    color2: Option<u32>,
    flat: Color,
) -> Vec<(V2, V2, Color)> {
    // One run of drawing along `from`→`to`: gradient
    // sub-steps at the usual midpoint samples when `steps`
    // carries them, else the run's single flat line.
    fn run(
        out: &mut Vec<(V2, V2, Color)>,
        from: V2,
        to: V2,
        steps: Option<Vec<u32>>,
        flat: Color,
    ) {
        match steps {
            Some(steps) => {
                let n = GRADIENT_STEPS as f32;
                for (k, hex) in steps.iter().enumerate() {
                    out.push((
                        point_at(from, to, k as f32 / n),
                        point_at(from, to, (k + 1) as f32 / n),
                        hex_color(*hex).unwrap_or(Color::WHITE),
                    ));
                }
            }
            None => out.push((from, to, flat)),
        }
    }
    let mut out = Vec::new();
    if bend == 0.0 {
        run(&mut out, a, b, gradient_steps(near, color2), flat);
    } else {
        let mid = model::bend_mid(a, b, bend);
        match color2 {
            Some(far) => {
                // Export's two-piece colour split: the first
                // piece's far colour is the whole segment's
                // t = 0.5 blend, and the second piece starts
                // from that same blend.
                let mid_hex = gradient_color(near, far, 0.5);
                run(&mut out, a, mid, gradient_steps(near, Some(mid_hex)), flat);
                run(&mut out, mid, b, gradient_steps(mid_hex, Some(far)), flat);
            }
            None => {
                run(&mut out, a, mid, None, flat);
                run(&mut out, mid, b, None, flat);
            }
        }
    }
    out
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
    /// Camera-frustum outline under `guides`.
    pub guides_frustum: bool,
    /// Action/title-safe frames under `guides`.
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
/// display frame — segments in per-node colour at real thickness
/// with round caps (a node with a `color2` gradient draws as
/// [`GRADIENT_STEPS`] blended sub-segments, mirroring the
/// exporters; a node with a nonzero bend at the display frame
/// arcs through `model::bend_mid` as two pieces, again
/// mirroring the exporters — see [`segment_pieces`]),
/// shape-node rings/polygons, head rings — plus
/// joint handles on the selected figure. Mirrors the GPUI stage's
/// `draw_data`/`paint_stage` decisions (opacity fades toward the
/// background; solo/hidden figures skipped via
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
        }
        if overlays.guides_safe {
            for prim in crate::rune_sec::stage_camera::safe_frame_geometry(project.stage) {
                let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
                shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
            }
        }
    }

    for (segs, color_hex, alpha) in onion_skins(core, overlays.onion_prev, overlays.onion_next) {
        let color = hex_color(color_hex).unwrap_or(Color::WHITE).alpha(alpha);
        for (a, b) in segs {
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 5.0 * s, LineCap::Round, color);
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
        let pos = core.positions_f(display, fi);
        // Per-node bends at the display frame (export.rs makes
        // the same per-figure fetch); empty when the project
        // carries no bend data, and every segment then takes
        // the straight path in `segment_pieces` unchanged.
        // Bends apply to this main figure pass only: the onion
        // skins above and the joint handles below stay
        // unbended, exactly as they were before bends reached
        // the stage (the rasterizer draws no ghosts at all, so
        // there is no exporter ghost behaviour to mirror).
        let bends = project.bends_at(display, fi);
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
                // Segment pieces from `segment_pieces`: flat,
                // gradient (GRADIENT_STEPS sub-segments blending
                // node colour → color2, parent joint → node
                // joint — the exporters' direction and lerp), and
                // arced through `model::bend_mid` when this node's
                // bend at the display frame is nonzero — the
                // indexing export uses (`bends` is per node; the
                // segment to node `i` takes node `i`'s bend). The
                // near hex is exactly the colour the flat path
                // draws (node colour, else the faded figure
                // colour), so the gradient starts where the flat
                // segment did; color2 is used raw, as export.rs
                // uses it.
                let bend = bends.get(i).copied().unwrap_or(0.0);
                let pieces = segment_pieces(
                    pos[p],
                    pos[i],
                    bend,
                    node.color.unwrap_or(faded),
                    node.style.color2,
                    node_color(node),
                );
                for (a, b, color) in pieces {
                    shape = shape.line(
                        rt.to_screen(a),
                        rt.to_screen(b),
                        node.thickness * s,
                        LineCap::Round,
                        color,
                    );
                }
            }
        }
        // Shape segments: circle rings and regular polygons
        // centred on the node (rasterizer + GPUI parity).
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
                        node_color(node),
                    );
                }
                model::Shape::Polygon(sides) => {
                    shape = shape.fill(ngon(centre, r, sides as usize), node_color(node));
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
    // rasterizer's pass order (export.rs render_frame_at).
    for prim in crate::rune_sec::stage_fx::fx_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim);
    }
    for prim in crate::rune_sec::stage_text::text_geometry(core) {
        let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = prim;
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
    }
    for prim in crate::rune_sec::stage_fx::weather_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim);
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
            shape = particle_prim(shape, &rt, s, prim);
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
            shape = particle_prim(shape, &rt, s, prim);
        }
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
    if core.playing {
        return false;
    }
    let Some((fi, ni)) = core.hit_test(p) else {
        return false;
    };
    if core.project.figures[fi].locked {
        core.status = "Figure is locked — unlock it in the layers list.".into();
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
        // by exactly the pan.
        assert_eq!(folded.to_screen([400.0, 300.0]), [413.0, 293.0]);
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
    fn gradient_color_endpoints_are_exact() {
        // t = 0 is the node colour, t = 1 is color2 — the
        // exporters' endpoints, reproduced exactly.
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 0.0), 0xFF0000);
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 1.0), 0x0000FF);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 0.0), 0x000000);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 1.0), 0xFFFFFF);
    }

    #[test]
    fn gradient_color_midpoint_matches_the_export_lerp() {
        // The same values style.rs pins for lerp_color itself:
        // 255 * 0.5 = 127.5 rounds to 128 per channel.
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 0.5), 0x800080);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 0.5), 0x808080);
    }

    #[test]
    fn gradient_steps_passthrough_without_color2() {
        // No color2 → None → the caller keeps the flat line.
        assert_eq!(gradient_steps(0x123456, None), None);
    }

    #[test]
    fn gradient_steps_samples_six_midpoints() {
        let steps = gradient_steps(0x000000, Some(0xFFFFFF)).expect("color2 set");
        assert_eq!(steps.len(), GRADIENT_STEPS);
        assert_eq!(steps.len(), 6);
        // Piece k is coloured at t = (k + 0.5) / 6 along
        // black → white (255 * t, rounded, per channel):
        // t = 1/12 → 21, t = 5/12 → 106, t = 7/12 → 149,
        // t = 11/12 → 234.
        assert_eq!(steps[0], 0x151515);
        assert_eq!(steps[2], 0x6A6A6A);
        assert_eq!(steps[3], 0x959595);
        assert_eq!(steps[5], 0xEAEAEA);
        // Strictly brightening piece by piece.
        for w in steps.windows(2) {
            assert!(w[0] < w[1], "gradient not monotonic: {steps:?}");
        }
    }

    #[test]
    fn point_at_endpoints_and_midpoint() {
        let (a, b) = ([0.0, 0.0], [10.0, 20.0]);
        assert_eq!(point_at(a, b, 0.0), a);
        assert_eq!(point_at(a, b, 1.0), b);
        assert_eq!(point_at(a, b, 0.5), [5.0, 10.0]);
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
        assert_eq!(core.status, "Figure is locked — unlock it in the layers list.");
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

    #[test]
    fn segment_pieces_straight_flat_is_one_line() {
        let pieces = segment_pieces([0.0, 0.0], [80.0, 40.0], 0.0, 0x123456, None, Color::WHITE);
        assert_eq!(pieces, vec![([0.0, 0.0], [80.0, 40.0], Color::WHITE)]);
    }

    #[test]
    fn segment_pieces_zero_bend_matches_the_straight_construction() {
        // The pre-bend drawing, built by hand: gradient pieces
        // coloured at the (k + 0.5) / 6 samples along the
        // straight segment, endpoints walking k/6 → (k+1)/6.
        // A zero bend must reproduce it exactly, so bend-less
        // and all-zero projects draw not one line differently.
        let (a, b) = ([10.0, 20.0], [110.0, 60.0]);
        let (near, far) = (0x102030u32, 0xE0D0C0u32);
        let n = GRADIENT_STEPS as f32;
        let expect: Vec<(V2, V2, Color)> = (0..GRADIENT_STEPS)
            .map(|k| {
                (
                    point_at(a, b, k as f32 / n),
                    point_at(a, b, (k + 1) as f32 / n),
                    hex_color(gradient_color(near, far, (k as f32 + 0.5) / n))
                        .unwrap_or(Color::WHITE),
                )
            })
            .collect();
        assert_eq!(segment_pieces(a, b, 0.0, near, Some(far), Color::WHITE), expect);
    }

    #[test]
    fn segment_pieces_bent_flat_splits_at_bend_mid() {
        let (a, b) = ([0.0, 0.0], [80.0, 40.0]);
        let bend = 0.6;
        let pieces = segment_pieces(a, b, bend, 0x123456, None, Color::WHITE);
        // Two lines where the straight segment drew one, both
        // in the flat colour, meeting exactly at bend_mid's
        // output (computed here by the model itself).
        assert_eq!(pieces.len(), 2);
        let mid = model::bend_mid(a, b, bend);
        assert_eq!(pieces[0], (a, mid, Color::WHITE));
        assert_eq!(pieces[1], (mid, b, Color::WHITE));
        // The joint really leaves the straight midpoint —
        // bend_mid's whole point for a nonzero bend.
        assert_ne!(mid, point_at(a, b, 0.5));
    }

    #[test]
    fn segment_pieces_bent_gradient_splits_each_half() {
        let (a, b) = ([0.0, 0.0], [90.0, 0.0]);
        let (near, far) = (0x000000u32, 0xFFFFFFu32);
        // Straight gradient: GRADIENT_STEPS pieces. Bent:
        // each half gets its own GRADIENT_STEPS sub-steps.
        let straight = segment_pieces(a, b, 0.0, near, Some(far), Color::WHITE);
        assert_eq!(straight.len(), GRADIENT_STEPS);
        let bent = segment_pieces(a, b, 0.5, near, Some(far), Color::WHITE);
        assert_eq!(bent.len(), 2 * GRADIENT_STEPS);
        // The halves meet exactly at the bend midpoint.
        let mid = model::bend_mid(a, b, 0.5);
        assert_eq!(bent[GRADIENT_STEPS - 1].1, mid);
        assert_eq!(bent[GRADIENT_STEPS].0, mid);
        // Export's colour split: the second half is coloured
        // over [mid blend, color2], so its first piece takes
        // that range's first midpoint sample.
        let mid_hex = gradient_color(near, far, 0.5);
        let expect = gradient_color(mid_hex, far, 0.5 / GRADIENT_STEPS as f32);
        assert_eq!(bent[GRADIENT_STEPS].2, hex_color(expect).unwrap_or(Color::WHITE));
        // And the first half over [near, mid blend]: its last
        // piece takes that range's last midpoint sample.
        let expect_first = gradient_color(
            near,
            mid_hex,
            (GRADIENT_STEPS as f32 - 0.5) / GRADIENT_STEPS as f32,
        );
        assert_eq!(
            bent[GRADIENT_STEPS - 1].2,
            hex_color(expect_first).unwrap_or(Color::WHITE)
        );
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
        let pieces = segment_pieces(
            pos[parent],
            pos[ni],
            bends.get(ni).copied().unwrap_or(0.0),
            0xE8E8EC,
            node.style.color2,
            Color::WHITE,
        );
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].1, model::bend_mid(pos[parent], pos[ni], 0.5));
        assert_eq!(pieces[1].0, model::bend_mid(pos[parent], pos[ni], 0.5));
    }
}
