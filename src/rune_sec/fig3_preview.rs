//! Phase L, section 1 — 3D PREVIEW GEOMETRY: pure viewport-space
//! segments for the 3D panel's figure preview, so the panel can
//! draw the fig3d session figure (and a ground grid) without any
//! of the projection maths living in the view code.
//!
//! ## Source of truth — the engine (`src/fig3d.rs`)
//!
//! Nothing here re-implements 3D maths. The figure's world points
//! come from `fig3d::evaluate3` (fig3d.rs:383) and the projection
//! from `fig3d::project` (fig3d.rs:995) — the same pair the bake
//! paths use (editor.rs fig3_bake / fig3_bake_animation). The
//! grid lines come from `fig3d::grid_line_points` and the
//! no-session camera from `fig3d::preview_camera` (both Phase L
//! section 3's additions to fig3d.rs). This file only decides
//! WHICH segments exist, in which colour and width, in viewport
//! coordinates; a consumer draws the returned prims in order.
//!
//! Conventions carried from fig3d:
//! - World is Y-up, the figure faces +Z. The engine defines
//!   no ground plane — `project` fixes only the Y-up
//!   convention — so the grid (drawn by `grid_line_points`
//!   on the y = 0 plane) is OFFSET here to the session
//!   figure's lowest evaluated joint: a session starts with
//!   its hips at the origin and its feet below it, and a grid
//!   left on y = 0 would slice through the hips instead of
//!   lying under the feet. With no session there is no
//!   figure to follow, and the grid stays on the origin
//!   plane. The ground shadow follows the same offset.
//! - `project` flips Y (world Y-up → stage Y-down) and clamps
//!   points at/behind the near plane to the stage centre with
//!   `depth = fig3d::BEHIND_DEPTH` (1e6). Any segment touching a
//!   clamped endpoint is SKIPPED here rather than drawn to the
//!   centre — a clamped xy is not a real position.
//! - `Proj.scale` is pixels-per-world-unit at the point's depth;
//!   the ground shadow's radius is measured through it.
//!
//! ## Prim order (painter's order)
//!
//! Grid first, then the ground shadow, then bones, then joint
//! crosses — later prims draw over earlier ones.
//!
//! ## Honest notes
//!
//! - Segments only, by design: joints are drawn as two 3 px
//!   crossing segments and the shadow as an 8-segment ring, so a
//!   consumer needs nothing but line drawing.
//! - The shadow ring is squashed vertically (radius × 0.35) so
//!   it reads as an ellipse lying on the ground plane rather
//!   than a circle floating on the screen.
//! - Joint crosses are 1.5 px wide — the brief fixed the bone
//!   (2.5), grid (1.0) and shadow (3.0) widths only; 1.5 keeps
//!   the crosses subordinate to the bones they sit on.

use std::cell::Cell;

use stykanimate_rs::fig3d::{self, Fig3State};

/// One segment in VIEWPORT coordinates (the space `fig3d::project`
/// returns for the stage size it was given), ready to stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewPrim {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// Packed `0xRRGGBB`, the packing the rest of the shell uses.
    pub color: u32,
    pub width: f32,
}

/// The 3D panel preview's nominal width in points; the actual
/// size is passed per call, this is the size the calibration
/// test (and the panel's layout) is written against.
pub const PREVIEW_W: f32 = 308.0;
/// The 3D panel preview's nominal height in points.
pub const PREVIEW_H: f32 = 220.0;

/// Ground-grid colour: a muted blue-grey, visible on the dark
/// stage without competing with the figure.
pub const GRID_COLOR: u32 = 0x3A4356;
/// Grid stroke width.
pub const GRID_WIDTH: f32 = 1.0;
/// Bone stroke width.
pub const BONE_WIDTH: f32 = 2.5;
/// Joint-cross stroke width (see the module docs).
pub const JOINT_WIDTH: f32 = 1.5;
/// Half-length of a joint cross's arms, in pixels.
pub const JOINT_HALF: f32 = 3.0;
/// Selection accent — the shell's primary accent colour.
pub const SELECTED_COLOR: u32 = 0x6D8CFF;
/// Ground-shadow colour and width.
pub const SHADOW_COLOR: u32 = 0x000000;
/// Ground-shadow stroke width.
pub const SHADOW_WIDTH: f32 = 3.0;
/// Shadow ring radius in WORLD units, scaled by the projected
/// point's `Proj.scale` (pixels per world unit).
pub const SHADOW_RADIUS: f32 = 30.0;
/// Number of segments in the shadow ring.
pub const SHADOW_SEGMENTS: usize = 8;
/// Vertical squash of the shadow ring (see the module docs).
pub const SHADOW_SQUASH: f32 = 0.35;
/// Grid extent/step handed to `fig3d::grid_line_points`: lines
/// every 50 world units out to ±5 steps (±250 units).
pub const GRID_EXTENT: i32 = 5;
/// Grid spacing in world units.
pub const GRID_STEP: f32 = 50.0;

thread_local! {
    /// Whether the ground grid shows in the 3D preview. Shell
    /// view state only (like the clips panel's selection): it is
    /// deliberately NOT part of `Fig3State`, which is engine
    /// session state. Defaults to visible.
    static SHOW_GRID: Cell<bool> = const { Cell::new(true) };
}

/// Whether the ground grid currently shows in the 3D preview.
pub fn grid_visible() -> bool {
    SHOW_GRID.with(|g| g.get())
}

/// Flip the ground grid's visibility; returns the new value.
pub fn toggle_grid() -> bool {
    SHOW_GRID.with(|g| {
        let next = !g.get();
        g.set(next);
        next
    })
}

/// True when a projected point was clamped behind the camera
/// (its `xy` is the stage centre, not a real position).
fn behind(p: &fig3d::Proj) -> bool {
    p.depth >= fig3d::BEHIND_DEPTH
}

/// Build the preview's segments for one frame of the panel.
///
/// - `state`: the editor's fig3d session, if one is active. The
///   figure is drawn from `state.camera`; with no session the
///   camera is `fig3d::preview_camera()` and only the grid can
///   appear.
/// - `show_grid`: draw the ground grid (the panel passes
///   [`grid_visible`], but the decision stays the caller's, so
///   this function is a pure function of its arguments).
/// - `viewport`: the preview's size in points, handed to
///   `fig3d::project` as its stage — returned coordinates are in
///   this space, origin top-left, Y down.
pub fn preview_prims(
    state: Option<&Fig3State>,
    show_grid: bool,
    viewport: [f32; 2],
) -> Vec<PreviewPrim> {
    let fallback_cam;
    let cam = match state {
        Some(s) => &s.camera,
        None => {
            fallback_cam = fig3d::preview_camera();
            &fallback_cam
        }
    };
    // One evaluation serves both the ground offset and the
    // figure pass below.
    let world = state.map(|s| fig3d::evaluate3(&s.figure, &s.pose));
    let ground_y = world
        .as_deref()
        .and_then(|w| w.iter().map(|p| p[1]).reduce(f32::min))
        .filter(|y| y.is_finite())
        .unwrap_or(0.0);

    let mut prims = Vec::new();

    if show_grid {
        push_grid(&mut prims, cam, viewport, ground_y);
    }

    if let (Some(s), Some(w)) = (state, world.as_deref()) {
        push_figure(&mut prims, s, w, cam, viewport, ground_y);
    }

    prims
}

/// The ground grid: `fig3d::grid_line_points` segments,
/// lifted from their y = 0 plane onto `ground_y` (the
/// session figure's lowest joint, or 0 with no session —
/// see the module docs), projected; any segment with a
/// behind-camera endpoint is skipped whole.
fn push_grid(
    prims: &mut Vec<PreviewPrim>,
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
) {
    let lines = fig3d::grid_line_points(GRID_EXTENT, GRID_STEP);
    let mut points = Vec::with_capacity(lines.len() * 2);
    for (a, b) in &lines {
        points.push([a[0], ground_y, a[2]]);
        points.push([b[0], ground_y, b[2]]);
    }
    let projected = fig3d::project(&points, cam, viewport);
    for (i, _) in lines.iter().enumerate() {
        let (a, b) = (&projected[2 * i], &projected[2 * i + 1]);
        if behind(a) || behind(b) {
            continue;
        }
        prims.push(PreviewPrim {
            a: a.xy,
            b: b.xy,
            color: GRID_COLOR,
            width: GRID_WIDTH,
        });
    }
}

/// The figure: ground shadow, bones, then joint crosses, all
/// from one `project` pass over the session pose's evaluated
/// points (evaluated once by the caller).
fn push_figure(
    prims: &mut Vec<PreviewPrim>,
    state: &Fig3State,
    world: &[[f32; 3]],
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
) {
    let fig = &state.figure;
    if world.is_empty() {
        return;
    }
    let projected = fig3d::project(world, cam, viewport);

    push_shadow(prims, fig, world, cam, viewport, ground_y);

    // Bones: one segment per node that has a parent.
    for (i, node) in fig.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let (a, b) = (&projected[p], &projected[i]);
            if behind(a) || behind(b) {
                continue;
            }
            prims.push(PreviewPrim {
                a: a.xy,
                b: b.xy,
                color: fig.color,
                width: BONE_WIDTH,
            });
        }
    }

    // Joint crosses: two short segments per joint, accent for
    // the selected joint, figure colour otherwise.
    for (i, p) in projected.iter().enumerate() {
        if behind(p) {
            continue;
        }
        let color = if i == state.selected {
            SELECTED_COLOR
        } else {
            fig.color
        };
        let [x, y] = p.xy;
        prims.push(PreviewPrim {
            a: [x - JOINT_HALF, y],
            b: [x + JOINT_HALF, y],
            color,
            width: JOINT_WIDTH,
        });
        prims.push(PreviewPrim {
            a: [x, y - JOINT_HALF],
            b: [x, y + JOINT_HALF],
            color,
            width: JOINT_WIDTH,
        });
    }
}

/// The ground shadow: an 8-segment ring centred on the point
/// of the ground plane (at `ground_y`, the figure's lowest
/// joint — see the module docs) directly under the figure's
/// root (the node with no parent), radius `SHADOW_RADIUS`
/// world units measured through that point's projected scale.
/// Skipped when the root or its ground point is behind the
/// camera.
fn push_shadow(
    prims: &mut Vec<PreviewPrim>,
    fig: &fig3d::Figure3,
    world: &[[f32; 3]],
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
) {
    let Some(root) = fig.nodes.iter().position(|n| n.parent.is_none()) else {
        return;
    };
    let r = world[root];
    let ground = fig3d::project(&[[r[0], ground_y, r[2]]], cam, viewport);
    let centre = ground[0];
    if behind(&centre) {
        return;
    }
    let rx = SHADOW_RADIUS * centre.scale;
    let ry = rx * SHADOW_SQUASH;
    let mut ring = Vec::with_capacity(SHADOW_SEGMENTS);
    for k in 0..SHADOW_SEGMENTS {
        let t = std::f32::consts::TAU * k as f32 / SHADOW_SEGMENTS as f32;
        ring.push([
            centre.xy[0] + rx * t.cos(),
            centre.xy[1] + ry * t.sin(),
        ]);
    }
    for k in 0..SHADOW_SEGMENTS {
        prims.push(PreviewPrim {
            a: ring[k],
            b: ring[(k + 1) % SHADOW_SEGMENTS],
            color: SHADOW_COLOR,
            width: SHADOW_WIDTH,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::fig3d::{Figure3, Pose3};

    const VP: [f32; 2] = [PREVIEW_W, PREVIEW_H];

    /// A session state over the rest humanoid, as `enter_3d`
    /// builds it but with the preview camera.
    fn rest_state(selected: usize) -> Fig3State {
        let figure = fig3d::humanoid3();
        let pose = Pose3::rest(&figure, [0.0, 0.0, 0.0]);
        Fig3State {
            figure,
            pose,
            camera: fig3d::preview_camera(),
            selected,
            pose_b: None,
        }
    }

    fn inside(p: [f32; 2]) -> bool {
        (0.0..=PREVIEW_W).contains(&p[0]) && (0.0..=PREVIEW_H).contains(&p[1])
    }

    #[test]
    fn rest_humanoid_projects_inside_the_viewport() {
        // The calibration proof: with §3's preview camera, every
        // endpoint of every figure prim (bones, joint crosses,
        // shadow ring) lands inside the preview viewport.
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP);
        assert!(!prims.is_empty(), "a rest figure draws something");
        for prim in &prims {
            assert!(inside(prim.a), "prim starts outside: {:?}", prim.a);
            assert!(inside(prim.b), "prim ends outside: {:?}", prim.b);
        }
        // Composition: one bone per parented node, two cross
        // segments per joint, one shadow ring.
        let bones = state
            .figure
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .count();
        let joints = state.figure.nodes.len();
        assert_eq!(prims.len(), bones + 2 * joints + SHADOW_SEGMENTS);
    }

    #[test]
    fn grid_prims_exist_when_enabled_and_vanish_when_disabled() {
        let on = preview_prims(None, true, VP);
        assert!(!on.is_empty(), "the grid draws with no session");
        assert!(on
            .iter()
            .all(|p| p.color == GRID_COLOR && p.width == GRID_WIDTH));
        let off = preview_prims(None, false, VP);
        assert!(off.is_empty(), "no session + no grid = no prims");
    }

    #[test]
    fn none_state_yields_grid_only() {
        let prims = preview_prims(None, true, VP);
        assert!(prims.iter().all(|p| p.color == GRID_COLOR));
        // With a session the grid comes first, then figure work.
        let state = rest_state(0);
        let both = preview_prims(Some(&state), true, VP);
        assert_eq!(both.len(), prims.len() + preview_prims(Some(&state), false, VP).len());
        assert!(both[..prims.len()]
            .iter()
            .all(|p| p.color == GRID_COLOR));
    }

    #[test]
    fn selected_joint_uses_the_accent_colour() {
        let state = rest_state(2);
        let prims = preview_prims(Some(&state), false, VP);
        let accent: Vec<&PreviewPrim> =
            prims.iter().filter(|p| p.color == SELECTED_COLOR).collect();
        assert_eq!(accent.len(), 2, "exactly the selected joint's cross");
        // The cross is centred on the head's projected position.
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let head = fig3d::project(&world, &state.camera, VP)[2];
        for prim in accent {
            let mid = [
                (prim.a[0] + prim.b[0]) / 2.0,
                (prim.a[1] + prim.b[1]) / 2.0,
            ];
            assert!((mid[0] - head.xy[0]).abs() < 1e-3);
            assert!((mid[1] - head.xy[1]).abs() < 1e-3);
        }
        // No other joint borrows the accent (the humanoid's own
        // colour is not the accent).
        assert_ne!(state.figure.color, SELECTED_COLOR);
    }

    #[test]
    fn bones_use_the_figure_colour_and_width() {
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP);
        let bones: Vec<&PreviewPrim> = prims
            .iter()
            .filter(|p| p.width == BONE_WIDTH)
            .collect();
        let expected = state
            .figure
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .count();
        assert_eq!(bones.len(), expected);
        assert!(bones.iter().all(|p| p.color == state.figure.color));
    }

    #[test]
    fn shadow_ring_sits_under_the_root() {
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP);
        let ring: Vec<&PreviewPrim> =
            prims.iter().filter(|p| p.color == SHADOW_COLOR).collect();
        assert_eq!(ring.len(), SHADOW_SEGMENTS);
        assert!(ring.iter().all(|p| p.width == SHADOW_WIDTH));
        // The ring's centroid matches the projected ground point
        // under the root: the root's x/z (the origin at rest) at
        // the figure's lowest joint (its feet, below the origin).
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let feet = world
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        let ground = fig3d::project(&[[0.0, feet, 0.0]], &state.camera, VP)[0];
        let cx = ring.iter().map(|p| p.a[0]).sum::<f32>() / SHADOW_SEGMENTS as f32;
        let cy = ring.iter().map(|p| p.a[1]).sum::<f32>() / SHADOW_SEGMENTS as f32;
        assert!((cx - ground.xy[0]).abs() < 1e-3);
        assert!((cy - ground.xy[1]).abs() < 1e-3);
    }

    #[test]
    fn figure_behind_the_camera_draws_nothing() {
        // Root far on the camera's side of the target: every
        // point clamps behind, so bones, joints and the shadow
        // are all skipped (the grid is a separate concern).
        let figure: Figure3 = fig3d::humanoid3();
        let pose = Pose3::rest(&figure, [0.0, 0.0, 10_000.0]);
        let state = Fig3State {
            figure,
            pose,
            camera: fig3d::preview_camera(),
            selected: 0,
            pose_b: None,
        };
        let prims = preview_prims(Some(&state), false, VP);
        assert!(prims.is_empty());
    }

    #[test]
    fn grid_follows_the_figures_feet_with_a_session() {
        // The engine defines no ground plane; with a session
        // the grid sits at the figure's lowest joint (its feet,
        // below the origin at rest), so on screen (Y down) the
        // grid reaches LOWER than the empty world's y = 0 grid.
        let state = rest_state(0);
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let feet = world.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        assert!(feet < 0.0, "the rest humanoid's feet are below the origin");
        let bare = preview_prims(None, true, VP);
        let with = preview_prims(Some(&state), true, VP);
        let grid_max_y = |prims: &[PreviewPrim]| {
            prims
                .iter()
                .filter(|p| p.color == GRID_COLOR)
                .flat_map(|p| [p.a[1], p.b[1]])
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!(grid_max_y(&with) > grid_max_y(&bare));
        assert_ne!(with, bare);
    }

    #[test]
    fn grid_visibility_defaults_on_and_toggles() {
        assert!(grid_visible(), "the grid starts visible");
        assert!(!toggle_grid());
        assert!(!grid_visible());
        assert!(toggle_grid());
        assert!(grid_visible());
    }
}
