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
/// End-cap rendering for a segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum CapKind {
    #[default]
    Round,
    Square,
    Point,
}

/// Per-segment rendering style (wave 4 prep; rendered by export/svg).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeStyle {
    /// Width multiplier at the far end (1.0 = uniform, 0.2 = sharp taper).
    pub taper: f32,
    pub cap: CapKind,
    /// Gradient end colour; `None` = solid.
    pub color2: Option<u32>,
    /// 0.0..=1.0 opacity multiplier.
    pub opacity: f32,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle { taper: 1.0, cap: CapKind::Round, color2: None, opacity: 1.0 }
    }
}

/// Media attached to a segment: a sprite from the project's asset
/// table, or a text label drawn along the segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SegmentMedia {
    Image { asset: usize },
    Text { content: String, size: f32 },
}

/// A named image in the project's asset table (PNG bytes).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageAsset {
    pub name: String,
    pub png: Vec<u8>,
}

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
    /// Stroke thickness of this segment in stage units.
    #[serde(default = "default_thickness")]
    pub thickness: f32,
    /// Per-segment colour override; `None` inherits the figure colour.
    #[serde(default)]
    pub color: Option<u32>,
    /// True for hands/feet: dragging this node solves two-bone IK.
    #[serde(default)]
    pub ik_end: bool,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub style: NodeStyle,
    #[serde(default)]
    pub media: Option<SegmentMedia>,
}

fn default_thickness() -> f32 {
    5.0
}

/// What a node draws as: a plain segment, a circle, or a regular
/// polygon (sides >= 3), centred on the node's position with radius
/// half its length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Line,
    Circle,
    Polygon(u8),
}

impl Default for Shape {
    fn default() -> Self {
        Shape::Line
    }
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
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
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
        for i in [4, 6, 8, 10] {
            nodes[i].ik_end = true;
        }
        FigureDef { nodes }
    }

    /// A two-segment-limb quadruped (dog/horse-like), root at the rear
    /// of the spine, head at the front. Side view, facing right.
    pub fn quadruped() -> Self {
        let n = |name: &str, parent: Option<usize>, length: f32, rest: f32| NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n("pelvis", None, 0.0, 0.0),              // 0 root (rear)
            n("chest", Some(0), 95.0, 90.0),          // 1 spine, points right
            n("neck", Some(1), 34.0, 50.0),           // 2 abs 140: up-forward
            n("head", Some(2), 26.0, 8.0),            // 3 abs 148
            n("hind upper L", Some(0), 40.0, 16.0),   // 4
            n("hind lower L", Some(4), 38.0, -10.0),  // 5 foot
            n("hind upper R", Some(0), 40.0, 27.0),   // 6
            n("hind lower R", Some(6), 38.0, -12.0),  // 7 foot
            n("front upper L", Some(1), 40.0, -78.0), // 8 (abs ~12: down)
            n("front lower L", Some(8), 38.0, 6.0),   // 9 foot
            n("front upper R", Some(1), 40.0, -68.0), // 10
            n("front lower R", Some(10), 38.0, 4.0),  // 11 foot
            n("tail", Some(0), 44.0, -118.0),         // 12 up-back
        ];
        nodes[3].head = true;
        nodes[5].bend = -1.0;
        nodes[7].bend = -1.0;
        for i in [5, 7, 9, 11] {
            nodes[i].ik_end = true;
        }
        FigureDef { nodes }
    }

    /// A compact 7-node figure: two-segment limbs are merged, so hands
    /// and feet aim with FK instead of IK.
    pub fn simple() -> Self {
        let n = |name: &str, parent: Option<usize>, length: f32, rest: f32| NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 6.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n("hips", None, 0.0, 0.0),
            n("chest", Some(0), 52.0, 180.0),
            n("head", Some(1), 30.0, 0.0),
            n("arm L", Some(1), 66.0, 112.0),
            n("arm R", Some(1), 66.0, -112.0),
            n("leg L", Some(0), 84.0, -9.0),
            n("leg R", Some(0), 84.0, 9.0),
        ];
        nodes[2].head = true;
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

impl Pose {
    /// Interpolate two poses of the same figure: root lerps linearly,
    /// angles take the shortest path around the circle.
    pub fn lerp(&self, other: &Pose, t: f32) -> Pose {
        let angles = self
            .angles
            .iter()
            .zip(other.angles.iter())
            .map(|(a, b)| norm_deg(a + norm_deg(b - a) * t))
            .collect();
        Pose {
            root: [
                self.root[0] + (other.root[0] - self.root[0]) * t,
                self.root[1] + (other.root[1] - self.root[1]) * t,
            ],
            angles,
        }
    }
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
    #[serde(default)]
    pub hidden: bool,
    /// When set, this instance is a movieclip: its pose comes from
    /// `Project::clips[clip]` looping, not from the timeline frames.
    #[serde(default)]
    pub clip: Option<usize>,
    /// Locked figures cannot be selected or posed in the editor.
    #[serde(default)]
    pub locked: bool,
    /// Layer opacity multiplier (1.0 = opaque).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

fn default_opacity() -> f32 {
    1.0
}

/// A movieclip: a self-contained looping animation of one figure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipDef {
    pub name: String,
    pub def: FigureDef,
    pub frames: Vec<Pose>,
}

/// A freehand brush stroke drawn on one frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stroke {
    pub frame: usize,
    pub points: Vec<V2>,
    pub color: u32,
    pub width: f32,
    /// Per-point widths for pressure strokes; empty = uniform `width`.
    #[serde(default)]
    pub widths: Vec<f32>,
    /// Closed-shape fill colour for this stroke, when filled.
    #[serde(default)]
    pub fill: Option<u32>,
}

/// An audio file placed on the timeline (used by MP4 export).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioClip {
    pub path: String,
    pub start_frame: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub poses: Vec<Pose>,
    /// Hold time multiplier: 2.0 shows this frame twice as long.
    #[serde(default = "default_duration")]
    pub duration: f32,
    /// Tween easing for the segment FROM this frame to the next.
    /// `None` inherits the project-wide easing.
    #[serde(default)]
    pub easing: Option<Easing>,
}

fn default_duration() -> f32 {
    1.0
}

/// One camera keyframe: at `frame`, the camera centres stage point
/// (x, y) and magnifies by `zoom`. Between keys the camera lerps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraKey {
    pub frame: usize,
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    Linear,
    Smooth,
    EaseIn,
    EaseOut,
}

impl Default for Easing {
    fn default() -> Self {
        Easing::Linear
    }
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::Smooth => t * t * (3.0 - 2.0 * t),
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        }
    }
    pub fn next(&self) -> Easing {
        match self {
            Easing::Linear => Easing::Smooth,
            Easing::Smooth => Easing::EaseIn,
            Easing::EaseIn => Easing::EaseOut,
            Easing::EaseOut => Easing::Linear,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Easing::Linear => "linear",
            Easing::Smooth => "smooth",
            Easing::EaseIn => "ease-in",
            Easing::EaseOut => "ease-out",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParticleKind {
    Burst,
    Sparkle,
    Smoke,
}

/// A deterministic particle effect anchored to a figure node (or a
/// fixed stage point when `figure` is None), starting at `frame`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleEvent {
    pub frame: usize,
    pub figure: Option<usize>,
    pub node: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub kind: ParticleKind,
    pub color: u32,
    pub seed: u32,
}

/// A timed VFX clip (see `vfx.rs`) anchored to one figure's node.

/// One keyframed transform for a sprite/text instance.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InstanceKey {
    pub frame: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub angle: f32,
}

/// A free-floating image (from Project.assets) placed on the stage
/// with keyframed transforms — how Stick Nodes sprite instances are
/// modelled.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteInstance {
    pub asset: usize,
    pub keys: Vec<InstanceKey>,
}

/// A free-floating text label with keyframed transforms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextInstance {
    pub content: String,
    pub size: f32,
    pub color: u32,
    pub keys: Vec<InstanceKey>,
}

impl InstanceKey {
    /// Linear interpolation of a key list at `frame_f` (clamped).
    pub fn sample(keys: &[InstanceKey], frame_f: f32) -> Option<InstanceKey> {
        if keys.is_empty() {
            return None;
        }
        if frame_f <= keys[0].frame {
            return Some(keys[0]);
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if frame_f <= b.frame {
                let t = if b.frame > a.frame {
                    (frame_f - a.frame) / (b.frame - a.frame)
                } else {
                    0.0
                };
                return Some(InstanceKey {
                    frame: frame_f,
                    x: a.x + (b.x - a.x) * t,
                    y: a.y + (b.y - a.y) * t,
                    scale: a.scale + (b.scale - a.scale) * t,
                    angle: a.angle + (b.angle - a.angle) * t,
                });
            }
        }
        keys.last().copied()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FxClipEntry {
    pub clip: crate::vfx::ParticleClip,
    pub figure: usize,
    pub node: usize,
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
    #[serde(default)]
    pub camera: Vec<CameraKey>,
    /// When true, playback and GIF export interpolate between
    /// authored frames using `easing`.
    #[serde(default)]
    pub tween_playback: bool,
    #[serde(default)]
    pub easing: Easing,
    /// Camera follows this figure's root when set (zoom still from keys).
    #[serde(default)]
    pub camera_follow: Option<usize>,
    #[serde(default)]
    pub particles: Vec<ParticleEvent>,
    #[serde(default)]
    pub strokes: Vec<Stroke>,
    #[serde(default)]
    pub audio: Vec<AudioClip>,
    #[serde(default)]
    pub clips: Vec<ClipDef>,
    #[serde(default)]
    pub fx_clips: Vec<FxClipEntry>,
    #[serde(default)]
    pub assets: Vec<ImageAsset>,
    /// Property channels for the graph editor (see graph.rs).
    #[serde(default)]
    pub channels: Vec<crate::graph::Channel>,
    #[serde(default)]
    pub sprites: Vec<SpriteInstance>,
    #[serde(default)]
    pub texts: Vec<TextInstance>,
    /// Continuous weather effect over the whole timeline.
    #[serde(default)]
    pub weather: Option<crate::vfx::Weather>,
}

impl Project {
    pub fn new_demo() -> Self {
        let def = FigureDef::humanoid();
        let figures = vec![FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "Styk".into(),
            color: 0xE8E8EC,
            def: def.clone(),
            hidden: false,
            clip: None,
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
            frames.push(Frame { poses: vec![pose], duration: 1.0, easing: None });
        }

        Project {
            name: "Untitled".into(),
            fps: 12.0,
            stage: [960.0, 720.0],
            background: 0x232830,
            figures,
            frames,
            camera: Vec::new(),
            tween_playback: false,
            easing: Easing::Linear,
            camera_follow: None,
            particles: Vec::new(),
            strokes: Vec::new(),
            audio: Vec::new(),
            clips: Vec::new(),
            fx_clips: Vec::new(),
            assets: Vec::new(),
            channels: Vec::new(),
            sprites: Vec::new(),
            texts: Vec::new(),
            weather: None,
        }
    }

    /// Poses of every figure at a fractional frame time. Without
    /// tween playback this is the authored frame (fraction ignored);
    /// with it, poses lerp between neighbouring frames with easing.
    pub fn poses_at(&self, frame_f: f32) -> Vec<Pose> {
        if self.frames.is_empty() {
            return Vec::new();
        }
        let last = self.frames.len() - 1;
        let f = frame_f.clamp(0.0, last as f32);
        let i = f.floor() as usize;
        if !self.tween_playback || i >= last || (f - i as f32) < 0.0001 {
            let mut poses = self.frames[i].poses.clone();
            self.apply_clips(&mut poses, f);
            return poses;
        }
        let easing = self.frames[i].easing.unwrap_or(self.easing);
        let t = easing.apply(f - i as f32);
        let mut poses: Vec<Pose> = self.frames[i]
            .poses
            .iter()
            .zip(self.frames[i + 1].poses.iter())
            .map(|(a, b)| a.lerp(b, t))
            .collect();
        self.apply_clips(&mut poses, f);
        poses
    }

    /// Replace the poses of movieclip instances with their looping
    /// clip poses at frame time `f`.
    fn apply_clips(&self, poses: &mut [Pose], f: f32) {
        for (fi, inst) in self.figures.iter().enumerate() {
            let Some(ci) = inst.clip else { continue };
            let Some(clip) = self.clips.get(ci) else { continue };
            if clip.frames.is_empty() || fi >= poses.len() {
                continue;
            }
            let n = clip.frames.len();
            let idx = (f.floor() as usize) % n;
            poses[fi] = if self.tween_playback && n > 1 {
                let t = self.easing.apply(f - f.floor());
                clip.frames[idx].lerp(&clip.frames[(idx + 1) % n], t)
            } else {
                clip.frames[idx].clone()
            };
        }
    }

    /// Total playback length in seconds, honouring frame durations.
    pub fn total_seconds(&self) -> f32 {
        self.frames.iter().map(|f| f.duration).sum::<f32>() / self.fps.max(1.0)
    }

    /// Camera (centre x, centre y, zoom) at a fractional frame time.
    /// With no keys the camera frames the whole stage.
    pub fn camera_at(&self, frame: f32) -> (f32, f32, f32) {
        let default = (self.stage[0] / 2.0, self.stage[1] / 2.0, 1.0);
        if let Some(fi) = self.camera_follow {
            if fi < self.figures.len() && !self.frames.is_empty() {
                let poses = self.poses_at(frame);
                let zoom = if self.camera.is_empty() {
                    1.0
                } else {
                    self.camera_zoom_at(frame)
                };
                return (poses[fi].root[0], poses[fi].root[1], zoom);
            }
        }
        if self.camera.is_empty() {
            return default;
        }
        let mut keys: Vec<&CameraKey> = self.camera.iter().collect();
        keys.sort_by_key(|k| k.frame);
        let first = keys[0];
        if frame <= first.frame as f32 {
            return (first.x, first.y, first.zoom);
        }
        let last = keys[keys.len() - 1];
        if frame >= last.frame as f32 {
            return (last.x, last.y, last.zoom);
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if frame >= a.frame as f32 && frame <= b.frame as f32 {
                let t = (frame - a.frame as f32) / (b.frame - a.frame).max(1) as f32;
                return (
                    a.x + (b.x - a.x) * t,
                    a.y + (b.y - a.y) * t,
                    a.zoom + (b.zoom - a.zoom) * t,
                );
            }
        }
        default
    }

    fn camera_zoom_at(&self, frame: f32) -> f32 {
        let mut keys: Vec<&CameraKey> = self.camera.iter().collect();
        keys.sort_by_key(|k| k.frame);
        if keys.is_empty() {
            return 1.0;
        }
        let first = keys[0];
        if frame <= first.frame as f32 {
            return first.zoom;
        }
        let last = keys[keys.len() - 1];
        if frame >= last.frame as f32 {
            return last.zoom;
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if frame >= a.frame as f32 && frame <= b.frame as f32 {
                let t = (frame - a.frame as f32) / (b.frame - a.frame).max(1) as f32;
                return a.zoom + (b.zoom - a.zoom) * t;
            }
        }
        1.0
    }

    /// Insert or replace the camera key at `frame`.
    pub fn set_camera_key(&mut self, frame: usize, x: f32, y: f32, zoom: f32) {
        if let Some(k) = self.camera.iter_mut().find(|k| k.frame == frame) {
            k.x = x;
            k.y = y;
            k.zoom = zoom;
        } else {
            self.camera.push(CameraKey { frame, x, y, zoom });
            self.camera.sort_by_key(|k| k.frame);
        }
    }

    pub fn remove_camera_key(&mut self, frame: usize) -> bool {
        let before = self.camera.len();
        self.camera.retain(|k| k.frame != frame);
        self.camera.len() != before
    }

    pub fn add_figure(&mut self, name: &str, color: u32, root: V2) {
        self.add_figure_with(FigureDef::humanoid(), name, color, root);
    }

    pub fn add_figure_with(&mut self, def: FigureDef, name: &str, color: u32, root: V2) {
        let pose = def.rest_pose(root);
        self.figures.push(FigureInstance {
            locked: false,
            opacity: 1.0,
            name: name.into(),
            color,
            def,
            hidden: false,
            clip: None,
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
    fn pose_lerp_takes_shortest_angle_path() {
        let def = FigureDef::humanoid();
        let mut a = def.rest_pose([0.0, 0.0]);
        let mut b = def.rest_pose([100.0, 0.0]);
        a.angles[1] = 170.0;
        b.angles[1] = -170.0;
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.root[0], 50.0);
        assert!((mid.angles[1].abs() - 180.0).abs() < 0.01, "got {}", mid.angles[1]);
    }

    #[test]
    fn camera_interpolates_between_keys() {
        let mut project = Project::new_demo();
        let (x, y, z) = project.camera_at(0.0);
        assert_eq!((x, y, z), (480.0, 360.0, 1.0)); // default: whole stage
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.set_camera_key(4, 680.0, 260.0, 2.0);
        let (x, y, z) = project.camera_at(2.0);
        assert!((x - 580.0).abs() < 0.01 && (y - 310.0).abs() < 0.01 && (z - 1.5).abs() < 0.01);
        let (x, _, z) = project.camera_at(99.0); // clamps to last key
        assert_eq!((x, z), (680.0, 2.0));
        assert!(project.remove_camera_key(4));
        assert_eq!(project.camera.len(), 1);
    }

    #[test]
    fn tween_poses_at_interpolates_with_easing() {
        let mut project = Project::new_demo();
        assert_eq!(project.poses_at(0.5)[0].root[0], 480.0); // no tween: authored frame
        project.tween_playback = true;
        project.easing = Easing::Linear;
        let mid = project.poses_at(0.5);
        assert!((mid[0].root[0] - 487.0).abs() < 0.01, "root {}", mid[0].root[0]);
        project.easing = Easing::EaseIn; // t^2 = 0.25 at halfway
        let eased = project.poses_at(0.5);
        assert!((eased[0].root[0] - 483.5).abs() < 0.01, "root {}", eased[0].root[0]);
    }

    #[test]
    fn per_frame_easing_overrides_project_easing() {
        let mut project = Project::new_demo();
        project.tween_playback = true;
        project.easing = Easing::Linear;
        // None inherits the project easing (linear): t = 0.5.
        let linear = project.poses_at(0.5);
        assert!((linear[0].root[0] - 487.0).abs() < 0.01, "root {}", linear[0].root[0]);
        // Some(EaseIn) on frame 0 wins for the 0 -> 1 segment: t = 0.25.
        project.frames[0].easing = Some(Easing::EaseIn);
        let eased = project.poses_at(0.5);
        assert!((eased[0].root[0] - 483.5).abs() < 0.01, "root {}", eased[0].root[0]);
    }

    #[test]
    fn durations_expand_and_total_time() {
        let mut project = Project::new_demo();
        project.frames[0].duration = 2.0;
        project.frames[1].duration = 3.0;
        let expanded = crate::export::expanded_frames(&project);
        assert_eq!(expanded.len(), 2 + 3 + 1 + 1 + 1);
        assert!((project.total_seconds() - 8.0 / 12.0).abs() < 0.001);
    }

    #[test]
    fn quadruped_stands_and_faces_right() {
        let def = FigureDef::quadruped();
        let pose = def.rest_pose([200.0, 300.0]);
        let pos = evaluate(&def, &pose);
        assert!(pos[3][0] > pos[0][0] + 80.0, "head must be well ahead of the pelvis");
        for foot in [5, 7, 9, 11] {
            assert!(pos[foot][1] > pos[0][1] + 40.0, "foot {foot} must be below the body");
            assert!(def.nodes[foot].ik_end);
        }
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
