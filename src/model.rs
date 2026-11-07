//! Document model for the Rust port of StykAnimate.
//!
//! Kept deliberately separate from the GPUI editor state, mirroring the
//! original app's separation between document data and editor/UI state:
//! everything in this module is plain serializable data plus pure
//! geometry (forward kinematics and two-bone IK). No UI types leak in.

use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

pub type V2 = [f32; 2];

pub fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn scale(a: V2, s: f32) -> V2 {
    [a[0] * s, a[1] * s]
}
pub fn len(a: V2) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}
pub fn dist(a: V2, b: V2) -> f32 {
    len(sub(a, b))
}

/// Direction for an angle in degrees. Screen space: y grows downward,
/// 0 degrees points down, positive angles rotate clockwise.
pub fn dir(angle_deg: f32) -> V2 {
    let r = angle_deg * TAU / 360.0;
    [r.sin(), r.cos()]
}

/// Angle in degrees of a vector, in the same convention as [`dir`].
pub fn angle_of(v: V2) -> f32 {
    v[0].atan2(v[1]) * 360.0 / TAU
}

pub fn norm_deg(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// One joint of a figure. The segment runs from the parent's position to
/// this node's position; `rest_angle` is relative to the parent segment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeDef {
    pub name: String,
    pub parent: Option<usize>,
    pub length: f32,
    pub rest_angle: f32,
    /// Draw a head circle at this node's position.
    pub head: bool,
    /// For end effectors (hands, feet): preferred IK bend side (+1 / -1).
    pub bend: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FigureDef {
    pub nodes: Vec<NodeDef>,
}

impl FigureDef {
    /// The classic Pivot-style humanoid, root at the hips.
    pub fn humanoid() -> Self {
        let n = |name: &str, parent: Option<usize>, length: f32, rest: f32| NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
        };
        let mut nodes = vec![
            n("hips", None, 0.0, 0.0),        // 0 root
            n("chest", Some(0), 46.0, 180.0), // 1 points up
            n("head", Some(1), 30.0, 0.0),    // 2
            n("upper-arm L", Some(1), 38.0, 118.0), // 3: abs 298 (down-left)
            n("forearm L", Some(3), 34.0, 4.0),     // 4 hand L
            n("upper-arm R", Some(1), 38.0, -118.0), // 5: abs 62 (down-right)
            n("forearm R", Some(5), 34.0, -4.0),     // 6 hand R
            n("thigh L", Some(0), 44.0, -11.0),      // 7 knee L
            n("shin L", Some(7), 42.0, 5.0),         // 8 foot L
            n("thigh R", Some(0), 44.0, 11.0),       // 9 knee R
            n("shin R", Some(9), 42.0, -5.0),        // 10 foot R
        ];
        nodes[2].head = true;
        nodes[4].bend = -1.0; // elbows bend backwards
        nodes[6].bend = -1.0;
        nodes[8].bend = 1.0; // knees bend forwards
        nodes[10].bend = 1.0;
        FigureDef { nodes }
    }

    pub fn rest_pose(&self, root: V2) -> Pose {
        Pose {
            root,
            angles: self.nodes.iter().map(|n| n.rest_angle).collect(),
        }
    }
}

/// One figure's pose in one frame: root position plus one relative
/// angle per node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub root: V2,
    pub angles: Vec<f32>,
}

/// Evaluate a pose to absolute joint positions (forward kinematics).
pub fn evaluate(def: &FigureDef, pose: &Pose) -> Vec<V2> {
    let mut pos = vec![[0.0, 0.0]; def.nodes.len()];
    let mut abs_angle = vec![0.0f32; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate() {
        match node.parent {
            None => {
                pos[i] = pose.root;
                abs_angle[i] = 0.0;
            }
            Some(p) => {
                abs_angle[i] = abs_angle[p] + pose.angles[i];
                pos[i] = add(pos[p], scale(dir(abs_angle[i]), node.length));
            }
        }
    }
    pos
}

/// Absolute angle of every node under a pose (helper for IK writes).
fn absolute_angles(def: &FigureDef, pose: &Pose) -> Vec<f32> {
    let mut abs = vec![0.0f32; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate() {
        abs[i] = match node.parent {
            None => 0.0,
            Some(p) => abs[p] + pose.angles[i],
        };
    }
    abs
}

/// Two-bone IK: move `end` (a grandchild) to `target` by solving the
/// chain grandparent -> parent -> end. Returns false when the chain
/// does not have that shape.
pub fn apply_ik(def: &FigureDef, pose: &mut Pose, end: usize, target: V2) -> bool {
    let mid = match def.nodes[end].parent {
        Some(m) => m,
        None => return false,
    };
    let base = match def.nodes[mid].parent {
        Some(b) => b,
        None => return false,
    };
    let positions = evaluate(def, pose);
    let a = positions[base];
    let l1 = def.nodes[mid].length;
    let l2 = def.nodes[end].length;
    let to = sub(target, a);
    let d = len(to).min(l1 + l2 - 0.001).max((l1 - l2).abs() + 0.001);
    let base_ang = angle_of(to);
    // Law of cosines: angle at the base between target direction and l1.
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a_off = cos_a.acos() * 360.0 / TAU;
    let bend = def.nodes[end].bend;
    let mid_abs = base_ang + bend * a_off;
    let mid_pos = add(a, scale(dir(mid_abs), l1));
    let end_abs = angle_of(sub(target, mid_pos));

    let abs = absolute_angles(def, pose);
    let parent_abs = |idx: usize, abs: &[f32]| -> f32 {
        match def.nodes[idx].parent {
            None => 0.0,
            Some(p) => abs[p],
        }
    };
    pose.angles[mid] = norm_deg(mid_abs - parent_abs(mid, &abs));
    // Recompute absolutes after the mid write before setting the end.
    let abs2 = absolute_angles(def, pose);
    pose.angles[end] = norm_deg(end_abs - abs2[mid]);
    true
}

/// Set a single node's segment to aim from its parent's position at
/// `target` (FK drag for ordinary joints).
pub fn aim_node(def: &FigureDef, pose: &mut Pose, node: usize, target: V2) -> bool {
    let parent = match def.nodes[node].parent {
        Some(p) => p,
        None => return false,
    };
    let positions = evaluate(def, pose);
    let abs = absolute_angles(def, pose);
    let want_abs = angle_of(sub(target, positions[parent]));
    // abs[parent] is the parent segment's absolute angle (0 for the root).
    pose.angles[node] = norm_deg(want_abs - abs[parent]);
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FigureInstance {
    pub name: String,
    pub color: u32,
    pub def: FigureDef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub poses: Vec<Pose>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub fps: f32,
    /// Logical stage size the poses were authored in.
    pub stage: [f32; 2],
    pub background: u32,
    pub figures: Vec<FigureInstance>,
    pub frames: Vec<Frame>,
}

impl Project {
    pub fn new_demo() -> Self {
        let def = FigureDef::humanoid();
        let mut figures = vec![FigureInstance {
            name: "Styk".into(),
            color: 0xE8E8EC,
            def: def.clone(),
        }];
        let center = [480.0, 470.0];
        let mut frames = Vec::new();
        // A tiny built-in wave so the first launch already plays.
        for (i, (root_x, wave)) in [(0.0f32, 0.0f32), (14.0, -14.0), (28.0, -26.0), (14.0, -14.0), (0.0, 0.0)]
            .iter()
            .enumerate()
        {
            let mut pose = def.rest_pose([center[0] + root_x, center[1]]);
            pose.angles[5] = -118.0 - wave * 2.2; // raise right arm (wave < 0 lifts)
            pose.angles[6] = -4.0 - wave * 0.8;
            let _ = i;
            frames.push(Frame { poses: vec![pose] });
        }
        figures.shrink_to_fit();
        Project {
            name: "Untitled".into(),
            fps: 12.0,
            stage: [960.0, 720.0],
            background: 0x232830,
            figures,
            frames,
        }
    }

    pub fn add_figure(&mut self, name: &str, color: u32, root: V2) {
        let def = FigureDef::humanoid();
        let pose = def.rest_pose(root);
        self.figures.push(FigureInstance {
            name: name.into(),
            color,
            def,
        });
        for frame in &mut self.frames {
            frame.poses.push(pose.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fk_head_above_hips() {
        let def = FigureDef::humanoid();
        let pose = def.rest_pose([100.0, 300.0]);
        let pos = evaluate(&def, &pose);
        assert!(pos[2][1] < pos[0][1], "head must be above hips");
        assert!((dist(pos[0], pos[1]) - 46.0).abs() < 0.01);
    }

    #[test]
    fn ik_reaches_target() {
        let def = FigureDef::humanoid();
        let mut pose = def.rest_pose([200.0, 300.0]);
        // Reachable: 62 units from the shoulder base, arm reach is 72.
        let target = [255.0, 225.0];
        assert!(apply_ik(&def, &mut pose, 6, target));
        let pos = evaluate(&def, &pose);
        assert!(dist(pos[6], target) < 1.5, "hand at {:?}, want {:?}", pos[6], target);
        // Unreachable targets clamp to full extension toward the target.
        let far = [400.0, 100.0];
        assert!(apply_ik(&def, &mut pose, 6, far));
        let pos = evaluate(&def, &pose);
        let reach = dist(pos[1], pos[6]);
        assert!((reach - 72.0).abs() < 1.5, "clamped reach {reach}");
    }

    #[test]
    fn ik_keeps_segment_lengths() {
        let def = FigureDef::humanoid();
        let mut pose = def.rest_pose([200.0, 300.0]);
        apply_ik(&def, &mut pose, 8, [150.0, 430.0]);
        let pos = evaluate(&def, &pose);
        assert!((dist(pos[7], pos[8]) - 42.0).abs() < 0.05);
        assert!((dist(pos[0], pos[7]) - 44.0).abs() < 0.05);
    }
}
