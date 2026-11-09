//! Document model for the Rust port of StykAnimate.
//!
//! Kept deliberately separate from the GPUI editor state, mirroring the
//! original app's separation between document data and editor/UI state:
//! everything in this module is plain serializable data plus pure
//! geometry (forward kinematics and two-bone IK). No UI types leak in.

use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

pub type V2 = [f32; 2];

/// Radius of the ring drawn around a figure's head joint,
/// in stage units. This constant is the single source for
/// the value: every renderer — the CPU rasterizer
/// (export.rs), the SVG export (svg.rs), the shell stage,
/// the figure thumbnails and the figure inspector's
/// bounding box — draws or measures the head ring with
/// this radius. It is a plain radius in stage units,
/// scaled by each site's own zoom/scale factor exactly as
/// the literal 17.0 was before the constant existed.
/// A project may scale the ring via
/// [`Project::head_ring_radius`], which multiplies this
/// base value by the project's `head_ring_scale`.
pub const HEAD_RING_RADIUS: f32 = 17.0;

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

/// Midpoint of the circular arc from `a` to `b` whose total bend
/// is `bend_rad` (the chord's midpoint pushed along the left
/// normal by (chord/2)·tan(bend/4)). Zero bend returns the chord
/// midpoint. Used to render bent segments as two straight pieces.
pub fn bend_mid(a: V2, b: V2, bend_rad: f32) -> V2 {
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    if bend_rad == 0.0 {
        return mid;
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if len < 0.001 {
        return mid;
    }
    let off = len / 2.0 * (bend_rad / 4.0).tan();
    let n = [-d[1] / len, d[0] / len];
    [mid[0] + n[0] * off, mid[1] + n[1] * off]
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
/// (PartialEq derived in Wave 12 so the save/load round-trip
/// tests can compare whole asset tables directly.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// How a movieclip's frames are sequenced when played back.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum ClipLoop {
    /// Wrap around to the first frame (the original behaviour).
    #[default]
    Loop,
    /// Play forward, then backward, then forward again.
    PingPong,
    /// Play to the last frame and hold it.
    Once,
}

/// The clip frame shown at timeline position `frame` for a clip of
/// `len` frames under `mode`. `len <= 1` always yields 0; callers
/// guard against empty clips as before.
pub fn clip_frame_index(mode: ClipLoop, frame: usize, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    match mode {
        ClipLoop::Loop => frame % len,
        ClipLoop::PingPong => {
            let period = 2 * len - 2;
            let p = frame % period;
            if p < len { p } else { period - p }
        }
        ClipLoop::Once => frame.min(len - 1),
    }
}

/// A movieclip: a self-contained looping animation of one figure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipDef {
    pub name: String,
    pub def: FigureDef,
    pub frames: Vec<Pose>,
    /// Frame sequencing when the clip plays; older projects without
    /// the field deserialize as `ClipLoop::Loop`.
    #[serde(default)]
    pub loop_mode: ClipLoop,
}

/// Spray-paint parameters for a stroke: the stroke's points are
/// rendered as deterministic dots scattered within `radius` of
/// each point (see `paint::spray_dots`), each dot drawn at up to
/// `opacity`. `seed` drives the scatter stream, so a spray stroke
/// renders identically on every machine and in every export.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SprayMark {
    pub seed: u64,
    pub radius: f32,
    pub opacity: f32,
}

/// A freehand brush stroke drawn on one frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Spray-paint mark for this stroke, when sprayed; older
    /// projects without the field deserialize as `None` (a plain
    /// solid stroke).
    #[serde(default)]
    pub spray: Option<SprayMark>,
    /// Per-stroke opacity recorded from the brush when the
    /// stroke was drawn; renderers apply it over the stroke
    /// colour. 1.0 = fully opaque. Projects saved before
    /// this field existed deserialize as 1.0.
    #[serde(default = "default_stroke_opacity")]
    pub opacity: f32,
    /// Scatter radius in stage units recorded from the
    /// brush when the stroke was drawn: renderers spread
    /// the stroke's own marks within this radius of the
    /// recorded path (see `paint::scatter_dabs`), instead
    /// of only through a spray mark. 0.0 = off — the
    /// stroke draws exactly on its path. Projects saved
    /// before this field existed deserialize as 0.0.
    #[serde(default)]
    pub scatter: f32,
}

fn default_stroke_opacity() -> f32 {
    1.0
}

fn default_audio_gain() -> f32 {
    1.0
}

/// An audio file placed on the timeline (used by MP4 export).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioClip {
    pub path: String,
    pub start_frame: usize,
    /// Per-clip volume multiplier (1.0 = unity). Applied wherever
    /// clips are mixed — playback and the MP4/WebM mux — and a
    /// lone clip with a non-unity gain is routed through the mix
    /// so the gain is honoured there too. Projects saved before
    /// this field existed deserialize as 1.0.
    #[serde(default = "default_audio_gain")]
    pub gain: f32,
    /// Muted clips stay on the timeline but are excluded
    /// from playback plans and export mixes when true.
    /// Projects saved before this field existed deserialize
    /// as unmuted.
    #[serde(default)]
    pub muted: bool,
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
    /// Per-figure, per-node segment bend in radians (wave 7):
    /// outer index = figure, inner = node. Empty (or a short inner
    /// vec) means straight segments. Pivot's bend arcs live here.
    #[serde(default)]
    pub bends: Vec<Vec<f32>>,
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
    /// Camera rotation in degrees (wave 6); 0 for all pre-v7 files.
    #[serde(default)]
    pub angle: f32,
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
    /// Hidden clips are skipped by the stage and by export
    /// renderers but remain editable. Projects saved before
    /// this field existed deserialize as visible.
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    // Wave 11 (E7 serde census): the six original fields
    // below carried no serde default — every other field
    // in this struct (and every later-added field in the
    // nested structs) has one. A project file missing any
    // of them failed to load at all, even though three of
    // them (`name`, `fps`, `stage`) are directly settable
    // through MCP tools (`new_project`, `set_project_fps`,
    // `set_stage_size`), so a hand-trimmed or partially
    // written MCP-authored file was a load-breaking
    // landmine. The defaults mirror the values both
    // project constructors already use (`Project::new_demo`
    // and the MCP `new_project` handler): "Untitled",
    // 12 fps, 960x720 stage, background 0x232830.
    #[serde(default = "default_project_name")]
    pub name: String,
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// Logical stage size the poses were authored in.
    #[serde(default = "default_stage")]
    pub stage: [f32; 2],
    #[serde(default = "default_background")]
    pub background: u32,
    #[serde(default)]
    pub figures: Vec<FigureInstance>,
    #[serde(default)]
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
    /// Per-project scale for the head ring radius, applied
    /// by [`Project::head_ring_radius`]. 1.0 draws the ring
    /// at [`HEAD_RING_RADIUS`]. Files saved before this
    /// field existed carry no value and load as 1.0.
    #[serde(default = "default_head_ring_scale")]
    pub head_ring_scale: f32,
}

fn default_head_ring_scale() -> f32 {
    1.0
}

fn default_project_name() -> String {
    "Untitled".to_string()
}

fn default_fps() -> f32 {
    12.0
}

fn default_stage() -> [f32; 2] {
    [960.0, 720.0]
}

fn default_background() -> u32 {
    0x232830
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
            frames.push(Frame { poses: vec![pose], duration: 1.0, easing: None, bends: Vec::new() });
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
            head_ring_scale: 1.0,
        }
    }

    /// The head ring radius for this project, in stage
    /// units: [`HEAD_RING_RADIUS`] multiplied by
    /// `head_ring_scale`. The scale is sanitised at read —
    /// a non-finite value falls back to 1.0 and any finite
    /// value is clamped into [0.25, 4.0] — so a hand-edited
    /// or corrupt project file can never produce an
    /// invisible or absurd ring.
    pub fn head_ring_radius(&self) -> f32 {
        let scale = if self.head_ring_scale.is_finite() {
            self.head_ring_scale.clamp(0.25, 4.0)
        } else {
            1.0
        };
        HEAD_RING_RADIUS * scale
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

    /// Interpolated per-node bends (radians) for figure `fi` at
    /// fractional frame time `frame_f`, mirroring `poses_at`'s
    /// indexing: empty when the frames carry no bend data.
    pub fn bends_at(&self, frame_f: f32, fi: usize) -> Vec<f32> {
        let n = self.figures.get(fi).map(|f| f.def.nodes.len()).unwrap_or(0);
        if n == 0 || self.frames.is_empty() {
            return Vec::new();
        }
        let get = |idx: usize| -> Vec<f32> {
            let mut v = self
                .frames
                .get(idx)
                .and_then(|f| f.bends.get(fi))
                .cloned()
                .unwrap_or_default();
            v.resize(n, 0.0);
            v
        };
        let last = self.frames.len() - 1;
        let f = frame_f.clamp(0.0, last as f32);
        let i = f.floor() as usize;
        let frac = f - i as f32;
        if !self.tween_playback || i >= last || frac < 0.0001 {
            let v = get(i);
            return if v.iter().all(|b| *b == 0.0) { Vec::new() } else { v };
        }
        let a = get(i);
        let b = get(i + 1);
        let v: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x + (y - x) * frac).collect();
        if v.iter().all(|b| *b == 0.0) { Vec::new() } else { v }
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
            let base = f.floor() as usize;
            let idx = clip_frame_index(clip.loop_mode, base, n);
            poses[fi] = if self.tween_playback && n > 1 {
                let t = self.easing.apply(f - f.floor());
                let next = clip_frame_index(clip.loop_mode, base + 1, n);
                clip.frames[idx].lerp(&clip.frames[next], t)
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

    /// Interpolated camera rotation (degrees) at `frame`, using the
    /// shortest angular path between keys. 0 when no keys exist.
    pub fn camera_angle_at(&self, frame: f32) -> f32 {
        if self.camera.is_empty() {
            return 0.0;
        }
        let mut keys: Vec<&CameraKey> = self.camera.iter().collect();
        keys.sort_by_key(|k| k.frame);
        let first = keys[0];
        if frame <= first.frame as f32 {
            return first.angle;
        }
        let last = keys[keys.len() - 1];
        if frame >= last.frame as f32 {
            return last.angle;
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if frame >= a.frame as f32 && frame <= b.frame as f32 {
                let t = (frame - a.frame as f32) / (b.frame - a.frame).max(1) as f32;
                let d = norm_deg(b.angle - a.angle);
                return norm_deg(a.angle + d * t);
            }
        }
        last.angle
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
            self.camera.push(CameraKey { frame, x, y, zoom, angle: 0.0 });
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
    fn head_ring_radius_is_pinned() {
        // Every renderer's head ring is drawn from this one
        // constant; pin the value so an accidental edit
        // fails loudly here instead of drifting the ring
        // in one renderer at a time.
        assert_eq!(HEAD_RING_RADIUS, 17.0);
    }

    #[test]
    fn head_ring_radius_defaults_to_the_base_constant() {
        let project = Project::new_demo();
        assert_eq!(project.head_ring_scale, 1.0);
        assert_eq!(project.head_ring_radius(), 17.0);
    }

    #[test]
    fn head_ring_radius_scales_with_the_project() {
        let mut project = Project::new_demo();
        project.head_ring_scale = 2.0;
        assert_eq!(project.head_ring_radius(), 34.0);
        project.head_ring_scale = 0.5;
        assert_eq!(project.head_ring_radius(), 8.5);
    }

    #[test]
    fn head_ring_radius_sanitises_the_scale_at_read() {
        let mut project = Project::new_demo();
        project.head_ring_scale = f32::NAN;
        assert_eq!(project.head_ring_radius(), 17.0);
        project.head_ring_scale = f32::INFINITY;
        assert_eq!(project.head_ring_radius(), 17.0);
        // Finite but out of range: clamped to the 4.0 cap
        // (and the 0.25 floor below).
        project.head_ring_scale = 100.0;
        assert_eq!(project.head_ring_radius(), 68.0);
        project.head_ring_scale = 0.0;
        assert_eq!(project.head_ring_radius(), 4.25);
    }

    #[test]
    fn project_without_head_ring_scale_loads_at_scale_one() {
        // Back-compatibility: a project file saved before
        // the field existed has no `head_ring_scale` key
        // and must still load, at the default scale.
        let mut value = serde_json::to_value(Project::new_demo()).unwrap();
        let removed = value
            .as_object_mut()
            .unwrap()
            .remove("head_ring_scale");
        assert!(removed.is_some(), "serialised project carries the field");
        let restored: Project = serde_json::from_value(value).unwrap();
        assert_eq!(restored.head_ring_scale, 1.0);
        assert_eq!(restored.head_ring_radius(), 17.0);
    }

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

#[cfg(test)]
mod clip_loop_tests {
    use super::{clip_frame_index, ClipDef, ClipLoop, Project};

    #[test]
    fn clip_index_loop_wraps() {
        let got: Vec<usize> = (0..7).map(|f| clip_frame_index(ClipLoop::Loop, f, 3)).collect();
        assert_eq!(got, vec![0, 1, 2, 0, 1, 2, 0]);
    }

    #[test]
    fn clip_index_pingpong_bounces() {
        let got: Vec<usize> = (0..6).map(|f| clip_frame_index(ClipLoop::PingPong, f, 3)).collect();
        assert_eq!(got, vec![0, 1, 2, 1, 0, 1]);
        let got4: Vec<usize> = (0..7).map(|f| clip_frame_index(ClipLoop::PingPong, f, 4)).collect();
        assert_eq!(got4, vec![0, 1, 2, 3, 2, 1, 0]);
    }

    #[test]
    fn clip_index_once_clamps_on_last_frame() {
        let got: Vec<usize> = (0..6).map(|f| clip_frame_index(ClipLoop::Once, f, 3)).collect();
        assert_eq!(got, vec![0, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn clip_index_single_frame_is_always_zero() {
        for mode in [ClipLoop::Loop, ClipLoop::PingPong, ClipLoop::Once] {
            for f in [0, 1, 7, 100] {
                assert_eq!(clip_frame_index(mode, f, 1), 0);
                assert_eq!(clip_frame_index(mode, f, 0), 0);
            }
        }
    }

    #[test]
    fn clip_loop_default_is_loop() {
        assert_eq!(ClipLoop::default(), ClipLoop::Loop);
    }

    #[test]
    fn clip_instance_honours_loop_mode_at_poses_at() {
        let mut project = Project::new_demo();
        let def = project.figures[0].def.clone();
        let frames = vec![
            def.rest_pose([10.0, 0.0]),
            def.rest_pose([20.0, 0.0]),
            def.rest_pose([30.0, 0.0]),
        ];
        project.clips.push(ClipDef {
            name: "wave".to_string(),
            def,
            frames,
            loop_mode: ClipLoop::PingPong,
        });
        project.figures[0].clip = Some(0);
        // The demo timeline has 5 frames (0..=4); poses_at clamps to it.
        // PingPong visits clip frames 0, 1, 2, 1, 0.
        let roots: Vec<f32> = (0..5).map(|f| project.poses_at(f as f32)[0].root[0]).collect();
        assert_eq!(roots, vec![10.0, 20.0, 30.0, 20.0, 10.0]);
        // Loop wraps: 0, 1, 2, 0, 1 — the pre-existing behaviour.
        project.clips[0].loop_mode = ClipLoop::Loop;
        let roots: Vec<f32> = (0..5).map(|f| project.poses_at(f as f32)[0].root[0]).collect();
        assert_eq!(roots, vec![10.0, 20.0, 30.0, 10.0, 20.0]);
        // Once holds the last frame: 0, 1, 2, 2, 2.
        project.clips[0].loop_mode = ClipLoop::Once;
        let roots: Vec<f32> = (0..5).map(|f| project.poses_at(f as f32)[0].root[0]).collect();
        assert_eq!(roots, vec![10.0, 20.0, 30.0, 30.0, 30.0]);
    }
}

// ---- Wave 13 spray data model ----
//
// Separate module on purpose: exact-name imports only, never a
// glob. Pins the serde contract of `Stroke::spray`: projects
// saved before the field existed must load with `spray: None`.
#[cfg(test)]
mod spray_wave13_tests {
    use crate::model::{SprayMark, Stroke};

    #[test]
    fn stroke_json_without_a_spray_field_loads_with_spray_none() {
        // Every field a pre-spray project file carries — and no
        // `spray` key at all.
        let json = r#"{
            "frame": 2,
            "points": [[10.0, 20.0], [30.0, 40.0]],
            "color": 16711680,
            "width": 4.0,
            "widths": [],
            "fill": null
        }"#;
        let stroke: Stroke =
            serde_json::from_str(json).expect("pre-spray stroke JSON must still parse");
        assert_eq!(stroke.spray, None, "missing spray field defaults to None");
        assert_eq!(stroke.frame, 2);
        assert_eq!(stroke.points.len(), 2);
    }

    #[test]
    fn spray_mark_round_trips_through_json() {
        let mark = SprayMark {
            seed: 42,
            radius: 12.0,
            opacity: 0.5,
        };
        let json = serde_json::to_string(&mark).expect("SprayMark serialises");
        let back: SprayMark = serde_json::from_str(&json).expect("SprayMark parses back");
        assert_eq!(back, mark);
    }

    #[test]
    fn stroke_with_a_spray_mark_round_trips_through_json() {
        let mut stroke: Stroke = serde_json::from_str(
            r#"{"frame":0,"points":[[0.0,0.0]],"color":0,"width":14.0,"widths":[],"fill":null}"#,
        )
        .expect("plain stroke parses");
        stroke.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        let json = serde_json::to_string(&stroke).expect("spray stroke serialises");
        assert!(json.contains("\"spray\""), "spray field is stored: {json}");
        let back: Stroke = serde_json::from_str(&json).expect("spray stroke parses back");
        assert_eq!(
            back.spray,
            Some(SprayMark {
                seed: 7,
                radius: 12.0,
                opacity: 0.5
            })
        );
    }
}

#[cfg(test)]
mod audio_gain_tests {
    use crate::model::{AudioClip, FxClipEntry, Project, Stroke};

    #[test]
    fn audio_clip_json_without_gain_defaults_to_unity() {
        // Projects saved before the gain field existed carry no
        // "gain" key in their clips; they must load at unity.
        let clip: AudioClip =
            serde_json::from_str(r#"{"path":"a.wav","start_frame":7}"#).unwrap();
        assert_eq!(clip.path, "a.wav");
        assert_eq!(clip.start_frame, 7);
        assert_eq!(clip.gain, 1.0);
    }

    #[test]
    fn audio_clip_json_with_gain_round_trips() {
        let clip: AudioClip =
            serde_json::from_str(r#"{"path":"a.wav","start_frame":7,"gain":0.25}"#).unwrap();
        assert_eq!(clip.gain, 0.25);
        let json = serde_json::to_string(&clip).unwrap();
        let back: AudioClip = serde_json::from_str(&json).unwrap();
        assert_eq!(back.gain, 0.25);
    }

    #[test]
    fn project_json_without_clip_gain_loads_clips_at_unity() {
        // Serialize a current project, strip the gain key to get
        // the pre-gain on-disk shape, and reload: the clip must
        // come back at unity gain, not fail or default to 0.
        let mut project = Project::new_demo();
        project.audio.push(AudioClip {
            path: "song.wav".to_string(),
            start_frame: 12,
            gain: 0.5,
            muted: false,
        });
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"gain\":0.5"), "gain is serialized: {json}");
        let old_json = json.replace(",\"gain\":0.5", "");
        assert!(!old_json.contains("\"gain\""));
        let back: Project = serde_json::from_str(&old_json).unwrap();
        assert_eq!(back.audio.len(), 1);
        assert_eq!(back.audio[0].gain, 1.0);
    }

    #[test]
    fn audio_clip_without_muted_loads_unmuted() {
        // Back-compatibility: a clip saved before the field
        // existed has no `muted` key and must load unmuted.
        let clip: AudioClip =
            serde_json::from_str(r#"{"path":"a.wav","start_frame":7,"gain":0.25}"#).unwrap();
        assert!(!clip.muted);
    }

    #[test]
    fn audio_clip_muted_round_trips() {
        let clip = AudioClip {
            path: "a.wav".to_string(),
            start_frame: 7,
            gain: 1.0,
            muted: true,
        };
        let json = serde_json::to_string(&clip).unwrap();
        assert!(json.contains("\"muted\":true"), "muted is serialized: {json}");
        let back: AudioClip = serde_json::from_str(&json).unwrap();
        assert!(back.muted);
    }

    #[test]
    fn stroke_without_opacity_loads_fully_opaque() {
        // Back-compatibility: a stroke saved before the
        // field existed has no `opacity` key and must load
        // at 1.0, not fail or default to 0.
        let stroke: Stroke = serde_json::from_str(
            r#"{"frame":3,"points":[[10.0,20.0],[30.0,40.0]],"color":16711680,"width":4.0}"#,
        )
        .unwrap();
        assert_eq!(stroke.opacity, 1.0);
    }

    #[test]
    fn stroke_opacity_round_trips() {
        let stroke = Stroke {
            frame: 3,
            points: vec![[10.0, 20.0], [30.0, 40.0]],
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 0.5,
            scatter: 0.0,
        };
        let json = serde_json::to_string(&stroke).unwrap();
        assert!(
            json.contains("\"opacity\":0.5"),
            "opacity is serialized: {json}"
        );
        let back: Stroke = serde_json::from_str(&json).unwrap();
        assert_eq!(back.opacity, 0.5);
    }

    fn fx_entry(hidden: bool) -> FxClipEntry {
        FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 16.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 11,
                angle: 0.0,
                length: 130.0,
                color2: None,
            },
            figure: 0,
            node: 0,
            hidden,
        }
    }

    #[test]
    fn fx_clip_entry_without_hidden_loads_visible() {
        // Back-compatibility: an entry saved before the
        // field existed has no `hidden` key and must load
        // visible. Strip the key from a serialised current
        // entry so the nested clip keeps the writer's shape.
        let mut value = serde_json::to_value(fx_entry(false)).unwrap();
        let removed = value.as_object_mut().unwrap().remove("hidden");
        assert!(removed.is_some(), "serialised entry carries the field");
        let restored: FxClipEntry = serde_json::from_value(value).unwrap();
        assert!(!restored.hidden);
    }

    #[test]
    fn fx_clip_entry_hidden_round_trips() {
        let json = serde_json::to_string(&fx_entry(true)).unwrap();
        assert!(
            json.contains("\"hidden\":true"),
            "hidden is serialized: {json}"
        );
        let back: FxClipEntry = serde_json::from_str(&json).unwrap();
        assert!(back.hidden);
    }
}

// ---- Wave 10 scatter data model ----
//
// Separate module on purpose: exact-name imports only, never a
// glob. Pins the serde contract of `Stroke::scatter`: projects
// saved before the field existed must load with `scatter: 0.0`
// (off — the stroke draws exactly on its path).
#[cfg(test)]
mod scatter_wave10_tests {
    use crate::model::{Project, Stroke};

    /// Every field a pre-scatter project file carries for a
    /// stroke — and no `scatter` key at all.
    const OLD_STROKE_JSON: &str = r#"{
        "frame": 2,
        "points": [[10.0, 20.0], [30.0, 40.0]],
        "color": 16711680,
        "width": 4.0,
        "widths": [],
        "fill": null,
        "spray": null,
        "opacity": 0.5
    }"#;

    #[test]
    fn stroke_json_without_a_scatter_field_loads_with_scatter_off() {
        let stroke: Stroke = serde_json::from_str(OLD_STROKE_JSON)
            .expect("pre-scatter stroke JSON must still parse");
        assert_eq!(stroke.scatter, 0.0, "missing scatter field defaults to off");
        // The rest of the old shape is untouched by the new field.
        assert_eq!(stroke.frame, 2);
        assert_eq!(stroke.points.len(), 2);
        assert_eq!(stroke.opacity, 0.5);
    }

    #[test]
    fn stroke_scatter_round_trips_through_json() {
        let mut stroke: Stroke =
            serde_json::from_str(OLD_STROKE_JSON).expect("plain stroke parses");
        stroke.scatter = 12.0;
        let json = serde_json::to_string(&stroke).expect("scattered stroke serialises");
        assert!(
            json.contains("\"scatter\":12.0"),
            "scatter field is stored: {json}"
        );
        let back: Stroke = serde_json::from_str(&json).expect("scattered stroke parses back");
        assert_eq!(back.scatter, 12.0);
    }

    #[test]
    fn demo_project_strokes_all_have_scatter_off() {
        // The demo project authors no strokes today, so plant
        // one old-format stroke on it — the shape a saved demo
        // project from before the field existed would carry —
        // and pin that every stroke on the project is
        // unscattered (the set is non-empty by construction).
        let mut project = Project::new_demo();
        let old: Stroke = serde_json::from_str(OLD_STROKE_JSON)
            .expect("pre-scatter stroke JSON must still parse");
        project.strokes.push(old);
        assert_eq!(project.strokes.len(), 1, "the planted stroke is present");
        assert!(
            project.strokes.iter().all(|s| s.scatter == 0.0),
            "every demo-project stroke has scatter 0.0"
        );
    }
}

// ---- Wave 11 MCP-authored project round trip ----
//
// Separate module on purpose: exact-name imports only, never a
// glob. Wave 11's stress case is a project built ENTIRELY
// through MCP calls: every field an MCP tool can set must
// survive a save -> load round trip with the value intact.
// This module plants non-default values in every MCP-settable
// field of the document model (plus the file-level fields no
// tool sets, for completeness), serialises, deserialises, and
// asserts field by field — `Project` has no `PartialEq`, and
// a whole-struct assert could not name the lost field anyway.
#[cfg(test)]
mod mcp_roundtrip_wave11_tests {
    use crate::graph::{Channel, ChannelTarget, Interp, Key};
    use crate::model::{
        AudioClip, CameraKey, CapKind, ClipDef, ClipLoop, Easing, FigureDef, Frame, FxClipEntry,
        ImageAsset, InstanceKey, ParticleEvent, ParticleKind, Pose, Project, SegmentMedia, Shape,
        SprayMark, SpriteInstance, Stroke, TextInstance,
    };
    use crate::vfx::{ClipKind, ParticleClip, Weather, WeatherKind};

    /// A project with a non-default value in every field the
    /// MCP surface can write (tool names in the E7 census),
    /// built the way the tools build it: from the demo shape,
    /// then mutated field by field.
    fn mcp_authored_project() -> Project {
        let mut project = Project::new_demo();
        // Project scalars: new_project / set_project_fps /
        // set_stage_size / set_head_ring_scale /
        // set_tween_playback / set_project_easing.
        project.name = "MCP Authored".to_string();
        project.fps = 24.0;
        project.stage = [800.0, 600.0];
        project.background = 0x101418;
        project.head_ring_scale = 2.5;
        project.tween_playback = true;
        project.easing = Easing::EaseIn;
        project.camera_follow = Some(0);
        // Camera: set_camera (incl. the angle arg).
        project.camera.push(CameraKey {
            frame: 2,
            x: 300.0,
            y: 250.0,
            zoom: 1.5,
            angle: 33.0,
        });
        // Figure flags: layers (hidden / locked / opacity),
        // figure_set_color, clip assignment.
        project.figures[0].hidden = true;
        project.figures[0].locked = true;
        project.figures[0].opacity = 0.5;
        project.figures[0].clip = Some(0);
        // Node fields: set_node (length / thickness / color /
        // shape), set_node_cap / set_node_color2 /
        // set_node_opacity (style), plus a media value for
        // file-level completeness (no MCP setter exists).
        {
            let node = &mut project.figures[0].def.nodes[1];
            node.length = 52.0;
            node.thickness = 7.5;
            node.color = Some(0x00FF00);
            node.shape = Shape::Circle;
            node.style.taper = 0.5;
            node.style.cap = CapKind::Point;
            node.style.color2 = Some(0x0000FF);
            node.style.opacity = 0.75;
            node.media = Some(SegmentMedia::Text {
                content: "hi".to_string(),
                size: 14.0,
            });
        }
        // Frame fields: set_frame_duration (duration),
        // set_frame_easing (easing), set_bend (bends),
        // set_pose (poses).
        project.frames[0].duration = 2.5;
        project.frames[0].easing = Some(Easing::Smooth);
        let node_count = project.figures[0].def.nodes.len();
        project.frames[0].bends = vec![vec![0.25; node_count]];
        project.frames[1].poses[0].root = [111.0, 222.0];
        project.frames[1].poses[0].angles[1] = 42.0;
        // Stroke fields: draw_stroke (frame / points / color /
        // width / scatter); widths, fill, spray and opacity
        // planted too — serde must carry them even though the
        // MCP draw tools cannot set them today (E7 census).
        project.strokes.push(Stroke {
            frame: 1,
            points: vec![[10.0, 20.0], [30.0, 40.0]],
            color: 0xFF0000,
            width: 6.0,
            widths: vec![2.0, 6.0],
            fill: Some(0x00FF00),
            spray: Some(SprayMark {
                seed: 99,
                radius: 8.0,
                opacity: 0.5,
            }),
            opacity: 0.5,
            scatter: 12.0,
        });
        // Audio fields: add_audio_clip (path / start_frame),
        // audio_set_gain (gain), audio_set_muted (muted).
        project.audio.push(AudioClip {
            path: "song.wav".to_string(),
            start_frame: 3,
            gain: 0.25,
            muted: true,
        });
        // FX entry: add_fx_clip + fx_set_color2 (clip fields),
        // hidden planted (serde-carried; MCP had no setter or
        // reader when this census was taken — the known
        // entering gap of Wave 11).
        project.fx_clips.push(FxClipEntry {
            clip: ParticleClip {
                start: 1.0,
                duration: 16.0,
                kind: ClipKind::Sparkle,
                color: 0xFFC65D,
                color2: Some(0x112233),
                seed: 77,
                angle: 15.0,
                length: 140.0,
            },
            figure: 0,
            node: 2,
            hidden: true,
        });
        // Particle event: particle_add_event (frame / x / y)
        // + particle_set_kind / particle_set_color; figure /
        // node / seed planted for serde completeness.
        project.particles.push(ParticleEvent {
            frame: 2,
            figure: Some(0),
            node: Some(4),
            x: 200.0,
            y: 150.0,
            kind: ParticleKind::Smoke,
            color: 0xABCDEF,
            seed: 1234,
        });
        // Movieclip: clip_create (name / def / frames),
        // set_clip_loop (loop_mode).
        let clip_def = FigureDef::simple();
        project.clips.push(ClipDef {
            name: "wave".to_string(),
            def: clip_def.clone(),
            frames: vec![
                clip_def.rest_pose([10.0, 0.0]),
                clip_def.rest_pose([20.0, 0.0]),
            ],
            loop_mode: ClipLoop::PingPong,
        });
        // Graph channel: graph_key (frame / value / tangents),
        // set_key_interp (interp).
        project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![
                Key {
                    frame: 0.0,
                    value: 1.0,
                    in_tan: 0.5,
                    out_tan: -0.25,
                    interp: Interp::Linear,
                },
                Key::flat(2.0, 44.0).with_interp(Interp::Step),
            ],
        });
        // Weather: set_weather (kind / amount / wind).
        project.weather = Some(Weather {
            kind: WeatherKind::Storm,
            amount: 140.0,
            wind: 30.0,
        });
        // Assets / sprites / texts: no MCP setter exists (E7
        // census) — planted so the file-level round trip is
        // proven for the fields an imported project carries.
        project.assets.push(ImageAsset {
            name: "dot.png".to_string(),
            png: vec![0x89, 0x50, 0x4E, 0x47],
        });
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 1.0,
                x: 50.0,
                y: 60.0,
                scale: 1.5,
                angle: 10.0,
            }],
        });
        project.texts.push(TextInstance {
            content: "Title".to_string(),
            size: 28.0,
            color: 0xFFFFFF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 80.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        project
    }

    #[test]
    fn mcp_authored_project_round_trips_field_by_field() {
        let project = mcp_authored_project();
        let json = serde_json::to_string(&project).expect("project serialises");
        let back: Project = serde_json::from_str(&json).expect("project parses back");

        // Project scalars.
        assert_eq!(back.name, "MCP Authored");
        assert_eq!(back.fps, 24.0);
        assert_eq!(back.stage, [800.0, 600.0]);
        assert_eq!(back.background, 0x101418);
        assert_eq!(back.head_ring_scale, 2.5);
        assert!(back.tween_playback);
        assert_eq!(back.easing, Easing::EaseIn);
        assert_eq!(back.camera_follow, Some(0));
        // Camera key, angle included.
        assert_eq!(back.camera.len(), 1);
        let key = &back.camera[0];
        assert_eq!(
            (key.frame, key.x, key.y, key.zoom, key.angle),
            (2, 300.0, 250.0, 1.5, 33.0)
        );
        // Figure flags + node fields.
        let fig = &back.figures[0];
        assert!(fig.hidden && fig.locked);
        assert_eq!(fig.opacity, 0.5);
        assert_eq!(fig.clip, Some(0));
        let node = &fig.def.nodes[1];
        assert_eq!(node.length, 52.0);
        assert_eq!(node.thickness, 7.5);
        assert_eq!(node.color, Some(0x00FF00));
        assert_eq!(node.shape, Shape::Circle);
        assert_eq!(node.style.taper, 0.5);
        assert_eq!(node.style.cap, CapKind::Point);
        assert_eq!(node.style.color2, Some(0x0000FF));
        assert_eq!(node.style.opacity, 0.75);
        assert_eq!(
            node.media,
            Some(SegmentMedia::Text {
                content: "hi".to_string(),
                size: 14.0
            })
        );
        // Frame fields.
        assert_eq!(back.frames[0].duration, 2.5);
        assert_eq!(back.frames[0].easing, Some(Easing::Smooth));
        assert_eq!(back.frames[0].bends.len(), 1);
        assert!(back.frames[0].bends[0].iter().all(|b| *b == 0.25));
        assert_eq!(back.frames[1].poses[0].root, [111.0, 222.0]);
        assert_eq!(back.frames[1].poses[0].angles[1], 42.0);
        // Stroke: whole-struct equality (Stroke is PartialEq)
        // covers widths / fill / spray / opacity / scatter.
        assert_eq!(back.strokes.len(), 1);
        assert_eq!(back.strokes[0], project.strokes[0]);
        // Audio.
        let clip = &back.audio[0];
        assert_eq!(
            (clip.path.as_str(), clip.start_frame, clip.gain, clip.muted),
            ("song.wav", 3, 0.25, true)
        );
        // FX entry + nested clip.
        let entry = &back.fx_clips[0];
        assert!(entry.hidden, "fx hidden survives the round trip");
        assert_eq!((entry.figure, entry.node), (0, 2));
        assert_eq!(entry.clip.kind, ClipKind::Sparkle);
        assert_eq!(entry.clip.color2, Some(0x112233));
        assert_eq!(
            (entry.clip.start, entry.clip.duration, entry.clip.seed, entry.clip.angle, entry.clip.length),
            (1.0, 16.0, 77, 15.0, 140.0)
        );
        // Particle event.
        let event = &back.particles[0];
        assert_eq!(
            (event.frame, event.figure, event.node, event.kind, event.color, event.seed),
            (2, Some(0), Some(4), ParticleKind::Smoke, 0xABCDEF, 1234)
        );
        assert_eq!((event.x, event.y), (200.0, 150.0));
        // Movieclip.
        let clip = &back.clips[0];
        assert_eq!(clip.name, "wave");
        assert_eq!(clip.loop_mode, ClipLoop::PingPong);
        assert_eq!(clip.frames.len(), 2);
        assert_eq!(clip.frames[1].root, [20.0, 0.0]);
        // Graph channel + keys (Channel and Key are PartialEq).
        assert_eq!(back.channels.len(), 1);
        assert_eq!(back.channels[0], project.channels[0]);
        // Weather.
        let weather = back.weather.expect("weather survives");
        assert_eq!(
            (weather.kind, weather.amount, weather.wind),
            (WeatherKind::Storm, 140.0, 30.0)
        );
        // Assets / sprites / texts.
        assert_eq!(back.assets[0].name, "dot.png");
        assert_eq!(back.assets[0].png, vec![0x89, 0x50, 0x4E, 0x47]);
        assert_eq!(back.sprites[0].asset, 0);
        assert_eq!(back.sprites[0].keys[0].scale, 1.5);
        assert_eq!(back.texts[0].content, "Title");
        assert_eq!(back.texts[0].keys[0].x, 100.0);
    }

    #[test]
    fn project_json_missing_the_original_six_fields_loads_with_defaults() {
        // Wave 11 (E7): the six original Project fields were
        // the last ones in the document model without a serde
        // default. Strip them from a serialised project whose
        // values differ from the defaults, so the defaults —
        // not the planted values — are what can come back.
        let mut project = Project::new_demo();
        project.name = "Custom Name".to_string();
        project.fps = 24.0;
        project.stage = [800.0, 600.0];
        project.background = 0x101010;
        let mut value = serde_json::to_value(&project).unwrap();
        let object = value.as_object_mut().unwrap();
        for field in ["name", "fps", "stage", "background", "figures", "frames"] {
            assert!(
                object.remove(field).is_some(),
                "serialised project carries {field}"
            );
        }
        let restored: Project = serde_json::from_value(value)
            .expect("a project file missing the original six fields must load");
        assert_eq!(restored.name, "Untitled");
        assert_eq!(restored.fps, 12.0);
        assert_eq!(restored.stage, [960.0, 720.0]);
        assert_eq!(restored.background, 0x232830);
        assert!(restored.figures.is_empty());
        assert!(restored.frames.is_empty());
    }

    #[test]
    fn pose_round_trips_root_and_angles() {
        // set_pose / get_pose move whole poses; pin the
        // smallest unit they exchange.
        let pose = Pose {
            root: [123.0, 456.0],
            angles: vec![0.0, 90.0, -45.5, 180.0],
        };
        let json = serde_json::to_string(&pose).unwrap();
        let back: Pose = serde_json::from_str(&json).unwrap();
        assert_eq!(back.root, pose.root);
        assert_eq!(back.angles, pose.angles);
    }
}

#[cfg(test)]
mod wave12_c4_serde_pins {
    //! Wave 12 (C4): saved-file meaning pins for the style /
    //! media / asset types. Wave 12 unifies the STYLE_PRESETS
    //! tables (C2) and adds an asset surface (B1); neither may
    //! change what an existing project file means. Preset
    //! values are stored inline per node (`NodeDef.style`), so
    //! files are insulated from table edits — the two things
    //! that CAN silently shift meaning are the `Default` impl
    //! below (used by the serde default on `NodeDef.style` and
    //! by `FigureDef::humanoid`) and the asset index space
    //! (`SegmentMedia::Image` / `SpriteInstance.asset` index
    //! into `Project.assets`, so order is semantics). Both are
    //! pinned here.
    use super::*;

    #[test]
    fn wave12_node_style_default_values_are_pinned_literally() {
        // The exact values Wave 12's refactors must not drift:
        // taper 1.0, Round cap, no gradient colour, opacity 1.0.
        let style = NodeStyle::default();
        assert_eq!(style.taper, 1.0);
        assert_eq!(style.cap, CapKind::Round);
        assert_eq!(style.color2, None);
        assert_eq!(style.opacity, 1.0);
        assert_eq!(CapKind::default(), CapKind::Round);
        // The wire shape of the default is pinned too: a saved
        // default-styled node must keep serialising identically.
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(
            json,
            r#"{"taper":1.0,"cap":"Round","color2":null,"opacity":1.0}"#
        );
    }

    #[test]
    fn wave12_node_style_round_trips_every_field_combination() {
        // Every CapKind x color2 (None / black / coloured) x
        // taper extreme x opacity extreme must survive JSON
        // unchanged — these are the values panel presets write
        // inline into saved files.
        let caps = [CapKind::Round, CapKind::Square, CapKind::Point];
        let color2s = [None, Some(0x000000), Some(0xFF8800)];
        let tapers = [0.0f32, 0.2, 1.0];
        let opacities = [0.0f32, 0.35, 1.0];
        for cap in caps {
            for color2 in color2s {
                for taper in tapers {
                    for opacity in opacities {
                        let style = NodeStyle { taper, cap, color2, opacity };
                        let json = serde_json::to_string(&style).unwrap();
                        let back: NodeStyle = serde_json::from_str(&json).unwrap();
                        assert_eq!(back, style, "round trip changed {style:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn wave12_node_without_style_or_media_loads_the_defaults() {
        // The real back-compatibility path: files saved before
        // per-node style/media existed carry neither key, and
        // `NodeDef`'s serde defaults must supply exactly the
        // pinned default style and no media.
        let node: NodeDef = serde_json::from_str(
            r#"{"name":"n","parent":null,"length":10.0,"rest_angle":0.0,"head":false,"bend":1.0}"#,
        )
        .unwrap();
        assert_eq!(node.style, NodeStyle::default());
        assert_eq!(node.media, None);
    }

    #[test]
    fn wave12_node_style_partial_object_is_rejected_today() {
        // Pins CURRENT behaviour, deliberately: NodeStyle's
        // fields carry no individual serde defaults, so a
        // partial style object fails to load rather than
        // silently filling from defaults. Serde always writes
        // all four fields, so no saved file can be partial —
        // but if a future change adds per-field defaults, this
        // test fails and forces that to be a conscious,
        // reviewed change to saved-file meaning.
        let partial = serde_json::from_str::<NodeStyle>(r#"{"taper":0.5}"#);
        assert!(partial.is_err(), "partial NodeStyle unexpectedly loaded");
    }

    #[test]
    fn wave12_image_asset_png_bytes_round_trip_byte_identically() {
        // PNG magic + every byte value + a longer pseudo-random
        // run: asset bytes are payload, any mutation corrupts
        // the image a saved index points at.
        let mut png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend(0u8..=255);
        let mut state: u32 = 0x12345678;
        for _ in 0..4096 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            png.push((state >> 24) as u8);
        }
        let asset = ImageAsset { name: "hero.png".to_string(), png: png.clone() };
        let json = serde_json::to_string(&asset).unwrap();
        let back: ImageAsset = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "hero.png");
        assert_eq!(back.png.len(), png.len());
        assert_eq!(back.png, png, "PNG bytes changed in round trip");
        // An empty payload is degenerate but must also survive.
        let empty = ImageAsset { name: "empty.png".to_string(), png: Vec::new() };
        let back: ImageAsset =
            serde_json::from_str(&serde_json::to_string(&empty).unwrap()).unwrap();
        assert_eq!(back.name, "empty.png");
        assert!(back.png.is_empty());
    }

    #[test]
    fn wave12_segment_media_round_trips_both_variants() {
        // The variant tag and field names are the wire format
        // every saved file uses; the asset index must come back
        // exactly, including index 0 (the first asset).
        for media in [
            SegmentMedia::Image { asset: 0 },
            SegmentMedia::Image { asset: 7 },
            SegmentMedia::Text { content: "héllo \"world\"".to_string(), size: 14.5 },
        ] {
            let json = serde_json::to_string(&media).unwrap();
            let back: SegmentMedia = serde_json::from_str(&json).unwrap();
            assert_eq!(back, media, "round trip changed {media:?}");
        }
        let json = serde_json::to_string(&SegmentMedia::Image { asset: 2 }).unwrap();
        assert_eq!(json, r#"{"Image":{"asset":2}}"#);
    }

    #[test]
    fn wave12_project_with_assets_media_and_sprites_round_trips_in_order() {
        // Assets order is index meaning: segment media and
        // sprite instances refer to assets BY INDEX, so the
        // table must come back in the same order with the same
        // bytes, and every reference must still point at the
        // same asset. Serialised with to_string_pretty — the
        // exact format project files are saved in
        // (projects.rs `save_project`).
        let mut project = Project::new_demo();
        project.assets = vec![
            ImageAsset { name: "first.png".to_string(), png: vec![1, 2, 3] },
            ImageAsset { name: "second.png".to_string(), png: vec![4, 5] },
            ImageAsset { name: "third.png".to_string(), png: vec![6, 7, 8, 9] },
        ];
        // A non-default style and both media kinds, stored
        // inline on nodes as the panel writes them.
        let styled = NodeStyle {
            taper: 0.2,
            cap: CapKind::Square,
            color2: Some(0x00FF00),
            opacity: 0.5,
        };
        project.figures[0].def.nodes[1].style = styled;
        project.figures[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 2 });
        project.figures[0].def.nodes[2].media = Some(SegmentMedia::Text {
            content: "label".to_string(),
            size: 12.0,
        });
        // Sprites reference assets in non-table order on
        // purpose: reordering either side breaks the pairing.
        project.sprites = vec![
            SpriteInstance {
                asset: 2,
                keys: vec![InstanceKey {
                    frame: 0.0,
                    x: 10.0,
                    y: 20.0,
                    scale: 1.5,
                    angle: 30.0,
                }],
            },
            SpriteInstance {
                asset: 0,
                keys: vec![
                    InstanceKey {
                        frame: 0.0,
                        x: 0.0,
                        y: 0.0,
                        scale: 1.0,
                        angle: 0.0,
                    },
                    InstanceKey {
                        frame: 9.0,
                        x: 50.0,
                        y: 60.0,
                        scale: 2.0,
                        angle: -45.0,
                    },
                ],
            },
        ];

        let json = serde_json::to_string_pretty(&project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        // Project has no PartialEq; a byte-identical second
        // serialisation is the full structural equality proof.
        let json2 = serde_json::to_string_pretty(&back).unwrap();
        assert_eq!(json, json2, "project changed across a save/load cycle");

        // Targeted semantic pins on top of the byte proof.
        assert_eq!(back.assets.len(), 3);
        let names: Vec<&str> = back.assets.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["first.png", "second.png", "third.png"]);
        assert_eq!(back.assets[0].png, [1, 2, 3]);
        assert_eq!(back.assets[1].png, [4, 5]);
        assert_eq!(back.assets[2].png, [6, 7, 8, 9]);
        assert_eq!(back.figures[0].def.nodes[1].style, styled);
        assert_eq!(
            back.figures[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 2 })
        );
        assert_eq!(
            back.assets[2].name, "third.png",
            "node media index 2 must still mean the third asset"
        );
        assert_eq!(
            back.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Text { content: "label".to_string(), size: 12.0 })
        );
        assert_eq!(back.sprites.len(), 2);
        assert_eq!(back.sprites[0].asset, 2);
        assert_eq!(back.sprites[1].asset, 0);
        assert_eq!(back.sprites[1].keys.len(), 2);
        assert_eq!(back.sprites[1].keys[1].frame, 9.0);
        assert_eq!(back.sprites[1].keys[1].x, 50.0);
        assert_eq!(back.sprites[0].keys[0].scale, 1.5);
    }

    #[test]
    fn wave12_empty_project_loads_with_empty_asset_tables() {
        // Default/empty behaviour pinned: a bare object (every
        // Project field defaulted) and a fresh demo project
        // both carry no assets, sprites or texts — B1's asset
        // surface starts from this empty state and must not
        // invent entries.
        let bare: Project = serde_json::from_str("{}").unwrap();
        assert!(bare.assets.is_empty());
        assert!(bare.sprites.is_empty());
        assert!(bare.texts.is_empty());
        let demo = Project::new_demo();
        assert!(demo.assets.is_empty());
        assert!(demo.sprites.is_empty());
        assert!(demo.texts.is_empty());
    }
}
