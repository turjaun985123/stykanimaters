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
use std::path::{Path, PathBuf};

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

/// Runtime state for editing one 3D figure inside the 2D editor: the
/// figure being posed, its current pose and camera, and the selected
/// node index. Deliberately not serialised — it is editor session
/// state, like the 2D selection; baking (see `EditorCore::fig3_bake`)
/// is what turns it into a normal 2D figure in the project.
#[derive(Clone, Debug)]
pub struct Fig3State {
    pub figure: Figure3,
    pub pose: Pose3,
    pub camera: Camera3,
    pub selected: usize,
    /// A second pose marked by the user (`EditorCore::fig3_mark_pose_b`):
    /// the target `EditorCore::fig3_bake_animation` interpolates to
    /// from the current pose. `None` until marked.
    pub pose_b: Option<Pose3>,
}

/// The orbit camera a fresh 3D editing session starts with: on +Z,
/// level, 400 units out, 40-degree vertical FOV, aimed at the world
/// origin (where a session figure's root sits).
pub fn default_camera() -> Camera3 {
    Camera3 {
        yaw: 0.0,
        pitch: 0.0,
        distance: 400.0,
        fov_deg: 40.0,
        target: [0.0, 0.0, 0.0],
    }
}

/// A named camera view, built from [`default_camera`] with only
/// yaw/pitch adjusted (distance, FOV and target stay the defaults):
///
/// - `"front"` — the default orientation (yaw 0, pitch 0): camera
///   on +Z, looking at the figure face-on.
/// - `"side"` — yaw +90°: the camera sits on +X (positive yaw
///   orbits toward +X), so the figure is seen in profile.
/// - `"three-quarter"` — yaw +35°, pitch +12°: front-side and
///   slightly from above. This module's convention is that
///   positive pitch *raises* the camera above the target (see
///   [`Camera3`]), so +12° is the elevated three-quarter view.
/// - `"top"` — pitch 89.9°, the most top-down this representation
///   allows: [`project`]'s view frame (and the editor's orbit)
///   clamp pitch to ±89.9° so the camera basis never degenerates
///   at the pole.
///
/// Any other name returns `None`. Matching is exact and
/// case-sensitive.
pub fn camera_preset(name: &str) -> Option<Camera3> {
    let base = default_camera();
    match name {
        "front" => Some(base),
        "side" => Some(Camera3 { yaw: 90.0, ..base }),
        "three-quarter" => Some(Camera3 {
            yaw: 35.0,
            pitch: 12.0,
            ..base
        }),
        "top" => Some(Camera3 {
            pitch: 89.9,
            ..base
        }),
        _ => None,
    }
}

/// Segments of a square ground grid on the y = 0 plane (the
/// engine defines no ground plane — [`project`] fixes only the
/// Y-up convention; consumers offset the plane as they need),
/// as pairs of world
/// endpoints ready to be projected with [`project`] and drawn as
/// lines: for each `i` in `-extent..=extent`, one segment runs
/// along z at `x = i * step` (from `z = -extent * step` to
/// `z = +extent * step`) and one runs along x at `z = i * step`
/// (from `x = -extent * step` to `x = +extent * step`). The
/// centre lines (i = 0) are included, so the count is
/// `2 * (2 * extent + 1)`.
///
/// Order is deterministic: first all the x-constant segments in
/// ascending `i` (the lines running along z), then all the
/// z-constant segments in ascending `i` (the lines running along
/// x). Degenerate inputs — `extent <= 0`, or `step` non-finite or
/// `<= 0.0` — return an empty vector rather than a meaningless
/// or unbounded grid.
pub fn grid_line_points(extent: i32, step: f32) -> Vec<([f32; 3], [f32; 3])> {
    if extent <= 0 || !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let reach = extent as f32 * step;
    let mut out = Vec::with_capacity(2 * (2 * extent as usize + 1));
    for i in -extent..=extent {
        let x = i as f32 * step;
        out.push(([x, 0.0, -reach], [x, 0.0, reach]));
    }
    for i in -extent..=extent {
        let z = i as f32 * step;
        out.push(([-reach, 0.0, z], [reach, 0.0, z]));
    }
    out
}

/// The 3D preview's default camera: frames the REST
/// [`humanoid3`] figure (posed by [`Pose3::rest`] at the origin)
/// in a small panel viewport (the shell's preview is 308×220).
///
/// The arithmetic, from [`project`]'s maths. The rest figure's
/// joints span y from −70 (the feet: hips at 0, thigh −4, shin
/// −34, foot −32) to +58 (the head joint: chest +30, head +28),
/// so its height is H = 128 world units and its vertical centre
/// is y = −6; horizontally the hands reach x = ±62.
/// [`project`]'s focal length for a viewport of height h is
/// `(h / 2) / tan(fov / 2)`, so a figure of height H projected
/// from depth d occupies `H * (h / 2) / (tan(fov / 2) * d)`
/// pixels vertically. Setting that equal to h gives the
/// exact-fill distance `(H / 2) / tan(fov / 2)` — at fov 40°
/// that is 64 / tan(20°) ≈ 175.8. This camera multiplies the
/// exact-fill distance by a headroom factor of ≈ 1.7 and rounds
/// to 300, and targets the figure's centre [0, −6, 0], so the
/// projected rest pose sits well inside the viewport
/// (x ≈ 100..218, y ≈ 44..173 of 308×220, verified against
/// [`project`]) with room for the head ring and modest posing.
/// Yaw 25° / pitch 10° give the three-quarter view that makes
/// the ground grid read in perspective; the FOV matches
/// [`default_camera`]'s 40°.
pub fn preview_camera() -> Camera3 {
    Camera3 {
        yaw: 25.0,
        pitch: 10.0,
        distance: 300.0,
        fov_deg: 40.0,
        target: [0.0, -6.0, 0.0],
    }
}

/// Frame `fig` in `pose` on `stage`: a front camera (the
/// [`default_camera`] orientation and FOV) targeted on the posed
/// figure's centroid, with its distance chosen so the figure's
/// projected bounding box lies inside the central 80% of the stage
/// in both dimensions — i.e. within `[0.1·w, 0.9·w] × [0.1·h,
/// 0.9·h]`. Testing the box against that region (rather than only
/// its size) is what guarantees every joint lands on stage with a
/// margin: the box is centred on the projected centroid only when
/// the pose is symmetric about it, and an off-centre box of the
/// right size could still spill past an edge.
///
/// The distance search is deterministic and bounded: starting from
/// the default 400 units, the distance doubles (at most 40 times)
/// until the box fits, then 48 bisection steps narrow to nearly
/// the smallest fitting distance. Projected size shrinks
/// monotonically as distance grows, so once a distance fits, every
/// larger one does too and the bisection is sound; points behind
/// the camera (clamped projections) never count as fitting. An
/// empty figure has nothing to frame and yields
/// [`default_camera`].
pub fn frame_camera(fig: &Figure3, pose: &Pose3, stage: [f32; 2]) -> Camera3 {
    let world = evaluate3(fig, pose);
    if world.is_empty() {
        return default_camera();
    }
    let n = world.len() as f32;
    let mut centroid = [0.0; 3];
    for p in &world {
        centroid[0] += p[0];
        centroid[1] += p[1];
        centroid[2] += p[2];
    }
    let centroid = [centroid[0] / n, centroid[1] / n, centroid[2] / n];
    let mut cam = Camera3 {
        target: centroid,
        ..default_camera()
    };
    let (x0, x1) = (stage[0] * 0.1, stage[0] * 0.9);
    let (y0, y1) = (stage[1] * 0.1, stage[1] * 0.9);
    let fits = |cam: &Camera3| -> bool {
        let proj = project(&world, cam, stage);
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in &proj {
            if p.depth >= BEHIND_DEPTH {
                return false;
            }
            min[0] = min[0].min(p.xy[0]);
            min[1] = min[1].min(p.xy[1]);
            max[0] = max[0].max(p.xy[0]);
            max[1] = max[1].max(p.xy[1]);
        }
        min[0] >= x0 && max[0] <= x1 && min[1] >= y0 && max[1] <= y1
    };
    // Bracket a fitting distance, growing from the default.
    let mut lo = 1.0f32;
    let mut hi = cam.distance;
    cam.distance = hi;
    let mut grows = 0;
    while !fits(&cam) && grows < 40 {
        hi *= 2.0;
        cam.distance = hi;
        grows += 1;
    }
    if fits(&cam) {
        // Bisect down to (very nearly) the smallest fitting
        // distance; `hi` only ever moves to distances that fit.
        for _ in 0..48 {
            let mid = (lo + hi) / 2.0;
            cam.distance = mid;
            if fits(&cam) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        cam.distance = hi;
    }
    cam
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

/// Extract Euler angles `[x, y, z]` (degrees) from a rotation
/// matrix — the documented inverse of [`mat3_from_euler`] for
/// `R = Rz · Ry · Rx`.
///
/// Expanding the product in this file's layout (row-major
/// `m[row][col]`, column vectors) gives:
///
/// ```text
/// R[0] = [ cz·cy,  cz·sy·sx − sz·cx,  cz·sy·cx + sz·sx ]
/// R[1] = [ sz·cy,  sz·sy·sx + cz·cx,  sz·sy·cx − cz·sx ]
/// R[2] = [ −sy,    cy·sx,             cy·cx            ]
/// ```
///
/// so `sin(y) = −R[2][0]`, and while `cos(y) ≠ 0` the other two
/// angles fall out of the last row and the first column:
/// `x = atan2(R[2][1], R[2][2])` and `z = atan2(R[1][0], R[0][0])`.
/// The returned `y` always lies in `[-90°, 90°]` (the `asin`
/// range); a matrix built with `y` outside that range comes back
/// in the equivalent `(x ± 180°, 180° − y, z ± 180°)` form — the
/// extracted angles always rebuild the same matrix.
///
/// **Gimbal lock** (`|R[2][0]| ≈ 1`, so `y = ±90°` and `cy = 0`):
/// the first row degenerates to one combined angle —
/// `R[0][1] = sin(x − z)`, `R[0][2] = cos(x − z)` at `y = +90°`,
/// and `R[0][1] = −sin(x + z)`, `R[0][2] = −cos(x + z)` at
/// `y = −90°` — so `x` and `z` are not independently determined.
/// By convention this extraction sets `z := 0` and recovers the
/// combined angle into `x`: `x = atan2(R[0][1], R[0][2])` at
/// `+90°`, `x = atan2(−R[0][1], −R[0][2])` at `−90°`.
pub fn mat3_to_euler(m: &Mat3) -> [f32; 3] {
    // |sin(y)| this close to 1 counts as exactly ±90°; any closer
    // to the pole than this, cos(y) ≲ 1.4e-3 keeps the regular
    // atan2 pairs numerically meaningful in f32.
    const GIMBAL_EPS: f32 = 1.0e-6;
    if m[2][0] < -(1.0 - GIMBAL_EPS) {
        // sin(y) = +1 → y = +90°; recover x − z with z := 0.
        [m[0][1].atan2(m[0][2]).to_degrees(), 90.0, 0.0]
    } else if m[2][0] > 1.0 - GIMBAL_EPS {
        // sin(y) = −1 → y = −90°; recover x + z with z := 0.
        [(-m[0][1]).atan2(-m[0][2]).to_degrees(), -90.0, 0.0]
    } else {
        let y = (-m[2][0]).clamp(-1.0, 1.0).asin();
        let x = m[2][1].atan2(m[2][2]);
        let z = m[1][0].atan2(m[0][0]);
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    }
}

/// Transpose of a 3×3 matrix. For a rotation matrix the transpose
/// is also the inverse, which is how the IK solver uses it.
fn mat3_transpose(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
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
// Two-bone IK.
// ---------------------------------------------------------------------

/// Per-node world rotations under `pose`, composed exactly as
/// [`evaluate3`] composes them internally (parent-first, local
/// Euler via [`mat3_from_euler`]). `evaluate3` keeps its rotations
/// private; the IK solver needs them, so they are recomputed here.
fn world_rotations(fig: &Figure3, pose: &Pose3) -> Vec<Mat3> {
    let mut out: Vec<Mat3> = Vec::with_capacity(fig.nodes.len());
    for (i, node) in fig.nodes.iter().enumerate() {
        let local = pose
            .rotations
            .get(i)
            .copied()
            .unwrap_or([0.0; 3]);
        let local_r = mat3_from_euler(local);
        match node.parent {
            None => out.push(local_r),
            Some(p) => out.push(mat3_mul(&out[p], &local_r)),
        }
    }
    out
}

/// Rotation matrix for `angle_rad` about the unit vector `axis`
/// (Rodrigues' formula, right-handed, column vectors).
fn mat3_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> Mat3 {
    let (x, y, z) = (axis[0], axis[1], axis[2]);
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// The shortest-arc rotation taking unit vector `from` to unit
/// vector `to` (both must already be normalised). Identity when
/// the two agree; for (near-)antiparallel inputs any perpendicular
/// axis serves, and a fixed one is chosen deterministically.
fn shortest_arc(from: [f32; 3], to: [f32; 3]) -> Mat3 {
    let cr = v_cross(from, to);
    let s = v_dot(cr, cr).sqrt();
    let c = v_dot(from, to).clamp(-1.0, 1.0);
    if s < 1.0e-7 {
        if c > 0.0 {
            return mat3_identity();
        }
        // Antiparallel: rotate 180° about any axis ⊥ `from`.
        let helper = if from[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let axis = v_norm(v_cross(from, helper));
        return mat3_from_axis_angle(axis, std::f32::consts::PI);
    }
    let axis = [cr[0] / s, cr[1] / s, cr[2] / s];
    mat3_from_axis_angle(axis, s.atan2(c))
}

/// Solve a two-bone IK chain: adjust the rotations of joints
/// `upper` and `fore` so that joint `end` moves to `target` in
/// world space. `fore` must be `upper`'s child and `end` must be
/// `fore`'s child. Every other joint's rotation, and the pose
/// root, are copied from the input pose verbatim; only `upper`
/// and `fore` change.
///
/// Method: with bone lengths `L1 = |offset[fore]|` and
/// `L2 = |offset[end]|`, the desired elbow is placed by the law
/// of cosines in projection form — `a = (L1² − L2² + d²) / (2d)`
/// along the root→target axis and `h = √(L1² − a²)` to the side —
/// where `d` is the root→target distance clamped to
/// `[|L1 − L2|, L1 + L2]`. An unreachable target therefore leaves
/// the chain fully extended toward it, and a too-close one fully
/// folded. The elbow's side (the pole) is the current elbow
/// direction projected perpendicular to the axis, so the chain
/// keeps bending the way it already bends; if the elbow currently
/// lies on the axis, a fixed perpendicular is used instead. Each
/// joint is then re-aimed by the shortest-arc rotation from its
/// current child direction to the desired one, composed in world
/// space and converted back to a local Euler via
/// [`mat3_to_euler`] (parent world rotations are inverted by
/// transposition, which is exact for rotations).
///
/// A malformed chain — an index out of range, `fore` not a child
/// of `upper`, `end` not a child of `fore`, or a zero-length
/// bone — returns the input pose unchanged.
pub fn solve_two_bone_ik(
    fig: &Figure3,
    pose: &Pose3,
    upper: usize,
    fore: usize,
    end: usize,
    target: [f32; 3],
) -> Pose3 {
    let n = fig.nodes.len();
    let chained = upper < n
        && fore < n
        && end < n
        && fig.nodes[fore].parent == Some(upper)
        && fig.nodes[end].parent == Some(fore);
    if !chained {
        return pose.clone();
    }
    let off1 = fig.nodes[fore].offset;
    let off2 = fig.nodes[end].offset;
    let l1 = v_dot(off1, off1).sqrt();
    let l2 = v_dot(off2, off2).sqrt();
    if l1 < 1.0e-9 || l2 < 1.0e-9 {
        return pose.clone();
    }

    let positions = evaluate3(fig, pose);
    let root_pos = positions[upper];
    let elbow_pos = positions[fore];
    let end_pos = positions[end];

    // --- Desired end and elbow, in world space. ---
    let to_target = v_sub(target, root_pos);
    let dist_raw = v_dot(to_target, to_target).sqrt();
    let axis = if dist_raw > 1.0e-9 {
        v_norm(to_target)
    } else {
        // The target sits on the joint itself: keep the chain's
        // current direction; without one there is nothing to solve.
        let cur = v_sub(end_pos, root_pos);
        if v_dot(cur, cur) < 1.0e-18 {
            return pose.clone();
        }
        v_norm(cur)
    };
    // The projection form needs |L1 − L2| ≤ d ≤ L1 + L2; clamp the
    // reach into that range (fully folded / fully extended at the
    // extremes).
    let d_min = (l1 - l2).abs().max(1.0e-4);
    let d = dist_raw.clamp(d_min, l1 + l2);
    let desired_end = v_add(root_pos, [axis[0] * d, axis[1] * d, axis[2] * d]);
    let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    // Pole: the current elbow's direction, made perpendicular to
    // the target axis, so the elbow keeps its side of the chain.
    let elbow_vec = v_sub(elbow_pos, root_pos);
    let along = v_dot(elbow_vec, axis);
    let mut perp = [
        elbow_vec[0] - axis[0] * along,
        elbow_vec[1] - axis[1] * along,
        elbow_vec[2] - axis[2] * along,
    ];
    if v_dot(perp, perp) < 1.0e-12 {
        // The elbow lies on the target axis: pick a fixed
        // perpendicular instead.
        let helper = if axis[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let hd = v_dot(helper, axis);
        perp = [
            helper[0] - axis[0] * hd,
            helper[1] - axis[1] * hd,
            helper[2] - axis[2] * hd,
        ];
    }
    let pole = v_norm(perp);
    let desired_elbow = v_add(
        root_pos,
        [
            axis[0] * a + pole[0] * h,
            axis[1] * a + pole[1] * h,
            axis[2] * a + pole[2] * h,
        ],
    );

    // --- Re-aim the two joints, world space first. ---
    let world = world_rotations(fig, pose);
    let parent_world = match fig.nodes[upper].parent {
        Some(p) => world[p],
        None => mat3_identity(),
    };

    let mut out = pose.clone();
    let need = upper.max(fore) + 1;
    if out.rotations.len() < need {
        // Materialise implicitly-identity entries so the two
        // solved joints have slots; the values written here are
        // exactly the identity those slots already meant.
        out.rotations.resize(need, [0.0; 3]);
    }

    // Upper joint: swing its child direction onto the desired
    // elbow position.
    let d1 = shortest_arc(
        v_norm(v_sub(elbow_pos, root_pos)),
        v_norm(v_sub(desired_elbow, root_pos)),
    );
    let w_upper = mat3_mul(&d1, &world[upper]);
    out.rotations[upper] =
        mat3_to_euler(&mat3_mul(&mat3_transpose(&parent_world), &w_upper));

    // Fore joint: the upper swing has already carried it (and the
    // end) by `d1`; swing what remains onto the desired end.
    let carried_dir = v_norm(mat3_apply(&d1, v_sub(end_pos, elbow_pos)));
    let d2 = shortest_arc(
        carried_dir,
        v_norm(v_sub(desired_end, desired_elbow)),
    );
    let w_fore = mat3_mul(&d2, &mat3_mul(&d1, &world[fore]));
    out.rotations[fore] =
        mat3_to_euler(&mat3_mul(&mat3_transpose(&w_upper), &w_fore));

    out
}

/// Solve two-bone IK for one of [`humanoid3`]'s arms — the
/// convenience form of [`solve_two_bone_ik`] that resolves the
/// chain indices from the side: left arm upperArm.L = 3,
/// foreArm.L = 4, hand.L = 5; right arm upperArm.R = 6,
/// foreArm.R = 7, hand.R = 8.
///
/// The indices are **humanoid-layout constants**: the figure is
/// expected to use `humanoid3`'s node layout. On any other
/// figure the same fixed indices are used, naming whatever body
/// part happens to sit there; when an index is out of range (or
/// the three nodes do not form a parent chain) the pose is
/// returned unchanged, exactly as [`solve_two_bone_ik`] does.
pub fn solve_arm_ik(
    fig: &Figure3,
    pose: &Pose3,
    side_is_left: bool,
    target: [f32; 3],
) -> Pose3 {
    let (upper, fore, end) = if side_is_left {
        (3, 4, 5)
    } else {
        (6, 7, 8)
    };
    solve_two_bone_ik(fig, pose, upper, fore, end, target)
}

/// Solve two-bone IK for one of [`humanoid3`]'s legs — the
/// convenience form of [`solve_two_bone_ik`] that resolves the
/// chain indices from the side: left leg thigh.L = 9,
/// shin.L = 10, foot.L = 11; right leg thigh.R = 12,
/// shin.R = 13, foot.R = 14.
///
/// The indices are **humanoid-layout constants**, with the same
/// contract as [`solve_arm_ik`]: on a figure with a different
/// layout the fixed indices are used as-is, and out-of-range
/// indices (or a non-chain) return the pose unchanged.
pub fn solve_leg_ik(
    fig: &Figure3,
    pose: &Pose3,
    side_is_left: bool,
    target: [f32; 3],
) -> Pose3 {
    let (upper, fore, end) = if side_is_left {
        (9, 10, 11)
    } else {
        (12, 13, 14)
    };
    solve_two_bone_ik(fig, pose, upper, fore, end, target)
}

#[cfg(test)]
mod ik_tests {
    use super::{
        evaluate3, humanoid3, mat3_from_euler, mat3_identity, mat3_to_euler,
        solve_arm_ik, solve_leg_ik, solve_two_bone_ik, Figure3, Mat3, Node3,
        Pose3,
    };

    fn assert_mat_approx(a: &Mat3, b: &Mat3, tol: f32) {
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    (a[r][c] - b[r][c]).abs() <= tol,
                    "matrix entry [{r}][{c}]: {} vs {}",
                    a[r][c],
                    b[r][c]
                );
            }
        }
    }

    fn assert_euler_approx(a: [f32; 3], b: [f32; 3], tol: f32) {
        for i in 0..3 {
            assert!(
                (a[i] - b[i]).abs() <= tol,
                "euler component {i}: {} vs {}",
                a[i],
                b[i]
            );
        }
    }

    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    #[test]
    fn euler_round_trip_non_singular() {
        let cases: [[f32; 3]; 9] = [
            [0.0, 0.0, 0.0],
            [30.0, 45.0, 60.0],
            [-45.0, -30.0, -120.0],
            [120.0, 10.0, -75.0],
            [-170.0, 80.0, 170.0],
            [15.0, -89.0, -15.0],
            [90.0, 0.0, 90.0],
            [-90.0, -45.0, 33.0],
            [10.0, 89.5, -170.0],
        ];
        for e in cases {
            let back = mat3_to_euler(&mat3_from_euler(e));
            assert_euler_approx(back, e, 0.01);
        }
    }

    #[test]
    fn euler_extraction_of_identity_is_zero() {
        assert_euler_approx(mat3_to_euler(&mat3_identity()), [0.0; 3], 1.0e-6);
    }

    #[test]
    fn euler_gimbal_lock_evaluates_the_same() {
        // y = +90°: only x − z survives; extraction puts it in x
        // with z := 0, and the matrix rebuilds identically.
        let e = [30.0, 90.0, 20.0];
        let back = mat3_to_euler(&mat3_from_euler(e));
        assert_euler_approx(back, [10.0, 90.0, 0.0], 0.01);
        assert_mat_approx(&mat3_from_euler(back), &mat3_from_euler(e), 1.0e-4);
        // y = −90°: only x + z survives.
        let e = [-50.0, -90.0, 65.0];
        let back = mat3_to_euler(&mat3_from_euler(e));
        assert_euler_approx(back, [15.0, -90.0, 0.0], 0.01);
        assert_mat_approx(&mat3_from_euler(back), &mat3_from_euler(e), 1.0e-4);
    }

    // Left arm chain of humanoid3(): upperArm.L = 3 (parent
    // chest), foreArm.L = 4, hand.L = 5; bone lengths 24 and 22,
    // full reach 46.
    const UPPER: usize = 3;
    const FORE: usize = 4;
    const END: usize = 5;

    #[test]
    fn ik_reaches_a_reachable_target() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        pose.rotations[1] = [0.0, 0.0, 20.0]; // chest twist
        pose.rotations[UPPER] = [10.0, -20.0, 30.0];
        let before = evaluate3(&fig, &pose);
        let shoulder = before[UPPER];
        let target = [
            shoulder[0] - 10.0,
            shoulder[1] - 25.0,
            shoulder[2] + 15.0,
        ];
        let solved = solve_two_bone_ik(&fig, &pose, UPPER, FORE, END, target);
        let after = evaluate3(&fig, &solved);
        assert!(
            dist(after[END], target) < 2.0,
            "end at {:?}, target {:?}",
            after[END],
            target
        );
        // Root and shoulder position untouched; every rotation
        // outside the two solved joints is byte-identical.
        assert_eq!(solved.root, pose.root);
        assert!(dist(after[UPPER], shoulder) < 1.0e-4);
        for i in 0..fig.nodes.len() {
            if i != UPPER && i != FORE {
                assert_eq!(
                    solved.rotations[i], pose.rotations[i],
                    "joint {i} changed"
                );
            }
        }
    }

    #[test]
    fn ik_unreachable_target_extends_the_chain() {
        let fig = humanoid3();
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let shoulder = evaluate3(&fig, &pose)[UPPER];
        // Far away, down and toward the viewer: the chain must
        // swing and end fully extended along that direction.
        let dir = [0.0, -200.0, 40.0];
        let target = [
            shoulder[0] + dir[0],
            shoulder[1] + dir[1],
            shoulder[2] + dir[2],
        ];
        let solved = solve_two_bone_ik(&fig, &pose, UPPER, FORE, END, target);
        let after = evaluate3(&fig, &solved);
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let expected = [
            shoulder[0] + 46.0 * dir[0] / len,
            shoulder[1] + 46.0 * dir[1] / len,
            shoulder[2] + 46.0 * dir[2] / len,
        ];
        assert!((dist(after[END], shoulder) - 46.0).abs() < 0.5);
        assert!(
            dist(after[END], expected) < 2.0,
            "end at {:?}, expected {:?}",
            after[END],
            expected
        );
    }

    #[test]
    fn ik_solving_for_the_current_end_is_a_no_op() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [5.0, -3.0, 2.0]);
        pose.rotations[UPPER] = [15.0, 10.0, -20.0];
        pose.rotations[FORE] = [0.0, 0.0, 35.0]; // elbow bent
        let before = evaluate3(&fig, &pose);
        let solved = solve_two_bone_ik(&fig, &pose, UPPER, FORE, END, before[END]);
        let after = evaluate3(&fig, &solved);
        for i in 0..fig.nodes.len() {
            assert!(
                dist(after[i], before[i]) < 0.05,
                "node {i} moved: {:?} -> {:?}",
                before[i],
                after[i]
            );
        }
    }

    #[test]
    fn ik_rejects_a_non_chain() {
        let fig = humanoid3();
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        // hand.L (5) is not upperArm.L's (3) child, so (3, 5, 4)
        // is not a chain: the pose comes back unchanged.
        let solved = solve_two_bone_ik(&fig, &pose, 3, 5, 4, [0.0, 0.0, 0.0]);
        assert_eq!(solved, pose);
    }

    #[test]
    fn arm_ik_left_reaches_a_reachable_target() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        pose.rotations[1] = [0.0, 0.0, 20.0]; // chest twist
        pose.rotations[3] = [10.0, -20.0, 30.0]; // upperArm.L posed
        let before = evaluate3(&fig, &pose);
        let shoulder = before[3];
        // |offset| = sqrt(950) ~ 30.8, inside the 24 + 22 reach.
        let target = [
            shoulder[0] - 10.0,
            shoulder[1] - 25.0,
            shoulder[2] + 15.0,
        ];
        let solved = solve_arm_ik(&fig, &pose, true, target);
        let after = evaluate3(&fig, &solved);
        assert!(
            dist(after[5], target) < 2.0,
            "hand.L at {:?}, target {:?}",
            after[5],
            target
        );
    }

    #[test]
    fn arm_ik_right_reaches_and_leaves_the_left_arm_untouched() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        pose.rotations[3] = [10.0, -20.0, 30.0]; // upperArm.L posed
        pose.rotations[4] = [0.0, 0.0, -45.0]; // foreArm.L bent
        pose.rotations[6] = [-10.0, 20.0, -30.0]; // upperArm.R posed
        let before = evaluate3(&fig, &pose);
        let shoulder = before[6];
        let target = [
            shoulder[0] + 10.0,
            shoulder[1] - 25.0,
            shoulder[2] + 15.0,
        ];
        let solved = solve_arm_ik(&fig, &pose, false, target);
        let after = evaluate3(&fig, &solved);
        assert!(
            dist(after[8], target) < 2.0,
            "hand.R at {:?}, target {:?}",
            after[8],
            target
        );
        // The solver writes only the right chain's slots (6, 7):
        // the left arm's rotations and evaluated hand position
        // are exactly as they were.
        assert_eq!(solved.rotations[3], pose.rotations[3]);
        assert_eq!(solved.rotations[4], pose.rotations[4]);
        assert!(
            dist(after[5], before[5]) < 1.0e-4,
            "hand.L moved: {:?} -> {:?}",
            before[5],
            after[5]
        );
    }

    #[test]
    fn leg_ik_left_moves_the_foot_below_the_hip() {
        let fig = humanoid3();
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let before = evaluate3(&fig, &pose);
        let hip = before[9]; // thigh.L
        // Below the hip and stepped forward/out:
        // |offset| = sqrt(3114) ~ 55.8, inside the 34 + 32 reach.
        let target = [hip[0] + 5.0, hip[1] - 55.0, hip[2] + 8.0];
        assert!(target[1] < hip[1], "target is below the hip");
        let solved = solve_leg_ik(&fig, &pose, true, target);
        let after = evaluate3(&fig, &solved);
        assert!(
            dist(after[11], target) < 2.0,
            "foot.L at {:?}, target {:?}",
            after[11],
            target
        );
        // The right leg was not part of the solve.
        assert!(
            dist(after[14], before[14]) < 1.0e-4,
            "foot.R moved: {:?} -> {:?}",
            before[14],
            after[14]
        );
    }

    #[test]
    fn arm_and_leg_ik_on_a_small_figure_return_the_pose_unchanged() {
        // A 3-node figure: every humanoid arm/leg index is out of
        // range, so all four conveniences are no-ops.
        let fig = Figure3 {
            name: "tiny".into(),
            color: 0,
            nodes: vec![
                Node3 { name: "root".into(), parent: None, offset: [0.0; 3], head: false },
                Node3 { name: "mid".into(), parent: Some(0), offset: [10.0, 0.0, 0.0], head: false },
                Node3 { name: "tip".into(), parent: Some(1), offset: [10.0, 0.0, 0.0], head: false },
            ],
        };
        let mut pose = Pose3::rest(&fig, [1.0, 2.0, 3.0]);
        pose.rotations[1] = [0.0, 0.0, 45.0];
        let target = [5.0, 5.0, 5.0];
        assert_eq!(solve_arm_ik(&fig, &pose, true, target), pose);
        assert_eq!(solve_arm_ik(&fig, &pose, false, target), pose);
        assert_eq!(solve_leg_ik(&fig, &pose, true, target), pose);
        assert_eq!(solve_leg_ik(&fig, &pose, false, target), pose);
    }
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

/// Per-segment depth shade for faking depth in a 2D renderer: one
/// value per segment, in exactly the order [`segments2d`] produces
/// (node order, one per parent→node pair). The figure is evaluated
/// and projected internally — view depth does not depend on the
/// stage size, so the projection uses [`DEFAULT_STAGE`].
///
/// **Normalisation rule:** each segment's depth is the mean of its
/// two projected endpoints' `Proj.depth` (so a segment behind the
/// camera carries the clamped [`BEHIND_DEPTH`] into its mean). The
/// means are then normalised against the figure's *own* minimum and
/// maximum mean depth: the nearest segment (smallest mean) shades
/// exactly 1.0 and the farthest shades exactly 0.35, with a linear
/// ramp between them — `shade = 1.0 − 0.65 · (d − min) / (max − min)`,
/// clamped to `0.35..=1.0`. Because the ramp is relative to the
/// figure itself, a figure whose segments all sit at (nearly) the
/// same depth — e.g. a planar figure facing the camera — has a
/// zero (or near-zero) depth range; that case is guarded and every
/// segment shades 1.0 rather than dividing by ~0. A figure with no
/// segments yields an empty vector. Pure data for renderers: this
/// function changes no rendering itself.
pub fn segment_shades(figure: &Figure3, pose: &Pose3, cam: &Camera3) -> Vec<f32> {
    /// Depth ranges at or below this count as "all segments at one
    /// depth" (the planar/uniform case).
    const ZERO_RANGE_EPS: f32 = 1e-4;
    /// The farthest segment's shade; nothing shades darker.
    const MIN_SHADE: f32 = 0.35;

    let world = evaluate3(figure, pose);
    let proj = project(&world, cam, DEFAULT_STAGE);
    let mut means: Vec<f32> = Vec::new();
    for (i, node) in figure.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            if let (Some(&a), Some(&b)) = (proj.get(p), proj.get(i)) {
                means.push((a.depth + b.depth) / 2.0);
            }
        }
    }
    if means.is_empty() {
        return means;
    }
    let min = means.iter().copied().fold(f32::INFINITY, f32::min);
    let max = means.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    if range <= ZERO_RANGE_EPS {
        return vec![1.0; means.len()];
    }
    means
        .iter()
        .map(|&d| {
            let t = (d - min) / range; // 0 = nearest, 1 = farthest
            (1.0 - t * (1.0 - MIN_SHADE)).clamp(MIN_SHADE, 1.0)
        })
        .collect()
}

// ---------------------------------------------------------------------
// Baking a posed 3D figure into 2D figure-definition data.
// ---------------------------------------------------------------------

/// Per-node data for baking a posed 3D figure into a 2D figure
/// definition (consumed by `EditorCore::fig3_bake` in the editor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeNode {
    /// Projected stage position of this node.
    pub xy: V2,
    /// Projected distance to the parent node (0 for the root).
    pub length: f32,
    /// Absolute direction of the segment parent -> this node, in the
    /// 2D model's angle convention ([`crate::model::angle_of`]);
    /// 0 for the root, whose children measure from absolute 0.
    pub abs_angle: f32,
    /// Segment thickness in stage units: [`SEGMENT_THICKNESS`] scaled
    /// by the mean of the two endpoints' projection scales.
    pub thickness: f32,
}

/// Evaluate and project a figure, reduced to one [`BakeNode`] per
/// node, in node order. A 2D definition built with these lengths and
/// with relative angles `norm_deg(abs_angle - parent.abs_angle)`
/// evaluates back to these projected points (up to float error),
/// because the 2D evaluator telescopes relative angles the same way
/// the other importers rely on.
pub fn bake_nodes(fig: &Figure3, pose: &Pose3, cam: &Camera3, stage: [f32; 2]) -> Vec<BakeNode> {
    let world = evaluate3(fig, pose);
    let proj = project(&world, cam, stage);
    fig.nodes
        .iter()
        .enumerate()
        .map(|(i, node)| match node.parent {
            None => BakeNode {
                xy: proj[i].xy,
                length: 0.0,
                abs_angle: 0.0,
                thickness: SEGMENT_THICKNESS * proj[i].scale,
            },
            Some(p) => {
                let d = [
                    proj[i].xy[0] - proj[p].xy[0],
                    proj[i].xy[1] - proj[p].xy[1],
                ];
                BakeNode {
                    xy: proj[i].xy,
                    length: (d[0] * d[0] + d[1] * d[1]).sqrt(),
                    abs_angle: crate::model::angle_of(d),
                    thickness: SEGMENT_THICKNESS * (proj[i].scale + proj[p].scale) / 2.0,
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------
// Pose interpolation, animated baking, and the 3D figure library.
// ---------------------------------------------------------------------

/// The stage size [`bake_range`] projects onto: the port's standard
/// project stage (see `Project::demo` in model.rs). Callers working
/// on a different stage use [`bake_range_staged`].
pub const DEFAULT_STAGE: [f32; 2] = [960.0, 720.0];

/// Interpolate two poses of the same figure. The root lerps
/// component-wise; each Euler component takes the **shortest path**
/// around the circle (per [`crate::model::norm_deg`]): interpolating
/// 350° → 10° passes through 0°, not backwards through 180°. A
/// rotation list shorter than the other is padded with identity
/// rotations, matching how [`evaluate3`] reads poses. Components of
/// the result are normalised into (−180°, 180°], so lerping between
/// two stored-but-noncanonical angles (e.g. 350°) yields the
/// equivalent canonical angle — the evaluated figure is identical.
pub fn lerp_pose(a: &Pose3, b: &Pose3, t: f32) -> Pose3 {
    let n = a.rotations.len().max(b.rotations.len());
    let rotations = (0..n)
        .map(|i| {
            let ra = a.rotations.get(i).copied().unwrap_or([0.0; 3]);
            let rb = b.rotations.get(i).copied().unwrap_or([0.0; 3]);
            let mut out = [0.0; 3];
            for c in 0..3 {
                let d = crate::model::norm_deg(rb[c] - ra[c]);
                out[c] = crate::model::norm_deg(ra[c] + d * t);
            }
            out
        })
        .collect();
    Pose3 {
        root: [
            a.root[0] + (b.root[0] - a.root[0]) * t,
            a.root[1] + (b.root[1] - a.root[1]) * t,
            a.root[2] + (b.root[2] - a.root[2]) * t,
        ],
        rotations,
    }
}

/// How close two offsets must be to count as mirror images when
/// [`mirror_pose`] pairs up nodes (see [`mirror_partners`]).
const MIRROR_OFFSET_EPS: f32 = 1e-4;

fn v_approx(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
}

/// The mirror partner of every node across the YZ plane: node `j`
/// partners node `i` when `j`'s offset is `i`'s offset with X
/// negated **and** `j`'s parent is the partner of `i`'s parent, so
/// whole subtrees swap sides consistently. In [`humanoid3`] the
/// `.L` chains trade places with the `.R` chains while the spine —
/// whose offsets all have X = 0 under self-partnered parents —
/// partners itself. Nodes are matched greedily in node order
/// (parents precede children, per the module rules); a node with no
/// geometric partner partners itself.
fn mirror_partners(fig: &Figure3) -> Vec<usize> {
    let n = fig.nodes.len();
    let mut partner: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        if partner[i].is_some() {
            continue;
        }
        let want = [
            -fig.nodes[i].offset[0],
            fig.nodes[i].offset[1],
            fig.nodes[i].offset[2],
        ];
        let mut found = None;
        for j in i..n {
            if partner[j].is_some() {
                continue;
            }
            let parents_match = match (fig.nodes[i].parent, fig.nodes[j].parent) {
                (None, None) => true,
                (Some(pi), Some(pj)) => partner[pi] == Some(pj),
                _ => false,
            };
            if parents_match && v_approx(fig.nodes[j].offset, want, MIRROR_OFFSET_EPS) {
                found = Some(j);
                break;
            }
        }
        let j = found.unwrap_or(i);
        partner[i] = Some(j);
        partner[j] = Some(i);
    }
    partner
        .into_iter()
        .enumerate()
        .map(|(i, p)| p.unwrap_or(i))
        .collect()
}

/// Mirror a pose across the YZ plane — the figure's left and right
/// swap sides. The root's X is negated, and every joint rotation is
/// replaced by its mirror image, with left/right partner joints
/// exchanging rotations (see [`mirror_partners`]).
///
/// **Why both steps are needed.** Write the mirror as the matrix
/// `M = diag(−1, 1, 1)`. A rotation's mirror image is the
/// conjugation `M·R·M`; for this module's Euler order
/// (`R = Rz·Ry·Rx`) that works out to negating the Y and Z angles
/// only — `M` commutes with `Rx` (the YZ plane is `Rx`'s own
/// plane) and flips the sense of `Ry` and `Rz` — so each joint
/// maps `(x, y, z) → (x, −y, −z)`. But conjugating the rotations
/// alone does **not** mirror the evaluated figure: a node's world
/// position is built from its ancestors' rotations applied to its
/// **offset**, and offsets carry X too (the humanoid's left forearm
/// hangs at X = −24 in rest, the right at +24). With rotations
/// also swapped between partner nodes, induction over the tree
/// gives the exact result, for every node `i`:
/// `evaluate3(fig, &mirror_pose(fig, pose))[partner(i)]` is
/// `evaluate3(fig, pose)[i]` with X negated — because
/// `W'·offset' = (M·W·M)·(M·offset) = M·(W·offset)` along the
/// whole chain. Under [`default_camera`] (yaw/pitch 0) the
/// projected points are therefore the originals flipped about the
/// stage's vertical centre line; the tests below pin both facts
/// against [`evaluate3`] and [`project`].
///
/// Nodes with no geometric mirror partner keep their own
/// (conjugated) rotation, so below such a node an asymmetric
/// figure's mirror is best-effort rather than exact. Angles are
/// negated, never normalised, so mirroring is an exact involution:
/// `mirror_pose(fig, &mirror_pose(fig, pose)) == pose`.
pub fn mirror_pose(figure: &Figure3, pose: &Pose3) -> Pose3 {
    let partners = mirror_partners(figure);
    let n = pose.rotations.len().max(figure.nodes.len());
    let mut rotations = vec![[0.0; 3]; n];
    for (i, &j) in partners.iter().enumerate() {
        let r = pose.rotations.get(i).copied().unwrap_or([0.0; 3]);
        rotations[j] = [r[0], -r[1], -r[2]];
    }
    // Entries beyond the node list carry no partner meaning;
    // conjugate them in place so a second mirror still restores
    // them exactly.
    for (i, r) in pose.rotations.iter().enumerate().skip(figure.nodes.len()) {
        rotations[i] = [r[0], -r[1], -r[2]];
    }
    Pose3 {
        root: [-pose.root[0], pose.root[1], pose.root[2]],
        rotations,
    }
}

/// Bake an animated range: `frames` steps interpolating `from` →
/// `to` (see [`lerp_pose`]), each baked with [`bake_nodes`] on the
/// given stage. Step `i` uses `t = i / (frames − 1)`, so the first
/// step is exactly `from` and the last exactly `to`. `frames == 0`
/// yields no steps; `frames == 1` yields the single `from` bake.
pub fn bake_range_staged(
    fig: &Figure3,
    from: &Pose3,
    to: &Pose3,
    cam: &Camera3,
    stage: [f32; 2],
    frames: usize,
) -> Vec<Vec<BakeNode>> {
    (0..frames)
        .map(|i| {
            let t = if frames <= 1 {
                0.0
            } else {
                i as f32 / (frames - 1) as f32
            };
            bake_nodes(fig, &lerp_pose(from, to, t), cam, stage)
        })
        .collect()
}

/// [`bake_range_staged`] on [`DEFAULT_STAGE`].
pub fn bake_range(
    fig: &Figure3,
    from: &Pose3,
    to: &Pose3,
    cam: &Camera3,
    frames: usize,
) -> Vec<Vec<BakeNode>> {
    bake_range_staged(fig, from, to, cam, DEFAULT_STAGE, frames)
}

/// File name a library figure called `name` is stored under:
/// `fig3d-<slug>.json`, where the slug is the lowercased name with
/// every run of non-alphanumeric characters collapsed to one `-`
/// (and no leading/trailing `-`); an empty slug becomes `figure`.
pub fn library_file_name(name: &str) -> String {
    format!("fig3d-{}.json", library_slug(name))
}

/// The slug a library display `name` is stored under: the lowercased
/// name with every run of non-alphanumeric characters collapsed to
/// one `-` (and no leading/trailing `-`); an empty slug becomes
/// `figure`. Shared by the figure library ([`library_file_name`])
/// and the pose library ([`pose_file_name`]) so both sanitise names
/// identically.
fn library_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.extend(ch.to_lowercase());
        } else {
            dash = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("figure");
    }
    slug
}

/// One library file's contents: a figure plus the pose it was saved
/// in, under the name the user saved it by.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LibraryEntry {
    name: String,
    figure: Figure3,
    pose: Pose3,
}

/// Save `fig` in `pose` into the 3D library directory `dir` (created
/// if missing) under `name`, returning the file's path. The file is
/// `<dir>/fig3d-<slug>.json` (see [`library_file_name`]) holding
/// `{name, figure, pose}` as JSON.
pub fn save_library_figure(
    dir: &Path,
    name: &str,
    fig: &Figure3,
    pose: &Pose3,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(library_file_name(name));
    let entry = LibraryEntry {
        name: name.to_string(),
        figure: fig.clone(),
        pose: pose.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Load a library file written by [`save_library_figure`], returning
/// the figure and its saved pose. `None` on any failure (missing
/// file, invalid JSON, wrong shape) — a corrupt library entry is
/// simply unusable, never an error worth aborting over.
pub fn load_library_figure(path: &Path) -> Option<(Figure3, Pose3)> {
    let text = std::fs::read_to_string(path).ok()?;
    let entry: LibraryEntry = serde_json::from_str(&text).ok()?;
    Some((entry.figure, entry.pose))
}

/// The display names of every figure saved in library directory
/// `dir`, sorted. Files that are not readable library entries are
/// skipped; a missing directory yields an empty list.
pub fn scan_library(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_library_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("fig3d-") && n.ends_with(".json"))
                .unwrap_or(false);
            if is_library_file {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<LibraryEntry>(&text) {
                        names.push(parsed.name);
                    }
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// File name a library pose called `name` is stored under:
/// `<slug>.json`, where the slug comes from [`library_slug`] — the
/// same sanitising the figure library applies (see
/// [`library_file_name`]), minus its `fig3d-` prefix.
fn pose_file_name(name: &str) -> String {
    format!("{}.json", library_slug(name))
}

/// One pose library file's contents: a pose under the name the user
/// saved it by (mirrors [`LibraryEntry`] for the figure library).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PoseLibraryEntry {
    name: String,
    pose: Pose3,
}

/// The directory the 3D pose library lives in: `./library3d/poses`,
/// beside the figure library's own files in `./library3d`.
fn pose_library_dir() -> PathBuf {
    PathBuf::from("library3d").join("poses")
}

/// Save `pose` into pose library directory `dir` (created if
/// missing) under `name`, returning the file's path. The file is
/// `<dir>/<slug>.json` (see [`pose_file_name`]) holding `{name,
/// pose}` as JSON.
fn save_pose_library_in(
    dir: &Path,
    pose: &Pose3,
    name: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(pose_file_name(name));
    let entry = PoseLibraryEntry {
        name: name.to_string(),
        pose: pose.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Save `pose` into the 3D pose library (`./library3d/poses`) under
/// `name`, returning the file's path. See [`save_pose_library_in`].
pub fn save_pose_library(pose: &Pose3, name: &str) -> std::io::Result<PathBuf> {
    save_pose_library_in(&pose_library_dir(), pose, name)
}

/// Load the pose saved under `name` from pose library directory
/// `dir`. An error is returned when no readable, well-formed entry
/// exists for that name (missing file, invalid JSON, wrong shape).
fn load_pose_library_in(dir: &Path, name: &str) -> std::io::Result<Pose3> {
    let path = dir.join(pose_file_name(name));
    let text = std::fs::read_to_string(&path)?;
    let entry: PoseLibraryEntry = serde_json::from_str(&text).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    Ok(entry.pose)
}

/// Load the pose saved under `name` from the 3D pose library
/// (`./library3d/poses`). See [`load_pose_library_in`].
pub fn load_pose_library(name: &str) -> std::io::Result<Pose3> {
    load_pose_library_in(&pose_library_dir(), name)
}

/// The display names of every pose saved in pose library directory
/// `dir`, sorted. Files that are not readable pose entries are
/// skipped; a missing directory yields an empty list.
fn scan_pose_library_in(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_pose_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".json"))
                .unwrap_or(false);
            if is_pose_file {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<PoseLibraryEntry>(&text) {
                        names.push(parsed.name);
                    }
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The display names of every pose saved in the 3D pose library
/// (`./library3d/poses`), sorted. See [`scan_pose_library_in`].
pub fn scan_pose_library() -> Vec<String> {
    scan_pose_library_in(&pose_library_dir())
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
    fn grid_line_points_count_endpoints_and_symmetry() {
        let segs = grid_line_points(5, 10.0);
        // 2 * (2 * extent + 1) segments: 11 lines each way.
        assert_eq!(segs.len(), 22);
        for (a, b) in &segs {
            // Every endpoint lies on the ground plane.
            assert_eq!(a[1], 0.0);
            assert_eq!(b[1], 0.0);
            // Symmetric reach: 5 * 10 = 50 in the running axis.
            let run_a = if a[0] == b[0] { a[2] } else { a[0] };
            let run_b = if a[0] == b[0] { b[2] } else { b[0] };
            assert_eq!(run_a, -50.0);
            assert_eq!(run_b, 50.0);
        }
        // Documented order: the x-constant (along-z) lines come
        // first, ascending; the first is x = -50, the last of
        // that block is x = +50, then the z-constant block.
        assert_eq!(segs[0], ([-50.0, 0.0, -50.0], [-50.0, 0.0, 50.0]));
        assert_eq!(segs[10], ([50.0, 0.0, -50.0], [50.0, 0.0, 50.0]));
        assert_eq!(segs[11], ([-50.0, 0.0, -50.0], [50.0, 0.0, -50.0]));
        assert_eq!(segs[21], ([-50.0, 0.0, 50.0], [50.0, 0.0, 50.0]));
    }

    #[test]
    fn grid_line_points_degenerate_inputs_are_empty() {
        assert!(grid_line_points(0, 10.0).is_empty());
        assert!(grid_line_points(-3, 10.0).is_empty());
        assert!(grid_line_points(5, 0.0).is_empty());
        assert!(grid_line_points(5, -10.0).is_empty());
        assert!(grid_line_points(5, f32::NAN).is_empty());
        assert!(grid_line_points(5, f32::INFINITY).is_empty());
    }

    #[test]
    fn preview_camera_is_finite_and_frames_the_rest_figure() {
        let cam = preview_camera();
        assert!(cam.yaw.is_finite() && cam.pitch.is_finite());
        assert!(cam.fov_deg.is_finite() && cam.fov_deg > 0.0);
        assert!(cam.distance.is_finite() && cam.distance > 0.0);
        assert!(cam.target.iter().all(|v| v.is_finite()));
        // The documented contract the preview relies on: the
        // projected rest pose lands inside a 308x220 viewport.
        let fig = humanoid3();
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let pts = evaluate3(&fig, &pose);
        let projected = project(&pts, &cam, [308.0, 220.0]);
        for p in &projected {
            assert!(p.xy[0] >= 0.0 && p.xy[0] <= 308.0, "x out: {:?}", p.xy);
            assert!(p.xy[1] >= 0.0 && p.xy[1] <= 220.0, "y out: {:?}", p.xy);
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
    fn segment_shades_far_segment_in_minus_z_scores_lower() {
        // One segment in the view plane, one pushed far in -Z (away
        // from the camera at +Z): the far segment's mean depth is
        // (400 + 600) / 2, the near one's is 400.
        let fig = Figure3 {
            name: "depth".into(),
            color: 0,
            nodes: vec![
                Node3 { name: "root".into(), parent: None, offset: [0.0; 3], head: false },
                Node3 { name: "near".into(), parent: Some(0), offset: [30.0, 0.0, 0.0], head: false },
                Node3 { name: "far".into(), parent: Some(0), offset: [0.0, 0.0, -200.0], head: false },
            ],
        };
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let shades = segment_shades(&fig, &pose, &cam());
        assert_eq!(shades.len(), 2, "one shade per segment, segments2d order");
        // Nearest segment is exactly 1.0; the farthest hits the floor.
        assert_eq!(shades[0], 1.0);
        assert!(shades[1] < shades[0], "far {} vs near {}", shades[1], shades[0]);
        assert!((shades[1] - 0.35).abs() < 1e-4, "far shade {}", shades[1]);
    }

    #[test]
    fn segment_shades_planar_figure_is_uniform() {
        // The rest humanoid is planar (every Z offset 0) facing the
        // camera: all endpoints share one depth, the range is zero,
        // and every segment must shade ~1.0 (no divide-by-zero ramp).
        let fig = humanoid3();
        let shades = segment_shades(&fig, &Pose3::rest(&fig, [0.0, 0.0, 0.0]), &cam());
        assert_eq!(shades.len(), fig.nodes.len() - 1);
        assert!(
            shades.iter().all(|&s| (s - 1.0).abs() < 1e-4),
            "planar shades {shades:?}"
        );
    }

    #[test]
    fn segment_shades_empty_figure_is_empty() {
        let fig = Figure3 { name: "empty".into(), color: 0, nodes: vec![] };
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        assert!(segment_shades(&fig, &pose, &cam()).is_empty());
        // A lone root has no segments either.
        let solo = Figure3 {
            name: "solo".into(),
            color: 0,
            nodes: vec![Node3 {
                name: "root".into(),
                parent: None,
                offset: [0.0; 3],
                head: false,
            }],
        };
        assert!(segment_shades(&solo, &Pose3::rest(&solo, [0.0; 3]), &cam()).is_empty());
    }

    #[test]
    fn segment_shades_stay_within_clamp_bounds() {
        // A depth-rich pose (chest pitched, one arm swung toward the
        // camera, root off-axis under an orbited camera): every
        // shade lands in 0.35..=1.0, the count matches segments2d,
        // and at least one segment is shaded below 1.0.
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [20.0, 0.0, -30.0]);
        pose.rotations[1] = [35.0, 0.0, 0.0]; // chest tips forward
        pose.rotations[3] = [0.0, 80.0, 0.0]; // upperArm.L swings in Z
        pose.rotations[9] = [0.0, 0.0, -25.0]; // thigh.L
        let cam = Camera3 { yaw: 30.0, pitch: 15.0, ..cam() };
        let shades = segment_shades(&fig, &pose, &cam);
        let world = evaluate3(&fig, &pose);
        let proj = project(&world, &cam, DEFAULT_STAGE);
        assert_eq!(shades.len(), segments2d(&fig, &proj).len());
        assert!(
            shades.iter().all(|&s| (0.35..=1.0).contains(&s)),
            "shades out of bounds {shades:?}"
        );
        assert!(shades.iter().any(|&s| s < 1.0), "expected some depth shading {shades:?}");
        assert!(shades.iter().any(|&s| s == 1.0), "nearest segment shades 1.0 {shades:?}");
    }

    #[test]
    fn default_camera_matches_documented_orbit() {
        let c = default_camera();
        assert_eq!((c.yaw, c.pitch, c.distance, c.fov_deg), (0.0, 0.0, 400.0, 40.0));
        assert_eq!(c.target, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn bake_nodes_match_projection_for_rest_pose() {
        let fig = humanoid3();
        let stage = [960.0, 720.0];
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let cam = default_camera();
        let baked = bake_nodes(&fig, &pose, &cam, stage);
        let world = evaluate3(&fig, &pose);
        let proj = project(&world, &cam, stage);
        assert_eq!(baked.len(), fig.nodes.len());
        for (b, p) in baked.iter().zip(&proj) {
            assert_eq!(b.xy, p.xy);
        }
        // Chest is 30 world units straight up from the hips: its
        // projected length is 30 * scale and its direction is
        // stage-up, which is angle 180 in the 2D convention.
        let scale = proj[0].scale;
        assert!(
            (baked[1].length - 30.0 * scale).abs() < 0.05,
            "chest length {}",
            baked[1].length
        );
        assert!(
            (baked[1].abs_angle - 180.0).abs() < 0.05,
            "chest angle {}",
            baked[1].abs_angle
        );
        assert_eq!(baked[0].length, 0.0);
        assert_eq!(baked[0].abs_angle, 0.0);
        assert!(baked[1].thickness > 0.0);
    }

    #[test]
    fn short_rotation_list_defaults_to_identity() {
        let fig = humanoid3();
        let pose = Pose3 { root: [1.0, 2.0, 3.0], rotations: vec![] };
        let pos = evaluate3(&fig, &pose);
        assert_eq!(pos[2], [1.0, 60.0, 3.0]);
    }

    #[test]
    fn lerp_pose_takes_the_shortest_path_around_the_circle() {
        let fig = humanoid3();
        let mut a = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let mut b = Pose3::rest(&fig, [100.0, -40.0, 20.0]);
        a.rotations[3] = [350.0, 0.0, 0.0];
        b.rotations[3] = [10.0, 0.0, 0.0];
        // The midpoint of 350 -> 10 is 0 (through 360), never 180.
        let mid = lerp_pose(&a, &b, 0.5);
        assert!(mid.rotations[3][0].abs() < 0.01, "mid {:?}", mid.rotations[3]);
        // A quarter of the way is 355, i.e. -5 normalised.
        let quarter = lerp_pose(&a, &b, 0.25);
        assert!(
            (quarter.rotations[3][0] - (-5.0)).abs() < 0.01,
            "quarter {:?}",
            quarter.rotations[3]
        );
        // Endpoints: values equivalent to a and b (350 normalises to
        // -10, which evaluates identically).
        let start = lerp_pose(&a, &b, 0.0);
        assert!((start.rotations[3][0] - (-10.0)).abs() < 0.01);
        assert_eq!(start.root, [0.0, 0.0, 0.0]);
        let end = lerp_pose(&a, &b, 1.0);
        assert!((end.rotations[3][0] - 10.0).abs() < 0.01);
        assert_eq!(end.root, [100.0, -40.0, 20.0]);
        // The root lerps linearly.
        assert_eq!(mid.root, [50.0, -20.0, 10.0]);
    }

    #[test]
    fn lerp_pose_pads_short_rotation_lists() {
        let fig = humanoid3();
        let a = Pose3 { root: [0.0; 3], rotations: vec![] };
        let mut b = Pose3::rest(&fig, [0.0; 3]);
        b.rotations[1] = [0.0, 0.0, 40.0];
        let mid = lerp_pose(&a, &b, 0.5);
        assert_eq!(mid.rotations.len(), fig.nodes.len());
        assert!((mid.rotations[1][2] - 20.0).abs() < 0.01);
    }

    fn bake_nodes_close(a: &[BakeNode], b: &[BakeNode], eps: f32) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| {
                (x.xy[0] - y.xy[0]).abs() < eps
                    && (x.xy[1] - y.xy[1]).abs() < eps
                    && (x.length - y.length).abs() < eps
                    && (x.abs_angle - y.abs_angle).abs() < 0.1
            })
    }

    #[test]
    fn bake_range_endpoints_equal_single_bakes() {
        let fig = humanoid3();
        let cam = default_camera();
        let from = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let mut to = Pose3::rest(&fig, [30.0, 0.0, 0.0]);
        to.rotations[3] = [0.0, 0.0, 60.0]; // upperArm.L swings
        to.rotations[1] = [20.0, 0.0, 0.0]; // chest tips forward
        let range = bake_range(&fig, &from, &to, &cam, 5);
        assert_eq!(range.len(), 5);
        let first = bake_nodes(&fig, &from, &cam, DEFAULT_STAGE);
        let last = bake_nodes(&fig, &to, &cam, DEFAULT_STAGE);
        assert!(bake_nodes_close(&range[0], &first, 0.01), "first step = from");
        assert!(bake_nodes_close(&range[4], &last, 0.01), "last step = to");
        // A middle step is genuinely between: the left hand is near
        // neither endpoint's position.
        let hand = 5;
        let mid = range[2][hand].xy;
        assert!(dist2(mid, first[hand].xy) > 1.0, "mid {mid:?} vs from {:?}", first[hand].xy);
        assert!(dist2(mid, last[hand].xy) > 1.0, "mid {mid:?} vs to {:?}", last[hand].xy);
        // Degenerate counts stay safe.
        assert!(bake_range(&fig, &from, &to, &cam, 0).is_empty());
        assert_eq!(bake_range(&fig, &from, &to, &cam, 1).len(), 1);
        assert!(bake_nodes_close(
            &bake_range(&fig, &from, &to, &cam, 1)[0],
            &first,
            0.01
        ));
    }

    fn dist2(a: V2, b: V2) -> f32 {
        let d = [a[0] - b[0], a[1] - b[1]];
        (d[0] * d[0] + d[1] * d[1]).sqrt()
    }

    #[test]
    fn library_save_load_scan_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "stykr-fig3d-library-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // Scanning a directory that does not exist yet is empty.
        assert!(scan_library(&dir).is_empty());
        assert!(load_library_figure(&dir.join("fig3d-nope.json")).is_none());

        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [5.0, 6.0, 7.0]);
        pose.rotations[2] = [0.0, 30.0, 0.0];
        let path = save_library_figure(&dir, "Humanoid3", &fig, &pose).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "fig3d-humanoid3.json"
        );
        // A second, messier name exercises the slug rules.
        let path2 = save_library_figure(&dir, "My Bot!!", &fig, &pose).unwrap();
        assert_eq!(
            path2.file_name().unwrap().to_str().unwrap(),
            "fig3d-my-bot.json"
        );
        assert_eq!(library_file_name("  --  "), "fig3d-figure.json");

        assert_eq!(scan_library(&dir), vec!["Humanoid3", "My Bot!!"]);
        let (loaded_fig, loaded_pose) = load_library_figure(&path).unwrap();
        assert_eq!(loaded_fig, fig);
        assert_eq!(loaded_pose, pose);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod mirror_tests {
    use super::{
        default_camera, evaluate3, humanoid3, mirror_partners, mirror_pose, project, Figure3,
        Node3, Pose3,
    };

    #[test]
    fn mirror_partners_humanoid_swaps_left_right_chains() {
        // Spine (0, 1, 2) partners itself; the .L chains (3-5, 9-11)
        // trade places with the .R chains (6-8, 12-14), subtree by
        // subtree: shin.L (10) partners shin.R (13), not itself,
        // because its parent thigh.L partners thigh.R.
        let fig = humanoid3();
        assert_eq!(
            mirror_partners(&fig),
            vec![0, 1, 2, 6, 7, 8, 3, 4, 5, 12, 13, 14, 9, 10, 11]
        );
    }

    #[test]
    fn mirror_partners_asymmetric_node_falls_back_to_itself() {
        // The child hangs at +X with no -X counterpart anywhere:
        // it partners itself (documented best-effort fallback).
        let fig = Figure3 {
            name: "lopsided".into(),
            color: 0,
            nodes: vec![
                Node3 { name: "root".into(), parent: None, offset: [0.0; 3], head: false },
                Node3 { name: "only".into(), parent: Some(0), offset: [10.0, 5.0, 0.0], head: false },
            ],
        };
        assert_eq!(mirror_partners(&fig), vec![0, 1]);
        // Its rotation is still conjugated in place, and the root
        // X still mirrors.
        let mut pose = Pose3::rest(&fig, [7.0, 0.0, 0.0]);
        pose.rotations[1] = [10.0, 20.0, 30.0];
        let m = mirror_pose(&fig, &pose);
        assert_eq!(m.root, [-7.0, 0.0, 0.0]);
        assert_eq!(m.rotations[1], [10.0, -20.0, -30.0]);
    }

    #[test]
    fn mirror_pose_swaps_and_conjugates_rotations() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [12.0, 0.0, 0.0]);
        pose.rotations[1] = [5.0, -15.0, 25.0]; // chest: self-partnered
        pose.rotations[3] = [10.0, 20.0, 30.0]; // upperArm.L
        let m = mirror_pose(&fig, &pose);
        assert_eq!(m.root, [-12.0, 0.0, 0.0]);
        // The chest keeps its slot but its Y/Z angles negate.
        assert_eq!(m.rotations[1], [5.0, 15.0, -25.0]);
        // upperArm.L's rotation lands, conjugated, on upperArm.R;
        // upperArm.L receives upperArm.R's rest rotation (identity).
        assert_eq!(m.rotations[6], [10.0, -20.0, -30.0]);
        assert_eq!(m.rotations[3], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn mirror_pose_twice_restores_the_pose_exactly() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [30.0, 10.0, -20.0]);
        pose.rotations[1] = [15.0, -40.0, 120.0];
        pose.rotations[3] = [0.0, 25.0, -60.0];
        pose.rotations[6] = [-80.0, 10.0, 5.0];
        pose.rotations[13] = [45.0, 0.0, 0.0];
        let twice = mirror_pose(&fig, &mirror_pose(&fig, &pose));
        assert_eq!(twice, pose);
    }

    #[test]
    fn mirrored_pose_projects_to_x_flipped_original() {
        // The brief's verification: under the default camera, the
        // mirrored pose's projection is the original projection
        // flipped about the stage's vertical centre line — with
        // each node compared against its mirror partner's point.
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [25.0, 5.0, -10.0]);
        pose.rotations[1] = [10.0, 20.0, 5.0]; // chest twists
        pose.rotations[2] = [0.0, -15.0, 0.0]; // head turns
        pose.rotations[3] = [0.0, 0.0, -60.0]; // upperArm.L raises
        pose.rotations[7] = [25.0, 0.0, 0.0]; // foreArm.R bends
        pose.rotations[10] = [30.0, 0.0, 0.0]; // shin.L bends
        let stage = [960.0, 720.0];
        let cam = default_camera();
        let orig = project(&evaluate3(&fig, &pose), &cam, stage);
        let mir = project(&evaluate3(&fig, &mirror_pose(&fig, &pose)), &cam, stage);
        let partners = mirror_partners(&fig);
        for i in 0..fig.nodes.len() {
            let o = orig[partners[i]];
            let m = mir[i];
            assert!(
                (m.xy[0] - (stage[0] - o.xy[0])).abs() < 0.01,
                "node {i} screen x: {} vs flipped {}",
                m.xy[0],
                stage[0] - o.xy[0]
            );
            assert!(
                (m.xy[1] - o.xy[1]).abs() < 0.01,
                "node {i} screen y: {} vs {}",
                m.xy[1],
                o.xy[1]
            );
            assert!(
                (m.depth - o.depth).abs() < 0.01,
                "node {i} depth: {} vs {}",
                m.depth,
                o.depth
            );
            assert!(
                (m.scale - o.scale).abs() < 0.001,
                "node {i} scale: {} vs {}",
                m.scale,
                o.scale
            );
        }
    }

    #[test]
    fn mirrored_world_positions_are_x_negated_partners() {
        // The 3D fact underneath the projection test: evaluated
        // node i of the mirrored pose sits at the X-negation of
        // node partner(i) of the original pose.
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [-15.0, 0.0, 30.0]);
        pose.rotations[1] = [-20.0, 35.0, 10.0];
        pose.rotations[4] = [0.0, 0.0, -45.0]; // foreArm.L
        pose.rotations[12] = [15.0, -10.0, 0.0]; // thigh.R
        let before = evaluate3(&fig, &pose);
        let after = evaluate3(&fig, &mirror_pose(&fig, &pose));
        let partners = mirror_partners(&fig);
        for i in 0..fig.nodes.len() {
            let b = before[partners[i]];
            let a = after[i];
            assert!(
                (a[0] + b[0]).abs() < 0.01
                    && (a[1] - b[1]).abs() < 0.01
                    && (a[2] - b[2]).abs() < 0.01,
                "node {i}: mirrored {a:?} vs partner original {b:?}"
            );
        }
    }

    #[test]
    fn raised_left_arm_mirrors_to_raised_right_arm() {
        let fig = humanoid3();
        let rest = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let rest_pos = evaluate3(&fig, &rest);
        let mut pose = rest.clone();
        // Rz(-60) on upperArm.L swings the sideways-pointing arm
        // upward: the left hand ends ~40 units above rest height.
        pose.rotations[3] = [0.0, 0.0, -60.0];
        let before = evaluate3(&fig, &pose);
        assert!(
            before[5][1] - rest_pos[5][1] > 30.0,
            "left hand raised: {:?} vs rest {:?}",
            before[5],
            rest_pos[5]
        );
        let after = evaluate3(&fig, &mirror_pose(&fig, &pose));
        // Now the RIGHT hand (node 8) is the raised one, at the
        // exact mirror image of the left hand's raised position...
        assert!(
            after[8][1] - rest_pos[8][1] > 30.0,
            "right hand raised: {:?} vs rest {:?}",
            after[8],
            rest_pos[8]
        );
        assert!(
            (after[8][0] + before[5][0]).abs() < 0.01
                && (after[8][1] - before[5][1]).abs() < 0.01
                && (after[8][2] - before[5][2]).abs() < 0.01,
            "right hand {a:?} vs mirrored left hand {b:?}",
            a = after[8],
            b = before[5]
        );
        // ...and the left hand is back at rest.
        assert!(
            (after[5][1] - rest_pos[5][1]).abs() < 0.01,
            "left hand back at rest: {:?} vs {:?}",
            after[5],
            rest_pos[5]
        );
    }
}

#[cfg(test)]
mod camera_framing_tests {
    use super::{
        camera_preset, default_camera, evaluate3, frame_camera, humanoid3, project, Figure3,
        Pose3,
    };

    #[test]
    fn all_four_presets_exist_and_unknown_names_are_none() {
        for name in ["front", "side", "three-quarter", "top"] {
            assert!(camera_preset(name).is_some(), "preset {name} missing");
        }
        assert!(camera_preset("back").is_none());
        assert!(camera_preset("").is_none());
        assert!(camera_preset("FRONT").is_none(), "names are case-sensitive");
    }

    #[test]
    fn presets_adjust_only_yaw_and_pitch() {
        let base = default_camera();
        let front = camera_preset("front").unwrap();
        assert_eq!(front, base, "front is the default orientation");
        let side = camera_preset("side").unwrap();
        assert_eq!(side.yaw, 90.0);
        assert_eq!(side.pitch, base.pitch);
        let tq = camera_preset("three-quarter").unwrap();
        assert_eq!(tq.yaw, 35.0);
        // Positive pitch raises the camera in this module's
        // convention, so the elevated three-quarter view is +12.
        assert_eq!(tq.pitch, 12.0);
        let top = camera_preset("top").unwrap();
        assert!(
            top.pitch >= 89.0,
            "top is as top-down as the pitch clamp allows: {}",
            top.pitch
        );
        // Every preset keeps the default distance, FOV and target.
        for name in ["front", "side", "three-quarter", "top"] {
            let c = camera_preset(name).unwrap();
            assert_eq!(c.distance, base.distance, "{name} distance");
            assert_eq!(c.fov_deg, base.fov_deg, "{name} fov");
            assert_eq!(c.target, base.target, "{name} target");
        }
    }

    #[test]
    fn side_preset_differs_from_front() {
        let front = camera_preset("front").unwrap();
        let side = camera_preset("side").unwrap();
        assert_ne!(side, front);
        assert_ne!(side.yaw, front.yaw);
    }

    #[test]
    fn frame_camera_frames_the_rest_humanoid() {
        let fig = humanoid3();
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        let stage = [960.0, 720.0];
        let cam = frame_camera(&fig, &pose, stage);
        // A front camera: default orientation and FOV, targeted on
        // the figure's centroid.
        assert_eq!((cam.yaw, cam.pitch), (0.0, 0.0));
        assert_eq!(cam.fov_deg, default_camera().fov_deg);
        assert!(cam.distance > 0.0);
        let world = evaluate3(&fig, &pose);
        let proj = project(&world, &cam, stage);
        // Every joint lands inside the stage with a margin — the
        // central-80% region the distance search targets.
        let (x0, x1) = (stage[0] * 0.1, stage[0] * 0.9);
        let (y0, y1) = (stage[1] * 0.1, stage[1] * 0.9);
        for (i, p) in proj.iter().enumerate() {
            assert!(
                p.xy[0] >= x0 && p.xy[0] <= x1 && p.xy[1] >= y0 && p.xy[1] <= y1,
                "joint {i} at {:?} outside the central region",
                p.xy
            );
        }
        // The fit is not absurdly loose: the bounding box covers
        // at least 40% of the stage in width or height.
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in &proj {
            min[0] = min[0].min(p.xy[0]);
            min[1] = min[1].min(p.xy[1]);
            max[0] = max[0].max(p.xy[0]);
            max[1] = max[1].max(p.xy[1]);
        }
        assert!(
            max[0] - min[0] >= 0.4 * stage[0] || max[1] - min[1] >= 0.4 * stage[1],
            "bbox {} x {} on a {} x {} stage is too loose",
            max[0] - min[0],
            max[1] - min[1],
            stage[0],
            stage[1]
        );
    }

    #[test]
    fn frame_camera_is_deterministic() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [15.0, -5.0, 10.0]);
        pose.rotations[3] = [0.0, 0.0, -50.0]; // upperArm.L raised
        pose.rotations[10] = [30.0, 0.0, 0.0]; // shin.L bent
        let stage = [960.0, 720.0];
        assert_eq!(
            frame_camera(&fig, &pose, stage),
            frame_camera(&fig, &pose, stage)
        );
    }

    #[test]
    fn frame_camera_empty_figure_is_the_default_camera() {
        let fig = Figure3 {
            name: "empty".into(),
            color: 0,
            nodes: vec![],
        };
        let pose = Pose3::rest(&fig, [0.0, 0.0, 0.0]);
        assert_eq!(frame_camera(&fig, &pose, [960.0, 720.0]), default_camera());
    }
}

#[cfg(test)]
mod pose_library_tests {
    use super::{
        humanoid3, library_file_name, load_pose_library, load_pose_library_in, pose_file_name,
        save_pose_library, save_pose_library_in, scan_pose_library, scan_pose_library_in, Pose3,
    };
    use std::path::PathBuf;

    #[test]
    fn pose_library_save_scan_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "stykr-fig3d-pose-library-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // Scanning a directory that does not exist yet is empty, and
        // loading a name that was never saved is an error.
        assert!(scan_pose_library_in(&dir).is_empty());
        assert!(load_pose_library_in(&dir, "Nope").is_err());

        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [5.0, -6.0, 7.5]);
        pose.rotations[2] = [0.0, 30.0, 0.0];
        pose.rotations[3] = [12.5, 0.0, -50.25];
        pose.rotations[10] = [30.0, -7.75, 0.0];
        let path = save_pose_library_in(&dir, &pose, "Wave Pose").unwrap();
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "wave-pose.json"
        );
        // A second, messier name exercises the slug rules.
        let path2 = save_pose_library_in(&dir, &pose, "My Pose!!").unwrap();
        assert_eq!(
            path2.file_name().unwrap().to_str().unwrap(),
            "my-pose.json"
        );

        assert_eq!(scan_pose_library_in(&dir), vec!["My Pose!!", "Wave Pose"]);
        let loaded = load_pose_library_in(&dir, "Wave Pose").unwrap();
        // The whole pose survives the round trip exactly, root and
        // every joint rotation (the figure library test's float
        // convention: exact equality after a JSON round trip).
        assert_eq!(loaded, pose);
        assert_eq!(loaded.root, pose.root);
        assert_eq!(loaded.rotations.len(), pose.rotations.len());
        for (got, want) in loaded.rotations.iter().zip(pose.rotations.iter()) {
            assert_eq!(got, want);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pose_file_name_slug_matches_figure_library() {
        // The pose library sanitises names exactly like the figure
        // library: same slug, only the `fig3d-` prefix differs.
        for name in ["Humanoid3", "My Bot!!", "Crouch  Pose", "  --  "] {
            assert_eq!(
                pose_file_name(name),
                library_file_name(name).strip_prefix("fig3d-").unwrap()
            );
        }
        assert_eq!(pose_file_name("Humanoid3"), "humanoid3.json");
        assert_eq!(pose_file_name("My Bot!!"), "my-bot.json");
        // The empty-slug fallback matches too.
        assert_eq!(pose_file_name("  --  "), "figure.json");
    }

    #[test]
    fn pose_library_public_api_uses_library3d_poses() {
        let fig = humanoid3();
        let mut pose = Pose3::rest(&fig, [1.0, 2.0, 3.0]);
        pose.rotations[6] = [0.0, -45.0, 15.5];
        let name = "Wave12 Pose Library Test";
        let path = save_pose_library(&pose, name).unwrap();
        assert_eq!(
            path,
            PathBuf::from("library3d")
                .join("poses")
                .join("wave12-pose-library-test.json")
        );
        assert!(scan_pose_library().iter().any(|n| n == name));
        assert_eq!(load_pose_library(name).unwrap(), pose);
        // Clean up the file this test wrote into ./library3d/poses.
        let _ = std::fs::remove_file(&path);
    }
}
