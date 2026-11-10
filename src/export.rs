//! PNG sequence export: a small CPU rasterizer so projects can be
//! rendered to images without a GPU or a display. Draws each figure's
//! evaluated segments as round-capped thick lines and the head as a
//! ring, with 2x supersampling for anti-aliasing.

use crate::model::{
    dist, evaluate, CapKind, ParticleKind, Project, Shape, V2,
};
use image::{ImageBuffer, Rgb};
use std::path::Path;

fn rgb(hex: u32) -> [u8; 3] {
    [
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    ]
}

/// Even-odd point-in-polygon test (ray cast toward +x) —
/// the filled-stroke pass's coverage test. Points exactly on
/// an edge may fall either way; at render scale that is a
/// sub-pixel ambiguity, and the supersample pass averages it.
fn point_in_polygon(p: [f32; 2], verts: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = verts.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (verts[i][0], verts[i][1]);
        let (xj, yj) = (verts[j][0], verts[j][1]);
        if (yi > p[1]) != (yj > p[1]) {
            let x_cross = xi + (p[1] - yi) / (yj - yi) * (xj - xi);
            if p[0] < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let t = {
        let denom = ab[0] * ab[0] + ab[1] * ab[1];
        if denom <= f32::EPSILON {
            0.0
        } else {
            (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / denom).clamp(0.0, 1.0)
        }
    };
    let c = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    dist(p, c)
}

/// A segment plus its rendering style (see `model::NodeStyle`):
/// width and colour interpolate from the near end (`a`, the parent
/// joint) to the far end (`b`); opacity blends over whatever is
/// already drawn. The default style (uniform width, solid colour,
/// opacity 1, round caps) reduces exactly to the classic test
/// `seg_dist <= half`.
#[derive(Clone, Copy)]
struct StyledSeg {
    a: [f32; 2],
    b: [f32; 2],
    half_near: f32,
    half_far: f32,
    color_near: u32,
    color_far: u32,
    opacity: f32,
    cap: CapKind,
}

impl StyledSeg {
    /// An unstyled segment (brush strokes): uniform, solid, opaque.
    fn solid(a: [f32; 2], b: [f32; 2], color: u32, half: f32) -> StyledSeg {
        StyledSeg {
            a,
            b,
            half_near: half,
            half_far: half,
            color_near: color,
            color_far: color,
            opacity: 1.0,
            cap: CapKind::Round,
        }
    }
}

/// Colour of `seg` at pixel `p` if the pixel is covered, else
/// `None`. The pixel is projected onto the segment axis (t in
/// 0..=1): the local half-width and colour interpolate along t, and
/// the pixel is covered when its distance to the segment is within
/// the local half-width. Round (and point) caps fall out of the
/// clamped distance test; square caps first extend both endpoints
/// by their local half-width along the segment direction.
fn styled_seg_color(p: [f32; 2], seg: &StyledSeg) -> Option<u32> {
    let ab = [seg.b[0] - seg.a[0], seg.b[1] - seg.a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 <= f32::EPSILON {
        return if dist(p, seg.a) <= seg.half_near {
            Some(seg.color_near)
        } else {
            None
        };
    }
    let t = (((p[0] - seg.a[0]) * ab[0] + (p[1] - seg.a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    let half = seg.half_near + (seg.half_far - seg.half_near) * t;
    let covered = match seg.cap {
        CapKind::Square => {
            let len = len2.sqrt();
            let dir = [ab[0] / len, ab[1] / len];
            let a2 = [
                seg.a[0] - dir[0] * seg.half_near,
                seg.a[1] - dir[1] * seg.half_near,
            ];
            let b2 = [
                seg.b[0] + dir[0] * seg.half_far,
                seg.b[1] + dir[1] * seg.half_far,
            ];
            seg_dist(p, a2, b2) <= half
        }
        _ => seg_dist(p, seg.a, seg.b) <= half,
    };
    if covered {
        Some(crate::style::lerp_color(seg.color_near, seg.color_far, t))
    } else {
        None
    }
}

/// Deterministic per-particle random in [0, 1) from a seed + index.
fn prand(seed: u32, i: u32, salt: u32) -> f32 {
    let mut h = seed ^ i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    (h % 100_000) as f32 / 100_000.0
}

/// Live particles at fractional frame time `f`:
/// (centre, radius, colour, alpha). Closed-form and deterministic —
/// the same frame always renders identically.
pub fn particles_at(project: &Project, f: f32) -> Vec<(V2, f32, [u8; 3], f32)> {
    let mut out = Vec::new();
    let fps = project.fps.max(1.0);
    for ev in &project.particles {
        let age = f - ev.frame as f32;
        let life = 0.7 * fps;
        if age < 0.0 || age > life {
            continue;
        }
        let t = age / life; // 0..1 through the effect
        let age_s = age / fps;
        let origin = [ev.x, ev.y];
        let color = rgb(ev.color);
        let (count, base_size) = match ev.kind {
            ParticleKind::Burst => (26, 3.2),
            ParticleKind::Sparkle => (14, 2.6),
            ParticleKind::Smoke => (12, 5.0),
        };
        for i in 0..count {
            let a = prand(ev.seed, i, 1) * std::f32::consts::TAU;
            let r1 = prand(ev.seed, i, 2);
            let (pos, size, alpha) = match ev.kind {
                ParticleKind::Burst => {
                    let speed = 40.0 + r1 * 110.0;
                    let d = speed * age_s * (1.0 - 0.45 * t);
                    (
                        [origin[0] + a.cos() * d, origin[1] + a.sin() * d + 30.0 * age_s * age_s],
                        base_size * (1.0 - 0.6 * t),
                        1.0 - t,
                    )
                }
                ParticleKind::Sparkle => {
                    let d = 8.0 + r1 * 26.0;
                    let twinkle = 0.55 + 0.45 * (age * 1.7 + i as f32 * 2.3).sin();
                    (
                        [origin[0] + a.cos() * d, origin[1] + a.sin() * d - 12.0 * age_s],
                        base_size * twinkle.max(0.2),
                        (1.0 - t) * twinkle,
                    )
                }
                ParticleKind::Smoke => {
                    let d = 4.0 + r1 * 16.0;
                    (
                        [origin[0] + a.cos() * d + 6.0 * age_s, origin[1] - 26.0 * age_s + a.sin() * d * 0.4],
                        base_size + 10.0 * t,
                        0.32 * (1.0 - t),
                    )
                }
            };
            if alpha > 0.01 {
                out.push((pos, size.max(0.4), color, alpha.clamp(0.0, 1.0)));
            }
        }
    }
    out
}


/// Rotate a stage-space offset by the camera rotation.
///
/// Convention (wave 6): `angle_deg` is the *camera* rotation in
/// degrees, mirroring Pivot Animator's per-frame camera angle (the
/// `angle_rad` f64 in the .piv v5 camera record — see pivot.rs).
/// Pivot's renderer applies the scene transform with the negated
/// angle (the original's `renderer-geometry.mjs` builds its camera
/// matrix from `angle = -camera.angle_rad`), so a positive angle
/// rotates the *scene counter-clockwise* on screen: a point to the
/// right of the camera centre lands above it at +90°. In y-down
/// screen coordinates that rotation is R(−θ):
/// `(dx, dy) -> (cos·dx + sin·dy, −sin·dx + cos·dy)`.
pub fn camera_rotate_offset(dx: f32, dy: f32, angle_deg: f32) -> [f32; 2] {
    let rad = angle_deg.to_radians();
    let (c, s) = (rad.cos(), rad.sin());
    [c * dx + s * dy, -s * dx + c * dy]
}

/// Recent path of one node of one figure, as fading segments in
/// stage coordinates: (a, b, colour, alpha). The node's position
/// is sampled at `samples` evenly spaced times from
/// `frame_f - span` up to `frame_f` (clamped at frame 0), and
/// consecutive positions are joined oldest-first; alpha grows
/// toward the present, reaching 0.5 on the newest segment. The
/// colour is the figure's own colour. Segments shorter than 0.75
/// stage units are dropped, so a node that did not move yields an
/// empty trail. Pure and deterministic, like everything else in
/// the render path. Invalid figure/node indices, `samples < 2`,
/// `span <= 0`, or `frame_f <= 0` (no past to trail through) all
/// yield an empty trail.
pub fn node_trail(
    project: &Project,
    frame_f: f32,
    fi: usize,
    ni: usize,
    samples: usize,
    span: f32,
) -> Vec<(V2, V2, [u8; 3], f32)> {
    let mut out = Vec::new();
    if samples < 2 || span <= 0.0 || frame_f <= 0.0 {
        return out;
    }
    let Some(fig) = project.figures.get(fi) else {
        return out;
    };
    if ni >= fig.def.nodes.len() {
        return out;
    }
    let color = rgb(fig.color);
    let mut pts: Vec<V2> = Vec::with_capacity(samples);
    for k in 0..samples {
        // 0 = oldest sample, 1 = the current frame.
        let frac = k as f32 / (samples - 1) as f32;
        let tf = (frame_f - span * (1.0 - frac)).max(0.0);
        // Wave P (P17): trail samples take the evaluated
        // (graph-channelled) frame like every other evaluation.
        // This helper is shared by the rasterizer, the SVG
        // writer AND the stage's stage_fx layer, so funnelling
        // here moves all three surfaces at once; with no
        // channels it is the raw `poses_at` path unchanged.
        let channelled = if project.channels.is_empty() {
            None
        } else {
            Some(crate::graph::apply_channels(project, &project.channels, tf))
        };
        let raw_poses = project.poses_at(tf);
        let poses: &Vec<crate::model::Pose> = match &channelled {
            Some(r) => &r.poses,
            None => &raw_poses,
        };
        let Some(pose) = poses.get(fi) else {
            return out;
        };
        let def = match &channelled {
            Some(r) => &r.defs[fi],
            None => &fig.def,
        };
        let pos = evaluate(def, pose);
        let Some(&p) = pos.get(ni) else {
            return out;
        };
        pts.push(p);
    }
    for j in 0..samples - 1 {
        let (a, b) = (pts[j], pts[j + 1]);
        if dist(a, b) < 0.75 {
            continue;
        }
        let alpha = 0.5 * (j + 1) as f32 / (samples - 1) as f32;
        out.push((a, b, color, alpha));
    }
    out
}

/// Render one frame index to an RGB buffer at the project stage size.
pub fn render_frame(project: &Project, frame_idx: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    render_frame_at(project, frame_idx as f32)
}

/// Render one frame index at `scale` times the project stage
/// size (Wave R, R10): the still-frame half of the export-scale
/// surface. The scale is validated (finite, > 0) and refused
/// with an `InvalidInput` error otherwise. At 1.0 the result
/// is byte-identical to [`render_frame`].
pub fn render_frame_scaled(
    project: &Project,
    frame_idx: usize,
    scale: f32,
) -> std::io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    render_frame_at_scaled(project, frame_idx as f32, scale)
}

/// The fractional-time half of [`render_frame_scaled`]:
/// renders at `frame_f` with the output multiplied by `scale`
/// end to end — the supersampled buffer, the camera/stage
/// transform and every derived size scale together, so a 2.0
/// render is a true 2x image, never an upscaled copy of the
/// 1.0 raster. The subtitle burn-in at the funnel's tail
/// multiplies its shared-helper layout by the same scale
/// (IA-R10-3), so the burn-in scales with the frame.
pub fn render_frame_at_scaled(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> std::io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    let scale = check_export_scale(scale)?;
    Ok(render_frame_at_scaled_inner(project, frame_f, scale))
}

/// Render one frame index to an RGBA buffer at the project
/// stage size (Wave X, X8 — SpriteLoop GAP P-5): the
/// transparent-background counterpart of [`render_frame`].
/// The background fill is skipped and every drawable's
/// coverage is carried out as per-pixel alpha, so a pixel
/// nothing covers is fully transparent and antialiased edges
/// carry partial alpha. Media, stage texts and the subtitle
/// burn-in composite exactly as in the RGB funnel — the only
/// difference is what lies under them.
pub fn render_frame_alpha(
    project: &Project,
    frame_idx: usize,
) -> image::RgbaImage {
    render_frame_at_alpha(project, frame_idx as f32)
}

/// The fractional-time half of [`render_frame_alpha`].
pub fn render_frame_at_alpha(
    project: &Project,
    frame_f: f32,
) -> image::RgbaImage {
    render_frame_at_scaled_inner_alpha(project, frame_f, 1.0)
}

/// Render one frame index at `scale` times the stage size
/// with coverage alpha (Wave X, X8): the scaled counterpart
/// of [`render_frame_alpha`], riding the same true re-render
/// scale rule as [`render_frame_scaled`] — the scale is
/// validated and refused with an `InvalidInput` error
/// otherwise. At 1.0 the result equals
/// [`render_frame_alpha`].
pub fn render_frame_alpha_scaled(
    project: &Project,
    frame_idx: usize,
    scale: f32,
) -> std::io::Result<image::RgbaImage> {
    render_frame_at_alpha_scaled(project, frame_idx as f32, scale)
}

/// The fractional-time half of [`render_frame_alpha_scaled`].
pub fn render_frame_at_alpha_scaled(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> std::io::Result<image::RgbaImage> {
    let scale = check_export_scale(scale)?;
    Ok(render_frame_at_scaled_inner_alpha(project, frame_f, scale))
}

/// Validate an export scale (Wave R, R10): the raster
/// funnel's rule, mirroring the MCP render tool's argument
/// rule — the scale must be a positive finite number.
/// Refusals are `InvalidInput`, the funnel's voice for bad
/// arguments (see `clamp_frame_range`).
fn check_export_scale(scale: f32) -> std::io::Result<f32> {
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("export scale must be a positive finite number, got {scale}"),
        ))
    }
}

/// The output pixel size of a render at `scale` (Wave R,
/// R10): stage × scale, rounded and floored at 1 px per
/// axis — the MCP render tool's long-standing dimension
/// formula, so the core and the tool agree on dimensions.
/// At exactly 1.0 this is the historical truncation
/// `stage as u32`, so scale-1.0 renders keep today's
/// dimensions for every stage, fractional or not.
fn scaled_output_size(project: &Project, scale: f32) -> (u32, u32) {
    if scale == 1.0 {
        (project.stage[0] as u32, project.stage[1] as u32)
    } else {
        (
            (project.stage[0] * scale).round().max(1.0) as u32,
            (project.stage[1] * scale).round().max(1.0) as u32,
        )
    }
}

/// Render at a fractional frame time (tween poses, camera and
/// particles all evaluate at `f`). The camera (centre + zoom +
/// rotation, interpolated between keys) is applied, as are
/// per-segment colours, thicknesses and shape segments. Rotation
/// lives inside the `view` closure, so segments, heads, shapes,
/// strokes, particles, weather and the media composite (which
/// receives this same view) all inherit it.
pub fn render_frame_at(project: &Project, frame_f: f32) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    render_frame_at_scaled_inner(project, frame_f, 1.0)
}

/// One frame's drawables, collected once and shared by
/// the raster funnels (Wave X, X8): everything the pixel
/// pass consumes, in the pass's own shapes — styled
/// segments, head rings (centre, colour, half-width,
/// alpha), shape rings (centre, radius, colour,
/// half-width, alpha), polygon outlines, filled-stroke
/// regions and the unified particle/disc list (FX,
/// weather, spray and scatter all land here).
struct FrameArt {
    segments: Vec<StyledSeg>,
    heads: Vec<([f32; 2], [u8; 3], f32, f32)>,
    rings: Vec<([f32; 2], f32, [u8; 3], f32, f32)>,
    polys: Vec<(Vec<[f32; 2]>, [u8; 3], f32, f32)>,
    fills: Vec<(Vec<[f32; 2]>, [u8; 3], f32)>,
    particles: Vec<([f32; 2], f32, [u8; 3], f32)>,
}

/// Collect one frame's drawables (extracted verbatim
/// from the raster funnel in Wave X, X8) so the RGB and
/// RGBA funnels share ONE collection: the evaluated
/// (channelled) poses, the figure loop, brush strokes,
/// FX clips and weather all resolve here, and the two
/// pixel passes below consume the identical art — the
/// alpha funnel can therefore never drift from the RGB
/// frame it parallels. `view` maps stage points to
/// supersampled buffer pixels; `px` is the same scale
/// factor the funnel derived, `zoom` the camera zoom
/// (widths scale by both).
fn collect_frame_art(
    project: &Project,
    frame_f: f32,
    view: &impl Fn([f32; 2]) -> [f32; 2],
    px: f32,
    zoom: f32,
) -> FrameArt {
    // Wave P (P17): the evaluated frame. The stage draws figures
    // through `EditorCore::positions_f`, which substitutes
    // graph-channelled poses AND defs whenever the project
    // carries channels; an export must render the frame the user
    // sees, so the same evaluation is taken here and shared by
    // every consumer below (the figure loop, FX clip anchors;
    // the media composite funnels through `media_items`, which
    // applies the same rule). With no channels this is exactly
    // the raw `poses_at` path, byte for byte.
    let channelled = if project.channels.is_empty() {
        None
    } else {
        Some(crate::graph::apply_channels(
            project,
            &project.channels,
            frame_f,
        ))
    };
    let raw_poses = project.poses_at(frame_f);
    let poses: &Vec<crate::model::Pose> = match &channelled {
        Some(r) => &r.poses,
        None => &raw_poses,
    };
    // Collect drawables: segments (a, b, color, half-width), head
    // rings (centre, color, half-width, alpha), shape circles and
    // polygons — heads/rings/polys carry the node's effective
    // alpha (Wave 13, S10) and blend in the pixel pass.
    let mut segments: Vec<StyledSeg> = Vec::new();
    let mut heads: Vec<([f32; 2], [u8; 3], f32, f32)> = Vec::new();
    let mut rings: Vec<([f32; 2], f32, [u8; 3], f32, f32)> = Vec::new();
    let mut polys: Vec<(Vec<[f32; 2]>, [u8; 3], f32, f32)> = Vec::new();
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let pose = &poses[fi];
        // Positions come from the channelled def when channels
        // exist (P17); node PROPERTIES (colour, style, shape,
        // thickness) are still read from the figure's own def in
        // the loop below — the same split the stage makes, since
        // `positions_f` likewise returns only positions.
        let def = match &channelled {
            Some(r) => &r.defs[fi],
            None => &fig.def,
        };
        let pos = evaluate(def, pose);
        // Per-node bends at this time (wave 7); empty when the
        // project carries no bend data. Wave Y (P-7): with
        // channels present the bends come from the channelled
        // evaluation's own rows — seeded from `bends_at` and
        // overridden by any Bend channel (graph.rs), so a
        // bend channel is visible in the render; the
        // no-channels path is `bends_at` verbatim, as before.
        let bends = match &channelled {
            Some(r) => r.bends.get(fi).cloned().unwrap_or_default(),
            None => project.bends_at(frame_f, fi),
        };
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let color_hex = node.color.unwrap_or(fig.color);
            // Heads/rings/polys (Wave 13, S10): the node's own
            // colour at the node's EFFECTIVE alpha — the shared
            // `effective_style` product (style.opacity × figure
            // opacity), composed as true alpha in the pixel pass
            // via `blend_over`, exactly as segments compose it
            // via StyledSeg.opacity. Before this they
            // hard-overwrote the pixel with the colour faded
            // toward the background by figure opacity alone, so
            // a node's style.opacity never reached them (and a
            // faded head erased whatever lay beneath it toward
            // the background instead of blending over it). At
            // effective alpha 1.0 `blend_over` returns the
            // colour exactly, so default-styled projects render
            // byte-identically.
            let color = rgb(color_hex);
            let node_alpha =
                crate::style::effective_style(node.style, fig.opacity).opacity;
            let half = node.thickness / 2.0 * zoom * px;
            if let Some(p) = node.parent {
                let style = node.style;
                // Wave 12: figure segments are built from the
                // shared piece generator in `paint` — the one
                // source of truth for how a `NodeStyle` becomes
                // geometry, shared with the stage. The generator
                // returns the segment as a chain of pieces (split
                // at `model::bend_mid` when this node's bend is
                // nonzero), each piece carrying the width/colour
                // sample at its start joint and the style's
                // opacity as alpha. The raster re-expresses the
                // chain as `StyledSeg`s whose far values are the
                // NEXT piece's samples, so the per-pixel
                // interpolation in `styled_seg_color` traces the
                // generator's own lerp end to end.
                //
                // Two things the piece list cannot carry stay
                // derived here, from the same inputs the
                // generator received:
                //  - the terminal far width/colour (the lerp's
                //    end): a piece carries the sample at its
                //    start, so the last piece's far end has no
                //    carrier. The width keeps this module's
                //    exact historical expression — a point cap
                //    forces zero, else `half * taper` with the
                //    taper floored at 0 — so the far half-width
                //    is bit-identical to the pre-generator
                //    construction.
                //  - for a square cap, the outer endpoints: the
                //    generator expresses the cap by extending
                //    the piece endpoints, but `styled_seg_color`
                //    extends analytically from the true joints
                //    (its width/colour parameter lives on the
                //    unextended axis), so the raster keeps the
                //    joints as the outer endpoints and lets the
                //    coverage test do the extending — adopting
                //    the extended endpoints too would extend
                //    twice and re-parameterise the taper across
                //    the extension.
                let bend = bends.get(i).copied().unwrap_or(0.0);
                let pieces = crate::paint::styled_segment_pieces(
                    pos[p],
                    pos[i],
                    bend,
                    node.thickness,
                    &style,
                    color_hex,
                );
                let fig_opacity = fig.opacity.clamp(0.0, 1.0);
                // A point cap forces the far width to zero
                // whatever the taper says.
                let half_far = if style.cap == CapKind::Point {
                    0.0
                } else {
                    half * style.taper.max(0.0)
                };
                let color_far = style.color2.unwrap_or(color_hex);
                if bend == 0.0 {
                    // Straight segment: one analytic `StyledSeg`
                    // from the first piece's near sample to the
                    // lerp's end — the same single segment this
                    // module has always built (zero bend, and
                    // bend-less projects whose `bends_at` is
                    // empty, take this path unchanged). Merging
                    // the chain here keeps the gradient's
                    // interpolation in one `lerp_color` sweep, so
                    // no intermediate rounding creeps in at piece
                    // joints the coverage test never needed.
                    if let Some(first) = pieces.first() {
                        segments.push(StyledSeg {
                            a: view(pos[p]),
                            b: view(pos[i]),
                            half_near: first.width / 2.0 * zoom * px,
                            half_far,
                            color_near: first.color,
                            color_far,
                            opacity: first.alpha * fig_opacity,
                            cap: style.cap,
                        });
                    }
                } else {
                    // Bent segment: one `StyledSeg` per piece,
                    // chained through the bend midpoint (wave 7's
                    // two-piece arc, generalised to the chain the
                    // generator returns). Every piece but the
                    // last gets a Round cap so the arc joint
                    // stays smooth whatever the segment's own cap
                    // is; the last piece keeps the style's cap
                    // and — for square — the true far joint as
                    // its endpoint, per the note above.
                    let last = pieces.len().saturating_sub(1);
                    for (k, piece) in pieces.iter().enumerate() {
                        let (far_half, far_color) = if k < last {
                            (
                                pieces[k + 1].width / 2.0 * zoom * px,
                                pieces[k + 1].color,
                            )
                        } else {
                            (half_far, color_far)
                        };
                        let a = if k == 0 && style.cap == CapKind::Square {
                            view(pos[p])
                        } else {
                            view(piece.a)
                        };
                        let b = if k == last && style.cap == CapKind::Square {
                            view(pos[i])
                        } else {
                            view(piece.b)
                        };
                        segments.push(StyledSeg {
                            a,
                            b,
                            half_near: piece.width / 2.0 * zoom * px,
                            half_far: far_half,
                            color_near: piece.color,
                            color_far: far_color,
                            opacity: piece.alpha * fig_opacity,
                            cap: if k < last { CapKind::Round } else { style.cap },
                        });
                    }
                }
            }
            if node.head {
                heads.push((view(pos[i]), color, half, node_alpha));
            }
            match node.shape {
                Shape::Circle => {
                    rings.push((view(pos[i]), node.length / 2.0 * zoom * px, color, half, node_alpha));
                }
                Shape::Polygon(sides) => {
                    let sides = (sides.max(3)) as usize;
                    let r = node.length / 2.0;
                    let verts: Vec<[f32; 2]> = (0..sides)
                        .map(|k| {
                            let a = k as f32 / sides as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                            view([pos[i][0] + a.cos() * r, pos[i][1] + a.sin() * r])
                        })
                        .collect();
                    polys.push((verts, color, half, node_alpha));
                }
                Shape::Line => {}
            }
        }
    }
    // Brush strokes belonging to the authored frame under `frame_f`.
    // A spray stroke (wave 13) skips the polyline entirely — a
    // sprayed path must read as spray, not as a line with dots on
    // it — and is collected as dots instead. Below, the dots join
    // `particles` and are drawn by the same disc/alpha code as the
    // weather and FX particles: one disc blender, and spray
    // geometry comes only from `paint::spray_dots`.
    let authored = (frame_f.floor() as usize).min(project.frames.len().saturating_sub(1));
    let mut spray: Vec<([f32; 2], f32, [u8; 3], f32)> = Vec::new();
    // Filled strokes (Wave 11, E8): screen-space polygon,
    // fill colour, opacity — painted by the pixel pass under
    // the linework. Collected in stroke order.
    let mut fills: Vec<(Vec<[f32; 2]>, [u8; 3], f32)> = Vec::new();
    for (index, stroke) in project.strokes.iter().enumerate() {
        if stroke.frame == authored {
            if let Some(mark) = &stroke.spray {
                let radius = (stroke.width * 0.5).max(1.0);
                let color = rgb(stroke.color);
                for (centre, alpha) in crate::paint::spray_dots(&stroke.points, mark) {
                    // Wave 12 (A5 parity): a stored mark's
                    // opacity is unclamped on load, so a dot
                    // alpha can exceed 1; the stage clamps
                    // (stage_layers' dot_alpha) and the SVG
                    // writer clamps at emission — clamp here
                    // too, or the disc blend below
                    // extrapolates past both colours.
                    spray.push((centre, radius, color, alpha.clamp(0.0, 1.0)));
                }
                continue;
            }
            // Filled stroke (Wave 11, E8): a stroke with a
            // fill and at least 3 points draws as a filled
            // polygon and NOTHING else — no outline, and
            // scatter does not texture it. That is the stage's
            // precedence (stage_layers: spray, then fill, then
            // scatter, then the plain polyline), which the SVG
            // exporter already mirrors; the rasterizer used to
            // drop fills entirely and draw the outline only.
            // The region paints in the pixel pass UNDER the
            // linework, at the stroke's recorded opacity.
            if let Some(fill) = stroke.fill {
                if stroke.points.len() >= 3 {
                    fills.push((
                        stroke.points.iter().map(|p| view(*p)).collect(),
                        rgb(fill),
                        stroke.opacity.clamp(0.0, 1.0),
                    ));
                    continue;
                }
            }
            // Scattered stroke (Wave 10 adoption): a
            // non-spray, non-filled stroke with scatter > 0
            // draws as dabs, not segments — the same placements
            // the stage and the SVG exporter compute
            // (paint::stroke_dabs at the shared spacing),
            // riding this same disc pass at the stroke's
            // recorded opacity.
            if stroke.scatter > 0.0 && !stroke.points.is_empty() {
                let radius = (stroke.width * 0.5).max(1.0);
                let color = rgb(stroke.color);
                let alpha = stroke.opacity.clamp(0.0, 1.0);
                for centre in crate::paint::stroke_dabs(
                    stroke,
                    index,
                    crate::paint::stroke_dab_spacing(stroke),
                ) {
                    spray.push((centre, radius, color, alpha));
                }
                continue;
            }
            let pressured = stroke.widths.len() == stroke.points.len();
            // Per-stroke opacity (Phase M), recorded from the
            // brush: it rides the segment's own opacity, so the
            // pixel pass composites the stroke through
            // `style::blend_over` — the same dst*(1-a) + src*a
            // blend the spray discs get in the particle pass.
            // At 1.0 `blend_over` returns the source colour
            // exactly, so opaque strokes render byte-identically
            // to before the field existed. Spray strokes never
            // reach this path (handled above; their dots carry
            // the mark's opacity, and their recorded stroke
            // opacity is always 1.0).
            let opacity = stroke.opacity.clamp(0.0, 1.0);
            for (i, w) in stroke.points.windows(2).enumerate() {
                let width = if pressured {
                    (stroke.widths[i] + stroke.widths[i + 1]) / 2.0
                } else {
                    stroke.width
                };
                segments.push(StyledSeg {
                    opacity,
                    ..StyledSeg::solid(
                        view(w[0]),
                        view(w[1]),
                        stroke.color,
                        width / 2.0 * zoom * px,
                    )
                });
            }
        }
    }
    let mut particles: Vec<([f32; 2], f32, [u8; 3], f32)> = particles_at(project, frame_f)
        .into_iter()
        .map(|(p, r, c, a)| (view(p), r * zoom * px, c, a))
        .collect();
    // Spray dots (wave 13) ride the particle path from here: same
    // view transform, same radius scaling, same disc blending in
    // the pixel pass below.
    particles.extend(
        spray
            .into_iter()
            .map(|(p, r, c, a)| (view(p), r * zoom * px, c, a)),
    );
    // Fog bands (wave 10): (centre, rx, ry, peak alpha) in screen
    // space, filled by the weather block below when the weather
    // kind is Fog. Blended in the pixel pass at the same layer as
    // the weather particles — over the figures, like rain and snow.
    for entry in &project.fx_clips {
        // Hidden clips (Phase M) stay on the timeline but
        // render nothing. The skip sits here, at the render
        // source, so every path built on `render_frame_at`
        // (image sequences, contact sheets, spritesheets)
        // honours it too.
        if entry.hidden {
            continue;
        }
        if entry.figure < project.figures.len() {
            let fig = &project.figures[entry.figure];
            if !fig.hidden {
                // FX anchors sit on the evaluated (channelled)
                // figure, exactly as the stage's stage_fx layer
                // anchors them through `positions_f` (P17).
                let def = match &channelled {
                    Some(r) => &r.defs[entry.figure],
                    None => &fig.def,
                };
                let pos = evaluate(def, &poses[entry.figure]);
                if let Some(&anchor) = pos.get(entry.node) {
                    for (p, r, c, a) in crate::vfx::clip_particles(&entry.clip, frame_f, anchor) {
                        particles.push((view(p), r * zoom * px, c, a));
                    }
                    // A live Trail clip also draws the node's real
                    // recent path (not just the ghost chain opposite
                    // `angle`): thin fading segments under the
                    // particles, spanning half the clip's duration,
                    // at most 12 frames back.
                    if entry.clip.kind == crate::vfx::ClipKind::Trail
                        && frame_f >= entry.clip.start
                        && frame_f < entry.clip.start + entry.clip.duration
                    {
                        let span = (entry.clip.duration * 0.5).clamp(2.0, 12.0);
                        for (a, b, c, alpha) in
                            node_trail(project, frame_f, entry.figure, entry.node, 8, span)
                        {
                            let hex = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
                            let half = 1.6 * zoom * px;
                            segments.push(StyledSeg {
                                a: view(a),
                                b: view(b),
                                half_near: half,
                                half_far: half,
                                color_near: hex,
                                color_far: hex,
                                opacity: alpha,
                                cap: CapKind::Round,
                            });
                        }
                    }
                }
            }
        }
    }
    if let Some(weather) = &project.weather {
        for (p, r, c, a) in
            crate::vfx::weather_particles(weather, frame_f, project.stage, project.fps)
        {
            particles.push((view(p), r * zoom * px, c, a));
        }
    }

    FrameArt {
        segments,
        heads,
        rings,
        polys,
        fills,
        particles,
    }
}

/// Whether a frame carries any media the composite
/// tail must run for (the gate the raster funnel's tail
/// applies — its Wave 11 / Wave 14 rationale lives at
/// the call site): node media on figure or clip defs,
/// or any free-standing sprite or stage text. Shared
/// by the RGB and RGBA funnels (Wave X, X8).
fn frame_has_media(project: &Project) -> bool {
    !project.sprites.is_empty()
        || !project.texts.is_empty()
        || project
            .figures
            .iter()
            .any(|f| f.def.nodes.iter().any(|n| n.media.is_some()))
        || project
            .clips
            .iter()
            .any(|c| c.def.nodes.iter().any(|n| n.media.is_some()))
}

/// Build the subtitle burn-in overlay for one frame
/// (extracted from the raster funnel in Wave X, X8, so
/// the RGB and RGBA funnels stamp identical subtitles):
/// a clone of the project with every other media source
/// cleared, its texts replaced by the live cues' stamp
/// items. `None` when no cue is live at the frame.
fn subtitle_overlay(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> Option<crate::model::Project> {
    let cue_frame = frame_f.round().max(0.0) as usize;
    let live_cues = crate::subtitles::live_subtitle_cues(project, cue_frame);
    if live_cues.is_empty() {
        return None;
    }
    // Placement: centred horizontally, stacked from the
    // style's anchor edge in cue order — the FIRST cue
    // closest to the edge. Glyph height and line centres
    // come from the core's shared layout helpers
    // (`subtitle_font_size`, `subtitle_line_center_y`):
    // the base 20-unit font at the style's sanitised
    // scale, each stacked line's centre one 24-unit line
    // height further from the edge, the edge line's
    // centre one edge margin (16) plus half a line
    // height in — the stage draw's and the SVG writer's
    // layout exactly, so all three surfaces put a line's
    // glyphs at the same size on the same centre (the
    // stamp's +0.5-cell glyph-block offset about its
    // anchor is the bitmap font's own, shared with the
    // stage's `label_boxes` placement). Wave R (R1)
    // retired this block's former proportional sizing —
    // glyph height 6% of the stage height, a 1.35x line
    // step and a 1.1x-size anchor — under which the
    // raster was the last surface off the shared layout.
    // Legibility on any background comes from the stamp
    // itself: each line is stamped in the backing colour
    // at the eight one-cell offsets first (an outline),
    // then in the text colour centred — all nine stamps
    // are ordinary text items through the one funnel, in
    // list order.
    //
    // The block is drawn in STAGE space, NOT through the
    // camera `view`: a subtitle is frame furniture pinned
    // to the picture, so camera pan, zoom and rotation
    // must not move it — the settlement the SVG writer
    // took on this tree (see `append_subtitles` in
    // svg.rs), kept here so the two exporters' bands
    // agree at any camera. The stamps are computed in
    // output space (the stage-space layout × the render
    // scale, IA-R10-3), so the composite view is the
    // identity at any scale.
    //
    // The style is sanitised at read via the core's
    // `SubtitleStyle::sanitised()` — the read-path rule
    // the SVG writer applies the same way: a non-finite
    // font scale falls back to 1.0, a finite one is
    // clamped into [0.5, 3.0]; position and colours pass
    // through verbatim. The placement arithmetic below
    // is the core's shared layout throughout (first cue
    // closest to the edge; at the bottom anchor a cue's
    // LAST line sits closest): the stack offset was
    // IA-P26-1 in the Wave P findings, executed by Q17
    // in Wave Q; the font size and line centre joined
    // the same helpers in Wave R (R1).
    let style = project.subtitle_style.sanitised();
    // IA-R10-3: on a scaled frame the burn-in layout is
    // the shared stage-space layout multiplied by the
    // render scale — font size, centre x and every line
    // centre scale together, so the burn-in scales with
    // the picture around it. At scale 1.0 every factor
    // is 1.0 and the stamps are byte-identical.
    let size = crate::subtitles::subtitle_font_size(style) * scale;
    let cell = size / 5.0;
    // The funnel draws only what `media_items` derives
    // from a project, so the cues ride a text-only
    // overlay: a clone of the project with every other
    // media source cleared (sprites, stage texts, and
    // segment media on figure and clip defs), leaving
    // exactly the subtitles' text items. Paid only on
    // frames where any cue is live.
    let mut overlay = project.clone();
    overlay.sprites.clear();
    overlay.texts.clear();
    for fig in &mut overlay.figures {
        for node in &mut fig.def.nodes {
            node.media = None;
        }
    }
    for clip in &mut overlay.clips {
        for node in &mut clip.def.nodes {
            node.media = None;
        }
    }
    let cx = project.stage[0] / 2.0 * scale;
    let mut lines_before = 0usize;
    for cue in &live_cues {
        let lines: Vec<String> = crate::subtitles::subtitle_display_lines(cue);
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            // Distance of this line from the anchored
            // edge, in lines — the shared layout
            // contract, computed by the core's
            // `subtitle_stack_line_offset`: at the
            // bottom anchor the cue's lines count DOWN
            // toward the edge — its last line is offset
            // 0 — while at the top they count away in
            // their own order. The line's centre is the
            // shared `subtitle_line_center_y` for that
            // offset — the same centre the stage's band
            // and the SVG writer's `<text>` use.
            let from_edge = crate::subtitles::subtitle_stack_line_offset(
                lines_before,
                lines.len(),
                i,
                style.position,
            );
            let y = crate::subtitles::subtitle_line_center_y(
                style,
                project.stage[1],
                from_edge,
            ) * scale;
            let stamp = |x: f32, y: f32, color: u32| -> crate::model::TextInstance {
                crate::model::TextInstance {
                    content: (*line).to_string(),
                    size,
                    color,
                    keys: vec![crate::model::InstanceKey {
                        frame: 0.0,
                        x,
                        y,
                        scale: 1.0,
                        angle: 0.0,
                        opacity: 1.0,
                        flip_x: false,
                        flip_y: false,
                        scale_y: 0.0,
                        pivot_x: 0.5,
                        pivot_y: 0.5,
                    }],
                }
            };
            for (dx, dy) in [
                (-cell, 0.0),
                (cell, 0.0),
                (0.0, -cell),
                (0.0, cell),
                (-cell, -cell),
                (cell, -cell),
                (-cell, cell),
                (cell, cell),
            ] {
                overlay.texts.push(stamp(cx + dx, y + dy, style.box_color));
            }
            overlay.texts.push(stamp(cx, y, style.text_color));
        }
        lines_before += lines.len();
    }
    Some(overlay)
}

/// The shared raster funnel behind [`render_frame_at`] and
/// its scaled variants (Wave R, R10). `scale` arrives
/// pre-validated by the public entries (exactly 1.0 from
/// `render_frame_at` itself). The supersample factor `ss`
/// anti-aliases as always; `scale` multiplies the OUTPUT
/// size, and `px` — stage units to supersampled buffer
/// pixels — is `ss * scale`, so the buffer, the transform
/// and every derived size stay in step. At scale 1.0 `px`
/// is exactly the historical `ss as f32` and the output
/// size is exactly the stage size, so the render is
/// byte-identical to the pre-scale funnel.
fn render_frame_at_scaled_inner(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let ss = 2u32; // supersample factor
    let px = ss as f32 * scale;
    let (ow, oh) = scaled_output_size(project, scale);
    let (w, h) = (ow * ss, oh * ss);
    let bg = rgb(project.background);
    let mut buf = ImageBuffer::from_pixel(w, h, Rgb(bg));
    let (cam_x, cam_y, zoom) = project.camera_at(frame_f);
    let angle = project.camera_angle_at(frame_f);
    let view = |p: [f32; 2]| -> [f32; 2] {
        let (dx, dy) = (p[0] - cam_x, p[1] - cam_y);
        // Camera rotation (wave 6): rotate the offset around the
        // camera centre before the pan/zoom mapping (see
        // `camera_rotate_offset` for the sign convention). At
        // angle 0 the offset is used verbatim, so the transform
        // is exactly the pre-rotation one.
        let (ox, oy) = if angle == 0.0 {
            (dx, dy)
        } else {
            let r = camera_rotate_offset(dx, dy, angle);
            (r[0], r[1])
        };
        [
            (ox * zoom + project.stage[0] / 2.0) * px,
            (oy * zoom + project.stage[1] / 2.0) * px,
        ]
    };
    // The head ring's radius is the project's own
    // `head_ring_radius()` (Phase L): the model's single
    // `HEAD_RING_RADIUS` constant scaled by the project's
    // `head_ring_scale` — the same value the stage and the
    // figure thumbnails use, so exports can never drift
    // from the editor's ring.
    let head_r = project.head_ring_radius() * zoom * px;

    let FrameArt {
        segments,
        heads,
        rings,
        polys,
        fills,
        particles,
    } = collect_frame_art(project, frame_f, &view, px, zoom);

    for y in 0..h {
        for x in 0..w {
            let p = [x as f32, y as f32];
            let mut px = bg;
            for (verts, color, opacity) in &fills {
                if point_in_polygon(p, verts) {
                    px = crate::style::blend_over(px, *color, *opacity);
                }
            }
            for seg in &segments {
                if let Some(c) = styled_seg_color(p, seg) {
                    px = crate::style::blend_over(px, rgb(c), seg.opacity);
                }
            }
            for (c, color, half, alpha) in &heads {
                let d = dist(p, *c);
                if (d - head_r).abs() <= *half {
                    px = crate::style::blend_over(px, *color, *alpha);
                }
            }
            for (c, r, color, half, alpha) in &rings {
                let d = dist(p, *c);
                if (d - r).abs() <= *half {
                    px = crate::style::blend_over(px, *color, *alpha);
                }
            }
            for (verts, color, half, alpha) in &polys {
                for k in 0..verts.len() {
                    if seg_dist(p, verts[k], verts[(k + 1) % verts.len()]) <= *half {
                        px = crate::style::blend_over(px, *color, *alpha);
                    }
                }
            }
            for (c, r, color, alpha) in &particles {
                if dist(p, *c) <= *r {
                    px = [
                        (px[0] as f32 * (1.0 - alpha) + color[0] as f32 * alpha) as u8,
                        (px[1] as f32 * (1.0 - alpha) + color[1] as f32 * alpha) as u8,
                        (px[2] as f32 * (1.0 - alpha) + color[2] as f32 * alpha) as u8,
                    ];
                }
            }
            buf.put_pixel(x, y, Rgb(px));
        }
    }
    // Downsample by the supersample factor (box average)
    // into the output size computed at the top — the stage
    // size at scale 1.0, stage × scale otherwise (R10).
    let mut out = ImageBuffer::new(ow, oh);
    for y in 0..oh {
        for x in 0..ow {
            let mut acc = [0u32; 3];
            for dy in 0..ss {
                for dx in 0..ss {
                    let p = buf.get_pixel(x * ss + dx, y * ss + dy);
                    acc[0] += p[0] as u32;
                    acc[1] += p[1] as u32;
                    acc[2] += p[2] as u32;
                }
            }
            let n = (ss * ss) as u32;
            out.put_pixel(
                x,
                y,
                Rgb([
                    (acc[0] / n) as u8,
                    (acc[1] / n) as u8,
                    (acc[2] / n) as u8,
                ]),
            );
        }
    }
    // Media segments (sprites / text) composite on the final image.
    // Wave 11 (E8): the gate used to check node media only, so a
    // project whose only media was a FREE-STANDING text or sprite
    // instance never composited at all — `media_items` has always
    // included those; the gate just never let them through. Load-
    // bearing now that MCP's text tools author stage texts.
    // Wave 14 (C3): the clips clause is the raster half of the
    // clip-def media fix (docs/wave13/CLIP_MEDIA.md §6.4) — the
    // funnel's per-node fallback (C1, §6.1) samples clip-def media
    // for clip instances, but a project whose ONLY media lives on
    // a clip def would still skip the composite entirely without
    // it. SVG and the stage have no equivalent gate; they call
    // the funnel unconditionally.
    let has_media = frame_has_media(project);
    if has_media {
        let mut rgba = image::DynamicImage::ImageRgb8(out).to_rgba8();
        // `view` returns supersampled buffer coordinates;
        // dividing by `ss` (not `px`) lands on OUTPUT pixels
        // at any scale, since the buffer is ss × the output
        // size (R10).
        let ssf = ss as f32;
        crate::media::composite_media(project, frame_f, &mut rgba, &|p| {
            let v = view(p);
            [v[0] / ssf, v[1] / ssf]
        });
        out = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    }
    // Wave P (P26): subtitle burn-in. When cues are live at
    // this frame, their text is drawn into the finished frame
    // through the SAME text path stage texts take —
    // `composite_media`'s built-in 3x5 bitmap-font stamp — so
    // every raster format built on this funnel (GIF, MP4,
    // WebM, PNG/JPEG sequences, spritesheet, contact sheet)
    // carries subtitles with no per-format work.
    // `render_frame_at` holds a `&Project`, not an
    // `EditorCore`, so the lookup is the core's shared one,
    // `subtitles::live_subtitle_cues`, over
    // `Project::subtitles` under the half-open liveness
    // rule: EVERY cue live at the nearest authored frame,
    // in stored order.
    if let Some(overlay) = subtitle_overlay(project, frame_f, scale) {
        let mut rgba = image::DynamicImage::ImageRgb8(out).to_rgba8();
        crate::media::composite_media(&overlay, frame_f, &mut rgba, &|p| p);
        out = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    }
    out
}

/// Porter-Duff `over` for one source colour at coverage
/// `a` onto a premultiplied f32 accumulator `[r, g, b, a]`
/// (colour channels in 0..=255 space, premultiplied by the
/// accumulated alpha). The alpha funnel's one blend: every
/// drawable the RGB pass composites with
/// `style::blend_over` or its inline particle lerp goes
/// through here instead, against a transparent destination.
/// `a` is clamped to [0, 1], `blend_over`'s own discipline.
fn over_premult(acc: &mut [f32; 4], color: [u8; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    acc[0] = color[0] as f32 * a + acc[0] * inv;
    acc[1] = color[1] as f32 * a + acc[1] * inv;
    acc[2] = color[2] as f32 * a + acc[2] * inv;
    acc[3] = a + acc[3] * inv;
}

/// The RGBA counterpart of [`render_frame_at_scaled_inner`]
/// (Wave X, X8 — SpriteLoop GAP P-5): the same collection
/// ([`collect_frame_art`]), the same coverage tests in the
/// same order, the same media and subtitle tails — but the
/// pixel pass composites over TRANSPARENT instead of the
/// project's background colour and carries coverage alpha
/// out. Accumulation is premultiplied f32, so the
/// supersample box downsample averages premultiplied
/// subpixels and un-premultiplies once at the end: a
/// half-covered edge pixel carries alpha ≈ 0.5 with the
/// covered part's true colour, never a background-tinted
/// fringe. Final quantisation rounds; unlike the RGB
/// funnel there is no historical byte stream to preserve.
fn render_frame_at_scaled_inner_alpha(
    project: &Project,
    frame_f: f32,
    scale: f32,
) -> image::RgbaImage {
    let ss = 2u32; // supersample factor, as in the RGB funnel
    let px = ss as f32 * scale;
    let (ow, oh) = scaled_output_size(project, scale);
    let (w, h) = (ow * ss, oh * ss);
    let (cam_x, cam_y, zoom) = project.camera_at(frame_f);
    let angle = project.camera_angle_at(frame_f);
    let view = |p: [f32; 2]| -> [f32; 2] {
        let (dx, dy) = (p[0] - cam_x, p[1] - cam_y);
        let (ox, oy) = if angle == 0.0 {
            (dx, dy)
        } else {
            let r = camera_rotate_offset(dx, dy, angle);
            (r[0], r[1])
        };
        [
            (ox * zoom + project.stage[0] / 2.0) * px,
            (oy * zoom + project.stage[1] / 2.0) * px,
        ]
    };
    let head_r = project.head_ring_radius() * zoom * px;

    let FrameArt {
        segments,
        heads,
        rings,
        polys,
        fills,
        particles,
    } = collect_frame_art(project, frame_f, &view, px, zoom);

    // The pixel pass: the RGB pass's coverage tests in the
    // RGB pass's order, every hit composited premultiplied
    // over transparent via `over_premult`.
    let mut acc: Vec<[f32; 4]> = vec![[0.0; 4]; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = [x as f32, y as f32];
            let mut pxa = [0.0f32; 4];
            for (verts, color, opacity) in &fills {
                if point_in_polygon(p, verts) {
                    over_premult(&mut pxa, *color, *opacity);
                }
            }
            for seg in &segments {
                if let Some(c) = styled_seg_color(p, seg) {
                    over_premult(&mut pxa, rgb(c), seg.opacity);
                }
            }
            for (c, color, half, alpha) in &heads {
                let d = dist(p, *c);
                if (d - head_r).abs() <= *half {
                    over_premult(&mut pxa, *color, *alpha);
                }
            }
            for (c, r, color, half, alpha) in &rings {
                let d = dist(p, *c);
                if (d - r).abs() <= *half {
                    over_premult(&mut pxa, *color, *alpha);
                }
            }
            for (verts, color, half, alpha) in &polys {
                for k in 0..verts.len() {
                    if seg_dist(p, verts[k], verts[(k + 1) % verts.len()]) <= *half {
                        over_premult(&mut pxa, *color, *alpha);
                    }
                }
            }
            for (c, r, color, alpha) in &particles {
                if dist(p, *c) <= *r {
                    over_premult(&mut pxa, *color, *alpha);
                }
            }
            acc[(y * w + x) as usize] = pxa;
        }
    }
    // Downsample by the supersample factor: average the
    // premultiplied subpixels and their alphas, then
    // un-premultiply once — the coverage-correct box filter.
    let mut out: image::RgbaImage = ImageBuffer::new(ow, oh);
    for y in 0..oh {
        for x in 0..ow {
            let mut sum = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let s = acc[((y * ss + dy) * w + (x * ss + dx)) as usize];
                    sum[0] += s[0];
                    sum[1] += s[1];
                    sum[2] += s[2];
                    sum[3] += s[3];
                }
            }
            let n = (ss * ss) as f32;
            let a = sum[3] / n;
            let (r, g, b) = if a > 0.0 {
                (sum[0] / n / a, sum[1] / n / a, sum[2] / n / a)
            } else {
                (0.0, 0.0, 0.0)
            };
            out.put_pixel(
                x,
                y,
                image::Rgba([
                    r.round().clamp(0.0, 255.0) as u8,
                    g.round().clamp(0.0, 255.0) as u8,
                    b.round().clamp(0.0, 255.0) as u8,
                    (a * 255.0).round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    // Media segments composite on the final image, exactly
    // as in the RGB funnel (same gate, same view) — the
    // buffer is already RGBA here, so sprite and text alpha
    // compose over the frame's own transparency.
    if frame_has_media(project) {
        let ssf = ss as f32;
        crate::media::composite_media(project, frame_f, &mut out, &|p| {
            let v = view(p);
            [v[0] / ssf, v[1] / ssf]
        });
    }
    // Subtitle burn-in, the RGB funnel's tail at parity:
    // the same overlay, stamped through the same funnel.
    if let Some(overlay) = subtitle_overlay(project, frame_f, scale) {
        crate::media::composite_media(&overlay, frame_f, &mut out, &|p| p);
    }
    out
}

/// Render a contact sheet: the listed frames as thumbnails in a
/// grid, for a quick visual overview of an animation without
/// opening every frame. Each index is rendered with
/// `render_frame_at` (at `idx as f32`, so no rasterisation is
/// duplicated) and shrunk to `thumb_w` pixels wide, keeping the
/// stage aspect (`thumb_h = round(thumb_w * stage_h / stage_w)`,
/// at least 1). Layout: tiles run left-to-right, top-to-bottom
/// with an 8 px gap between tiles and the same 8 px as outer
/// padding, so for `cols` columns and `rows = ceil(n / cols)`:
///
///   sheet_w = cols * thumb_w + (cols + 1) * 8
///   sheet_h = rows * thumb_h + (rows + 1) * 8
///
/// The sheet background is the fixed dark rgb(26, 29, 36) — it
/// reads as a neutral matte whatever the project's own background
/// is. `cols == 0` is treated as 1 and `thumb_w == 0` as 1, so
/// the function never divides by zero. An empty frame list
/// yields a 1x1 image of the sheet background.
pub fn render_contact_sheet(
    project: &Project,
    frame_indices: &[usize],
    cols: usize,
    thumb_w: u32,
) -> image::RgbaImage {
    const GAP: u32 = 8;
    const BG: [u8; 4] = [26, 29, 36, 255];
    if frame_indices.is_empty() {
        return ImageBuffer::from_pixel(1, 1, image::Rgba(BG));
    }
    let cols = cols.max(1);
    let thumb_w = thumb_w.max(1);
    // Render and shrink every frame first: the grid's row height
    // comes from the (uniform) thumbnail height of the first one.
    let mut thumbs = Vec::with_capacity(frame_indices.len());
    let mut thumb_h = 0u32;
    for &idx in frame_indices {
        let frame = render_frame_at(project, idx as f32);
        if thumb_h == 0 {
            thumb_h = ((thumb_w as f32 * frame.height() as f32
                / frame.width().max(1) as f32)
                .round() as u32)
                .max(1);
        }
        let rgba = image::DynamicImage::ImageRgb8(frame).to_rgba8();
        thumbs.push(image::imageops::resize(
            &rgba,
            thumb_w,
            thumb_h,
            image::imageops::FilterType::Triangle,
        ));
    }
    let rows = (frame_indices.len() + cols - 1) / cols;
    let sheet_w = cols as u32 * thumb_w + (cols as u32 + 1) * GAP;
    let sheet_h = rows as u32 * thumb_h + (rows as u32 + 1) * GAP;
    let mut sheet = ImageBuffer::from_pixel(sheet_w, sheet_h, image::Rgba(BG));
    for (i, thumb) in thumbs.iter().enumerate() {
        let x = GAP + (i % cols) as u32 * (thumb_w + GAP);
        let y = GAP + (i / cols) as u32 * (thumb_h + GAP);
        image::imageops::overlay(&mut sheet, thumb, x as i64, y as i64);
    }
    sheet
}

/// Authored frame indices expanded by per-frame durations
/// (a frame with duration 2 appears twice, rounded, min once).
pub fn expanded_frames(project: &Project) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, frame) in project.frames.iter().enumerate() {
        let reps = (frame.duration.round() as usize).max(1);
        for _ in 0..reps {
            out.push(i);
        }
    }
    out
}

/// Fractional authored-frame time for a playback time in seconds,
/// walking the per-frame durations.
fn authored_at(project: &Project, seconds: f32) -> f32 {
    let fps = project.fps.max(1.0);
    let mut t = seconds * fps; // in frame units
    for (i, frame) in project.frames.iter().enumerate() {
        if t < frame.duration {
            return i as f32 + t / frame.duration;
        }
        t -= frame.duration;
    }
    (project.frames.len() - 1) as f32
}

/// Frame times (fractional authored frames) for a full pass at the
/// project fps: tween-sampled when tween playback is on, otherwise
/// the duration-expanded authored frames.
fn sample_times(project: &Project) -> Vec<f32> {
    if project.tween_playback {
        let count = (project.total_seconds() * project.fps.max(1.0)).ceil().max(1.0) as usize;
        (0..count)
            .map(|k| authored_at(project, k as f32 / project.fps.max(1.0)))
            .collect()
    } else {
        expanded_frames(project)
            .into_iter()
            .map(|i| i as f32)
            .collect()
    }
}

/// Clamp an inclusive authored-frame range `from..=to` to the
/// timeline. Both ends clamp into `0..frames.len()`; a range that
/// is empty after clamping (including an inverted range or an
/// empty project) is an `InvalidInput` error.
fn clamp_frame_range(project: &Project, from: usize, to: usize) -> std::io::Result<(usize, usize)> {
    let n = project.frames.len();
    if n == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cannot export a frame range from an empty project",
        ));
    }
    let from = from.min(n - 1);
    let to = to.min(n - 1);
    if from > to {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("empty frame range: frame {from} comes after frame {to}"),
        ));
    }
    Ok((from, to))
}

/// Frame times for the authored range `from..=to` only, following
/// the same sampling rules as [`sample_times`]: with tween
/// playback the range's span is sampled at the project fps
/// starting at the range's offset in the timeline; without it, the
/// range's authored frames are expanded by their own durations.
/// For the full range this returns exactly what `sample_times`
/// returns.
fn sample_times_range(project: &Project, from: usize, to: usize) -> Vec<f32> {
    if project.tween_playback {
        let fps = project.fps.max(1.0);
        let start: f32 = project.frames[..from].iter().map(|f| f.duration).sum();
        let span: f32 = project.frames[from..=to].iter().map(|f| f.duration).sum();
        let count = ((span / fps) * fps).ceil().max(1.0) as usize;
        (0..count)
            .map(|k| authored_at(project, (start + k as f32) / fps))
            .collect()
    } else {
        expanded_frames(project)
            .into_iter()
            .filter(|&i| (from..=to).contains(&i))
            .map(|i| i as f32)
            .collect()
    }
}

/// Encode the given frame times as a looping GIF at the project
/// fps (the shared encoder behind the GIF exports).
fn encode_gif(project: &Project, times: &[f32], path: &Path) -> std::io::Result<usize> {
    use image::codecs::gif::{GifEncoder, Repeat};
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = GifEncoder::new_with_speed(file, 10);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(std::io::Error::other)?;
    let delay = image::Delay::from_numer_denom_ms((1000.0 / project.fps.max(1.0)) as u32, 1);
    let mut frames = Vec::new();
    for t in times {
        let img = image::DynamicImage::ImageRgb8(render_frame_at(project, *t)).to_rgba8();
        frames.push(image::Frame::from_parts(img, 0, 0, delay));
    }
    let n = frames.len();
    encoder.encode_frames(frames).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Encode the given frame times as a looping GIF at the project
/// fps at `scale` times the stage size (Wave S, S4): the scaled
/// counterpart of [`encode_gif`], rendering every sample through
/// [`render_frame_at_scaled`] — a true re-render at the scaled
/// size, never a resample of the 1.0 raster.
fn encode_gif_scaled(
    project: &Project,
    times: &[f32],
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    use image::codecs::gif::{GifEncoder, Repeat};
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = GifEncoder::new_with_speed(file, 10);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(std::io::Error::other)?;
    let delay = image::Delay::from_numer_denom_ms((1000.0 / project.fps.max(1.0)) as u32, 1);
    let mut frames = Vec::new();
    for t in times {
        let img =
            image::DynamicImage::ImageRgb8(render_frame_at_scaled(project, *t, scale)?).to_rgba8();
        frames.push(image::Frame::from_parts(img, 0, 0, delay));
    }
    let n = frames.len();
    encoder.encode_frames(frames).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Export the animation as a looping GIF at the project fps.
pub fn export_gif(project: &Project, path: &Path) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        return encode_gif(project, &[], path);
    }
    export_gif_range(project, 0, project.frames.len() - 1, path)
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as a looping GIF at the project fps. Frames keep
/// their own durations (duration-expanded or tween-sampled,
/// exactly as in a full export). Returns the number of GIF frames
/// written; an empty or inverted range is an error.
pub fn export_gif_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_gif(project, &times, path)
}

/// Export the animation as a looping GIF at the project fps, at
/// `scale` times the stage size (Wave S, S4) — the export-scale
/// counterpart of [`export_gif`]. Every frame is a true re-render
/// at the scaled size via [`render_frame_at_scaled`]. At exactly
/// 1.0 this delegates to [`export_gif`], so the output is
/// byte-identical. An invalid scale is refused before anything
/// is written.
pub fn export_gif_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_gif(project, path);
    }
    if project.frames.is_empty() {
        return encode_gif_scaled(project, &[], path, scale);
    }
    export_gif_range_scaled(project, 0, project.frames.len() - 1, path, scale)
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as a looping GIF at the project fps, at `scale`
/// times the stage size (Wave S, S4) — the scaled counterpart of
/// [`export_gif_range`], with the same frame sampling and range
/// rules. At exactly 1.0 this delegates to [`export_gif_range`],
/// so the output is byte-identical. An invalid scale is refused
/// before the range is even clamped, so a refused call writes
/// nothing.
pub fn export_gif_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_gif_range(project, from, to, path);
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_gif_scaled(project, &times, path, scale)
}

/// One progress update from the **encode phase** of an MP4 /
/// WebM export (Wave U, U11): the phase that begins once every
/// sampled frame has been rendered to the temporary PNG
/// sequence and `ffmpeg` is turning that sequence into video.
///
/// `encoded_frames` is the encoder's own output-frame count,
/// read from ffmpeg's machine-readable `-progress` stream —
/// never the render loop's counter, so an observer can tell
/// the two phases apart by construction. `total_frames` is
/// the number of sampled frame times handed to the encoder
/// (duration-expanded / tween-sampled, exactly the count the
/// export returns on success), **not** the authored frame
/// count.
///
/// Delivery contract for the `*_with_encode_progress` entries:
///
/// - While ffmpeg runs, an event fires whenever the encoder's
///   reported frame count advances: `finished == false`,
///   `encoded_frames` strictly increasing, always within
///   `1..=total_frames`. The encoder's opening `frame=0`
///   block is suppressed (no output frame exists yet), and
///   duplicate / regressive reports are dropped.
/// - Exactly one terminal event fires, and only after ffmpeg
///   has exited successfully and its progress stream has been
///   fully drained: `encoded_frames == total_frames`,
///   `finished == true`. A spawn failure or a non-zero exit
///   produces no terminal event.
/// - `finished` closes the **video encode** only. An MP4
///   export with audio still has its separate mux step after
///   the encode, and that step can still fail the export —
///   the terminal event is not an export-success signal.
/// - Passing no callback (every pre-existing entry point)
///   runs the encode exactly as before: no `-progress` flag,
///   no pipe, no thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodeProgressEvent {
    /// Output frames the encoder reports completed so far.
    pub encoded_frames: usize,
    /// Sampled frames the encoder was given in total.
    pub total_frames: usize,
    /// True only on the single terminal event (see above).
    pub finished: bool,
}

/// The encode-progress hook accepted by the
/// `*_with_encode_progress` export entries: `None`, or a
/// shared reference to a callable invoked with each
/// [`EncodeProgressEvent`]. Intermediate events are invoked
/// from the encode's drainer thread (hence `Sync`); the
/// terminal event is invoked from the caller's thread. A
/// panicking callback cannot fail or hang the export — the
/// panic is contained at the call site and that event is
/// dropped.
pub type EncodeProgressCallback<'a> =
    Option<&'a (dyn Fn(EncodeProgressEvent) + Sync)>;

/// One progress update from the **render phase** of a frame
/// sequence export (Wave V, V12): the loop that renders each
/// authored frame and writes it as its own file
/// (`frame-XXXX.png` / `frame-XXXX.jpg`). Since Wave W
/// (W2) the same type also reports the render loop inside
/// the MP4 / WebM funnels, which renders each sampled frame
/// time to a temporary PNG before ffmpeg encodes it; the
/// `*_with_progress` video entries carry it as their second
/// hook, beside the encode hook. It is the render-phase
/// counterpart of [`EncodeProgressEvent`] — and a separate
/// type by construction, so the two phases' counts can
/// never be conflated (the conflation U11's encode contract
/// was written to exclude).
///
/// `rendered_frames` is the render loop's own count of
/// frames rendered **and written** so far. `total_frames`
/// is the number of authored frames the export will write
/// (the whole timeline for a full export, `to - from + 1`
/// for a range export, after clamping) — exactly the count
/// the export returns on success. Sequence exports render
/// authored frames, never sampled times, so neither count
/// depends on the project fps. A video export's render loop
/// renders sampled frame times instead (duration-expanded /
/// tween-sampled): its `total_frames` is the number of
/// sampled times handed to the funnel — the same total the
/// encode phase reports against, and likewise exactly the
/// frame count the export returns on success.
///
/// Delivery contract for the `*_with_render_progress`
/// sequence entries and the render hook of the
/// `*_with_progress` video entries:
///
/// - Delivery is deterministic and synchronous, unlike the
///   encode phase (whose intermediate count is ffmpeg's
///   wall-clock business): every event fires from the
///   caller's thread, exactly one event per written frame.
///   After frame `k` of `total` is written, an event fires
///   with `rendered_frames == k` — `finished == false` for
///   `k < total`, and the final frame's event is the
///   terminal one (below) rather than an unfinished event.
///   A successful export of `total` frames therefore
///   delivers exactly `total` events, strictly increasing.
///   In a video export the phases are sequential, so the
///   whole render stream is delivered before the encode
///   phase reports anything.
/// - Exactly one terminal event fires, and only when the
///   render loop has written its final frame:
///   `rendered_frames == total_frames`, `finished == true`,
///   delivered last, after the final frame's file is
///   written. For a sequence export that is when the export
///   is about to return `Ok`. A hooked full export of an
///   empty project returns `Ok(0)` and fires exactly one
///   event — that terminal, at `(0, 0)` — after creating
///   the directory.
/// - A refused call (invalid scale, out-of-timeline range)
///   fires no event and writes nothing, exactly as the
///   plain entries behave. A frame whose render or write
///   fails mid-loop propagates the historical error and
///   fires no terminal event; events already delivered
///   stay delivered (each was true when sent) and are
///   never retracted.
/// - `finished` closes the **render phase**. For a
///   sequence export that is also the whole export: the
///   frame files are final output and nothing runs after
///   the loop. For a video export the encode phase — and
///   an MP4's audio mux — still runs after the render
///   terminal, so there the terminal is not an
///   export-success signal (the same caveat the encode
///   phase's own terminal carries about the mux).
/// - Passing no callback (every pre-existing entry point)
///   runs the export exactly as before.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderProgressEvent {
    /// Frames rendered and written so far.
    pub rendered_frames: usize,
    /// Frames the render loop will write in total:
    /// authored frames for a sequence export, sampled
    /// frame times for a video export.
    pub total_frames: usize,
    /// True only on the single terminal event (see above).
    pub finished: bool,
}

/// The render-progress hook accepted by the
/// `*_with_render_progress` sequence export entries and
/// by the `*_with_progress` video export entries (as the
/// render hook beside the encode hook): `None`, or a
/// shared reference to a callable invoked with each
/// [`RenderProgressEvent`]. Every event is invoked from
/// the caller's thread (the render loops run
/// synchronously; there is no worker or drainer thread),
/// and a panicking callback cannot fail the export — the
/// panic is contained at the call site and that event is
/// dropped.
pub type RenderProgressCallback<'a> =
    Option<&'a (dyn Fn(RenderProgressEvent) + Sync)>;

/// The outcome of a successful MP4 export, full or range:
/// the frames written, and the audible clips that had to be
/// left out because their files are missing. A non-empty
/// `missing_audio` is NOT a failure — the video is complete
/// and the export succeeded — but the loss is named here so
/// the entry points can tell the user, instead of the mux
/// plan dropping the clips without a trace (the plan's own
/// business stays deciding what sounds: [`mux_plan`]; this
/// outcome is the report of what could not).
///
/// Since Wave X (X12) the WebM RANGE entries
/// ([`export_webm_range`] and its twins) return this same
/// type, so a WebM range export's skipped clips are named
/// exactly as an MP4 range export's are. (The full WebM
/// entry carries no audio at all and keeps its plain
/// frame-count return.)
#[derive(Clone, Debug, PartialEq)]
pub struct Mp4ExportOutcome {
    /// Frames written to the video file.
    pub frames: usize,
    /// Paths of unmuted clips skipped because their files do
    /// not exist, in project order (range exports: only clips
    /// that may intersect the exported window). Empty when
    /// every audible clip was included.
    pub missing_audio: Vec<String>,
}

impl Mp4ExportOutcome {
    /// The one-sentence warning naming the skipped clips, for
    /// status lines — `None` when nothing was skipped, so an
    /// export with all its audio present reports exactly as
    /// it did before this outcome existed.
    pub fn missing_audio_note(&self) -> Option<String> {
        if self.missing_audio.is_empty() {
            None
        } else {
            Some(format!(
                "Skipped {} audio clip(s) whose files are missing: {}.",
                self.missing_audio.len(),
                self.missing_audio.join(", ")
            ))
        }
    }
}

/// Export an MP4 (H.264, yuv420p) via the system `ffmpeg`. When
/// the project has an audio clip whose file exists, it is muxed in
/// at its start frame. Errors if ffmpeg is missing. A clip whose
/// file is missing never fails the export: it is skipped and
/// named in the outcome's `missing_audio` ([`Mp4ExportOutcome`]).
pub fn export_mp4(project: &Project, path: &Path) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_with_encode_progress(project, path, None)
}

/// [`export_mp4`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]): the callback observes the
/// silent-video encode; the audio mux that may follow it
/// reports nothing. With `None` this is `export_mp4` itself.
pub fn export_mp4_with_encode_progress(
    project: &Project,
    path: &Path,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_with_progress(project, path, progress, None)
}

/// [`export_mp4`] with both phase progress hooks (Wave W,
/// W2): the render hook ([`RenderProgressCallback`])
/// observes the funnel's temporary-frame render loop, the
/// encode hook ([`EncodeProgressCallback`]) observes the
/// silent-video encode; the audio mux that may follow
/// reports nothing. Each hook reports its own phase
/// through its own event type — the two are never merged.
pub fn export_mp4_with_progress(
    project: &Project,
    path: &Path,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    // Audible = unmuted with an existing file — the same
    // exclusion `mux_plan` applies. This check used to count
    // muted clips too, so an all-muted project took the mux
    // path, the plan came back None, and the export failed
    // with "audio mux failed" instead of writing the silent
    // video (Wave 11, C9). The range export already treats
    // its None plan as silent success.
    let has_audio = project
        .audio
        .iter()
        .any(|c| !c.muted && std::path::Path::new(&c.path).exists());
    let missing_audio = crate::audio::missing_audio_clips(&project.audio);
    if !has_audio {
        let frames = ffmpeg_encode(project, path, &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"], encode_progress, render_progress)?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("mp4");
    let n = ffmpeg_encode(project, &silent, &["-c:v", "libx264", "-pix_fmt", "yuv420p"], encode_progress, render_progress)?;
    let muxed = mux_audio(project, &silent, path)?;
    let _ = std::fs::remove_file(&silent);
    if muxed {
        Ok(Mp4ExportOutcome {
            frames: n,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Export an MP4 (H.264, yuv420p) via the system `ffmpeg` at
/// `scale` times the stage size (Wave S, S5) — the
/// export-scale counterpart of [`export_mp4`]. Every frame
/// is a true re-render at the scaled size via
/// [`render_frame_at_scaled`]; the audio mux is untouched
/// (audio does not scale), and the missing-audio reporting
/// is [`export_mp4`]'s own. At exactly 1.0 the call routes
/// through `export_mp4` itself, so a 1.0 export is the
/// unscaled export byte for byte. An invalid scale is
/// refused before anything is written.
pub fn export_mp4_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_scaled_with_encode_progress(project, path, scale, None)
}

/// [`export_mp4_scaled`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]): the callback observes the
/// silent-video encode; the audio mux that may follow it
/// reports nothing. At exactly 1.0 the call routes through
/// [`export_mp4_with_encode_progress`], hook included.
pub fn export_mp4_scaled_with_encode_progress(
    project: &Project,
    path: &Path,
    scale: f32,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_scaled_with_progress(project, path, scale, progress, None)
}

/// [`export_mp4_scaled`] with both phase progress hooks
/// (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the scaled
/// funnel's temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the silent-video
/// encode; the audio mux that may follow reports nothing.
/// At exactly 1.0 the call routes through
/// [`export_mp4_with_progress`], both hooks included.
pub fn export_mp4_scaled_with_progress(
    project: &Project,
    path: &Path,
    scale: f32,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_mp4_with_progress(project, path, encode_progress, render_progress);
    }
    let has_audio = project
        .audio
        .iter()
        .any(|c| !c.muted && std::path::Path::new(&c.path).exists());
    let missing_audio = crate::audio::missing_audio_clips(&project.audio);
    if !has_audio {
        let frames = ffmpeg_encode_scaled(
            project,
            path,
            &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"],
            scale,
            encode_progress,
            render_progress,
        )?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("mp4");
    let n = ffmpeg_encode_scaled(
        project,
        &silent,
        &["-c:v", "libx264", "-pix_fmt", "yuv420p"],
        scale,
        encode_progress,
        render_progress,
    )?;
    let muxed = mux_audio(project, &silent, path)?;
    let _ = std::fs::remove_file(&silent);
    if muxed {
        Ok(Mp4ExportOutcome {
            frames: n,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as an MP4 (H.264, yuv420p) via the system
/// `ffmpeg`. Frames keep their own durations (duration-expanded
/// or tween-sampled, exactly as in a full export). An empty or
/// inverted range is an error; errors if ffmpeg is missing.
///
/// AUDIO: the segments of the project's audio that intersect
/// the range are muxed in, trimmed and offset to the range —
/// see [`range_mux_plan`] / [`mux_audio_range`]. Muted clips
/// never sound, exactly as in the full export. A clip's length
/// is probed by decoding it as WAV or, for a non-WAV file
/// (MP3, OGG, …), with `ffprobe` ([`probe_duration_secs`]); a
/// non-WAV segment that has to be mixed — because other
/// segments sound with it, or its own gain is not 1.0 — is
/// decoded to a temporary WAV with `ffmpeg` first, so its
/// mute and gain are honoured exactly as a WAV clip's are. A
/// clip no probe can measure cannot be scheduled into a
/// sub-range: if such a clip may intersect the range, the
/// export fails with an error naming it, before anything is
/// written, rather than silently producing a file without
/// its audio — the same stance the full export takes when a
/// mux fails. A clip whose file is missing entirely is the
/// opposite case: nothing exists to include, so it never
/// fails the export — it is skipped and named in the
/// outcome's `missing_audio` ([`Mp4ExportOutcome`]). When no audible clip intersects the range the
/// export is the plain video-only encode it always was,
/// unchanged.
pub fn export_mp4_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_range_with_encode_progress(project, from, to, path, None)
}

/// [`export_mp4_range`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]): the callback observes the
/// silent-video encode; the range audio mux that may follow
/// it reports nothing. The hook is not invoked at all when
/// the range's audio plan refuses the export before the
/// encode (the unprobeable-clip error above).
pub fn export_mp4_range_with_encode_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_range_with_progress(project, from, to, path, progress, None)
}

/// [`export_mp4_range`] with both phase progress hooks
/// (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the funnel's
/// temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the silent-video
/// encode; the range audio mux that may follow reports
/// nothing. Neither hook is invoked at all when the
/// range's audio plan refuses the export before the
/// encode (the unprobeable-clip error above) — a refusal
/// happens before any frame is rendered.
pub fn export_mp4_range_with_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    let frames = times.len();
    let (win_start, win_len) = range_window(project, from, times.len());
    let outcome = range_plan_outcome(project, win_start, win_len, &probe_duration_secs);
    if !outcome.unprobeable.is_empty() {
        return Err(std::io::Error::other(format!(
            "audio could not be included in the range export: the length of {} \
             clip(s) could not be probed (not readable as WAV, and ffprobe \
             failed or is not installed), so whether they intersect the range \
             is unknown: {}",
            outcome.unprobeable.len(),
            outcome.unprobeable.join(", ")
        )));
    }
    let missing_audio = outcome.missing;
    let plan = outcome.plan;
    if matches!(plan, RangeMuxPlan::None) {
        ffmpeg_encode_times(
            project,
            &times,
            path,
            &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"],
            encode_progress,
            render_progress,
        )?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("mp4");
    ffmpeg_encode_times(
        project,
        &times,
        &silent,
        &["-c:v", "libx264", "-pix_fmt", "yuv420p"],
        encode_progress,
        render_progress,
    )?;
    let muxed = mux_audio_range(&plan, &silent, path)?;
    let _ = std::fs::remove_file(&silent);
    if muxed {
        Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as an MP4 at `scale` times the stage size
/// (Wave S, S5) — the export-scale counterpart of
/// [`export_mp4_range`], with the same range rules, the same
/// unprobeable/missing-audio treatment and the same range mux
/// plan (audio does not scale). At exactly 1.0 the call routes
/// through `export_mp4_range` itself, so a 1.0 export is the
/// unscaled export byte for byte. An invalid scale is refused
/// before the range is even clamped, so a refused call writes
/// nothing.
pub fn export_mp4_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_range_scaled_with_encode_progress(project, from, to, path, scale, None)
}

/// [`export_mp4_range_scaled`] with an encode-phase progress
/// hook ([`EncodeProgressCallback`]): the callback observes
/// the silent-video encode; the range audio mux that may
/// follow it reports nothing. At exactly 1.0 the call routes
/// through [`export_mp4_range_with_encode_progress`], hook
/// included.
pub fn export_mp4_range_scaled_with_encode_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_mp4_range_scaled_with_progress(project, from, to, path, scale, progress, None)
}

/// [`export_mp4_range_scaled`] with both phase progress
/// hooks (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the scaled
/// funnel's temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the silent-video
/// encode; the range audio mux that may follow reports
/// nothing. At exactly 1.0 the call routes through
/// [`export_mp4_range_with_progress`], both hooks
/// included.
pub fn export_mp4_range_scaled_with_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_mp4_range_with_progress(
            project,
            from,
            to,
            path,
            encode_progress,
            render_progress,
        );
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    let frames = times.len();
    let (win_start, win_len) = range_window(project, from, times.len());
    let outcome = range_plan_outcome(project, win_start, win_len, &probe_duration_secs);
    if !outcome.unprobeable.is_empty() {
        return Err(std::io::Error::other(format!(
            "audio could not be included in the range export: the length of {} \
             clip(s) could not be probed (not readable as WAV, and ffprobe \
             failed or is not installed), so whether they intersect the range \
             is unknown: {}",
            outcome.unprobeable.len(),
            outcome.unprobeable.join(", ")
        )));
    }
    let missing_audio = outcome.missing;
    let plan = outcome.plan;
    if matches!(plan, RangeMuxPlan::None) {
        ffmpeg_encode_times_scaled(
            project,
            &times,
            path,
            &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"],
            scale,
            encode_progress,
            render_progress,
        )?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("mp4");
    ffmpeg_encode_times_scaled(
        project,
        &times,
        &silent,
        &["-c:v", "libx264", "-pix_fmt", "yuv420p"],
        scale,
        encode_progress,
        render_progress,
    )?;
    let muxed = mux_audio_range(&plan, &silent, path)?;
    let _ = std::fs::remove_file(&silent);
    if muxed {
        Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Deliver one encode-progress event to the hook, if any.
/// A panicking observer is contained here: it drops that one
/// event and can neither fail the export nor kill the
/// drainer thread (which would risk the child blocking on a
/// full progress pipe).
fn report_encode_progress(progress: EncodeProgressCallback<'_>, event: EncodeProgressEvent) {
    if let Some(callback) = progress {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    }
}

/// Deliver one render-progress event to the hook, if any —
/// the render phase's counterpart of
/// [`report_encode_progress`], with the same containment:
/// a panicking observer drops that one event and can never
/// fail the export.
fn report_render_progress(progress: RenderProgressCallback<'_>, event: RenderProgressEvent) {
    if let Some(callback) = progress {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    }
}

/// The frame total a full video export's encode phase
/// reports against: the number of sampled frame times in
/// one full pass over the timeline (duration-expanded /
/// tween-sampled — see [`EncodeProgressEvent`], whose
/// `total_frames` is exactly this count, and exactly the
/// count a successful full video export returns). Exposed
/// for the bundle progress contract below: a video bundle
/// member's share of the bundle total must be this number,
/// allocated before the member runs, and the sampling
/// itself is this module's private business.
pub fn sampled_frame_total(project: &Project) -> usize {
    sample_times(project).len()
}

/// One progress update from a whole **bundle** export
/// (Wave X, X11): several member exports run under one
/// request (the `export_bundle` tool's image members —
/// PNG sequence, JPEG sequence, SVG, spritesheet — or its
/// video members — MP4, WebM), and this is the one count
/// they share. It exists because the members' own phase
/// contracts cannot be forwarded raw: each member's
/// counter starts at its own 1, so one observer carrying
/// the members' events in turn would watch the count
/// reset at every member boundary — progress running
/// backwards. [`BundleProgress`] aggregates the members'
/// events into this type instead.
///
/// `total_frames` is the sum of every member's frame
/// total in that member's own reporting sense — authored
/// frames for the sequence/SVG/spritesheet members,
/// sampled frame times ([`sampled_frame_total`]) for the
/// video members — computed and fixed before the first
/// member runs; it never changes during the bundle.
/// `completed_frames` counts the frames the members have
/// verifiably completed: a hooked member's phase events
/// advance it as they fire, and a member with no phase
/// contract of its own (SVG, spritesheet) contributes
/// its frames atomically when it completes — during such
/// a member the count lags, but it never overstates.
///
/// Delivery contract for the bundle stream a
/// [`BundleProgress`] tracker emits:
///
/// - Emitted `completed_frames` values strictly
///   increase, with one exception: the terminal event
///   (below) may repeat the total the last member's own
///   terminal translation already reached. `finished`
///   is what distinguishes it. Member events are
///   clamped to the member's allocated share and
///   duplicates / regressions are dropped, so no
///   member can push the count past its share or
///   backwards.
/// - Exactly one terminal event fires —
///   `completed_frames == total_frames`,
///   `finished == true`, delivered last — and only
///   when **every** member succeeded, signalled by the
///   driver's `finish()` call after the last member.
///   If a member fails, no terminal event fires (the
///   member contracts' own rule: a failure produces no
///   terminal); counts already delivered stay delivered
///   — each was true when sent — and a later member's
///   counts continue from the failed member's reported
///   high-water mark rather than retracting, so the
///   stream cannot run backwards even across a failure.
/// - A tracker over zero members fires exactly the one
///   terminal at `(0, 0)` from `finish()` alone — the
///   sequence contract's empty-project leg, one level
///   up.
/// - Events are delivered synchronously from whichever
///   thread the running member's own contract delivers
///   on (the caller's thread for render members, the
///   encode drainer thread for video members), serialised
///   by the tracker. A panicking observer cannot fail
///   any member export or the bundle — the panic is
///   contained at the call site and that event is
///   dropped, exactly as in the phase contracts.
/// - `finished` closes the **bundle**, and the bundle is
///   a set of independent member exports: the driver's
///   reply remains the only per-member success record.
///   With no terminal (a member failed) the reply still
///   arrives and names the failure, as it always has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundleProgressEvent {
    /// Frames completed across all members so far.
    pub completed_frames: usize,
    /// Every member's frame total, summed and allocated
    /// before the first member ran.
    pub total_frames: usize,
    /// True only on the single terminal event (see above).
    pub finished: bool,
}

/// The bundle-progress hook accepted by
/// [`BundleProgress`]: `None`, or a shared reference to a
/// callable invoked with each [`BundleProgressEvent`].
/// Events may be invoked from a member encode's drainer
/// thread as well as the caller's (hence `Sync`); a
/// panicking callback cannot fail any export — the panic
/// is contained at the call site and that event is
/// dropped.
pub type BundleProgressCallback<'a> =
    Option<&'a (dyn Fn(BundleProgressEvent) + Sync)>;

/// Deliver one bundle-progress event to the hook, if
/// any — the bundle counterpart of
/// [`report_encode_progress`], with the same containment:
/// a panicking observer drops that one event and can
/// neither fail a member export nor kill a drainer
/// thread.
fn report_bundle_progress(progress: BundleProgressCallback<'_>, event: BundleProgressEvent) {
    if let Some(callback) = progress {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    }
}

/// Aggregates one bundle's member exports into the
/// single [`BundleProgressEvent`] stream (Wave X, X11).
/// Construct with the member frame totals **in run
/// order**, allocated before the first member runs; then,
/// for each member in order: take its translator
/// ([`BundleProgress::render_callback`] or
/// [`BundleProgress::encode_callback`]) immediately
/// before running it, pass it to the member's hooked
/// entry, and call [`BundleProgress::complete_member`]
/// with the member's outcome before taking the next
/// member's translator. After the last member, call
/// [`BundleProgress::finish`] once. Members with no
/// phase contract take no translator — `complete_member`
/// alone banks their frames.
///
/// The tracker is the honesty mechanism of the bundle
/// contract: the member translators bank against the
/// frames banked at the moment they were taken, clamp
/// every member event to that member's allocated share,
/// and emit only strict advances, so the emitted stream
/// is monotonic by construction whatever the members do.
pub struct BundleProgress<'a> {
    progress: BundleProgressCallback<'a>,
    /// Each member's allocated frame total, in run order.
    member_totals: Vec<usize>,
    /// The bundle total: `member_totals` summed once, at
    /// construction, before any member ran.
    total_frames: usize,
    state: std::sync::Mutex<BundleProgressState>,
}

/// The tracker's mutable aggregation state, behind one
/// mutex: member events can arrive from an encode
/// drainer thread while the driver thread is between
/// members, so every translation is serialised.
#[derive(Default)]
struct BundleProgressState {
    /// Frames banked from completed members: a succeeded
    /// member banks its whole allocation; a failed one
    /// banks only its reported peak (delivered counts
    /// are never retracted).
    banked: usize,
    /// The running member's reported peak, in that
    /// member's own count (clamped to its allocation).
    running_peak: usize,
    /// The last `completed_frames` value emitted — the
    /// monotonic floor of the stream.
    emitted: usize,
    /// Members the driver has completed so far.
    members_done: usize,
    /// Any member failed: the terminal can never fire.
    any_failed: bool,
    /// The terminal event has been emitted.
    terminal_sent: bool,
}

impl<'a> BundleProgress<'a> {
    /// A tracker over `member_totals` (frame totals in
    /// run order, allocated before the first member
    /// runs), reporting to `progress`.
    pub fn new(member_totals: &[usize], progress: BundleProgressCallback<'a>) -> Self {
        BundleProgress {
            progress,
            member_totals: member_totals.to_vec(),
            total_frames: member_totals.iter().sum(),
            state: std::sync::Mutex::new(BundleProgressState::default()),
        }
    }

    /// The bundle total allocated at construction: the
    /// `total_frames` every emitted event carries.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// The render-phase translator for member `member`
    /// (an index into the construction totals): take it
    /// immediately before running that member's hooked
    /// entry, after the previous member's
    /// `complete_member`. The returned closure translates
    /// the member's [`RenderProgressEvent`]s into bundle
    /// events against the frames banked when it was
    /// taken.
    pub fn render_callback(
        &self,
        member: usize,
    ) -> impl Fn(RenderProgressEvent) + Sync + '_ {
        let (base, allocated) = self.member_window(member);
        move |event: RenderProgressEvent| {
            self.translate(base, allocated, event.rendered_frames);
        }
    }

    /// The encode-phase translator for member `member` —
    /// the counterpart of
    /// [`BundleProgress::render_callback`] for a video
    /// member's [`EncodeProgressEvent`]s.
    pub fn encode_callback(
        &self,
        member: usize,
    ) -> impl Fn(EncodeProgressEvent) + Sync + '_ {
        let (base, allocated) = self.member_window(member);
        move |event: EncodeProgressEvent| {
            self.translate(base, allocated, event.encoded_frames);
        }
    }

    /// The frames banked when member `member`'s
    /// translator is taken, and that member's allocated
    /// share. Members run strictly in order, so the
    /// banked value cannot move while the member runs —
    /// it changes only in `complete_member`, between
    /// members.
    fn member_window(&self, member: usize) -> (usize, usize) {
        let allocated = self.member_totals[member];
        let state = self.lock_state();
        (state.banked, allocated)
    }

    /// Translate one member event: clamp the member's
    /// own count to its allocation, add the banked base,
    /// and emit only on a strict advance of the bundle
    /// count. The member's own terminal event translates
    /// like any other — the bundle terminal is
    /// `finish()`'s, never a member's.
    fn translate(&self, base: usize, allocated: usize, member_done: usize) {
        let clamped = member_done.min(allocated);
        let candidate = base + clamped;
        let mut state = self.lock_state();
        if clamped > state.running_peak {
            state.running_peak = clamped;
        }
        if candidate > state.emitted {
            state.emitted = candidate;
            report_bundle_progress(
                self.progress,
                BundleProgressEvent {
                    completed_frames: candidate,
                    total_frames: self.total_frames,
                    finished: false,
                },
            );
        }
    }

    /// Bank the member the driver just ran, in run
    /// order: on success its whole allocation banks; on
    /// failure only its reported peak banks (delivered
    /// counts are never retracted) and the bundle
    /// terminal is forfeited. Call once per member,
    /// after the member's entry returns and before the
    /// next member's translator is taken.
    pub fn complete_member(&self, succeeded: bool) {
        let mut state = self.lock_state();
        let member = state.members_done;
        let allocated = self.member_totals.get(member).copied().unwrap_or(0);
        if succeeded {
            state.banked += allocated;
        } else {
            state.banked += state.running_peak;
            state.any_failed = true;
        }
        state.running_peak = 0;
        state.members_done += 1;
    }

    /// Signal that every member has run: fire the
    /// single terminal event at `(total, total)` if —
    /// and only if — every member succeeded. Idempotent:
    /// a second call, or a call after any member failed,
    /// emits nothing.
    pub fn finish(&self) {
        let mut state = self.lock_state();
        if state.terminal_sent
            || state.any_failed
            || state.members_done < self.member_totals.len()
        {
            return;
        }
        state.terminal_sent = true;
        state.emitted = self.total_frames;
        report_bundle_progress(
            self.progress,
            BundleProgressEvent {
                completed_frames: self.total_frames,
                total_frames: self.total_frames,
                finished: true,
            },
        );
    }

    /// Lock the aggregation state, recovering a poisoned
    /// mutex instead of propagating the panic: the only
    /// code running under the lock besides arithmetic is
    /// the contained observer call, so a poisoned lock
    /// means an observer panicked mid-report in a way
    /// the containment already bounded — the counts
    /// remain consistent and the bundle must go on.
    fn lock_state(&self) -> std::sync::MutexGuard<'_, BundleProgressState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The frame count on one line of ffmpeg's `-progress`
/// stream (`frame=  123`), or `None` for every other line —
/// the stream's other values (`fps`, `bitrate`, `out_time_*`,
/// `speed`, the closing `progress=` marker) are never read:
/// several are `N/A` in the opening blocks, `out_time_ms` is
/// microseconds despite its name, and unknown keys from other
/// ffmpeg versions must stay ignorable by construction.
fn parse_ffmpeg_progress_frame(line: &str) -> Option<usize> {
    line.strip_prefix("frame=")?.trim().parse().ok()
}

/// Drain one ffmpeg `-progress` stream to EOF, reporting
/// each advancing encoder frame count (see
/// [`EncodeProgressEvent`] for the delivery contract). Runs
/// on a dedicated thread for the child's whole life: the
/// stream is wall-clock-bound (hundreds of bytes per second),
/// so an undrained pipe would fill its buffer after a few
/// minutes of encode and deadlock the child. The terminal
/// event is NOT sent here — only the runner knows the exit
/// status — so a failed encode never reports `finished`.
fn drain_ffmpeg_progress<R: std::io::Read>(
    reader: R,
    total_frames: usize,
    progress: EncodeProgressCallback<'_>,
) {
    use std::io::BufRead as _;
    let mut reader = std::io::BufReader::new(reader);
    let mut last_reported = 0usize;
    let mut raw_line: Vec<u8> = Vec::new();
    loop {
        raw_line.clear();
        match reader.read_until(b'\n', &mut raw_line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = String::from_utf8_lossy(&raw_line);
        if let Some(frame) = parse_ffmpeg_progress_frame(&line) {
            let done = frame.min(total_frames);
            if done > last_reported {
                last_reported = done;
                report_encode_progress(
                    progress,
                    EncodeProgressEvent {
                        encoded_frames: done,
                        total_frames,
                        finished: false,
                    },
                );
            }
        }
    }
}

/// Run one ffmpeg video encode (the process tail of
/// [`ffmpeg_encode_times`] / [`ffmpeg_encode_times_scaled`];
/// `args` is the full argument list, output path last, and
/// `total_frames` the sampled frames rendered into the PNG
/// sequence). With no progress hook the invocation is the
/// historical one — nulled streams, one blocking `status()`.
/// With a hook, ffmpeg additionally gets `-nostats -progress
/// pipe:1` (global options, at the head of the argument
/// list), stdout is piped, and a drainer thread reads the
/// progress stream for the child's whole life. An ffmpeg too
/// old to know `-progress` (pre-1.0, 2012) fails a hooked
/// encode with the usual `ffmpeg exited with …` error; the
/// unhooked invocation is untouched by that risk, and no
/// capability probe or silent retry is attempted.
fn run_ffmpeg_encode(
    args: &[String],
    total_frames: usize,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<()> {
    if progress.is_none() {
        return match std::process::Command::new("ffmpeg")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(std::io::Error::other(format!(
                "ffmpeg exited with {status}"
            ))),
            Err(e) => Err(std::io::Error::other(format!(
                "could not run ffmpeg: {e}"
            ))),
        };
    }
    let mut progress_args: Vec<String> =
        vec!["-nostats".into(), "-progress".into(), "pipe:1".into()];
    progress_args.extend_from_slice(args);
    let mut child = std::process::Command::new("ffmpeg")
        .args(&progress_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| std::io::Error::other(format!("could not run ffmpeg: {e}")))?;
    let stdout = child.stdout.take();
    let status = std::thread::scope(|scope| {
        let drainer = stdout.map(|stdout| {
            scope.spawn(move || drain_ffmpeg_progress(stdout, total_frames, progress))
        });
        let status = child.wait();
        if let Some(drainer) = drainer {
            let _ = drainer.join();
        }
        status
    });
    match status {
        Ok(status) if status.success() => {
            if total_frames > 0 {
                report_encode_progress(
                    progress,
                    EncodeProgressEvent {
                        encoded_frames: total_frames,
                        total_frames,
                        finished: true,
                    },
                );
            }
            Ok(())
        }
        Ok(status) => Err(std::io::Error::other(format!(
            "ffmpeg exited with {status}"
        ))),
        Err(e) => Err(std::io::Error::other(format!(
            "could not run ffmpeg: {e}"
        ))),
    }
}

/// Encode rendered frames with ffmpeg using the given video args.
static ENCODE_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn unique_temp(tag: &str) -> std::path::PathBuf {
    let seq = ENCODE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("styk-{tag}-{}-{seq}", std::process::id()))
}

/// Encode the whole timeline (see `sample_times`) with ffmpeg
/// using the given video args.
fn ffmpeg_encode(
    project: &Project,
    path: &Path,
    video_args: &[&str],
    progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let times = sample_times(project);
    ffmpeg_encode_times(project, &times, path, video_args, progress, render_progress)
}

/// Encode the given frame times with ffmpeg using the given
/// video args: render each time to a PNG in a temp dir, then run
/// ffmpeg over the sequence at the project fps. Returns the
/// number of frames encoded. Errors if ffmpeg is missing or
/// exits non-zero. The render loop reports through
/// `render_progress` (see [`RenderProgressEvent`]): one
/// event per temporary frame written, the terminal when the
/// loop hands over to the encode — which reports separately
/// through `progress`.
fn ffmpeg_encode_times(
    project: &Project,
    times: &[f32],
    path: &Path,
    video_args: &[&str],
    progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = unique_temp("enc");
    std::fs::create_dir_all(&tmp)?;
    let total = times.len();
    for (k, t) in times.iter().enumerate() {
        render_frame_at(project, *t)
            .save(tmp.join(format!("frame-{:04}.png", k + 1)))
            .map_err(std::io::Error::other)?;
        let done = k + 1;
        if done < total {
            report_render_progress(
                render_progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        render_progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-framerate".into(),
        format!("{}", project.fps.max(1.0) as i32),
        "-i".into(),
        tmp.join("frame-%04d.png").to_string_lossy().into_owned(),
    ];
    args.extend(video_args.iter().map(|a| a.to_string()));
    args.push(path.to_string_lossy().into_owned());
    let result = run_ffmpeg_encode(&args, times.len(), progress);
    let _ = std::fs::remove_dir_all(&tmp);
    result.map(|()| times.len())
}

/// Encode the whole timeline (see `sample_times`) with ffmpeg
/// at `scale` times the stage size (Wave S, S5) — the
/// export-scale counterpart of [`ffmpeg_encode`]. At exactly
/// 1.0 it routes through `ffmpeg_encode` itself, so a 1.0
/// encode is the unscaled encode byte for byte.
fn ffmpeg_encode_scaled(
    project: &Project,
    path: &Path,
    video_args: &[&str],
    scale: f32,
    progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if scale == 1.0 {
        return ffmpeg_encode(project, path, video_args, progress, render_progress);
    }
    let times = sample_times(project);
    ffmpeg_encode_times_scaled(
        project,
        &times,
        path,
        video_args,
        scale,
        progress,
        render_progress,
    )
}

/// Encode the given frame times with ffmpeg at `scale` times
/// the stage size (Wave S, S5): every sampled frame is a true
/// re-render via [`render_frame_at_scaled`], never an upscale
/// of the 1.0 raster — [`ffmpeg_encode_times`] with the scaled
/// frame funnel swapped in. At exactly 1.0 it routes through
/// `ffmpeg_encode_times` itself.
fn ffmpeg_encode_times_scaled(
    project: &Project,
    times: &[f32],
    path: &Path,
    video_args: &[&str],
    scale: f32,
    progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if scale == 1.0 {
        return ffmpeg_encode_times(project, times, path, video_args, progress, render_progress);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = unique_temp("enc");
    std::fs::create_dir_all(&tmp)?;
    let total = times.len();
    for (k, t) in times.iter().enumerate() {
        render_frame_at_scaled(project, *t, scale)?
            .save(tmp.join(format!("frame-{:04}.png", k + 1)))
            .map_err(std::io::Error::other)?;
        let done = k + 1;
        if done < total {
            report_render_progress(
                render_progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        render_progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-framerate".into(),
        format!("{}", project.fps.max(1.0) as i32),
        "-i".into(),
        tmp.join("frame-%04d.png").to_string_lossy().into_owned(),
    ];
    args.extend(video_args.iter().map(|a| a.to_string()));
    args.push(path.to_string_lossy().into_owned());
    let result = run_ffmpeg_encode(&args, times.len(), progress);
    let _ = std::fs::remove_dir_all(&tmp);
    result.map(|()| times.len())
}

/// Export a WebM (VP9). Frames are opaque; the alpha channel is
/// present but fully opaque.
pub fn export_webm(project: &Project, path: &Path) -> std::io::Result<usize> {
    export_webm_with_encode_progress(project, path, None)
}

/// [`export_webm`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]). With `None` this is
/// `export_webm` itself.
pub fn export_webm_with_encode_progress(
    project: &Project,
    path: &Path,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<usize> {
    export_webm_with_progress(project, path, progress, None)
}

/// [`export_webm`] with both phase progress hooks (Wave W,
/// W2): the render hook ([`RenderProgressCallback`])
/// observes the funnel's temporary-frame render loop, the
/// encode hook ([`EncodeProgressCallback`]) observes the
/// encode. Each hook reports its own phase through its
/// own event type — the two are never merged.
pub fn export_webm_with_progress(
    project: &Project,
    path: &Path,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    ffmpeg_encode(
        project,
        path,
        &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
        encode_progress,
        render_progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as a WebM (VP9) via the system `ffmpeg`. Frames
/// keep their own durations (duration-expanded or tween-sampled,
/// exactly as in a full export). An empty or inverted range is
/// an error; errors if ffmpeg is missing.
///
/// AUDIO: unlike the full WebM entry, the range entry muxes the
/// segments of the project's audio that intersect the range,
/// trimmed and offset to the range — the same
/// [`range_mux_plan`] / [`range_plan_outcome`] decision and the
/// same trimming/mixing rules as [`export_mp4_range`], with the
/// audio encoded as Opus (the WebM counterpart of the MP4 path's
/// AAC). Muted clips never sound. A clip no probe can measure
/// fails the export before anything is written when it may
/// intersect the range, rather than being dropped silently; a
/// clip whose file is missing contributes no segment and is
/// named in the outcome's `missing_audio` ([`Mp4ExportOutcome`],
/// the shared video-range outcome — Wave X, X12). When no
/// audible clip intersects the range the export is the plain
/// video-only encode it always was.
pub fn export_webm_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<Mp4ExportOutcome> {
    export_webm_range_with_encode_progress(project, from, to, path, None)
}

/// [`export_webm_range`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]): the callback observes the
/// silent-video encode; the range audio mux that may follow
/// it reports nothing. The hook is not invoked at all when
/// the range's audio plan refuses the export before the
/// encode (the unprobeable-clip error above).
pub fn export_webm_range_with_encode_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_webm_range_with_progress(project, from, to, path, progress, None)
}

/// [`export_webm_range`] with both phase progress hooks
/// (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the funnel's
/// temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the silent-video
/// encode; the range audio mux that may follow reports
/// nothing. Neither hook is invoked at all when the
/// range's audio plan refuses the export before the
/// encode (the unprobeable-clip error above) — a refusal
/// happens before any frame is rendered.
pub fn export_webm_range_with_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    let frames = times.len();
    let (win_start, win_len) = range_window(project, from, times.len());
    let outcome = range_plan_outcome(project, win_start, win_len, &probe_duration_secs);
    if !outcome.unprobeable.is_empty() {
        return Err(std::io::Error::other(format!(
            "audio could not be included in the range export: the length of {} \
             clip(s) could not be probed (not readable as WAV, and ffprobe \
             failed or is not installed), so whether they intersect the range \
             is unknown: {}",
            outcome.unprobeable.len(),
            outcome.unprobeable.join(", ")
        )));
    }
    let missing_audio = outcome.missing;
    let plan = outcome.plan;
    if matches!(plan, RangeMuxPlan::None) {
        ffmpeg_encode_times(
            project,
            &times,
            path,
            &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
            encode_progress,
            render_progress,
        )?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("webm");
    let encode_result = ffmpeg_encode_times(
        project,
        &times,
        &silent,
        &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
        encode_progress,
        render_progress,
    );
    let muxed = match encode_result {
        Ok(_) => mux_audio_range_webm(&plan, &silent, path),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_file(&silent);
    if muxed? {
        Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Export a WebM (VP9) at `scale` times the stage size
/// (Wave S, S5) — the export-scale counterpart of
/// [`export_webm`]. Every frame is a true re-render at the
/// scaled size; video only, like the unscaled entry. At
/// exactly 1.0 the call routes through `export_webm` itself,
/// so a 1.0 export is the unscaled export byte for byte. An
/// invalid scale is refused before anything is written.
pub fn export_webm_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_webm_scaled_with_encode_progress(project, path, scale, None)
}

/// [`export_webm_scaled`] with an encode-phase progress hook
/// ([`EncodeProgressCallback`]). At exactly 1.0 the call
/// routes through [`export_webm_with_encode_progress`], hook
/// included.
pub fn export_webm_scaled_with_encode_progress(
    project: &Project,
    path: &Path,
    scale: f32,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<usize> {
    export_webm_scaled_with_progress(project, path, scale, progress, None)
}

/// [`export_webm_scaled`] with both phase progress hooks
/// (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the scaled
/// funnel's temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the encode. At
/// exactly 1.0 the call routes through
/// [`export_webm_with_progress`], both hooks included.
pub fn export_webm_scaled_with_progress(
    project: &Project,
    path: &Path,
    scale: f32,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_webm_with_progress(project, path, encode_progress, render_progress);
    }
    ffmpeg_encode_scaled(
        project,
        path,
        &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
        scale,
        encode_progress,
        render_progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as a WebM at `scale` times the stage size
/// (Wave S, S5) — the export-scale counterpart of
/// [`export_webm_range`], with the same range rules and the
/// same range audio plan (audio does not scale). At
/// exactly 1.0 the call routes through `export_webm_range`
/// itself. An invalid scale is refused before the range is
/// even clamped, so a refused call writes nothing.
pub fn export_webm_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<Mp4ExportOutcome> {
    export_webm_range_scaled_with_encode_progress(project, from, to, path, scale, None)
}

/// [`export_webm_range_scaled`] with an encode-phase progress
/// hook ([`EncodeProgressCallback`]): the callback observes
/// the silent-video encode; the range audio mux that may
/// follow it reports nothing. At exactly 1.0 the call
/// routes through [`export_webm_range_with_encode_progress`],
/// hook included. An invalid scale is refused before the
/// range is even clamped, so a refused call fires no event
/// and writes nothing.
pub fn export_webm_range_scaled_with_encode_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
    progress: EncodeProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    export_webm_range_scaled_with_progress(project, from, to, path, scale, progress, None)
}

/// [`export_webm_range_scaled`] with both phase progress
/// hooks (Wave W, W2): the render hook
/// ([`RenderProgressCallback`]) observes the scaled
/// funnel's temporary-frame render loop, the encode hook
/// ([`EncodeProgressCallback`]) observes the silent-video
/// encode; the range audio mux that may follow reports
/// nothing. At exactly 1.0 the call routes through
/// [`export_webm_range_with_progress`], both hooks
/// included. An invalid scale is refused before the
/// range is even clamped, so a refused call fires no
/// event on either hook and writes nothing.
pub fn export_webm_range_scaled_with_progress(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
    encode_progress: EncodeProgressCallback<'_>,
    render_progress: RenderProgressCallback<'_>,
) -> std::io::Result<Mp4ExportOutcome> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_webm_range_with_progress(
            project,
            from,
            to,
            path,
            encode_progress,
            render_progress,
        );
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    let frames = times.len();
    let (win_start, win_len) = range_window(project, from, times.len());
    let outcome = range_plan_outcome(project, win_start, win_len, &probe_duration_secs);
    if !outcome.unprobeable.is_empty() {
        return Err(std::io::Error::other(format!(
            "audio could not be included in the range export: the length of {} \
             clip(s) could not be probed (not readable as WAV, and ffprobe \
             failed or is not installed), so whether they intersect the range \
             is unknown: {}",
            outcome.unprobeable.len(),
            outcome.unprobeable.join(", ")
        )));
    }
    let missing_audio = outcome.missing;
    let plan = outcome.plan;
    if matches!(plan, RangeMuxPlan::None) {
        ffmpeg_encode_times_scaled(
            project,
            &times,
            path,
            &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
            scale,
            encode_progress,
            render_progress,
        )?;
        return Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        });
    }
    let silent = unique_temp("silent").with_extension("webm");
    let encode_result = ffmpeg_encode_times_scaled(
        project,
        &times,
        &silent,
        &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"],
        scale,
        encode_progress,
        render_progress,
    );
    let muxed = match encode_result {
        Ok(_) => mux_audio_range_webm(&plan, &silent, path),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_file(&silent);
    if muxed? {
        Ok(Mp4ExportOutcome {
            frames,
            missing_audio,
        })
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// What an export should do with the project's audio, decided
/// purely from the clip list, file existence and the project fps.
#[derive(Clone, Debug, PartialEq)]
enum MuxPlan {
    /// No clip has an existing file: export stays silent.
    None,
    /// Exactly one audible clip: mux its file directly, offset by
    /// ffmpeg like the original single-clip path. The clip's
    /// `gain` travels with the plan: `mux_audio` can only honour
    /// a non-unity gain by routing the clip through the mix.
    Single {
        path: String,
        start_secs: f32,
        gain: f32,
    },
    /// Two or more audible clips: mix them into one WAV first.
    /// `(path, start_secs, gain)` triples sorted by start offset.
    Mixed(Vec<(String, f32, f32)>),
}

/// Decide the mux plan for a project: muted clips and clips
/// whose files are missing are skipped (the same exclusion the
/// playback plan applies in `audio::playback_plan`), start
/// offsets are `start_frame / fps` seconds, and the surviving
/// clips are sorted by start offset.
fn mux_plan(project: &Project) -> MuxPlan {
    let fps = project.fps.max(1.0);
    let mut audible: Vec<(String, f32, f32)> = project
        .audio
        .iter()
        .filter(|clip| !clip.muted && std::path::Path::new(&clip.path).exists())
        .map(|clip| {
            (
                clip.path.clone(),
                clip.start_frame as f32 / fps,
                clip.gain,
            )
        })
        .collect();
    audible.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    match audible.len() {
        0 => MuxPlan::None,
        1 => {
            let (path, start_secs, gain) = audible.into_iter().next().unwrap();
            MuxPlan::Single {
                path,
                start_secs,
                gain,
            }
        }
        _ => MuxPlan::Mixed(audible),
    }
}

/// The audio codec each video container carries: MP4 takes
/// AAC; WebM cannot carry AAC, so its range mux encodes the
/// same plan as Opus. The video stream is copied either way.
const MP4_AUDIO_CODEC: &str = "aac";
const WEBM_AUDIO_CODEC: &str = "libopus";

/// Write `mixed` to a temporary WAV and mux it onto the silent
/// MP4 `video` as AAC (see
/// [`mux_mixed_wav_with_audio_codec`]).
fn mux_mixed_wav(
    video: &Path,
    mixed: &crate::audio_edit::WavData,
    out: &Path,
) -> std::io::Result<bool> {
    mux_mixed_wav_with_audio_codec(video, mixed, out, MP4_AUDIO_CODEC)
}

/// Write `mixed` to a temporary WAV and mux it onto the silent
/// `video`, encoding the audio with `audio_codec`. The mix
/// starts at t=0 because each clip's start offset is already
/// baked into it, so the ffmpeg call has no `-itsoffset`. The
/// temp file is removed either way.
fn mux_mixed_wav_with_audio_codec(
    video: &Path,
    mixed: &crate::audio_edit::WavData,
    out: &Path,
    audio_codec: &str,
) -> std::io::Result<bool> {
    let tmp = unique_temp("mixdown").with_extension("wav");
    std::fs::write(&tmp, crate::audio_edit::write_wav(mixed))?;
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            video.to_str().unwrap_or(""),
            "-i",
            tmp.to_str().unwrap_or(""),
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c:v",
            "copy",
            "-c:a",
            audio_codec,
            "-shortest",
            out.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = std::fs::remove_file(&tmp);
    Ok(status?.success())
}

/// Mux the project's audio into an already-encoded silent MP4.
/// With exactly one audible clip at unity gain its file is passed
/// to ffmpeg directly, offset by its start frame (`-itsoffset`).
/// With two or more clips — or a single clip whose gain is not
/// 1.0, which the direct path cannot scale — every clip is first
/// mixed into one WAV with `audio_edit::mix` (each clip placed at
/// its start-frame offset and scaled by its gain) and the mix is
/// muxed instead; the mixed WAV starts at t=0
/// because the offsets are baked into it, so that ffmpeg call has
/// no `-itsoffset`. Clips whose files are missing are skipped; with
/// no audible clip at all this returns `Ok(false)`.
pub fn mux_audio(project: &Project, video: &Path, out: &Path) -> std::io::Result<bool> {
    match mux_plan(project) {
        MuxPlan::None => Ok(false),
        MuxPlan::Single {
            path,
            start_secs,
            gain,
        } => {
            if gain != 1.0 {
                // The direct ffmpeg path cannot scale a file's
                // volume, so a gained lone clip renders a one-clip
                // mix with the gain applied and muxes that. The
                // mix engine reads WAV only: if the clip is not
                // WAV the mix fails and we fall through to the
                // direct path below (gain unapplied), mirroring
                // the multi-clip fallback.
                let mix_clips = vec![crate::audio_edit::MixClip {
                    path: path.clone(),
                    start_secs,
                    gain,
                    fade_in: 0.0,
                    fade_out: 0.0,
                }];
                if let Ok(mixed) = crate::audio_edit::mix(&mix_clips) {
                    return mux_mixed_wav(video, &mixed, out);
                }
            }
            let status = std::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i",
                    video.to_str().unwrap_or(""),
                    "-itsoffset",
                    &format!("{start_secs:.3}"),
                    "-i",
                    &path,
                    "-map",
                    "0:v",
                    "-map",
                    "1:a",
                    "-c:v",
                    "copy",
                    "-c:a",
                    "aac",
                    "-shortest",
                    out.to_str().unwrap_or(""),
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;
            Ok(status.success())
        }
        MuxPlan::Mixed(clips) => {
            let mix_clips: Vec<crate::audio_edit::MixClip> = clips
                .iter()
                .map(|(path, start_secs, gain)| crate::audio_edit::MixClip {
                    path: path.clone(),
                    start_secs: *start_secs,
                    gain: *gain,
                    fade_in: 0.0,
                    fade_out: 0.0,
                })
                .collect();
            let mixed = match crate::audio_edit::mix(&mix_clips) {
                Ok(m) => m,
                Err(_) => {
                    // The mix engine reads WAV only: with a non-WAV
                    // clip among the audible ones, fall back to the
                    // earliest audible clip alone (the pre-Wave-10
                    // behaviour) instead of failing the export.
                    let (path, start_secs, _gain) = &clips[0];
                    let status = std::process::Command::new("ffmpeg")
                        .args([
                            "-y",
                            "-i",
                            video.to_str().unwrap_or(""),
                            "-itsoffset",
                            &format!("{start_secs:.3}"),
                            "-i",
                            path,
                            "-map",
                            "0:v",
                            "-map",
                            "1:a",
                            "-c:v",
                            "copy",
                            "-c:a",
                            "aac",
                            "-shortest",
                            out.to_str().unwrap_or(""),
                        ])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()?;
                    return Ok(status.success());
                }
            };
            mux_mixed_wav(video, &mixed, out)
        }
    }
}

/// The part of one clip that falls inside an export window,
/// all in seconds: where to start reading the clip's file,
/// where the segment lands in the exported video, and how
/// much of it plays.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClipWindow {
    file_offset_secs: f32,
    placement_secs: f32,
    len_secs: f32,
}

/// Intersect a clip's timeline span
/// `[clip_start_secs, clip_start_secs + clip_dur_secs)` with an
/// export window `[win_start_secs, win_start_secs + win_len_secs)`.
/// `None` when the two do not overlap (a clip ending exactly at
/// the window's start, or starting exactly at its end, plays
/// nothing). Because both edges of the intersection clamp
/// against the same two spans, exactly one of
/// `file_offset_secs` / `placement_secs` is ever non-zero: a
/// clip cut by the window's start is seeked into, a clip
/// starting inside the window is shifted — never both.
fn clip_window(
    clip_start_secs: f32,
    clip_dur_secs: f32,
    win_start_secs: f32,
    win_len_secs: f32,
) -> Option<ClipWindow> {
    let seg_start = clip_start_secs.max(win_start_secs);
    let seg_end = (clip_start_secs + clip_dur_secs).min(win_start_secs + win_len_secs);
    let len_secs = seg_end - seg_start;
    if len_secs <= 0.0 {
        return None;
    }
    Some(ClipWindow {
        file_offset_secs: seg_start - clip_start_secs,
        placement_secs: seg_start - win_start_secs,
        len_secs,
    })
}

/// The timeline window a range export covers, as
/// `(start_secs, len_secs)`: the offset of the range's first
/// frame within the full timeline, and the exported video's
/// own length (`sample_count` frames at the project fps). The
/// start is counted in the same frame units
/// [`sample_times_range`] samples from — raw per-frame
/// durations under tween playback, duration-rounded
/// repetitions otherwise — so the window names exactly the
/// span of the timeline the range's frames were sampled from,
/// on the same `start_frame / fps` clock the mux paths place
/// clips on.
fn range_window(project: &Project, from: usize, sample_count: usize) -> (f32, f32) {
    let fps = project.fps.max(1.0);
    let start_units: f32 = if project.tween_playback {
        project.frames[..from].iter().map(|f| f.duration).sum()
    } else {
        project.frames[..from]
            .iter()
            .map(|f| (f.duration.round() as usize).max(1) as f32)
            .sum()
    };
    (start_units / fps, sample_count as f32 / fps)
}

/// How long the file at `path` runs, in seconds. The first
/// probe is the engine's own: decoding the file as WAV
/// (`audio_edit::read_wav`, the loader behind
/// `audio_ops::clip_duration_secs`) — when the file decodes
/// as WAV its verdict is final, including `None` for one
/// that decodes to no sample frames. A file the WAV reader
/// cannot decode at all (an MP3, OGG, …) is probed with
/// `ffprobe` instead ([`probe_duration_ffprobe`]). `None`
/// when neither probe measures the file — missing,
/// unreadable, or undecodable by both — because such a
/// clip's end cannot be scheduled inside a sub-range, the
/// same exclusion playback planning applies in
/// `audio::playback_plan`.
fn probe_duration_secs(path: &str) -> Option<f32> {
    match crate::audio_edit::read_wav(path) {
        Ok(data) => {
            if data.sample_rate == 0 || data.frames() == 0 {
                None
            } else {
                Some(data.frames() as f32 / data.sample_rate as f32)
            }
        }
        Err(_) => probe_duration_ffprobe(path),
    }
}

/// Parse the duration (in seconds) out of `ffprobe`'s
/// output for `-show_entries format=duration` with the
/// `noprint_wrappers`/`nokey` default writer: the output is
/// normally a bare number, but a `duration=` prefix on the
/// line is accepted too. The first line carrying a finite,
/// positive value wins; `N/A`, an empty output, zero and
/// negative values are no duration at all (`None`), the
/// same verdict an empty WAV gets in
/// [`probe_duration_secs`]. Pure, so the fixture tests need
/// no `ffprobe` binary.
fn parse_ffprobe_duration(output: &str) -> Option<f32> {
    for line in output.lines() {
        let value = line.trim();
        let value = value.strip_prefix("duration=").unwrap_or(value);
        if let Ok(secs) = value.parse::<f32>() {
            if secs.is_finite() && secs > 0.0 {
                return Some(secs);
            }
        }
    }
    None
}

/// Probe the file at `path` with the system `ffprobe` (the
/// companion tool of the `ffmpeg` every MP4 export already
/// shells out to). `None` when ffprobe is missing, exits
/// non-zero (an undecodable or missing file), or prints no
/// usable duration ([`parse_ffprobe_duration`]).
fn probe_duration_ffprobe(path: &str) -> Option<f32> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ffprobe_duration(&String::from_utf8_lossy(&output.stdout))
}

/// One clip's contribution to a range export: its file, the
/// window segment to take from it ([`ClipWindow`] flattened
/// in), and its gain.
#[derive(Clone, Debug, PartialEq)]
struct RangeSegment {
    path: String,
    file_offset_secs: f32,
    placement_secs: f32,
    len_secs: f32,
    gain: f32,
}

/// What a range export should do with the project's audio,
/// decided purely from the clip list, probed durations and
/// the export window. The range counterpart of [`MuxPlan`].
#[derive(Clone, Debug, PartialEq)]
enum RangeMuxPlan {
    /// No audible clip intersects the window: video only.
    None,
    /// Exactly one segment: mux it directly when its gain is
    /// unity, through a one-clip mix otherwise (see
    /// [`mux_audio_range`]).
    Single(RangeSegment),
    /// Two or more segments: mix the trimmed segments into
    /// one WAV first. Sorted by placement in the window.
    Mixed(Vec<RangeSegment>),
}

/// The full outcome of planning a range export's audio:
/// the mux plan itself, plus the audible clips that had to
/// be left out of it. Kept separate from [`RangeMuxPlan`]
/// so the plain plan stays the small decision
/// [`mux_audio_range`] consumes, while the export entry
/// point can still tell the user the truth about what it
/// could not include (see [`export_mp4_range`]): clips that
/// exist but cannot be measured in `unprobeable` (they fail
/// the export), clips whose files are missing in `missing`
/// (they are skipped, but named).
#[derive(Clone, Debug, PartialEq)]
struct RangePlanOutcome {
    plan: RangeMuxPlan,
    /// Paths of unmuted clips whose length no probe could
    /// measure ([`probe_duration_secs`]) and whose start
    /// falls before the window's end, so they may intersect
    /// the range — as far as the export can tell, their
    /// audio belongs in it.
    unprobeable: Vec<String>,
    /// Paths of unmuted clips whose files are missing and
    /// whose start falls before the window's end, so they
    /// may intersect the range. Unlike `unprobeable` these
    /// never fail the export — there is nothing to schedule
    /// — but they are carried out in the export outcome so
    /// the skip is named instead of silent (Wave P, P15).
    missing: Vec<String>,
}

/// Decide the mux plan for the window
/// `[win_start_secs, win_start_secs + win_len_secs)` of a
/// project, probing clip lengths with `probe`: muted clips
/// are skipped (the exclusion the full mux and the playback
/// plan both apply), each surviving clip's span —
/// `start_frame / fps`, the placement convention of
/// [`mux_plan`] — is intersected with the window by
/// [`clip_window`]. Clips that miss the window contribute
/// nothing; clips no probe can measure contribute no
/// segment either, but are reported in
/// [`RangePlanOutcome::unprobeable`] when they may
/// intersect, so the loss is never silent — and clips
/// whose files are missing are reported in
/// [`RangePlanOutcome::missing`] on the same terms.
fn range_plan_outcome(
    project: &Project,
    win_start_secs: f32,
    win_len_secs: f32,
    probe: &dyn Fn(&str) -> Option<f32>,
) -> RangePlanOutcome {
    let fps = project.fps.max(1.0);
    let win_end_secs = win_start_secs + win_len_secs;
    let mut segments: Vec<RangeSegment> = Vec::new();
    let mut unprobeable: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for clip in project.audio.iter().filter(|clip| !clip.muted) {
        let clip_start_secs = clip.start_frame as f32 / fps;
        match probe(&clip.path) {
            Some(dur) => {
                if let Some(w) = clip_window(
                    clip_start_secs,
                    dur,
                    win_start_secs,
                    win_len_secs,
                ) {
                    segments.push(RangeSegment {
                        path: clip.path.clone(),
                        file_offset_secs: w.file_offset_secs,
                        placement_secs: w.placement_secs,
                        len_secs: w.len_secs,
                        gain: clip.gain,
                    });
                }
            }
            None => {
                if clip_start_secs < win_end_secs {
                    if std::path::Path::new(&clip.path).exists() {
                        unprobeable.push(clip.path.clone());
                    } else {
                        missing.push(clip.path.clone());
                    }
                }
            }
        }
    }
    segments.sort_by(|a, b| {
        a.placement_secs
            .partial_cmp(&b.placement_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let plan = match segments.len() {
        0 => RangeMuxPlan::None,
        1 => RangeMuxPlan::Single(segments.into_iter().next().unwrap()),
        _ => RangeMuxPlan::Mixed(segments),
    };
    RangePlanOutcome {
        plan,
        unprobeable,
        missing,
    }
}

/// Decide the mux plan for the window
/// `[win_start_secs, win_start_secs + win_len_secs)` of a
/// project with the real duration probe
/// ([`probe_duration_secs`]): the plan half of
/// [`range_plan_outcome`], for callers that only need the
/// segments. Production goes through
/// [`range_plan_outcome`] itself (it also needs the
/// unprobeable list); this entry is retained for the plan
/// tests, which pin the plan's shape directly.
#[allow(dead_code)]
fn range_mux_plan(project: &Project, win_start_secs: f32, win_len_secs: f32) -> RangeMuxPlan {
    range_plan_outcome(project, win_start_secs, win_len_secs, &probe_duration_secs).plan
}

/// The ffmpeg argument list muxing one window segment onto
/// the silent range MP4 `video` as AAC (see
/// [`range_direct_args_with_audio_codec`]).
fn range_direct_args(video: &Path, seg: &RangeSegment, out: &Path) -> Vec<String> {
    range_direct_args_with_audio_codec(video, seg, out, MP4_AUDIO_CODEC)
}

/// The ffmpeg argument list muxing one window segment onto
/// the silent range WebM `video` as Opus — the same argument
/// shape as [`range_direct_args`], only the audio codec
/// differs, because WebM cannot carry AAC.
fn range_direct_args_webm(video: &Path, seg: &RangeSegment, out: &Path) -> Vec<String> {
    range_direct_args_with_audio_codec(video, seg, out, WEBM_AUDIO_CODEC)
}

/// The ffmpeg argument list muxing one window segment onto
/// the silent range `video`, in the shape of the
/// full-timeline direct path in [`mux_audio`]: the video is
/// stream-copied, the audio re-encoded with `audio_codec`,
/// `-shortest` guards the tail. The segment's trim is
/// expressed with input options on the audio input — an
/// input `-t` bounds how much of the file is read, and the
/// offset is either an input `-ss` seek (the clip began
/// before the window) or an `-itsoffset` shift (the clip
/// begins inside it); [`clip_window`] guarantees at most
/// one of the two is non-zero, so they are never combined.
fn range_direct_args_with_audio_codec(
    video: &Path,
    seg: &RangeSegment,
    out: &Path,
    audio_codec: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        video.to_str().unwrap_or("").into(),
    ];
    if seg.file_offset_secs > 0.0 {
        args.push("-ss".into());
        args.push(format!("{:.3}", seg.file_offset_secs));
    } else if seg.placement_secs > 0.0 {
        args.push("-itsoffset".into());
        args.push(format!("{:.3}", seg.placement_secs));
    }
    args.push("-t".into());
    args.push(format!("{:.3}", seg.len_secs));
    args.push("-i".into());
    args.push(seg.path.clone());
    args.extend(
        ["-map", "0:v", "-map", "1:a", "-c:v", "copy"]
            .iter()
            .map(|a| a.to_string()),
    );
    args.push("-c:a".into());
    args.push(audio_codec.into());
    args.push("-shortest".into());
    args.push(out.to_str().unwrap_or("").into());
    args
}

/// Run the direct range mux of one segment onto an MP4 (see
/// [`range_direct_args`]). The segment's gain is NOT applied
/// here — ffmpeg's direct path cannot scale a file's volume,
/// which is why [`mux_audio_range`] routes gained clips
/// through the mix and uses this only as the unity-gain path
/// and the mix-failure fallback.
fn run_range_direct(video: &Path, seg: &RangeSegment, out: &Path) -> std::io::Result<bool> {
    run_range_direct_with_audio_codec(video, seg, out, MP4_AUDIO_CODEC)
}

/// Run the direct range mux of one segment, encoding the
/// audio with `audio_codec` (see
/// [`range_direct_args_with_audio_codec`]).
fn run_range_direct_with_audio_codec(
    video: &Path,
    seg: &RangeSegment,
    out: &Path,
    audio_codec: &str,
) -> std::io::Result<bool> {
    let status = std::process::Command::new("ffmpeg")
        .args(range_direct_args_with_audio_codec(video, seg, out, audio_codec))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(status.success())
}

/// Cut `[start_secs, start_secs + len_secs)` out of a decoded
/// WAV, frame-accurately at the data's own sample rate. Both
/// edges clamp to the data, so a segment running past the
/// file's end (or starting past it) yields a shorter — or
/// empty — WAV rather than an error.
fn trim_wav(
    data: &crate::audio_edit::WavData,
    start_secs: f32,
    len_secs: f32,
) -> crate::audio_edit::WavData {
    let rate = data.sample_rate as f32;
    let channels = data.channels.max(1) as usize;
    let total = data.frames();
    let start = ((start_secs.max(0.0) * rate).round() as usize).min(total);
    let end = (((start_secs.max(0.0) + len_secs.max(0.0)) * rate).round() as usize)
        .min(total)
        .max(start);
    crate::audio_edit::WavData {
        sample_rate: data.sample_rate,
        channels: data.channels,
        samples: data.samples[start * channels..end * channels].to_vec(),
    }
}

/// The ffmpeg argument list decoding one segment's window
/// out of its source file into a WAV: an input `-ss`/`-t`
/// pair cuts `[file_offset_secs, file_offset_secs +
/// len_secs)` — the same window [`clip_window`] computed —
/// and the audio is written as 16-bit PCM, which
/// `audio_edit::read_wav` reads back losslessly. Pure, so
/// the argument shape is testable without ffmpeg.
fn decode_window_args(seg: &RangeSegment, out: &Path) -> Vec<String> {
    vec![
        "-y".into(),
        "-ss".into(),
        format!("{:.3}", seg.file_offset_secs),
        "-t".into(),
        format!("{:.3}", seg.len_secs),
        "-i".into(),
        seg.path.clone(),
        "-c:a".into(),
        "pcm_s16le".into(),
        out.to_str().unwrap_or("").into(),
    ]
}

/// Decode a segment's window out of a source the WAV
/// reader cannot handle (MP3, OGG, …) into a temporary
/// WAV with the system `ffmpeg` (see
/// [`decode_window_args`]). The caller owns the returned
/// file and removes it. An `Err` — ffmpeg missing, or
/// exiting non-zero on an undecodable file — fails the
/// caller's mix, which [`mux_audio_range`] answers with
/// its existing direct-mux fallback rather than silence.
fn decode_segment_window(seg: &RangeSegment) -> Result<std::path::PathBuf, String> {
    let tmp = unique_temp("rangedec").with_extension("wav");
    let status = std::process::Command::new("ffmpeg")
        .args(decode_window_args(seg, &tmp))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("could not run ffmpeg to decode {}: {e}", seg.path))?;
    if status.success() {
        Ok(tmp)
    } else {
        let _ = std::fs::remove_file(&tmp);
        Err(format!("ffmpeg could not decode {}", seg.path))
    }
}

/// Render the plan's segments into one mixed WAV: each
/// segment's source file is decoded, trimmed to its window
/// segment ([`trim_wav`]) and written to a temporary WAV,
/// then the temporaries are mixed with `audio_edit::mix` at
/// their placements and gains — the mix engine takes whole
/// files from their beginnings, so the head trim has to
/// happen before it. A source the WAV reader cannot decode
/// (a non-WAV clip) is first cut to exactly its window by
/// [`decode_segment_window`] and enters the mix already
/// trimmed, indistinguishable from a WAV segment — same
/// placement, same gain. The temp files are removed either
/// way.
fn mix_range_segments(segments: &[RangeSegment]) -> Result<crate::audio_edit::WavData, String> {
    let mut temps: Vec<std::path::PathBuf> = Vec::new();
    let result = (|| {
        let mut mix_clips = Vec::with_capacity(segments.len());
        for seg in segments {
            let (data, trim_offset_secs) = match crate::audio_edit::read_wav(&seg.path) {
                Ok(data) => (data, seg.file_offset_secs),
                Err(_) => {
                    let decoded = decode_segment_window(seg)?;
                    temps.push(decoded.clone());
                    let decoded_path = decoded.to_string_lossy().into_owned();
                    (crate::audio_edit::read_wav(&decoded_path)?, 0.0)
                }
            };
            let trimmed = trim_wav(&data, trim_offset_secs, seg.len_secs);
            let tmp = unique_temp("rangemix").with_extension("wav");
            std::fs::write(&tmp, crate::audio_edit::write_wav(&trimmed))
                .map_err(|e| e.to_string())?;
            temps.push(tmp.clone());
            mix_clips.push(crate::audio_edit::MixClip {
                path: tmp.to_string_lossy().into_owned(),
                start_secs: seg.placement_secs,
                gain: seg.gain,
                fade_in: 0.0,
                fade_out: 0.0,
            });
        }
        crate::audio_edit::mix(&mix_clips)
    })();
    for tmp in &temps {
        let _ = std::fs::remove_file(tmp);
    }
    result
}

/// Mux a range plan's audio into an already-encoded silent
/// range MP4 as AAC (see
/// [`mux_audio_range_with_audio_codec`]).
fn mux_audio_range(plan: &RangeMuxPlan, video: &Path, out: &Path) -> std::io::Result<bool> {
    mux_audio_range_with_audio_codec(plan, video, out, MP4_AUDIO_CODEC)
}

/// Mux a range plan's audio into an already-encoded silent
/// range WebM as Opus — the same plan, trimming, mixing and
/// fallback rules as the MP4 range mux; only the audio
/// codec differs, because WebM cannot carry AAC.
fn mux_audio_range_webm(plan: &RangeMuxPlan, video: &Path, out: &Path) -> std::io::Result<bool> {
    mux_audio_range_with_audio_codec(plan, video, out, WEBM_AUDIO_CODEC)
}

/// Mux a range plan's audio into an already-encoded silent
/// range video, encoding the audio with `audio_codec` and
/// following the full-timeline [`mux_audio`] split: exactly
/// one segment at unity gain goes to ffmpeg directly
/// ([`run_range_direct_with_audio_codec`] — ffmpeg reads
/// the segment's file itself, so a non-WAV source needs no
/// preparation there); two or more segments — or a single
/// segment whose gain is not 1.0, which the direct path
/// cannot scale — are trimmed and mixed into one WAV first
/// and muxed with [`mux_mixed_wav_with_audio_codec`] (the
/// mixed WAV starts at the window's zero, each segment's
/// placement baked in; a non-WAV segment is decoded to WAV
/// for the mix, see [`mix_range_segments`]). If the mix
/// cannot be rendered the earliest segment is muxed
/// directly instead, gain unapplied, mirroring the full
/// path's mix-failure fallback. A `None` plan muxes nothing
/// and returns `Ok(false)`.
fn mux_audio_range_with_audio_codec(
    plan: &RangeMuxPlan,
    video: &Path,
    out: &Path,
    audio_codec: &str,
) -> std::io::Result<bool> {
    let segments: &[RangeSegment] = match plan {
        RangeMuxPlan::None => return Ok(false),
        RangeMuxPlan::Single(seg) => std::slice::from_ref(seg),
        RangeMuxPlan::Mixed(segs) => segs,
    };
    if let RangeMuxPlan::Single(seg) = plan {
        if seg.gain == 1.0 {
            return run_range_direct_with_audio_codec(video, seg, out, audio_codec);
        }
    }
    match mix_range_segments(segments) {
        Ok(mixed) => mux_mixed_wav_with_audio_codec(video, &mixed, out, audio_codec),
        Err(_) => run_range_direct_with_audio_codec(video, &segments[0], out, audio_codec),
    }
}

#[cfg(test)]
mod mux_tests {
    use super::{mux_plan, MuxPlan};
    use crate::model::{AudioClip, Project};

    /// Create an empty uniquely-named temp file (the plan decision
    /// only checks existence) and return its path as a String.
    fn temp_audio(tag: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-mux-{tag}-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, b"").unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A demo project at 10 fps carrying exactly the given
    /// `(path, start_frame)` audio clips.
    fn project_with(clips: Vec<(&str, usize)>) -> Project {
        let mut project = Project::new_demo();
        project.fps = 10.0;
        project.audio = clips
            .into_iter()
            .map(|(path, start_frame)| AudioClip {
                path: path.to_string(),
                start_frame,
                gain: 1.0,
                muted: false,
            })
            .collect();
        project
    }

    #[test]
    fn no_clips_is_none_plan() {
        let project = project_with(vec![]);
        assert_eq!(mux_plan(&project), MuxPlan::None);
    }

    #[test]
    fn all_files_missing_is_none_plan() {
        let project = project_with(vec![
            ("/nonexistent/styk-mux-a.wav", 0),
            ("/nonexistent/styk-mux-b.wav", 25),
        ]);
        assert_eq!(mux_plan(&project), MuxPlan::None);
    }

    #[test]
    fn muted_clips_are_excluded_from_the_plan() {
        // Phase M: a muted clip never reaches the mux, exactly
        // as the playback plan excludes it — one muted + one
        // audible clip is a Single plan for the audible one,
        // and an all-muted project is silent.
        let muted_path = temp_audio("muted");
        let audible_path = temp_audio("audible");
        let mut project = project_with(vec![
            (muted_path.as_str(), 0),
            (audible_path.as_str(), 10),
        ]);
        project.audio[0].muted = true;
        assert_eq!(
            mux_plan(&project),
            MuxPlan::Single {
                path: audible_path.clone(),
                start_secs: 1.0,
                gain: 1.0,
            }
        );
        project.audio[1].muted = true;
        assert_eq!(mux_plan(&project), MuxPlan::None);
    }

    #[test]
    fn single_audible_clip_is_flagged_single() {
        let real = temp_audio("single");
        // A missing first clip is skipped, not a silent export.
        let project = project_with(vec![
            ("/nonexistent/styk-mux-gone.wav", 0),
            (real.as_str(), 25),
        ]);
        assert_eq!(
            mux_plan(&project),
            MuxPlan::Single {
                path: real.clone(),
                start_secs: 2.5,
                gain: 1.0,
            }
        );
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn multiple_clips_are_mixed_sorted_by_start() {
        let a = temp_audio("multi-a");
        let b = temp_audio("multi-b");
        let c = temp_audio("multi-c");
        // Given out of order, with one missing file in between.
        let project = project_with(vec![
            (a.as_str(), 25),
            ("/nonexistent/styk-mux-gone.wav", 10),
            (b.as_str(), 0),
            (c.as_str(), 5),
        ]);
        assert_eq!(
            mux_plan(&project),
            MuxPlan::Mixed(vec![
                (b.clone(), 0.0, 1.0),
                (c.clone(), 0.5, 1.0),
                (a.clone(), 2.5, 1.0),
            ])
        );
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
        let _ = std::fs::remove_file(&c);
    }

    /// A demo project at 10 fps carrying exactly the given
    /// `(path, start_frame, gain)` audio clips.
    fn project_with_gains(clips: Vec<(&str, usize, f32)>) -> Project {
        let mut project = Project::new_demo();
        project.fps = 10.0;
        project.audio = clips
            .into_iter()
            .map(|(path, start_frame, gain)| AudioClip {
                path: path.to_string(),
                start_frame,
                gain,
                muted: false,
            })
            .collect();
        project
    }

    #[test]
    fn single_gained_clip_plans_single_with_its_gain() {
        let real = temp_audio("single-gain");
        let project = project_with_gains(vec![(real.as_str(), 25, 0.5)]);
        // The plan stays Single — mux_audio sees the non-unity
        // gain and routes the clip through the mix itself.
        assert_eq!(
            mux_plan(&project),
            MuxPlan::Single {
                path: real.clone(),
                start_secs: 2.5,
                gain: 0.5,
            }
        );
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn mixed_plan_carries_per_clip_gains() {
        let a = temp_audio("gains-a");
        let b = temp_audio("gains-b");
        let project = project_with_gains(vec![(a.as_str(), 25, 2.0), (b.as_str(), 0, 0.5)]);
        assert_eq!(
            mux_plan(&project),
            MuxPlan::Mixed(vec![(b.clone(), 0.0, 0.5), (a.clone(), 2.5, 2.0),])
        );
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }
}

#[cfg(test)]
mod range_mux_tests {
    use super::{
        clip_window, decode_window_args, export_mp4_range, export_webm_range,
        export_webm_range_scaled, parse_ffprobe_duration, probe_duration_secs, range_direct_args,
        range_direct_args_webm, range_mux_plan, range_plan_outcome, range_window, trim_wav,
        ClipWindow, RangeMuxPlan, RangeSegment,
    };
    use crate::model::{AudioClip, Project};
    use std::path::Path;

    /// Write a REAL silent WAV of `secs` seconds at `rate`
    /// Hz (the range plan decodes files to probe them, so an
    /// empty placeholder will not do) and return its path.
    fn temp_wav(tag: &str, secs: f32, rate: u32) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-range-{tag}-{}.wav",
            std::process::id()
        ));
        let data = crate::audio_edit::WavData {
            sample_rate: rate,
            channels: 1,
            samples: vec![0.0; (secs * rate as f32).round() as usize],
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&data)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// Write a REAL WAV of `secs` seconds at `rate` Hz
    /// carrying a 440 Hz tone and return its path. The WebM
    /// range pins (Wave W, W6) call this helper; it was
    /// referenced but never defined, which the Wave W CI
    /// round-1 build caught as E0425 in the lib test target
    /// (the headless workflow, which builds no test targets,
    /// stayed green). A tone rather than silence keeps the
    /// fixture honest as audible content; the mux path the
    /// pins exercise is identical either way.
    fn temp_tone_wav(tag: &str, secs: f32, rate: u32) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-range-tone-{tag}-{}.wav",
            std::process::id()
        ));
        let n = (secs * rate as f32).round() as usize;
        let samples = (0..n)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin() * 0.5
            })
            .collect();
        let data = crate::audio_edit::WavData {
            sample_rate: rate,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&data)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// True when the EBML bytes carry an Opus audio track:
    /// the `A_OPUS` CodecID appears verbatim in the WebM
    /// Tracks element. Companion to `temp_tone_wav` — also
    /// referenced by W6's pins but never defined (E0425,
    /// Wave W CI round 1).
    fn webm_has_opus(bytes: &[u8]) -> bool {
        bytes.windows(6).any(|w| w == b"A_OPUS")
    }

    /// A demo project at 10 fps carrying exactly the given
    /// `(path, start_frame, gain)` audio clips.
    fn project_with(clips: Vec<(&str, usize, f32)>) -> Project {
        let mut project = Project::new_demo();
        project.fps = 10.0;
        project.audio = clips
            .into_iter()
            .map(|(path, start_frame, gain)| AudioClip {
                path: path.to_string(),
                start_frame,
                gain,
                muted: false,
            })
            .collect();
        project
    }

    // ---- clip_window: the intersection maths ----

    #[test]
    fn window_clip_fully_inside_plays_whole_at_its_offset() {
        // Clip [2s, 5s) inside window [1s, 6s): no seek, the
        // segment lands 1s into the export, full 3s of it.
        assert_eq!(
            clip_window(2.0, 3.0, 1.0, 5.0),
            Some(ClipWindow {
                file_offset_secs: 0.0,
                placement_secs: 1.0,
                len_secs: 3.0,
            })
        );
    }

    #[test]
    fn window_clip_cut_by_the_start_is_seeked_not_shifted() {
        // Clip [0s, 4s), window starts at 1.5s: read from
        // 1.5s into the file, play from the export's zero,
        // and only the 2.5s inside the window plays.
        assert_eq!(
            clip_window(0.0, 4.0, 1.5, 5.0),
            Some(ClipWindow {
                file_offset_secs: 1.5,
                placement_secs: 0.0,
                len_secs: 2.5,
            })
        );
    }

    #[test]
    fn window_clip_spanning_the_window_fills_it_exactly() {
        assert_eq!(
            clip_window(0.0, 10.0, 2.0, 3.0),
            Some(ClipWindow {
                file_offset_secs: 2.0,
                placement_secs: 0.0,
                len_secs: 3.0,
            })
        );
    }

    #[test]
    fn window_clip_cut_by_the_end_is_trimmed() {
        // Clip [4s, 9s), window [1s, 6s): starts inside (3s
        // in), tail trimmed to the window's end.
        assert_eq!(
            clip_window(4.0, 5.0, 1.0, 5.0),
            Some(ClipWindow {
                file_offset_secs: 0.0,
                placement_secs: 3.0,
                len_secs: 2.0,
            })
        );
    }

    #[test]
    fn window_touching_clips_play_nothing() {
        // Ends exactly at the window's start.
        assert_eq!(clip_window(0.0, 2.0, 2.0, 3.0), None);
        // Starts exactly at the window's end.
        assert_eq!(clip_window(5.0, 2.0, 2.0, 3.0), None);
        // Entirely before / after the window.
        assert_eq!(clip_window(0.0, 1.0, 2.0, 3.0), None);
        assert_eq!(clip_window(9.0, 1.0, 2.0, 3.0), None);
        // A zero-length window intersects nothing.
        assert_eq!(clip_window(0.0, 4.0, 1.0, 0.0), None);
    }

    // ---- range_window: where the range sits in the timeline ----

    #[test]
    fn range_window_counts_expanded_repetitions_without_tween() {
        let mut project = Project::new_demo();
        project.fps = 10.0;
        project.tween_playback = false;
        project.frames[0].duration = 2.0;
        project.frames[1].duration = 0.4; // rounds to 0, min one rep
        // Frames before `from` occupy 2 + 1 expanded samples.
        let (start, len) = range_window(&project, 2, 25);
        assert_eq!(start, 0.3);
        assert_eq!(len, 2.5);
        // The first frame's window starts at zero.
        assert_eq!(range_window(&project, 0, 25).0, 0.0);
    }

    #[test]
    fn range_window_sums_raw_durations_with_tween() {
        let mut project = Project::new_demo();
        project.fps = 10.0;
        project.tween_playback = true;
        project.frames[0].duration = 2.0;
        project.frames[1].duration = 0.4;
        // Tween sampling walks the raw durations: 2.0 + 0.4
        // frame units = 0.24s at 10 fps.
        let (start, len) = range_window(&project, 2, 25);
        assert!((start - 0.24).abs() < 1e-6, "start {start}");
        assert_eq!(len, 2.5);
    }

    // ---- probe_duration_secs ----

    #[test]
    fn probe_reads_a_real_wavs_duration() {
        let real = temp_wav("probe", 1.5, 8000);
        assert_eq!(probe_duration_secs(&real), Some(1.5));
        let _ = std::fs::remove_file(&real);
        assert_eq!(probe_duration_secs("/nonexistent/styk-range-gone.wav"), None);
    }

    #[test]
    fn probe_rejects_undecodable_and_empty_files() {
        // Bytes that are not audio at all: the WAV reader
        // rejects them, and ffprobe — where it is installed
        // — cannot decode them either, so no probe measures
        // a length. (Where ffprobe is absent the probe falls
        // back to the same `None`.)
        let junk = std::env::temp_dir().join(format!(
            "styk-range-junk-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&junk, b"this is not audio").unwrap();
        assert_eq!(probe_duration_secs(&junk.to_string_lossy()), None);
        let _ = std::fs::remove_file(&junk);
        // A valid header decoding to no samples is no duration.
        let empty = temp_wav("empty", 0.0, 8000);
        assert_eq!(probe_duration_secs(&empty), None);
        let _ = std::fs::remove_file(&empty);
    }

    // ---- parse_ffprobe_duration: the ffprobe output parser ----
    // Fixtures only — no test in this module needs an
    // ffprobe binary; the subprocess probe itself is a thin
    // shell over this parser.

    #[test]
    fn ffprobe_duration_parses_the_bare_number_output() {
        // What `-of default=noprint_wrappers=1:nokey=1`
        // actually prints for a 12.345678s file.
        assert_eq!(parse_ffprobe_duration("12.345678\n"), Some(12.345678));
        assert_eq!(parse_ffprobe_duration("0.500000"), Some(0.5));
        // The keyed form (a writer without `nokey`) is
        // accepted too.
        assert_eq!(parse_ffprobe_duration("duration=3.250000\n"), Some(3.25));
    }

    #[test]
    fn ffprobe_duration_rejects_unusable_output() {
        // ffprobe prints N/A for a stream/container with no
        // duration, and nothing at all on some failures.
        assert_eq!(parse_ffprobe_duration("N/A\n"), None);
        assert_eq!(parse_ffprobe_duration(""), None);
        assert_eq!(parse_ffprobe_duration("not a number"), None);
        // Zero and negative durations schedule nothing —
        // the same verdict an empty WAV gets.
        assert_eq!(parse_ffprobe_duration("0.000000\n"), None);
        assert_eq!(parse_ffprobe_duration("-1.500000\n"), None);
        assert_eq!(parse_ffprobe_duration("duration=N/A\n"), None);
    }

    // ---- range_mux_plan ----

    /// Window [1s, 3s) at the helper project's 10 fps.
    const WIN: (f32, f32) = (1.0, 2.0);

    #[test]
    fn plan_single_clip_starting_inside_the_window() {
        // Starts at frame 20 (2s), runs 3s: placed 1s into
        // the export, trimmed to the window's end at 3s.
        let real = temp_wav("single", 3.0, 8000);
        let project = project_with(vec![(real.as_str(), 20, 1.0)]);
        assert_eq!(
            range_mux_plan(&project, WIN.0, WIN.1),
            RangeMuxPlan::Single(RangeSegment {
                path: real.clone(),
                file_offset_secs: 0.0,
                placement_secs: 1.0,
                len_secs: 1.0,
                gain: 1.0,
            })
        );
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn plan_single_clip_cut_by_the_window_start() {
        // Starts at frame 0, runs 1.5s: only [1s, 1.5s) of
        // the file sounds, from the export's zero.
        let real = temp_wav("cut", 1.5, 8000);
        let project = project_with(vec![(real.as_str(), 0, 1.0)]);
        assert_eq!(
            range_mux_plan(&project, WIN.0, WIN.1),
            RangeMuxPlan::Single(RangeSegment {
                path: real.clone(),
                file_offset_secs: 1.0,
                placement_secs: 0.0,
                len_secs: 0.5,
                gain: 1.0,
            })
        );
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn plan_mixes_intersecting_clips_sorted_by_placement() {
        let a = temp_wav("mix-a", 1.5, 8000); // [0s, 1.5s)
        let b = temp_wav("mix-b", 3.0, 8000); // [2s, 5s)
        // Given in reverse placement order on purpose.
        let project = project_with(vec![(b.as_str(), 20, 0.5), (a.as_str(), 0, 2.0)]);
        assert_eq!(
            range_mux_plan(&project, WIN.0, WIN.1),
            RangeMuxPlan::Mixed(vec![
                RangeSegment {
                    path: a.clone(),
                    file_offset_secs: 1.0,
                    placement_secs: 0.0,
                    len_secs: 0.5,
                    gain: 2.0,
                },
                RangeSegment {
                    path: b.clone(),
                    file_offset_secs: 0.0,
                    placement_secs: 1.0,
                    len_secs: 1.0,
                    gain: 0.5,
                },
            ])
        );
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn plan_is_none_when_nothing_audible_intersects() {
        let before = temp_wav("before", 0.5, 8000); // ends at 0.5s
        let after = temp_wav("after", 1.0, 8000); // starts at frame 50 (5s)
        let project = project_with(vec![(before.as_str(), 0, 1.0), (after.as_str(), 50, 1.0)]);
        assert_eq!(range_mux_plan(&project, WIN.0, WIN.1), RangeMuxPlan::None);
        // A missing file inside the window plans nothing.
        let project = project_with(vec![("/nonexistent/styk-range-gone.wav", 15, 1.0)]);
        assert_eq!(range_mux_plan(&project, WIN.0, WIN.1), RangeMuxPlan::None);
        let _ = std::fs::remove_file(&before);
        let _ = std::fs::remove_file(&after);
    }

    #[test]
    fn plan_excludes_muted_and_unprobeable_clips() {
        let muted = temp_wav("muted", 3.0, 8000);
        let junk_path = std::env::temp_dir().join(format!(
            "styk-range-plan-junk-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&junk_path, b"not audio").unwrap();
        let junk = junk_path.to_string_lossy().into_owned();
        let mut project = project_with(vec![(muted.as_str(), 10, 1.0), (junk.as_str(), 10, 1.0)]);
        project.audio[0].muted = true;
        // The muted clip is excluded by the mute filter, the
        // MP3-named junk file by the duration probe: its
        // bytes are not audio, so neither the WAV reader nor
        // ffprobe can measure a length for it.
        assert_eq!(range_mux_plan(&project, WIN.0, WIN.1), RangeMuxPlan::None);
        let _ = std::fs::remove_file(&muted);
        let _ = std::fs::remove_file(&junk_path);
    }

    // ---- range_plan_outcome: non-WAV clips and the
    // unprobeable report (Wave 10) ----
    // The probe is stubbed with the duration a real ffprobe
    // would report for the fake non-WAV paths, so plan
    // construction is proven with no ffprobe binary (and no
    // ffmpeg) on the test machine.

    #[test]
    fn plan_includes_a_non_wav_clip_at_its_probed_duration() {
        // A "song.mp3" the stub measures at 3s, starting at
        // frame 20 (2s): placed 1s into the [1s, 3s) window
        // and trimmed to its end — the identical segment a
        // 3s WAV plans (see
        // `plan_single_clip_starting_inside_the_window`).
        let project = project_with(vec![("song.mp3", 20, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| Some(3.0));
        assert_eq!(
            outcome.plan,
            RangeMuxPlan::Single(RangeSegment {
                path: "song.mp3".into(),
                file_offset_secs: 0.0,
                placement_secs: 1.0,
                len_secs: 1.0,
                gain: 1.0,
            })
        );
        assert!(outcome.unprobeable.is_empty());
    }

    #[test]
    fn plan_mixes_a_non_wav_segment_with_a_wav_segment() {
        let wav = temp_wav("mixw", 1.5, 8000); // spans [0s, 1.5s)
        let project = project_with(vec![
            ("voice.ogg", 20, 0.5), // stub-probed at 3s: [2s, 5s)
            (wav.as_str(), 0, 1.0),
        ]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|path| {
            if path.ends_with(".ogg") {
                Some(3.0)
            } else {
                Some(1.5)
            }
        });
        assert_eq!(
            outcome.plan,
            RangeMuxPlan::Mixed(vec![
                RangeSegment {
                    path: wav.clone(),
                    file_offset_secs: 1.0,
                    placement_secs: 0.0,
                    len_secs: 0.5,
                    gain: 1.0,
                },
                RangeSegment {
                    path: "voice.ogg".into(),
                    file_offset_secs: 0.0,
                    placement_secs: 1.0,
                    len_secs: 1.0,
                    gain: 0.5,
                },
            ])
        );
        assert!(outcome.unprobeable.is_empty());
        let _ = std::fs::remove_file(&wav);
    }

    #[test]
    fn plan_reports_unprobeable_clips_that_may_intersect() {
        // A real file (so existence holds) no probe can
        // measure, starting before the window's end:
        // reported by path, not silently dropped.
        let junk_path = std::env::temp_dir().join(format!(
            "styk-range-unprobeable-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&junk_path, b"not audio").unwrap();
        let junk = junk_path.to_string_lossy().into_owned();
        let project = project_with(vec![(junk.as_str(), 10, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert_eq!(outcome.plan, RangeMuxPlan::None);
        assert_eq!(outcome.unprobeable, vec![junk.clone()]);
        // A missing file is not unprobeable — there is
        // nothing there to probe — so it stays out of this
        // list; since Wave P (P15) it is reported in
        // `missing` instead of being skipped silently
        // (pinned by
        // `plan_reports_missing_clips_that_may_intersect`).
        let project = project_with(vec![("/nonexistent/styk-range-gone.mp3", 10, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert!(outcome.unprobeable.is_empty());
        assert_eq!(
            outcome.missing,
            vec!["/nonexistent/styk-range-gone.mp3".to_string()]
        );
        // A clip starting after the window's end (frame 50
        // = 5s > 3s) cannot intersect it, probed or not:
        // not reported.
        let project = project_with(vec![(junk.as_str(), 50, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert!(outcome.unprobeable.is_empty());
        // A muted clip never sounds, so its unprobeability
        // costs the export nothing: not reported.
        let mut project = project_with(vec![(junk.as_str(), 10, 1.0)]);
        project.audio[0].muted = true;
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert!(outcome.unprobeable.is_empty());
        let _ = std::fs::remove_file(&junk_path);
    }

    #[test]
    fn plan_reports_missing_clips_that_may_intersect() {
        // Wave P (P15): a missing file whose clip starts
        // before the window's end may have sounded inside
        // the range, so the outcome names it — the plan
        // still skips it (nothing exists to schedule) and
        // the export still succeeds; only the silence is
        // gone.
        let project = project_with(vec![("/nonexistent/styk-range-gone.wav", 10, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert_eq!(outcome.plan, RangeMuxPlan::None);
        assert_eq!(
            outcome.missing,
            vec!["/nonexistent/styk-range-gone.wav".to_string()]
        );
        assert!(outcome.unprobeable.is_empty());
        // A missing clip starting after the window's end
        // (frame 50 = 5s > 3s) could not have sounded in
        // the range: not reported.
        let project = project_with(vec![("/nonexistent/styk-range-gone.wav", 50, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert!(outcome.missing.is_empty());
        // A muted missing clip is intentionally silent:
        // its absence loses nothing, so it is not
        // reported.
        let mut project = project_with(vec![("/nonexistent/styk-range-gone.wav", 10, 1.0)]);
        project.audio[0].muted = true;
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &|_| None);
        assert!(outcome.missing.is_empty());
        // A clip whose file exists and probes fine is
        // planned, not reported.
        let real = temp_wav("present", 1.5, 8000);
        let project = project_with(vec![(real.as_str(), 10, 1.0)]);
        let outcome = range_plan_outcome(&project, WIN.0, WIN.1, &probe_duration_secs);
        assert!(outcome.missing.is_empty());
        let _ = std::fs::remove_file(&real);
    }

    // ---- decode_window_args: the non-WAV mix decode ----

    #[test]
    fn decode_args_cut_the_segment_window_to_pcm_wav() {
        let seg = RangeSegment {
            path: "song.mp3".into(),
            file_offset_secs: 1.25,
            placement_secs: 0.0,
            len_secs: 0.5,
            gain: 2.0,
        };
        let args = decode_window_args(&seg, Path::new("window.wav"));
        let joined = args.join(" ");
        // The cut is an input-option pair before the
        // source: exactly the window the plan computed.
        // Placement and gain are NOT ffmpeg's business here
        // — the mix applies them afterwards, as it does for
        // WAV segments.
        assert!(joined.contains("-ss 1.250"), "{joined}");
        assert!(joined.contains("-t 0.500"), "{joined}");
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        let input = args.iter().position(|a| a == "song.mp3").unwrap();
        assert!(ss < input, "{joined}");
        assert!(joined.contains("-c:a pcm_s16le"), "{joined}");
        assert_eq!(args.last().map(String::as_str), Some("window.wav"));
    }

    // ---- export_mp4_range: unprobeable audio ----

    #[test]
    fn range_export_errors_rather_than_dropping_unprobeable_audio() {
        // A junk "MP3" starting at frame 0 of the range:
        // neither probe can measure it, so the export
        // refuses — before encoding, so no video-only file
        // is written either. The refusal happens ahead of
        // every encode, so the test holds with or without
        // ffprobe/ffmpeg installed.
        let junk_path = std::env::temp_dir().join(format!(
            "styk-range-export-junk-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&junk_path, b"this is not audio").unwrap();
        let junk = junk_path.to_string_lossy().into_owned();
        let project = project_with(vec![(junk.as_str(), 0, 1.0)]);
        let out = std::env::temp_dir().join(format!(
            "styk-range-export-out-{}.mp4",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let err = export_mp4_range(&project, 0, 2, &out).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("could not be probed"), "{msg}");
        assert!(msg.contains("junk"), "{msg}");
        assert!(!out.exists(), "no silent file is written");
        let _ = std::fs::remove_file(&junk_path);
    }

    // ---- export_mp4_range: missing audio (Wave P, P15) ----

    #[test]
    fn range_export_names_missing_audio_instead_of_dropping_it_silently() {
        // The mirror image of the unprobeable refusal
        // above: a clip whose file is MISSING never fails
        // the export — there is nothing to include — but
        // the outcome names it, where the old `()` outcome
        // dropped it without a trace.
        let mut project = project_with(vec![("/nonexistent/styk-range-gone.wav", 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-export-missing-out-{}.mp4",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let outcome = export_mp4_range(&project, 0, 2, &out).unwrap();
        assert_eq!(
            outcome.missing_audio,
            vec!["/nonexistent/styk-range-gone.wav".to_string()]
        );
        let note = outcome.missing_audio_note().expect("a skipped clip is named");
        assert!(note.contains("styk-range-gone.wav"), "{note}");
        assert!(outcome.frames > 0, "the video itself is complete");
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn range_export_with_present_audio_reports_no_missing() {
        // The same export with its clip's file present
        // (a real WAV, muxed in) carries no warning: the
        // outcome reports exactly as it did before the
        // field existed.
        let real = temp_wav("export-present", 1.5, 8000);
        let mut project = project_with(vec![(real.as_str(), 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-export-present-out-{}.mp4",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let outcome = export_mp4_range(&project, 0, 2, &out).unwrap();
        assert!(outcome.missing_audio.is_empty());
        assert_eq!(outcome.missing_audio_note(), None);
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&real);
    }

    // ---- export_webm_range: range audio (Wave W, W6) ----

    #[test]
    fn webm_range_export_muxes_an_intersecting_clip_as_opus() {
        // The WebM counterpart of the MP4 range pins above:
        // a WAV clip intersecting the exported window is cut
        // to the range and muxed in. WebM cannot carry the
        // MP4 path's AAC, so the file must carry an Opus
        // track (its `A_OPUS` CodecID in the EBML header).
        let real = temp_tone_wav("webm-present", 1.5, 8000);
        let mut project = project_with(vec![(real.as_str(), 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-present-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let outcome = export_webm_range(&project, 0, 2, &out).unwrap();
        // Wave X (X12): the range entry returns the shared
        // video-range outcome — with its clip present, the
        // missing-audio report is empty, exactly as the
        // MP4 range outcome reports.
        assert!(outcome.missing_audio.is_empty());
        assert_eq!(outcome.missing_audio_note(), None);
        assert!(outcome.frames > 0, "the video itself is complete");
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "an EBML/WebM file");
        assert!(webm_has_opus(&bytes), "the range audio is muxed as Opus");
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn webm_range_export_mixes_a_gained_clip_like_the_mp4_path() {
        // A lone clip at non-unity gain cannot take the
        // direct path on either format: it is trimmed and
        // mixed first (gain applied), then muxed — here as
        // Opus through the shared mixed-WAV mux.
        let real = temp_tone_wav("webm-gained", 1.5, 8000);
        let mut project = project_with(vec![(real.as_str(), 0, 0.5)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-gained-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        export_webm_range(&project, 0, 2, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(webm_has_opus(&bytes), "the gained mix is muxed as Opus");
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn webm_range_export_is_video_only_when_no_clip_intersects() {
        // Silence where the range has no audio: the only
        // clip starts at frame 40 (4 s in at 10 fps), far
        // past the exported window, so the plan is empty and
        // the export is the plain video-only encode — a
        // valid WebM with no audio track at all.
        let real = temp_tone_wav("webm-outside", 1.0, 8000);
        let mut project = project_with(vec![(real.as_str(), 40, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-silent-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        export_webm_range(&project, 0, 2, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "an EBML/WebM file");
        assert!(!webm_has_opus(&bytes), "no audio track outside the range");
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&real);
    }

    #[test]
    fn webm_range_export_errors_rather_than_dropping_unprobeable_audio() {
        // The MP4 range refusal, mirrored: a clip that
        // exists but no probe can measure fails the export
        // before anything is written, rather than shipping
        // a silently audio-less file.
        let junk_path = std::env::temp_dir().join(format!(
            "styk-range-webm-junk-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&junk_path, b"this is not audio").unwrap();
        let junk = junk_path.to_string_lossy().into_owned();
        let mut project = project_with(vec![(junk.as_str(), 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-junk-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let err = export_webm_range(&project, 0, 2, &out).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("could not be probed"), "{msg}");
        assert!(msg.contains("junk"), "{msg}");
        assert!(!out.exists(), "no silent file is written");
        let _ = std::fs::remove_file(&junk_path);
    }

    #[test]
    fn webm_range_export_names_missing_audio_instead_of_dropping_it_silently() {
        // The WebM mirror of the MP4 range pin of record
        // (Wave X, X12): a clip whose file is MISSING never
        // fails the export — there is nothing to include —
        // but since the outcome widening the skip is named
        // in the returned outcome, where the old `()`
        // return dropped it without a trace. The missing
        // clip contributes no segment, so the file itself
        // is the plain video-only encode (no Opus track).
        let mut project = project_with(vec![("/nonexistent/styk-range-gone.wav", 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-missing-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let outcome = export_webm_range(&project, 0, 2, &out).unwrap();
        assert_eq!(
            outcome.missing_audio,
            vec!["/nonexistent/styk-range-gone.wav".to_string()]
        );
        let note = outcome.missing_audio_note().expect("a skipped clip is named");
        assert!(note.contains("styk-range-gone.wav"), "{note}");
        assert!(outcome.frames > 0, "the video itself is complete");
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], &[0x1A, 0x45, 0xDF, 0xA3], "an EBML/WebM file");
        assert!(!webm_has_opus(&bytes), "a missing clip contributes no segment");
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn webm_range_scaled_export_names_missing_audio_too() {
        // The scaled range entry plans its audio exactly
        // like the unscaled one, so its outcome names a
        // missing clip the same way (at 2.0 the scaled
        // body itself runs — 1.0 would route through the
        // unscaled entry).
        let mut project = project_with(vec![("/nonexistent/styk-range-gone.wav", 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-scaled-missing-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        let outcome = export_webm_range_scaled(&project, 0, 2, &out, 2.0).unwrap();
        assert_eq!(
            outcome.missing_audio,
            vec!["/nonexistent/styk-range-gone.wav".to_string()]
        );
        assert!(outcome.frames > 0, "the video itself is complete");
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn webm_range_scaled_export_carries_the_range_audio_too() {
        // The scaled range entry plans its audio exactly
        // like the unscaled one (audio does not scale): the
        // scaled silent encode is muxed the same way.
        let real = temp_tone_wav("webm-scaled", 1.5, 8000);
        let mut project = project_with(vec![(real.as_str(), 0, 1.0)]);
        project.stage = [96.0, 72.0];
        let out = std::env::temp_dir().join(format!(
            "styk-range-webm-scaled-out-{}.webm",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        export_webm_range_scaled(&project, 0, 2, &out, 2.0).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(webm_has_opus(&bytes), "the scaled range audio is muxed");
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&real);
    }

    // ---- range_direct_args: the ffmpeg invocation ----

    fn segment(file_offset_secs: f32, placement_secs: f32, len_secs: f32) -> RangeSegment {
        RangeSegment {
            path: "clip.wav".into(),
            file_offset_secs,
            placement_secs,
            len_secs,
            gain: 1.0,
        }
    }

    #[test]
    fn direct_args_seek_a_clip_cut_by_the_window_start() {
        let args = range_direct_args(Path::new("silent.mp4"), &segment(1.0, 0.0, 0.5), Path::new("out.mp4"));
        let joined = args.join(" ");
        assert!(joined.contains("-ss 1.000"), "{joined}");
        assert!(!joined.contains("-itsoffset"), "{joined}");
        assert!(joined.contains("-t 0.500"), "{joined}");
        // The seek is an input option: it precedes the
        // audio input it belongs to.
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        let input = args.iter().position(|a| a == "clip.wav").unwrap();
        assert!(ss < input, "{joined}");
        // Same tail as the full-timeline direct path.
        assert!(joined.contains("-map 1:a"), "{joined}");
        assert!(joined.contains("-c:a aac"), "{joined}");
        assert!(joined.contains("-shortest"), "{joined}");
        assert_eq!(args.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn direct_args_shift_a_clip_starting_inside_the_window() {
        let args = range_direct_args(Path::new("silent.mp4"), &segment(0.0, 1.0, 1.0), Path::new("out.mp4"));
        let joined = args.join(" ");
        assert!(joined.contains("-itsoffset 1.000"), "{joined}");
        assert!(!joined.contains("-ss"), "{joined}");
        assert!(joined.contains("-t 1.000"), "{joined}");
    }

    #[test]
    fn direct_args_have_no_offset_when_the_clip_aligns() {
        let args = range_direct_args(Path::new("silent.mp4"), &segment(0.0, 0.0, 2.0), Path::new("out.mp4"));
        let joined = args.join(" ");
        assert!(!joined.contains("-ss"), "{joined}");
        assert!(!joined.contains("-itsoffset"), "{joined}");
        assert!(joined.contains("-t 2.000"), "{joined}");
    }

    #[test]
    fn webm_direct_args_encode_opus_with_the_same_trim_shape() {
        // Wave W (W6): the WebM direct mux is the MP4 one
        // with exactly one substitution — Opus for AAC,
        // because WebM cannot carry AAC. The seek/offset
        // trim shape is shared, pinned here on a clip cut
        // by the window start.
        let args = range_direct_args_webm(
            Path::new("silent.webm"),
            &segment(1.0, 0.0, 0.5),
            Path::new("out.webm"),
        );
        let joined = args.join(" ");
        assert!(joined.contains("-c:a libopus"), "{joined}");
        assert!(!joined.contains("aac"), "{joined}");
        assert!(joined.contains("-ss 1.000"), "{joined}");
        assert!(joined.contains("-t 0.500"), "{joined}");
        assert!(joined.contains("-c:v copy"), "{joined}");
    }

    // ---- trim_wav: the mix path's head/tail cut ----

    /// Stereo WAV at 10 Hz whose sample frames are numbered
    /// (frame f holds f in both channels), 20 frames = 2s.
    fn numbered_wav() -> crate::audio_edit::WavData {
        let mut samples = Vec::new();
        for f in 0..20 {
            samples.push(f as f32);
            samples.push(f as f32);
        }
        crate::audio_edit::WavData {
            sample_rate: 10,
            channels: 2,
            samples,
        }
    }

    #[test]
    fn trim_cuts_whole_frames_from_both_ends() {
        let trimmed = trim_wav(&numbered_wav(), 0.5, 1.0);
        assert_eq!(trimmed.frames(), 10);
        assert_eq!(trimmed.samples[0], 5.0);
        assert_eq!(*trimmed.samples.last().unwrap(), 14.0);
        // Rate and channel count travel with the cut.
        assert_eq!(trimmed.sample_rate, 10);
        assert_eq!(trimmed.channels, 2);
    }

    #[test]
    fn trim_clamps_to_the_data() {
        let trimmed = trim_wav(&numbered_wav(), 0.8, 5.0);
        assert_eq!(trimmed.frames(), 12); // frames 8..20
        assert_eq!(trimmed.samples[0], 8.0);
        // A start past the end cuts everything, not an error.
        assert_eq!(trim_wav(&numbered_wav(), 3.0, 1.0).frames(), 0);
    }
}

/// Write a spritesheet of `indices` to `path` (the shared
/// writer behind the spritesheet exports, as
/// `write_contact_sheet` is behind the contact-sheet
/// exports): tiles are full-size frames on a square-root
/// grid, laid out in the order the indices are given. An
/// empty index list writes a 1x1 blank sheet and returns
/// `Ok(0)`, mirroring the contact sheet's empty-project
/// behaviour (the previous full-only export divided by
/// zero on an empty project instead).
fn write_spritesheet(
    project: &Project,
    indices: &[usize],
    path: &Path,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if indices.is_empty() {
        let sheet = ImageBuffer::from_pixel(1, 1, Rgb([0u8, 0, 0]));
        sheet.save(path).map_err(std::io::Error::other)?;
        return Ok(0);
    }
    let n = indices.len();
    let (w, h) = (project.stage[0] as u32, project.stage[1] as u32);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = (n as u32 + cols - 1) / cols;
    let mut sheet = ImageBuffer::new(w * cols, h * rows);
    for (k, &i) in indices.iter().enumerate() {
        let img = render_frame(project, i);
        let (ox, oy) = ((k as u32 % cols) * w, (k as u32 / cols) * h);
        image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
    }
    sheet.save(path).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Write a spritesheet of `indices` to `path` at `scale` times
/// the stage size (Wave S, S4): the scaled counterpart of
/// [`write_spritesheet`] — the same square-root grid, tile order
/// and empty-list blank sheet, but the tile size is
/// [`scaled_output_size`] at the scale and every tile is a true
/// re-render via [`render_frame_scaled`], never an upscaled copy
/// of the 1.0 tile.
fn write_spritesheet_scaled(
    project: &Project,
    indices: &[usize],
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if indices.is_empty() {
        let sheet = ImageBuffer::from_pixel(1, 1, Rgb([0u8, 0, 0]));
        sheet.save(path).map_err(std::io::Error::other)?;
        return Ok(0);
    }
    let n = indices.len();
    let (w, h) = scaled_output_size(project, scale);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = (n as u32 + cols - 1) / cols;
    let mut sheet = ImageBuffer::new(w * cols, h * rows);
    for (k, &i) in indices.iter().enumerate() {
        let img = render_frame_scaled(project, i, scale)?;
        let (ox, oy) = ((k as u32 % cols) * w, (k as u32 / cols) * h);
        image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
    }
    sheet.save(path).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Export all authored frames as one spritesheet PNG (grid layout).
pub fn export_spritesheet(project: &Project, path: &Path) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        return write_spritesheet(project, &[], path);
    }
    export_sheet_range(project, 0, project.frames.len() - 1, path)
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as one spritesheet PNG: exactly those
/// frames as full-size tiles on a square-root grid sized by
/// the frame count in the range, in timeline order. Unlike
/// the GIF/APNG range exports, a sheet tiles each authored
/// frame exactly once: per-frame durations are not expanded,
/// because a sheet is an overview, not a playback. Returns
/// the number of frames tiled; an empty or inverted range
/// is an error.
pub fn export_sheet_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let indices: Vec<usize> = (from..=to).collect();
    write_spritesheet(project, &indices, path)
}

/// Export all authored frames as one spritesheet PNG (grid
/// layout) at `scale` times the stage size (Wave S, S4) — the
/// export-scale counterpart of [`export_spritesheet`]: tiles are
/// [`scaled_output_size`] at the scale, each a true re-render.
/// At exactly 1.0 this delegates to [`export_spritesheet`], so
/// the output is byte-identical. An invalid scale is refused
/// before anything is written.
pub fn export_sheet_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_spritesheet(project, path);
    }
    if project.frames.is_empty() {
        return write_spritesheet_scaled(project, &[], path, scale);
    }
    export_sheet_range_scaled(project, 0, project.frames.len() - 1, path, scale)
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as one spritesheet PNG at `scale` times the
/// stage size (Wave S, S4) — the scaled counterpart of
/// [`export_sheet_range`], with the same one-tile-per-authored-
/// frame rule and range rules. At exactly 1.0 this delegates to
/// [`export_sheet_range`], so the output is byte-identical. An
/// invalid scale is refused before the range is even clamped,
/// so a refused call writes nothing.
pub fn export_sheet_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_sheet_range(project, from, to, path);
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let indices: Vec<usize> = (from..=to).collect();
    write_spritesheet_scaled(project, &indices, path, scale)
}

/// The alpha counterpart of [`write_spritesheet`] (Wave X,
/// X8): the same square-root grid, tile order and
/// one-tile-per-authored-frame rule, but tiles are
/// [`render_frame_alpha`] renders on a transparent sheet —
/// nothing is flattened onto a background, so per-tile
/// coverage alpha survives into the sheet. An empty index
/// list writes a 1x1 TRANSPARENT sheet and returns `Ok(0)`:
/// transparent is this family's blank, where the RGB
/// writer's is black.
fn write_spritesheet_alpha(
    project: &Project,
    indices: &[usize],
    path: &Path,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if indices.is_empty() {
        let sheet = ImageBuffer::from_pixel(1, 1, image::Rgba([0u8, 0, 0, 0]));
        sheet.save(path).map_err(std::io::Error::other)?;
        return Ok(0);
    }
    let n = indices.len();
    let (w, h) = (project.stage[0] as u32, project.stage[1] as u32);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = (n as u32 + cols - 1) / cols;
    let mut sheet: image::RgbaImage = ImageBuffer::new(w * cols, h * rows);
    for (k, &i) in indices.iter().enumerate() {
        let img = render_frame_alpha(project, i);
        let (ox, oy) = ((k as u32 % cols) * w, (k as u32 / cols) * h);
        image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
    }
    sheet.save(path).map_err(std::io::Error::other)?;
    Ok(n)
}

/// The alpha counterpart of [`write_spritesheet_scaled`]
/// (Wave X, X8): tile size is [`scaled_output_size`] at the
/// scale and every tile is a true re-render via
/// [`render_frame_alpha_scaled`].
fn write_spritesheet_alpha_scaled(
    project: &Project,
    indices: &[usize],
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if indices.is_empty() {
        let sheet = ImageBuffer::from_pixel(1, 1, image::Rgba([0u8, 0, 0, 0]));
        sheet.save(path).map_err(std::io::Error::other)?;
        return Ok(0);
    }
    let n = indices.len();
    let (w, h) = scaled_output_size(project, scale);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = (n as u32 + cols - 1) / cols;
    let mut sheet: image::RgbaImage = ImageBuffer::new(w * cols, h * rows);
    for (k, &i) in indices.iter().enumerate() {
        let img = render_frame_alpha_scaled(project, i, scale)?;
        let (ox, oy) = ((k as u32 % cols) * w, (k as u32 / cols) * h);
        image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
    }
    sheet.save(path).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Export all authored frames as one spritesheet PNG with
/// coverage alpha (Wave X, X8): the transparent-background
/// counterpart of [`export_spritesheet`] — same grid, tiles
/// keep their per-pixel alpha.
pub fn export_spritesheet_alpha(project: &Project, path: &Path) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        return write_spritesheet_alpha(project, &[], path);
    }
    export_sheet_alpha_range(project, 0, project.frames.len() - 1, path)
}

/// Export the authored frames `from..=to` (inclusive,
/// clamped to the timeline) as one alpha spritesheet PNG —
/// the counterpart of [`export_sheet_range`], with the same
/// range rules.
pub fn export_sheet_alpha_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let indices: Vec<usize> = (from..=to).collect();
    write_spritesheet_alpha(project, &indices, path)
}

/// Export all authored frames as one alpha spritesheet PNG
/// at `scale` times the stage size (Wave X, X8) — the
/// scaled counterpart of [`export_spritesheet_alpha`]. At
/// exactly 1.0 this delegates to
/// [`export_spritesheet_alpha`], mirroring the RGB family.
/// An invalid scale is refused before anything is written.
pub fn export_sheet_alpha_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_spritesheet_alpha(project, path);
    }
    if project.frames.is_empty() {
        return write_spritesheet_alpha_scaled(project, &[], path, scale);
    }
    export_sheet_alpha_range_scaled(project, 0, project.frames.len() - 1, path, scale)
}

/// Export the authored frames `from..=to` (inclusive,
/// clamped to the timeline) as one alpha spritesheet PNG at
/// `scale` times the stage size (Wave X, X8) — the scaled
/// counterpart of [`export_sheet_alpha_range`], with the
/// same one-tile-per-authored-frame rule and range rules.
/// At exactly 1.0 this delegates to
/// [`export_sheet_alpha_range`]. An invalid scale is
/// refused before the range is even clamped, so a refused
/// call writes nothing.
pub fn export_sheet_alpha_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_sheet_alpha_range(project, from, to, path);
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let indices: Vec<usize> = (from..=to).collect();
    write_spritesheet_alpha_scaled(project, &indices, path, scale)
}

/// Export every frame as `frame-0001.png`, `frame-0002.png`, ... in `dir`.
pub fn export_png_sequence(project: &Project, dir: &Path) -> std::io::Result<usize> {
    export_png_sequence_with_render_progress(project, dir, None)
}

/// [`export_png_sequence`] with a render-phase progress
/// hook ([`RenderProgressCallback`]): the callback observes
/// every frame the render loop writes (see
/// [`RenderProgressEvent`] for the delivery contract). An
/// empty project creates the directory, fires only the
/// terminal event at `(0, 0)` and returns `Ok(0)`.
pub fn export_png_sequence_with_render_progress(
    project: &Project,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_png_sequence_range_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as `frame-XXXX.png` files in `dir`, keeping each
/// frame's authored index in its file name (frame 2 exports as
/// `frame-0003.png`, as in a full sequence export). Returns the
/// number of files written; an empty or inverted range is an
/// error.
pub fn export_png_sequence_range(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
) -> std::io::Result<usize> {
    export_png_sequence_range_with_render_progress(project, from, to, dir, None)
}

/// [`export_png_sequence_range`] with a render-phase
/// progress hook ([`RenderProgressCallback`]).
pub fn export_png_sequence_range_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    let total = to - from + 1;
    for (written, i) in (from..=to).enumerate() {
        let img = render_frame(project, i);
        img.save(dir.join(format!("frame-{:04}.png", i + 1)))
            .map_err(std::io::Error::other)?;
        let done = written + 1;
        if done < total {
            report_render_progress(
                progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    Ok(total)
}

/// Export every frame as `frame-0001.png`, `frame-0002.png`,
/// ... in `dir`, at `scale` times the stage size (Wave R,
/// R10) — the export-scale counterpart of
/// [`export_png_sequence`]. Every frame is a true re-render
/// at the scaled size via [`render_frame_scaled`], never an
/// upscaled copy. An invalid scale is refused before
/// anything is written.
pub fn export_png_sequence_scaled(
    project: &Project,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_png_sequence_scaled_with_render_progress(project, dir, scale, None)
}

/// [`export_png_sequence_scaled`] with a render-phase
/// progress hook ([`RenderProgressCallback`]). The scale
/// is validated before anything is written or reported,
/// exactly as in the plain entry; an empty project
/// creates the directory, fires only the terminal event
/// at `(0, 0)` and returns `Ok(0)`.
pub fn export_png_sequence_scaled_with_render_progress(
    project: &Project,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_png_sequence_range_scaled_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        scale,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as `frame-XXXX.png` files in `dir` at
/// `scale` times the stage size (Wave R, R10) — the scaled
/// counterpart of [`export_png_sequence_range`], with the
/// same file naming (each frame keeps its authored index)
/// and the same range rules. An invalid scale is refused
/// before the range is even clamped, so a refused call
/// writes nothing and creates no directory.
pub fn export_png_sequence_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_png_sequence_range_scaled_with_render_progress(project, from, to, dir, scale, None)
}

/// [`export_png_sequence_range_scaled`] with a
/// render-phase progress hook
/// ([`RenderProgressCallback`]).
pub fn export_png_sequence_range_scaled_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    let total = to - from + 1;
    for (written, i) in (from..=to).enumerate() {
        let img = render_frame_scaled(project, i, scale)?;
        img.save(dir.join(format!("frame-{:04}.png", i + 1)))
            .map_err(std::io::Error::other)?;
        let done = written + 1;
        if done < total {
            report_render_progress(
                progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    Ok(total)
}

/// Export every frame as `frame-0001.png`, `frame-0002.png`,
/// ... in `dir`, with coverage alpha (Wave X, X8 —
/// SpriteLoop GAP P-5): the transparent-background
/// counterpart of [`export_png_sequence`]. Every frame is a
/// [`render_frame_alpha`] render — transparent where
/// nothing covers the pixel, partial alpha on antialiased
/// edges — written as RGBA PNG. Naming, range and progress
/// rules are the PNG family's own.
pub fn export_png_alpha_sequence(project: &Project, dir: &Path) -> std::io::Result<usize> {
    export_png_alpha_sequence_with_render_progress(project, dir, None)
}

/// [`export_png_alpha_sequence`] with a render-phase
/// progress hook ([`RenderProgressCallback`]): the same
/// delivery contract as [`export_png_sequence_with_render_progress`]
/// — an empty project creates the directory, fires only
/// the terminal event at `(0, 0)` and returns `Ok(0)`.
pub fn export_png_alpha_sequence_with_render_progress(
    project: &Project,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_png_alpha_sequence_range_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive,
/// clamped to the timeline) as RGBA `frame-XXXX.png` files
/// in `dir`, keeping each frame's authored index in its
/// file name — the alpha counterpart of
/// [`export_png_sequence_range`], with the same range
/// rules.
pub fn export_png_alpha_sequence_range(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
) -> std::io::Result<usize> {
    export_png_alpha_sequence_range_with_render_progress(project, from, to, dir, None)
}

/// [`export_png_alpha_sequence_range`] with a render-phase
/// progress hook ([`RenderProgressCallback`]).
pub fn export_png_alpha_sequence_range_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    export_png_alpha_sequence_range_scaled_with_render_progress(
        project, from, to, dir, 1.0, progress,
    )
}

/// Export every frame as RGBA `frame-0001.png`, ... in
/// `dir` at `scale` times the stage size (Wave X, X8): the
/// scaled counterpart of [`export_png_alpha_sequence`],
/// riding the export-scale entries exactly as the RGB
/// family does — every frame a true re-render at the
/// scaled size via [`render_frame_alpha_scaled`], never an
/// upscaled copy. An invalid scale is refused before
/// anything is written.
pub fn export_png_alpha_sequence_scaled(
    project: &Project,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_png_alpha_sequence_scaled_with_render_progress(project, dir, scale, None)
}

/// [`export_png_alpha_sequence_scaled`] with a
/// render-phase progress hook ([`RenderProgressCallback`]).
/// The scale is validated before anything is written or
/// reported, exactly as in the plain entry; an empty
/// project creates the directory, fires only the terminal
/// event at `(0, 0)` and returns `Ok(0)`.
pub fn export_png_alpha_sequence_scaled_with_render_progress(
    project: &Project,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_png_alpha_sequence_range_scaled_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        scale,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive,
/// clamped to the timeline) as RGBA `frame-XXXX.png` files
/// in `dir` at `scale` times the stage size (Wave X, X8) —
/// the scaled counterpart of
/// [`export_png_alpha_sequence_range`], with the same file
/// naming and range rules. An invalid scale is refused
/// before the range is even clamped, so a refused call
/// writes nothing and creates no directory.
pub fn export_png_alpha_sequence_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_png_alpha_sequence_range_scaled_with_render_progress(
        project, from, to, dir, scale, None,
    )
}

/// [`export_png_alpha_sequence_range_scaled`] with a
/// render-phase progress hook
/// ([`RenderProgressCallback`]). The family's one loop
/// body: the unscaled entries delegate here at scale 1.0,
/// which always validates, so their observable refusal
/// order (clamp, then create) is preserved.
pub fn export_png_alpha_sequence_range_scaled_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    let total = to - from + 1;
    for (written, i) in (from..=to).enumerate() {
        let img = render_frame_alpha_scaled(project, i, scale)?;
        img.save(dir.join(format!("frame-{:04}.png", i + 1)))
            .map_err(std::io::Error::other)?;
        let done = written + 1;
        if done < total {
            report_render_progress(
                progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    Ok(total)
}

/// Export every authored frame as `frame-0001.jpg`, `frame-0002.jpg`,
/// ... in `dir` (JPEG, quality 90).
pub fn export_jpeg_sequence(project: &Project, dir: &Path) -> std::io::Result<usize> {
    export_jpeg_sequence_with_render_progress(project, dir, None)
}

/// [`export_jpeg_sequence`] with a render-phase progress
/// hook ([`RenderProgressCallback`]): the callback
/// observes every frame the render loop writes (see
/// [`RenderProgressEvent`] for the delivery contract). An
/// empty project creates the directory, fires only the
/// terminal event at `(0, 0)` and returns `Ok(0)`.
pub fn export_jpeg_sequence_with_render_progress(
    project: &Project,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_jpeg_sequence_range_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as `frame-XXXX.jpg` files in `dir`, keeping each
/// frame's authored index in its file name (frame 2 exports as
/// `frame-0003.jpg`, as in a full sequence export). Returns the
/// number of files written; an empty or inverted range is an
/// error.
pub fn export_jpeg_sequence_range(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
) -> std::io::Result<usize> {
    export_jpeg_sequence_range_with_render_progress(project, from, to, dir, None)
}

/// [`export_jpeg_sequence_range`] with a render-phase
/// progress hook ([`RenderProgressCallback`]).
pub fn export_jpeg_sequence_range_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    let total = to - from + 1;
    for (written, i) in (from..=to).enumerate() {
        let img = render_frame(project, i);
        let path = dir.join(format!("frame-{:04}.jpg", i + 1));
        let mut file = std::fs::File::create(&path)?;
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90);
        encoder.encode_image(&img).map_err(std::io::Error::other)?;
        let done = written + 1;
        if done < total {
            report_render_progress(
                progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    Ok(total)
}

/// Export every authored frame as `frame-0001.jpg`, `frame-0002.jpg`,
/// ... in `dir`, at `scale` times the stage size (Wave T, T7) — the
/// export-scale counterpart of [`export_jpeg_sequence`]. Every frame
/// is a true re-render at the scaled size via
/// [`render_frame_scaled`], never an upscaled copy, encoded as JPEG
/// at quality 90 exactly as the unscaled entry encodes. At exactly
/// 1.0 this delegates to [`export_jpeg_sequence`], so the output is
/// byte-identical. An invalid scale is refused before anything is
/// written.
pub fn export_jpeg_sequence_scaled(
    project: &Project,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_jpeg_sequence_scaled_with_render_progress(project, dir, scale, None)
}

/// [`export_jpeg_sequence_scaled`] with a render-phase
/// progress hook ([`RenderProgressCallback`]). At exactly
/// 1.0 the call routes through
/// [`export_jpeg_sequence_with_render_progress`], hook
/// included; an empty project at any other scale creates
/// the directory, fires only the terminal event at
/// `(0, 0)` and returns `Ok(0)`.
pub fn export_jpeg_sequence_scaled_with_render_progress(
    project: &Project,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_jpeg_sequence_with_render_progress(project, dir, progress);
    }
    if project.frames.is_empty() {
        std::fs::create_dir_all(dir)?;
        report_render_progress(
            progress,
            RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            },
        );
        return Ok(0);
    }
    export_jpeg_sequence_range_scaled_with_render_progress(
        project,
        0,
        project.frames.len() - 1,
        dir,
        scale,
        progress,
    )
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as `frame-XXXX.jpg` files in `dir` at `scale`
/// times the stage size (Wave T, T7) — the scaled counterpart of
/// [`export_jpeg_sequence_range`], with the same file naming (each
/// frame keeps its authored index), the same range rules and the
/// same JPEG encoding (quality 90). At exactly 1.0 this delegates
/// to [`export_jpeg_sequence_range`], so the output is
/// byte-identical. An invalid scale is refused before the range is
/// even clamped, so a refused call writes nothing and creates no
/// directory.
pub fn export_jpeg_sequence_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    export_jpeg_sequence_range_scaled_with_render_progress(project, from, to, dir, scale, None)
}

/// [`export_jpeg_sequence_range_scaled`] with a
/// render-phase progress hook
/// ([`RenderProgressCallback`]). At exactly 1.0 the call
/// routes through
/// [`export_jpeg_sequence_range_with_render_progress`],
/// hook included.
pub fn export_jpeg_sequence_range_scaled_with_render_progress(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
    scale: f32,
    progress: RenderProgressCallback<'_>,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_jpeg_sequence_range_with_render_progress(
            project, from, to, dir, progress,
        );
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    let total = to - from + 1;
    for (written, i) in (from..=to).enumerate() {
        let img = render_frame_scaled(project, i, scale)?;
        let path = dir.join(format!("frame-{:04}.jpg", i + 1));
        let mut file = std::fs::File::create(&path)?;
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90);
        encoder.encode_image(&img).map_err(std::io::Error::other)?;
        let done = written + 1;
        if done < total {
            report_render_progress(
                progress,
                RenderProgressEvent {
                    rendered_frames: done,
                    total_frames: total,
                    finished: false,
                },
            );
        }
    }
    report_render_progress(
        progress,
        RenderProgressEvent {
            rendered_frames: total,
            total_frames: total,
            finished: true,
        },
    );
    Ok(total)
}

/// Encode the given frame times as an APNG (animated PNG) at the
/// project fps, looping forever (the shared encoder behind the
/// APNG exports). Uses the `png` crate's native APNG writer
/// (`set_animated`): the first frame is stored as IDAT and the rest
/// as fdAT. An empty `times` creates the parent directory but
/// writes no file, returning `Ok(0)`.
fn encode_apng(project: &Project, times: &[f32], path: &Path) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if times.is_empty() {
        return Ok(0);
    }
    let first = image::DynamicImage::ImageRgb8(render_frame_at(project, times[0])).to_rgba8();
    let (w, h) = (first.width(), first.height());
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(times.len() as u32, 0) // 0 plays = loop forever
        .map_err(std::io::Error::other)?;
    // Frame delay 1/fps, expressed exactly as 100 / (fps * 100).
    let den = (project.fps.max(1.0) * 100.0).round().clamp(1.0, u16::MAX as f32) as u16;
    encoder
        .set_frame_delay(100, den)
        .map_err(std::io::Error::other)?;
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(first.as_raw())
        .map_err(std::io::Error::other)?;
    for t in &times[1..] {
        let img = image::DynamicImage::ImageRgb8(render_frame_at(project, *t)).to_rgba8();
        writer
            .write_image_data(img.as_raw())
            .map_err(std::io::Error::other)?;
    }
    writer.finish().map_err(std::io::Error::other)?;
    Ok(times.len())
}

/// Encode the given frame times as an APNG at the project fps,
/// looping forever, at `scale` times the stage size (Wave S,
/// S4): the scaled counterpart of [`encode_apng`], rendering
/// every sample through [`render_frame_at_scaled`] — a true
/// re-render at the scaled size, never a resample of the 1.0
/// raster. Same empty-`times` contract: the parent directory is
/// created but no file is written, returning `Ok(0)`.
fn encode_apng_scaled(
    project: &Project,
    times: &[f32],
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if times.is_empty() {
        return Ok(0);
    }
    let first =
        image::DynamicImage::ImageRgb8(render_frame_at_scaled(project, times[0], scale)?).to_rgba8();
    let (w, h) = (first.width(), first.height());
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(times.len() as u32, 0) // 0 plays = loop forever
        .map_err(std::io::Error::other)?;
    // Frame delay 1/fps, expressed exactly as 100 / (fps * 100).
    let den = (project.fps.max(1.0) * 100.0).round().clamp(1.0, u16::MAX as f32) as u16;
    encoder
        .set_frame_delay(100, den)
        .map_err(std::io::Error::other)?;
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(first.as_raw())
        .map_err(std::io::Error::other)?;
    for t in &times[1..] {
        let img =
            image::DynamicImage::ImageRgb8(render_frame_at_scaled(project, *t, scale)?).to_rgba8();
        writer
            .write_image_data(img.as_raw())
            .map_err(std::io::Error::other)?;
    }
    writer.finish().map_err(std::io::Error::other)?;
    Ok(times.len())
}

/// Export the animation as an APNG (animated PNG) at the project
/// fps, looping forever. Frames are the same full-canvas samples
/// the GIF/MP4 exports use, converted to RGBA.
pub fn export_apng(project: &Project, path: &Path) -> std::io::Result<usize> {
    let times = sample_times(project);
    encode_apng(project, &times, path)
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as an APNG (animated PNG) at the project fps,
/// looping forever. Frames keep their own durations
/// (duration-expanded or tween-sampled, exactly as in a full
/// export). Returns the number of APNG frames written; an empty
/// or inverted range is an error.
pub fn export_apng_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_apng(project, &times, path)
}

/// Export the animation as an APNG (animated PNG) at the project
/// fps, looping forever, at `scale` times the stage size (Wave S,
/// S4) — the export-scale counterpart of [`export_apng`]. Every
/// frame is a true re-render at the scaled size via
/// [`render_frame_at_scaled`]. At exactly 1.0 this delegates to
/// [`export_apng`], so the output is byte-identical. An invalid
/// scale is refused before anything is written.
pub fn export_apng_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_apng(project, path);
    }
    let times = sample_times(project);
    encode_apng_scaled(project, &times, path, scale)
}

/// Export the authored frames `from..=to` (inclusive, clamped to
/// the timeline) as an APNG (animated PNG) at the project fps,
/// looping forever, at `scale` times the stage size (Wave S,
/// S4) — the scaled counterpart of [`export_apng_range`], with
/// the same frame sampling and range rules. At exactly 1.0 this
/// delegates to [`export_apng_range`], so the output is
/// byte-identical. An invalid scale is refused before the range
/// is even clamped, so a refused call writes nothing.
pub fn export_apng_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_apng_range(project, from, to, path);
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_apng_scaled(project, &times, path, scale)
}

/// Encode the given frame times as an alpha APNG (animated
/// PNG) at the project fps, looping forever (Wave Y, Y13):
/// the transparent-background counterpart of [`encode_apng`],
/// riding X8's alpha funnel — every frame is a
/// [`render_frame_at_alpha`] render whose RGBA pixels (no
/// background fill; coverage alpha on antialiased edges) feed
/// the same `png`-crate APNG writer directly. The opaque
/// encoder above is untouched, so its output stays
/// byte-identical. Same empty-`times` contract: the parent
/// directory is created but no file is written, returning
/// `Ok(0)`.
fn encode_apng_alpha(project: &Project, times: &[f32], path: &Path) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if times.is_empty() {
        return Ok(0);
    }
    let first = render_frame_at_alpha(project, times[0]);
    let (w, h) = (first.width(), first.height());
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(times.len() as u32, 0) // 0 plays = loop forever
        .map_err(std::io::Error::other)?;
    // Frame delay 1/fps, expressed exactly as 100 / (fps * 100).
    let den = (project.fps.max(1.0) * 100.0).round().clamp(1.0, u16::MAX as f32) as u16;
    encoder
        .set_frame_delay(100, den)
        .map_err(std::io::Error::other)?;
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(first.as_raw())
        .map_err(std::io::Error::other)?;
    for t in &times[1..] {
        let img = render_frame_at_alpha(project, *t);
        writer
            .write_image_data(img.as_raw())
            .map_err(std::io::Error::other)?;
    }
    writer.finish().map_err(std::io::Error::other)?;
    Ok(times.len())
}

/// Encode the given frame times as an alpha APNG at the
/// project fps, looping forever, at `scale` times the stage
/// size (Wave Y, Y13): the scaled counterpart of
/// [`encode_apng_alpha`], rendering every sample through
/// [`render_frame_at_alpha_scaled`] — a true re-render at the
/// scaled size, never a resample of the 1.0 raster. Same
/// empty-`times` contract as [`encode_apng_alpha`].
fn encode_apng_alpha_scaled(
    project: &Project,
    times: &[f32],
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if times.is_empty() {
        return Ok(0);
    }
    let first = render_frame_at_alpha_scaled(project, times[0], scale)?;
    let (w, h) = (first.width(), first.height());
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(times.len() as u32, 0) // 0 plays = loop forever
        .map_err(std::io::Error::other)?;
    // Frame delay 1/fps, expressed exactly as 100 / (fps * 100).
    let den = (project.fps.max(1.0) * 100.0).round().clamp(1.0, u16::MAX as f32) as u16;
    encoder
        .set_frame_delay(100, den)
        .map_err(std::io::Error::other)?;
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(first.as_raw())
        .map_err(std::io::Error::other)?;
    for t in &times[1..] {
        let img = render_frame_at_alpha_scaled(project, *t, scale)?;
        writer
            .write_image_data(img.as_raw())
            .map_err(std::io::Error::other)?;
    }
    writer.finish().map_err(std::io::Error::other)?;
    Ok(times.len())
}

/// Export the animation as an alpha APNG (animated PNG) at
/// the project fps, looping forever (Wave Y, Y13) — the
/// transparent-background counterpart of [`export_apng`]:
/// frames are transparent where nothing covers the pixel,
/// with per-pixel coverage alpha instead of the project
/// background colour.
pub fn export_apng_alpha(project: &Project, path: &Path) -> std::io::Result<usize> {
    let times = sample_times(project);
    encode_apng_alpha(project, &times, path)
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as an alpha APNG at the project fps,
/// looping forever (Wave Y, Y13) — the alpha counterpart of
/// [`export_apng_range`], with the same range rules: returns
/// the number of APNG frames written; an empty or inverted
/// range is an error.
pub fn export_apng_alpha_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_apng_alpha(project, &times, path)
}

/// Export the animation as an alpha APNG at the project fps,
/// looping forever, at `scale` times the stage size (Wave Y,
/// Y13) — the scaled counterpart of [`export_apng_alpha`].
/// Every frame is a true re-render at the scaled size via
/// [`render_frame_at_alpha_scaled`]. At exactly 1.0 this
/// delegates to [`export_apng_alpha`], so the output is
/// byte-identical within the alpha family. An invalid scale
/// is refused before anything is written.
pub fn export_apng_alpha_scaled(
    project: &Project,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_apng_alpha(project, path);
    }
    let times = sample_times(project);
    encode_apng_alpha_scaled(project, &times, path, scale)
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as an alpha APNG at the project fps,
/// looping forever, at `scale` times the stage size (Wave Y,
/// Y13) — the scaled counterpart of [`export_apng_alpha_range`],
/// with the same frame sampling and range rules. At exactly
/// 1.0 this delegates to [`export_apng_alpha_range`], so the
/// output is byte-identical within the alpha family. An
/// invalid scale is refused before the range is even clamped,
/// so a refused call writes nothing.
pub fn export_apng_alpha_range_scaled(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
    scale: f32,
) -> std::io::Result<usize> {
    let scale = check_export_scale(scale)?;
    if scale == 1.0 {
        return export_apng_alpha_range(project, from, to, path);
    }
    let (from, to) = clamp_frame_range(project, from, to)?;
    let times = sample_times_range(project, from, to);
    encode_apng_alpha_scaled(project, &times, path, scale)
}

/// Columns and thumbnail width used by the contact-sheet file
/// exports: the same defaults the MCP `render_contact_sheet`
/// tool uses (4 columns, 240 px thumbnails).
const CONTACT_SHEET_COLS: usize = 4;
const CONTACT_SHEET_THUMB_W: u32 = 240;

/// Write a contact sheet of `indices` to `path` (the shared
/// writer behind the contact-sheet exports, as `encode_gif` is
/// behind the GIF exports). The image format comes from the
/// path's extension, as with the spritesheet export. Returns the
/// number of frames tiled.
fn write_contact_sheet(
    project: &Project,
    indices: &[usize],
    path: &Path,
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    render_contact_sheet(project, indices, CONTACT_SHEET_COLS, CONTACT_SHEET_THUMB_W)
        .save(path)
        .map_err(std::io::Error::other)?;
    Ok(indices.len())
}

/// Export a contact sheet of the whole timeline to `path`: one
/// tile per authored frame, in order. Returns the number of
/// frames tiled. An empty project still writes the renderer's
/// 1x1 background sheet and returns `Ok(0)`, mirroring the GIF
/// export's empty-project behaviour.
pub fn export_contact_sheet(project: &Project, path: &Path) -> std::io::Result<usize> {
    if project.frames.is_empty() {
        return write_contact_sheet(project, &[], path);
    }
    export_contact_sheet_range(project, 0, project.frames.len() - 1, path)
}

/// Export a contact sheet of the authored frames `from..=to`
/// (inclusive, clamped to the timeline) to `path`, written
/// exactly at `path` with no name suffix. Unlike the GIF/APNG
/// range exports, a sheet tiles each authored frame exactly
/// once: per-frame durations are not expanded, because a sheet
/// is an overview, not a playback. Returns the number of frames
/// tiled; an empty or inverted range is an error.
pub fn export_contact_sheet_range(
    project: &Project,
    from: usize,
    to: usize,
    path: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    let indices: Vec<usize> = (from..=to).collect();
    write_contact_sheet(project, &indices, path)
}

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as one SVG file per frame in `dir`,
/// keeping each frame's authored index in its file name
/// (frame 2 exports as `frame-0003.svg`, as in a full SVG
/// export). Returns the number of files written; an empty
/// or inverted range is an error.
///
/// The per-frame SVG writer lives in `crate::svg` (Phase M
/// extracted it from the whole-timeline export), so this is a
/// direct delegation: only the range's frames are rendered,
/// under their authored file names.
pub fn export_svg_range(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
) -> std::io::Result<usize> {
    crate::svg::export_svg_range(project, from, to, dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    #[test]
    fn write_sample_frame_for_visual_check() {
        let project = Project::new_demo();
        let img = render_frame(&project, 2);
        img.save("/tmp/styk-export.png").unwrap();
    }

    #[test]
    fn gif_export_writes_a_real_gif() {
        let project = Project::new_demo();
        let path = std::path::Path::new("/tmp/styk-test.gif");
        let n = export_gif(&project, path).unwrap();
        assert_eq!(n, project.frames.len());
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.starts_with(b"GIF8"), "not a GIF");
        assert!(bytes.len() > 2000, "suspiciously small: {}", bytes.len());
    }

    #[test]
    fn camera_zoom_changes_the_render() {
        let mut project = Project::new_demo();
        let plain = render_frame(&project, 2);
        project.set_camera_key(2, 508.0, 400.0, 2.5);
        let zoomed = render_frame(&project, 2);
        let bg = rgb(project.background);
        let count = |img: &ImageBuffer<Rgb<u8>, Vec<u8>>| {
            img.pixels().filter(|p| p.0 != bg).count()
        };
        assert!(count(&zoomed) > count(&plain), "zoomed-in figure must cover more pixels");
    }

    #[test]
    fn camera_rotate_offset_matches_pivot_convention() {
        // Positive angle = the scene rotates counter-clockwise on
        // screen (mirrors Pivot's camera angle; see the helper doc).
        let close = |a: [f32; 2], b: [f32; 2]| {
            (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
        };
        assert!(close(camera_rotate_offset(50.0, 0.0, 90.0), [0.0, -50.0]), "right -> top at +90");
        assert!(close(camera_rotate_offset(0.0, 50.0, 90.0), [50.0, 0.0]), "bottom -> right at +90");
        assert!(close(camera_rotate_offset(50.0, 0.0, -90.0), [0.0, 50.0]), "right -> bottom at -90");
        assert!(close(camera_rotate_offset(10.0, 0.0, 360.0), [10.0, 0.0]), "full turn is identity");
        // Zero angle is exact, not approximate.
        assert_eq!(camera_rotate_offset(30.0, 40.0, 0.0), [30.0, 40.0]);
    }

    /// A white bar to the right of the stage centre on a black
    /// 200x200 stage, one frame, no figures — a precise marker for
    /// where the camera transform puts things.
    fn rotation_marker_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [200.0, 200.0];
        project.background = 0x000000;
        project.figures.clear();
        project.frames = vec![crate::model::Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.strokes = vec![crate::model::Stroke {
            frame: 0,
            points: vec![[150.0, 100.0], [180.0, 100.0]],
            color: 0xFFFFFF,
            width: 12.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        }];
        project.camera.clear();
        project
    }

    #[test]
    fn camera_rotation_moves_the_marker_counter_clockwise() {
        let mut project = rotation_marker_project();
        project.set_camera_key(0, 100.0, 100.0, 1.0);
        let plain = render_frame_at(&project, 0.0);
        let white = Rgb([255u8, 255, 255]);
        let black = Rgb([0u8, 0, 0]);
        // Unrotated: the bar sits right of centre.
        assert_eq!(plain.get_pixel(165, 100), &white);
        assert_eq!(plain.get_pixel(100, 35), &black);
        // +90 degrees: the bar must sit above centre (scene CCW).
        project.camera[0].angle = 90.0;
        let rotated = render_frame_at(&project, 0.0);
        assert_eq!(rotated.get_pixel(100, 35), &white, "+90 must move the bar to the top");
        assert_eq!(rotated.get_pixel(165, 100), &black, "+90 must vacate the right side");
    }

    /// One horizontal red stroke at the given opacity on a
    /// mid-grey 200x200 stage, no figures, camera cleared —
    /// stage units map 1:1 onto output pixels, and the
    /// stroke's centre pixel (100, 100) is covered at every
    /// supersample sub-pixel, so it shows the stroke colour
    /// composited over the background at exactly the stroke's
    /// opacity (no partial-coverage averaging).
    fn opacity_marker_project(opacity: f32) -> Project {
        let mut project = Project::new_demo();
        project.stage = [200.0, 200.0];
        project.background = 0x646464;
        project.figures.clear();
        project.frames = vec![crate::model::Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.strokes = vec![crate::model::Stroke {
            frame: 0,
            points: vec![[50.0, 100.0], [150.0, 100.0]],
            color: 0xFF0000,
            width: 12.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity,
            scatter: 0.0,
        }];
        project.camera.clear();
        project
    }

    #[test]
    fn stroke_opacity_blends_over_the_background() {
        let bg = Rgb([100u8, 100, 100]);
        // Opacity 1.0: the centre pixel is the pure stroke
        // colour — the byte-identical fast path.
        let opaque = render_frame_at(&opacity_marker_project(1.0), 0.0);
        assert_eq!(opaque.get_pixel(100, 100), &Rgb([255u8, 0, 0]));
        assert_eq!(opaque.get_pixel(10, 10), &bg);
        // Opacity 0.5: approximately the 50% blend of stroke
        // colour and background ((100+255)/2, 50, 50) — the
        // segment blend truncates, so allow a small tolerance.
        let half = render_frame_at(&opacity_marker_project(0.5), 0.0);
        let px = half.get_pixel(100, 100).0;
        let ideal = [177.5f32, 50.0, 50.0];
        for ch in 0..3 {
            assert!(
                (px[ch] as f32 - ideal[ch]).abs() <= 2.0,
                "channel {ch}: got {px:?}, ideal {ideal:?}"
            );
        }
        assert_eq!(half.get_pixel(10, 10), &bg);
        // Opacity 0.0: the background is untouched, exactly.
        let clear = render_frame_at(&opacity_marker_project(0.0), 0.0);
        assert_eq!(clear.get_pixel(100, 100), &bg);
        assert_eq!(clear.get_pixel(10, 10), &bg);
    }

    /// R10 scale marker: on a black 128x96 stage, a white
    /// filled rectangle spanning stage (40,30)..(88,66) and
    /// a hairline white segment (width 0.5) at y=84 — precise
    /// probes for whether a scaled render truly re-renders
    /// at the new size instead of resampling the 1.0 raster.
    fn scale_marker_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [128.0, 96.0];
        project.background = 0x000000;
        project.figures.clear();
        project.frames = vec![crate::model::Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.strokes = vec![
            crate::model::Stroke {
                frame: 0,
                points: vec![
                    [40.0, 30.0],
                    [88.0, 30.0],
                    [88.0, 66.0],
                    [40.0, 66.0],
                ],
                color: 0xFFFFFF,
                width: 1.0,
                widths: Vec::new(),
                fill: Some(0xFFFFFF),
                spray: None,
                opacity: 1.0,
                scatter: 0.0,
            },
            crate::model::Stroke {
                frame: 0,
                points: vec![[8.0, 84.0], [120.0, 84.0]],
                color: 0xFFFFFF,
                width: 0.5,
                widths: Vec::new(),
                fill: None,
                spray: None,
                opacity: 1.0,
                scatter: 0.0,
            },
        ];
        project.camera.clear();
        project
    }

    #[test]
    fn scaled_render_dimensions_follow_the_scale() {
        let project = scale_marker_project();
        let half = render_frame_at_scaled(&project, 0.0, 0.5).unwrap();
        assert_eq!((half.width(), half.height()), (64, 48), "0.5x dims");
        let one = render_frame_at_scaled(&project, 0.0, 1.0).unwrap();
        assert_eq!((one.width(), one.height()), (128, 96), "1.0x dims");
        let two = render_frame_at_scaled(&project, 0.0, 2.0).unwrap();
        assert_eq!((two.width(), two.height()), (256, 192), "2.0x dims");
        // The 1.0 size rule is the historical truncation, even
        // for a fractional stage — pinned directly so a future
        // "tidy" rounding change cannot slip through.
        let mut frac = scale_marker_project();
        frac.stage = [100.5, 50.25];
        assert_eq!(scaled_output_size(&frac, 1.0), (100, 50));
        assert_eq!(scaled_output_size(&frac, 2.0), (201, 101));
    }

    #[test]
    fn scaled_render_at_one_is_byte_identical() {
        let project = scale_marker_project();
        let plain = render_frame_at(&project, 0.0);
        let scaled = render_frame_at_scaled(&project, 0.0, 1.0).unwrap();
        assert_eq!(plain.dimensions(), scaled.dimensions());
        assert_eq!(plain.as_raw(), scaled.as_raw(), "1.0 must not change a byte");
        let plain_idx = render_frame(&project, 0);
        let scaled_idx = render_frame_scaled(&project, 0, 1.0).unwrap();
        assert_eq!(plain_idx.as_raw(), scaled_idx.as_raw());
    }

    #[test]
    fn scaled_render_truly_rescales_content() {
        let project = scale_marker_project();
        let white = Rgb([255u8, 255, 255]);
        let one = render_frame_at_scaled(&project, 0.0, 1.0).unwrap();
        let two = render_frame_at_scaled(&project, 0.0, 2.0).unwrap();
        let half = render_frame_at_scaled(&project, 0.0, 0.5).unwrap();
        // The filled rectangle's centre lands at the scaled
        // coordinate at every scale.
        assert_eq!(one.get_pixel(64, 48), &white);
        assert_eq!(two.get_pixel(128, 96), &white);
        assert_eq!(half.get_pixel(32, 24), &white);
        // Its pure-white area grows with the SQUARE of the
        // scale (within edge tolerance) — a resample of the
        // 1.0 raster would smear the edges and add far less.
        let count = |img: &ImageBuffer<Rgb<u8>, Vec<u8>>| {
            img.pixels().filter(|p| p.0 == [255, 255, 255]).count() as f32
        };
        let ratio = count(&two) / count(&one);
        assert!(
            (3.0..=5.0).contains(&ratio),
            "2.0x white area ratio {ratio}, expected ~4"
        );
        // The hairline (0.5 stage units wide) is the sharpest
        // witness: at 1.0 its centre pixel is a half-covered
        // grey, but a true 2.0 re-render covers the centre
        // fully — an upscale of the 1.0 image could never
        // raise the centre to full white.
        let centre_1x = one.get_pixel(64, 84).0[0];
        assert!(centre_1x < 200, "1.0 hairline centre should be grey, got {centre_1x}");
        assert_eq!(two.get_pixel(128, 168), &white, "2.0 hairline centre must be full white");
    }

    #[test]
    fn scaled_render_refuses_invalid_scales() {
        let project = scale_marker_project();
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let err = render_frame_at_scaled(&project, 0.0, bad)
                .err()
                .expect("invalid scale must refuse");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "scale {bad}");
            assert!(render_frame_scaled(&project, 0, bad).is_err(), "scale {bad}");
            assert!(export_png_sequence_scaled(&project, Path::new("/tmp/styk-r10-bad"), bad).is_err());
            assert!(
                export_png_sequence_range_scaled(&project, 0, 0, Path::new("/tmp/styk-r10-bad"), bad)
                    .is_err()
            );
        }
        // Refusal happens before any filesystem work.
        assert!(!Path::new("/tmp/styk-r10-bad").exists());
    }

    #[test]
    fn scaled_png_sequence_writes_scaled_frames() {
        let project = scale_marker_project();
        let base = std::env::temp_dir().join(format!("styk-r10-seq-{}", std::process::id()));
        let dir2x = base.join("two");
        let n = export_png_sequence_scaled(&project, &dir2x, 2.0).unwrap();
        assert_eq!(n, 1);
        let img = image::open(dir2x.join("frame-0001.png")).unwrap();
        assert_eq!((img.width(), img.height()), (256, 192), "2.0x sequence frame dims");
        let dir_half = base.join("half");
        let n = export_png_sequence_range_scaled(&project, 0, 0, &dir_half, 0.5).unwrap();
        assert_eq!(n, 1);
        let img = image::open(dir_half.join("frame-0001.png")).unwrap();
        assert_eq!((img.width(), img.height()), (64, 48), "0.5x sequence frame dims");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scaled_jpeg_sequence_writes_scaled_frames() {
        let project = tiny_project(); // 96x72 stage, 5 frames
        let base = std::env::temp_dir().join(format!("styk-t7-jpeg-seq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir2x = base.join("two");
        let n = export_jpeg_sequence_scaled(&project, &dir2x, 2.0).unwrap();
        assert_eq!(n, 5);
        let img = image::open(dir2x.join("frame-0001.jpg")).unwrap();
        assert_eq!((img.width(), img.height()), (192, 144), "2.0x JPEG frame dims");
        let bytes = std::fs::read(dir2x.join("frame-0001.jpg")).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "scaled frame: missing JPEG SOI marker");
        let dir_half = base.join("half");
        let n = export_jpeg_sequence_range_scaled(&project, 0, 0, &dir_half, 0.5).unwrap();
        assert_eq!(n, 1);
        let img = image::open(dir_half.join("frame-0001.jpg")).unwrap();
        assert_eq!((img.width(), img.height()), (48, 36), "0.5x JPEG frame dims");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scaled_jpeg_sequence_at_one_is_byte_identical() {
        let project = tiny_project();
        let base = std::env::temp_dir().join(format!("styk-t7-jpeg-one-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let plain_dir = base.join("plain");
        let scaled_dir = base.join("scaled");
        let ranged_dir = base.join("ranged");
        let n_plain = export_jpeg_sequence(&project, &plain_dir).unwrap();
        let n_scaled = export_jpeg_sequence_scaled(&project, &scaled_dir, 1.0).unwrap();
        let n_ranged =
            export_jpeg_sequence_range_scaled(&project, 0, 4, &ranged_dir, 1.0).unwrap();
        assert_eq!(n_plain, 5);
        assert_eq!(n_plain, n_scaled);
        assert_eq!(n_plain, n_ranged);
        for i in 1..=n_plain {
            let name = format!("frame-{i:04}.jpg");
            let plain_bytes = std::fs::read(plain_dir.join(&name)).unwrap();
            assert_eq!(
                plain_bytes,
                std::fs::read(scaled_dir.join(&name)).unwrap(),
                "{name}: scaled 1.0 bytes"
            );
            assert_eq!(
                plain_bytes,
                std::fs::read(ranged_dir.join(&name)).unwrap(),
                "{name}: range scaled 1.0 bytes"
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scaled_jpeg_sequence_refuses_invalid_scales() {
        let project = tiny_project();
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let dir = std::env::temp_dir()
                .join(format!("styk-t7-jpeg-bad-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let err = export_jpeg_sequence_scaled(&project, &dir, bad)
                .err()
                .expect("invalid scale must refuse");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "scale {bad}");
            assert!(
                export_jpeg_sequence_range_scaled(&project, 0, 0, &dir, bad).is_err(),
                "scale {bad}"
            );
            assert!(!dir.exists(), "refused JPEG export created a directory (scale {bad})");
        }
    }

    #[test]
    fn scaled_gif_writes_scaled_frames() {
        let project = tiny_project(); // 96x72 stage, 5 frames
        let path = temp_file("gif-scaled", "gif");
        let n = export_gif_scaled(&project, &path, 2.0).unwrap();
        assert_eq!(n, 5);
        assert_eq!(
            image::image_dimensions(&path).unwrap(),
            (192, 144),
            "2.0x GIF frame size"
        );
        use image::AnimationDecoder as _;
        let file = std::fs::File::open(&path).unwrap();
        let decoder =
            image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        assert_eq!(decoder.into_frames().count(), 5, "all 5 frames at 2.0x");
        let range_path = temp_file("gif-scaled-range", "gif");
        let n = export_gif_range_scaled(&project, 1, 3, &range_path, 0.5).unwrap();
        assert_eq!(n, 3);
        assert_eq!(
            image::image_dimensions(&range_path).unwrap(),
            (48, 36),
            "0.5x GIF frame size"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn scaled_apng_writes_scaled_frames() {
        let project = tiny_project(); // 96x72 stage, 5 frames
        let path = temp_file("apng-scaled", "png");
        let n = export_apng_scaled(&project, &path, 2.0).unwrap();
        assert_eq!(n, 5);
        assert_eq!(
            image::image_dimensions(&path).unwrap(),
            (192, 144),
            "2.0x APNG frame size"
        );
        let range_path = temp_file("apng-scaled-range", "png");
        let n = export_apng_range_scaled(&project, 1, 3, &range_path, 0.5).unwrap();
        assert_eq!(n, 3);
        assert_eq!(
            image::image_dimensions(&range_path).unwrap(),
            (48, 36),
            "0.5x APNG frame size"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn scaled_sheet_tiles_at_the_scaled_tile_size() {
        let project = tiny_project(); // 96x72 tiles, 5 frames
        let path = temp_file("sheet-scaled", "png");
        let n = export_sheet_scaled(&project, &path, 2.0).unwrap();
        assert_eq!(n, 5);
        // Tiles are 192x144 on the square-root grid: 3 cols x 2 rows.
        let sheet = image::open(&path).unwrap().to_rgb8();
        assert_eq!((sheet.width(), sheet.height()), (576, 288), "2.0x sheet size");
        // Tile 0 is the true 2.0 re-render of authored frame 0.
        let tile0 = image::imageops::crop_imm(&sheet, 0, 0, 192, 144)
            .to_image()
            .into_raw();
        assert_eq!(
            tile0,
            render_frame_scaled(&project, 0, 2.0).unwrap().into_raw(),
            "tile 0 must be the 2.0 re-render of frame 0"
        );
        // Range at 0.5: frames 1..=3 as 48x36 tiles, 2 cols x 2 rows.
        let range_path = temp_file("sheet-scaled-range", "png");
        let n = export_sheet_range_scaled(&project, 1, 3, &range_path, 0.5).unwrap();
        assert_eq!(n, 3);
        let sheet = image::open(&range_path).unwrap().to_rgb8();
        assert_eq!((sheet.width(), sheet.height()), (96, 72), "0.5x sheet size");
        let tile1 = image::imageops::crop_imm(&sheet, 48, 0, 48, 36)
            .to_image()
            .into_raw();
        assert_eq!(
            tile1,
            render_frame_scaled(&project, 2, 0.5).unwrap().into_raw(),
            "tile 1 of the range sheet must be the 0.5 re-render of frame 2"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn scaled_animation_exports_at_one_are_byte_identical() {
        let project = tiny_project();
        // GIF: full and range entries at 1.0 delegate to the
        // unscaled code path, so the files must match byte for
        // byte — the guarantee S5's MCP routing relies on when
        // a caller passes scale 1.0 explicitly.
        let plain = temp_file("gif-one-plain", "gif");
        let scaled = temp_file("gif-one-scaled", "gif");
        let ranged = temp_file("gif-one-ranged", "gif");
        export_gif(&project, &plain).unwrap();
        export_gif_scaled(&project, &scaled, 1.0).unwrap();
        export_gif_range_scaled(&project, 0, 4, &ranged, 1.0).unwrap();
        let plain_bytes = std::fs::read(&plain).unwrap();
        assert_eq!(plain_bytes, std::fs::read(&scaled).unwrap(), "GIF 1.0 bytes");
        assert_eq!(plain_bytes, std::fs::read(&ranged).unwrap(), "GIF range 1.0 bytes");
        // APNG: same trio.
        let plain = temp_file("apng-one-plain", "png");
        let scaled = temp_file("apng-one-scaled", "png");
        let ranged = temp_file("apng-one-ranged", "png");
        export_apng(&project, &plain).unwrap();
        export_apng_scaled(&project, &scaled, 1.0).unwrap();
        export_apng_range_scaled(&project, 0, 4, &ranged, 1.0).unwrap();
        let plain_bytes = std::fs::read(&plain).unwrap();
        assert_eq!(plain_bytes, std::fs::read(&scaled).unwrap(), "APNG 1.0 bytes");
        assert_eq!(plain_bytes, std::fs::read(&ranged).unwrap(), "APNG range 1.0 bytes");
        // Spritesheet: same trio.
        let plain = temp_file("sheet-one-plain", "png");
        let scaled = temp_file("sheet-one-scaled", "png");
        let ranged = temp_file("sheet-one-ranged", "png");
        export_spritesheet(&project, &plain).unwrap();
        export_sheet_scaled(&project, &scaled, 1.0).unwrap();
        export_sheet_range_scaled(&project, 0, 4, &ranged, 1.0).unwrap();
        let plain_bytes = std::fs::read(&plain).unwrap();
        assert_eq!(plain_bytes, std::fs::read(&scaled).unwrap(), "sheet 1.0 bytes");
        assert_eq!(plain_bytes, std::fs::read(&ranged).unwrap(), "sheet range 1.0 bytes");
        for p in [
            "gif-one-plain", "gif-one-scaled", "gif-one-ranged",
        ] {
            let _ = std::fs::remove_file(temp_file(p, "gif"));
        }
        for p in [
            "apng-one-plain", "apng-one-scaled", "apng-one-ranged",
            "sheet-one-plain", "sheet-one-scaled", "sheet-one-ranged",
        ] {
            let _ = std::fs::remove_file(temp_file(p, "png"));
        }
    }

    #[test]
    fn scaled_animation_exports_refuse_invalid_scales() {
        let project = tiny_project();
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let gif_path = temp_file("gif-bad", "gif");
            let _ = std::fs::remove_file(&gif_path);
            let err = export_gif_scaled(&project, &gif_path, bad)
                .err()
                .expect("invalid scale must refuse");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "scale {bad}");
            assert!(export_gif_range_scaled(&project, 0, 4, &gif_path, bad).is_err());
            assert!(!gif_path.exists(), "refused GIF export created a file (scale {bad})");

            let apng_path = temp_file("apng-bad", "png");
            let _ = std::fs::remove_file(&apng_path);
            let err = export_apng_scaled(&project, &apng_path, bad)
                .err()
                .expect("invalid scale must refuse");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "scale {bad}");
            assert!(export_apng_range_scaled(&project, 0, 4, &apng_path, bad).is_err());
            assert!(!apng_path.exists(), "refused APNG export created a file (scale {bad})");

            let sheet_path = temp_file("sheet-bad", "png");
            let _ = std::fs::remove_file(&sheet_path);
            let err = export_sheet_scaled(&project, &sheet_path, bad)
                .err()
                .expect("invalid scale must refuse");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "scale {bad}");
            assert!(export_sheet_range_scaled(&project, 0, 4, &sheet_path, bad).is_err());
            assert!(!sheet_path.exists(), "refused sheet export created a file (scale {bad})");
        }
    }

    #[test]
    fn hidden_fx_clip_renders_nothing() {
        // A Burst clip on the demo figure's root, starting at
        // frame 0: at t = 0 its particles sit on the anchor at
        // full alpha, so the visible render must differ from
        // the plain project; the same clip flagged hidden must
        // render byte-identically to no clip at all.
        let burst = |hidden: bool| crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 10.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFF4040,
                seed: 5,
                angle: 0.0,
                length: 80.0,
                color2: None,
            },
            figure: 0,
            node: 0,
            hidden,
        };
        let plain = render_frame_at(&Project::new_demo(), 0.0);
        let mut visible_project = Project::new_demo();
        visible_project.fx_clips.push(burst(false));
        let visible = render_frame_at(&visible_project, 0.0);
        assert_ne!(
            visible.as_raw(),
            plain.as_raw(),
            "a live Burst clip must draw at its start frame"
        );
        let mut hidden_project = Project::new_demo();
        hidden_project.fx_clips.push(burst(true));
        let hidden = render_frame_at(&hidden_project, 0.0);
        assert_eq!(
            hidden.as_raw(),
            plain.as_raw(),
            "a hidden clip must render exactly as if it were absent"
        );
    }

    /// Wave P (P17): graph channels are part of the evaluated
    /// frame. The stage draws the figure through
    /// `EditorCore::positions_f` (channelled poses + defs), so
    /// the rasterizer must render the same frame: a project
    /// whose channels move/lengthen/rotate the figure exports
    /// byte-identically to the same project with those values
    /// baked into its poses and def, and differently from the
    /// un-channelled project. A Burst clip (anchor evaluation)
    /// and a Trail clip (the shared `node_trail` helper) ride
    /// along, so the FX anchor and trail funnels are pinned by
    /// the same byte comparison.
    #[test]
    fn channelled_figure_exports_at_the_channelled_position() {
        use crate::graph::{Channel, ChannelTarget, Key};
        let base = || {
            let mut project = Project::new_demo();
            let clip = |kind| crate::model::FxClipEntry {
                clip: crate::vfx::ParticleClip {
                    start: 0.0,
                    duration: 10.0,
                    kind,
                    color: 0xFF4040,
                    seed: 5,
                    angle: 0.0,
                    length: 80.0,
                    color2: None,
                },
                figure: 0,
                node: 5,
                hidden: false,
            };
            project.fx_clips.push(clip(crate::vfx::ClipKind::Burst));
            project.fx_clips.push(clip(crate::vfx::ClipKind::Trail));
            project
        };
        let chan = |target: ChannelTarget, value: f32| Channel {
            figure: 0,
            target,
            keys: vec![Key::flat(0.0, value)],
        };
        let mut channelled = base();
        channelled.channels = vec![
            chan(ChannelTarget::RootX, 600.0),
            chan(ChannelTarget::Angle(1), 45.0),
            chan(ChannelTarget::Length(1), 99.0),
        ];
        // The baked equivalent: the same three values written
        // into every frame's pose and the figure's def.
        let mut baked = base();
        for frame in &mut baked.frames {
            frame.poses[0].root[0] = 600.0;
            frame.poses[0].angles[1] = 45.0;
        }
        baked.figures[0].def.nodes[1].length = 99.0;

        // The trail helper itself: channelled samples must be
        // the baked samples, point for point (and non-empty, or
        // the byte comparison below would not cover it).
        let trail_channels = node_trail(&channelled, 2.0, 0, 5, 8, 5.0);
        let trail_baked = node_trail(&baked, 2.0, 0, 5, 8, 5.0);
        assert!(!trail_baked.is_empty(), "fixture must draw a trail");
        assert_eq!(trail_channels, trail_baked, "node_trail must follow channels");

        // Frame 2: figure + trail path. Frame 0: the Burst
        // clip's particles sit exactly on the anchor, so the
        // anchor funnel is byte-pinned there too.
        for f in [2.0, 0.0] {
            let plain = render_frame_at(&base(), f);
            let via_channels = render_frame_at(&channelled, f);
            let via_bake = render_frame_at(&baked, f);
            assert_ne!(
                via_channels.as_raw(),
                plain.as_raw(),
                "channels must move the exported figure (frame {f})"
            );
            assert_eq!(
                via_channels.as_raw(),
                via_bake.as_raw(),
                "channelled export must equal the baked equivalent (frame {f})"
            );
        }
    }

    /// A single horizontal white bar (length 200, thickness 10)
    /// on black: root (100, 150) -> joint (300, 150).
    fn bend_bar_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [400.0, 300.0];
        project.background = 0x000000;
        project.figures.clear();
        for frame in &mut project.frames {
            frame.poses.clear();
        }
        project.camera.clear();
        let src = crate::model::FigureDef::humanoid();
        let mut bar = src.nodes[1].clone();
        bar.parent = Some(0);
        bar.length = 200.0;
        bar.rest_angle = 90.0; // dir(90 deg) = +x
        bar.thickness = 10.0;
        bar.color = Some(0xFFFFFF);
        bar.head = false;
        bar.shape = crate::model::Shape::Line;
        let def = crate::model::FigureDef {
            nodes: vec![src.nodes[0].clone(), bar],
        };
        project.add_figure_with(def, "Bar", 0xFFFFFF, [100.0, 150.0]);
        project
    }

    #[test]
    fn bent_segment_arcs_through_the_bend_midpoint() {
        let mut project = bend_bar_project();
        let white = Rgb([255u8, 255, 255]);
        let black = Rgb([0u8, 0, 0]);
        // Straight: the chord midpoint is covered; 18px below the
        // chord is background (the half-width is only 5).
        let straight = render_frame_at(&project, 0.0);
        assert_eq!(straight.get_pixel(200, 150), &white);
        assert_eq!(straight.get_pixel(200, 168), &black);
        // Bend node 1 by +0.7 rad (wave 7): the joints stay put
        // but the drawn path bulges through bend_mid.
        project.frames[0].bends = vec![vec![0.0, 0.7]];
        let bent = render_frame_at(&project, 0.0);
        assert_ne!(straight.as_raw(), bent.as_raw(), "a bend must change the frame");
        let mid = crate::model::bend_mid([100.0, 150.0], [300.0, 150.0], 0.7);
        let (mx, my) = (mid[0].round() as u32, mid[1].round() as u32);
        assert_eq!(bent.get_pixel(mx, my), &white, "arc joint at {mid:?} must be covered");
        assert_eq!(bent.get_pixel(200, 168), &white, "the bulge passes below the chord");
        assert_eq!(bent.get_pixel(200, 150), &black, "the chord midpoint is vacated");
    }

    /// A lone white head node at the centre of a black
    /// 400x300 stage, one figure, no camera keys (so the
    /// camera is the stage centre at zoom 1.0) — nothing
    /// else draws, so the only non-background pixels are
    /// the head ring's band.
    fn head_only_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [400.0, 300.0];
        project.background = 0x000000;
        project.figures.clear();
        for frame in &mut project.frames {
            frame.poses.clear();
        }
        project.camera.clear();
        let src = crate::model::FigureDef::humanoid();
        let mut head = src.nodes[0].clone();
        head.head = true;
        head.thickness = 4.0;
        head.color = Some(0xFFFFFF);
        let def = crate::model::FigureDef { nodes: vec![head] };
        project.add_figure_with(def, "Head", 0xFFFFFF, [200.0, 150.0]);
        project
    }

    #[test]
    fn head_ring_scale_doubles_the_rendered_ring_radius() {
        let white = Rgb([255u8, 255, 255]);
        let black = Rgb([0u8, 0, 0]);
        // Default scale (1.0): the ring band (radius 17,
        // half-width 2 at zoom 1) covers the pixel 17 px
        // right of the head centre (200, 150); 34 px out is
        // background.
        let project = head_only_project();
        let plain = render_frame_at(&project, 0.0);
        assert_eq!(plain.get_pixel(217, 150), &white, "base ring at radius 17");
        assert_eq!(plain.get_pixel(234, 150), &black, "nothing at radius 34 at scale 1");
        // Scale 2.0: `head_ring_radius()` is 34.0, so the
        // band moves out to 34 x zoom — covered at 34 px,
        // vacated at 17 px.
        let mut project = head_only_project();
        project.head_ring_scale = 2.0;
        assert_eq!(project.head_ring_radius(), 34.0);
        let scaled = render_frame_at(&project, 0.0);
        assert_eq!(scaled.get_pixel(234, 150), &white, "scaled ring at radius 34");
        assert_eq!(scaled.get_pixel(217, 150), &black, "radius 17 vacated at scale 2");
    }

    #[test]
    fn camera_angle_zero_renders_byte_identical() {
        let project = Project::new_demo();
        let plain = render_frame(&project, 2);
        let mut keyed = Project::new_demo();
        keyed.set_camera_key(2, 480.0, 360.0, 1.0); // == default view, angle 0
        let with_key = render_frame(&keyed, 2);
        assert_eq!(plain.as_raw(), with_key.as_raw());
    }

    #[test]
    fn write_quadruped_frame_for_visual_check() {
        let mut project = Project::new_demo();
        project.figures.clear();
        for frame in &mut project.frames {
            frame.poses.clear();
        }
        project.add_figure_with(
            crate::model::FigureDef::quadruped(),
            "Dog",
            0xFFC65D,
            [300.0, 400.0],
        );
        project.set_camera_key(2, 420.0, 390.0, 1.6);
        render_frame(&project, 2).save("/tmp/styk-quad.png").unwrap();
        render_frame(&project, 0).save("/tmp/styk-quad-wide.png").unwrap();
    }

    #[test]
    fn particles_are_deterministic_and_live_mid_effect() {
        let mut project = Project::new_demo();
        project.particles.push(crate::model::ParticleEvent {
            frame: 1,
            figure: Some(0),
            node: Some(6),
            x: 300.0,
            y: 300.0,
            kind: ParticleKind::Burst,
            color: 0xFFC65D,
            seed: 42,
        });
        let a = particles_at(&project, 3.0);
        let b = particles_at(&project, 3.0);
        assert_eq!(a.len(), b.len());
        assert!(!a.is_empty(), "burst should be alive 2 frames in");
        assert!(particles_at(&project, 40.0).is_empty(), "and long dead later");
    }

    #[test]
    fn mp4_export_produces_a_video_file() {
        let project = Project::new_demo();
        let path = std::path::Path::new("/tmp/styk-test.mp4");
        export_mp4(&project, path).unwrap();
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() > 3000, "mp4 too small: {}", bytes.len());
        assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
    }

    #[test]
    fn write_shapes_and_particles_for_visual_check() {
        let mut project = Project::new_demo();
        // Give the demo figure a circle "shield" on the left hand and
        // a triangular segment on the right hand, plus a burst.
        project.figures[0].def.nodes[4].shape = Shape::Circle;
        project.figures[0].def.nodes[4].length = 44.0;
        project.figures[0].def.nodes[6].shape = Shape::Polygon(3);
        project.figures[0].def.nodes[6].length = 40.0;
        project.particles.push(crate::model::ParticleEvent {
            frame: 2,
            figure: Some(0),
            node: Some(6),
            x: 620.0,
            y: 300.0,
            kind: ParticleKind::Burst,
            color: 0xFFC65D,
            seed: 7,
        });
        render_frame_at(&project, 4.0).save("/tmp/styk-shapes.png").unwrap();
    }

    #[test]
    fn webm_export_produces_a_webm_file() {
        let project = Project::new_demo();
        let path = std::path::Path::new("/tmp/styk-test.webm");
        export_webm(&project, path).unwrap();
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() > 2000, "webm too small: {}", bytes.len());
        assert_eq!(&bytes[0..4], &[0x1A, 0x45, 0xDF, 0xA3], "not an EBML/WebM file");
    }

    #[test]
    fn renders_non_background_pixels() {
        let project = Project::new_demo();
        let img = render_frame(&project, 0);
        let bg = rgb(project.background);
        let drawn = img.pixels().filter(|p| p.0 != bg).count();
        assert!(drawn > 500, "expected the figure to be drawn, got {drawn} px");
    }

    #[test]
    fn contact_sheet_tiles_frames_on_a_grid() {
        let project = Project::new_demo(); // 960x720 stage
        let sheet = render_contact_sheet(&project, &[0, 1, 2, 3], 2, 48);
        // Thumbs keep the 4:3 stage aspect: 48 wide, 36 tall.
        // Sheet = 2 thumbs + 3 gaps of 8 wide, 2 rows + 3 gaps tall.
        assert_eq!(sheet.width(), 2 * 48 + 3 * 8, "sheet width");
        assert_eq!(sheet.height(), 2 * 36 + 3 * 8, "sheet height");
        // The outer padding is the sheet background...
        assert_eq!(sheet.get_pixel(0, 0).0, [26, 29, 36, 255]);
        // ...and every tile area was actually drawn into (a tile's
        // centre pixel comes from a rendered frame, whose stage
        // background differs from the sheet matte).
        let centre = sheet.get_pixel(8 + 24, 8 + 18).0;
        assert_ne!(centre, [26, 29, 36, 255], "first tile should hold a rendered frame");
    }

    #[test]
    fn contact_sheet_empty_list_is_1x1_background() {
        let project = Project::new_demo();
        let sheet = render_contact_sheet(&project, &[], 3, 64);
        assert_eq!((sheet.width(), sheet.height()), (1, 1));
        assert_eq!(sheet.get_pixel(0, 0).0, [26, 29, 36, 255]);
    }

    /// The 5-frame demo project on a small stage so renders
    /// stay fast (Phase I section 8: local copies of the
    /// range module's helpers — sibling test modules cannot
    /// share private helpers).
    fn tiny_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [96.0, 72.0];
        project
    }

    fn temp_file(tag: &str, ext: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "styk-sheet-{tag}-{}.{ext}",
            std::process::id()
        ))
    }

    #[test]
    fn contact_sheet_range_tiles_exactly_the_sub_range_frames() {
        let project = tiny_project();
        assert_eq!(project.frames.len(), 5);
        let path = temp_file("sheet-sub", "png");
        let n = export_contact_sheet_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 3, "frames 1..=3 of 5 must tile 3 frames");
        // The written sheet is pixel-for-pixel the in-memory sheet
        // of exactly frames 1, 2 and 3 at the export defaults.
        let written = image::open(&path).unwrap().to_rgba8();
        let expected = render_contact_sheet(
            &project,
            &[1, 2, 3],
            CONTACT_SHEET_COLS,
            CONTACT_SHEET_THUMB_W,
        );
        assert_eq!(written.dimensions(), expected.dimensions());
        assert_eq!(written.as_raw(), expected.as_raw());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn contact_sheet_range_clamps_to_the_timeline() {
        let project = tiny_project();
        let path = temp_file("sheet-clamp", "png");
        let n = export_contact_sheet_range(&project, 3, 999, &path).unwrap();
        assert_eq!(n, 2, "3..=999 on 5 frames clamps to frames 3..=4");
        let written = image::open(&path).unwrap().to_rgba8();
        let expected = render_contact_sheet(
            &project,
            &[3, 4],
            CONTACT_SHEET_COLS,
            CONTACT_SHEET_THUMB_W,
        );
        assert_eq!(written.as_raw(), expected.as_raw());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn contact_sheet_range_rejects_empty_and_inverted_ranges() {
        let project = tiny_project();
        let path = temp_file("sheet-err", "png");
        assert!(
            export_contact_sheet_range(&project, 3, 1, &path).is_err(),
            "inverted range"
        );
        assert!(
            export_contact_sheet_range(&project, 99, 2, &path).is_err(),
            "99..=2 clamps to 4..=2, still empty"
        );
        let mut empty = tiny_project();
        empty.frames.clear();
        assert!(
            export_contact_sheet_range(&empty, 0, 0, &path).is_err(),
            "an empty project has no range to export"
        );
    }

    #[test]
    fn contact_sheet_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_path = temp_file("sheet-full", "png");
        let range_path = temp_file("sheet-fullrange", "png");
        let n_full = export_contact_sheet(&project, &full_path).unwrap();
        let n_range = export_contact_sheet_range(
            &project,
            0,
            project.frames.len() - 1,
            &range_path,
        )
        .unwrap();
        assert_eq!(n_full, 5);
        assert_eq!(n_full, n_range);
        // Same tiles through the same writer: the files are
        // byte-identical, as in the Wave 10 determinism suite.
        assert_eq!(
            std::fs::read(&full_path).unwrap(),
            std::fs::read(&range_path).unwrap()
        );
        let _ = std::fs::remove_file(&full_path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn contact_sheet_export_of_empty_project_writes_the_blank_sheet() {
        let mut project = tiny_project();
        project.frames.clear();
        let path = temp_file("sheet-empty", "png");
        let n = export_contact_sheet(&project, &path).unwrap();
        assert_eq!(n, 0, "an empty project tiles zero frames");
        let written = image::open(&path).unwrap().to_rgba8();
        assert_eq!(written.dimensions(), (1, 1));
        assert_eq!(written.get_pixel(0, 0).0, [26, 29, 36, 255]);
        let _ = std::fs::remove_file(&path);
    }


    /// One-figure test project: a single horizontal segment from
    /// (50, 100) to (150, 100) on a 200x200 black stage, thickness
    /// 20 (half-width 10), coloured `seg_color`, styled `style`.
    fn styled_project(style: crate::model::NodeStyle, seg_color: u32) -> Project {
        use crate::model::{
            Easing, FigureDef, FigureInstance, Frame, NodeDef, Pose, Shape,
        };
        let node = |parent: Option<usize>, length: f32, thickness: f32| NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: crate::model::NodeStyle::default(),
            media: None,
        };
        let mut seg = node(Some(0), 100.0, 20.0);
        seg.style = style;
        seg.color = Some(seg_color);
        Project {
            name: "style-test".into(),
            fps: 12.0,
            stage: [200.0, 200.0],
            background: 0x000000,
            figures: vec![FigureInstance {
                locked: false,
                opacity: 1.0,
                name: "F".into(),
                color: 0xFFFFFF,
                def: FigureDef {
                    nodes: vec![node(None, 0.0, 20.0), seg],
                },
                hidden: false,
                clip: None,
                states: Vec::new(),
            }],
            frames: vec![Frame {
                poses: vec![Pose {
                    root: [50.0, 100.0],
                    angles: vec![0.0, 90.0],
                }],
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            }],
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
            format_version: crate::model::FORMAT_VERSION_CURRENT,
        }
    }

    #[test]
    fn point_cap_narrows_the_far_end() {
        use crate::model::{CapKind, NodeStyle};
        let style = NodeStyle {
            cap: CapKind::Point,
            ..NodeStyle::default()
        };
        let styled = render_frame(&styled_project(style, 0xFF0000), 0);
        let plain = render_frame(&styled_project(NodeStyle::default(), 0xFF0000), 0);
        // 5-6 units off the centreline near the far end (t ~ 0.96,
        // local half ~0.4 with a point cap): covered by the default
        // round segment, bare background with the point cap.
        assert_eq!(plain.get_pixel(145, 105).0, [255, 0, 0]);
        assert_eq!(styled.get_pixel(145, 105).0, [0, 0, 0]);
        // Near the near end (t ~ 0.06, local half ~9.4) the point
        // cap still covers the same pixel row.
        assert_eq!(styled.get_pixel(55, 105).0, [255, 0, 0]);
    }

    #[test]
    fn taper_narrows_the_far_end() {
        use crate::model::NodeStyle;
        let style = NodeStyle {
            taper: 0.2,
            ..NodeStyle::default()
        };
        let styled = render_frame(&styled_project(style, 0xFF0000), 0);
        let plain = render_frame(&styled_project(NodeStyle::default(), 0xFF0000), 0);
        // At t ~ 0.96 the tapered half-width is ~2.4, so 5-6 units
        // off-axis is outside; the uniform segment covers it.
        assert_eq!(plain.get_pixel(145, 105).0, [255, 0, 0]);
        assert_eq!(styled.get_pixel(145, 105).0, [0, 0, 0]);
        // The centreline is covered either way.
        assert_eq!(styled.get_pixel(145, 100).0, [255, 0, 0]);
    }

    #[test]
    fn square_cap_extends_past_the_endpoints() {
        use crate::model::{CapKind, NodeStyle};
        let style = NodeStyle {
            cap: CapKind::Square,
            ..NodeStyle::default()
        };
        let styled = render_frame(&styled_project(style, 0xFF0000), 0);
        let plain = render_frame(&styled_project(NodeStyle::default(), 0xFF0000), 0);
        // 8 units beyond the far joint, dead on the centreline: a
        // square cap reaches half-width (10) past the end, a round
        // cap only covers within the radius *around* the point, so
        // an on-axis pixel 8 past the end is inside both reaches —
        // use an off-axis pixel instead: 8 past the end and 8 off
        // axis is inside the square (perp 8 <= 10) but ~11.3 from
        // the endpoint, outside the round cap.
        assert_eq!(plain.get_pixel(158, 108).0, [0, 0, 0]);
        assert_eq!(styled.get_pixel(158, 108).0, [255, 0, 0]);
    }

    #[test]
    fn gradient_reaches_the_midpoint_colour() {
        use crate::model::NodeStyle;
        let style = NodeStyle {
            color2: Some(0x0000FF),
            ..NodeStyle::default()
        };
        let img = render_frame(&styled_project(style, 0xFF0000), 0);
        let mid = img.get_pixel(100, 100).0;
        assert!(
            (mid[0] as i32 - 128).abs() <= 6 && mid[1] == 0 && (mid[2] as i32 - 128).abs() <= 6,
            "midpoint should be ~ (128, 0, 128), got {mid:?}"
        );
        // The near end stays (nearly) pure red.
        let near = img.get_pixel(52, 100).0;
        assert!(near[0] > 240 && near[2] < 16, "near end should stay red, got {near:?}");
        // And the far end reaches (nearly) pure blue (wave 11):
        // the gradient runs the whole segment axis, not just to
        // the midpoint.
        let far = img.get_pixel(148, 100).0;
        assert!(far[2] > 240 && far[0] < 16, "far end should reach blue, got {far:?}");
    }

    #[test]
    fn no_gradient_render_is_byte_identical() {
        use crate::model::NodeStyle;
        // The determinism contract: engaging the gradient path
        // with both endpoints equal to the segment colour must
        // change zero bytes versus the color2 = None twin —
        // lerp_color(a, a, t) is exactly `a` for every t.
        let plain = render_frame(&styled_project(NodeStyle::default(), 0xFF0000), 0);
        let flat_gradient = render_frame(
            &styled_project(
                NodeStyle {
                    color2: Some(0xFF0000),
                    ..NodeStyle::default()
                },
                0xFF0000,
            ),
            0,
        );
        assert_eq!(plain.as_raw(), flat_gradient.as_raw());
        // Non-vacuity: a real gradient to a different colour does
        // change the buffer (otherwise the equality above would
        // prove nothing).
        let gradient = render_frame(
            &styled_project(
                NodeStyle {
                    color2: Some(0x0000FF),
                    ..NodeStyle::default()
                },
                0xFF0000,
            ),
            0,
        );
        assert_ne!(plain.as_raw(), gradient.as_raw());
    }

    #[test]
    fn opacity_blends_over_the_background() {
        use crate::model::NodeStyle;
        let style = NodeStyle {
            opacity: 0.5,
            ..NodeStyle::default()
        };
        let img = render_frame(&styled_project(style, 0xFF0000), 0);
        let px = img.get_pixel(100, 100).0;
        assert!(
            (120..=135).contains(&px[0]) && px[1] == 0 && px[2] == 0,
            "50% red over black should be ~(127, 0, 0), got {px:?}"
        );
    }

    /// Wave 13 (S10, raster half): a three-figure fixture whose
    /// only painted features near the sampled pixels are a head
    /// ring, a Circle shape ring and a Polygon shape edge, each
    /// on its own node carrying `node_opacity` (the figure
    /// carries `fig_opacity`). Stage 200x200 on black, camera
    /// cleared, so stage points map x2 onto the supersampled
    /// buffer and each sampled output pixel's four sub-pixels
    /// sit squarely inside the feature's band (thickness 8 ->
    /// half-width 8 in buffer units, samples within ~1.5 of the
    /// centreline), clear of every segment (each node's own
    /// segment ends at least 28 buffer units away).
    ///
    /// Sampled output pixels: head ring (77, 100) — ring centre
    /// (60, 100) stage, radius 17, rightmost point; Circle ring
    /// (70, 170) — centre (70, 150), radius 20 (the node's
    /// length 40 drives both its position and the ring radius),
    /// bottom point; Polygon edge (170, 30) — centre (160, 40),
    /// r 20, the midpoint of the top-right edge.
    fn head_shape_project(node_opacity: f32, fig_opacity: f32) -> Project {
        use crate::model::{
            Easing, FigureDef, FigureInstance, Frame, NodeDef, NodeStyle, Pose, Shape,
        };
        let node = |parent: Option<usize>, length: f32| NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 8.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let figure = |special: NodeDef| FigureInstance {
            locked: false,
            opacity: fig_opacity,
            name: "F".into(),
            color: 0xFFFFFF,
            def: FigureDef {
                nodes: vec![node(None, 0.0), special],
            },
            hidden: false,
            clip: None,
            states: Vec::new(),
        };
        let styled = NodeStyle {
            opacity: node_opacity,
            ..NodeStyle::default()
        };
        let mut head_node = node(Some(0), 30.0);
        head_node.head = true;
        head_node.color = Some(0xFF0000);
        head_node.style = styled;
        let mut circle_node = node(Some(0), 40.0);
        circle_node.shape = Shape::Circle;
        circle_node.color = Some(0x0000FF);
        circle_node.style = styled;
        let mut poly_node = node(Some(0), 40.0);
        poly_node.shape = Shape::Polygon(4);
        poly_node.color = Some(0x00FF00);
        poly_node.style = styled;
        let pose = |root: [f32; 2]| Pose {
            root,
            angles: vec![0.0, 90.0],
        };
        Project {
            name: "s10-test".into(),
            fps: 12.0,
            stage: [200.0, 200.0],
            background: 0x000000,
            figures: vec![
                figure(head_node),
                figure(circle_node),
                figure(poly_node),
            ],
            frames: vec![Frame {
                poses: vec![pose([30.0, 100.0]), pose([30.0, 150.0]), pose([120.0, 40.0])],
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            }],
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
            format_version: crate::model::FORMAT_VERSION_CURRENT,
        }
    }

    #[test]
    fn head_ring_and_shape_nodes_compose_node_opacity() {
        // Opacity 1.0: the pure node colours, exactly — and the
        // render is byte-identical to the twin whose nodes carry
        // the DEFAULT style (the S10 change must not move a
        // single byte for default-styled projects).
        let opaque = render_frame(&head_shape_project(1.0, 1.0), 0);
        assert_eq!(opaque.get_pixel(77, 100).0, [255, 0, 0], "head ring at 1.0");
        assert_eq!(opaque.get_pixel(70, 170).0, [0, 0, 255], "circle ring at 1.0");
        assert_eq!(opaque.get_pixel(170, 30).0, [0, 255, 0], "polygon edge at 1.0");
        let mut twin = head_shape_project(1.0, 1.0);
        for fig in &mut twin.figures {
            fig.def.nodes[1].style = crate::model::NodeStyle::default();
        }
        assert_eq!(
            opaque.as_raw(),
            render_frame(&twin, 0).as_raw(),
            "explicit opacity 1.0 must equal the default style byte for byte"
        );
        // Opacity 0.5: each feature is its colour blended at 50%
        // over the black background (~127 per lit channel — the
        // blend truncates, so allow the house tolerance). Before
        // S10 these pixels were the pure colours at ANY style
        // opacity (hard overwrite of the figure-opacity fade).
        let half = render_frame(&head_shape_project(0.5, 1.0), 0);
        for (x, y, ch, what) in [
            (77, 100, 0, "head ring"),
            (70, 170, 2, "circle ring"),
            (170, 30, 1, "polygon edge"),
        ] {
            let px = half.get_pixel(x, y).0;
            assert!(
                (125..=129).contains(&px[ch]),
                "{what} at 0.5: lit channel {} should be ~127, got {px:?}",
                px[ch]
            );
            for other in 0..3 {
                if other != ch {
                    assert_eq!(px[other], 0, "{what} at 0.5: channel {other} must stay 0, got {px:?}");
                }
            }
        }
        assert_ne!(
            opaque.as_raw(),
            half.as_raw(),
            "non-vacuity: the 0.5 render must differ from the 1.0 render"
        );
        // Opacity 0.0: the features vanish exactly (true alpha
        // over the background, not a background-coloured stamp).
        let clear = render_frame(&head_shape_project(0.0, 1.0), 0);
        assert_eq!(clear.get_pixel(77, 100).0, [0, 0, 0]);
        assert_eq!(clear.get_pixel(70, 170).0, [0, 0, 0]);
        assert_eq!(clear.get_pixel(170, 30).0, [0, 0, 0]);
    }

    #[test]
    fn head_ring_opacity_is_the_style_times_figure_product() {
        // style 0.5 x figure 0.5 = effective 0.25: red at 25%
        // over black is ~63. The pre-S10 fade (figure opacity
        // alone, hard overwrite) would paint ~128 here, and a
        // style-only composition ~127 — 63 pins the product.
        let img = render_frame(&head_shape_project(0.5, 0.5), 0);
        let px = img.get_pixel(77, 100).0;
        assert!(
            (61..=65).contains(&px[0]) && px[1] == 0 && px[2] == 0,
            "head ring at style 0.5 x figure 0.5 should be ~(63, 0, 0), got {px:?}"
        );
    }

    #[test]
    fn head_ring_blends_over_the_segment_beneath_it() {
        use crate::model::{
            Easing, FigureDef, FigureInstance, Frame, NodeDef, NodeStyle, Pose, Shape,
        };
        // Figure A: one thick white segment along y = 100.
        // Figure B: a head node at (100, 100) whose ring's
        // rightmost point, (117, 100), lies ON A's segment. The
        // ring (red, style opacity 0.5) must blend over the
        // white beneath it -> (255, ~127, ~127). The pre-S10
        // hard overwrite painted pure red (255, 0, 0); a
        // fade-toward-background composition would too (figure
        // opacity is 1.0 here) — only true alpha over the
        // actual pixel gives the pink.
        let node = |parent: Option<usize>, length: f32, thickness: f32| NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let mut bar = node(Some(0), 170.0, 20.0);
        bar.color = Some(0xFFFFFF);
        let mut head_node = node(Some(0), 40.0, 8.0);
        head_node.head = true;
        head_node.color = Some(0xFF0000);
        head_node.style = NodeStyle {
            opacity: 0.5,
            ..NodeStyle::default()
        };
        let figure = |nodes: Vec<NodeDef>| FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "F".into(),
            color: 0xFFFFFF,
            def: FigureDef { nodes },
            hidden: false,
            clip: None,
            states: Vec::new(),
        };
        let project = Project {
            name: "s10-over".into(),
            fps: 12.0,
            stage: [200.0, 200.0],
            background: 0x000000,
            figures: vec![
                figure(vec![node(None, 0.0, 4.0), bar]),
                figure(vec![node(None, 0.0, 4.0), head_node]),
            ],
            frames: vec![Frame {
                poses: vec![
                    Pose {
                        root: [20.0, 100.0],
                        angles: vec![0.0, 90.0],
                    },
                    Pose {
                        root: [100.0, 60.0],
                        angles: vec![0.0, 0.0],
                    },
                ],
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            }],
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
            format_version: crate::model::FORMAT_VERSION_CURRENT,
        };
        let img = render_frame(&project, 0);
        let px = img.get_pixel(117, 100).0;
        assert!(
            px[0] == 255 && (125..=129).contains(&px[1]) && (125..=129).contains(&px[2]),
            "red ring at 0.5 over a white segment should be (255, ~127, ~127), got {px:?}"
        );
    }

    #[test]
    fn jpeg_sequence_writes_real_jpegs() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-jpeg-test");
        let n = export_jpeg_sequence(&project, dir).unwrap();
        assert_eq!(n, project.frames.len());
        for i in [1, n] {
            let bytes = std::fs::read(dir.join(format!("frame-{i:04}.jpg"))).unwrap();
            assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "frame {i}: missing JPEG SOI marker");
            assert_eq!(
                &bytes[bytes.len() - 2..],
                &[0xFF, 0xD9],
                "frame {i}: missing JPEG EOI marker"
            );
            assert!(bytes.len() > 1000, "frame {i}: jpeg suspiciously small: {}", bytes.len());
        }
    }

    #[test]
    fn apng_export_has_actl_and_all_frames() {
        let project = Project::new_demo();
        let path = std::path::Path::new("/tmp/styk-test.apng");
        let n = export_apng(&project, path).unwrap();
        assert_eq!(n, project.frames.len());
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG file");
        // Walk the chunk list properly (type strings can also occur
        // inside compressed data, so don't just substring-search).
        let mut fctl_count = 0usize;
        let mut actl_frames: Option<u32> = None;
        let mut actl_plays: Option<u32> = None;
        let mut off = 8usize;
        loop {
            assert!(off + 8 <= bytes.len(), "truncated chunk header at {off}");
            let len = u32::from_be_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
            let ty = &bytes[off + 4..off + 8];
            match ty {
                b"acTL" => {
                    actl_frames = Some(u32::from_be_bytes(
                        bytes[off + 8..off + 12].try_into().unwrap(),
                    ));
                    actl_plays = Some(u32::from_be_bytes(
                        bytes[off + 12..off + 16].try_into().unwrap(),
                    ));
                }
                b"fcTL" => fctl_count += 1,
                b"IEND" => break,
                _ => {}
            }
            off += 12 + len;
        }
        assert_eq!(actl_frames, Some(n as u32), "acTL must declare every frame");
        assert_eq!(actl_plays, Some(0), "acTL must loop forever (num_plays = 0)");
        assert_eq!(fctl_count, n, "one fcTL chunk per animation frame");
    }
}


#[cfg(test)]
mod media_integration_tests {
    use super::*;

    #[test]
    fn media_segments_composite_into_rendered_frames() {
        // Integration guard for the wave-4 media path: a project with
        // a sprite asset on one segment must render without panicking
        // and produce a stage-sized frame whose sprite area differs
        // from the plain render.
        let mut project = Project::new_demo();
        // 4x2 solid green PNG as the asset.
        let mut png = Vec::new();
        {
            let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 200, 0, 255]));
            let mut cur = std::io::Cursor::new(&mut png);
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut cur, image::ImageFormat::Png)
                .unwrap();
        }
        project.assets.push(crate::model::ImageAsset { name: "g.png".into(), png });
        project.figures[0].def.nodes[1].media =
            Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false });
        let with = render_frame_at(&project, 0.0);
        assert_eq!((with.width(), with.height()), (960, 720));
        project.figures[0].def.nodes[1].media = None;
        let without = render_frame_at(&project, 0.0);
        let diff = with
            .pixels()
            .zip(without.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(diff > 50, "sprite should change the render ({diff} px)");
    }

    /// Wave 14 (C3) — a project whose ONLY media is clip-def
    /// media (docs/wave13/CLIP_MEDIA.md §6.6, export pin; §6.4
    /// gate): figure 0 is a movieclip instance whose own def is
    /// media-free; the image lives on the clip's def alone. The
    /// funnel's fallback (C1) yields the item, and the extended
    /// `has_media` gate must let `render_frame_at` composite it —
    /// before the gate clause this render skipped the composite
    /// entirely and the image never reached the raster. The
    /// clip's single frame pose is the demo's frame-0 pose, so
    /// the instance renders exactly where the plain figure does.
    fn clip_only_project() -> Project {
        let mut project = Project::new_demo();
        // 4x2 solid green PNG as the asset (as the test above).
        let mut png = Vec::new();
        {
            let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 200, 0, 255]));
            let mut cur = std::io::Cursor::new(&mut png);
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut cur, image::ImageFormat::Png)
                .unwrap();
        }
        project.assets.push(crate::model::ImageAsset { name: "g.png".into(), png });
        let mut clip_def = project.figures[0].def.clone();
        clip_def.nodes[1].media = Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false });
        let pose0 = project.frames[0].poses[0].clone();
        project.clips = vec![crate::model::ClipDef {
            name: "clip".into(),
            def: clip_def,
            frames: vec![pose0],
            loop_mode: crate::model::ClipLoop::Loop,
        }];
        project.figures[0].clip = Some(0);
        project
    }

    #[test]
    fn clip_def_only_media_renders_into_rendered_frames() {
        let project = clip_only_project();
        // Precondition: the project's only media is the clip
        // def's — no sprites, no texts, no figure-def media.
        assert!(project.sprites.is_empty() && project.texts.is_empty());
        assert!(project
            .figures
            .iter()
            .all(|f| f.def.nodes.iter().all(|n| n.media.is_none())));
        // The funnel yields exactly the one clip-sourced image
        // (C1's fallback), and its quad centre is the pixel the
        // composite must paint: the default camera maps stage
        // units 1:1 onto the output image.
        let items = crate::media::media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "exactly the clip-sourced image");
        let centre = match &items[0] {
            crate::media::MediaItem::Image { corners, .. } => [
                (corners[0][0] + corners[2][0]) / 2.0,
                (corners[0][1] + corners[2][1]) / 2.0,
            ],
            other => panic!("expected an image item, got {other:?}"),
        };
        let with = render_frame_at(&project, 0.0);
        assert_eq!((with.width(), with.height()), (960, 720));
        let px = with.get_pixel(centre[0] as u32, centre[1] as u32);
        assert!(
            px[1] > 150 && px[0] < 80 && px[2] < 80,
            "the media quad's centre pixel is the asset's green, got {px:?}"
        );
        // The same project with the clip-def media cleared
        // renders the plain figure there instead.
        let mut bare = project.clone();
        bare.clips[0].def.nodes[1].media = None;
        let without = render_frame_at(&bare, 0.0);
        let bare_px = without.get_pixel(centre[0] as u32, centre[1] as u32);
        assert!(
            !(bare_px[1] > 150 && bare_px[0] < 80 && bare_px[2] < 80),
            "without the clip-def media the centre pixel is not the asset's green, got {bare_px:?}"
        );
        let diff = with
            .pixels()
            .zip(without.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(diff > 50, "clip-def media should change the render ({diff} px)");
    }

    #[test]
    fn media_free_clip_defs_leave_the_render_unchanged() {
        // The §6.4 clause must not flip the gate for clip defs
        // that carry no media: a project with no media anywhere
        // renders pixel-identically whether its clip table is
        // empty, holds an unreferenced media-free clip, or holds
        // a media-free clip the figure actually instances.
        let base = Project::new_demo();
        let plain = render_frame_at(&base, 0.0);

        let mut unreferenced = base.clone();
        unreferenced.clips = vec![crate::model::ClipDef {
            name: "unused".into(),
            def: unreferenced.figures[0].def.clone(),
            frames: vec![unreferenced.frames[0].poses[0].clone()],
            loop_mode: crate::model::ClipLoop::Loop,
        }];
        assert_eq!(
            render_frame_at(&unreferenced, 0.0).as_raw(),
            plain.as_raw(),
            "an unreferenced media-free clip changes nothing"
        );

        let mut instanced = unreferenced.clone();
        instanced.figures[0].clip = Some(0);
        assert_eq!(
            render_frame_at(&instanced, 0.0).as_raw(),
            plain.as_raw(),
            "a media-free clip instance changes nothing"
        );
    }
}

#[cfg(test)]
mod weather_integration_tests {
    use super::*;

    #[test]
    fn weather_changes_the_rendered_frame() {
        // Integration guard for the wave-5 weather pass: the same
        // project rendered with and without rain must differ, and
        // the frame keeps its stage size either way.
        let mut project = Project::new_demo();
        project.weather = Some(crate::vfx::Weather {
            kind: crate::vfx::WeatherKind::Rain,
            amount: 150.0,
            wind: 40.0,
        });
        let with = render_frame_at(&project, 3.0);
        assert_eq!((with.width(), with.height()), (960, 720));
        project.weather = None;
        let without = render_frame_at(&project, 3.0);
        let diff = with
            .pixels()
            .zip(without.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(diff > 50, "rain should change the render ({diff} px)");
    }
}

#[cfg(test)]
mod trail_tests {
    use super::*;

    /// Demo project whose root marches 40 units/frame along y=300,
    /// so node 0's path is a straight, exactly known line.
    fn marching_project() -> Project {
        let mut project = Project::new_demo();
        for (i, frame) in project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [200.0 + 40.0 * i as f32, 300.0];
        }
        project
    }

    #[test]
    fn node_trail_follows_a_moving_node() {
        let project = marching_project();
        // Samples land exactly on frames 0..4 (tween is off in the
        // demo), so all four steps survive the length threshold.
        let trail = node_trail(&project, 4.0, 0, 0, 5, 4.0);
        assert_eq!(trail.len(), 4, "one segment per frame step");
        assert_eq!(trail[0].0, [200.0, 300.0], "trail starts at the oldest sample");
        assert_eq!(trail[3].1, [360.0, 300.0], "trail ends at the current position");
        assert_eq!(trail[0].2, [232, 232, 236], "trail uses the figure colour 0xE8E8EC");
        let alphas: Vec<f32> = trail.iter().map(|s| s.3).collect();
        assert_eq!(alphas, vec![0.125, 0.25, 0.375, 0.5], "alpha fades with age");
        // Deterministic: same call, same trail.
        assert_eq!(trail, node_trail(&project, 4.0, 0, 0, 5, 4.0));
    }

    #[test]
    fn node_trail_is_empty_for_a_static_node() {
        let mut project = marching_project();
        for frame in project.frames.iter_mut() {
            frame.poses[0].root = [300.0, 300.0];
        }
        assert!(
            node_trail(&project, 4.0, 0, 0, 5, 4.0).is_empty(),
            "a node that never moved leaves no trail"
        );
    }

    #[test]
    fn node_trail_rejects_bad_arguments() {
        let project = marching_project();
        assert!(node_trail(&project, 4.0, 0, 0, 1, 4.0).is_empty(), "samples < 2");
        assert!(node_trail(&project, 4.0, 0, 0, 5, 0.0).is_empty(), "span <= 0");
        assert!(node_trail(&project, 0.0, 0, 0, 5, 4.0).is_empty(), "no past at frame 0");
        assert!(node_trail(&project, 4.0, 9, 0, 5, 4.0).is_empty(), "bad figure");
        assert!(node_trail(&project, 4.0, 0, 99, 5, 4.0).is_empty(), "bad node");
    }

    #[test]
    fn trail_clip_draws_the_motion_path() {
        // Integration guard for the wave-6 trail pass: with a live
        // Trail clip on the root node, a pixel on the node's recent
        // path (well clear of the figure and of the clip's ghost
        // chain) is tinted toward the figure colour; without the
        // clip it stays background.
        let project = marching_project();
        let plain = render_frame_at(&project, 4.0);
        let bg = rgb(project.background);
        assert_eq!(
            plain.get_pixel(265, 300).0,
            bg,
            "path pixel should be background without the clip"
        );
        let mut with_clip = project.clone();
        with_clip.fx_clips.push(crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 10.0,
                kind: crate::vfx::ClipKind::Trail,
                color: 0xFF4040,
                seed: 5,
                angle: 0.0,
                length: 80.0,
                color2: None,
            },
            figure: 0,
            node: 0,
            hidden: false,
        });
        let trailed = render_frame_at(&with_clip, 4.0);
        let px = trailed.get_pixel(265, 300).0;
        assert_ne!(px, bg, "trail segment should cover the path pixel");
        let lift = |c: [u8; 3]| c[0] as u32 + c[1] as u32 + c[2] as u32;
        assert!(
            lift(px) > lift(bg),
            "trail pixel should blend toward the pale figure colour: {px:?} vs bg {bg:?}"
        );
    }
}

#[cfg(test)]
mod range_export_tests {
    use super::{
        drain_ffmpeg_progress, export_apng, export_apng_range, export_gif, export_gif_range,
        export_jpeg_sequence, export_jpeg_sequence_range,
        export_jpeg_sequence_with_render_progress,
        export_jpeg_sequence_range_scaled_with_render_progress,
        export_jpeg_sequence_scaled_with_render_progress, export_mp4, export_mp4_range,
        export_mp4_range_scaled, export_mp4_range_scaled_with_progress,
        export_mp4_range_with_encode_progress, export_mp4_range_with_progress,
        export_mp4_scaled, export_mp4_scaled_with_progress, export_mp4_with_encode_progress,
        export_mp4_with_progress, export_png_sequence, export_png_sequence_range,
        export_png_sequence_range_scaled_with_render_progress,
        export_png_sequence_range_with_render_progress,
        export_png_sequence_scaled_with_render_progress, export_png_sequence_with_render_progress,
        export_sheet_range, export_spritesheet, export_svg_range, export_webm_range,
        export_webm_range_scaled, export_webm_range_scaled_with_progress,
        export_webm_range_with_progress, export_webm_scaled, export_webm_scaled_with_progress,
        export_webm_scaled_with_encode_progress, export_webm_with_progress,
        parse_ffmpeg_progress_frame, render_frame, sampled_frame_total, BundleProgress,
        BundleProgressEvent, EncodeProgressEvent, Mp4ExportOutcome, RenderProgressEvent,
    };
    use crate::model::{AudioClip, Project};
    use image::AnimationDecoder as _;
    use std::path::{Path, PathBuf};

    /// The 5-frame demo project (all durations 1.0, tween
    /// playback off) on a small stage so renders stay fast.
    fn tiny_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [96.0, 72.0];
        project
    }

    fn temp_file(tag: &str, ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "styk-range-{tag}-{}.{ext}",
            std::process::id()
        ))
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("styk-range-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Number of frames in a written GIF, decoded with the image
    /// crate's GIF decoder.
    fn gif_frame_count(path: &Path) -> usize {
        let file = std::fs::File::open(path).unwrap();
        let decoder =
            image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        decoder.into_frames().count()
    }

    /// The frame counts inside a written APNG: the `acTL` chunk's
    /// declared frame count and the number of `fcTL` chunks,
    /// walked chunk by chunk like the `mod tests` APNG check
    /// (type strings can also occur inside compressed data, so
    /// don't just substring-search).
    fn apng_frame_counts(path: &Path) -> (u32, usize) {
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG file");
        let mut actl_frames: Option<u32> = None;
        let mut fctl_count = 0usize;
        let mut off = 8usize;
        loop {
            assert!(off + 8 <= bytes.len(), "truncated chunk header at {off}");
            let len = u32::from_be_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
            let ty = &bytes[off + 4..off + 8];
            match ty {
                b"acTL" => {
                    actl_frames = Some(u32::from_be_bytes(
                        bytes[off + 8..off + 12].try_into().unwrap(),
                    ));
                }
                b"fcTL" => fctl_count += 1,
                b"IEND" => break,
                _ => {}
            }
            off += 12 + len;
        }
        (actl_frames.expect("APNG has no acTL chunk"), fctl_count)
    }

    #[test]
    fn gif_range_writes_exactly_the_sub_range_frames() {
        let project = tiny_project();
        assert_eq!(project.frames.len(), 5);
        let path = temp_file("gif-sub", "gif");
        let n = export_gif_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 3, "frames 1..=3 of 5 must write 3 GIF frames");
        assert_eq!(gif_frame_count(&path), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gif_range_clamps_to_the_timeline() {
        let project = tiny_project();
        let path = temp_file("gif-clamp", "gif");
        let n = export_gif_range(&project, 3, 999, &path).unwrap();
        assert_eq!(n, 2, "3..=999 on 5 frames clamps to frames 3..=4");
        assert_eq!(gif_frame_count(&path), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gif_range_keeps_each_frames_own_duration() {
        let mut project = tiny_project();
        project.frames[2].duration = 2.0; // frame 2 is held twice as long
        let path = temp_file("gif-dur", "gif");
        let n = export_gif_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 4, "frames 1, 2 (held x2), 3 expand to 4 GIF frames");
        assert_eq!(gif_frame_count(&path), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gif_range_rejects_empty_and_inverted_ranges() {
        let project = tiny_project();
        let path = temp_file("gif-err", "gif");
        assert!(export_gif_range(&project, 3, 1, &path).is_err(), "inverted range");
        assert!(
            export_gif_range(&project, 99, 2, &path).is_err(),
            "99..=2 clamps to 4..=2, still empty"
        );
    }

    #[test]
    fn gif_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_path = temp_file("gif-full", "gif");
        let range_path = temp_file("gif-fullrange", "gif");
        let n_full = export_gif(&project, &full_path).unwrap();
        let n_range =
            export_gif_range(&project, 0, project.frames.len() - 1, &range_path).unwrap();
        assert_eq!(n_full, n_range);
        assert_eq!(gif_frame_count(&full_path), gif_frame_count(&range_path));
        let _ = std::fs::remove_file(&full_path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn png_range_writes_exactly_the_clamped_file_count() {
        let project = tiny_project();
        let dir = temp_dir("png-range");
        let n = export_png_sequence_range(&project, 2, 99, &dir).unwrap();
        assert_eq!(n, 3, "2..=99 on 5 frames clamps to frames 2..=4");
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        // File names keep the authored indices, as in a full export.
        let expected: Vec<String> = ["frame-0003.png", "frame-0004.png", "frame-0005.png"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(names, expected);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_dir = temp_dir("png-full");
        let range_dir = temp_dir("png-fullrange");
        let n_full = export_png_sequence(&project, &full_dir).unwrap();
        let n_range =
            export_png_sequence_range(&project, 0, project.frames.len() - 1, &range_dir)
                .unwrap();
        assert_eq!(n_full, 5);
        assert_eq!(n_full, n_range);
        assert_eq!(std::fs::read_dir(&full_dir).unwrap().count(), 5);
        assert_eq!(std::fs::read_dir(&range_dir).unwrap().count(), 5);
        let _ = std::fs::remove_dir_all(&full_dir);
        let _ = std::fs::remove_dir_all(&range_dir);
    }

    #[test]
    fn png_range_rejects_inverted_ranges() {
        let project = tiny_project();
        let dir = temp_dir("png-err");
        assert!(export_png_sequence_range(&project, 4, 1, &dir).is_err());
    }

    #[test]
    fn mp4_range_writes_a_nonempty_video_file() {
        let project = tiny_project();
        let path = temp_file("mp4-sub", "mp4");
        export_mp4_range(&project, 1, 3, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "mp4 range export is empty");
        assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mp4_export_names_missing_audio_instead_of_dropping_it_silently() {
        // Wave P (P15): a full export whose only clip's
        // file is missing succeeds — the video is
        // complete — and the outcome names the clip the
        // mux plan had to skip, where the old bare frame
        // count dropped it without a trace.
        let mut project = tiny_project();
        project.audio = vec![AudioClip {
            path: "/nonexistent/styk-export-gone.wav".to_string(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        }];
        let path = temp_file("mp4-missing-audio", "mp4");
        let outcome = export_mp4(&project, &path).unwrap();
        assert_eq!(
            outcome.missing_audio,
            vec!["/nonexistent/styk-export-gone.wav".to_string()]
        );
        let note = outcome.missing_audio_note().expect("a skipped clip is named");
        assert!(note.contains("styk-export-gone.wav"), "{note}");
        assert!(outcome.frames > 0, "the video itself is complete");
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mp4_export_with_present_audio_reports_no_missing() {
        // The same export with its clip's file present
        // (a real WAV, muxed in) carries no warning: the
        // outcome reports exactly as it did before the
        // field existed.
        let wav = std::env::temp_dir().join(format!(
            "styk-export-present-{}.wav",
            std::process::id()
        ));
        let data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0; 8000],
        };
        std::fs::write(&wav, crate::audio_edit::write_wav(&data)).unwrap();
        let mut project = tiny_project();
        project.audio = vec![AudioClip {
            path: wav.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        }];
        let path = temp_file("mp4-present-audio", "mp4");
        let outcome = export_mp4(&project, &path).unwrap();
        assert!(outcome.missing_audio.is_empty());
        assert_eq!(outcome.missing_audio_note(), None);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&wav);
    }

    #[test]
    fn mp4_outcome_note_names_every_skipped_clip() {
        // The note is the status-line sentence the entry
        // points append: absent when nothing was skipped,
        // naming every skipped clip when some were.
        let quiet = Mp4ExportOutcome {
            frames: 3,
            missing_audio: Vec::new(),
        };
        assert_eq!(quiet.missing_audio_note(), None);
        let skipped = Mp4ExportOutcome {
            frames: 3,
            missing_audio: vec!["a.wav".to_string(), "b.wav".to_string()],
        };
        let note = skipped.missing_audio_note().unwrap();
        assert!(note.contains("a.wav"), "{note}");
        assert!(note.contains("b.wav"), "{note}");
    }

    #[test]
    fn webm_range_writes_a_nonempty_video_file() {
        let project = tiny_project();
        let path = temp_file("webm-sub", "webm");
        export_webm_range(&project, 1, 3, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "webm range export is empty");
        assert_eq!(
            &bytes[0..4],
            &[0x1A, 0x45, 0xDF, 0xA3],
            "not an EBML/WebM file"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mp4_range_scaled_writes_a_valid_container() {
        // Wave S (S5): the scaled range entry renders its
        // frames through the scaled funnel — a real MP4 over
        // exactly the range's frames (2.0 on the 96x72 stage
        // is 192x144).
        let project = tiny_project();
        let path = temp_file("mp4-scaled", "mp4");
        let outcome = export_mp4_range_scaled(&project, 1, 3, &path, 2.0).unwrap();
        assert_eq!(outcome.frames, 3);
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "scaled mp4 range export is empty");
        assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn webm_scaled_writes_a_valid_container() {
        // Wave S (S5): the scaled full entry at 0.5 (48x36 on
        // the tiny stage) writes a real WebM over the whole
        // timeline.
        let project = tiny_project();
        let path = temp_file("webm-scaled", "webm");
        let frames = export_webm_scaled(&project, &path, 0.5).unwrap();
        assert_eq!(frames, project.frames.len());
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "scaled webm export is empty");
        assert_eq!(
            &bytes[0..4],
            &[0x1A, 0x45, 0xDF, 0xA3],
            "not an EBML/WebM file"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn video_scaled_at_1_0_reports_the_unscaled_outcome() {
        // Wave S (S5): the 1.0 route-through — the scaled
        // entry at 1.0 IS the unscaled entry, so it reports
        // exactly the unscaled outcome and writes a real file.
        let project = tiny_project();
        let plain = temp_file("mp4-plain", "mp4");
        let scaled = temp_file("mp4-scaled-1", "mp4");
        let a = export_mp4(&project, &plain).unwrap();
        let b = export_mp4_scaled(&project, &scaled, 1.0).unwrap();
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.missing_audio, b.missing_audio);
        assert!(std::fs::metadata(&scaled).unwrap().len() > 0);
        let _ = std::fs::remove_file(&plain);
        let _ = std::fs::remove_file(&scaled);
    }

    #[test]
    fn video_scaled_refuses_invalid_scales_before_writing() {
        // Wave S (S5): the shared scale validation fires
        // before any encode — nothing is written for a
        // refused scale, full or range, MP4 or WebM.
        let project = tiny_project();
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mp4_path = temp_file("mp4-badscale", "mp4");
            let _ = std::fs::remove_file(&mp4_path);
            let err = export_mp4_scaled(&project, &mp4_path, bad).unwrap_err();
            assert!(
                err.to_string().contains("positive finite"),
                "scale {bad}: {err}"
            );
            assert!(!mp4_path.exists(), "scale {bad} wrote an MP4 file");
            let webm_path = temp_file("webm-badscale", "webm");
            let _ = std::fs::remove_file(&webm_path);
            let err = export_webm_range_scaled(&project, 1, 3, &webm_path, bad).unwrap_err();
            assert!(
                err.to_string().contains("positive finite"),
                "scale {bad}: {err}"
            );
            assert!(!webm_path.exists(), "scale {bad} wrote a WebM file");
        }
    }

    #[test]
    fn video_range_rejects_empty_and_inverted_ranges() {
        let project = tiny_project();
        let mp4_path = temp_file("mp4-err", "mp4");
        let webm_path = temp_file("webm-err", "webm");
        assert!(export_mp4_range(&project, 3, 1, &mp4_path).is_err(), "inverted range");
        assert!(
            export_mp4_range(&project, 99, 2, &mp4_path).is_err(),
            "99..=2 clamps to 4..=2, still empty"
        );
        assert!(export_webm_range(&project, 3, 1, &webm_path).is_err(), "inverted range");
        assert!(
            export_webm_range(&project, 99, 2, &webm_path).is_err(),
            "99..=2 clamps to 4..=2, still empty"
        );
    }

    #[test]
    fn video_full_range_exports_succeed() {
        let project = tiny_project();
        let last = project.frames.len() - 1;
        let mp4_path = temp_file("mp4-fullrange", "mp4");
        export_mp4_range(&project, 0, last, &mp4_path).unwrap();
        assert!(std::fs::metadata(&mp4_path).unwrap().len() > 0);
        let webm_path = temp_file("webm-fullrange", "webm");
        export_webm_range(&project, 0, last, &webm_path).unwrap();
        assert!(std::fs::metadata(&webm_path).unwrap().len() > 0);
        let _ = std::fs::remove_file(&mp4_path);
        let _ = std::fs::remove_file(&webm_path);
    }

    /// The delivery contract every hooked encode must keep
    /// (Wave U, U11): intermediate events unfinished,
    /// strictly increasing and in range; exactly one
    /// terminal event, last, at (total, total). How many
    /// intermediate events a given encode produces is
    /// ffmpeg's wall-clock business and is NOT pinned.
    fn assert_encode_progress_contract(events: &[EncodeProgressEvent], total: usize) {
        let (terminal, intermediate) = events
            .split_last()
            .expect("a successful hooked encode reports at least its terminal event");
        assert_eq!(
            *terminal,
            EncodeProgressEvent {
                encoded_frames: total,
                total_frames: total,
                finished: true,
            },
            "the terminal event closes the encode at (total, total)"
        );
        assert_eq!(
            events.iter().filter(|e| e.finished).count(),
            1,
            "exactly one terminal event"
        );
        let mut last = 0usize;
        for event in intermediate {
            assert!(!event.finished, "no early terminal event: {event:?}");
            assert_eq!(event.total_frames, total);
            assert!(
                event.encoded_frames > last && event.encoded_frames <= total,
                "intermediate events strictly increase within the total: {event:?}"
            );
            last = event.encoded_frames;
        }
    }

    /// The exact event stream a successful hooked sequence
    /// export of `total` frames must deliver (Wave V, V12):
    /// one event per written frame, unfinished until the
    /// final frame's event, which is the single terminal at
    /// (total, total). Delivery is synchronous, so — unlike
    /// the encode contract above, whose intermediate count
    /// is wall-clock business — the FULL stream is pinned.
    fn expected_render_events(total: usize) -> Vec<RenderProgressEvent> {
        (1..=total)
            .map(|done| RenderProgressEvent {
                rendered_frames: done,
                total_frames: total,
                finished: done == total,
            })
            .collect()
    }

    /// A render-progress sink in the U11 idiom: the shared
    /// event vector plus the callback that fills it.
    fn render_event_sink()
    -> (
        std::sync::Arc<std::sync::Mutex<Vec<RenderProgressEvent>>>,
        impl Fn(RenderProgressEvent),
    ) {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: RenderProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        (events, callback)
    }

    /// An encode-progress sink in the same idiom as
    /// [`render_event_sink`]: the shared event vector plus
    /// the callback that fills it (Wave W, W2 — the
    /// dual-hook pins record both phases at once).
    fn encode_event_sink()
    -> (
        std::sync::Arc<std::sync::Mutex<Vec<EncodeProgressEvent>>>,
        impl Fn(EncodeProgressEvent),
    ) {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: EncodeProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        (events, callback)
    }

    #[test]
    fn png_sequence_range_render_progress_fires_per_frame_and_terminates_once() {
        // Wave V (V12): frames 1..=3 of the 5-frame fixture —
        // authored frames, so the stream is exact and no fps
        // is involved. Files keep their authored indices.
        let project = tiny_project();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-range");
        let n = export_png_sequence_range_with_render_progress(
            &project,
            1,
            3,
            &dir,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, 3);
        assert_eq!(*events.lock().unwrap(), expected_render_events(3));
        for name in ["frame-0002.png", "frame-0003.png", "frame-0004.png"] {
            assert!(dir.join(name).is_file(), "missing {name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_sequence_render_progress_covers_the_full_timeline() {
        let project = tiny_project();
        let total = project.frames.len();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-full");
        let n = export_png_sequence_with_render_progress(&project, &dir, Some(&callback))
            .unwrap();
        assert_eq!(n, total);
        assert_eq!(*events.lock().unwrap(), expected_render_events(total));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_sequence_scaled_render_progress_reports_authored_frames() {
        // The scaled funnel reports the same authored-frame
        // counts — scale changes the rasters, never the
        // stream. The full scaled entry's empty/range
        // plumbing rides the same hooked range entry.
        let project = tiny_project();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-scaled");
        let n = export_png_sequence_range_scaled_with_render_progress(
            &project,
            0,
            1,
            &dir,
            2.0,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, 2);
        assert_eq!(*events.lock().unwrap(), expected_render_events(2));
        let _ = std::fs::remove_dir_all(&dir);

        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-scaled-full");
        let n = export_png_sequence_scaled_with_render_progress(
            &project,
            &dir,
            0.5,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, project.frames.len());
        assert_eq!(
            *events.lock().unwrap(),
            expected_render_events(project.frames.len())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jpeg_sequence_render_progress_survives_the_scale_one_route_through() {
        // At exactly 1.0 the scaled entries route through
        // the unscaled hooked entries — the hook must reach
        // the render loop, not be dropped at the route.
        let project = tiny_project();
        let total = project.frames.len();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("jpeg-progress-one");
        let n = export_jpeg_sequence_scaled_with_render_progress(
            &project,
            &dir,
            1.0,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, total);
        assert_eq!(*events.lock().unwrap(), expected_render_events(total));
        assert!(dir.join("frame-0001.jpg").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jpeg_sequence_range_scaled_render_progress_reports_authored_frames() {
        let project = tiny_project();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("jpeg-progress-scaled");
        let n = export_jpeg_sequence_range_scaled_with_render_progress(
            &project,
            2,
            4,
            &dir,
            2.0,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, 3);
        assert_eq!(*events.lock().unwrap(), expected_render_events(3));
        assert!(dir.join("frame-0005.jpg").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_project_render_progress_fires_only_the_terminal() {
        // The Ok(0) success: exactly one event — the
        // terminal at (0, 0) — after the directory is
        // created, on both the unscaled and scaled entries.
        let mut project = tiny_project();
        project.frames.clear();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-empty");
        let n = export_png_sequence_with_render_progress(&project, &dir, Some(&callback))
            .unwrap();
        assert_eq!(n, 0);
        assert!(dir.is_dir());
        assert_eq!(
            *events.lock().unwrap(),
            vec![RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);

        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-empty-scaled");
        let n = export_png_sequence_scaled_with_render_progress(
            &project,
            &dir,
            2.0,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, 0);
        assert_eq!(
            *events.lock().unwrap(),
            vec![RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refused_sequence_render_progress_fires_nothing() {
        // Refusals keep the plain entries' contract exactly:
        // an invalid scale is refused before anything is
        // written or reported (no directory either), and an
        // inverted range errors in the clamp with no event.
        let project = tiny_project();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-refused-scale");
        let refused = export_png_sequence_scaled_with_render_progress(
            &project,
            &dir,
            0.0,
            Some(&callback),
        );
        assert!(refused.is_err());
        assert!(events.lock().unwrap().is_empty());
        assert!(!dir.exists(), "a refused call creates no directory");
        let _ = std::fs::remove_dir_all(&dir);

        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-refused-range");
        let refused = export_png_sequence_range_with_render_progress(
            &project,
            3,
            1,
            &dir,
            Some(&callback),
        );
        assert!(refused.is_err());
        assert!(events.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_frame_write_reports_no_terminal_render_event() {
        // Frame 2's target path is a directory, so its save
        // fails after frame 1 was written and reported: the
        // error propagates in the historical voice, the one
        // true event stays delivered, and no terminal fires.
        let project = tiny_project();
        let (events, callback) = render_event_sink();
        let dir = temp_dir("png-progress-failure");
        std::fs::create_dir_all(dir.join("frame-0002.png")).unwrap();
        let failed = export_png_sequence_range_with_render_progress(
            &project,
            0,
            2,
            &dir,
            Some(&callback),
        );
        assert!(failed.is_err());
        assert_eq!(
            *events.lock().unwrap(),
            vec![RenderProgressEvent {
                rendered_frames: 1,
                total_frames: 3,
                finished: false,
            }]
        );
        assert!(dir.join("frame-0001.png").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn panicking_render_observer_cannot_fail_the_export() {
        // The containment mirrors the encode hook's: every
        // event is attempted, every panic is dropped at the
        // call site, and the export completes in full.
        let project = tiny_project();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = attempts.clone();
        let callback = move |_event: RenderProgressEvent| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("observer exploded");
        };
        let dir = temp_dir("png-progress-panic");
        let n = export_png_sequence_range_with_render_progress(
            &project,
            0,
            2,
            &dir,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(n, 3);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
        for name in ["frame-0001.png", "frame-0002.png", "frame-0003.png"] {
            assert!(dir.join(name).is_file(), "missing {name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ffmpeg_progress_parser_reads_only_the_frame_count() {
        // Wave U (U11): ffmpeg's -progress stream is a
        // key=value block format; the frame count is the one
        // lawful signal. Whitespace inside the value is
        // ffmpeg's own padding; N/A and every other key —
        // measured, version-specific or future — read as
        // "no frame on this line", never as an error.
        assert_eq!(parse_ffmpeg_progress_frame("frame=  42"), Some(42));
        assert_eq!(parse_ffmpeg_progress_frame("frame=0"), Some(0));
        assert_eq!(parse_ffmpeg_progress_frame("frame=N/A"), None);
        assert_eq!(parse_ffmpeg_progress_frame("fps=12.5"), None);
        assert_eq!(parse_ffmpeg_progress_frame("out_time_ms=7933333"), None);
        assert_eq!(parse_ffmpeg_progress_frame("progress=end"), None);
        assert_eq!(parse_ffmpeg_progress_frame("stream_0_0_q=-1.0"), None);
    }

    #[test]
    fn ffmpeg_progress_drain_reports_only_advancing_counts() {
        // A captured-shape stream: the opening frame=0 is
        // suppressed, duplicates and regressions drop,
        // counts clamp at the total, unknown / N/A lines are
        // skipped — and the drainer itself never sends the
        // terminal event (it cannot know the exit status).
        let stream = "\
frame=0
fps=0.00
bitrate=N/A
progress=continue
frame=2
progress=continue
frame=2
frame=99
frame=3
progress=end
";
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: EncodeProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        drain_ffmpeg_progress(
            std::io::Cursor::new(stream.as_bytes().to_vec()),
            5,
            Some(&callback),
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                EncodeProgressEvent {
                    encoded_frames: 2,
                    total_frames: 5,
                    finished: false,
                },
                EncodeProgressEvent {
                    encoded_frames: 5,
                    total_frames: 5,
                    finished: false,
                },
            ]
        );
    }

    #[test]
    fn mp4_range_encode_progress_fires_and_terminates_once() {
        // Wave U (U11): the hooked range encode on a
        // 24 fps fixture — frames 1..=3 sample to exactly 3
        // encoder frames (durations 1.0, tween playback off).
        let mut project = tiny_project();
        project.fps = 24.0;
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: EncodeProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        let path = temp_file("mp4-progress", "mp4");
        let outcome = export_mp4_range_with_encode_progress(
            &project,
            1,
            3,
            &path,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(outcome.frames, 3);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
        assert_encode_progress_contract(&events.lock().unwrap(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn webm_scaled_encode_progress_fires_and_terminates_once() {
        // The scaled funnel carries the same hook: the full
        // 5-frame timeline at 0.5 on the tiny stage.
        let mut project = tiny_project();
        project.fps = 24.0;
        let total = project.frames.len();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: EncodeProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        let path = temp_file("webm-progress", "webm");
        let frames = export_webm_scaled_with_encode_progress(
            &project,
            &path,
            0.5,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(frames, total);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[0..4],
            &[0x1A, 0x45, 0xDF, 0xA3],
            "not an EBML/WebM file"
        );
        assert_encode_progress_contract(&events.lock().unwrap(), total);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn failed_encode_reports_no_terminal_progress_event() {
        // An output extension ffmpeg has no format for makes
        // the encode exit non-zero: whatever intermediate
        // counts were reported, none may claim `finished`.
        let mut project = tiny_project();
        project.fps = 24.0;
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: EncodeProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        let path = temp_file("mp4-progress-bad", "xyz");
        let _ = std::fs::remove_file(&path);
        let err = export_mp4_with_encode_progress(&project, &path, Some(&callback))
            .unwrap_err();
        assert!(
            err.to_string().contains("ffmpeg exited"),
            "the failure is ffmpeg's exit, in the usual voice: {err}"
        );
        assert!(
            events.lock().unwrap().iter().all(|e| !e.finished),
            "a failed encode never reports a terminal event"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A bundle-progress sink in the U11 idiom: the
    /// shared event vector plus the callback that fills
    /// it (Wave X, X11).
    fn bundle_event_sink() -> (
        std::sync::Arc<std::sync::Mutex<Vec<BundleProgressEvent>>>,
        impl Fn(BundleProgressEvent),
    ) {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: BundleProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        (events, callback)
    }

    /// Shorthand for one expected bundle event.
    fn bundle_event(completed: usize, total: usize, finished: bool) -> BundleProgressEvent {
        BundleProgressEvent {
            completed_frames: completed,
            total_frames: total,
            finished,
        }
    }

    #[test]
    fn sampled_frame_total_counts_sampled_times_not_authored_frames() {
        // Wave X (X11): the video bundle members' share of
        // a bundle total is the encode contract's total —
        // sampled times, so a held frame counts twice.
        let mut project = tiny_project();
        assert_eq!(sampled_frame_total(&project), 5);
        project.frames[2].duration = 2.0;
        assert_eq!(sampled_frame_total(&project), 6);
    }

    #[test]
    fn bundle_progress_translates_member_streams_into_one_monotonic_count() {
        // Wave X (X11): two members ([3, 2]) under one
        // tracker — the second member's counts CONTINUE
        // the count instead of resetting to 1, both
        // translator kinds feed the same stream, a
        // member's own terminal translates as an
        // unfinished bundle event, and the only finished
        // event is finish()'s terminal at (total, total).
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[3, 2], Some(&callback));
        assert_eq!(tracker.total_frames(), 5);
        let render = tracker.render_callback(0);
        for event in expected_render_events(3) {
            render(event);
        }
        tracker.complete_member(true);
        let encode = tracker.encode_callback(1);
        encode(EncodeProgressEvent {
            encoded_frames: 1,
            total_frames: 2,
            finished: false,
        });
        encode(EncodeProgressEvent {
            encoded_frames: 2,
            total_frames: 2,
            finished: true,
        });
        tracker.complete_member(true);
        tracker.finish();
        tracker.finish();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                bundle_event(1, 5, false),
                bundle_event(2, 5, false),
                bundle_event(3, 5, false),
                bundle_event(4, 5, false),
                bundle_event(5, 5, false),
                bundle_event(5, 5, true),
            ],
            "one monotonic stream, exactly one terminal, last"
        );
    }

    #[test]
    fn bundle_progress_failed_member_banks_only_its_peak_and_forfeits_the_terminal() {
        // Wave X (X11): member 0 reports 2 of its 3 frames
        // and fails. Its delivered counts are never
        // retracted — member 1 continues from the banked
        // peak (2), so the stream never runs backwards —
        // and finish() fires no terminal, mirroring the
        // member contracts' failure rule.
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[3, 2], Some(&callback));
        let render = tracker.render_callback(0);
        render(RenderProgressEvent {
            rendered_frames: 1,
            total_frames: 3,
            finished: false,
        });
        render(RenderProgressEvent {
            rendered_frames: 2,
            total_frames: 3,
            finished: false,
        });
        tracker.complete_member(false);
        let render = tracker.render_callback(1);
        for event in expected_render_events(2) {
            render(event);
        }
        tracker.complete_member(true);
        tracker.finish();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                bundle_event(1, 5, false),
                bundle_event(2, 5, false),
                bundle_event(3, 5, false),
                bundle_event(4, 5, false),
            ],
            "no terminal after a member failure; counts continue from the peak"
        );
    }

    #[test]
    fn bundle_progress_silent_member_banks_atomically_at_completion() {
        // Wave X (X11): a member with no phase contract
        // (the bundle's SVG / spritesheet members) takes
        // no translator — its frames land in the count
        // when it completes, so the next member's events
        // continue past them and the terminal still
        // closes at (total, total). Nothing is emitted
        // for the silent member itself: banking is not
        // an event, only member reports and the terminal
        // are.
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[2, 2], Some(&callback));
        tracker.complete_member(true);
        let render = tracker.render_callback(1);
        for event in expected_render_events(2) {
            render(event);
        }
        tracker.complete_member(true);
        tracker.finish();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                bundle_event(3, 4, false),
                bundle_event(4, 4, false),
                bundle_event(4, 4, true),
            ]
        );
    }

    #[test]
    fn bundle_progress_clamps_member_overreports_and_drops_regressions() {
        // Wave X (X11): a member event past the member's
        // allocation clamps to its share (the bundle
        // count can never pass the member's boundary),
        // and a regressive event is dropped outright.
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[3], Some(&callback));
        let render = tracker.render_callback(0);
        render(RenderProgressEvent {
            rendered_frames: 2,
            total_frames: 3,
            finished: false,
        });
        render(RenderProgressEvent {
            rendered_frames: 1,
            total_frames: 3,
            finished: false,
        });
        render(RenderProgressEvent {
            rendered_frames: 9,
            total_frames: 3,
            finished: true,
        });
        tracker.complete_member(true);
        tracker.finish();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                bundle_event(2, 3, false),
                bundle_event(3, 3, false),
                bundle_event(3, 3, true),
            ]
        );
    }

    #[test]
    fn bundle_progress_zero_members_terminates_at_zero() {
        // Wave X (X11): the empty bundle is the sequence
        // contract's empty-project leg one level up —
        // finish() alone fires exactly the (0, 0)
        // terminal, once.
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[], Some(&callback));
        tracker.finish();
        tracker.finish();
        assert_eq!(*events.lock().unwrap(), vec![bundle_event(0, 0, true)]);
    }

    #[test]
    fn bundle_progress_panicking_observer_cannot_break_the_bundle() {
        // Wave X (X11): the reporter contains observer
        // panics per event (the phase contracts' rule) —
        // every tracker call returns normally and the
        // aggregation state stays usable throughout.
        let callback = |_event: BundleProgressEvent| panic!("observer boom");
        let tracker = BundleProgress::new(&[2, 1], Some(&callback));
        let render = tracker.render_callback(0);
        render(RenderProgressEvent {
            rendered_frames: 1,
            total_frames: 2,
            finished: false,
        });
        render(RenderProgressEvent {
            rendered_frames: 2,
            total_frames: 2,
            finished: true,
        });
        tracker.complete_member(true);
        let render = tracker.render_callback(1);
        render(RenderProgressEvent {
            rendered_frames: 1,
            total_frames: 1,
            finished: true,
        });
        tracker.complete_member(true);
        tracker.finish();
        assert_eq!(tracker.total_frames(), 3);
    }

    #[test]
    fn bundle_progress_over_real_sequence_members_is_exact() {
        // Wave X (X11): the tracker driven the way the
        // export_bundle handler drives it — each member's
        // hooked entry under its translator,
        // complete_member between members, finish() at
        // the end. Render delivery is synchronous, so the
        // full bundle stream is pinned exactly: the two
        // members' counts continue across the boundary
        // instead of resetting, and the only finished
        // event is finish()'s terminal.
        let project = tiny_project();
        let n = project.frames.len();
        let (events, callback) = bundle_event_sink();
        let tracker = BundleProgress::new(&[n, n], Some(&callback));
        let png_dir = temp_dir("bundle-progress-png");
        let member = tracker.render_callback(0);
        let count =
            export_png_sequence_with_render_progress(&project, &png_dir, Some(&member))
                .unwrap();
        assert_eq!(count, n);
        tracker.complete_member(true);
        let jpeg_dir = temp_dir("bundle-progress-jpeg");
        let member = tracker.render_callback(1);
        let count =
            export_jpeg_sequence_with_render_progress(&project, &jpeg_dir, Some(&member))
                .unwrap();
        assert_eq!(count, n);
        tracker.complete_member(true);
        tracker.finish();
        let mut expected: Vec<BundleProgressEvent> = (1..=2 * n)
            .map(|done| bundle_event(done, 2 * n, false))
            .collect();
        expected.push(bundle_event(2 * n, 2 * n, true));
        assert_eq!(*events.lock().unwrap(), expected);
        let _ = std::fs::remove_dir_all(&png_dir);
        let _ = std::fs::remove_dir_all(&jpeg_dir);
    }

    #[test]
    fn mp4_range_dual_hooks_report_their_own_phases() {
        // Wave W (W2): the range entry with both hooks —
        // frames 1..=3 at 24 fps sample to exactly 3 frames.
        // The render stream is synchronous, so it is pinned
        // in full; the encode stream keeps U11's contract
        // shape (its intermediate count is wall-clock). The
        // two vectors fill through separate callbacks, so a
        // count can never land in the wrong phase's stream.
        let mut project = tiny_project();
        project.fps = 24.0;
        let (render_events, render_callback) = render_event_sink();
        let (encode_events, encode_callback) = encode_event_sink();
        let path = temp_file("mp4-dual-progress", "mp4");
        let outcome = export_mp4_range_with_progress(
            &project,
            1,
            3,
            &path,
            Some(&encode_callback),
            Some(&render_callback),
        )
        .unwrap();
        assert_eq!(outcome.frames, 3);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
        assert_eq!(*render_events.lock().unwrap(), expected_render_events(3));
        assert_encode_progress_contract(&encode_events.lock().unwrap(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mp4_scaled_dual_hooks_survive_the_scale_one_route_through() {
        // At exactly 1.0 the scaled entry routes through
        // the unscaled dual-hook entry — both hooks must
        // reach the funnel, not be dropped at the route.
        let mut project = tiny_project();
        project.fps = 24.0;
        let total = project.frames.len();
        let (render_events, render_callback) = render_event_sink();
        let (encode_events, encode_callback) = encode_event_sink();
        let path = temp_file("mp4-dual-one", "mp4");
        let outcome = export_mp4_scaled_with_progress(
            &project,
            &path,
            1.0,
            Some(&encode_callback),
            Some(&render_callback),
        )
        .unwrap();
        assert_eq!(outcome.frames, total);
        assert_eq!(
            *render_events.lock().unwrap(),
            expected_render_events(total)
        );
        assert_encode_progress_contract(&encode_events.lock().unwrap(), total);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn webm_range_scaled_dual_hooks_report_sampled_frames() {
        // The scaled funnel's render loop reports the
        // sampled frame count (3 for frames 1..=3), exactly
        // like the unscaled one — scale changes the
        // rasters, never the stream.
        let mut project = tiny_project();
        project.fps = 24.0;
        let (render_events, render_callback) = render_event_sink();
        let (encode_events, encode_callback) = encode_event_sink();
        let path = temp_file("webm-dual-scaled", "webm");
        export_webm_range_scaled_with_progress(
            &project,
            1,
            3,
            &path,
            2.0,
            Some(&encode_callback),
            Some(&render_callback),
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[0..4],
            &[0x1A, 0x45, 0xDF, 0xA3],
            "not an EBML/WebM file"
        );
        assert_eq!(*render_events.lock().unwrap(), expected_render_events(3));
        assert_encode_progress_contract(&encode_events.lock().unwrap(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn failed_encode_still_reports_the_completed_render_phase() {
        // The render terminal closes the render phase, not
        // the export: when ffmpeg then refuses the output
        // (an extension it has no format for), the export
        // fails in the usual voice, the render stream
        // stands complete, and no encode event claims
        // `finished`. The mirror image of U11's
        // failed-encode pin, on the phase boundary.
        let mut project = tiny_project();
        project.fps = 24.0;
        let total = project.frames.len();
        let (render_events, render_callback) = render_event_sink();
        let (encode_events, encode_callback) = encode_event_sink();
        let path = temp_file("mp4-dual-bad", "xyz");
        let _ = std::fs::remove_file(&path);
        let err = export_mp4_with_progress(
            &project,
            &path,
            Some(&encode_callback),
            Some(&render_callback),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("ffmpeg exited"),
            "the failure is ffmpeg's exit, in the usual voice: {err}"
        );
        assert_eq!(
            *render_events.lock().unwrap(),
            expected_render_events(total),
            "the render phase completed and terminated before the encode ran"
        );
        assert!(
            encode_events.lock().unwrap().iter().all(|e| !e.finished),
            "a failed encode never reports a terminal event"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn refused_video_range_fires_no_event_on_either_hook() {
        // An inverted range is refused by the clamp before
        // any frame is rendered: both hooks stay silent,
        // exactly as the plain entries behave.
        let mut project = tiny_project();
        project.fps = 24.0;
        let (render_events, render_callback) = render_event_sink();
        let (encode_events, encode_callback) = encode_event_sink();
        let path = temp_file("mp4-dual-refused", "mp4");
        let refused = export_mp4_range_with_progress(
            &project,
            3,
            1,
            &path,
            Some(&encode_callback),
            Some(&render_callback),
        );
        assert!(refused.is_err(), "inverted range");
        assert!(render_events.lock().unwrap().is_empty());
        assert!(encode_events.lock().unwrap().is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn panicking_render_observer_cannot_fail_the_video_export() {
        // The containment is the shared `report_render_progress`
        // one: every event is attempted, every panic is
        // dropped at the call site, and the export completes
        // in full.
        let mut project = tiny_project();
        project.fps = 24.0;
        let total = project.frames.len();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = attempts.clone();
        let callback = move |_event: RenderProgressEvent| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("observer exploded");
        };
        let path = temp_file("webm-dual-panic", "webm");
        let frames = export_webm_with_progress(&project, &path, None, Some(&callback))
            .unwrap();
        assert_eq!(frames, total);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), total);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apng_range_writes_exactly_the_sub_range_frames() {
        let project = tiny_project();
        let path = temp_file("apng-sub", "png");
        let n = export_apng_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 3, "frames 1..=3 of 5 must write 3 APNG frames");
        let (actl, fctl) = apng_frame_counts(&path);
        assert_eq!(actl, 3, "acTL must declare the sub-range's frames");
        assert_eq!(fctl, 3, "one fcTL chunk per animation frame");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apng_range_clamps_to_the_timeline() {
        let project = tiny_project();
        let path = temp_file("apng-clamp", "png");
        let n = export_apng_range(&project, 3, 999, &path).unwrap();
        assert_eq!(n, 2, "3..=999 on 5 frames clamps to frames 3..=4");
        assert_eq!(apng_frame_counts(&path), (2, 2));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apng_range_keeps_each_frames_own_duration() {
        let mut project = tiny_project();
        project.frames[2].duration = 2.0; // frame 2 is held twice as long
        let path = temp_file("apng-dur", "png");
        let n = export_apng_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 4, "frames 1, 2 (held x2), 3 expand to 4 APNG frames");
        assert_eq!(apng_frame_counts(&path), (4, 4));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apng_range_rejects_empty_and_inverted_ranges() {
        let project = tiny_project();
        let path = temp_file("apng-err", "png");
        assert!(export_apng_range(&project, 3, 1, &path).is_err(), "inverted range");
        assert!(
            export_apng_range(&project, 99, 2, &path).is_err(),
            "99..=2 clamps to 4..=2, still empty"
        );
    }

    #[test]
    fn apng_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_path = temp_file("apng-full", "png");
        let range_path = temp_file("apng-fullrange", "png");
        let n_full = export_apng(&project, &full_path).unwrap();
        let n_range =
            export_apng_range(&project, 0, project.frames.len() - 1, &range_path).unwrap();
        assert_eq!(n_full, n_range);
        assert_eq!(apng_frame_counts(&full_path), apng_frame_counts(&range_path));
        // The encoder is deterministic (the determinism suite
        // pins it), so the full range through the range path is
        // byte-identical to the wrapper's output.
        assert_eq!(
            std::fs::read(&full_path).unwrap(),
            std::fs::read(&range_path).unwrap()
        );
        let _ = std::fs::remove_file(&full_path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn jpeg_range_writes_exactly_the_clamped_file_count() {
        let project = tiny_project();
        let dir = temp_dir("jpeg-range");
        let n = export_jpeg_sequence_range(&project, 2, 99, &dir).unwrap();
        assert_eq!(n, 3, "2..=99 on 5 frames clamps to frames 2..=4");
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        // File names keep the authored indices, as in a full export.
        let expected: Vec<String> = ["frame-0003.jpg", "frame-0004.jpg", "frame-0005.jpg"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(names, expected);
        for name in &names {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "{name}: missing JPEG SOI marker");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jpeg_range_rejects_inverted_ranges() {
        let project = tiny_project();
        let dir = temp_dir("jpeg-err");
        assert!(export_jpeg_sequence_range(&project, 4, 1, &dir).is_err());
    }

    #[test]
    fn jpeg_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_dir = temp_dir("jpeg-full");
        let range_dir = temp_dir("jpeg-fullrange");
        let n_full = export_jpeg_sequence(&project, &full_dir).unwrap();
        let n_range =
            export_jpeg_sequence_range(&project, 0, project.frames.len() - 1, &range_dir)
                .unwrap();
        assert_eq!(n_full, 5);
        assert_eq!(n_full, n_range);
        // Same file set, byte for byte (the JPEG encoder is
        // deterministic, as the determinism suite pins).
        let sorted_names = |dir: &Path| -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        let full_names = sorted_names(&full_dir);
        assert_eq!(full_names, sorted_names(&range_dir));
        for name in &full_names {
            assert_eq!(
                std::fs::read(full_dir.join(name)).unwrap(),
                std::fs::read(range_dir.join(name)).unwrap(),
                "{name}: full and full-range exports differ"
            );
        }
        let _ = std::fs::remove_dir_all(&full_dir);
        let _ = std::fs::remove_dir_all(&range_dir);
    }

    /// One tile of a written spritesheet as raw RGB bytes:
    /// the `k`-th tile in reading order on a grid of `cols`
    /// columns of `w`x`h` tiles.
    fn sheet_tile(path: &Path, k: usize, cols: u32, w: u32, h: u32) -> Vec<u8> {
        let sheet = image::open(path).unwrap().to_rgb8();
        let (x, y) = ((k as u32 % cols) * w, (k as u32 / cols) * h);
        image::imageops::crop_imm(&sheet, x, y, w, h)
            .to_image()
            .into_raw()
    }

    #[test]
    fn sheet_range_tiles_exactly_the_sub_range_frames() {
        let project = tiny_project(); // 96x72 stage, 5 frames
        let path = temp_file("sheet-range", "png");
        let n = export_sheet_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 3, "frames 1..=3 tile exactly 3 frames");
        // 3 tiles on the square-root grid: 2 columns x 2 rows.
        let sheet = image::open(&path).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (192, 144));
        // Tiles are the authored frames themselves, in order,
        // pixel for pixel.
        for (k, frame) in (1..=3usize).enumerate() {
            assert_eq!(
                sheet_tile(&path, k, 2, 96, 72),
                render_frame(&project, frame).into_raw(),
                "tile {k} must be authored frame {frame}"
            );
        }
        // The grid's unused fourth cell stays blank.
        assert!(
            sheet_tile(&path, 3, 2, 96, 72).iter().all(|&b| b == 0),
            "the unused grid cell must stay blank"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sheet_range_clamps_to_the_timeline() {
        let project = tiny_project();
        let path = temp_file("sheet-clamp", "png");
        let n = export_sheet_range(&project, 2, 99, &path).unwrap();
        assert_eq!(n, 3, "2..=99 on 5 frames clamps to frames 2..=4");
        assert_eq!(
            sheet_tile(&path, 0, 2, 96, 72),
            render_frame(&project, 2).into_raw()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sheet_range_single_frame_is_one_tile() {
        let project = tiny_project();
        let path = temp_file("sheet-one", "png");
        let n = export_sheet_range(&project, 2, 2, &path).unwrap();
        assert_eq!(n, 1);
        let sheet = image::open(&path).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (96, 72));
        assert_eq!(
            sheet_tile(&path, 0, 1, 96, 72),
            render_frame(&project, 2).into_raw()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sheet_full_export_matches_the_full_range() {
        let project = tiny_project();
        let full_path = temp_file("sheet-full", "png");
        let range_path = temp_file("sheet-fullrange", "png");
        let n_full = export_spritesheet(&project, &full_path).unwrap();
        let n_range =
            export_sheet_range(&project, 0, project.frames.len() - 1, &range_path).unwrap();
        assert_eq!(n_full, 5);
        assert_eq!(n_full, n_range);
        // Same writer, same tiles: the files are byte-identical.
        assert_eq!(
            std::fs::read(&full_path).unwrap(),
            std::fs::read(&range_path).unwrap()
        );
        let _ = std::fs::remove_file(&full_path);
        let _ = std::fs::remove_file(&range_path);
    }

    #[test]
    fn sheet_range_rejects_inverted_ranges_and_empty_projects() {
        let project = tiny_project();
        let path = temp_file("sheet-err", "png");
        assert!(export_sheet_range(&project, 4, 1, &path).is_err());
        let mut empty = tiny_project();
        empty.frames.clear();
        assert!(export_sheet_range(&empty, 0, 0, &path).is_err());
    }

    #[test]
    fn spritesheet_full_export_of_empty_project_writes_the_blank_sheet() {
        let mut project = tiny_project();
        project.frames.clear();
        let path = temp_file("sheet-blank", "png");
        let n = export_spritesheet(&project, &path).unwrap();
        assert_eq!(n, 0);
        let sheet = image::open(&path).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (1, 1));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn svg_range_writes_exactly_the_sub_range_files() {
        let project = tiny_project();
        let full_dir = temp_dir("svg-full");
        let range_dir = temp_dir("svg-range");
        let n_full = crate::svg::export_svg(&project, &full_dir).unwrap();
        assert_eq!(n_full, 5);
        let n = export_svg_range(&project, 1, 3, &range_dir).unwrap();
        assert_eq!(n, 3);
        let mut names: Vec<String> = std::fs::read_dir(&range_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        // File names keep the authored indices, as in a full export.
        let expected: Vec<String> = ["frame-0002.svg", "frame-0003.svg", "frame-0004.svg"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(names, expected);
        // And the files are the full exporter's own output,
        // byte for byte.
        for name in &names {
            assert_eq!(
                std::fs::read(range_dir.join(name)).unwrap(),
                std::fs::read(full_dir.join(name)).unwrap(),
                "{name}: range and full exports differ"
            );
        }
        let _ = std::fs::remove_dir_all(&full_dir);
        let _ = std::fs::remove_dir_all(&range_dir);
    }

    #[test]
    fn svg_range_single_frame_writes_one_file() {
        let project = tiny_project();
        let dir = temp_dir("svg-one");
        let n = export_svg_range(&project, 2, 2, &dir).unwrap();
        assert_eq!(n, 1);
        assert!(dir.join("frame-0003.svg").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn svg_range_rejects_inverted_ranges_and_empty_projects() {
        let project = tiny_project();
        let dir = temp_dir("svg-err");
        assert!(export_svg_range(&project, 4, 1, &dir).is_err());
        let mut empty = tiny_project();
        empty.frames.clear();
        assert!(export_svg_range(&empty, 0, 0, &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod alpha_export_tests {
    use super::{
        export_png_alpha_sequence_range_scaled_with_render_progress,
        export_png_alpha_sequence_with_render_progress, export_sheet_alpha_scaled,
        export_spritesheet_alpha, render_frame_alpha, render_frame_alpha_scaled,
        RenderProgressEvent,
    };
    use crate::model::Project;
    use std::path::PathBuf;

    /// The 5-frame demo project on a small stage so renders
    /// stay fast (range_export_tests' fixture idiom).
    fn tiny_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [96.0, 72.0];
        // The demo figure stands at the 960x720 stage's
        // centre (root 480, 470); on a 96x72 stage it is
        // hundreds of units off-canvas and NOTHING renders
        // — the range fixtures never notice (their pins are
        // self-consistency), but these pins assert absolute
        // coverage, so re-root every pose at the tiny
        // centre, the move the weather/trail fixtures make.
        // (Wave X CI round 2 caught the empty renders:
        // max alpha was 0.)
        for frame in &mut project.frames {
            for pose in &mut frame.poses {
                pose.root = [48.0, 36.0];
            }
        }
        project
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("styk-alpha-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn temp_file(tag: &str, ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "styk-alpha-{tag}-{}.{ext}",
            std::process::id()
        ))
    }

    fn render_event_sink() -> (
        std::sync::Arc<std::sync::Mutex<Vec<RenderProgressEvent>>>,
        impl Fn(RenderProgressEvent),
    ) {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let callback = move |event: RenderProgressEvent| {
            sink.lock().unwrap().push(event);
        };
        (events, callback)
    }

    fn alpha_stats(img: &image::RgbaImage) -> (bool, bool, bool) {
        let mut any_opaque = false;
        let mut any_clear = false;
        let mut any_partial = false;
        for p in img.pixels() {
            match p[3] {
                255 => any_opaque = true,
                0 => any_clear = true,
                _ => any_partial = true,
            }
        }
        (any_opaque, any_clear, any_partial)
    }

    #[test]
    fn empty_stage_renders_fully_transparent() {
        let mut project = tiny_project();
        project.figures.clear();
        project.strokes.clear();
        project.fx_clips.clear();
        project.particles.clear();
        project.weather = None;
        project.sprites.clear();
        project.texts.clear();
        project.subtitles.clear();
        let img = render_frame_alpha(&project, 0);
        assert_eq!(img.dimensions(), (96, 72));
        assert!(
            img.pixels().all(|p| p[3] == 0),
            "a frame with no drawables is transparent everywhere"
        );
    }

    #[test]
    fn alpha_render_carries_coverage_and_partial_alpha() {
        let project = tiny_project();
        let img = render_frame_alpha(&project, 0);
        let (opaque, clear, partial) = alpha_stats(&img);
        assert!(opaque, "figure strokes cover some pixels fully");
        assert!(clear, "the background is transparent, not the project colour");
        assert!(
            partial,
            "supersampled antialiasing leaves partial-coverage edge pixels"
        );
    }

    #[test]
    fn alpha_render_respects_figure_opacity() {
        let mut project = tiny_project();
        // Only figure art may draw: strokes, FX, weather,
        // ambient particles and media carry their own
        // opacities and would defeat the cap.
        project.strokes.clear();
        project.fx_clips.clear();
        project.particles.clear();
        project.weather = None;
        project.sprites.clear();
        project.texts.clear();
        project.subtitles.clear();
        for fig in &mut project.figures {
            fig.opacity = 0.5;
        }
        let img = render_frame_alpha(&project, 0);
        let max_alpha = img.pixels().map(|p| p[3]).max().unwrap_or(0);
        // The cap is on the COMPOSITE, not on 128: a
        // single-coverage pixel caps at the figure's 0.5
        // (≈128), but strokes composite where they
        // overlap, and at a joint three capsules meet —
        // 1-(1-0.5)^3 = 0.875 ≈ 223. So the pin is
        // three-way: nothing is fully opaque (the figure
        // opacity is honoured), single-coverage interiors
        // sit near 128, and the max reaches the
        // triple-overlap composite (the compositing is
        // real, not a per-pixel clamp). Wave X CI round 3
        // measured exactly 223.
        assert!(
            (200..=235).contains(&max_alpha),
            "a half-opacity figure caps coverage at the triple-overlap composite near 223, got {max_alpha}"
        );
        assert!(
            img.pixels().any(|p| (110..=145).contains(&p[3])),
            "single-coverage stroke interiors sit near 128 at half opacity"
        );
    }

    #[test]
    fn alpha_render_scaled_dimensions_and_refusals() {
        let project = tiny_project();
        let img = render_frame_alpha_scaled(&project, 0, 2.0).unwrap();
        assert_eq!(img.dimensions(), (192, 144));
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let err = render_frame_alpha_scaled(&project, 0, bad)
                .err()
                .expect("an invalid scale is refused");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn png_alpha_sequence_writes_rgba_frames_with_exact_progress() {
        let project = tiny_project();
        let dir = temp_dir("seq");
        let (events, callback) = render_event_sink();
        let written = export_png_alpha_sequence_range_scaled_with_render_progress(
            &project,
            1,
            3,
            &dir,
            2.0,
            Some(&callback),
        )
        .unwrap();
        assert_eq!(written, 3);
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                RenderProgressEvent {
                    rendered_frames: 1,
                    total_frames: 3,
                    finished: false,
                },
                RenderProgressEvent {
                    rendered_frames: 2,
                    total_frames: 3,
                    finished: false,
                },
                RenderProgressEvent {
                    rendered_frames: 3,
                    total_frames: 3,
                    finished: true,
                },
            ],
            "one event per written frame, single terminal last"
        );
        for i in 2..=4 {
            assert!(
                dir.join(format!("frame-{i:04}.png")).exists(),
                "frames land under authored indices"
            );
        }
        let decoded = image::open(dir.join("frame-0002.png")).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgba8);
        let rgba = decoded.to_rgba8();
        assert_eq!(rgba.dimensions(), (192, 144));
        let (opaque, clear, _) = alpha_stats(&rgba);
        assert!(opaque && clear, "the written frame carries real alpha");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_alpha_sequence_empty_project_fires_only_the_terminal() {
        let mut project = tiny_project();
        project.frames.clear();
        let dir = temp_dir("empty");
        let (events, callback) = render_event_sink();
        let written =
            export_png_alpha_sequence_with_render_progress(&project, &dir, Some(&callback))
                .unwrap();
        assert_eq!(written, 0);
        assert!(dir.exists(), "the directory is still created");
        assert_eq!(
            *events.lock().unwrap(),
            vec![RenderProgressEvent {
                rendered_frames: 0,
                total_frames: 0,
                finished: true,
            }],
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spritesheet_alpha_tiles_on_a_transparent_sheet() {
        let project = tiny_project();
        let path = temp_file("sheet", "png");
        let tiled = export_spritesheet_alpha(&project, &path).unwrap();
        assert_eq!(tiled, 5);
        let decoded = image::open(&path).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgba8);
        let sheet = decoded.to_rgba8();
        // 5 tiles on a 3x2 square-root grid of 96x72 tiles.
        assert_eq!(sheet.dimensions(), (288, 144));
        assert_eq!(
            sheet.get_pixel(0, 0)[3],
            0,
            "the sheet background is transparent"
        );
        let (opaque, _, _) = alpha_stats(&sheet);
        assert!(opaque, "the tiles carry their coverage");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sheet_alpha_scaled_uses_scaled_tiles() {
        let project = tiny_project();
        let path = temp_file("sheet-scaled", "png");
        let tiled = export_sheet_alpha_scaled(&project, &path, 2.0).unwrap();
        assert_eq!(tiled, 5);
        let sheet = image::open(&path).unwrap().to_rgba8();
        assert_eq!(sheet.dimensions(), (576, 288));
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod spray_render_tests {
    use crate::export::render_frame_at;
    use crate::model::{Frame, Project, SprayMark, Stroke};
    use crate::paint::spray_dots;
    use image::Rgb;

    /// One frame, no figures, black 200x200 stage, a single white
    /// horizontal stroke from (50, 100) to (150, 100) at width 8 —
    /// plain or sprayed depending on `spray`. With the camera
    /// cleared the default view maps stage units 1:1 onto output
    /// pixels, so pixel coordinates below are stage coordinates.
    fn spray_project(spray: Option<SprayMark>) -> Project {
        let mut project = Project::new_demo();
        project.stage = [200.0, 200.0];
        project.background = 0x000000;
        project.figures.clear();
        project.frames = vec![Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.camera.clear();
        project.strokes = vec![Stroke {
            frame: 0,
            points: vec![[50.0, 100.0], [150.0, 100.0]],
            color: 0xFFFFFF,
            width: 8.0,
            widths: Vec::new(),
            fill: None,
            spray,
            opacity: 1.0,
            scatter: 0.0,
        }];
        project
    }

    fn mark() -> SprayMark {
        SprayMark {
            seed: 42,
            radius: 6.0,
            opacity: 0.85,
        }
    }

    #[test]
    fn spray_stroke_renders_as_dots_not_a_polyline() {
        let mark = mark();
        let sprayed = render_frame_at(&spray_project(Some(mark)), 0.0);
        let plain = render_frame_at(&spray_project(None), 0.0);
        assert_ne!(
            sprayed.as_raw(),
            plain.as_raw(),
            "a spray stroke must not render like its polyline"
        );

        // The dots actually land: the strongest dot's centre pixel
        // is lit. Expected geometry comes from `spray_dots` itself
        // — the single spray geometry source — not from a second
        // computation here.
        let points = [[50.0, 100.0], [150.0, 100.0]];
        let dots = spray_dots(&points, &mark);
        assert!(!dots.is_empty(), "the mark must produce dots");
        let (centre, _) = dots
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        let (cx, cy) = (centre[0].round() as u32, centre[1].round() as u32);
        assert_ne!(
            sprayed.get_pixel(cx, cy),
            &Rgb([0u8, 0, 0]),
            "strongest dot centre {centre:?} must be painted"
        );

        // No polyline: along the stroke's own line, every pixel
        // that no dot can reach (centre farther than the dot
        // radius 4 plus the pixel footprint) stays background in
        // the spray render, while the plain render paints it
        // solid — the path itself is gone, only dots remain.
        let black = Rgb([0u8, 0, 0]);
        let white = Rgb([255u8, 255, 255]);
        let mut checked = 0;
        for x in (62..=138).step_by(2) {
            let p = [x as f32 + 0.5, 100.5];
            let nearest = dots
                .iter()
                .map(|(c, _)| {
                    let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
                    (dx * dx + dy * dy).sqrt()
                })
                .fold(f32::INFINITY, f32::min);
            if nearest > 6.0 {
                checked += 1;
                assert_eq!(
                    sprayed.get_pixel(x, 100),
                    &black,
                    "pixel ({x}, 100) is beyond every dot but on the path"
                );
                assert_eq!(
                    plain.get_pixel(x, 100),
                    &white,
                    "plain polyline must cover ({x}, 100)"
                );
            }
        }
        assert!(checked > 0, "the scan must find dot-free path pixels");
    }

    #[test]
    fn spray_render_is_deterministic() {
        let project = spray_project(Some(mark()));
        let a = render_frame_at(&project, 0.0);
        let b = render_frame_at(&project, 0.0);
        assert_eq!(a.as_raw(), b.as_raw(), "same frame, same pixels");
    }

    #[test]
    fn plain_stroke_keeps_its_polyline_render() {
        // The None path is the classic solid polyline: covered on
        // the path (including the round cap at the endpoint),
        // background well off it.
        let plain = render_frame_at(&spray_project(None), 0.0);
        assert_eq!(plain.get_pixel(100, 100), &Rgb([255u8, 255, 255]));
        assert_eq!(plain.get_pixel(50, 100), &Rgb([255u8, 255, 255]));
        assert_eq!(plain.get_pixel(100, 80), &Rgb([0u8, 0, 0]));
    }
}

// ---------------------------------------------------------------------
// Wave P (P26): raster subtitle burn-in pins, re-based on
// the shared layout in Wave R (R1).
//
// The fixture stage is 480x400. Since R1 the burn-in lays
// out through the core's shared helpers (src/subtitles.rs)
// exactly as the stage draw and the SVG writer do: under
// the default style (bottom anchor, font scale 1.0, white
// text on black backing) the font size is 20 stage units
// and the line height 24, so the stacked line centres sit
// at 372 / 348 / 324 from the top at the bottom anchor
// (28 / 52 / 76 at the top anchor) — edge -/+ (16 margin +
// offset x 24 line height + 12 half line). A line's
// text-colour face spans [centre - 8, centre + 11] rows:
// the 3x5 stamp at cell 4 puts pixel centres at
// (row - 1.5) x 4 about the anchor and each paints a 4x4
// block. The bands below (a midpoint partition of the
// stack) each contain exactly one line's face:
//   bottom: B0 = 360..=390, B1 = 336..=359, B2 = 312..=335
//   top:    T0 =  16..= 40, T1 =  41..= 64, T2 =  65..= 88
// Before R1 the raster sized its stamps proportionally
// (6% of stage height: size 24, step 32.4, centres
// 373.6 / 341.2 / 308.8) and the bands partitioned THAT
// stack; the bands moved because the layout did — the
// assertions' kinds are unchanged.
// The fixture carries no figures, so the ONLY source of
// the exact text colour in a render is the burn-in's
// centre stamp (the backing is the box colour; the
// background is 0x333333).
// ---------------------------------------------------------------------
#[cfg(test)]
mod wave_p_subtitle_burn_in_tests {
    use super::render_frame;
    use crate::model::{Frame, Project};
    use crate::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};
    use image::{ImageBuffer, Rgb};

    const WHITE: [u8; 3] = [255, 255, 255];

    fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
            speaker: None,
        }
    }

    /// An empty 480x400 stage (background 0x333333, no
    /// figures) carrying exactly `cues` under `style`, at
    /// 24 fps (set explicitly, per the wave's timing rule).
    fn fixture(cues: Vec<SubtitleCue>, style: SubtitleStyle) -> Project {
        let mut project = Project::new_demo();
        project.fps = 24.0;
        project.stage = [480.0, 400.0];
        project.background = 0x333333;
        project.figures.clear();
        project.frames = vec![Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.subtitles = cues;
        project.subtitle_style = style;
        project
    }

    /// How many pixels of exactly `color` sit in `rows`.
    fn count_in_rows(
        img: &ImageBuffer<Rgb<u8>, Vec<u8>>,
        rows: std::ops::RangeInclusive<u32>,
        color: [u8; 3],
    ) -> usize {
        let mut n = 0;
        for y in rows {
            for x in 0..img.width() {
                if img.get_pixel(x, y).0 == color {
                    n += 1;
                }
            }
        }
        n
    }

    fn white_in(img: &ImageBuffer<Rgb<u8>, Vec<u8>>, rows: std::ops::RangeInclusive<u32>) -> usize {
        count_in_rows(img, rows, WHITE)
    }

    #[test]
    fn burn_in_single_cue_paints_inside_the_bottom_band() {
        let project = fixture(
            vec![cue(0, 30, "HELLO")],
            SubtitleStyle::default(),
        );
        let mut bare = project.clone();
        bare.subtitles.clear();
        let img = render_frame(&project, 0);
        let control = render_frame(&bare, 0);
        // Every changed pixel sits in the bottom band (the
        // burn-in's full extent, backing included, is rows
        // 360..387); the rest of the frame is untouched.
        let mut diffs = 0usize;
        for y in 0..img.height() {
            for x in 0..img.width() {
                if img.get_pixel(x, y) != control.get_pixel(x, y) {
                    diffs += 1;
                    assert!(
                        y >= 350,
                        "subtitle paint leaked to row {y}, above the bottom band"
                    );
                }
            }
        }
        assert!(diffs > 0, "a live cue must change the frame");
        assert!(
            white_in(&img, 360..=390) > 0,
            "the cue's text-colour face must paint in the bottom band"
        );
    }

    #[test]
    fn burn_in_no_live_cue_is_byte_identical() {
        let project = fixture(
            vec![cue(0, 30, "HELLO")],
            SubtitleStyle::default(),
        );
        let mut bare = project.clone();
        bare.subtitles.clear();
        // Frame 40 is past the cue's half-open range: the
        // render must be the pre-subtitle render, byte for
        // byte.
        let img = render_frame(&project, 40);
        let control = render_frame(&bare, 40);
        assert_eq!(
            img.as_raw(),
            control.as_raw(),
            "a frame with no live cue must render exactly as before subtitles"
        );
        // Sanity: at a live frame the same pair DOES differ,
        // so the equality above is the gate working, not a
        // fixture that never paints.
        assert_ne!(
            render_frame(&project, 5).as_raw(),
            render_frame(&bare, 5).as_raw(),
            "the cue must paint while it is live"
        );
    }

    /// The staggered-stack order proof, run at both anchors:
    /// cue A is live from frame 0, B from 10, C from 20 (all
    /// end at 30). Frame 5 shows A alone — it must sit in the
    /// band nearest the anchor edge; frame 15 adds B in the
    /// next band out; frame 25 adds C in the third. In the
    /// all-live frame the three faces occupy disjoint bands
    /// and no text-colour pixel exists outside them.
    fn stacked_order_and_disjoint_bands(position: SubtitlePosition) {
        let style = SubtitleStyle {
            position,
            ..SubtitleStyle::default()
        };
        let project = fixture(
            vec![
                cue(0, 30, "AAAA"),
                cue(10, 30, "BBBB"),
                cue(20, 30, "CCCC"),
            ],
            style,
        );
        // Bands nearest-to-farthest from the anchor edge.
        let bands: [std::ops::RangeInclusive<u32>; 3] = match position {
            SubtitlePosition::Bottom => [360..=390, 336..=359, 312..=335],
            SubtitlePosition::Top => [16..=40, 41..=64, 65..=88],
        };
        let f5 = render_frame(&project, 5);
        assert!(white_in(&f5, bands[0].clone()) > 0, "cue A paints at the edge band");
        assert_eq!(white_in(&f5, bands[1].clone()), 0, "no second line yet");
        assert_eq!(white_in(&f5, bands[2].clone()), 0, "no third line yet");

        let f15 = render_frame(&project, 15);
        assert!(white_in(&f15, bands[0].clone()) > 0, "cue A stays at the edge band");
        assert!(white_in(&f15, bands[1].clone()) > 0, "cue B paints in the next band out");
        assert_eq!(white_in(&f15, bands[2].clone()), 0, "no third line yet");

        let f25 = render_frame(&project, 25);
        let mut total = 0usize;
        for (i, band) in bands.iter().enumerate() {
            let n = white_in(&f25, band.clone());
            assert!(n > 0, "stacked cue {} must paint in its band", i + 1);
            total += n;
        }
        assert_eq!(
            white_in(&f25, 0..=399),
            total,
            "every text-colour pixel sits inside one of the three disjoint bands"
        );
    }

    #[test]
    fn burn_in_stacked_cues_disjoint_bands_bottom_anchor() {
        stacked_order_and_disjoint_bands(SubtitlePosition::Bottom);
    }

    #[test]
    fn burn_in_stacked_cues_disjoint_bands_top_anchor() {
        stacked_order_and_disjoint_bands(SubtitlePosition::Top);
    }

    #[test]
    fn burn_in_multiline_cue_puts_its_last_line_nearest_the_bottom_edge() {
        // One cue, two lines: a single narrow glyph over a
        // ten-glyph line. At the bottom anchor the cue's LAST
        // line sits closest to the edge, so the wide line's
        // face fills the edge band and the narrow line sits
        // one band up — proven by the face-pixel counts,
        // which differ by an order of magnitude.
        let project = fixture(
            vec![cue(0, 30, "I\nWWWWWWWWWW")],
            SubtitleStyle::default(),
        );
        let img = render_frame(&project, 0);
        let edge = white_in(&img, 360..=390);
        let above = white_in(&img, 336..=359);
        assert!(above > 0, "the cue's first line paints in the band above");
        assert!(
            edge > 4 * above,
            "the cue's last (wide) line must dominate the edge band: edge {edge}, above {above}"
        );
    }

    #[test]
    fn burn_in_honours_style_position_colours_and_scale() {
        let style = SubtitleStyle {
            position: SubtitlePosition::Top,
            font_scale: 2.0,
            text_color: 0xFF0000,
            box_color: 0x00FF00,
        };
        let project = fixture(vec![cue(0, 30, "HELLO")], style);
        let img = render_frame(&project, 0);
        let red = [255, 0, 0];
        let green = [0, 255, 0];
        // Top-anchored at scale 2.0 (font size 40, line
        // height 48): the line centre is 16 + 24 = 40 and
        // the red face spans rows 24..63 (cell 8: pixel
        // centres at (row - 1.5) x 8 about the anchor, each
        // an 8x8 block). It must reach rows 48..63, which
        // an unscaled top cue (face rows 20..39) could
        // never touch.
        assert!(
            count_in_rows(&img, 48..=63, red) > 0,
            "the scaled cue's text colour must paint in the top band"
        );
        assert!(
            count_in_rows(&img, 0..=80, green) > 0,
            "the backing colour must paint around the text"
        );
        assert_eq!(
            count_in_rows(&img, 200..=399, red),
            0,
            "a top-anchored cue paints nothing in the bottom half"
        );
    }
}

#[cfg(test)]
mod alpha_apng_tests {
    use super::{
        export_apng, export_apng_alpha, export_apng_alpha_range, export_apng_alpha_range_scaled,
        export_apng_alpha_scaled,
    };
    use crate::model::Project;
    use std::path::{Path, PathBuf};

    /// The 5-frame demo project on a small stage so renders
    /// stay fast, with every pose re-rooted at the tiny centre
    /// (alpha_export_tests' fixture idiom: at the demo's native
    /// root the figure stands hundreds of units off a 96x72
    /// stage and nothing renders, which would defeat these
    /// pins' absolute-coverage premises). fps is pinned to
    /// 24.0 — the APNG frame delay derives from it.
    fn tiny_project() -> Project {
        let mut project = Project::new_demo();
        project.stage = [96.0, 72.0];
        project.fps = 24.0;
        for frame in &mut project.frames {
            for pose in &mut frame.poses {
                pose.root = [48.0, 36.0];
            }
        }
        project
    }

    fn temp_file(tag: &str, ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "styk-alpha-apng-{tag}-{}.{ext}",
            std::process::id()
        ))
    }

    /// Decode every frame of a written APNG through the `png`
    /// crate's reader (the encoder writes full-canvas frames,
    /// so each decoded frame is the displayed frame). Panics
    /// on a malformed file — these pins only read files the
    /// exports just wrote.
    fn decode_apng_frames(path: &Path) -> Vec<image::RgbaImage> {
        let file = std::fs::File::open(path).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let mut reader = decoder.read_info().unwrap();
        let (w, h) = (reader.info().width, reader.info().height);
        let mut frames = Vec::new();
        loop {
            let mut buf = vec![0u8; reader.output_buffer_size()];
            match reader.next_frame(&mut buf) {
                Ok(info) => {
                    buf.truncate(info.buffer_size());
                    frames.push(
                        image::RgbaImage::from_raw(info.width, info.height, buf)
                            .expect("decoded frame size matches its buffer"),
                    );
                }
                Err(_) => break,
            }
        }
        assert!(!frames.is_empty(), "APNG at {} decoded no frames", path.display());
        assert!(
            frames.iter().all(|f| f.dimensions() == (w, h)),
            "every APNG frame is the full canvas"
        );
        frames
    }

    /// (any fully opaque, any fully transparent, any partial)
    /// alpha over one decoded frame — alpha_export_tests'
    /// statistic, applied to decoded APNG frames.
    fn alpha_stats(img: &image::RgbaImage) -> (bool, bool, bool) {
        let mut any_opaque = false;
        let mut any_clear = false;
        let mut any_partial = false;
        for p in img.pixels() {
            match p[3] {
                255 => any_opaque = true,
                0 => any_clear = true,
                _ => any_partial = true,
            }
        }
        (any_opaque, any_clear, any_partial)
    }

    #[test]
    fn alpha_apng_frames_carry_coverage_alpha() {
        let project = tiny_project();
        let path = temp_file("full", "png");
        let n = export_apng_alpha(&project, &path).unwrap();
        assert_eq!(n, project.frames.len());
        let frames = decode_apng_frames(&path);
        assert_eq!(frames.len(), n, "one decoded frame per written frame");
        let (opaque, clear, partial) = alpha_stats(&frames[0]);
        assert!(opaque, "figure strokes cover some pixels fully");
        assert!(clear, "the background is transparent, not the project colour");
        assert!(
            partial,
            "supersampled antialiasing leaves partial-coverage edge pixels"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_apng_range_writes_exactly_the_sub_range_frames() {
        let project = tiny_project();
        let path = temp_file("range", "png");
        let n = export_apng_alpha_range(&project, 1, 3, &path).unwrap();
        assert_eq!(n, 3);
        let frames = decode_apng_frames(&path);
        assert_eq!(frames.len(), 3, "the range is exactly frames 1..=3");
        let (opaque, clear, _) = alpha_stats(&frames[0]);
        assert!(opaque && clear, "ranged alpha frames carry coverage alpha too");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_apng_empty_stage_is_fully_transparent() {
        let mut project = tiny_project();
        project.figures.clear();
        project.strokes.clear();
        project.fx_clips.clear();
        project.particles.clear();
        project.weather = None;
        project.sprites.clear();
        project.texts.clear();
        project.subtitles.clear();
        let path = temp_file("empty-stage", "png");
        let n = export_apng_alpha(&project, &path).unwrap();
        assert_eq!(n, project.frames.len());
        let frames = decode_apng_frames(&path);
        assert!(
            frames.iter().all(|f| f.pixels().all(|p| p[3] == 0)),
            "a frame with no drawables is transparent everywhere, in every frame"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_apng_scaled_doubles_dimensions_and_keeps_alpha() {
        let project = tiny_project();
        let path = temp_file("scaled", "png");
        let n = export_apng_alpha_scaled(&project, &path, 2.0).unwrap();
        assert_eq!(n, project.frames.len());
        let frames = decode_apng_frames(&path);
        assert_eq!(
            frames[0].dimensions(),
            (192, 144),
            "2.0x APNG frame size (a true re-render, not an upscale)"
        );
        let (opaque, clear, _) = alpha_stats(&frames[0]);
        assert!(opaque && clear, "scaled alpha frames still carry coverage alpha");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_apng_scale_one_delegates_byte_identically() {
        // The alpha family's own 1.0 contract, mirroring the
        // opaque family's trio pin: the scaled and ranged
        // entries at exactly 1.0 write the unscaled bytes.
        let project = tiny_project();
        let plain = temp_file("one-plain", "png");
        let scaled = temp_file("one-scaled", "png");
        let ranged = temp_file("one-ranged", "png");
        export_apng_alpha(&project, &plain).unwrap();
        export_apng_alpha_scaled(&project, &scaled, 1.0).unwrap();
        export_apng_alpha_range_scaled(&project, 0, 4, &ranged, 1.0).unwrap();
        let plain_bytes = std::fs::read(&plain).unwrap();
        assert_eq!(plain_bytes, std::fs::read(&scaled).unwrap(), "alpha APNG 1.0 bytes");
        assert_eq!(
            plain_bytes,
            std::fs::read(&ranged).unwrap(),
            "alpha APNG range 1.0 bytes"
        );
        for p in [&plain, &scaled, &ranged] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn alpha_apng_invalid_scale_refused_before_any_write() {
        let project = tiny_project();
        for bad in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let path = temp_file("bad", "png");
            let _ = std::fs::remove_file(&path);
            assert!(
                export_apng_alpha_scaled(&project, &path, bad).is_err(),
                "scale {bad} must be refused"
            );
            assert!(
                export_apng_alpha_range_scaled(&project, 0, 4, &path, bad).is_err(),
                "ranged scale {bad} must be refused"
            );
            assert!(!path.exists(), "refused alpha APNG export created a file (scale {bad})");
        }
    }

    #[test]
    fn alpha_apng_empty_project_writes_no_file() {
        // The encoders' empty-times contract, at the public
        // entry: Ok(0), the parent directory exists, no file.
        let mut project = tiny_project();
        project.frames.clear();
        let path = temp_file("no-frames", "png");
        let _ = std::fs::remove_file(&path);
        let n = export_apng_alpha(&project, &path).unwrap();
        assert_eq!(n, 0);
        assert!(!path.exists(), "an empty alpha APNG export writes no file");
    }

    #[test]
    fn opaque_apng_stays_fully_opaque_and_differs_from_alpha() {
        // The opaque family's guard rail, expressible in-tree:
        // its encoders are untouched by this slice, its decoded
        // frames stay alpha-255 everywhere (the RGBA conversion
        // it has always done), and its bytes differ from the
        // alpha family's for the same project.
        let project = tiny_project();
        let opaque_path = temp_file("opaque", "png");
        let alpha_path = temp_file("alpha", "png");
        export_apng(&project, &opaque_path).unwrap();
        export_apng_alpha(&project, &alpha_path).unwrap();
        let frames = decode_apng_frames(&opaque_path);
        assert!(
            frames.iter().all(|f| f.pixels().all(|p| p[3] == 255)),
            "opaque APNG frames carry no transparency"
        );
        assert_ne!(
            std::fs::read(&opaque_path).unwrap(),
            std::fs::read(&alpha_path).unwrap(),
            "the alpha family's bytes differ from the opaque family's"
        );
        let _ = std::fs::remove_file(&opaque_path);
        let _ = std::fs::remove_file(&alpha_path);
    }
}
