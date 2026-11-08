//! Phase M, section 9 — 3D FIGURE ON THE MAIN STAGE: pure
//! stage-space segments for the fig3d session figure, so the
//! stage view can draw the 3D figure on the main stage (not
//! only in the 3D panel's preview canvas, where Phase L left
//! it) without any projection maths living in the view code.
//!
//! ## Relationship to `fig3_preview`
//!
//! This module is the panel preview's sibling, not a copy.
//! The drawing rules are the preview's own, shared through
//! `fig3_preview`'s public constants and helpers:
//!
//! - The ground rule is identical: the engine defines no
//!   ground plane, so the grid and the ground shadow sit at
//!   `fig3_preview::ground_y_of(world)` — the figure's lowest
//!   evaluated joint — never on y = 0 through its hips.
//! - Colours and widths are `fig3_preview`'s constants
//!   (`GRID_COLOR`, `BONE_WIDTH`, `JOINT_HALF`, …), so the
//!   stage figure and the panel figure read as one figure.
//! - Every segment is cut to the stage rect through
//!   `fig3_preview::clip_segment_to_viewport`: the Rune
//!   canvas does not clip its drawing (Phase L's headless
//!   snap proved grid lines painting over the shell), and on
//!   the main stage unclipped lines would paint over the
//!   whole window.
//!
//! The differences from the preview are the point of the
//! module: the projection viewport is the PROJECT STAGE
//! (`core.project.stage`), so the returned coordinates are
//! stage coordinates like every other stage drawing; the
//! camera is always the session camera (there is no
//! no-session fallback — with no session there is nothing
//! to draw); and the prim type is this module's own
//! [`StageFig3Prim`], the fixed contract the stage view
//! consumes.
//!
//! ## Prim order (painter's order)
//!
//! Ground grid, ground shadow, bones, joint crosses — the
//! preview's order, so later prims draw over earlier ones.
//!
//! ## Honest notes
//!
//! - Segments only, exactly like the preview: joints are
//!   two 3 pt crossing segments and the shadow an 8-segment
//!   squashed ring, so a consumer needs nothing but line
//!   drawing.
//! - A segment touching a behind-camera point (depth at
//!   `fig3d::BEHIND_DEPTH`, whose xy is the stage centre,
//!   not a real position) is skipped whole rather than
//!   drawn to the centre.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fig3d::{self, Fig3State};

use crate::rune_sec::fig3_preview;

/// One segment in STAGE coordinates, ready to stroke.
/// This is the fixed contract the stage view (§11) draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageFig3Prim {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// Packed `0xRRGGBB`, the packing the rest of the shell uses.
    pub color: u32,
    pub width: f32,
}

/// True when a projected point was clamped behind the camera
/// (its `xy` is the stage centre, not a real position).
/// Mirrors `fig3_preview`'s private helper of the same name.
fn behind(p: &fig3d::Proj) -> bool {
    p.depth >= fig3d::BEHIND_DEPTH
}

/// Build the main stage's segments for the fig3d session
/// figure, in stage coordinates.
///
/// - No session (`core.fig3` is `None`) → an empty vec: the
///   stage draws nothing extra.
/// - With a session, the figure is evaluated once and
///   projected with the SESSION camera through the project
///   stage size, so the prims land where the stage's own
///   camera transform expects stage coordinates to land.
/// - Every returned segment is clipped to the stage rect
///   `[0, stage_w] × [0, stage_h]`; segments wholly outside
///   are dropped.
///
/// A pure function of the core: no shell state, no
/// thread-locals.
pub fn stage_fig3_prims(core: &EditorCore) -> Vec<StageFig3Prim> {
    let Some(state) = core.fig3.as_ref() else {
        return Vec::new();
    };
    let stage = core.project.stage;
    // One evaluation serves the ground offset and the figure.
    let world = fig3d::evaluate3(&state.figure, &state.pose);
    let ground_y = fig3_preview::ground_y_of(&world);

    let mut prims = Vec::new();
    push_grid(&mut prims, state, stage, ground_y);
    push_figure(&mut prims, state, &world, stage, ground_y);

    // The stage canvas does not clip its drawing to its own
    // rect (the Phase L grid bleed), so every segment is cut
    // to the stage here — the one place all stage-fig3
    // drawing passes through.
    prims
        .into_iter()
        .filter_map(|p| {
            fig3_preview::clip_segment_to_viewport(p.a, p.b, stage).map(|(a, b)| {
                StageFig3Prim {
                    a,
                    b,
                    color: p.color,
                    width: p.width,
                }
            })
        })
        .collect()
}

/// The ground grid: `fig3d::grid_line_points` segments at
/// the preview's extent/step, lifted from their y = 0 plane
/// onto `ground_y` (the figure's lowest joint — see the
/// module docs), projected through the session camera;
/// any segment with a behind-camera endpoint is skipped
/// whole. Mirrors `fig3_preview`'s grid pass.
fn push_grid(
    prims: &mut Vec<StageFig3Prim>,
    state: &Fig3State,
    stage: [f32; 2],
    ground_y: f32,
) {
    let lines = fig3d::grid_line_points(fig3_preview::GRID_EXTENT, fig3_preview::GRID_STEP);
    let mut points = Vec::with_capacity(lines.len() * 2);
    for (a, b) in &lines {
        points.push([a[0], ground_y, a[2]]);
        points.push([b[0], ground_y, b[2]]);
    }
    let projected = fig3d::project(&points, &state.camera, stage);
    for (i, _) in lines.iter().enumerate() {
        let (a, b) = (&projected[2 * i], &projected[2 * i + 1]);
        if behind(a) || behind(b) {
            continue;
        }
        prims.push(StageFig3Prim {
            a: a.xy,
            b: b.xy,
            color: fig3_preview::GRID_COLOR,
            width: fig3_preview::GRID_WIDTH,
        });
    }
}

/// The figure: ground shadow, bones, then joint crosses, all
/// from one `project` pass over the session pose's evaluated
/// points (evaluated once by the caller). Mirrors
/// `fig3_preview`'s figure pass.
fn push_figure(
    prims: &mut Vec<StageFig3Prim>,
    state: &Fig3State,
    world: &[[f32; 3]],
    stage: [f32; 2],
    ground_y: f32,
) {
    let fig = &state.figure;
    if world.is_empty() {
        return;
    }
    let projected = fig3d::project(world, &state.camera, stage);

    push_shadow(prims, state, world, stage, ground_y);

    // Bones: one segment per node that has a parent.
    for (i, node) in fig.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let (a, b) = (&projected[p], &projected[i]);
            if behind(a) || behind(b) {
                continue;
            }
            prims.push(StageFig3Prim {
                a: a.xy,
                b: b.xy,
                color: fig.color,
                width: fig3_preview::BONE_WIDTH,
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
            fig3_preview::SELECTED_COLOR
        } else {
            fig.color
        };
        let [x, y] = p.xy;
        prims.push(StageFig3Prim {
            a: [x - fig3_preview::JOINT_HALF, y],
            b: [x + fig3_preview::JOINT_HALF, y],
            color,
            width: fig3_preview::JOINT_WIDTH,
        });
        prims.push(StageFig3Prim {
            a: [x, y - fig3_preview::JOINT_HALF],
            b: [x, y + fig3_preview::JOINT_HALF],
            color,
            width: fig3_preview::JOINT_WIDTH,
        });
    }
}

/// The ground shadow: an 8-segment ring centred on the point
/// of the ground plane (at `ground_y`, the figure's lowest
/// joint — see the module docs) directly under the figure's
/// root (the node with no parent), radius
/// `fig3_preview::SHADOW_RADIUS` world units measured
/// through that point's projected scale, squashed vertically
/// by `fig3_preview::SHADOW_SQUASH`. Skipped when the root
/// or its ground point is behind the camera. Mirrors
/// `fig3_preview`'s shadow pass.
fn push_shadow(
    prims: &mut Vec<StageFig3Prim>,
    state: &Fig3State,
    world: &[[f32; 3]],
    stage: [f32; 2],
    ground_y: f32,
) {
    let fig = &state.figure;
    let Some(root) = fig.nodes.iter().position(|n| n.parent.is_none()) else {
        return;
    };
    let r = world[root];
    let ground = fig3d::project(&[[r[0], ground_y, r[2]]], &state.camera, stage);
    let centre = ground[0];
    if behind(&centre) {
        return;
    }
    let rx = fig3_preview::SHADOW_RADIUS * centre.scale;
    let ry = rx * fig3_preview::SHADOW_SQUASH;
    let mut ring = Vec::with_capacity(fig3_preview::SHADOW_SEGMENTS);
    for k in 0..fig3_preview::SHADOW_SEGMENTS {
        let t = std::f32::consts::TAU * k as f32 / fig3_preview::SHADOW_SEGMENTS as f32;
        ring.push([
            centre.xy[0] + rx * t.cos(),
            centre.xy[1] + ry * t.sin(),
        ]);
    }
    for k in 0..fig3_preview::SHADOW_SEGMENTS {
        prims.push(StageFig3Prim {
            a: ring[k],
            b: ring[(k + 1) % fig3_preview::SHADOW_SEGMENTS],
            color: fig3_preview::SHADOW_COLOR,
            width: fig3_preview::SHADOW_WIDTH,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::fig3d::Pose3;

    fn inside_stage(p: [f32; 2], stage: [f32; 2]) -> bool {
        (0.0..=stage[0]).contains(&p[0]) && (0.0..=stage[1]).contains(&p[1])
    }

    /// A session state over the rest humanoid with the
    /// preview camera, as `fig3_preview`'s tests build it.
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

    #[test]
    fn no_session_yields_no_prims() {
        let core = EditorCore::new();
        assert!(core.fig3.is_none(), "a fresh core has no 3D session");
        assert!(stage_fig3_prims(&core).is_empty());
    }

    #[test]
    fn rest_session_draws_inside_the_stage() {
        // The engine's own session path: enter_3d loads the
        // rest humanoid with the default orbit camera.
        let mut core = EditorCore::new();
        core.enter_3d();
        let stage = core.project.stage;
        let prims = stage_fig3_prims(&core);
        assert!(!prims.is_empty(), "a rest session draws something");
        for prim in &prims {
            assert!(inside_stage(prim.a, stage), "prim starts outside: {:?}", prim.a);
            assert!(inside_stage(prim.b, stage), "prim ends outside: {:?}", prim.b);
        }
        // The full drawing is present: grid, shadow and bones
        // all contribute at the default stage size.
        assert!(prims.iter().any(|p| p.color == fig3_preview::GRID_COLOR));
        assert!(prims.iter().any(|p| p.color == fig3_preview::SHADOW_COLOR));
        let fig_color = core.fig3.as_ref().unwrap().figure.color;
        assert!(prims
            .iter()
            .any(|p| p.color == fig_color && p.width == fig3_preview::BONE_WIDTH));
    }

    #[test]
    fn grid_is_present_and_clipped_on_a_small_stage() {
        // A stage far smaller than the projection wants to
        // fill: the grid's lines run hundreds of points past
        // it, so clipping must both keep grid segments and
        // hold every endpoint inside the small rect — with
        // at least one endpoint cut exactly onto its edge.
        let mut core = EditorCore::new();
        core.project.stage = [200.0, 150.0];
        core.fig3 = Some(rest_state(0));
        let stage = core.project.stage;
        let prims = stage_fig3_prims(&core);
        let grid: Vec<&StageFig3Prim> = prims
            .iter()
            .filter(|p| p.color == fig3_preview::GRID_COLOR)
            .collect();
        assert!(!grid.is_empty(), "grid segments survive clipping");
        for prim in &prims {
            assert!(inside_stage(prim.a, stage), "prim starts outside: {:?}", prim.a);
            assert!(inside_stage(prim.b, stage), "prim ends outside: {:?}", prim.b);
            assert!(prim.a[0] <= 200.0 && prim.a[1] <= 150.0);
            assert!(prim.b[0] <= 200.0 && prim.b[1] <= 150.0);
        }
        assert!(
            prims.iter().any(|p| {
                p.a[0] == stage[0] || p.b[0] == stage[0] || p.a[1] == stage[1] || p.b[1] == stage[1]
            }),
            "some segment is cut exactly onto the stage edge"
        );
    }
}
