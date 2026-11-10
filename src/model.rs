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

/// Point at fraction `t` (0..=1) of the circular arc from `a`
/// to `b` whose total bend is `bend_rad` — the arc [`bend_mid`]
/// samples at t = 0.5 (Wave Y, Y9 — GAP proposal P-11, the
/// strip-warp spine sampler). `bend_mid`'s offset is the
/// sagitta of exactly this circle, so the closed form is:
/// with chord length L, unit chord direction d, left normal
/// n (bend_mid's convention) and theta = `bend_rad`,
/// P(t) = mid + d * L*sin((t-1/2)*theta) / (2 sin(theta/2))
///      + n * L*sin(t*theta/2)*sin((1-t)*theta/2) / sin(theta/2).
/// The product form keeps both terms numerically stable as
/// theta -> 0 (no difference of near-equal cosines scaled by
/// a huge radius). Exactness anchors: t <= 0 returns `a`,
/// t >= 1 returns `b`, and t == 0.5 delegates to `bend_mid`
/// verbatim — a consumer sampling the arc at K = 2 gets
/// bend_mid's own point, bit for bit. Zero bend and a
/// degenerate chord fall back to the chord lerp. The domain
/// is bend_mid's own: |bend_rad| < 2*pi.
pub fn bend_arc_point(a: V2, b: V2, bend_rad: f32, t: f32) -> V2 {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    if t == 0.5 {
        return bend_mid(a, b, bend_rad);
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if bend_rad == 0.0 || len < 0.001 {
        return [a[0] + d[0] * t, a[1] + d[1] * t];
    }
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let dir = [d[0] / len, d[1] / len];
    let n = [-dir[1], dir[0]];
    let half = bend_rad / 2.0;
    let s = half.sin();
    let along = len * ((t - 0.5) * bend_rad).sin() / (2.0 * s);
    let out = len * (t * half).sin() * ((1.0 - t) * half).sin() / s;
    [
        mid[0] + dir[0] * along + n[0] * out,
        mid[1] + dir[1] * along + n[1] * out,
    ]
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

fn default_media_scale() -> f32 {
    1.0
}

fn media_offset_is_default(offset: &[f32; 2]) -> bool {
    *offset == [0.0, 0.0]
}

fn media_scale_is_default(scale: &f32) -> bool {
    *scale == 1.0
}

fn media_angle_is_default(angle_offset: &f32) -> bool {
    *angle_offset == 0.0
}

fn media_flip_is_default(flip: &bool) -> bool {
    !*flip
}

fn default_media_opacity() -> f32 {
    1.0
}

fn media_opacity_is_default(opacity: &f32) -> bool {
    *opacity == 1.0
}

/// Media attached to a segment: a sprite from the project's asset
/// table, or a text label drawn along the segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SegmentMedia {
    Image {
        asset: usize,
        /// Local transform (Wave Y, P-8): where the image
        /// sits relative to the segment it rides, on top of
        /// the segment's own placement — composed in the one
        /// placement funnel (`media::media_items`), so the
        /// stage placeholder, the raster exporter and the SVG
        /// exporter all follow together. The four fields
        /// below are that transform; each serde-defaults to
        /// the identity component and is skipped on write
        /// while it holds it, so a pre-transform file (whose
        /// `Image` payloads name only `asset`) loads as the
        /// identity transform, and a project whose media all
        /// carry the identity serialises byte-identically to
        /// a pre-transform file.
        ///
        /// Translation of the image centre, in stage units,
        /// expressed in the segment's own frame: `offset[0]`
        /// along the segment direction (parent end → node
        /// end), `offset[1]` along its normal.
        #[serde(default, skip_serializing_if = "media_offset_is_default")]
        offset: [f32; 2],
        /// Uniform multiplier on the fitted quad (1.0 = the
        /// historical fit: width = segment length, height =
        /// width / asset aspect).
        #[serde(default = "default_media_scale", skip_serializing_if = "media_scale_is_default")]
        scale: f32,
        /// Extra rotation of the image about its own centre,
        /// in degrees (stage space, y-down — the sprite-key
        /// angle convention), applied after the offset.
        #[serde(default, skip_serializing_if = "media_angle_is_default")]
        angle_offset: f32,
        /// Mirror the image along the segment axis: the end
        /// of the image that sat at the parent end now sits
        /// at the node end. (Composed as a reversed source u
        /// range — see `media::media_items`.)
        #[serde(default, skip_serializing_if = "media_flip_is_default")]
        flip: bool,
        /// The image's own opacity (Wave Z, Z5): a fifth
        /// payload field beside the transform, serde-defaulted
        /// to 1.0 and skipped on write while it holds 1.0, so
        /// pre-opacity files load fully opaque and projects
        /// whose media are all opaque serialise byte-identically
        /// to before. The one placement funnel multiplies it
        /// into the item alpha (figure opacity × carrier style
        /// opacity × this value, each clamped) — see
        /// `media::media_items`. This is the home the layered
        /// importer (P-9) stores a part's manifest opacity in.
        #[serde(
            default = "default_media_opacity",
            skip_serializing_if = "media_opacity_is_default"
        )]
        opacity: f32,
    },
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
    /// Joint limits on this node's RELATIVE angle (degrees, the
    /// `Pose::angles` quantity): the segment may not be solved
    /// outside the arc from `min_angle` to `max_angle`. `None`
    /// on a side leaves that side open; both `None` = no limit
    /// (the historical behaviour, and what every pre-limit
    /// project file deserialises to). Enforced where angles are
    /// SOLVED — [`aim_node`], [`apply_ik`] and graph Angle
    /// channels — never by rewriting stored poses. See
    /// [`NodeDef::clamp_angle`] for the exact arc semantics.
    #[serde(default)]
    pub min_angle: Option<f32>,
    #[serde(default)]
    pub max_angle: Option<f32>,
}

impl NodeDef {
    /// Clamp a relative joint angle (degrees) to this node's
    /// joint limits.
    ///
    /// With no limits the angle is returned UNCHANGED — not
    /// even normalised — so an unlimited joint behaves exactly
    /// as it did before limits existed, on every surface.
    ///
    /// With at least one bound set the angle is first
    /// normalised to (−180, 180], then:
    ///
    /// * both bounds, `min <= max`: a plain clamp to
    ///   `[min, max]` (`min == max` pins the joint);
    /// * both bounds, `min > max`: the allowed set is the
    ///   wrap arc `[min, 180] ∪ (−180, max]` — the arc that
    ///   runs clockwise from `min` through ±180 to `max` —
    ///   and a value in the forbidden arc takes the NEARER
    ///   endpoint;
    /// * a single bound clamps only its own side of the
    ///   normalised circle.
    pub fn clamp_angle(&self, angle: f32) -> f32 {
        if !self.has_limits() {
            return angle;
        }
        let v = norm_deg(angle);
        match (self.min_angle, self.max_angle) {
            (Some(lo), Some(hi)) => {
                let (lo, hi) = (norm_deg(lo), norm_deg(hi));
                if lo <= hi {
                    v.clamp(lo, hi)
                } else if v >= lo || v <= hi {
                    v
                } else if (v - hi) <= (lo - v) {
                    hi
                } else {
                    lo
                }
            }
            (Some(lo), None) => {
                let lo = norm_deg(lo);
                if v < lo {
                    lo
                } else {
                    v
                }
            }
            (None, Some(hi)) => {
                let hi = norm_deg(hi);
                if v > hi {
                    hi
                } else {
                    v
                }
            }
            (None, None) => unreachable!("guarded by has_limits above"),
        }
    }

    /// True when this node carries any joint limit.
    pub fn has_limits(&self) -> bool {
        self.min_angle.is_some() || self.max_angle.is_some()
    }

    /// The anatomical one-way range (min, max) the
    /// humanoid constructor ships on its elbows (Wave X,
    /// P-4): flexion to +160°, a 10° grace past straight.
    /// One source for the range: `FigureDef::humanoid`
    /// writes it through this constant and the segment
    /// inspector's preset row applies it (Wave Y, Y12) —
    /// the two can never drift apart.
    pub const ELBOW_LIMITS: (f32, f32) = (-10.0, 160.0);

    /// The anatomical one-way range (min, max) the
    /// humanoid constructor ships on its knees: flexion
    /// to −160°, a 10° grace past straight. See
    /// [`NodeDef::ELBOW_LIMITS`] for the one-source rule.
    pub const KNEE_LIMITS: (f32, f32) = (-160.0, 10.0);
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
            min_angle: None,
            max_angle: None,
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
        // One-way joint presets (Wave X, P-4): elbows flex
        // one way, knees the other, and neither hyperextends.
        // In this model's relative angles, IK flexion lands
        // the forearm at POSITIVE angles and the shin at
        // NEGATIVE ones on both sides (the bend signs above
        // produce exactly that), so: elbows travel
        // [-10, +160], knees [-160, +10]. The 10° grace past
        // straight keeps this constructor's own rest pose
        // (forearm splay ±4°, shin splay ±5°) inside its
        // limits. Only the solvers and graph channels enforce
        // these — stored poses are never rewritten.
        for i in [4, 6] {
            nodes[i].min_angle = Some(NodeDef::ELBOW_LIMITS.0);
            nodes[i].max_angle = Some(NodeDef::ELBOW_LIMITS.1);
        }
        for i in [8, 10] {
            nodes[i].min_angle = Some(NodeDef::KNEE_LIMITS.0);
            nodes[i].max_angle = Some(NodeDef::KNEE_LIMITS.1);
        }
        FigureDef { nodes }
    }

    /// A two-segment-limb quadruped (dog/horse-like), root at the rear
    /// of the spine, head at the front. Side view, facing right.
    pub fn quadruped() -> Self {
        let n = |name: &str, parent: Option<usize>, length: f32, rest: f32| NodeDef {
            min_angle: None,
            max_angle: None,
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
            min_angle: None,
            max_angle: None,
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

    /// The rig's rest (setup) angles, one per node, in node
    /// order — the vector form of the `rest_angle` fields.
    pub fn rest_angles(&self) -> Vec<f32> {
        self.nodes.iter().map(|n| n.rest_angle).collect()
    }

    /// Overwrite the rest angles from `angles`, zipped by node
    /// (callers pass exactly one angle per node), and return
    /// the angles that were replaced. Only the `rest_angle`
    /// fields change.
    pub fn set_rest_angles(&mut self, angles: &[f32]) -> Vec<f32> {
        let old = self.rest_angles();
        for (node, a) in self.nodes.iter_mut().zip(angles.iter()) {
            node.rest_angle = *a;
        }
        old
    }
}

/// How [`Project::set_rest_from_pose`] treats the stored
/// animation when a figure's rest (setup) pose is replaced
/// (Wave Y, P-10; GAP2 §3.1's two modes, user-chosen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetRestMode {
    /// Shift every authored angle in the three angle stores
    /// (`Frame.poses`, rig-matching `ClipDef.frames`, and the
    /// figure's `Angle` channels) by the per-node rest delta
    /// `new − old`, so every pose keeps its offset from the
    /// rest stance: the animation's content is preserved
    /// against the new rig, a frame that held the old rest
    /// pose now holds the new one, and the evaluated poses
    /// re-anchor rigidly (each node's relative angle moves by
    /// exactly the delta). GAP2 §3.1(a) writes the shift as
    /// `(old_rest − new_rest)` "so existing animation is
    /// visually unchanged" — that is the delta-stored-pose
    /// formula; this engine's poses are absolute (GAP2 §2.1),
    /// where the shift that preserves the animation against
    /// the stance is `new − old`, and `(old − new)` would move
    /// every evaluated pose by −delta and preserve nothing.
    /// See `~/workspace/wave-y/findings/y7.md` §3.
    Rebase,
    /// Set the rest angles and leave every stored angle
    /// untouched (the behaviour an importer or constructor
    /// write always had, now an explicit choice): evaluated
    /// playback is bit-identical, and only the rest stance
    /// itself — Reset's target, future seeded poses, clip-fit
    /// padding — changes, its evaluated world positions moving
    /// by exactly the rest delta.
    Absolute,
}

/// What [`Project::set_rest_from_pose`] did, for status lines
/// and pins: the rest angles before and after, the per-node
/// delta the rebase applied, and how many of the project's
/// clips and channels the rebase carried (always 0 in
/// `Absolute` mode).
#[derive(Clone, Debug, PartialEq)]
pub struct SetRestOutcome {
    pub old_rest: Vec<f32>,
    pub new_rest: Vec<f32>,
    pub delta: Vec<f32>,
    pub clips_rebased: usize,
    pub channels_rebased: usize,
}

/// Add `delta` (one entry per node) onto a pose's stored
/// angles, in place, WITHOUT re-wrapping — the graph module's
/// rule for angle values, and what makes the rebase's offset
/// invariant hold by exact addition. A pose shorter than the
/// delta (a misfit store) shifts only the angles it has.
fn shift_pose_angles(pose: &mut Pose, delta: &[f32]) {
    for (a, d) in pose.angles.iter_mut().zip(delta.iter()) {
        *a += d;
    }
}

/// Was `clip` authored on the rig `fig_def` whose rest angles
/// are `old_rest`? Clip creation clones the figure's def into
/// the clip, so an authored clip matches bit for bit: same
/// node count, same parent map, and its def's rest angles
/// exactly the figure's old rest. A clip from another rig —
/// or the same shape carrying a different rest — is not this
/// figure's animation and never rebases with it. (Two figures
/// sharing one identical rig share this identity: a clip
/// authored on either is authored on the rig.)
fn clip_rig_matches(fig_def: &FigureDef, old_rest: &[f32], clip: &ClipDef) -> bool {
    clip.def.nodes.len() == fig_def.nodes.len()
        && clip
            .def
            .nodes
            .iter()
            .zip(fig_def.nodes.iter())
            .all(|(c, f)| c.parent == f.parent)
        && clip.def.nodes.iter().map(|c| c.rest_angle).eq(old_rest.iter().copied())
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

    let abs = absolute_angles(def, pose);
    let parent_abs = |idx: usize, abs: &[f32]| -> f32 {
        match def.nodes[idx].parent {
            None => 0.0,
            Some(p) => abs[p],
        }
    };
    // Joint limits (Wave X, P-4): the mid joint's solved
    // relative angle is clamped FIRST, and the mid position
    // the end solve aims from is recomputed from the clamped
    // angle, so a limited mid never leaves the chain in a
    // state `evaluate` would draw differently — segment
    // lengths hold exactly, the end just lands where the
    // clamped chain can reach. With no limits on the mid,
    // `clamp_angle` returns the historical relative angle
    // and `dir` of the recomputed absolute is the original
    // `mid_abs` direction: the unlimited solve is unchanged.
    let mid_rel = def.nodes[mid].clamp_angle(norm_deg(mid_abs - parent_abs(mid, &abs)));
    pose.angles[mid] = mid_rel;
    let mid_pos = add(a, scale(dir(parent_abs(mid, &abs) + mid_rel), l1));
    let end_abs = angle_of(sub(target, mid_pos));
    // Recompute absolutes after the mid write before setting the end.
    let abs2 = absolute_angles(def, pose);
    pose.angles[end] = def.nodes[end].clamp_angle(norm_deg(end_abs - abs2[mid]));
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
    // Joint limits (Wave X, P-4): the solved relative angle is
    // clamped to the node's limits; an unlimited node gets the
    // historical value back unchanged from `clamp_angle`.
    pose.angles[node] =
        def.nodes[node].clamp_angle(norm_deg(want_abs - abs[parent]));
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
    /// Animation-state keys (Wave X, P-1): when a key is in force
    /// at the current timeline frame it picks the clip and the
    /// local playback time (see `StateKey`), so one figure can
    /// switch clips — idle → walk → attack — across a scene.
    /// Before the first key, and whenever the vec is empty,
    /// `clip` governs exactly as before. Projects saved before
    /// this field existed deserialize with no keys.
    #[serde(default)]
    pub states: Vec<StateKey>,
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

/// One animation-state key on a figure instance's own timeline
/// (Wave X, P-1): at timeline frame `frame` the figure switches
/// to clip `clip`, starting `offset` frames into the clip and
/// playing it back at `speed`× the timeline rate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateKey {
    /// Timeline frame at which this state takes over.
    pub frame: usize,
    /// Index into `Project::clips`.
    pub clip: usize,
    /// Frames into the clip at which playback starts when the
    /// state begins. Keys written before this field existed, or
    /// authored without it, start at the clip's first frame.
    #[serde(default)]
    pub offset: usize,
    /// Playback rate multiplier on the clip's clock (1.0 = the
    /// timeline rate). Sanitised at read in `state_local_time`.
    #[serde(default = "default_state_speed")]
    pub speed: f32,
}

fn default_state_speed() -> f32 {
    1.0
}

/// The state key in force on `inst` at timeline time `f`: the key
/// with the greatest `frame` that is not past `f` — a key takes
/// over AT its frame. Keys need not be stored sorted; ties on
/// `frame` resolve to the later key in the vec, so authoring
/// order is the override order. `None` when the figure has no
/// keys or `f` precedes the first key.
pub fn active_state_key(inst: &FigureInstance, f: f32) -> Option<&StateKey> {
    let mut best: Option<&StateKey> = None;
    for key in &inst.states {
        if (key.frame as f32) <= f {
            let better = match best {
                None => true,
                Some(b) => key.frame >= b.frame,
            };
            if better {
                best = Some(key);
            }
        }
    }
    best
}

/// Local clip time for `key` at timeline time `f`:
/// `(f − key.frame) · speed + key.offset`, with the elapsed part
/// clamped at 0. The speed is sanitised at read — a non-finite
/// or non-positive speed plays at 1.0, and a finite speed is
/// capped at 1e6 — so a hand-edited or corrupt project can never
/// push the local time into NaN/∞ territory (which would NaN
/// the tween lerp in `apply_clips`).
pub fn state_local_time(key: &StateKey, f: f32) -> f32 {
    let speed = if key.speed.is_finite() && key.speed > 0.0 {
        key.speed.min(1.0e6)
    } else {
        1.0
    };
    let elapsed = (f - key.frame as f32).max(0.0);
    elapsed * speed + key.offset as f32
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

fn default_key_opacity() -> f32 {
    1.0
}

fn default_key_pivot() -> f32 {
    0.5
}

/// One keyframed transform for a sprite/text instance.
///
/// The enrichment fields (Wave X, P-6) ride on every key with
/// serde defaults that reproduce the pre-enrichment behaviour
/// exactly, so a project file written before they existed loads
/// unchanged: opacity 1, no flips, the pivot at the image centre
/// (where the passes always anchored), and `scale_y` 0.0 — the
/// follow-`scale` value defined by [`InstanceKey::eff_scale_y`].
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InstanceKey {
    pub frame: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub angle: f32,
    /// Key alpha (Stick Nodes' per-record style opacity).
    /// Evaluation clamps it to 0..=1.
    #[serde(default = "default_key_opacity")]
    pub opacity: f32,
    /// Mirror the image around the pivot on the x axis (the
    /// format's flipX flag; a negative scale is never stored —
    /// `scale` keeps the magnitude, the flip rides here).
    #[serde(default)]
    pub flip_x: bool,
    /// Mirror around the pivot on the y axis (flipY).
    #[serde(default)]
    pub flip_y: bool,
    /// Second-axis scale (the format's scaleY magnitude).
    /// 0.0 means the axis follows `scale` — the only value a
    /// pre-enrichment file can imply; see `eff_scale_y`.
    #[serde(default)]
    pub scale_y: f32,
    /// Anchor point, image-relative (0.5 = centre, where every
    /// pass anchored before the field existed). The key's
    /// position is where this point of the image sits.
    #[serde(default = "default_key_pivot")]
    pub pivot_x: f32,
    #[serde(default = "default_key_pivot")]
    pub pivot_y: f32,
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
    /// The second-axis scale in effect: the stored `scale_y`
    /// when it is a positive value, else `scale` (a stored 0.0 —
    /// every pre-enrichment key — means the axis follows the
    /// uniform scale; a NaN stored value falls back the same
    /// way, since NaN is not > 0.0).
    pub fn eff_scale_y(&self) -> f32 {
        if self.scale_y > 0.0 {
            self.scale_y
        } else {
            self.scale
        }
    }

    /// Linear interpolation of a key list at `frame_f` (clamped).
    ///
    /// Opacity, `scale_y` and the pivot interpolate linearly
    /// like position/scale/angle — `scale_y` interpolating the
    /// EFFECTIVE values (`eff_scale_y` per endpoint), so a
    /// segment between a follow-`scale` key and an explicit one
    /// still sweeps the axis the passes will draw; read a
    /// sampled key's `scale_y` through `eff_scale_y`, as every
    /// consumer does. The flips are discrete: a segment yields
    /// its start key's flips, switching to the end key's exactly
    /// at t = 1 — the frame the continuous fields arrive there.
    /// Clamped samples (before the first / after the last key)
    /// return the stored key verbatim, as they always have.
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
                let (ay, by) = (a.eff_scale_y(), b.eff_scale_y());
                let (flip_x, flip_y) = if t >= 1.0 {
                    (b.flip_x, b.flip_y)
                } else {
                    (a.flip_x, a.flip_y)
                };
                return Some(InstanceKey {
                    frame: frame_f,
                    x: a.x + (b.x - a.x) * t,
                    y: a.y + (b.y - a.y) * t,
                    scale: a.scale + (b.scale - a.scale) * t,
                    angle: a.angle + (b.angle - a.angle) * t,
                    opacity: a.opacity + (b.opacity - a.opacity) * t,
                    flip_x,
                    flip_y,
                    // Both endpoints following `scale` (the
                    // scale_y <= 0 sentinel) means the sample
                    // follows too: keep the sentinel, or the
                    // enrichment readout loses its "(follows
                    // Scale)" suffix on sampled keys (the
                    // dopesheet lane reads samples only).
                    // eff_scale_y() is unaffected — `scale`
                    // itself is lerped with the same t, which
                    // is exactly the value resolved here before.
                    scale_y: if a.scale_y <= 0.0 && b.scale_y <= 0.0 {
                        0.0
                    } else {
                        ay + (by - ay) * t
                    },
                    pivot_x: a.pivot_x + (b.pivot_x - a.pivot_x) * t,
                    pivot_y: a.pivot_y + (b.pivot_y - a.pivot_y) * t,
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

/// The on-disk project format version this build writes
/// ([`Project::format_version`]). Version 1 is the format
/// as it stood when versioning was introduced (Wave P):
/// every field carries a serde default, so a version-1
/// file may omit any field and still load. Bump this only
/// for a change that a loader cannot absorb through serde
/// defaults alone, and land the migration for older
/// versions in the same change.
pub const FORMAT_VERSION_CURRENT: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    /// Format version this project was written in (see
    /// [`FORMAT_VERSION_CURRENT`]). Files saved before the
    /// field existed carry no value and load as
    /// `FORMAT_VERSION_CURRENT`: the pre-versioning format
    /// IS version 1, because every field such a file may
    /// omit already has a serde default.
    #[serde(default = "default_format_version")]
    pub format_version: u32,
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
    /// Subtitle cues over the timeline (see subtitles.rs).
    /// Files saved before this field existed carry no value
    /// and load with no cues.
    #[serde(default)]
    pub subtitles: Vec<crate::subtitles::SubtitleCue>,
    /// How subtitle cues are presented (see subtitles.rs).
    /// Files saved before this field existed carry no value
    /// and load with the default style — bottom-anchored,
    /// unscaled, white text over a black backing.
    #[serde(default)]
    pub subtitle_style: crate::subtitles::SubtitleStyle,
}

fn default_format_version() -> u32 {
    FORMAT_VERSION_CURRENT
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
            states: Vec::new(),
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
            format_version: FORMAT_VERSION_CURRENT,
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
            subtitles: Vec::new(),
            subtitle_style: Default::default(),
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
    /// clip poses at frame time `f`. A state key in force on the
    /// instance (Wave X, P-1) picks the clip and the local time;
    /// without one, the instance's `clip` governs at local time
    /// `f` — the original phase-locked behaviour.
    fn apply_clips(&self, poses: &mut [Pose], f: f32) {
        for (fi, inst) in self.figures.iter().enumerate() {
            let (ci, local) = match active_state_key(inst, f) {
                Some(key) => (key.clip, state_local_time(key, f)),
                None => match inst.clip {
                    Some(ci) => (ci, f),
                    None => continue,
                },
            };
            let Some(clip) = self.clips.get(ci) else { continue };
            if clip.frames.is_empty() || fi >= poses.len() {
                continue;
            }
            let n = clip.frames.len();
            let base = local.floor() as usize;
            let idx = clip_frame_index(clip.loop_mode, base, n);
            poses[fi] = if self.tween_playback && n > 1 {
                let t = self.easing.apply(local - local.floor());
                let next = clip_frame_index(clip.loop_mode, base + 1, n);
                clip.frames[idx].lerp(&clip.frames[next], t)
            } else {
                clip.frames[idx].clone()
            };
        }
    }

    /// Replace figure `fi`'s rest (setup) angles with
    /// `pose`'s angles (Wave Y, Y7, P-10; GAP2 §3.1). The
    /// pose's root is ignored — rest is angles only, and no
    /// root anywhere moves. In [`SetRestMode::Rebase`] the
    /// per-node delta (`new − old`, shortest path) is added
    /// WITHOUT re-wrapping onto every stored angle in the
    /// three angle stores — `Frame.poses`, the frames of
    /// every clip authored on this figure's rig (see
    /// `clip_rig_matches`), and the figure's `Angle` graph
    /// channels (key values only; tangents, being slopes,
    /// are invariant under a constant shift) — so every
    /// pose keeps its offset from the rest stance and the
    /// animation's content is preserved against the new
    /// rig. In [`SetRestMode::Absolute`] only the rest
    /// angles change and evaluated playback is identical.
    /// State keys carry no angles and are never touched:
    /// state-driven poses ride the shift through the clip
    /// store itself.
    ///
    /// Refuses — `None`, no mutation — on an unknown
    /// figure, an angle count that mismatches the rig, or
    /// any non-finite source angle. New rest angles are
    /// stored through [`norm_deg`].
    pub fn set_rest_from_pose(
        &mut self,
        fi: usize,
        pose: &Pose,
        mode: SetRestMode,
    ) -> Option<SetRestOutcome> {
        let fig = self.figures.get(fi)?;
        let n = fig.def.nodes.len();
        if pose.angles.len() != n || !pose.angles.iter().all(|a| a.is_finite()) {
            return None;
        }
        let old_rest = fig.def.rest_angles();
        let new_rest: Vec<f32> = pose.angles.iter().map(|a| norm_deg(*a)).collect();
        let delta: Vec<f32> = new_rest
            .iter()
            .zip(old_rest.iter())
            .map(|(nw, od)| norm_deg(nw - od))
            .collect();
        let mut clips_rebased = 0usize;
        let mut channels_rebased = 0usize;
        if mode == SetRestMode::Rebase {
            for frame in self.frames.iter_mut() {
                if let Some(p) = frame.poses.get_mut(fi) {
                    shift_pose_angles(p, &delta);
                }
            }
            let fig_def = self.figures[fi].def.clone();
            for clip in self.clips.iter_mut() {
                if clip_rig_matches(&fig_def, &old_rest, clip) {
                    for p in clip.frames.iter_mut() {
                        shift_pose_angles(p, &delta);
                    }
                    clip.def.set_rest_angles(&new_rest);
                    clips_rebased += 1;
                }
            }
            for ch in self.channels.iter_mut() {
                if ch.figure == fi {
                    if let crate::graph::ChannelTarget::Angle(node) = ch.target {
                        if let Some(d) = delta.get(node) {
                            let d = *d;
                            for k in ch.keys.iter_mut() {
                                k.value += d;
                            }
                            channels_rebased += 1;
                        }
                    }
                }
            }
        }
        self.figures[fi].def.set_rest_angles(&new_rest);
        Some(SetRestOutcome {
            old_rest,
            new_rest,
            delta,
            clips_rebased,
            channels_rebased,
        })
    }

    /// Total playback length in seconds, honouring frame durations.
    pub fn total_seconds(&self) -> f32 {
        self.timeline_beats() / self.fps.max(1.0)
    }

    /// The timeline's length in beats — the one duration-weighted
    /// time definition the editor, transport timecode and export
    /// sampling all share (Roadmap Wave 1, RM7 / F-6). One beat is
    /// one frame period at the project fps; a frame lasts
    /// `duration` beats. Durations are floored at 0.05 — the floor
    /// playback has always applied — and a non-finite duration
    /// counts as one beat, so a corrupt stored value can neither
    /// erase a frame nor poison every total derived from it.
    pub fn timeline_beats(&self) -> f32 {
        self.frames.iter().map(|f| Self::duration_beats(f)).sum()
    }

    fn duration_beats(frame: &Frame) -> f32 {
        if frame.duration.is_finite() {
            frame.duration.max(0.05)
        } else {
            1.0
        }
    }

    /// Timeline beats elapsed at fractional authored-frame time
    /// `frame_time` (frame `i` spans `[i, i + 1)` in this
    /// coordinate): each whole frame before it contributes its
    /// duration, and the frame it lands in contributes the
    /// matching fraction of its own duration. Times at or past
    /// the end read as the whole timeline.
    pub fn beats_at_frame_time(&self, frame_time: f32) -> f32 {
        if self.frames.is_empty() || !frame_time.is_finite() || frame_time <= 0.0 {
            return 0.0;
        }
        let mut beats = 0.0;
        for (i, frame) in self.frames.iter().enumerate() {
            let duration = Self::duration_beats(frame);
            if frame_time < (i + 1) as f32 {
                return beats + (frame_time - i as f32) * duration;
            }
            beats += duration;
        }
        beats
    }

    /// Fractional authored-frame time at `beats` beats into the
    /// timeline — the inverse of [`Project::beats_at_frame_time`],
    /// walking each frame's duration in turn so a tick that
    /// crosses a boundary is charged at each frame's own rate.
    /// Beats at or past the end read as the last frame.
    pub fn frame_time_at_beats(&self, beats: f32) -> f32 {
        if self.frames.is_empty() || !beats.is_finite() || beats <= 0.0 {
            return 0.0;
        }
        let mut rest = beats;
        for (i, frame) in self.frames.iter().enumerate() {
            let duration = Self::duration_beats(frame);
            if rest < duration {
                return i as f32 + rest / duration;
            }
            rest -= duration;
        }
        (self.frames.len() - 1) as f32
    }

    /// Seconds elapsed at fractional authored-frame time
    /// `frame_time`, on the shared duration-weighted clock.
    pub fn seconds_at_frame_time(&self, frame_time: f32) -> f32 {
        self.beats_at_frame_time(frame_time) / self.fps.max(1.0)
    }

    /// Fractional authored-frame time at `seconds` seconds into
    /// the timeline, on the shared duration-weighted clock.
    pub fn frame_time_at_seconds(&self, seconds: f32) -> f32 {
        self.frame_time_at_beats(seconds * self.fps.max(1.0))
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
            states: Vec::new(),
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
    fn bend_arc_point_anchors_and_shape() {
        // Wave Y (Y9, GAP P-11): the strip-warp spine sampler.
        // Anchors are EXACT: the endpoints are the segment's
        // own joints and the midpoint is bend_mid verbatim —
        // a K = 2 sampling of the arc must reproduce the
        // two-piece split the line renderer has always made.
        let a = [100.0, 100.0];
        let b = [200.0, 100.0];
        for bend in [0.9, -0.9, 2.0] {
            assert_eq!(bend_arc_point(a, b, bend, 0.0), a, "t=0 is a");
            assert_eq!(bend_arc_point(a, b, bend, 1.0), b, "t=1 is b");
            assert_eq!(
                bend_arc_point(a, b, bend, 0.5),
                bend_mid(a, b, bend),
                "t=0.5 is bend_mid verbatim"
            );
        }
        // Zero bend is the chord itself.
        assert_eq!(bend_arc_point(a, b, 0.0, 0.25), [125.0, 100.0]);
        assert_eq!(bend_arc_point(a, b, 0.0, 0.75), [175.0, 100.0]);
        // A positive bend bulges toward bend_mid's left
        // normal (here +y, screen-down): quarter points sit
        // off the chord on that side, symmetric about the
        // midpoint, and lower than the midpoint's own bulge.
        let q1 = bend_arc_point(a, b, 2.0, 0.25);
        let q3 = bend_arc_point(a, b, 2.0, 0.75);
        let mid = bend_arc_point(a, b, 2.0, 0.5);
        assert!(q1[1] > 100.0 && q3[1] > 100.0, "bulge side: {q1:?} {q3:?}");
        assert!(
            (q1[1] - q3[1]).abs() < 0.01,
            "quarter offsets symmetric: {q1:?} {q3:?}"
        );
        assert!(
            q1[1] < mid[1],
            "quarter bulge below the midpoint's: {q1:?} vs {mid:?}"
        );
        // The quarter points also advance along the chord.
        assert!(q1[0] > 100.0 && q1[0] < 150.0, "q1 advances: {q1:?}");
        assert!(q3[0] > 150.0 && q3[0] < 200.0, "q3 advances: {q3:?}");
        // A negative bend bulges the other way.
        let nq = bend_arc_point(a, b, -2.0, 0.25);
        assert!(nq[1] < 100.0, "negative bend flips the bulge: {nq:?}");
        assert!(
            (nq[1] - (200.0 - q1[1])).abs() < 0.01,
            "negative bend mirrors the positive arc: {nq:?} vs {q1:?}"
        );
    }

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
        project.fps = 24.0;
        project.frames[0].duration = 2.0;
        project.frames[1].duration = 3.0;
        let expanded = crate::export::expanded_frames(&project);
        assert_eq!(expanded.len(), 2 + 3 + 1 + 1 + 1);
        assert!((project.total_seconds() - 8.0 / 24.0).abs() < 0.001);
    }

    #[test]
    fn timeline_clock_walks_fractional_durations() {
        // Roadmap Wave 1 (RM7 / F-6): the shared clock charges
        // each frame its own fractional duration — a 1.5-beat
        // hold followed by a 0.5-beat hold is a 2-beat span in
        // which the second frame starts at beat 1.5, not at a
        // rounded boundary.
        let mut project = Project::new_demo();
        project.fps = 24.0;
        project.frames[0].duration = 1.5;
        project.frames[1].duration = 0.5;
        project.frames[2].duration = 2.0;
        project.frames[3].duration = 1.0;
        project.frames[4].duration = 1.0;
        assert!((project.timeline_beats() - 6.0).abs() < 1e-6);
        assert!((project.total_seconds() - 6.0 / 24.0).abs() < 1e-6);
        // Beat 1.5 is exactly the second frame's start; beat
        // 1.75 is halfway through its half-beat hold.
        assert!((project.frame_time_at_beats(1.5) - 1.0).abs() < 1e-6);
        assert!((project.frame_time_at_beats(1.75) - 1.5).abs() < 1e-6);
        // The inverse map agrees, in beats and in seconds.
        assert!((project.beats_at_frame_time(1.5) - 1.75).abs() < 1e-6);
        assert!(
            (project.seconds_at_frame_time(1.5) - 1.75 / 24.0).abs() < 1e-6,
            "seconds at frame 1.5: {}",
            project.seconds_at_frame_time(1.5)
        );
        assert!(
            (project.frame_time_at_seconds(1.75 / 24.0) - 1.5).abs() < 1e-6,
            "frame time at 1.75 beats: {}",
            project.frame_time_at_seconds(1.75 / 24.0)
        );
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

    /// A limit-less node def for the clamp pins: the
    /// `simple()` constructor's root, reparented freely by
    /// the tests (only its limit fields are read).
    fn unpinned_node() -> NodeDef {
        FigureDef::simple().nodes.into_iter().next().unwrap()
    }

    #[test]
    fn clamp_angle_without_limits_returns_the_input_unchanged() {
        // Wave X (P-4): the unlimited path must not even
        // normalise — every historical surface (solvers,
        // graph channels) keeps its exact values.
        let node = unpinned_node();
        assert!(!node.has_limits());
        assert_eq!(node.clamp_angle(350.0), 350.0);
        assert_eq!(node.clamp_angle(-190.0), -190.0);
        assert_eq!(node.clamp_angle(45.0), 45.0);
    }

    #[test]
    fn clamp_angle_both_bounds_wrap_arc_and_single_sides() {
        let mut node = unpinned_node();
        node.min_angle = Some(-10.0);
        node.max_angle = Some(160.0);
        assert_eq!(node.clamp_angle(90.0), 90.0, "inside passes through");
        assert_eq!(node.clamp_angle(170.0), 160.0, "above max clamps down");
        assert_eq!(node.clamp_angle(-45.0), -10.0, "below min clamps up");
        // Unnormalised input is normalised first: 200° is -160°.
        assert_eq!(node.clamp_angle(200.0), -10.0);
        // min == max pins the joint.
        node.min_angle = Some(45.0);
        node.max_angle = Some(45.0);
        assert_eq!(node.clamp_angle(0.0), 45.0);
        assert_eq!(node.clamp_angle(120.0), 45.0);
        // Wrap arc (min > max): the allowed set runs clockwise
        // from min through ±180 to max; a forbidden value takes
        // the NEARER endpoint (a tie takes max, by the `<=`).
        node.min_angle = Some(170.0);
        node.max_angle = Some(-170.0);
        assert_eq!(node.clamp_angle(175.0), 175.0, "inside the wrap arc");
        assert_eq!(node.clamp_angle(-175.0), -175.0, "inside the wrap arc");
        assert_eq!(node.clamp_angle(90.0), 170.0, "nearer min");
        assert_eq!(node.clamp_angle(-90.0), -170.0, "nearer max");
        assert_eq!(node.clamp_angle(0.0), -170.0, "exact tie takes max");
        // Single bounds clamp only their own side.
        node.min_angle = Some(30.0);
        node.max_angle = None;
        assert_eq!(node.clamp_angle(0.0), 30.0);
        assert_eq!(node.clamp_angle(90.0), 90.0);
        node.min_angle = None;
        node.max_angle = Some(-30.0);
        assert_eq!(node.clamp_angle(0.0), -30.0);
        assert_eq!(node.clamp_angle(-90.0), -90.0);
    }

    #[test]
    fn node_def_without_limit_fields_deserialises_unlimited() {
        // Project files written before joint limits existed
        // carry no min/max keys at all; both must read None.
        let node: NodeDef = serde_json::from_value(serde_json::json!({
            "name": "n",
            "parent": null,
            "length": 10.0,
            "rest_angle": 0.0,
            "head": false,
            "bend": 1.0,
        }))
        .unwrap();
        assert_eq!(node.min_angle, None);
        assert_eq!(node.max_angle, None);
        // And a limited def round-trips its limits exactly.
        let def = FigureDef::humanoid();
        let text = serde_json::to_string(&def).unwrap();
        let back: FigureDef = serde_json::from_str(&text).unwrap();
        assert_eq!(back.nodes[4].min_angle, Some(-10.0));
        assert_eq!(back.nodes[4].max_angle, Some(160.0));
        assert_eq!(back.nodes[8].min_angle, Some(-160.0));
        assert_eq!(back.nodes[8].max_angle, Some(10.0));
    }

    #[test]
    fn humanoid_joint_presets_are_one_way_and_hold_the_rest_pose() {
        let def = FigureDef::humanoid();
        for i in [4, 6] {
            assert_eq!(def.nodes[i].min_angle, Some(-10.0), "elbow {i} min");
            assert_eq!(def.nodes[i].max_angle, Some(160.0), "elbow {i} max");
        }
        for i in [8, 10] {
            assert_eq!(def.nodes[i].min_angle, Some(-160.0), "knee {i} min");
            assert_eq!(def.nodes[i].max_angle, Some(10.0), "knee {i} max");
        }
        for (i, node) in def.nodes.iter().enumerate() {
            if ![4, 6, 8, 10].contains(&i) {
                assert!(!node.has_limits(), "node {i} stays unlimited");
            }
            // The rest pose must sit inside its own limits —
            // a preset the figure itself violates would fight
            // every solve back toward the bounds.
            assert_eq!(
                node.clamp_angle(node.rest_angle),
                node.rest_angle,
                "rest angle of node {i} inside its limits"
            );
        }
    }

    #[test]
    fn aim_node_respects_joint_limits() {
        let limited = |min: f32, max: f32| NodeDef {
            min_angle: Some(min),
            max_angle: Some(max),
            name: "limb".into(),
            parent: Some(0),
            length: 40.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let def = FigureDef {
            nodes: vec![
                NodeDef {
                    min_angle: None,
                    max_angle: None,
                    name: "root".into(),
                    parent: None,
                    length: 0.0,
                    rest_angle: 0.0,
                    head: false,
                    bend: 1.0,
                    thickness: 5.0,
                    color: None,
                    ik_end: false,
                    shape: Shape::Line,
                    style: NodeStyle::default(),
                    media: None,
                },
                limited(-30.0, 30.0),
            ],
        };
        let mut pose = def.rest_pose([0.0, 0.0]);
        // Aiming straight up asks for a 180° relative angle;
        // the joint stops at its +30° limit.
        assert!(aim_node(&def, &mut pose, 1, [0.0, -40.0]));
        assert!((pose.angles[1] - 30.0).abs() < 0.01, "got {}", pose.angles[1]);
        // Aiming left asks for -90°; the joint stops at -30°.
        assert!(aim_node(&def, &mut pose, 1, [-40.0, 0.0]));
        assert!((pose.angles[1] + 30.0).abs() < 0.01, "got {}", pose.angles[1]);
        // Inside the limits the aim is exact: straight down is 0°.
        assert!(aim_node(&def, &mut pose, 1, [0.0, 40.0]));
        assert!(pose.angles[1].abs() < 0.01, "got {}", pose.angles[1]);
    }

    #[test]
    fn apply_ik_respects_joint_limits_and_keeps_lengths() {
        // A two-bone chain pointing straight down at rest.
        // The unconstrained solve for a target beside the
        // base folds the mid to ≈+130° and the end to
        // ≈-170°; the limits below cut both, and the chain
        // must stay coherent (lengths exact) afterwards.
        let node = |name: &str, parent: Option<usize>, min: Option<f32>, max: Option<f32>| NodeDef {
            min_angle: min,
            max_angle: max,
            name: name.into(),
            parent,
            length: 40.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let def = FigureDef {
            nodes: vec![
                node("root", None, None, None),
                node("mid", Some(0), None, Some(100.0)),
                node("end", Some(1), Some(-90.0), None),
            ],
        };
        let mut pose = def.rest_pose([0.0, 0.0]);
        assert!(apply_ik(&def, &mut pose, 2, [5.0, 5.0]));
        assert!(
            (pose.angles[1] - 100.0).abs() < 0.01,
            "mid clamped to its max, got {}",
            pose.angles[1]
        );
        assert!(
            (pose.angles[2] + 90.0).abs() < 0.01,
            "end clamped to its min, got {}",
            pose.angles[2]
        );
        let pos = evaluate(&def, &pose);
        assert!((dist(pos[0], pos[1]) - 40.0).abs() < 0.05, "mid length holds");
        assert!((dist(pos[1], pos[2]) - 40.0).abs() < 0.05, "end length holds");
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

// ---- Wave X (P-1) animation states ----
//
// Separate module on purpose: exact-name imports only, never a
// glob. Pins the StateKey contract: a figure with state keys
// switches clips across its timeline; a figure without them is
// the legacy phase-locked movieclip, byte for byte.
#[cfg(test)]
mod state_key_tests {
    use super::{
        active_state_key, state_local_time, ClipDef, ClipLoop, FigureInstance, Project, StateKey,
    };

    fn key(frame: usize, clip: usize) -> StateKey {
        StateKey { frame, clip, offset: 0, speed: 1.0 }
    }

    fn inst_with(states: Vec<StateKey>) -> FigureInstance {
        let mut inst = Project::new_demo().figures[0].clone();
        inst.states = states;
        inst
    }

    fn clip_with_roots(project: &Project, name: &str, roots: &[f32]) -> ClipDef {
        let def = project.figures[0].def.clone();
        ClipDef {
            name: name.to_string(),
            def: def.clone(),
            frames: roots.iter().map(|x| def.rest_pose([*x, 0.0])).collect(),
            loop_mode: ClipLoop::Loop,
        }
    }

    /// The demo timeline has 5 frames (0..=4); poses_at clamps to it.
    fn roots(project: &Project) -> Vec<f32> {
        (0..5).map(|f| project.poses_at(f as f32)[0].root[0]).collect()
    }

    #[test]
    fn state_key_json_without_offset_and_speed_defaults() {
        let key: StateKey = serde_json::from_str(r#"{"frame": 2, "clip": 1}"#)
            .expect("a key without offset/speed must still parse");
        assert_eq!(key.offset, 0);
        assert_eq!(key.speed, 1.0);
        let full = StateKey { frame: 3, clip: 0, offset: 2, speed: 1.5 };
        let json = serde_json::to_string(&full).expect("key serializes");
        assert_eq!(serde_json::from_str::<StateKey>(&json).expect("key re-parses"), full);
    }

    #[test]
    fn figure_instance_json_without_states_loads_with_no_keys() {
        let mut project = Project::new_demo();
        project.figures[0].clip = Some(0);
        let inst = project.figures[0].clone();
        let mut value = serde_json::to_value(&inst).expect("instance serializes");
        value.as_object_mut().expect("instance is an object").remove("states");
        let loaded: FigureInstance =
            serde_json::from_value(value).expect("pre-state instance JSON must still parse");
        assert!(loaded.states.is_empty(), "missing states field defaults to no keys");
        assert_eq!(loaded.clip, Some(0));
    }

    #[test]
    fn active_state_key_picks_the_latest_key_not_past_f() {
        // Deliberately unsorted: resolution must not depend on
        // storage order.
        let inst = inst_with(vec![key(5, 1), key(2, 0)]);
        let at = |f: f32| active_state_key(&inst, f).map(|k| k.clip);
        assert_eq!(at(0.0), None);
        assert_eq!(at(1.99), None);
        assert_eq!(at(2.0), Some(0), "a key takes over AT its frame");
        assert_eq!(at(4.99), Some(0));
        assert_eq!(at(5.0), Some(1));
        assert_eq!(at(100.0), Some(1));
        // Equal frames: the later vec entry wins (authoring order
        // is the override order).
        let tied = inst_with(vec![key(3, 0), key(3, 1)]);
        assert_eq!(active_state_key(&tied, 3.0).map(|k| k.clip), Some(1));
        // No keys at all: never a state.
        assert_eq!(active_state_key(&inst_with(vec![]), 3.0), None);
    }

    #[test]
    fn state_local_time_applies_offset_then_speed() {
        let k = StateKey { frame: 2, clip: 0, offset: 3, speed: 2.0 };
        assert_eq!(state_local_time(&k, 2.0), 3.0, "at the key frame only the offset counts");
        assert_eq!(state_local_time(&k, 3.0), 5.0);
        assert_eq!(state_local_time(&k, 4.5), 8.0);
        let slow = StateKey { frame: 2, clip: 0, offset: 3, speed: 0.5 };
        assert_eq!(state_local_time(&slow, 6.0), 5.0);
        // Before the key frame the elapsed part clamps at 0.
        assert_eq!(state_local_time(&k, 1.0), 3.0);
    }

    #[test]
    fn state_local_time_sanitises_corrupt_speeds() {
        let at = |speed: f32| {
            state_local_time(&StateKey { frame: 2, clip: 0, offset: 0, speed }, 4.0)
        };
        assert_eq!(at(0.0), 2.0, "zero speed plays at 1.0");
        assert_eq!(at(-2.0), 2.0, "no reverse playback: negative plays at 1.0");
        assert_eq!(at(f32::NAN), 2.0);
        assert_eq!(at(f32::INFINITY), 2.0);
        let huge = state_local_time(&StateKey { frame: 2, clip: 0, offset: 0, speed: 1.0e30 }, 3.0);
        assert_eq!(huge, 1.0e6, "finite speeds are capped at 1e6");
        assert!(huge.is_finite());
    }

    #[test]
    fn poses_at_switches_clips_at_state_keys() {
        let mut project = Project::new_demo();
        project.clips.push(clip_with_roots(&project, "idle", &[10.0, 20.0, 30.0]));
        project.clips.push(clip_with_roots(&project, "run", &[100.0, 200.0]));
        project.figures[0].states = vec![key(0, 0), key(3, 1)];
        assert_eq!(roots(&project), vec![10.0, 20.0, 30.0, 100.0, 200.0]);
    }

    #[test]
    fn poses_at_state_offset_and_speed_reclock_the_clip() {
        let mut project = Project::new_demo();
        project.clips.push(clip_with_roots(&project, "spin", &[10.0, 20.0, 30.0, 40.0]));
        project.figures[0].states =
            vec![StateKey { frame: 0, clip: 0, offset: 1, speed: 2.0 }];
        // Local times 1, 3, 5, 7, 9 over a 4-frame loop → 1, 3, 1, 3, 1.
        assert_eq!(roots(&project), vec![20.0, 40.0, 20.0, 40.0, 20.0]);
    }

    #[test]
    fn poses_at_before_the_first_key_falls_back_to_the_legacy_clip() {
        let mut project = Project::new_demo();
        project.clips.push(clip_with_roots(&project, "legacy", &[7.0, 8.0]));
        project.clips.push(clip_with_roots(&project, "state", &[100.0]));
        project.figures[0].clip = Some(0);
        project.figures[0].states = vec![key(3, 1)];
        assert_eq!(roots(&project), vec![7.0, 8.0, 7.0, 100.0, 100.0]);
    }

    #[test]
    fn poses_at_dangling_state_clip_keeps_the_timeline_pose() {
        let mut project = Project::new_demo();
        project.figures[0].states = vec![key(0, 9)];
        // Clip 9 does not exist: the figure is skipped, exactly as
        // a dangling legacy `clip` is, and the demo timeline's own
        // poses stand.
        assert_eq!(roots(&project), vec![480.0, 494.0, 508.0, 494.0, 480.0]);
    }

    #[test]
    fn poses_at_tween_follows_the_state_clock() {
        let mut project = Project::new_demo();
        project.tween_playback = true;
        project.clips.push(clip_with_roots(&project, "glide", &[0.0, 100.0]));
        project.figures[0].states =
            vec![StateKey { frame: 0, clip: 0, offset: 0, speed: 2.0 }];
        // f = 0.25 at speed 2 is local 0.5: halfway between the
        // clip's two frames.
        assert_eq!(project.poses_at(0.25)[0].root[0], 50.0);
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
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
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
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
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
            SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 },
            SegmentMedia::Image { asset: 7, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 },
            SegmentMedia::Text { content: "héllo \"world\"".to_string(), size: 14.5 },
        ] {
            let json = serde_json::to_string(&media).unwrap();
            let back: SegmentMedia = serde_json::from_str(&json).unwrap();
            assert_eq!(back, media, "round trip changed {media:?}");
        }
        let json = serde_json::to_string(&SegmentMedia::Image { asset: 2, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }).unwrap();
        assert_eq!(json, r#"{"Image":{"asset":2}}"#);
    }

    #[test]
    fn wave_y_segment_media_transform_serde_defaults_and_skips() {
        // Wave Y (Y3, GAP P-8): a pre-transform file — the
        // `Image` payload names only `asset` — loads with the
        // identity local transform...
        let back: SegmentMedia =
            serde_json::from_str(r#"{"Image":{"asset":2}}"#).unwrap();
        assert_eq!(
            back,
            SegmentMedia::Image {
                asset: 2,
                offset: [0.0, 0.0],
                scale: 1.0,
                angle_offset: 0.0,
                flip: false,
                opacity: 1.0,
            }
        );
        // ...and writes back byte-identically (every transform
        // field skips while it holds its identity value), so
        // untransformed projects see no save-file churn.
        assert_eq!(serde_json::to_string(&back).unwrap(), r#"{"Image":{"asset":2}}"#);
        // A non-identity transform round-trips exactly.
        let m = SegmentMedia::Image {
            asset: 1,
            offset: [3.0, -2.0],
            scale: 2.0,
            angle_offset: 45.0,
            flip: true,
            opacity: 1.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<SegmentMedia>(&json).unwrap(), m);
        // A partial transform object fills its remaining
        // fields from the defaults, and re-writes with only
        // the non-default field present.
        let back: SegmentMedia =
            serde_json::from_str(r#"{"Image":{"asset":1,"scale":2.0}}"#).unwrap();
        assert_eq!(
            back,
            SegmentMedia::Image {
                asset: 1,
                offset: [0.0, 0.0],
                scale: 2.0,
                angle_offset: 0.0,
                flip: false,
                opacity: 1.0,
            }
        );
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            r#"{"Image":{"asset":1,"scale":2.0}}"#
        );
    }

    #[test]
    fn wave_z_segment_media_opacity_serde_default_and_skip() {
        // Wave Z (Z5): the payload's own opacity defaults
        // to 1.0 — a pre-opacity file (transform fields
        // only, or `asset` alone) loads fully opaque...
        let back: SegmentMedia =
            serde_json::from_str(r#"{"Image":{"asset":2,"scale":2.0}}"#).unwrap();
        assert_eq!(
            back,
            SegmentMedia::Image {
                asset: 2,
                offset: [0.0, 0.0],
                scale: 2.0,
                angle_offset: 0.0,
                flip: false,
                opacity: 1.0,
            }
        );
        // ...and an opaque payload writes back without the
        // field, byte-identical to a pre-opacity file.
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            r#"{"Image":{"asset":2,"scale":2.0}}"#
        );
        // A non-default opacity round-trips exactly and is
        // the only extra field written.
        let m = SegmentMedia::Image {
            asset: 2,
            offset: [0.0, 0.0],
            scale: 1.0,
            angle_offset: 0.0,
            flip: false,
            opacity: 0.5,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, r#"{"Image":{"asset":2,"opacity":0.5}}"#);
        assert_eq!(serde_json::from_str::<SegmentMedia>(&json).unwrap(), m);
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
            Some(SegmentMedia::Image { asset: 2, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
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
                    opacity: 1.0,
                    flip_x: false,
                    flip_y: false,
                    scale_y: 0.0,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
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
                        opacity: 1.0,
                        flip_x: false,
                        flip_y: false,
                        scale_y: 0.0,
                        pivot_x: 0.5,
                        pivot_y: 0.5,
                    },
                    InstanceKey {
                        frame: 9.0,
                        x: 50.0,
                        y: 60.0,
                        scale: 2.0,
                        angle: -45.0,
                        opacity: 1.0,
                        flip_x: false,
                        flip_y: false,
                        scale_y: 0.0,
                        pivot_x: 0.5,
                        pivot_y: 0.5,
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
            Some(SegmentMedia::Image { asset: 2, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 })
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

#[cfg(test)]
mod wave13_c6_sprite_text_pins {
    //! Wave 13 (C6): the sprite/text data layer the A squad
    //! builds the new GUI surface on. Wave 12 pinned sprites
    //! as passengers in an assets project (two instances, no
    //! texts, no literal wire shapes) and Wave 11's E7 census
    //! planted one single-key sprite + text with spot checks;
    //! `InstanceKey::sample` — the interpolation every sprite
    //! and text placement reads (media.rs) — had no edge pins
    //! at all. Those are the gaps closed here.
    //!
    //! No shape changes: `InstanceKey`, `SpriteInstance` and
    //! `TextInstance` derive no `PartialEq`, so equality is
    //! proven the Wave 12 way — field-by-field asserts plus a
    //! byte-identical second serialisation. One sampling
    //! semantic is pinned deliberately because the A squad
    //! will trip over it otherwise: a CLAMPED sample (before
    //! the first key / after the last) returns the stored key
    //! verbatim, including its stored `frame`, while an
    //! INTERPOLATED sample carries the requested frame in
    //! its `frame` field. Consumers read x / y / scale /
    //! angle from the result, so the asymmetry is harmless
    //! today — but it is current behaviour, pinned as such.
    use super::*;

    fn key(frame: f32, x: f32, y: f32, scale: f32, angle: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x,
            y,
            scale,
            angle,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    /// Field-by-field key comparison (InstanceKey has no
    /// PartialEq). Every value used below is exactly
    /// representable in f32 and every interpolation factor
    /// is a binary fraction (1/4, 1/2, 1), so plain equality
    /// is the honest assert — no tolerance anywhere.
    fn assert_key(actual: &InstanceKey, frame: f32, x: f32, y: f32, scale: f32, angle: f32) {
        assert_eq!(actual.frame, frame, "frame differs in {actual:?}");
        assert_eq!(actual.x, x, "x differs in {actual:?}");
        assert_eq!(actual.y, y, "y differs in {actual:?}");
        assert_eq!(actual.scale, scale, "scale differs in {actual:?}");
        assert_eq!(actual.angle, angle, "angle differs in {actual:?}");
    }

    // ---- Serde: literal wire shapes -------------------------

    #[test]
    fn wave13_instance_key_wire_shape_is_pinned_literally() {
        // Field names, field order and f32 rendering are the
        // saved-file format for every sprite/text key.
        let json = serde_json::to_string(&key(2.5, -12.75, 8.0, 0.5, -45.0)).unwrap();
        // Wave X (X9, P-6): the key gained six serde-defaulted
        // enrichment fields (opacity, flips, scale_y, pivot),
        // serialised in declaration order after `angle`; old
        // five-field files still load (X9's legacy pin).
        assert_eq!(
            json,
            r#"{"frame":2.5,"x":-12.75,"y":8.0,"scale":0.5,"angle":-45.0,"opacity":1.0,"flip_x":false,"flip_y":false,"scale_y":0.0,"pivot_x":0.5,"pivot_y":0.5}"#
        );
        let back: InstanceKey = serde_json::from_str(&json).unwrap();
        assert_key(&back, 2.5, -12.75, 8.0, 0.5, -45.0);
    }

    #[test]
    fn wave13_sprite_instance_wire_shape_is_pinned_literally() {
        let sprite = SpriteInstance {
            asset: 2,
            keys: vec![key(0.0, 10.0, 20.0, 1.5, 30.0)],
        };
        let json = serde_json::to_string(&sprite).unwrap();
        assert_eq!(
            json,
            r#"{"asset":2,"keys":[{"frame":0.0,"x":10.0,"y":20.0,"scale":1.5,"angle":30.0,"opacity":1.0,"flip_x":false,"flip_y":false,"scale_y":0.0,"pivot_x":0.5,"pivot_y":0.5}]}"#
        );
        let back: SpriteInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(back.asset, 2);
        assert_eq!(back.keys.len(), 1);
        assert_key(&back.keys[0], 0.0, 10.0, 20.0, 1.5, 30.0);
    }

    #[test]
    fn wave13_text_instance_wire_shape_is_pinned_literally() {
        // `color` is a plain u32 on the wire (0xFFFFFF =
        // 16777215), fields in declaration order.
        let text = TextInstance {
            content: "Title".to_string(),
            size: 28.0,
            color: 0xFFFFFF,
            keys: vec![key(0.0, 100.0, 80.0, 1.0, 0.0)],
        };
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(
            json,
            r#"{"content":"Title","size":28.0,"color":16777215,"keys":[{"frame":0.0,"x":100.0,"y":80.0,"scale":1.0,"angle":0.0,"opacity":1.0,"flip_x":false,"flip_y":false,"scale_y":0.0,"pivot_x":0.5,"pivot_y":0.5}]}"#
        );
        let back: TextInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "Title");
        assert_eq!(back.size, 28.0);
        assert_eq!(back.color, 0xFFFFFF);
        assert_eq!(back.keys.len(), 1);
        assert_key(&back.keys[0], 0.0, 100.0, 80.0, 1.0, 0.0);
    }

    // ---- Serde: instance tables round-trip ------------------

    #[test]
    fn wave13_sprite_and_text_tables_round_trip_field_by_field() {
        // The degenerate members must survive alongside the
        // rich ones: an instance with NO keys yet (the state
        // A1's add op starts from before its first key, and a
        // legal saved state), an empty-content text, negative
        // and fractional values, and u32::MAX as a colour.
        let sprites = vec![
            SpriteInstance {
                asset: 3,
                keys: vec![
                    key(0.0, 0.0, 0.0, 1.0, 0.0),
                    key(9.5, -4.5, 12.25, 0.75, -270.0),
                ],
            },
            SpriteInstance { asset: 0, keys: Vec::new() },
        ];
        let texts = vec![
            TextInstance {
                content: "líne\nbreak \"quoted\"".to_string(),
                size: 0.5,
                color: u32::MAX,
                keys: vec![key(1.25, -0.5, 0.5, 2.0, 45.5)],
            },
            TextInstance {
                content: String::new(),
                size: 12.0,
                color: 0,
                keys: Vec::new(),
            },
        ];

        let json = serde_json::to_string(&sprites).unwrap();
        let back: Vec<SpriteInstance> = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].asset, 3);
        assert_eq!(back[0].keys.len(), 2);
        assert_key(&back[0].keys[1], 9.5, -4.5, 12.25, 0.75, -270.0);
        assert_eq!(back[1].asset, 0);
        assert!(back[1].keys.is_empty(), "a keyless sprite stays keyless");

        let json = serde_json::to_string(&texts).unwrap();
        let back: Vec<TextInstance> = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].content, "líne\nbreak \"quoted\"");
        assert_eq!(back[0].size, 0.5);
        assert_eq!(back[0].color, u32::MAX);
        assert_key(&back[0].keys[0], 1.25, -0.5, 0.5, 2.0, 45.5);
        assert_eq!(back[1].content, "");
        assert!(back[1].keys.is_empty(), "a keyless text stays keyless");
    }

    #[test]
    fn wave13_project_with_keyed_sprites_and_texts_round_trips_byte_stably() {
        // The full save path (to_string_pretty, as
        // projects.rs writes files): several sprites AND
        // texts, multi-key instances in table order,
        // fractional key frames, negative positions/angles —
        // and a byte-identical second save as the structural
        // equality proof (Project has no PartialEq).
        let mut project = Project::new_demo();
        project.assets = vec![
            ImageAsset { name: "first.png".to_string(), png: vec![1, 2, 3] },
            ImageAsset { name: "second.png".to_string(), png: vec![4, 5] },
        ];
        project.sprites = vec![
            SpriteInstance {
                asset: 1,
                keys: vec![
                    key(0.0, 480.0, 360.0, 1.0, 0.0),
                    key(12.0, 240.5, -30.0, 2.5, 180.0),
                    key(20.0, 0.0, 0.0, 0.5, -90.25),
                ],
            },
            SpriteInstance { asset: 0, keys: Vec::new() },
            SpriteInstance {
                asset: 1,
                keys: vec![key(3.5, -12.75, 8.25, 0.25, 359.5)],
            },
        ];
        project.texts = vec![
            TextInstance {
                content: "Héllo \"sprites\"".to_string(),
                size: 28.5,
                color: 0xFF8800,
                keys: vec![
                    key(0.0, 100.0, 80.0, 1.0, 0.0),
                    key(6.0, 110.5, 90.25, 1.25, -15.0),
                ],
            },
            TextInstance {
                content: String::new(),
                size: 12.0,
                color: 0,
                keys: Vec::new(),
            },
        ];

        let json = serde_json::to_string_pretty(&project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&back).unwrap();
        assert_eq!(json, json2, "project changed across a save/load cycle");

        // Order is semantics on both sides: instance order
        // in each table, key order inside each instance, and
        // asset indices still naming the same asset.
        assert_eq!(back.sprites.len(), 3);
        let sprite_assets: Vec<usize> = back.sprites.iter().map(|s| s.asset).collect();
        assert_eq!(sprite_assets, [1, 0, 1]);
        assert_eq!(back.assets[1].name, "second.png");
        assert_eq!(back.sprites[0].keys.len(), 3);
        assert_key(&back.sprites[0].keys[1], 12.0, 240.5, -30.0, 2.5, 180.0);
        assert_key(&back.sprites[0].keys[2], 20.0, 0.0, 0.0, 0.5, -90.25);
        assert!(back.sprites[1].keys.is_empty());
        assert_key(&back.sprites[2].keys[0], 3.5, -12.75, 8.25, 0.25, 359.5);
        assert_eq!(back.texts.len(), 2);
        assert_eq!(back.texts[0].content, "Héllo \"sprites\"");
        assert_eq!(back.texts[0].size, 28.5);
        assert_eq!(back.texts[0].color, 0xFF8800);
        assert_eq!(back.texts[0].keys.len(), 2);
        assert_key(&back.texts[0].keys[1], 6.0, 110.5, 90.25, 1.25, -15.0);
        assert_eq!(back.texts[1].content, "");
        assert!(back.texts[1].keys.is_empty());
    }

    #[test]
    fn wave13_project_file_without_sprites_or_texts_loads_them_empty() {
        // The back-compatibility path for files saved before
        // the fields existed (or hand-trimmed since): both
        // Project fields carry a serde default, so a document
        // with the two keys REMOVED loads with empty tables —
        // while the rest of the project survives untouched.
        // (Wave 12's bare-`{}` pin covers the all-defaults
        // case; this pins the sprite/text defaults inside a
        // populated document.)
        let mut project = Project::new_demo();
        project.sprites = vec![SpriteInstance {
            asset: 0,
            keys: vec![key(0.0, 1.0, 2.0, 1.0, 0.0)],
        }];
        project.texts = vec![TextInstance {
            content: "gone".to_string(),
            size: 12.0,
            color: 0,
            keys: Vec::new(),
        }];
        let mut doc = serde_json::to_value(&project).unwrap();
        let obj = doc.as_object_mut().unwrap();
        assert!(obj.remove("sprites").is_some(), "fixture must carry sprites");
        assert!(obj.remove("texts").is_some(), "fixture must carry texts");
        let back: Project = serde_json::from_value(doc).unwrap();
        assert!(back.sprites.is_empty());
        assert!(back.texts.is_empty());
        assert_eq!(back.name, project.name);
        assert_eq!(back.figures.len(), project.figures.len());
    }

    // ---- InstanceKey::sample edges --------------------------

    #[test]
    fn wave13_sample_of_empty_keys_is_none() {
        // media_items skips an instance whose sample is None,
        // so a keyless instance renders nothing — the A squad
        // relies on this when an add precedes its first key.
        assert!(InstanceKey::sample(&[], 0.0).is_none());
        assert!(InstanceKey::sample(&[], -10.0).is_none());
        assert!(InstanceKey::sample(&[], 123.5).is_none());
    }

    #[test]
    fn wave13_sample_of_single_key_is_that_key_at_any_frame() {
        // One key clamps the whole timeline to itself, and
        // the clamped result is the STORED key verbatim —
        // its `frame` stays 4.0 even when sampled at 999
        // (the asymmetry pinned in the module docs).
        let keys = [key(4.0, 50.0, 60.0, 2.0, 30.0)];
        for frame_f in [-100.0, 0.0, 4.0, 4.5, 999.0] {
            let got = InstanceKey::sample(&keys, frame_f).expect("one key always samples");
            assert_key(&got, 4.0, 50.0, 60.0, 2.0, 30.0);
        }
    }

    #[test]
    fn wave13_sample_before_first_and_after_last_clamps_to_the_end_keys() {
        let keys = [
            key(2.0, 10.0, 20.0, 1.0, 0.0),
            key(8.0, 110.0, 220.0, 3.0, 90.0),
        ];
        // Before (and exactly at) the first key: first key
        // verbatim, stored frame included.
        for frame_f in [-5.0, 0.0, 2.0] {
            let got = InstanceKey::sample(&keys, frame_f).unwrap();
            assert_key(&got, 2.0, 10.0, 20.0, 1.0, 0.0);
        }
        // After the last key: last key verbatim — NOT a
        // key re-framed to the sampled frame.
        let got = InstanceKey::sample(&keys, 1000.0).unwrap();
        assert_key(&got, 8.0, 110.0, 220.0, 3.0, 90.0);
    }

    #[test]
    fn wave13_sample_at_exact_key_frames_returns_those_keys() {
        let keys = [
            key(0.0, 0.0, 0.0, 1.0, 0.0),
            key(10.0, 100.0, 50.0, 2.0, 45.0),
            key(20.0, 300.0, 150.0, 4.0, 90.0),
        ];
        // First key: the early clamp branch.
        assert_key(&InstanceKey::sample(&keys, 0.0).unwrap(), 0.0, 0.0, 0.0, 1.0, 0.0);
        // Middle key: reached as t = 1 at the end of the
        // first window — values land exactly on the key.
        assert_key(&InstanceKey::sample(&keys, 10.0).unwrap(), 10.0, 100.0, 50.0, 2.0, 45.0);
        // Last key: t = 1 at the end of the last window.
        assert_key(&InstanceKey::sample(&keys, 20.0).unwrap(), 20.0, 300.0, 150.0, 4.0, 90.0);
    }

    #[test]
    fn wave13_sample_interpolates_position_scale_and_angle() {
        // Every interpolated field is pinned separately at
        // two factors (1/2 and 1/4), so a field accidentally
        // left un-lerped (copied from either end) fails here.
        let keys = [
            key(0.0, 0.0, 100.0, 1.0, 0.0),
            key(10.0, 200.0, -100.0, 3.0, 90.0),
        ];
        // Midpoint: x 0->200 = 100, y 100->-100 = 0,
        // scale 1->3 = 2, angle 0->90 = 45; the result
        // carries the REQUESTED frame, unlike a clamp.
        assert_key(&InstanceKey::sample(&keys, 5.0).unwrap(), 5.0, 100.0, 0.0, 2.0, 45.0);
        // Quarter: x = 50, y = 50, scale = 1.5, angle = 22.5.
        assert_key(&InstanceKey::sample(&keys, 2.5).unwrap(), 2.5, 50.0, 50.0, 1.5, 22.5);
    }

    #[test]
    fn wave13_sample_second_segment_interpolates_between_its_own_keys() {
        // A sample past the middle key must interpolate the
        // SECOND window, not stretch the first: at 15 the
        // values sit halfway between keys 2 and 3.
        let keys = [
            key(0.0, 0.0, 0.0, 1.0, 0.0),
            key(10.0, 100.0, 0.0, 2.0, 0.0),
            key(20.0, 300.0, 100.0, 4.0, 180.0),
        ];
        assert_key(&InstanceKey::sample(&keys, 15.0).unwrap(), 15.0, 200.0, 50.0, 3.0, 90.0);
    }

    #[test]
    fn wave13_sample_handles_fractional_key_frames() {
        // Keys need not sit on integer frames (sample takes
        // f32 for tween playback): the factor is measured
        // between the fractional endpoints.
        let keys = [
            key(2.5, 10.0, 0.0, 1.0, 0.0),
            key(7.5, 110.0, 50.0, 2.0, 60.0),
        ];
        // Exactly at the fractional first key: clamp branch.
        assert_key(&InstanceKey::sample(&keys, 2.5).unwrap(), 2.5, 10.0, 0.0, 1.0, 0.0);
        // Frame 5 is halfway between 2.5 and 7.5.
        assert_key(&InstanceKey::sample(&keys, 5.0).unwrap(), 5.0, 60.0, 25.0, 1.5, 30.0);
        // Past the fractional last key: last key verbatim.
        assert_key(&InstanceKey::sample(&keys, 8.0).unwrap(), 7.5, 110.0, 50.0, 2.0, 60.0);
    }
}

#[cfg(test)]
mod wave_x_p6_key_enrichment {
    //! Wave X (P-6): the enriched `InstanceKey` fields — serde
    //! defaults reproducing the pre-enrichment behaviour, the
    //! `eff_scale_y` follow-`scale` rule, and `sample`'s
    //! treatment of the new fields (continuous for opacity /
    //! effective scale_y / pivot, discrete for the flips).
    use super::*;

    fn bare_key(frame: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x: 0.0,
            y: 0.0,
            scale: 2.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    #[test]
    fn key_without_enrichment_fields_loads_with_todays_behaviour() {
        // A pre-enrichment key as an old project file stores it:
        // only the original five fields on the wire.
        let k: InstanceKey = serde_json::from_str(
            r#"{"frame": 3.0, "x": 10.0, "y": 20.0, "scale": 1.5, "angle": 30.0}"#,
        )
        .expect("legacy key parses");
        assert_eq!(k.opacity, 1.0);
        assert!(!k.flip_x && !k.flip_y);
        assert_eq!(k.scale_y, 0.0, "stored form keeps the follow-scale 0.0");
        assert_eq!(k.eff_scale_y(), 1.5, "the axis follows scale");
        assert_eq!((k.pivot_x, k.pivot_y), (0.5, 0.5), "centre anchor");
    }

    #[test]
    fn eff_scale_y_prefers_an_explicit_positive_value() {
        let mut k = bare_key(0.0);
        assert_eq!(k.eff_scale_y(), 2.0, "0.0 follows scale");
        k.scale_y = 0.25;
        assert_eq!(k.eff_scale_y(), 0.25, "explicit value wins");
        k.scale_y = -1.0;
        assert_eq!(k.eff_scale_y(), 2.0, "non-positive falls back");
        k.scale_y = f32::NAN;
        assert_eq!(k.eff_scale_y(), 2.0, "NaN falls back");
    }

    #[test]
    fn sample_interpolates_enrichment_and_steps_the_flips() {
        let a = InstanceKey {
            opacity: 0.0,
            flip_x: true,
            scale_y: 0.0, // follows a.scale = 2.0
            pivot_x: 0.0,
            pivot_y: 1.0,
            ..bare_key(0.0)
        };
        let b = InstanceKey {
            opacity: 1.0,
            flip_y: true,
            scale_y: 4.0,
            pivot_x: 1.0,
            pivot_y: 0.0,
            ..bare_key(10.0)
        };
        let mid = InstanceKey::sample(&[a, b], 5.0).expect("mid-segment sample");
        assert_eq!(mid.opacity, 0.5, "opacity lerps");
        assert_eq!(
            mid.scale_y, 3.0,
            "scale_y lerps the effective endpoints (2.0 -> 4.0)"
        );
        assert_eq!((mid.pivot_x, mid.pivot_y), (0.5, 0.5), "pivot lerps");
        assert!(
            mid.flip_x && !mid.flip_y,
            "mid-segment flips are the start key's"
        );
        let end = InstanceKey::sample(&[a, b], 10.0).expect("segment end sample");
        assert!(
            !end.flip_x && end.flip_y,
            "at t = 1 the flips are the end key's"
        );
        assert_eq!(end.opacity, 1.0);
        // Clamped samples return the stored key verbatim,
        // follow-scale 0.0 and all (the wave-13 C6 asymmetry).
        let clamped = InstanceKey::sample(&[a, b], -1.0).expect("clamped sample");
        assert_eq!(clamped.scale_y, 0.0, "stored form, not materialised");
        assert_eq!(clamped.eff_scale_y(), 2.0);
    }

    #[test]
    fn enriched_key_round_trips_through_json() {
        let k = InstanceKey {
            opacity: 0.25,
            flip_x: true,
            flip_y: true,
            scale_y: 0.75,
            pivot_x: 0.25,
            pivot_y: 0.75,
            ..bare_key(4.0)
        };
        let json = serde_json::to_string(&k).expect("key serialises");
        let back: InstanceKey = serde_json::from_str(&json).expect("key parses back");
        assert_eq!(back.opacity, 0.25);
        assert!(back.flip_x && back.flip_y);
        assert_eq!(back.scale_y, 0.75);
        assert_eq!((back.pivot_x, back.pivot_y), (0.25, 0.75));
    }
}

#[cfg(test)]
mod wave_y_p10_setup_pose_tests {
    use super::*;
    use crate::graph::{Channel, ChannelTarget, Key};

    fn json<T: serde::Serialize + ?Sized>(v: &T) -> String {
        serde_json::to_string(v).expect("serialises")
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// A pose holding `rest` with one node's angle moved.
    fn moved_pose(rest: &[f32], node: usize, add: f32) -> Pose {
        let mut angles = rest.to_vec();
        angles[node] += add;
        Pose {
            root: [480.0, 470.0],
            angles,
        }
    }

    fn authored_clip(p: &Project, name: &str) -> ClipDef {
        let def = p.figures[0].def.clone();
        let rest = def.rest_angles();
        ClipDef {
            name: name.into(),
            def: def.clone(),
            frames: vec![def.rest_pose([480.0, 470.0]), moved_pose(&rest, 5, 12.0)],
            loop_mode: ClipLoop::Loop,
        }
    }

    #[test]
    fn rebase_shifts_frames_clips_and_channels_by_the_rest_delta() {
        let mut p = Project::new_demo();
        let n = p.figures[0].def.nodes.len();
        // Frame 0 becomes the exact rest copy for this pin.
        let root = p.frames[0].poses[0].root;
        p.frames[0].poses[0] = p.figures[0].def.rest_pose(root);
        // Bends are not an angle store: plant some and pin
        // them untouched.
        p.frames[1].bends = vec![vec![0.3; n]];
        let bends_before = json(&p.frames[1].bends);
        // A second figure, for the other-figure channel.
        let mut second = p.figures[0].clone();
        second.name = "Second".into();
        p.figures.push(second);
        for f in p.frames.iter_mut() {
            let pose = f.poses[0].clone();
            f.poses.push(pose);
        }
        // Clips: one authored on the rig, one other-rig
        // (a node short), one same-shape/other-rest.
        p.clips.push(authored_clip(&p, "authored"));
        let mut other_rig = authored_clip(&p, "other-rig");
        other_rig.def.nodes.pop();
        p.clips.push(other_rig);
        let mut other_rest = authored_clip(&p, "other-rest");
        other_rest.def.nodes[3].rest_angle += 7.0;
        p.clips.push(other_rest);
        let other_rig_before = json(&p.clips[1]);
        let other_rest_before = json(&p.clips[2]);
        // Channels: the figure's Angle channel on node 5,
        // plus RootX / Length / other-figure channels that
        // must not move.
        p.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(5),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(4.0, 50.0)],
        });
        p.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 100.0)],
        });
        p.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Length(5),
            keys: vec![Key::flat(0.0, 40.0)],
        });
        p.channels.push(Channel {
            figure: 1,
            target: ChannelTarget::Angle(5),
            keys: vec![Key::flat(0.0, 20.0)],
        });
        let mid_before = p.channels[0].value_at(2.0);
        let bystanders_before = json(&p.channels[1..]);

        let old_rest = p.figures[0].def.rest_angles();
        let frame0_node2_before = p.frames[0].poses[0].angles[2];
        let frame1_node5_before = p.frames[1].poses[0].angles[5];
        let clip_frame1_before = p.clips[0].frames[1].angles[5];
        let new_pose = moved_pose(&old_rest, 5, 30.0);
        let out = p
            .set_rest_from_pose(0, &new_pose, SetRestMode::Rebase)
            .expect("rebase applies");
        assert_eq!(out.old_rest, old_rest);
        assert!(out
            .new_rest
            .iter()
            .zip(new_pose.angles.iter())
            .all(|(a, b)| approx(*a, norm_deg(*b))));
        let d5 = out.delta[5];
        assert!(approx(d5, 30.0), "delta carried verbatim: {d5}");
        assert_eq!(out.clips_rebased, 1);
        assert_eq!(out.channels_rebased, 1);
        // Frames shifted by delta; roots never move.
        assert!(approx(p.frames[1].poses[0].angles[5], frame1_node5_before + d5));
        assert!(approx(p.frames[0].poses[0].angles[2], frame0_node2_before + out.delta[2]));
        assert_eq!(p.frames[1].poses[0].root, [494.0, 470.0]);
        // Frame 0 (the rest copy) is now the new rest.
        assert!(p.frames[0].poses[0]
            .angles
            .iter()
            .zip(out.new_rest.iter())
            .all(|(a, b)| approx(*a, *b)));
        // Bends untouched.
        assert_eq!(json(&p.frames[1].bends), bends_before);
        // The authored clip shifted and took the new rest;
        // the other clips are byte-identical.
        assert!(approx(p.clips[0].frames[1].angles[5], clip_frame1_before + d5));
        assert_eq!(p.clips[0].def.rest_angles(), out.new_rest);
        assert_eq!(json(&p.clips[1]), other_rig_before);
        assert_eq!(json(&p.clips[2]), other_rest_before);
        // The Angle channel's values shifted by exactly the
        // node's delta, tangents untouched, mid-segment
        // value moved by delta.
        assert!(approx(p.channels[0].keys[0].value, 10.0 + d5));
        assert!(approx(p.channels[0].keys[1].value, 50.0 + d5));
        assert_eq!(p.channels[0].keys[0].in_tan, 0.0);
        assert!(approx(p.channels[0].value_at(2.0), mid_before + d5));
        // RootX / Length / other-figure channels identical.
        assert_eq!(json(&p.channels[1..]), bystanders_before);
    }

    #[test]
    fn rebase_preserves_the_animation_relative_to_the_stance() {
        let mut p = Project::new_demo();
        let def_before = p.figures[0].def.clone();
        let old_rest = def_before.rest_angles();
        let offsets_before: Vec<Vec<V2>> = p
            .frames
            .iter()
            .map(|f| {
                let pose = &f.poses[0];
                let off = Pose {
                    root: pose.root,
                    angles: pose
                        .angles
                        .iter()
                        .zip(old_rest.iter())
                        .map(|(a, r)| a - r)
                        .collect(),
                };
                evaluate(&def_before, &off)
            })
            .collect();
        let new_pose = moved_pose(&old_rest, 5, 30.0);
        p.set_rest_from_pose(0, &new_pose, SetRestMode::Rebase)
            .expect("rebase applies");
        let def_after = p.figures[0].def.clone();
        let new_rest = def_after.rest_angles();
        for (i, f) in p.frames.iter().enumerate() {
            let pose = &f.poses[0];
            let off = Pose {
                root: pose.root,
                angles: pose
                    .angles
                    .iter()
                    .zip(new_rest.iter())
                    .map(|(a, r)| a - r)
                    .collect(),
            };
            let world = evaluate(&def_after, &off);
            for (a, b) in world.iter().zip(offsets_before[i].iter()) {
                assert!(approx(a[0], b[0]) && approx(a[1], b[1]), "frame {i} moved");
            }
        }
    }

    #[test]
    fn absolute_mode_changes_only_the_rest_angles() {
        let mut p = Project::new_demo();
        p.tween_playback = true;
        p.clips.push(authored_clip(&p, "authored"));
        p.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(5),
            keys: vec![Key::flat(0.0, 10.0)],
        });
        let stores_before = json(&(&p.frames, &p.clips, &p.channels));
        let at_int_before = json(&p.poses_at(0.0));
        let at_frac_before = json(&p.poses_at(2.5));
        let old_rest = p.figures[0].def.rest_angles();
        let stance_before = evaluate(
            &p.figures[0].def,
            &p.figures[0].def.rest_pose([480.0, 470.0]),
        );
        let new_pose = moved_pose(&old_rest, 5, 30.0);
        let out = p
            .set_rest_from_pose(0, &new_pose, SetRestMode::Absolute)
            .expect("absolute applies");
        assert_eq!(out.clips_rebased, 0);
        assert_eq!(out.channels_rebased, 0);
        assert_eq!(json(&(&p.frames, &p.clips, &p.channels)), stores_before);
        assert_eq!(json(&p.poses_at(0.0)), at_int_before);
        assert_eq!(json(&p.poses_at(2.5)), at_frac_before);
        let new_rest = p.figures[0].def.rest_angles();
        assert!(new_rest
            .iter()
            .zip(out.new_rest.iter())
            .all(|(a, b)| approx(*a, *b)));
        assert!(approx(new_rest[5] - old_rest[5], out.delta[5]));
        let stance_after = evaluate(
            &p.figures[0].def,
            &p.figures[0].def.rest_pose([480.0, 470.0]),
        );
        assert!(
            stance_before
                .iter()
                .zip(stance_after.iter())
                .any(|(a, b)| !approx(a[0], b[0]) || !approx(a[1], b[1])),
            "the rest stance itself moved"
        );
    }

    #[test]
    fn state_keys_ride_the_rebase_through_the_clip_store() {
        let mut p = Project::new_demo();
        p.clips.push(authored_clip(&p, "authored"));
        p.figures[0].states = vec![StateKey {
            frame: 2,
            clip: 0,
            offset: 1,
            speed: 2.0,
        }];
        let keys_before = json(&p.figures[0].states);
        let timeline_before = p.poses_at(0.0)[0].angles.clone();
        let driven_before = p.poses_at(3.0)[0].angles.clone();
        let old_rest = p.figures[0].def.rest_angles();
        let new_pose = moved_pose(&old_rest, 5, 30.0);
        let out = p
            .set_rest_from_pose(0, &new_pose, SetRestMode::Rebase)
            .expect("rebase applies");
        let d = out.delta[5];
        let timeline_after = p.poses_at(0.0)[0].angles.clone();
        let driven_after = p.poses_at(3.0)[0].angles.clone();
        assert!(approx(timeline_after[5], timeline_before[5] + d));
        assert!(approx(driven_after[5], driven_before[5] + d));
        assert_eq!(json(&p.figures[0].states), keys_before);
    }

    #[test]
    fn set_rest_refuses_bad_inputs_and_changes_nothing() {
        let mut p = Project::new_demo();
        let before = json(&p);
        let rest = p.figures[0].def.rest_angles();
        let good = moved_pose(&rest, 5, 30.0);
        assert!(p.set_rest_from_pose(9, &good, SetRestMode::Rebase).is_none());
        let short = Pose {
            root: [0.0, 0.0],
            angles: vec![0.0; 2],
        };
        assert!(p.set_rest_from_pose(0, &short, SetRestMode::Rebase).is_none());
        let mut nan = good.clone();
        nan.angles[0] = f32::NAN;
        assert!(p.set_rest_from_pose(0, &nan, SetRestMode::Absolute).is_none());
        let mut inf = good.clone();
        inf.angles[1] = f32::INFINITY;
        assert!(p.set_rest_from_pose(0, &inf, SetRestMode::Rebase).is_none());
        assert_eq!(json(&p), before);
    }
}
