//! Phase E, section 4/40 — STAGE CAMERA ROTATION + GUIDES
//! for the Rune editor shell. Declared from `main.rs` only
//! (bin-crate code, not part of the engine library).
//!
//! Two gaps this section closes *on paper* — the integrator
//! wires them into `stage_view` / `main.rs` (instructions
//! below); nothing here is mounted yet:
//!
//! 1. **Camera rotation.** The model has carried per-key
//!    camera rotation for a long time (`CameraKey.angle`,
//!    degrees, src/model.rs:546; interpolated by
//!    `Project::camera_angle_at(frame)`, model.rs:904), the
//!    camera panel already edits it (`rotate_camera` /
//!    ±15° buttons), and the **export rasterizer already
//!    renders it** (`export::render_frame_at`'s view closure
//!    applies `export::camera_rotate_offset`, export.rs:191).
//!    Only the editor stage ignores it: `stage_view` builds
//!    an unrotated [`ViewTransform`] and its module docs say
//!    so. [`RotTransform`] is the rotated mapping that fixes
//!    the stage without changing the unrotated one.
//! 2. **Guides.** A centre cross + rule-of-thirds grid +
//!    stage border in stage space ([`guides_geometry`]).
//!    Earlier sections deliberately shipped no guides toggle
//!    because no flag or geometry existed (see the notes in
//!    `settings_popover.rs` / `menus.rs`); this file is the
//!    geometry half of that follow-up.
//!
//! Phase F extends the guides family with two more
//! stage-space overlays in the same style, drawn by the
//! same guides pass (step 4 below — no new wiring, no new
//! flag): [`frustum_geometry`] outlines the region the
//! *current camera* frames, and [`safe_frame_geometry`]
//! draws the broadcast action-safe / title-safe rectangles
//! of the stage itself.
//!
//! ## Sign convention — proven parity with export, not taste
//!
//! `export::camera_rotate_offset` (export.rs:178-195)
//! documents the engine's wave-6 convention: `angle` is the
//! *camera* rotation, so a positive angle rotates the *scene
//! counter-clockwise* on screen — a point right of the camera
//! centre lands *above* it at +90°. In y-down screen
//! coordinates the rasterizer applies R(−θ) to the
//! stage-space offset from the camera centre, before the
//! pan/zoom mapping.
//!
//! [`RotTransform::to_screen`] rotates the *screen point*
//! about the element centre by −θ instead. The two are the
//! same map: the unrotated `to_screen` sends the camera
//! centre to the element centre, the remaining mapping is a
//! uniform scale `s > 0`, and rotation commutes with uniform
//! scaling —
//! `centre + R(−θ)·(s·(p − cam)) = centre + s·(R(−θ)·(p − cam))`.
//! The test `editor_matches_export_rasterizer_rotation` pins
//! this identity against the rasterizer's own function, and
//! `positive_angle_moves_a_right_point_up` pins the +90°
//! direction in plain terms. If either ever fails, the
//! editor and the exported video disagree — that is a bug
//! in this file, not a style choice.
//!
//! ## Integrator wiring (do all of it, or none)
//!
//! 1. `src/rune_sec/mod.rs`: add `pub mod stage_camera;`.
//! 2. `main.rs` — `State` gains the guides flag (shell
//!    view-state; the engine has none, by design):
//!    `pub guides: bool`, initialised `false` in `init()`.
//!    The settings section / View menu can then flip it —
//!    that closes the "guides toggle omitted" follow-up
//!    those sections recorded.
//! 3. `stage_view::stage_shape` — rotate the drawing. It
//!    already computes `display = core.display_frame()` and
//!    `cam = project.camera_at(display)`; keep both, then:
//!    ```ignore
//!    let rt = RotTransform::for_display(core, el_size);
//!    let vt = rt.vt; // scale etc. unchanged: let s = vt.scale;
//!    ```
//!    and replace every `to_screen(&vt, p)` call inside
//!    `stage_shape` (and inside `onion_skins`' caller path —
//!    the skins are mapped in `stage_shape` too) with
//!    `rt.to_screen(p)`. `vt.scale` is untouched by rotation,
//!    so thicknesses, handle radii and head rings need no
//!    changes.
//! 4. `stage_view::stage_shape` — guides FIRST in the
//!    `Shape`, before the onion skins, so they sit under
//!    everything:
//!    ```ignore
//!    if guides {
//!        let ink = hex_color(0x8A93A6).map(|c| c.alpha(0.55));
//!        if let Some(ink) = ink {
//!            for prim in guides_geometry(project.stage) {
//!                let GuidePrim::Line { a, b } = prim;
//!                shape = shape.line(rt.to_screen(a), rt.to_screen(b),
//!                                   1.0, LineCap::Round, ink);
//!            }
//!        }
//!    }
//!    ```
//!    `stage_shape` needs the flag: change its signature to
//!    `stage_shape(core, el_size, guides: bool)` and pass
//!    `s.guides` at the `main.rs` call site (or add a
//!    `stage_shape_guides` wrapper — one of the two, not
//!    both). The 1.0 width is in *element points*, so guides
//!    stay hairlines at any zoom — deliberate.
//! 5. ⚠️ **`main.rs::stage_drag` MUST rotate too — this is
//!    the loud warning.** Today (main.rs:129-130) it builds
//!    `view_transform([d.w, d.h], …, core.camera_here())`
//!    and converts with the unrotated `to_stage`. If the
//!    stage *draws* rotated but drags convert unrotated,
//!    posing silently breaks under any non-zero angle: the
//!    grabbed joint sits a full rotation away from the
//!    pointer (at 90°, a horizontal drag moves the joint
//!    vertically; hit-tests land on the wrong joint or
//!    miss). Replace both lines with:
//!    ```ignore
//!    let rt = RotTransform::for_drag(&s.core, [d.w, d.h]);
//!    let p = rt.to_stage([d.x, d.y]);
//!    ```
//!    `for_drag` mirrors the drag path's existing frame
//!    source (`camera_here()` = the *current* frame) while
//!    `for_display` uses `display_frame()` — the same
//!    asymmetry pan/zoom already has (identical whenever
//!    playback is stopped, which is when posing happens).
//!    Drawing rotated + dragging unrotated is the one
//!    combination that must never ship; unrotated +
//!    unrotated (today) and rotated + rotated are both
//!    coherent.
//! 6. Optional: a `--scene` named e.g. `rotated` in
//!    `scenes.rs` that sets a camera key with `angle = 30`
//!    gives `--snap` a rotation screenshot for free.
//!
//! ## What rotation does NOT change
//!
//! Stroke widths (uniform scale only), the `view_box`-free
//! coordinate design in `stage_view`'s docs, onion falloff,
//! hit-test *radii* in the core (they live in stage space,
//! and `to_stage` now returns true stage points again), and
//! anything in the engine. At `angle == 0.0` every function
//! here reduces *exactly* to the unrotated transform — the
//! rotation branch is skipped, not evaluated — which a test
//! pins with `assert_eq!`, so enabling this file cannot
//! perturb the existing unrotated stage by one bit.

use stykanimate_rs::editor::EditorCore;

use crate::stage_view::{self, ViewTransform};

/// The stage camera mapping with rotation: an unrotated
/// [`ViewTransform`] plus the camera angle (degrees, the
/// model's `CameraKey.angle` unit) in effect for the frame
/// being drawn or dragged.
///
/// `to_screen` / `to_stage` are exact inverses (roundtrip
/// drift < 1e-3 stage units, the tolerance `stage_view`'s
/// own roundtrip test uses), and at `angle_deg == 0.0` they
/// are *identical* to `stage_view::to_screen` /
/// `stage_view::to_stage` — see the module docs for the
/// sign convention and its proven parity with the export
/// rasterizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotTransform {
    /// The unrotated pan/zoom mapping (element size, camera
    /// centre, fit·zoom scale).
    pub vt: ViewTransform,
    /// Camera rotation in degrees (positive = camera turns
    /// clockwise = scene appears counter-clockwise).
    pub angle_deg: f32,
}

/// Rotate `p` about `centre` by `deg` degrees using the
/// standard rotation matrix in y-down screen coordinates
/// (`[dx·cos − dy·sin, dx·sin + dy·cos]`); positive `deg`
/// turns a point clockwise *as seen on screen*. Callers pass
/// negated angles where the engine's camera convention
/// requires it — see the module docs.
fn rotate_about(p: [f32; 2], centre: [f32; 2], deg: f32) -> [f32; 2] {
    let (s, c) = deg.to_radians().sin_cos();
    let dx = p[0] - centre[0];
    let dy = p[1] - centre[1];
    [centre[0] + dx * c - dy * s, centre[1] + dx * s + dy * c]
}

impl RotTransform {
    /// Wrap an existing transform with a camera angle.
    pub fn new(vt: ViewTransform, angle_deg: f32) -> Self {
        RotTransform { vt, angle_deg }
    }

    /// One-call builder matching `stage_view::view_transform`
    /// plus the angle: element size, stage size, camera
    /// `(x, y, zoom)`, angle in degrees.
    pub fn from_parts(
        el_size: [f32; 2],
        stage: [f32; 2],
        cam: (f32, f32, f32),
        angle_deg: f32,
    ) -> Self {
        RotTransform::new(stage_view::view_transform(el_size, stage, cam), angle_deg)
    }

    /// The transform the *drawing* uses: pan/zoom and angle
    /// both sampled at `core.display_frame()` — the exact
    /// frame source `stage_view::stage_shape` already uses
    /// for `camera_at`, now paired with `camera_angle_at`
    /// at the same frame.
    pub fn for_display(core: &EditorCore, el_size: [f32; 2]) -> Self {
        let display = core.display_frame();
        let cam = core.project.camera_at(display);
        let angle = core.project.camera_angle_at(display);
        RotTransform::from_parts(el_size, core.project.stage, cam, angle)
    }

    /// The transform *drag conversion* uses: pan/zoom from
    /// `core.camera_here()` (the current frame — what
    /// `main.rs::stage_drag` already builds from), angle
    /// from `camera_angle_at(current)`. Must be used
    /// together with [`RotTransform::for_display`] — see
    /// the module docs' loud warning.
    pub fn for_drag(core: &EditorCore, el_size: [f32; 2]) -> Self {
        let cam = core.camera_here();
        let angle = core.project.camera_angle_at(core.current as f32);
        RotTransform::from_parts(el_size, core.project.stage, cam, angle)
    }

    /// The rotation centre in element points: the element
    /// centre, which is where the unrotated transform puts
    /// the camera centre.
    pub fn centre(&self) -> [f32; 2] {
        [self.vt.size[0] / 2.0, self.vt.size[1] / 2.0]
    }

    /// Stage point → canvas element point, rotated: the
    /// unrotated screen point turned about the view centre
    /// by **−angle** (the camera convention — a positive
    /// camera angle turns the scene counter-clockwise).
    /// At angle 0 this is `stage_view::to_screen`, verbatim.
    pub fn to_screen(&self, p: [f32; 2]) -> [f32; 2] {
        let base = stage_view::to_screen(&self.vt, p);
        if self.angle_deg == 0.0 {
            return base;
        }
        rotate_about(base, self.centre(), -self.angle_deg)
    }

    /// Canvas element point → stage point (exact inverse of
    /// [`RotTransform::to_screen`]): the local point turned
    /// about the view centre by **+angle**, then mapped back
    /// through the unrotated transform. At angle 0 this is
    /// `stage_view::to_stage`, verbatim.
    pub fn to_stage(&self, local: [f32; 2]) -> [f32; 2] {
        if self.angle_deg == 0.0 {
            return stage_view::to_stage(&self.vt, local);
        }
        let unrotated = rotate_about(local, self.centre(), self.angle_deg);
        stage_view::to_stage(&self.vt, unrotated)
    }
}

/// Map a stage-space polyline through the rotated transform
/// — the shape of every segment, ring and guide line the
/// stage draws. Equivalent to mapping each point with
/// [`RotTransform::to_screen`]; exists so call sites read
/// as one mapping, not a loop of them.
pub fn rotated_points(rt: &RotTransform, pts: &[[f32; 2]]) -> Vec<[f32; 2]> {
    pts.iter().map(|&p| rt.to_screen(p)).collect()
}

/// One guides primitive, in stage coordinates. Only lines
/// exist today (cross, thirds, border, camera frustum and
/// safe-area rectangles are all straight); the enum leaves
/// room for a future circle primitive without changing any
/// geometry function's signature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuidePrim {
    /// A straight guide line from `a` to `b` (stage space).
    Line { a: [f32; 2], b: [f32; 2] },
}

/// The full guides overlay for a stage of `stage` units, in
/// stage coordinates, in draw order:
///
/// 1. **border** — the stage rectangle (4 lines), so the
///    frame edge is visible against the letterbox;
/// 2. **rule-of-thirds** — verticals at w/3 and 2w/3,
///    horizontals at h/3 and 2h/3 (4 lines);
/// 3. **centre cross** — the vertical and horizontal
///    centrelines (2 lines).
///
/// 10 primitives for any non-degenerate stage. A stage with
/// a non-positive axis yields no guides at all (there is no
/// frame to annotate) rather than lines at nonsense
/// positions. The integrator maps each line through
/// `RotTransform::to_screen` (guides rotate with the
/// camera — they describe the *stage*, not the window) and
/// draws them first in the `Shape`; see the module docs.
pub fn guides_geometry(stage: [f32; 2]) -> Vec<GuidePrim> {
    let (w, h) = (stage[0], stage[1]);
    if !(w > 0.0 && h > 0.0) {
        return Vec::new();
    }
    let line = |a: [f32; 2], b: [f32; 2]| GuidePrim::Line { a, b };
    vec![
        // Border.
        line([0.0, 0.0], [w, 0.0]),
        line([w, 0.0], [w, h]),
        line([w, h], [0.0, h]),
        line([0.0, h], [0.0, 0.0]),
        // Rule of thirds.
        line([w / 3.0, 0.0], [w / 3.0, h]),
        line([2.0 * w / 3.0, 0.0], [2.0 * w / 3.0, h]),
        line([0.0, h / 3.0], [w, h / 3.0]),
        line([0.0, 2.0 * h / 3.0], [w, 2.0 * h / 3.0]),
        // Centre cross.
        line([w / 2.0, 0.0], [w / 2.0, h]),
        line([0.0, h / 2.0], [w, h / 2.0]),
    ]
}

/// The four sides of an axis-aligned rectangle as guide
/// lines, in the same winding [`guides_geometry`]'s border
/// uses (top-left → top-right → bottom-right → bottom-left).
fn rect_lines(x0: f32, y0: f32, x1: f32, y1: f32) -> [GuidePrim; 4] {
    [
        GuidePrim::Line { a: [x0, y0], b: [x1, y0] },
        GuidePrim::Line { a: [x1, y0], b: [x1, y1] },
        GuidePrim::Line { a: [x1, y1], b: [x0, y1] },
        GuidePrim::Line { a: [x0, y1], b: [x0, y0] },
    ]
}

/// The camera frustum as a stage-space outline: the
/// rectangle (4 lines, closed loop) bounding exactly the
/// region the camera in effect at the display frame frames
/// — in other words, the footprint of an exported frame on
/// the stage. Pan/zoom (follow included) come from
/// `camera_at` and the angle from `camera_angle_at`, both
/// sampled at `core.display_frame()` via
/// [`RotTransform::for_display`] — the same camera the
/// stage drawing uses, so the outline can never disagree
/// with the view it annotates.
///
/// ## How the rectangle is computed
///
/// The transform is built at the stage's own size
/// (`el_size = project.stage`, so `fit = 1` and
/// `scale = zoom`), and the four element corners are
/// inverse-mapped with [`RotTransform::to_stage`]. Building
/// at the stage's size makes the frustum a property of the
/// *camera*, not of the window: a real window with a
/// different aspect ratio letterboxes, showing more of the
/// stage on one axis, but the camera's own frame — what an
/// export at the stage's aspect contains — is this
/// rectangle. At the default camera (stage centre, zoom 1,
/// angle 0) it *is* the stage rectangle and coincides with
/// [`guides_geometry`]'s border; zooming in shrinks it
/// toward the camera centre, panning moves it (it may
/// extend past the stage edges — the camera can frame
/// off-stage space, and the outline says so), and a rotated
/// camera rotates it into a true rotated rectangle, still
/// emitted as four straight lines in element-corner order
/// (top-left first, the border's winding).
///
/// A stage with a non-positive axis yields no frustum, the
/// same degenerate rule as [`guides_geometry`]. The
/// integrator draws the lines through `rt.to_screen` with
/// the rest of the guides pass; see the module docs.
pub fn frustum_geometry(core: &EditorCore) -> Vec<GuidePrim> {
    let stage = core.project.stage;
    let (w, h) = (stage[0], stage[1]);
    if !(w > 0.0 && h > 0.0) {
        return Vec::new();
    }
    let rt = RotTransform::for_display(core, stage);
    let corners = [
        rt.to_stage([0.0, 0.0]),
        rt.to_stage([w, 0.0]),
        rt.to_stage([w, h]),
        rt.to_stage([0.0, h]),
    ];
    vec![
        GuidePrim::Line { a: corners[0], b: corners[1] },
        GuidePrim::Line { a: corners[1], b: corners[2] },
        GuidePrim::Line { a: corners[2], b: corners[3] },
        GuidePrim::Line { a: corners[3], b: corners[0] },
    ]
}

/// The broadcast safe areas of the stage itself, in stage
/// coordinates — camera-independent (unlike
/// [`frustum_geometry`], nothing here moves when the camera
/// does):
///
/// 1. **action-safe** — the stage inset by 5% of each axis
///    on every side (`w/20`, `h/20`); action inside it
///    survives overscan/cropping (4 lines);
/// 2. **title-safe** — the stage inset by 10% of each axis
///    on every side (`w/10`, `h/10`); text and titles
///    belong inside it (4 lines).
///
/// The insets are per-axis percentages — 5% of the *width*
/// horizontally, 5% of the *height* vertically — the
/// broadcast convention, so on a non-square stage the two
/// margins differ in absolute units. 8 primitives for any
/// non-degenerate stage, action-safe first (the larger
/// rect) then title-safe; a stage with a non-positive axis
/// yields none, matching [`guides_geometry`]. Drawn through
/// `rt.to_screen` with the guides pass, like every overlay
/// in this file.
pub fn safe_frame_geometry(stage: [f32; 2]) -> Vec<GuidePrim> {
    let (w, h) = (stage[0], stage[1]);
    if !(w > 0.0 && h > 0.0) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(8);
    // Action-safe: 5% of each axis per side.
    let (ax, ay) = (w / 20.0, h / 20.0);
    out.extend(rect_lines(ax, ay, w - ax, h - ay));
    // Title-safe: 10% of each axis per side.
    let (tx, ty) = (w / 10.0, h / 10.0);
    out.extend(rect_lines(tx, ty, w - tx, h - ty));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The demo-shaped setup stage_view's own tests use:
    /// 960×720 stage, and an element exactly stage-sized so
    /// fit = 1 and scale = zoom.
    fn vt(cam: (f32, f32, f32)) -> ViewTransform {
        stage_view::view_transform([960.0, 720.0], [960.0, 720.0], cam)
    }

    fn close(a: [f32; 2], b: [f32; 2], tol: f32) -> bool {
        (a[0] - b[0]).abs() < tol && (a[1] - b[1]).abs() < tol
    }

    #[test]
    fn roundtrip_is_exact_at_all_angles() {
        let points = [
            [0.0, 0.0],
            [960.0, 720.0],
            [480.0, 360.0],
            [123.4, 567.8],
            [-50.0, 1000.0],
        ];
        for (el_size, cam) in [
            ([960.0, 720.0], (480.0, 360.0, 1.0)),
            ([1440.0, 900.0], (123.0, 456.0, 2.5)),
            ([333.0, 777.0], (700.0, 100.0, 0.3)),
        ] {
            let base = stage_view::view_transform(el_size, [960.0, 720.0], cam);
            for angle in [0.0, 15.0, 90.0, -30.0] {
                let rt = RotTransform::new(base, angle);
                for p in points {
                    let back = rt.to_stage(rt.to_screen(p));
                    assert!(
                        close(back, p, 1e-3),
                        "stage roundtrip drift at {angle}° for {p:?}: {back:?}"
                    );
                    let fwd = rt.to_screen(rt.to_stage(p));
                    assert!(
                        close(fwd, p, 1e-3),
                        "screen roundtrip drift at {angle}° for {p:?}: {fwd:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn angle_zero_is_the_unrotated_transform_exactly() {
        let base = vt((123.0, 456.0, 2.0));
        let rt = RotTransform::new(base, 0.0);
        for p in [[0.0, 0.0], [960.0, 720.0], [123.4, 567.8]] {
            assert_eq!(rt.to_screen(p), stage_view::to_screen(&base, p));
            assert_eq!(rt.to_stage(p), stage_view::to_stage(&base, p));
        }
        // The core-built transforms agree too: a fresh core
        // has no camera keys, so its angle is 0.
        let core = EditorCore::new();
        let rt = RotTransform::for_display(&core, [960.0, 720.0]);
        assert_eq!(rt.angle_deg, 0.0);
        assert_eq!(rt.to_screen([100.0, 100.0]), stage_view::to_screen(&rt.vt, [100.0, 100.0]));
        let rt = RotTransform::for_drag(&core, [960.0, 720.0]);
        assert_eq!(rt.angle_deg, 0.0);
    }

    #[test]
    fn camera_centre_is_the_rotation_centre() {
        let base = vt((480.0, 360.0, 2.0));
        let centre = [480.0, 360.0]; // element centre
        for angle in [15.0, 90.0, -30.0, 180.0] {
            let rt = RotTransform::new(base, angle);
            // The camera centre draws at the element centre
            // whatever the angle…
            assert!(close(rt.to_screen(base.cam), centre, 1e-3), "angle {angle}");
            // …and the element centre converts back to it.
            assert!(close(rt.to_stage(centre), base.cam, 1e-3), "angle {angle}");
        }
    }

    #[test]
    fn positive_angle_moves_a_right_point_up() {
        // The engine's documented convention (export.rs):
        // at +90° a point right of the camera centre lands
        // above it. Scale-1 transform, cam at stage centre:
        // (580, 360) is 100 units right of centre on screen
        // before rotation; after, it must be 100 above.
        let rt = RotTransform::new(vt((480.0, 360.0, 1.0)), 90.0);
        let got = rt.to_screen([580.0, 360.0]);
        assert!(close(got, [480.0, 260.0], 1e-3), "got {got:?}");
        // And to_stage brings it straight back.
        assert!(close(rt.to_stage(got), [580.0, 360.0], 1e-3));
    }

    #[test]
    fn editor_matches_export_rasterizer_rotation() {
        // Parity with the shipped renderer: at fit = zoom = 1
        // the element centre is the stage centre, so the
        // screen offset from centre must equal
        // export::camera_rotate_offset applied to the stage
        // offset from the camera centre — for every angle.
        let base = vt((480.0, 360.0, 1.0));
        let centre = [480.0, 360.0];
        for angle in [15.0, 90.0, -30.0, 45.0] {
            let rt = RotTransform::new(base, angle);
            for p in [[580.0, 360.0], [480.0, 460.0], [100.0, 100.0], [959.0, 719.0]] {
                let off = stykanimate_rs::export::camera_rotate_offset(
                    p[0] - base.cam[0],
                    p[1] - base.cam[1],
                    angle,
                );
                let want = [centre[0] + off[0], centre[1] + off[1]];
                let got = rt.to_screen(p);
                assert!(
                    close(got, want, 1e-3),
                    "editor/export rotation disagree at {angle}° for {p:?}: {got:?} vs {want:?}"
                );
            }
        }
    }

    #[test]
    fn rotated_points_maps_each_point() {
        let base = vt((480.0, 360.0, 1.0));
        let pts = [[580.0, 360.0], [480.0, 360.0], [380.0, 360.0]];
        // Angle 0: identical to the unrotated mapping.
        let rt0 = RotTransform::new(base, 0.0);
        let out = rotated_points(&rt0, &pts);
        assert_eq!(out.len(), pts.len());
        for (got, &p) in out.iter().zip(&pts) {
            assert_eq!(*got, stage_view::to_screen(&base, p));
        }
        // Angle 90: the horizontal trio becomes vertical —
        // right point up, left point down, centre fixed.
        let rt90 = RotTransform::new(base, 90.0);
        let out = rotated_points(&rt90, &pts);
        assert!(close(out[0], [480.0, 260.0], 1e-3), "{out:?}");
        assert!(close(out[1], [480.0, 360.0], 1e-3), "{out:?}");
        assert!(close(out[2], [480.0, 460.0], 1e-3), "{out:?}");
        // Empty in, empty out.
        assert!(rotated_points(&rt90, &[]).is_empty());
    }

    fn has_line(prims: &[GuidePrim], a: [f32; 2], b: [f32; 2]) -> bool {
        prims.iter().any(|p| *p == GuidePrim::Line { a, b })
    }

    #[test]
    fn guides_for_960x720_stage() {
        let prims = guides_geometry([960.0, 720.0]);
        // 4 border + 4 thirds + 2 centre = 10.
        assert_eq!(prims.len(), 10);
        // Border.
        assert!(has_line(&prims, [0.0, 0.0], [960.0, 0.0]));
        assert!(has_line(&prims, [960.0, 0.0], [960.0, 720.0]));
        assert!(has_line(&prims, [960.0, 720.0], [0.0, 720.0]));
        assert!(has_line(&prims, [0.0, 720.0], [0.0, 0.0]));
        // Thirds: verticals at 320/640, horizontals at 240/480.
        assert!(has_line(&prims, [320.0, 0.0], [320.0, 720.0]));
        assert!(has_line(&prims, [640.0, 0.0], [640.0, 720.0]));
        assert!(has_line(&prims, [0.0, 240.0], [960.0, 240.0]));
        assert!(has_line(&prims, [0.0, 480.0], [960.0, 480.0]));
        // Centre cross.
        assert!(has_line(&prims, [480.0, 0.0], [480.0, 720.0]));
        assert!(has_line(&prims, [0.0, 360.0], [960.0, 360.0]));
        // Draw order: border first, centre cross last.
        assert_eq!(prims[0], GuidePrim::Line { a: [0.0, 0.0], b: [960.0, 0.0] });
        assert_eq!(
            prims[9],
            GuidePrim::Line { a: [0.0, 360.0], b: [960.0, 360.0] }
        );
    }

    #[test]
    fn guides_scale_with_the_stage_and_reject_degenerate() {
        let prims = guides_geometry([300.0, 200.0]);
        assert_eq!(prims.len(), 10);
        assert!(has_line(&prims, [100.0, 0.0], [100.0, 200.0]));
        assert!(has_line(&prims, [150.0, 0.0], [150.0, 200.0]));
        assert!(guides_geometry([0.0, 720.0]).is_empty());
        assert!(guides_geometry([960.0, 0.0]).is_empty());
        assert!(guides_geometry([-10.0, -10.0]).is_empty());
    }

    /// A fresh core whose only camera key sits at frame 0
    /// (the fresh core's display frame) centred on the demo
    /// stage, with the given zoom and angle.
    fn keyed_core(zoom: f32, angle: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.camera.push(stykanimate_rs::model::CameraKey {
            frame: 0,
            x: 480.0,
            y: 360.0,
            zoom,
            angle,
        });
        core
    }

    #[test]
    fn frustum_at_rest_is_the_stage_rect() {
        // A fresh core's camera is the default (stage
        // centre, zoom 1, angle 0), so the frustum is the
        // stage rectangle itself — the same four lines as
        // guides_geometry's border, in the same order.
        let core = EditorCore::new();
        let prims = frustum_geometry(&core);
        assert_eq!(
            prims,
            vec![
                GuidePrim::Line { a: [0.0, 0.0], b: [960.0, 0.0] },
                GuidePrim::Line { a: [960.0, 0.0], b: [960.0, 720.0] },
                GuidePrim::Line { a: [960.0, 720.0], b: [0.0, 720.0] },
                GuidePrim::Line { a: [0.0, 720.0], b: [0.0, 0.0] },
            ]
        );
    }

    #[test]
    fn frustum_zoom_two_halves_the_span() {
        // Zoom 2 about the stage centre: the framed span
        // halves on both axes — 480×360 centred on
        // (480, 360), i.e. corners (240, 180)…(720, 540).
        // All values exact in f32 (angle 0, scale 2).
        let core = keyed_core(2.0, 0.0);
        let prims = frustum_geometry(&core);
        assert_eq!(
            prims,
            vec![
                GuidePrim::Line { a: [240.0, 180.0], b: [720.0, 180.0] },
                GuidePrim::Line { a: [720.0, 180.0], b: [720.0, 540.0] },
                GuidePrim::Line { a: [720.0, 540.0], b: [240.0, 540.0] },
                GuidePrim::Line { a: [240.0, 540.0], b: [240.0, 180.0] },
            ]
        );
    }

    #[test]
    fn frustum_rotates_with_the_camera() {
        // Angle +90°, zoom 1, camera at the stage centre.
        // Hand-computed via to_stage: each element corner
        // first turns +90° about the element centre
        // (480, 360) — (dx, dy) → (−dy, dx) — then unmaps
        // at scale 1 about the same centre:
        //   TL (0, 0)     → (840, −120)
        //   TR (960, 0)   → (840, 840)
        //   BR (960, 720) → (120, 840)
        //   BL (0, 720)   → (120, −120)
        // The span swaps axes: 720 wide × 960 tall. f32
        // sin/cos at 90° is not exact, hence the 1e-3
        // tolerance every rotation test here uses.
        let core = keyed_core(1.0, 90.0);
        let prims = frustum_geometry(&core);
        let want = [
            ([840.0, -120.0], [840.0, 840.0]),
            ([840.0, 840.0], [120.0, 840.0]),
            ([120.0, 840.0], [120.0, -120.0]),
            ([120.0, -120.0], [840.0, -120.0]),
        ];
        assert_eq!(prims.len(), want.len());
        for (prim, (a, b)) in prims.iter().zip(want) {
            let GuidePrim::Line { a: ga, b: gb } = *prim;
            assert!(
                close(ga, a, 1e-3) && close(gb, b, 1e-3),
                "frustum side {ga:?}→{gb:?}, want {a:?}→{b:?}"
            );
        }
    }

    #[test]
    fn frustum_rejects_a_degenerate_stage() {
        let mut core = EditorCore::new();
        core.project.stage = [0.0, 720.0];
        assert!(frustum_geometry(&core).is_empty());
        core.project.stage = [960.0, -5.0];
        assert!(frustum_geometry(&core).is_empty());
    }

    #[test]
    fn safe_frames_are_exact_insets() {
        // 960×720: action-safe = 5% per axis → inset
        // (48, 36), rect (48, 36)…(912, 684); title-safe
        // = 10% → inset (96, 72), rect (96, 72)…(864, 648).
        // w/20, h/20, w/10, h/10 are all exact in f32 at
        // this size, so the lines compare exactly.
        let prims = safe_frame_geometry([960.0, 720.0]);
        assert_eq!(
            prims,
            vec![
                // Action-safe first.
                GuidePrim::Line { a: [48.0, 36.0], b: [912.0, 36.0] },
                GuidePrim::Line { a: [912.0, 36.0], b: [912.0, 684.0] },
                GuidePrim::Line { a: [912.0, 684.0], b: [48.0, 684.0] },
                GuidePrim::Line { a: [48.0, 684.0], b: [48.0, 36.0] },
                // Then title-safe.
                GuidePrim::Line { a: [96.0, 72.0], b: [864.0, 72.0] },
                GuidePrim::Line { a: [864.0, 72.0], b: [864.0, 648.0] },
                GuidePrim::Line { a: [864.0, 648.0], b: [96.0, 648.0] },
                GuidePrim::Line { a: [96.0, 648.0], b: [96.0, 72.0] },
            ]
        );
        // A second size pins the per-axis percentages (not
        // absolute margins): 1000×500 → action inset
        // (50, 25), title inset (100, 50).
        let prims = safe_frame_geometry([1000.0, 500.0]);
        assert_eq!(prims.len(), 8);
        assert!(has_line(&prims, [50.0, 25.0], [950.0, 25.0]));
        assert!(has_line(&prims, [950.0, 475.0], [50.0, 475.0]));
        assert!(has_line(&prims, [100.0, 50.0], [900.0, 50.0]));
        assert!(has_line(&prims, [900.0, 450.0], [100.0, 450.0]));
        // Degenerate stages get no safe frames.
        assert!(safe_frame_geometry([0.0, 720.0]).is_empty());
        assert!(safe_frame_geometry([960.0, 0.0]).is_empty());
        assert!(safe_frame_geometry([-10.0, -10.0]).is_empty());
    }
}
