//! True-3D figure subsystem (first verified slice).
//!
//! A small, self-contained skeletal model in 3D: figures are trees of
//! named nodes with 3D offsets, poses are per-node Euler rotations,
//! forward kinematics composes parent-first, and a perspective camera
//! projects world points back onto the 2D stage. This module does not
//! touch the 2D model in `model.rs`; it exists so 3D figures can be
//! evaluated, projected and depth-sorted (painter's order) anywhere the
//! 2D pipeline needs them.
//!
//! ## Conventions (read before trusting the numbers)
//!
//! - **World axes:** right-handed — X right, Y **up**, Z toward the
//!   viewer (out of the screen). The 2D stage is Y-down; the flip
//!   happens once, inside [`project`].
//! - **Euler order:** a rotation is `[x, y, z]` degrees and builds the
//!   matrix `R = Rz(z) · Ry(y) · Rx(x)` (column-vector convention:
//!   `v' = R · v`), so a vector is rotated about X first, then Y, then
//!   Z. Consequence worth knowing: a +90° rotation about Y maps +X to
//!   **−Z** (the point swings away from the viewer).
//! - **FK:** each node's local transform is `T(offset) · R(euler)` and
//!   its world transform is `parent_world · local`. A node's own
//!   rotation therefore never moves its own position — only its
//!   children's — exactly like a skeletal rig. A node's world position
//!   is `parent_pos + parent_world_rotation · offset`. Nodes must be
//!   stored parent-before-child (the same rule as the 2D model).
//! - **Poses:** `rotations` may be shorter than the node list; missing
//!   entries are identity. Extra entries are ignored.
//! - **Projection:** vertical field of view; `focal = (stage_h / 2) /
//!   tan(fov / 2)`. `Proj.scale` is pixels-per-world-unit at that
//!   point's depth (`focal / depth`), so nearer points always have a
//!   larger scale. Points at or behind the near plane (depth ≤ 0.01)
//!   are clamped: `xy` = stage centre, `depth` = 1e6, `scale` =
//!   `focal / 1e6`. They still sort first in [`draw_order`], which is
//!   what a painter's renderer wants.

use crate::model::V2;
use serde::{Deserialize, Serialize};

/// One joint of a 3D figure: `offset` is the rest-pose vector from the
/// parent joint to this joint, in world-axis directions (Y up).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node3 {
    pub name: String,
    pub parent: Option<usize>,
    pub offset: [f32; 3],
    /// Head joints get a head ring when rendered (parity with 2D).
    pub head: bool,
}

/// A 3D stick figure definition: an ordered node tree plus a colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Figure3 {
    pub name: String,
    pub nodes: Vec<Node3>,
    pub color: u32,
}

/// A pose for a [`Figure3`]: where the root sits, plus one Euler
/// rotation `[x, y, z]` (degrees, XYZ order — see module docs) per
/// node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose3 {
    pub root: [f32; 3],
    pub rotations: Vec<[f32; 3]>,
}

impl Pose3 {
    /// The rest pose: root at `root`, every joint unrotated.
    pub fn rest(fig: &Figure3, root: [f32; 3]) -> Self {
        Pose3 {
            root,
            rotations: vec![[0.0; 3]; fig.nodes.len()],
        }
    }
}

/// Perspective camera looking at `target` from `distance` world units.
/// `yaw`/`pitch` are degrees: yaw 0 places the camera on +Z from the
/// target (looking down −Z), positive yaw orbits toward +X; positive
/// pitch raises the camera above the target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera3 {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_deg: f32,
    pub target: [f32; 3],
}

/// One projected point: stage position, view depth (larger = farther,
/// 1e6 = clamped behind-camera), and pixels-per-world-unit scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proj {
    pub xy: V2,
    pub depth: f32,
    pub scale: f32,
}

// ---------------------------------------------------------------------
// Small vector / matrix helpers (row-major 3x3, column vectors).
// ---------------------------------------------------------------------

pub type Mat3 = [[f32; 3]; 3];

pub fn mat3_identity() -> Mat3 {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

/// `a · b` — applying `b` to a vector first, then `a`.
pub fn mat3_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            out[r][c] = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
        }
    }
    out
}

pub fn mat3_apply(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Build a rotation matrix from Euler angles `[x, y, z]` in degrees,
/// XYZ order: `R = Rz · Ry · Rx` (see module docs).
pub fn mat3_from_euler(e: [f32; 3]) -> Mat3 {
    let (x, y, z) = (
        e[0].to_radians(),
        e[1].to_radians(),
        e[2].to_radians(),
    );
    let (cx, sx) = (x.cos(), x.sin());
    let (cy, sy) = (y.cos(), y.sin());
    let (cz, sz) = (z.cos(), z.sin());
    let rx: Mat3 = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let ry: Mat3 = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rz: Mat3 = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mat3_mul(&rz, &mat3_mul(&ry, &rx))
}

fn v_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn v_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn v_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn v_norm(a: [f32; 3]) -> [f32; 3] {
    let len = v_dot(a, a).sqrt();
    if len < 1e-9 {
        return [0.0; 3];
    }
    [a[0] / len, a[1] / len, a[2] / len]
}

// ---------------------------------------------------------------------
// Forward kinematics.
// ---------------------------------------------------------------------

/// Evaluate a figure in a pose, returning every node's world position.
/// Parent rotations compose parent-first; see the module docs for the
/// exact transform convention.
pub fn evaluate3(fig: &Figure3, pose: &Pose3) -> Vec<[f32; 3]> {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(fig.nodes.len());
    let mut rotations: Vec<Mat3> = Vec::with_capacity(fig.nodes.len());
    for (i, node) in fig.nodes.iter().enumerate() {
        let local = pose
            .rotations
            .get(i)
            .copied()
            .unwrap_or([0.0; 3]);
        let local_r = mat3_from_euler(local);
        match node.parent {
            None => {
                positions.push(v_add(pose.root, node.offset));
                rotations.push(local_r);
            }
            Some(p) => {
                let world_r = mat3_mul(&rotations[p], &local_r);
                let pos = v_add(positions[p], mat3_apply(&rotations[p], node.offset));
                positions.push(pos);
                rotations.push(world_r);
            }
        }
    }
    positions
}

// ---------------------------------------------------------------------
// Camera + projection.
// ---------------------------------------------------------------------

/// Depth assigned to points at/behind the near plane.
pub const BEHIND_DEPTH: f32 = 1.0e6;
/// Points with view depth at or below this count as behind the camera.
const NEAR_PLANE: f32 = 0.01;

/// (camera position, right, up, forward, focal length in pixels).
fn view_frame(cam: &Camera3, stage: [f32; 2]) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3], f32) {
    // Clamp pitch off the poles so the basis never degenerates.
    let pitch = cam.pitch.clamp(-89.9, 89.9).to_radians();
    let yaw = cam.yaw.to_radians();
    let dir = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let pos = [
        cam.target[0] + dir[0] * cam.distance,
        cam.target[1] + dir[1] * cam.distance,
        cam.target[2] + dir[2] * cam.distance,
    ];
    let forward = v_norm(v_sub(cam.target, pos));
    let right = v_norm(v_cross(forward, [0.0, 1.0, 0.0]));
    let up = v_cross(right, forward);
    let focal = (stage[1] / 2.0) / (cam.fov_deg.to_radians() / 2.0).tan();
    (pos, right, up, forward, focal)
}

/// Project world points to the stage. Y is flipped here (world Y-up →
/// stage Y-down); behind-camera points clamp as documented above.
pub fn project(points: &[[f32; 3]], cam: &Camera3, stage: [f32; 2]) -> Vec<Proj> {
    let (pos, right, up, forward, focal) = view_frame(cam, stage);
    let centre = [stage[0] / 2.0, stage[1] / 2.0];
    points
        .iter()
        .map(|&p| {
            let d = v_sub(p, pos);
            let depth = v_dot(d, forward);
            if depth <= NEAR_PLANE {
                return Proj {
                    xy: centre,
                    depth: BEHIND_DEPTH,
                    scale: focal / BEHIND_DEPTH,
                };
            }
            let x_c = v_dot(d, right);
            let y_c = v_dot(d, up);
            Proj {
                xy: [
                    centre[0] + x_c * focal / depth,
                    centre[1] - y_c * focal / depth,
                ],
                depth,
                scale: focal / depth,
            }
        })
        .collect()
}

/// Node indices sorted far-to-near by view depth, for painter's-order
/// rendering (draw the farthest first). Behind-camera points carry the
/// clamp depth, so they sort first of all.
pub fn draw_order(points: &[[f32; 3]], cam: &Camera3) -> Vec<usize> {
    let (pos, _r, _u, forward, _f) = view_frame(cam, [1.0, 1.0]);
    let mut order: Vec<usize> = (0..points.len()).collect();
    let depth_of = |p: [f32; 3]| {
        let d = v_dot(v_sub(p, pos), forward);
        if d <= NEAR_PLANE {
            BEHIND_DEPTH
        } else {
            d
        }
    };
    order.sort_by(|&a, &b| {
        depth_of(points[b])
            .partial_cmp(&depth_of(points[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

// ---------------------------------------------------------------------
// Preset figure + 2D segment extraction.
// ---------------------------------------------------------------------

/// A 3D stick humanoid in rest pose (all Z offsets 0, so it starts
/// planar): hips → chest → head, two arms, two legs.
pub fn humanoid3() -> Figure3 {
    let n = |name: &str, parent: Option<usize>, offset: [f32; 3], head: bool| Node3 {
        name: name.into(),
        parent,
        offset,
        head,
    };
    Figure3 {
        name: "Humanoid3".into(),
        color: 0xE8E8EC,
        nodes: vec![
            n("hips", None, [0.0, 0.0, 0.0], false),
            n("chest", Some(0), [0.0, 30.0, 0.0], false),
            n("head", Some(1), [0.0, 28.0, 0.0], true),
            n("upperArm.L", Some(1), [-16.0, 22.0, 0.0], false),
            n("foreArm.L", Some(3), [-24.0, 0.0, 0.0], false),
            n("hand.L", Some(4), [-22.0, 0.0, 0.0], false),
            n("upperArm.R", Some(1), [16.0, 22.0, 0.0], false),
            n("foreArm.R", Some(6), [24.0, 0.0, 0.0], false),
            n("hand.R", Some(7), [22.0, 0.0, 0.0], false),
            n("thigh.L", Some(0), [-9.0, -4.0, 0.0], false),
            n("shin.L", Some(9), [0.0, -34.0, 0.0], false),
            n("foot.L", Some(10), [0.0, -32.0, 0.0], false),
            n("thigh.R", Some(0), [9.0, -4.0, 0.0], false),
            n("shin.R", Some(12), [0.0, -34.0, 0.0], false),
            n("foot.R", Some(13), [0.0, -32.0, 0.0], false),
        ],
    }
}

/// Nominal segment thickness in world units, used by [`segments2d`]
/// (Node3 carries no per-node thickness in this slice).
pub const SEGMENT_THICKNESS: f32 = 5.0;

/// Turn projected points into drawable 2D segments: one
/// `(from, to, width_px)` per parent→node pair, where the width is
/// the nominal thickness scaled by the average of the endpoints'
/// projection scales (nearer segments draw thicker).
pub fn segments2d(fig: &Figure3, pts: &[Proj]) -> Vec<(V2, V2, f32)> {
    let mut out = Vec::new();
    for (i, node) in fig.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            if let (Some(&a), Some(&b)) = (pts.get(p), pts.get(i)) {
                let width = SEGMENT_THICKNESS * (a.scale + b.scale) / 2.0;
                out.push((a.xy, b.xy, width));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
    }

    fn cam() -> Camera3 {
        Camera3 {
            yaw: 0.0,
            pitch: 0.0,
            distance: 400.0,
            fov_deg: 40.0,
            target: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn identity_pose_reproduces_rest_offsets_exactly() {
        let fig = humanoid3();
        let root = [10.0, 20.0, 5.0];
        let pos = evaluate3(&fig, &Pose3::rest(&fig, root));
        // Hand-accumulated rest positions (all values are exact in f32).
        assert_eq!(pos[0], [10.0, 20.0, 5.0]); // hips
        assert_eq!(pos[1], [10.0, 50.0, 5.0]); // chest
        assert_eq!(pos[2], [10.0, 78.0, 5.0]); // head
        assert_eq!(pos[5], [-52.0, 72.0, 5.0]); // hand.L
        assert_eq!(pos[8], [72.0, 72.0, 5.0]); // hand.R
        assert_eq!(pos[11], [1.0, -50.0, 5.0]); // foot.L
        assert_eq!(pos[14], [19.0, -50.0, 5.0]); // foot.R
    }

    #[test]
    fn fk_composes_parent_first() {
        // A bent elbow: shoulder rotates the whole arm chain, the
        // elbow's own rotation moves only the hand.
        let fig = Figure3 {
            name: "arm".into(),
            color: 0,
            nodes: vec![
                Node3 { name: "root".into(), parent: None, offset: [0.0; 3], head: false },
                Node3 { name: "elbow".into(), parent: Some(0), offset: [10.0, 0.0, 0.0], head: false },
                Node3 { name: "hand".into(), parent: Some(1), offset: [10.0, 0.0, 0.0], head: false },
            ],
        };
        let mut pose = Pose3::rest(&fig, [0.0; 3]);
        pose.rotations[0] = [0.0, 0.0, 90.0]; // root: +X -> +Y
        pose.rotations[1] = [0.0, 0.0, 90.0]; // elbow: another 90 about Z
        let pos = evaluate3(&fig, &pose);
        assert!(approx(pos[1], [0.0, 10.0, 0.0], 1e-3), "elbow {:?}", pos[1]);
        assert!(approx(pos[2], [-10.0, 10.0, 0.0], 1e-3), "hand {:?}", pos[2]);
    }

    #[test]
    fn root_yaw_90_swings_plus_x_to_minus_z() {
        // Documented convention: +90 deg about Y maps +X to -Z.
        let fig = Figure3 {
            name: "t".into(),
            color: 0,
            nodes: vec![
                Node3 { name: "root".into(), parent: None, offset: [0.0; 3], head: false },
                Node3 { name: "tip".into(), parent: Some(0), offset: [10.0, 0.0, 0.0], head: false },
            ],
        };
        let mut pose = Pose3::rest(&fig, [0.0; 3]);
        pose.rotations[0] = [0.0, 90.0, 0.0];
        let pos = evaluate3(&fig, &pose);
        assert_eq!(pos[0], [0.0; 3], "root position is not moved by its own rotation");
        assert!(approx(pos[1], [0.0, 0.0, -10.0], 1e-3), "tip {:?}", pos[1]);
    }

    #[test]
    fn target_projects_to_stage_centre_at_any_yaw_pitch() {
        let stage = [960.0, 720.0];
        for (yaw, pitch) in [(0.0, 0.0), (45.0, 20.0), (180.0, -30.0), (270.0, 60.0)] {
            let c = Camera3 { yaw, pitch, ..cam() };
            let p = project(&[[0.0, 0.0, 0.0]], &c, stage)[0];
            assert!(
                (p.xy[0] - 480.0).abs() < 0.01 && (p.xy[1] - 360.0).abs() < 0.01,
                "yaw {yaw} pitch {pitch} -> {:?}",
                p.xy
            );
            assert!((p.depth - 400.0).abs() < 0.01);
        }
    }

    #[test]
    fn yaw_and_pitch_move_projection_predictably() {
        let stage = [960.0, 720.0];
        // +X point sits right of centre at yaw 0, left at yaw 180.
        let pt = [[50.0, 0.0, 0.0]];
        let right = project(&pt, &cam(), stage)[0];
        assert!(right.xy[0] > 480.0, "{:?}", right.xy);
        let flipped = project(&pt, &Camera3 { yaw: 180.0, ..cam() }, stage)[0];
        assert!(flipped.xy[0] < 480.0, "{:?}", flipped.xy);
        // A point above the target projects above stage centre
        // (world Y-up flips to stage Y-down).
        let above = project(&[[0.0, 50.0, 0.0]], &cam(), stage)[0];
        assert!(above.xy[1] < 360.0, "{:?}", above.xy);
        // Raising the camera (pitch) makes it look down at the
        // target: a point between camera and target starts on-axis
        // (stage centre) and sinks below centre as pitch grows.
        let near_pt = [[0.0, 0.0, 50.0]];
        let level = project(&near_pt, &cam(), stage)[0];
        assert!((level.xy[1] - 360.0).abs() < 0.01, "level {:?}", level.xy);
        let pitched = project(&near_pt, &Camera3 { pitch: 45.0, ..cam() }, stage)[0];
        assert!(pitched.xy[1] > level.xy[1], "pitched {:?} vs level {:?}", pitched.xy, level.xy);
    }

    #[test]
    fn nearer_point_projects_larger() {
        let stage = [960.0, 720.0];
        let pts = project(&[[0.0, 0.0, 100.0], [0.0, 0.0, -100.0]], &cam(), stage);
        assert!(pts[0].depth < pts[1].depth);
        assert!(pts[0].scale > pts[1].scale, "{} vs {}", pts[0].scale, pts[1].scale);
        // Both on the view axis: same centre, scale ratio = depth ratio.
        assert!((pts[0].xy[0] - 480.0).abs() < 0.01 && (pts[1].xy[0] - 480.0).abs() < 0.01);
        let ratio = pts[0].scale / pts[1].scale;
        assert!((ratio - 500.0 / 300.0).abs() < 0.01, "ratio {ratio}");
    }

    #[test]
    fn behind_camera_clamps_to_centre_with_large_depth() {
        let stage = [960.0, 720.0];
        // Camera sits at z=+400 looking to origin; z=+500 is behind it.
        let p = project(&[[0.0, 0.0, 500.0]], &cam(), stage)[0];
        assert_eq!(p.xy, [480.0, 360.0]);
        assert_eq!(p.depth, BEHIND_DEPTH);
    }

    #[test]
    fn draw_order_is_far_to_near() {
        let pts = [
            [0.0, 0.0, 100.0],  // near (depth 300)
            [0.0, 0.0, -100.0], // far (depth 500)
            [0.0, 0.0, 0.0],    // mid (depth 400)
            [0.0, 0.0, 900.0],  // behind camera (clamped, drawn first)
        ];
        let order = draw_order(&pts, &cam());
        assert_eq!(order, vec![3, 1, 2, 0]);
    }

    #[test]
    fn segments2d_pairs_and_scales_width_by_depth() {
        let fig = humanoid3();
        let stage = [960.0, 720.0];
        let world = evaluate3(&fig, &Pose3::rest(&fig, [0.0, 0.0, 0.0]));
        let near = project(&world, &cam(), stage);
        let segs = segments2d(&fig, &near);
        assert_eq!(segs.len(), fig.nodes.len() - 1, "every non-root node gives one segment");
        assert!(segs.iter().all(|&(_, _, w)| w > 0.0));
        // The same figure twice as far away draws thinner segments.
        let far = project(&world, &Camera3 { distance: 800.0, ..cam() }, stage);
        let segs_far = segments2d(&fig, &far);
        assert!(segs_far[0].2 < segs[0].2, "{} vs {}", segs_far[0].2, segs[0].2);
    }

    #[test]
    fn short_rotation_list_defaults_to_identity() {
        let fig = humanoid3();
        let pose = Pose3 { root: [1.0, 2.0, 3.0], rotations: vec![] };
        let pos = evaluate3(&fig, &pose);
        assert_eq!(pos[2], [1.0, 60.0, 3.0]);
    }
}
