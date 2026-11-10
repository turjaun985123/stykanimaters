//! SVG export: one vector file per authored frame, drawn with the
//! same camera, shapes, strokes and particles as the rasterizer.
//!
//! Wave 11 (E8) parity pass: filled strokes, FX-clip particles
//! (including Trail segments) and weather particles now emit
//! here too — previously this writer silently dropped all
//! three, so an MCP client exporting SVG got a different
//! picture from the rasterizer/stage for the same project.
//! Wave 12 (C1) closed the remaining figure gaps the Wave 11
//! report named — figure opacity, solid-colour taper and bent
//! segments now emit the way the rasterizer draws them (see
//! the figure loop below); of that list only z-order differed.
//! Wave 13 (C1) closed that too: the stroke block now emits
//! AFTER the figures, matching the rasterizer's paint order
//! (its segment pass composites strokes over figure segments;
//! media still composites last), and node style opacity now
//! reaches head rings and shape outlines, composed with the
//! figure factor exactly as segments compose it.
//! Wave 14 (C4): clip-def media — media stored only on a
//! movieclip's definition — reaches this exporter through
//! the same `media_items` funnel, via C1's per-node
//! fallback there; no clip-specific emission exists here
//! (docs/wave13/CLIP_MEDIA.md §6).
//!
//! XML safety: almost no user-controlled string is
//! interpolated into the document — every `format!` argument
//! below is a number, a `hex()` colour (u32-masked), or a
//! fixed literal. The exceptions are media text content
//! (Wave 12, C3) and subtitle cue text (Wave P, P27, see
//! `append_subtitles`), which route through [`xml_escape`] —
//! the pinned helper any future text emission (names, labels)
//! MUST also route through.
//!
//! Camera rotation (wave 6) is applied by rotating points inside
//! the `view` closure — the same `export::camera_rotate_offset`
//! helper the rasterizer uses — rather than wrapping the scene in
//! a `<g transform="rotate(...)">`. Point-level rotation keeps the
//! SVG geometry in exact agreement with the rasterizer (including
//! the userSpaceOnUse gradient endpoints, which a group transform
//! would also rotate, but only implicitly), and every element this
//! exporter emits is positioned through `view`, so nothing can be
//! left unrotated by accident.

use crate::export::{camera_rotate_offset, node_trail, particles_at};
use crate::model::{evaluate, CapKind, Project, Shape, V2};
use std::path::Path;

fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
}

/// Escape text for safe interpolation into SVG text content
/// or a quoted attribute value. Delegates to
/// `text_edit::xml_escape` (Wave 12, C7): the five
/// XML-significant characters become entities (`&` first —
/// it introduces every other entity), and control
/// characters become spaces, because XML 1.0 forbids C0
/// controls even as character references — this module's
/// original mapping passed them through, which would have
/// made any document carrying imported or MCP-authored text
/// with a control character in it invalid XML. Media text
/// emission (C3, below) routes through here; the tests below
/// pin the five-character mapping this helper has always
/// promised.
pub fn xml_escape(text: &str) -> String {
    crate::text_edit::xml_escape(text)
}

/// The ` stroke-opacity="…"` attribute for a drawing stroke
/// whose opacity is below 1: the value is clamped to
/// [0.0, 1.0] at read and formatted to three decimals with
/// trailing zeros trimmed (0.5 renders as "0.5", 0.25 as
/// "0.25"). A fully opaque stroke — opacity 1.0, anything
/// above it before the clamp, or NaN — yields an empty
/// string, so output for existing projects stays
/// byte-identical to before the field existed. Spray
/// strokes never pass through here: their dots already
/// carry the spray mark's own per-dot alpha.
fn stroke_opacity_attr(opacity: f32) -> String {
    opacity_attr("stroke-opacity", opacity)
}

/// The ` fill-opacity="…"` twin of [`stroke_opacity_attr`],
/// for filled elements (a pressured stroke's degenerate
/// segment disc, Wave 12 C9).
fn fill_opacity_attr(opacity: f32) -> String {
    opacity_attr("fill-opacity", opacity)
}

fn opacity_attr(kind: &str, opacity: f32) -> String {
    let opacity = opacity.clamp(0.0, 1.0);
    if !(opacity < 1.0) {
        return String::new();
    }
    let text = format!("{opacity:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!(" {kind}=\"{text}\"")
}

pub fn export_svg(project: &Project, dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for frame_idx in 0..project.frames.len() {
        write_frame_svg(
            project,
            frame_idx,
            &dir.join(format!("frame-{:04}.svg", frame_idx + 1)),
        )?;
    }
    Ok(project.frames.len())
}

/// Write ONE authored frame of `project` as an SVG file at
/// `path`, drawn with the same camera, shapes, strokes and
/// particles as the rasterizer. `export_svg` is this writer
/// applied to every frame under the `frame-{:04}.svg` names;
/// the content is exactly what the whole-timeline export
/// writes for the same frame. The parent directory is NOT
/// created — callers do that, as `export_svg` always has.
pub fn write_frame_svg(project: &Project, frame: usize, path: &Path) -> std::io::Result<()> {
    // Wave P (P17): the evaluated frame, as in the rasterizer —
    // when the project carries graph channels, poses AND defs
    // come from `graph::apply_channels` (the stage's
    // `positions_f` evaluation), shared by the figure loop and
    // the FX clip anchors below; the media pass funnels through
    // `media_items`, which applies the same rule. With no
    // channels this is exactly the raw `poses_at` path.
    let channelled = if project.channels.is_empty() {
        None
    } else {
        Some(crate::graph::apply_channels(
            project,
            &project.channels,
            frame as f32,
        ))
    };
    let raw_poses = project.poses_at(frame as f32);
    let poses: &Vec<crate::model::Pose> = match &channelled {
        Some(r) => &r.poses,
        None => &raw_poses,
    };
    let (cam_x, cam_y, zoom) = project.camera_at(frame as f32);
    let angle = project.camera_angle_at(frame as f32);
    let view = |p: V2| -> V2 {
        let (dx, dy) = (p[0] - cam_x, p[1] - cam_y);
        // Camera rotation (wave 6): same convention and helper
        // as the rasterizer; angle 0 keeps the offset verbatim.
        let (ox, oy) = if angle == 0.0 {
            (dx, dy)
        } else {
            let r = camera_rotate_offset(dx, dy, angle);
            (r[0], r[1])
        };
        [
            ox * zoom + project.stage[0] / 2.0,
            oy * zoom + project.stage[1] / 2.0,
        ]
    };
    let mut svg = String::new();
    svg += &format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">\n",
        project.stage[0], project.stage[1], project.stage[0], project.stage[1]
    );
    svg += &format!(
        "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n",
        hex(project.background)
    );
    // Gradient definitions are collected while drawing the
    // figures and substituted in before the file is written.
    svg += "<!--DEFS-->\n";
    let mut defs = String::new();
    let mut grad_n = 0usize;
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        // Positions from the channelled def when channels
        // exist (P17); node properties stay on the figure's own
        // def below — the stage's split (`positions_f` returns
        // only positions).
        let def = match &channelled {
            Some(r) => &r.defs[fi],
            None => &fig.def,
        };
        let pos = evaluate(def, &poses[fi]);
        // Per-node bends at this frame (Wave 12, C1) — the same
        // source the rasterizer reads; empty when the project
        // carries no bend data. Wave Y (P-7): with channels
        // present the bends come from the channelled
        // evaluation's own rows — seeded from `bends_at` and
        // overridden by any Bend channel (graph.rs), so a
        // bend channel is visible in the export; the
        // no-channels path is `bends_at` verbatim, as before.
        let bends = match &channelled {
            Some(r) => r.bends.get(fi).cloned().unwrap_or_default(),
            None => project.bends_at(frame as f32, fi),
        };
        // Figure (layer) opacity, clamped once per figure. The
        // rasterizer composes it multiplicatively into every
        // segment's alpha (export.rs: style.opacity *
        // fig.opacity) and fades heads/rings/shapes toward the
        // background by it (they have no alpha plumbing there).
        // SVG has real alpha, so this writer applies the figure
        // factor as an opacity attribute instead: over the
        // background rect the result is identical to the
        // raster's colour fade, and over other elements it is
        // the true composite the raster's segment pass makes.
        // At 1.0 nothing extra is emitted, so output for
        // existing projects stays byte-identical.
        let fig_opacity = fig.opacity.clamp(0.0, 1.0);
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let color_hex = node.color.unwrap_or(fig.color);
            let color = hex(color_hex);
            let w = node.thickness * zoom;
            if let Some(par) = node.parent {
                let a = view(pos[par]);
                let b = view(pos[i]);
                let style = node.style;
                let half_near = w / 2.0;
                // A point cap forces the far width to zero.
                let half_far = if style.cap == CapKind::Point {
                    0.0
                } else {
                    half_near * style.taper.max(0.0)
                };
                // The segment's alpha in every renderer: the
                // node style's opacity composed with the figure
                // (layer) opacity — the rasterizer's StyledSeg
                // opacity is exactly this product (export.rs).
                let seg_opacity = style.opacity * fig_opacity;
                // Pieces: an unbent segment is a single piece;
                // a bent one is four pieces through the arc's
                // quarter points — the piece generator's
                // construction (paint::styled_segment_pieces,
                // Wave Y P-7; the rasterizer chains the same
                // generator): the joints are bend_mid at
                // t = 0.5 and its recursion at half the bend
                // for t = 0.25 / 0.75, the widths and colours
                // are the whole-segment interpolations at
                // those t values, and every piece but the
                // last has a Round cap so the arc joints stay
                // smooth whatever the segment's own cap is.
                // Each piece is (near point, far point, near
                // half-width, far half-width, cap, near
                // colour, far colour).
                let bend = bends.get(i).copied().unwrap_or(0.0);
                let color_far = style.color2.unwrap_or(color_hex);
                let pieces: Vec<(V2, V2, f32, f32, CapKind, u32, u32)> = if bend == 0.0 {
                    vec![(a, b, half_near, half_far, style.cap, color_hex, color_far)]
                } else {
                    let mid_s = crate::model::bend_mid(pos[par], pos[i], bend);
                    let joints = [
                        a,
                        view(crate::model::bend_mid(pos[par], mid_s, bend * 0.5)),
                        view(mid_s),
                        view(crate::model::bend_mid(mid_s, pos[i], bend * 0.5)),
                        b,
                    ];
                    let half_at =
                        |t: f32| half_near + (half_far - half_near) * t;
                    let color_at =
                        |t: f32| crate::style::lerp_color(color_hex, color_far, t);
                    (0..4)
                        .map(|k| {
                            let t0 = k as f32 / 4.0;
                            let t1 = (k + 1) as f32 / 4.0;
                            (
                                joints[k],
                                joints[k + 1],
                                half_at(t0),
                                half_at(t1),
                                if k < 3 { CapKind::Round } else { style.cap },
                                color_at(t0),
                                color_at(t1),
                            )
                        })
                        .collect()
                };
                for (pa, pb, p_near, p_far, p_cap, p_col_near, p_col_far) in pieces {
                    if style.color2.is_some() || p_far != p_near {
                        // Tapered and/or gradient piece: a
                        // filled quad spanning its near/far
                        // widths. The gradient fill exists only
                        // when the style carries a second
                        // colour; a tapered SOLID segment takes
                        // the same quad with a solid fill.
                        // (Before Wave 12 the quad was built
                        // only for gradient segments, so a
                        // tapered solid segment fell through to
                        // a uniform-width line and lost its
                        // taper; the rasterizer tapers from the
                        // style regardless of colour2.)
                        let fill = match style.color2 {
                            Some(_) => {
                                let id = format!("grad{grad_n}");
                                grad_n += 1;
                                defs += &format!(
                                    "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>\n",
                                    id, pa[0], pa[1], pb[0], pb[1], hex(p_col_near),
                                    hex(p_col_far)
                                );
                                format!("url(#{id})")
                            }
                            None => hex(p_col_near),
                        };
                        let d = [pb[0] - pa[0], pb[1] - pa[1]];
                        let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(f32::EPSILON);
                        let n = [-d[1] / len, d[0] / len];
                        let pts = format!(
                            "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                            pa[0] + n[0] * p_near,
                            pa[1] + n[1] * p_near,
                            pb[0] + n[0] * p_far,
                            pb[1] + n[1] * p_far,
                            pb[0] - n[0] * p_far,
                            pb[1] - n[1] * p_far,
                            pa[0] - n[0] * p_near,
                            pa[1] - n[1] * p_near
                        );
                        let opacity = if seg_opacity < 1.0 {
                            format!(" fill-opacity=\"{:.2}\"", seg_opacity)
                        } else {
                            String::new()
                        };
                        svg += &format!(
                            "<polygon points=\"{}\" fill=\"{}\"{}/>\n",
                            pts, fill, opacity
                        );
                    } else {
                        let cap = match p_cap {
                            CapKind::Round => "round",
                            CapKind::Square => "square",
                            // A point is not expressible as a
                            // line cap; butt ends the stroke
                            // flush at the joint.
                            CapKind::Point => "butt",
                        };
                        let opacity = if seg_opacity < 1.0 {
                            format!(" stroke-opacity=\"{:.2}\"", seg_opacity)
                        } else {
                            String::new()
                        };
                        svg += &format!(
                            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"{}\"{}/>\n",
                            pa[0], pa[1], pb[0], pb[1], color, p_near * 2.0, cap, opacity
                        );
                    }
                }
            }
            // Heads, rings and shape outlines (Wave 13, C1 —
            // the S10 SVG half): their alpha is the node's
            // OWN style opacity composed with the figure
            // factor, the same product the segments above
            // use (before this wave only the figure factor
            // was applied here, so a head or shape node
            // styled translucent drew fully opaque in SVG
            // whenever the figure itself was opaque). SVG
            // carries it as a stroke-opacity attribute —
            // identical over the background to the
            // rasterizer's colour fade, and the true
            // composite over other elements. At a product of
            // 1.0 nothing is emitted, so unstyled projects
            // stay byte-identical.
            let node_opacity = node.style.opacity * fig_opacity;
            let node_opacity_attr = if node_opacity < 1.0 {
                format!(" stroke-opacity=\"{:.2}\"", node_opacity)
            } else {
                String::new()
            };
            if node.head {
                let c = view(pos[i]);
                svg += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"{}/>\n",
                    c[0], c[1], project.head_ring_radius() * zoom, color, w, node_opacity_attr
                );
            }
            match node.shape {
                Shape::Circle => {
                    let c = view(pos[i]);
                    svg += &format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"{}/>\n",
                        c[0], c[1], node.length / 2.0 * zoom, color, w, node_opacity_attr
                    );
                }
                Shape::Polygon(sides) => {
                    let sides = sides.max(3) as usize;
                    let c = view(pos[i]);
                    let r = node.length / 2.0 * zoom;
                    let pts: Vec<String> = (0..sides)
                        .map(|k| {
                            let a = k as f32 / sides as f32 * std::f32::consts::TAU
                                - std::f32::consts::FRAC_PI_2;
                            format!("{:.1},{:.1}", c[0] + r * a.cos(), c[1] + r * a.sin())
                        })
                        .collect();
                    svg += &format!(
                        "<polygon points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"{}/>\n",
                        pts.join(" "),
                        color,
                        w,
                        node_opacity_attr
                    );
                }
                Shape::Line => {}
            }
        }
    }
    // Drawing strokes (Wave 13, C1): emitted AFTER the
    // figures, matching the rasterizer's paint order —
    // export.rs collects the figure segments first and
    // pushes the stroke segments into the same segment
    // pass after them, so strokes composite OVER the
    // figures there; this writer used to emit its stroke
    // block first, which put every stroke UNDER the
    // figures in an SVG renderer. The particle discs
    // still follow (the raster blends them after the
    // segment pass) and media still composites last
    // (append_media, below). No element's own bytes
    // change in the move — only the block order — so a
    // frame with no strokes serialises byte-identically
    // to before.
    for (index, stroke) in project.strokes.iter().enumerate() {
        if stroke.frame != frame {
            continue;
        }
        if let Some(mark) = &stroke.spray {
            // Spray stroke: dots only, no polyline — the same
            // decision the rasterizer makes, with the dot
            // geometry from the single spray source
            // (paint::spray_dots), so every renderer agrees on
            // where the dots land. Centres go through `view`
            // exactly like polyline points do. The radius is
            // scaled by the camera zoom, as the rasterizer
            // scales it (Wave 11 E8: this was emitted raw,
            // so dots shrank relative to the scene whenever
            // the camera zoomed in).
            let r = (stroke.width * 0.5).max(1.0) * zoom;
            for (centre, alpha) in crate::paint::spray_dots(&stroke.points, mark) {
                let q = view(centre);
                // Spray alphas are clamped (Wave 12, A5): a
                // stored mark can carry an alpha above 1, and
                // an unclamped `fill-opacity="1.40"` is not
                // valid attribute data.
                svg += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" fill-opacity=\"{:.2}\"/>\n",
                    q[0],
                    q[1],
                    r,
                    hex(stroke.color),
                    alpha.clamp(0.0, 1.0)
                );
            }
        } else if stroke.fill.is_some() && stroke.points.len() >= 3 {
            // Filled closed shape (Wave 11 E8): the stage
            // draws `stroke.fill` as a filled polygon and —
            // per its precedence in stage_layers.rs (spray,
            // then fill, then scatter, then the plain
            // polyline) — nothing else for this stroke: no
            // outline, and scatter does NOT texture a filled
            // stroke. This branch mirrors that precedence so
            // SVG and stage agree; the rasterizer paints the
            // same region in its pixel pass (Wave 11, E8).
            // The fill alpha is the stroke's recorded
            // opacity, uniform across the region, exactly as
            // the stage's Filled prim carries it.
            let fill = stroke.fill.unwrap();
            let alpha = stroke.opacity.clamp(0.0, 1.0);
            let opacity = if alpha < 1.0 {
                format!(" fill-opacity=\"{alpha:.2}\"")
            } else {
                String::new()
            };
            let pts: Vec<String> = stroke
                .points
                .iter()
                .map(|p| {
                    let q = view(*p);
                    format!("{:.1},{:.1}", q[0], q[1])
                })
                .collect();
            svg += &format!(
                "<polygon points=\"{}\" fill=\"{}\"{}/>\n",
                pts.join(" "),
                hex(fill),
                opacity
            );
        } else if stroke.scatter > 0.0 && !stroke.points.is_empty() {
            // Scattered stroke (Wave 10 adoption): dabs,
            // not a polyline — the same placements the
            // stage and the rasterizer compute
            // (paint::stroke_dabs at the shared spacing),
            // one circle per dab at the stroke's recorded
            // opacity, uniform across dabs. The radius is
            // zoom-scaled like the spray radius above (and
            // like the rasterizer's disc pass).
            let r = (stroke.width * 0.5).max(1.0) * zoom;
            let alpha = stroke.opacity.clamp(0.0, 1.0);
            for centre in crate::paint::stroke_dabs(
                stroke,
                index,
                crate::paint::stroke_dab_spacing(stroke),
            ) {
                let q = view(centre);
                svg += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" fill-opacity=\"{:.2}\"/>\n",
                    q[0],
                    q[1],
                    r,
                    hex(stroke.color),
                    alpha
                );
            }
        } else if stroke.points.len() > 1 {
            if stroke.widths.len() == stroke.points.len() {
                // Pressured stroke (Wave 12, C9): the nib's
                // per-point widths are baked into
                // `stroke.widths` at stroke finish, and the
                // rasterizer draws the stroke as per-segment
                // widths — the average of each segment's two
                // endpoint widths, the raster's own predicate
                // and arithmetic (export.rs). A single
                // <polyline> has only one width, so this
                // branch emits one <line> per segment at that
                // average, round-capped (the caps are what
                // join the segments smoothly, as the raster's
                // coverage joins them). A degenerate segment
                // (both endpoints the same point) has no line
                // to draw: it emits as a disc of the averaged
                // width, which is what the raster's round cap
                // paints there. The stroke opacity rides every
                // segment, as the raster's per-segment opacity
                // does. Strokes without parallel widths keep
                // the single-polyline form below, byte-identical
                // to before this branch existed.
                let opacity = stroke_opacity_attr(stroke.opacity);
                for (i, w) in stroke.points.windows(2).enumerate() {
                    let qa = view(w[0]);
                    let qb = view(w[1]);
                    let width = (stroke.widths[i] + stroke.widths[i + 1]) / 2.0 * zoom;
                    if qa == qb {
                        svg += &format!(
                            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\"{}/>\n",
                            qa[0],
                            qa[1],
                            width / 2.0,
                            hex(stroke.color),
                            fill_opacity_attr(stroke.opacity)
                        );
                    } else {
                        svg += &format!(
                            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"{}/>\n",
                            qa[0],
                            qa[1],
                            qb[0],
                            qb[1],
                            hex(stroke.color),
                            width,
                            opacity
                        );
                    }
                }
            } else {
                let pts: Vec<String> = stroke
                    .points
                    .iter()
                    .map(|p| {
                        let q = view(*p);
                        format!("{:.1},{:.1}", q[0], q[1])
                    })
                    .collect();
                svg += &format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"{}/>\n",
                    pts.join(" "),
                    hex(stroke.color),
                    stroke.width * zoom,
                    stroke_opacity_attr(stroke.opacity)
                );
            }
        }
    }
    // Particle discs, in the rasterizer's own draw order:
    // authored particle events first, then FX clips, then
    // weather (export.rs `render_frame_at` appends them to
    // one disc pass in exactly this order; the stage's
    // stage_fx layer evaluates the same sources). Wave 11
    // E8: this writer used to emit ONLY `particles_at`, so
    // FX clips and weather vanished from SVG exports.
    let mut discs: Vec<(V2, f32, [u8; 3], f32)> = particles_at(project, frame as f32);
    for entry in &project.fx_clips {
        // Hidden clips render nothing (the rasterizer's
        // skip at its render source, mirrored here).
        if entry.hidden {
            continue;
        }
        let Some(fig) = project.figures.get(entry.figure) else {
            continue;
        };
        if fig.hidden {
            continue;
        }
        // `poses.get` where the rasterizer indexes directly:
        // a figure absent from this frame has no anchor and
        // its clip cannot emit — identical output wherever
        // the rasterizer itself survives.
        let Some(pose) = poses.get(entry.figure) else {
            continue;
        };
        // FX anchors sit on the evaluated (channelled) figure,
        // as in the rasterizer and the stage's stage_fx layer.
        let def = match &channelled {
            Some(r) => &r.defs[entry.figure],
            None => &fig.def,
        };
        let pos = evaluate(def, pose);
        let Some(&anchor) = pos.get(entry.node) else {
            continue;
        };
        for (p, r, c, a) in crate::vfx::clip_particles(&entry.clip, frame as f32, anchor) {
            discs.push((p, r, c, a));
        }
        // A live Trail clip also draws the node's real
        // recent path as thin fading segments UNDER the
        // particles (the rasterizer pushes them into its
        // segment pass with half-width 1.6 * zoom, i.e. a
        // 3.2 * zoom line here, at the trail alpha).
        if entry.clip.kind == crate::vfx::ClipKind::Trail
            && (frame as f32) >= entry.clip.start
            && (frame as f32) < entry.clip.start + entry.clip.duration
        {
            let span = (entry.clip.duration * 0.5).clamp(2.0, 12.0);
            for (a, b, c, alpha) in node_trail(project, frame as f32, entry.figure, entry.node, 8, span)
            {
                let qa = view(a);
                let qb = view(b);
                let opacity = if alpha < 1.0 {
                    format!(" stroke-opacity=\"{alpha:.2}\"")
                } else {
                    String::new()
                };
                svg += &format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#{:02X}{:02X}{:02X}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"{}/>\n",
                    qa[0],
                    qa[1],
                    qb[0],
                    qb[1],
                    c[0],
                    c[1],
                    c[2],
                    3.2 * zoom,
                    opacity
                );
            }
        }
    }
    if let Some(weather) = &project.weather {
        for (p, r, c, a) in
            crate::vfx::weather_particles(weather, frame as f32, project.stage, project.fps)
        {
            discs.push((p, r, c, a));
        }
    }
    for (p, r, c, a) in discs {
        let q = view(p);
        svg += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"#{:02X}{:02X}{:02X}\" fill-opacity=\"{:.2}\"/>\n",
            q[0], q[1], r * zoom, c[0], c[1], c[2], a
        );
    }
    // Media last (Wave 12, C3): in the rasterizer, media
    // composites OVER the particle pass (export.rs runs
    // `composite_media` after the disc pass), so the media
    // elements emit after the discs here too. An empty item
    // list appends nothing, so media-free projects stay
    // byte-identical.
    append_media(&mut svg, &mut defs, project, frame, &view, angle, zoom);
    // Subtitles (Wave P, P27): every cue live at this frame
    // is emitted as centre `<text>` AFTER the media pass, so
    // subtitles paint over everything else in the frame —
    // the raster burn-in composites its cues into the
    // finished frame the same way. The lookup is over
    // `Project::subtitles` under the half-open liveness rule
    // (this writer works from the project alone; there is no
    // core here): ALL live cues in stored order, stacked per
    // the layout contract `append_subtitles` documents. A
    // frame with no live cue appends nothing, so cue-free
    // projects stay byte-identical.
    let live_cues: Vec<&crate::subtitles::SubtitleCue> = project
        .subtitles
        .iter()
        .filter(|c| c.start_frame <= frame && frame < c.end_frame)
        .collect();
    if !live_cues.is_empty() {
        append_subtitles(&mut svg, project, &live_cues);
    }
    svg += "</svg>\n";
    let svg = svg.replace("<!--DEFS-->", &defs);
    std::fs::write(path, svg)
}

/// Subtitle emission (Wave P, P27): emit every live cue as
/// centre `<text>` stacked from the style's anchor edge.
///
/// Position is in STAGE coordinates, deliberately NOT
/// routed through the camera `view` closure: a subtitle is
/// frame furniture pinned to the picture, so camera pan,
/// zoom and rotation must not move it (the raster frame the
/// burn-in draws into is stage space as well). The viewBox
/// is exactly the stage, so stage coordinates are document
/// coordinates.
///
/// Style: the anchor conventions match this file's existing
/// `<text>` emission (media text in `append_media`) —
/// `text-anchor="middle"` with
/// `dominant-baseline="central"`. The face follows subtitle
/// convention rather than the media text's monospace:
/// sans-serif at 20 units times the project style's font
/// scale, the style's text colour as fill with its backing
/// colour as a stroke under `paint-order="stroke"` so the
/// glyphs stay legible over arbitrary scene content. Under
/// the default style (bottom, scale 1.0, white on black)
/// the emitted values are the house subtitle look.
///
/// Stacking: ONE `<text>` element per line — each line is
/// independently centred and positioned, no `tspan`/`dy`
/// bookkeeping. Lines stack away from the anchored edge in
/// cue order — the first cue closest to the edge: at the
/// bottom anchor a cue's LAST line sits closest to the
/// edge, so it always lands at the same position however
/// many lines or cues stack above it. That offset rule is
/// the shared layout contract the raster burn-in and the
/// stage draw with the same maths, computed by the core's
/// shared helper, `subtitles::subtitle_stack_line_offset`
/// (Wave P, P23; swapped in per IA-P27-1 in the wave
/// findings) — the pins below are the parity evidence.
///
/// The text is split on line boundaries BEFORE escaping:
/// [`xml_escape`] maps control characters (newline
/// included) to spaces, so escaping first would destroy
/// the line breaks. Each line is then escaped on its own,
/// as the module header requires of every text emission.
fn append_subtitles(
    svg: &mut String,
    project: &Project,
    cues: &[&crate::subtitles::SubtitleCue],
) {
    let style = project.subtitle_style;
    // The shared layout helpers directly (Wave S, IA-S20-1):
    // this writer previously restated their constants and
    // the sanitisation rule inline — arithmetically
    // identical and pinned equal; the restatement is
    // retired so the layout has exactly one home.
    let font_scale = style.sanitised().font_scale;
    let font_size = crate::subtitles::subtitle_font_size(style);
    let fill = hex(style.text_color);
    let stroke = hex(style.box_color);
    // The outline width scales with the font so a scaled
    // style keeps its outline proportion; `matrix_num`'s
    // trimmed formatting keeps the default's literal `3`.
    let stroke_width = matrix_num(3.0 * font_scale);
    let cx = project.stage[0] / 2.0;
    let mut lines_before = 0usize;
    for cue in cues {
        let lines: Vec<String> = crate::subtitles::subtitle_display_lines(cue);
        for (i, line) in lines.iter().enumerate() {
            // Distance of this line from the anchored edge,
            // in lines — the shared layout contract, computed
            // by the core's `subtitle_stack_line_offset`: at
            // the bottom anchor the cue's lines count DOWN
            // toward the edge — its last line is offset 0 —
            // while at the top they count away in their own
            // order.
            let from_edge = crate::subtitles::subtitle_stack_line_offset(
                lines_before,
                lines.len(),
                i,
                style.position,
            );
            // Centre of this line, from the shared helper
            // the stage and the raster also use: the block
            // sits one edge margin from the anchored edge and
            // the `from_edge` offset steps it one line height
            // further in; `dominant-baseline="central"`
            // centres the glyph box on this y.
            let y = crate::subtitles::subtitle_line_center_y(style, project.stage[1], from_edge);
            *svg += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"{:.1}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>\n",
                cx,
                y,
                font_size,
                fill,
                stroke,
                stroke_width,
                xml_escape(line)
            );
        }
        lines_before += lines.len();
    }
}

/// One matrix component for a media `<use>` transform:
/// formatted to 4 decimals with trailing zeros trimmed
/// (`1` for 1.0, `0.5` for 0.5, `0` for anything that rounds
/// to zero — including a negative zero, which is not useful
/// attribute data).
fn matrix_num(v: f32) -> String {
    let text = format!("{v:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" || text == "-" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// Media emission (Wave 12, C3): segment media and
/// free-standing sprite/text instances, from the same
/// `media::media_items` placement the rasterizer composites
/// — so the SVG and the raster agree on where every item
/// sits, and an item the raster skips (hidden figure,
/// missing or undecodable asset) is skipped here too.
///
/// Images: the asset's PNG bytes are embedded ONCE per asset
/// (first-use order) as an `<image>` in the defs, under the
/// generated id `asset{k}` — disjoint from the gradients'
/// `grad{n}` — and each item is a `<use>` whose matrix maps
/// the image's pixel space onto the item's quad in view
/// space: the a/b components are the first corner edge over
/// the pixel width, c/d the second edge over the pixel
/// height, e/f the first corner itself. Asset NAMES are
/// never emitted — bytes and generated ids only. An item
/// alpha below 1 (A6 §3a's figure × style product) rides as
/// an `opacity` attribute on the `<use>`.
///
/// Bent segment images (Wave X, X4; strip warp Wave Y,
/// Y9): an item whose `src_rect` is not the whole image is
/// one strip of a bent segment's warped quad (a half at
/// K = 2). Its matrix is the same affine with
/// the source rect folded in (the image's pixel space is
/// first mapped so the rect spans the unit square the quad
/// edges then carry), and the `<use>` is wrapped in a
/// `<g clip-path>` whose defs-side `<clipPath>` polygon is
/// the item's own view-space quad — the affine alone would
/// paint the rest of the image outside the quad. Each strip
/// emits its own `<use>`, so a bent image is K uses over
/// one shared asset def. Full-rect items keep the bare
/// `<use>` emission byte for byte. Honest divergences
/// from the raster, by construction: the raster stamps text
/// as 3×5 bitmap glyphs while SVG emits a real `<text>` in a
/// monospace font (the original content is preserved
/// verbatim through [`xml_escape`]; the centre agrees, the
/// advance width cannot), and `image-rendering="pixelated"`
/// is a hint a renderer may ignore, not a guarantee of the
/// raster's nearest-neighbour sampling. The MIME type
/// follows the stored bytes (Wave R, R9): the asset table
/// stores PNG or JPEG bytes (the import funnel validates
/// the magic), and the embed sniffs the same magic so a
/// JPEG asset is labelled `image/jpeg`, never mislabelled
/// as PNG.
///
/// Clip-def media (Wave 14, C4): since C1's per-node
/// fallback in `media_items` (docs/wave13/CLIP_MEDIA.md
/// §6.1), an item sourced from a movieclip definition
/// arrives through this same loop as an ordinary item —
/// its alpha already carries the instance-figure ×
/// clip-node-style product and its text colour the
/// clip-node fallback (§6.2) — so this writer
/// deliberately has NO clip-specific branch; the C4
/// tests below pin the equivalence.
///
/// Text: centred at the item position (`text-anchor` +
/// `dominant-baseline`), font size = glyph height in view
/// units, rotated by the item angle MINUS the camera angle —
/// the view already rotated the position, and a stage-space
/// direction at angle θ lands at screen angle θ − φ under
/// the camera convention `export::camera_rotate_offset`
/// implements (derived there, Wave 6). The item alpha
/// (the carrier node's style opacity — the same value
/// the stage applies to its glyph prims) rides as an
/// `opacity` attribute under the image arm's emission
/// rule: present only below 1.0, two decimals, clamped
/// (Wave R, R8 — the segment-text alpha gap P17 left).
fn append_media(
    svg: &mut String,
    defs: &mut String,
    project: &Project,
    frame: usize,
    view: &dyn Fn(V2) -> V2,
    cam_angle: f32,
    zoom: f32,
) {
    let mut asset_ids: Vec<usize> = Vec::new();
    let mut clip_count = 0usize;
    for item in crate::media::media_items(project, frame as f32) {
        match item {
            crate::media::MediaItem::Image {
                corners,
                asset,
                alpha,
                src_rect,
            } => {
                let Some((w, h)) = crate::media::png_dims(project, asset) else {
                    continue;
                };
                let Some(asset_rec) = project.assets.get(asset) else {
                    continue;
                };
                let k = match asset_ids.iter().position(|&a| a == asset) {
                    Some(k) => k,
                    None => {
                        let k = asset_ids.len();
                        asset_ids.push(asset);
                        // Wave R (R9): the MIME follows the
                        // stored bytes — JPEG SOI magic means
                        // `image/jpeg`, anything else the PNG
                        // the funnel also accepts.
                        let mime = if asset_rec.png.starts_with(&[0xFF, 0xD8, 0xFF]) {
                            "image/jpeg"
                        } else {
                            "image/png"
                        };
                        defs.push_str(&format!(
                            "<image id=\"asset{k}\" width=\"{w}\" height=\"{h}\" image-rendering=\"pixelated\" href=\"data:{mime};base64,{}\"/>\n",
                            crate::mcp_io::base64_encode(&asset_rec.png)
                        ));
                        k
                    }
                };
                let v: Vec<V2> = corners.iter().map(|c| view(*c)).collect();
                let opacity = if alpha < 1.0 {
                    format!(" opacity=\"{:.2}\"", alpha.clamp(0.0, 1.0))
                } else {
                    String::new()
                };
                if src_rect == [0.0, 0.0, 1.0, 1.0] {
                    *svg += &format!(
                        "<use href=\"#asset{k}\" transform=\"matrix({} {} {} {} {:.1} {:.1})\"{}/>\n",
                        matrix_num((v[1][0] - v[0][0]) / w as f32),
                        matrix_num((v[1][1] - v[0][1]) / w as f32),
                        matrix_num((v[3][0] - v[0][0]) / h as f32),
                        matrix_num((v[3][1] - v[0][1]) / h as f32),
                        v[0][0],
                        v[0][1],
                        opacity
                    );
                } else {
                    // Wave X (X4): one strip of a bent
                    // segment's warped quad. The matrix folds
                    // the source rect in: an image point p
                    // lands at v0 + ((p.x/w - u0)/du) * e1 +
                    // ((p.y/h - v0)/dv) * e2, so the rect —
                    // and only the rect — covers the quad;
                    // the clip polygon (the quad itself, in
                    // view space) cuts the rest of the image
                    // away.
                    let e1 = [v[1][0] - v[0][0], v[1][1] - v[0][1]];
                    let e2 = [v[3][0] - v[0][0], v[3][1] - v[0][1]];
                    let du = src_rect[2] - src_rect[0];
                    let dv = src_rect[3] - src_rect[1];
                    let n = clip_count;
                    clip_count += 1;
                    defs.push_str(&format!(
                        "<clipPath id=\"mclip{n}\"><polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\"/></clipPath>\n",
                        v[0][0], v[0][1], v[1][0], v[1][1], v[2][0], v[2][1], v[3][0], v[3][1],
                    ));
                    *svg += &format!(
                        "<g clip-path=\"url(#mclip{n})\"><use href=\"#asset{k}\" transform=\"matrix({} {} {} {} {:.1} {:.1})\"{}/></g>\n",
                        matrix_num(e1[0] / (du * w as f32)),
                        matrix_num(e1[1] / (du * w as f32)),
                        matrix_num(e2[0] / (dv * h as f32)),
                        matrix_num(e2[1] / (dv * h as f32)),
                        v[0][0] - (src_rect[0] / du) * e1[0] - (src_rect[1] / dv) * e2[0],
                        v[0][1] - (src_rect[0] / du) * e1[1] - (src_rect[1] / dv) * e2[1],
                        opacity
                    );
                }
            }
            crate::media::MediaItem::Text { pos, angle_deg, content, size, color, alpha } => {
                let q = view(pos);
                let rot = angle_deg - cam_angle;
                let rotate = if rot != 0.0 {
                    format!(" transform=\"rotate({:.1} {:.1} {:.1})\"", rot, q[0], q[1])
                } else {
                    String::new()
                };
                // Wave R (R8): the item alpha — the carrier
                // node's style opacity, the stage's rule —
                // emitted exactly as the Image arm above emits
                // its own: an `opacity` attribute only below
                // 1.0, two decimals, clamped at emission. At
                // full alpha the element is byte-identical to
                // the pre-alpha output.
                let opacity = if alpha < 1.0 {
                    format!(" opacity=\"{:.2}\"", alpha.clamp(0.0, 1.0))
                } else {
                    String::new()
                };
                *svg += &format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"monospace\" font-size=\"{:.1}\" fill=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\"{}{}>{}</text>\n",
                    q[0],
                    q[1],
                    size * zoom,
                    hex(color),
                    rotate,
                    opacity,
                    xml_escape(&content)
                );
            }
        }
    }
}

/// Clamp an authored range `from..=to` to the timeline,
/// mirroring `export::clamp_frame_range` rule for rule (the
/// original is private to export.rs): an empty project is an
/// error, both ends clamp to the last frame, and a range that
/// is inverted after clamping is an error.
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

/// Export the authored frames `from..=to` (inclusive, clamped
/// to the timeline) as one SVG file per frame in `dir`,
/// keeping each frame's authored index in its file name
/// (frame 2 exports as `frame-0003.svg`, as in a full SVG
/// export). Returns the number of files written; an empty
/// or inverted range is an error. Unlike the export.rs
/// version this renders ONLY the frames in the range, via
/// `write_frame_svg` — the files are byte-identical to the
/// whole-timeline export's files for the same frames.
pub fn export_svg_range(
    project: &Project,
    from: usize,
    to: usize,
    dir: &Path,
) -> std::io::Result<usize> {
    let (from, to) = clamp_frame_range(project, from, to)?;
    std::fs::create_dir_all(dir)?;
    for frame_idx in from..=to {
        write_frame_svg(
            project,
            frame_idx,
            &dir.join(format!("frame-{:04}.svg", frame_idx + 1)),
        )?;
    }
    Ok(to - from + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;


    /// Wave P (P17): the SVG writer takes the same evaluated
    /// (graph-channelled) frame as the rasterizer and the
    /// stage. A project whose channels move/lengthen/rotate
    /// the figure must emit byte-identically to the same
    /// project with those values baked into its poses and def,
    /// and differently from the un-channelled project. A Burst
    /// clip rides along so the FX anchor funnel is covered by
    /// the same comparison.
    #[test]
    fn svg_channelled_figure_matches_the_baked_equivalent() {
        use crate::graph::{Channel, ChannelTarget, Key};
        let base = || {
            let mut project = Project::new_demo();
            project.fx_clips.push(crate::model::FxClipEntry {
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
                node: 5,
                hidden: false,
            });
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
        let mut baked = base();
        for frame in &mut baked.frames {
            frame.poses[0].root[0] = 600.0;
            frame.poses[0].angles[1] = 45.0;
        }
        baked.figures[0].def.nodes[1].length = 99.0;

        let render = |project: &Project, name: &str, frame: usize| -> String {
            let dir = std::path::Path::new(name);
            let _ = std::fs::remove_dir_all(dir);
            // write_frame_svg does not create the parent dir.
            std::fs::create_dir_all(dir).unwrap();
            write_frame_svg(project, frame, &dir.join("frame.svg")).unwrap();
            std::fs::read_to_string(dir.join("frame.svg")).unwrap()
        };
        // Frame 2: the figure. Frame 0: the Burst clip's
        // particles sit exactly on the anchor, pinning the
        // FX anchor funnel as well.
        for frame in [2, 0] {
            let plain = render(&base(), "/tmp/styk-svg-chan-plain", frame);
            let via_channels = render(&channelled, "/tmp/styk-svg-chan-moved", frame);
            let via_bake = render(&baked, "/tmp/styk-svg-chan-baked", frame);
            assert_ne!(
                via_channels, plain,
                "channels must move the SVG figure (frame {frame})"
            );
            assert_eq!(
                via_channels, via_bake,
                "channelled SVG must equal the baked equivalent (frame {frame})"
            );
        }
    }

    #[test]
    fn svg_renders_gradient_polygons_and_square_caps() {
        let mut project = Project::new_demo();
        project.figures[0].def.nodes[2].style.color2 = Some(0x0000FF);
        project.figures[0].def.nodes[2].style.taper = 0.3;
        project.figures[0].def.nodes[3].style.cap = crate::model::CapKind::Square;
        project.figures[0].def.nodes[4].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-style-test");
        let n = export_svg(&project, dir).unwrap();
        assert_eq!(n, 5);
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(text.contains("<linearGradient"), "gradient def missing:\n{text}");
        assert!(text.contains("stop-color=\"#0000FF\""), "gradient end colour missing");
        assert!(text.contains("<polygon"), "styled segment polygon missing");
        assert!(text.contains("fill=\"url(#grad0)\""), "polygon not filled by the gradient");
        assert!(
            text.contains("stroke-linecap=\"square\""),
            "square cap missing"
        );
        assert!(text.contains("stroke-opacity=\"0.50\""), "opacity missing");
        // Default segments keep the classic round-cap line form.
        assert!(text.contains("stroke-linecap=\"round\""), "default lines changed");
    }

    #[test]
    fn svg_camera_rotation_rotates_points_like_the_rasterizer() {
        // Demo figure, frame 0: hips at (480, 470), chest joint at
        // (480, 424) — the hips->chest segment is a <line> with
        // those endpoints when the camera is unrotated.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        let dir = std::path::Path::new("/tmp/styk-svg-rot-test");
        export_svg(&project, dir).unwrap();
        let plain = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            plain.contains("x1=\"480.0\" y1=\"470.0\" x2=\"480.0\" y2=\"424.0\""),
            "unrotated chest segment missing:\n{plain}"
        );
        // +90 degrees about the camera centre (480, 360): the
        // hips' downward offset (0, 110) becomes a rightward one
        // (110, 0) — the Pivot sign convention (scene CCW).
        project.camera[0].angle = 90.0;
        export_svg(&project, dir).unwrap();
        let rotated = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            rotated.contains("x1=\"590.0\" y1=\"360.0\" x2=\"544.0\" y2=\"360.0\""),
            "rotated chest segment missing:\n{rotated}"
        );
    }

    #[test]
    fn svg_frames_are_real_vector_files() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-test");
        let n = export_svg(&project, dir).unwrap();
        assert_eq!(n, 5);
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(text.starts_with("<svg"), "not an svg: {}", &text[..40.min(text.len())]);
        assert!(text.contains("<line"), "figure segments missing");
    }

    #[test]
    fn svg_spray_stroke_exports_dots_not_a_polyline() {
        let mut project = Project::new_demo();
        let mark = crate::model::SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        };
        let spray_points = vec![[100.0, 100.0], [200.0, 120.0], [300.0, 100.0]];
        // A spray stroke: must come out as <circle> dots only.
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: spray_points.clone(),
            color: 0xC0FFEE,
            width: 14.0,
            widths: Vec::new(),
            fill: None,
            spray: Some(mark),
            opacity: 1.0,
            scatter: 0.0,
        });
        // A plain stroke on the same frame: unchanged polyline.
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 600.0], [300.0, 600.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-spray-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // The dot geometry comes from the single spray source;
        // the export must emit exactly one circle per dot.
        let dots = crate::paint::spray_dots(&spray_points, &mark);
        assert_eq!(dots.len(), spray_points.len() * 6, "spray_dots contract");
        assert_eq!(
            text.matches("fill=\"#C0FFEE\"").count(),
            dots.len(),
            "one <circle> per spray dot:\n{text}"
        );
        assert!(text.contains("fill-opacity="), "dots must carry their alpha");
        assert!(
            text.contains("r=\"7.0\""),
            "dot radius is (width * 0.5).max(1.0):\n{text}"
        );
        // The only polyline in the file is the plain stroke's —
        // the spray stroke did not draw one.
        assert_eq!(
            text.matches("<polyline").count(),
            1,
            "spray stroke drew a polyline:\n{text}"
        );
        assert!(text.contains("stroke=\"#123456\""), "plain stroke changed");
    }

    /// Sorted file names present in a directory (test helper).
    fn dir_file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn svg_range_writes_only_the_range_with_absolute_names_and_identical_bytes() {
        let project = Project::new_demo();
        let full_dir = std::path::Path::new("/tmp/styk-svg-range-full-test");
        let range_dir = std::path::Path::new("/tmp/styk-svg-range-only-test");
        let _ = std::fs::remove_dir_all(full_dir);
        let _ = std::fs::remove_dir_all(range_dir);
        let n_full = export_svg(&project, full_dir).unwrap();
        assert_eq!(n_full, 5);
        let n = export_svg_range(&project, 1, 3, range_dir).unwrap();
        assert_eq!(n, 3);
        // Only the range's files, under their absolute
        // whole-timeline names — frames 0 and 4 are absent,
        // so nothing outside the range was rendered/copied.
        assert_eq!(
            dir_file_names(range_dir).join(","),
            "frame-0002.svg,frame-0003.svg,frame-0004.svg"
        );
        // Byte-identical to the whole-timeline export's files
        // for the same frames: the range writer is the same
        // per-frame writer the full export is built on.
        for name in ["frame-0002.svg", "frame-0003.svg", "frame-0004.svg"] {
            assert_eq!(
                std::fs::read(range_dir.join(name)).unwrap(),
                std::fs::read(full_dir.join(name)).unwrap(),
                "{name} differs between range and full export"
            );
        }
    }

    #[test]
    fn svg_range_clamps_past_the_end_and_writes_single_frames() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-range-clamp-test");
        let _ = std::fs::remove_dir_all(dir);
        // `to` past the last frame clamps to it (frame index 4).
        let n = export_svg_range(&project, 3, 99, dir).unwrap();
        assert_eq!(n, 2);
        assert_eq!(
            dir_file_names(dir).join(","),
            "frame-0004.svg,frame-0005.svg"
        );
        // A one-frame range writes exactly that frame, under
        // its absolute name.
        let dir = std::path::Path::new("/tmp/styk-svg-range-one-test");
        let _ = std::fs::remove_dir_all(dir);
        let n = export_svg_range(&project, 2, 2, dir).unwrap();
        assert_eq!(n, 1);
        assert_eq!(dir_file_names(dir).join(","), "frame-0003.svg");
    }

    #[test]
    fn svg_range_rejects_inverted_ranges_and_empty_projects() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-range-err-test");
        // Inverted after clamping (4 > 1): an error, mirroring
        // export::clamp_frame_range.
        assert!(export_svg_range(&project, 4, 1, dir).is_err());
        let mut empty = Project::new_demo();
        empty.frames.clear();
        assert!(export_svg_range(&empty, 0, 0, dir).is_err());
    }

    /// A demo project plus one plain stroke on frame 0 with
    /// the given stroke opacity (test helper).
    fn stroke_project(opacity: f32) -> Project {
        let mut project = Project::new_demo();
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 600.0], [300.0, 600.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity,
            scatter: 0.0,
        });
        project
    }

    #[test]
    fn svg_stroke_opacity_below_one_emits_the_trimmed_attribute() {
        let dir = std::path::Path::new("/tmp/styk-svg-opacity-test");
        let project = stroke_project(0.5);
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // Exactly one trimmed "0.5" attribute in the file (the
        // node-style form would render "0.50", so this string
        // can only be the stroke's), on its own polyline.
        assert_eq!(text.matches("stroke-opacity=\"0.5\"").count(), 1, "{text}");
        let line = text
            .lines()
            .find(|l| l.contains("stroke=\"#123456\""))
            .expect("the test stroke's polyline");
        assert!(line.ends_with("stroke-opacity=\"0.5\"/>"), "{line}");
        // Quarter opacity keeps both decimals after trimming.
        let project = stroke_project(0.25);
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(text.contains("stroke-opacity=\"0.25\""), "{text}");
    }

    #[test]
    fn svg_stroke_opacity_one_emits_nothing() {
        let dir = std::path::Path::new("/tmp/styk-svg-opacity-one-test");
        for opacity in [1.0, 1.5] {
            let project = stroke_project(opacity);
            export_svg(&project, dir).unwrap();
            let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
            let line = text
                .lines()
                .find(|l| l.contains("stroke=\"#123456\""))
                .expect("the test stroke's polyline");
            // The classic element form with no attribute —
            // 1.5 clamps to fully opaque at read, so existing
            // output stays byte-identical.
            assert!(!line.contains("stroke-opacity"), "{line}");
            assert!(line.ends_with("stroke-linecap=\"round\"/>"), "{line}");
        }
    }

    #[test]
    fn xml_escape_escapes_the_five_significant_characters() {
        assert_eq!(xml_escape("plain text 123"), "plain text 123");
        assert_eq!(
            xml_escape("a<b>c&d\"e'f"),
            "a&lt;b&gt;c&amp;d&quot;e&apos;f"
        );
        // `&` escapes first: a pre-written entity double-escapes
        // rather than passing through as markup.
        assert_eq!(xml_escape("&lt;"), "&amp;lt;");
        assert_eq!(xml_escape(""), "");
    }

    #[test]
    fn svg_user_names_cannot_break_the_markup() {
        // MCP clients can set arbitrary project/figure names.
        // No name is interpolated into the SVG document (every
        // format argument is numeric or a hex colour), so a
        // hostile name can neither appear nor break the XML —
        // this pins that audit result against regressions.
        let mut project = Project::new_demo();
        project.name = "x</svg><script>alert(1)</script>".into();
        project.figures[0].name = "evil<name> & \"friends\"".into();
        let dir = std::path::Path::new("/tmp/styk-svg-xml-names-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(!text.contains("<script"), "{text}");
        assert!(!text.contains("evil"), "{text}");
        assert!(text.ends_with("</svg>\n"), "{text}");
    }

    #[test]
    fn svg_gradient_stops_are_both_self_closed() {
        // The first <stop> used to be left unclosed, making
        // every gradient-bearing SVG malformed XML (strict
        // parsers — resvg, browsers in XML mode — reject it).
        let mut project = Project::new_demo();
        project.figures[0].def.nodes[2].style.color2 = Some(0x0000FF);
        let dir = std::path::Path::new("/tmp/styk-svg-stop-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("/><stop offset=\"1\""),
            "first stop not self-closed:\n{text}"
        );
    }

    #[test]
    fn svg_spray_radius_scales_with_camera_zoom() {
        // The rasterizer scales spray/scatter dot radii by the
        // camera zoom; the SVG writer emitted the raw radius,
        // so at zoom 2 the dots rendered half their true size.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 2.0);
        let mark = crate::model::SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        };
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [200.0, 120.0]],
            color: 0xC0FFEE,
            width: 14.0,
            widths: Vec::new(),
            fill: None,
            spray: Some(mark),
            opacity: 1.0,
            scatter: 0.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-spray-zoom-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("r=\"14.0\""),
            "spray radius not zoom-scaled (7.0 * 2):\n{text}"
        );
    }

    #[test]
    fn svg_filled_stroke_exports_a_polygon_and_beats_scatter() {
        // Stage precedence (stage_layers.rs): spray, then
        // fill, then scatter, then the plain polyline. A
        // filled stroke draws as its filled polygon ONLY —
        // scatter must not turn it into dabs, and no polyline
        // outline is drawn for it.
        let mut project = Project::new_demo();
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [300.0, 100.0], [200.0, 300.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: Some(0x112233),
            spray: None,
            opacity: 0.5,
            scatter: 9.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-fill-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let poly = text
            .lines()
            .find(|l| l.contains("fill=\"#112233\""))
            .expect("filled stroke polygon missing");
        assert!(poly.starts_with("<polygon"), "{poly}");
        assert!(poly.contains("fill-opacity=\"0.50\""), "{poly}");
        // No polyline and no dab circles carry this stroke's
        // outline colour: fill won over scatter and plain.
        assert!(
            !text.contains("stroke=\"#123456\""),
            "filled stroke also drew its outline:\n{text}"
        );
        assert!(
            !text.contains("fill=\"#123456\""),
            "filled stroke also drew scatter dabs:\n{text}"
        );
    }

    #[test]
    fn svg_fx_clip_and_weather_discs_are_emitted() {
        // Wave 11 E8: FX clips and weather used to vanish
        // from SVG exports entirely. The disc count must grow
        // by exactly what the shared vfx sources produce.
        let plain = Project::new_demo();
        let plain_dir = std::path::Path::new("/tmp/styk-svg-fx-plain-test");
        export_svg(&plain, plain_dir).unwrap();
        let plain_text = std::fs::read_to_string(plain_dir.join("frame-0001.svg")).unwrap();
        let plain_circles = plain_text.matches("<circle").count();

        let mut project = Project::new_demo();
        let clip = crate::vfx::ParticleClip {
            start: 0.0,
            duration: 3.0,
            kind: crate::vfx::ClipKind::Burst,
            color: 0xFFC65D,
            color2: None,
            seed: 7,
            angle: 0.0,
            length: 130.0,
        };
        project.fx_clips.push(crate::model::FxClipEntry {
            clip,
            figure: 0,
            node: 0,
            hidden: false,
        });
        let weather = crate::vfx::Weather {
            kind: crate::vfx::WeatherKind::Snow,
            amount: 10.0,
            wind: 0.0,
        };
        project.weather = Some(weather);

        let poses = project.poses_at(0.0);
        let pos = crate::model::evaluate(&project.figures[0].def, &poses[0]);
        let clip_n = crate::vfx::clip_particles(&clip, 0.0, pos[0]).len();
        let weather_n =
            crate::vfx::weather_particles(&weather, 0.0, project.stage, project.fps).len();
        assert!(clip_n > 0, "burst emits discs at frame 0");
        assert!(weather_n > 0, "snow emits discs at frame 0");

        let dir = std::path::Path::new("/tmp/styk-svg-fx-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(
            text.matches("<circle").count(),
            plain_circles + clip_n + weather_n,
            "fx + weather discs missing or duplicated:\n{text}"
        );
        // A hidden clip emits nothing.
        project.fx_clips[0].hidden = true;
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(
            text.matches("<circle").count(),
            plain_circles + weather_n,
            "hidden fx clip emitted discs:\n{text}"
        );
    }

    #[test]
    fn svg_rich_project_serialises_byte_identically_twice() {
        // Determinism across every branch of the writer at
        // once (gradient defs, spray, scatter, fill, fx,
        // weather): the same project serialised twice must
        // produce the same bytes — no hash-order, clock or
        // RNG state may leak into the document. (The crate
        // integration suite pins the same for the demo
        // project; this pins it for the rich one.)
        let mut project = Project::new_demo();
        project.figures[0].def.nodes[2].style.color2 = Some(0x0000FF);
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [200.0, 120.0]],
            color: 0xC0FFEE,
            width: 14.0,
            widths: Vec::new(),
            fill: None,
            spray: Some(crate::model::SprayMark {
                seed: 7,
                radius: 12.0,
                opacity: 0.5,
            }),
            opacity: 1.0,
            scatter: 0.0,
        });
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [300.0, 100.0], [200.0, 300.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: Some(0x112233),
            spray: None,
            opacity: 0.5,
            scatter: 0.0,
        });
        project.fx_clips.push(crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 3.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFFC65D,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        });
        project.weather = Some(crate::vfx::Weather {
            kind: crate::vfx::WeatherKind::Snow,
            amount: 10.0,
            wind: 3.0,
        });
        // Wave 12 (C3): media joins the determinism pin — the
        // embedded bytes and generated ids must serialise
        // identically too.
        project.assets.push(crate::model::ImageAsset {
            name: "rich.png".into(),
            png: png_bytes(4, 2),
        });
        project.sprites.push(crate::model::SpriteInstance {
            asset: 0,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 500.0,
                y: 400.0,
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
        project.texts.push(crate::model::TextInstance {
            content: "Rich".into(),
            size: 18.0,
            color: 0xFF0000,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 480.0,
                y: 360.0,
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
        let dir_a = std::path::Path::new("/tmp/styk-svg-rich-det-a");
        let dir_b = std::path::Path::new("/tmp/styk-svg-rich-det-b");
        export_svg(&project, dir_a).unwrap();
        export_svg(&project, dir_b).unwrap();
        for name in ["frame-0001.svg", "frame-0003.svg", "frame-0005.svg"] {
            assert_eq!(
                std::fs::read(dir_a.join(name)).unwrap(),
                std::fs::read(dir_b.join(name)).unwrap(),
                "{name} differs between two serialisations"
            );
        }
    }

    #[test]
    fn svg_figure_opacity_composes_into_segment_and_head_alpha() {
        // Wave 12 (C1): the rasterizer composes figure (layer)
        // opacity multiplicatively into segment alpha
        // (style.opacity * fig.opacity, export.rs) and fades
        // heads/rings/shapes by the figure factor alone; the
        // SVG writer used to ignore fig.opacity entirely.
        let mut project = Project::new_demo();
        project.figures[0].opacity = 0.5;
        project.figures[0].def.nodes[4].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-fig-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // Product on the styled node: 0.5 * 0.5 = 0.25.
        assert!(
            text.contains("stroke-opacity=\"0.25\""),
            "style x figure opacity product missing:\n{text}"
        );
        // Figure factor alone on unstyled segments.
        assert!(
            text.contains("stroke-opacity=\"0.50\""),
            "figure opacity missing:\n{text}"
        );
        // …and on the head ring (a fill="none" circle), which
        // carries no style opacity of its own.
        let head = text
            .lines()
            .find(|l| l.contains("<circle") && l.contains("fill=\"none\""))
            .expect("head ring missing");
        assert!(head.contains("stroke-opacity=\"0.50\""), "{head}");
    }

    #[test]
    fn svg_tapered_segment_without_gradient_emits_a_tapered_polygon() {
        // Wave 12 (C1): a tapered segment with NO color2 used
        // to fall through to a uniform-width <line>; it must
        // emit the tapered quad with a solid fill, and no
        // gradient def may appear anywhere.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.figures[0].def.nodes[2].style.taper = 0.25;
        let dir = std::path::Path::new("/tmp/styk-svg-taper-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            !text.contains("<linearGradient"),
            "solid taper grew a gradient:\n{text}"
        );
        // Expected quad for node 2's segment, computed with
        // the exporter's own geometry (the camera key above
        // makes `view` the identity, as the rotation test
        // relies on): near half-width thickness/2, far
        // half-width a quarter of it — differing end widths.
        let poses = project.poses_at(0.0);
        let pos = evaluate(&project.figures[0].def, &poses[0]);
        let node = &project.figures[0].def.nodes[2];
        let a = pos[node.parent.unwrap()];
        let b = pos[2];
        let half_near = node.thickness / 2.0;
        let half_far = half_near * 0.25;
        assert!(half_far < half_near, "test needs differing end widths");
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(f32::EPSILON);
        let n = [-d[1] / len, d[0] / len];
        let pts = format!(
            "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            a[0] + n[0] * half_near,
            a[1] + n[1] * half_near,
            b[0] + n[0] * half_far,
            b[1] + n[1] * half_far,
            b[0] - n[0] * half_far,
            b[1] - n[1] * half_far,
            a[0] - n[0] * half_near,
            a[1] - n[1] * half_near
        );
        assert!(
            text.contains(&format!("<polygon points=\"{pts}\"")),
            "tapered solid quad missing (expected points {pts}):\n{text}"
        );
        // Filled with the node's solid colour, not a gradient
        // reference.
        let line = text
            .lines()
            .find(|l| l.contains(&pts))
            .expect("tapered quad line");
        assert!(line.contains("fill=\"#"), "{line}");
        assert!(!line.contains("url(#"), "{line}");
    }

    #[test]
    fn svg_bent_segment_routes_through_the_bend_midpoint() {
        // Wave 12 (C1): bent segments used to be drawn as one
        // straight line parent->child; the rasterizer draws
        // pieces through model::bend_mid (export.rs).
        // Wave Y (P-7): the chain is now FOUR pieces through
        // the arc's quarter points — the same joints
        // paint::styled_segment_pieces returns for this
        // segment, asserted below from both sides.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        let node_counts: Vec<usize> = project
            .figures
            .iter()
            .map(|f| f.def.nodes.len())
            .collect();
        project.frames[0].bends = node_counts.iter().map(|&n| vec![0.0; n]).collect();
        project.frames[0].bends[0][2] = 0.7;
        let dir = std::path::Path::new("/tmp/styk-svg-bend-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let poses = project.poses_at(0.0);
        let pos = evaluate(&project.figures[0].def, &poses[0]);
        let par = project.figures[0].def.nodes[2].parent.unwrap();
        let (a, b) = (pos[par], pos[2]);
        let mid = crate::model::bend_mid(a, b, 0.7);
        let q1 = crate::model::bend_mid(a, mid, 0.35);
        let q3 = crate::model::bend_mid(mid, b, 0.35);
        // The bend actually displaces the midpoint off the
        // chord, so the routing assertions below can bite.
        assert_ne!(
            format!("{:.1},{:.1}", mid[0], mid[1]),
            format!("{:.1},{:.1}", (a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0),
            "test bend must displace the midpoint"
        );
        // Piece 2 ends at bend_mid; piece 3 starts there.
        assert!(
            text.contains(&format!("x2=\"{:.1}\" y2=\"{:.1}\"", mid[0], mid[1])),
            "second bent piece does not end at bend_mid:\n{text}"
        );
        assert!(
            text.contains(&format!("x1=\"{:.1}\" y1=\"{:.1}\"", mid[0], mid[1])),
            "third bent piece does not start at bend_mid:\n{text}"
        );
        // Cross-surface parity (Wave Y, P-7): the generator
        // the stage and rasterizer use returns exactly four
        // pieces for this segment, with joints a, q1, mid,
        // q3, b — and the SVG text carries each of the four
        // as a line with those exact endpoints.
        let node = &project.figures[0].def.nodes[2];
        let gen = crate::paint::styled_segment_pieces(
            a,
            b,
            0.7,
            node.thickness,
            &node.style,
            node.color.unwrap_or(project.figures[0].color),
        );
        assert_eq!(gen.len(), 4, "generator piece count for the bent segment");
        let joints = [a, q1, mid, q3, b];
        for (k, w) in joints.windows(2).enumerate() {
            assert_eq!(gen[k].a, w[0], "generator joint {k} (near)");
            assert_eq!(gen[k].b, w[1], "generator joint {k} (far)");
            assert!(
                text.contains(&format!(
                    "x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"",
                    w[0][0], w[0][1], w[1][0], w[1][1]
                )),
                "svg piece {k} does not run {:?} -> {:?}:\n{text}",
                w[0],
                w[1]
            );
        }
        // The straight parent->child line for this segment is
        // gone.
        assert!(
            !text.contains(&format!(
                "x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"",
                a[0], a[1], b[0], b[1]
            )),
            "bent segment still drawn straight:\n{text}"
        );
    }

    #[test]
    fn svg_bend_channel_routes_the_segment_without_frame_bends() {
        // Wave Y (P-7): the writer's bends come from the
        // channelled evaluation's rows (Y1's core), so a Bend
        // channel ALONE — no Frame.bends authored anywhere —
        // bends the exported segment through the same arc
        // midpoint. Before the routing, this export was
        // straight: the writer read bends_at, which knows
        // nothing of channels.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.channels.push(crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::Bend(2),
            keys: vec![crate::graph::Key::flat(0.0, 0.7)],
        });
        let dir = std::path::Path::new("/tmp/styk-svg-bend-channel-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // Bend channels are render-only: the evaluated
        // positions are the raw poses, exactly as the
        // frame-bend test above computes them.
        let poses = project.poses_at(0.0);
        let pos = evaluate(&project.figures[0].def, &poses[0]);
        let par = project.figures[0].def.nodes[2].parent.unwrap();
        let (a, b) = (pos[par], pos[2]);
        let mid = crate::model::bend_mid(a, b, 0.7);
        assert!(
            text.contains(&format!("x2=\"{:.1}\" y2=\"{:.1}\"", mid[0], mid[1])),
            "bend channel does not reach the svg segment:\n{text}"
        );
        assert!(
            !text.contains(&format!(
                "x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"",
                a[0], a[1], b[0], b[1]
            )),
            "bend-channel segment still drawn straight:\n{text}"
        );
    }

    // ---- Wave 12 (C9): pressured strokes ----

    fn pressured_project(widths: Vec<f32>, opacity: f32) -> Project {
        let mut project = Project::new_demo();
        // Identity view: the camera key sits at the stage
        // centre, so view(p) == p for every point below.
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 100.0], [200.0, 100.0], [200.0, 200.0]],
            color: 0x112233,
            width: 4.0,
            widths,
            fill: None,
            spray: None,
            opacity,
            scatter: 0.0,
        });
        project
    }

    #[test]
    fn svg_pressured_stroke_emits_per_segment_lines_at_averaged_widths() {
        // Widths 2 / 10 / 4 bake per-point; the rasterizer
        // averages each segment's endpoints, so the segments
        // draw at (2+10)/2 = 6.0 and (10+4)/2 = 7.0.
        let project = pressured_project(vec![2.0, 10.0, 4.0], 1.0);
        let dir = std::path::Path::new("/tmp/styk-svg-pressured-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<line x1=\"100.0\" y1=\"100.0\" x2=\"200.0\" y2=\"100.0\" stroke=\"#112233\" stroke-width=\"6.0\" stroke-linecap=\"round\"/>"),
            "first pressured segment missing:\n{text}"
        );
        assert!(
            text.contains("<line x1=\"200.0\" y1=\"100.0\" x2=\"200.0\" y2=\"200.0\" stroke=\"#112233\" stroke-width=\"7.0\" stroke-linecap=\"round\"/>"),
            "second pressured segment missing:\n{text}"
        );
        // The single-polyline form is gone for this stroke.
        assert!(
            !text.contains("points=\"100.0,100.0 200.0,100.0 200.0,200.0\""),
            "pressured stroke still emitted as one polyline:\n{text}"
        );
    }

    #[test]
    fn svg_pressured_stroke_degenerate_segment_emits_a_disc() {
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[150.0, 150.0], [150.0, 150.0], [250.0, 150.0]],
            color: 0x112233,
            width: 4.0,
            widths: vec![4.0, 4.0, 4.0],
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-pressured-degenerate-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // The doubled point has no line: a disc of the
        // averaged width (4.0 → r 2.0), as the raster's round
        // cap paints it.
        assert!(
            text.contains("<circle cx=\"150.0\" cy=\"150.0\" r=\"2.0\" fill=\"#112233\"/>"),
            "degenerate segment disc missing:\n{text}"
        );
        assert!(
            text.contains("<line x1=\"150.0\" y1=\"150.0\" x2=\"250.0\" y2=\"150.0\" stroke=\"#112233\" stroke-width=\"4.0\" stroke-linecap=\"round\"/>"),
            "surviving segment missing:\n{text}"
        );
    }

    #[test]
    fn svg_pressured_stroke_opacity_rides_every_segment() {
        let project = pressured_project(vec![2.0, 10.0, 4.0], 0.5);
        let dir = std::path::Path::new("/tmp/styk-svg-pressured-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("stroke-width=\"6.0\" stroke-linecap=\"round\" stroke-opacity=\"0.5\"/>"),
            "first segment lost the stroke opacity:\n{text}"
        );
        assert!(
            text.contains("stroke-width=\"7.0\" stroke-linecap=\"round\" stroke-opacity=\"0.5\"/>"),
            "second segment lost the stroke opacity:\n{text}"
        );
    }

    #[test]
    fn svg_unpressured_stroke_keeps_the_single_polyline() {
        // No baked widths: the pre-C9 form, byte for byte.
        let project = pressured_project(Vec::new(), 1.0);
        let dir = std::path::Path::new("/tmp/styk-svg-unpressured-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<polyline points=\"100.0,100.0 200.0,100.0 200.0,200.0\" fill=\"none\" stroke=\"#112233\" stroke-width=\"4.0\" stroke-linecap=\"round\"/>"),
            "unpressured polyline changed:\n{text}"
        );
    }

    #[test]
    fn svg_nonparallel_widths_fall_back_to_the_polyline() {
        // Widths that are not parallel to the points are not
        // a pressure profile (the rasterizer's predicate) —
        // the flat polyline form applies.
        let project = pressured_project(vec![1.0, 2.0], 1.0);
        let dir = std::path::Path::new("/tmp/styk-svg-nonparallel-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<polyline points=\"100.0,100.0 200.0,100.0 200.0,200.0\""),
            "non-parallel widths did not fall back:\n{text}"
        );
    }

    // ---- Wave 12 (C3): media emission ----

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 255, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn media_project() -> Project {
        let mut project = Project::new_demo();
        // Identity view (camera key at the stage centre).
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.assets.push(crate::model::ImageAsset {
            name: "dot.png".into(),
            png: png_bytes(4, 2),
        });
        project.sprites.push(crate::model::SpriteInstance {
            asset: 0,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 500.0,
                y: 400.0,
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
    fn svg_media_sprite_emits_a_use_with_the_placement_matrix() {
        // Sprite: 4×2 px at scale 1 centred (500, 400) → the
        // quad's first corner is (498, 399) and the matrix is
        // the identity scale about it.
        let project = media_project();
        let dir = std::path::Path::new("/tmp/styk-svg-media-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<image id=\"asset0\" width=\"4\" height=\"2\""),
            "asset def missing:\n{text}"
        );
        assert!(
            text.contains("href=\"data:image/png;base64,"),
            "asset bytes not embedded:\n{text}"
        );
        assert!(
            text.contains("<use href=\"#asset0\" transform=\"matrix(1 0 0 1 498.0 399.0)\"/>"),
            "placement matrix wrong:\n{text}"
        );
        // The asset's NAME is never emitted — bytes and the
        // generated id only.
        assert!(!text.contains("dot.png"), "asset name leaked:\n{text}");
    }

    #[test]
    fn svg_media_assets_dedupe_by_first_use() {
        let mut project = media_project();
        project.sprites.push(crate::model::SpriteInstance {
            asset: 0,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 300.0,
                y: 200.0,
                scale: 2.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let dir = std::path::Path::new("/tmp/styk-svg-media-dedupe-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(text.matches("<image id=").count(), 1, "asset embedded twice:\n{text}");
        assert_eq!(text.matches("<use href=\"#asset0\"").count(), 2, "expected two uses:\n{text}");
        // The scaled instance: 4×2 px at scale 2 centred
        // (300, 200) → corner (296, 198), matrix scale 2.
        assert!(
            text.contains("transform=\"matrix(2 0 0 2 296.0 198.0)\""),
            "scaled placement wrong:\n{text}"
        );
    }

    #[test]
    fn svg_media_text_emits_escaped_centred_text() {
        let mut project = media_project();
        project.texts.push(crate::model::TextInstance {
            content: "A<B & C".into(),
            size: 20.0,
            color: 0xFF0000,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 480.0,
                y: 360.0,
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
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<text x=\"480.0\" y=\"360.0\" font-family=\"monospace\" font-size=\"20.0\" fill=\"#FF0000\" text-anchor=\"middle\" dominant-baseline=\"central\">A&lt;B &amp; C</text>"),
            "text element wrong or unescaped:\n{text}"
        );
    }

    #[test]
    fn svg_media_text_rotation_subtracts_the_camera_angle() {
        let mut project = media_project();
        project.texts.push(crate::model::TextInstance {
            content: "R".into(),
            size: 20.0,
            color: 0xFF0000,
            keys: vec![crate::model::InstanceKey {
                frame: 0.0,
                x: 480.0,
                y: 360.0,
                scale: 1.0,
                angle: 30.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        // Camera +90 about its own centre: the centre point
        // stays put, and a 30° stage direction lands at
        // 30 − 90 = −60 on screen.
        project.camera[0].angle = 90.0;
        let dir = std::path::Path::new("/tmp/styk-svg-media-rot-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("transform=\"rotate(-60.0 480.0 360.0)\""),
            "text rotation did not subtract the camera angle:\n{text}"
        );
    }

    #[test]
    fn svg_media_segment_image_carries_the_opacity_product() {
        let mut project = media_project();
        project.figures[0].def.nodes[2].media =
            Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        project.figures[0].opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-media-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // The segment image's <use> — and only it; the sprite
        // instance rides at alpha 1 and carries no attribute.
        assert_eq!(
            text.matches(" opacity=\"0.50\"").count(),
            1,
            "segment media opacity product missing or leaked:\n{text}"
        );
    }

    #[test]
    fn svg_bent_segment_image_emits_two_clipped_uses() {
        // Wave X (X4, GAP P-2): the instance's arm segment
        // (100,100)->(200,100) carries the image and bends
        // by 0.9, so the funnel yields two half-quad items
        // and this writer emits one clipped <use> per half
        // over a single shared asset def — the unbent
        // single-use form is gone for this segment.
        let mut project = clip_media_project(
            (Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), None),
            (None, None),
        );
        project.frames[0].bends = vec![vec![0.0, 0.9, 0.0]];
        let dir = std::path::Path::new("/tmp/styk-svg-bent-media-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(
            text.matches("<image id=").count(),
            1,
            "the two halves share one asset def:\n{text}"
        );
        assert_eq!(
            text.matches("<use href=\"#asset0\"").count(),
            2,
            "expected two half uses:\n{text}"
        );
        assert_eq!(
            text.matches("<g clip-path=\"url(#mclip").count(),
            2,
            "each half use is clipped to its quad:\n{text}"
        );
        // The clip polygons are the two half-quads: outer
        // corners are the straight quad's own (height 50
        // about the segment), the shared edge is bend_mid
        // displaced by the half-height.
        let arc_mid = crate::model::bend_mid([100.0, 100.0], [200.0, 100.0], 0.9);
        let lo = format!("{:.1},{:.1}", arc_mid[0], arc_mid[1] - 25.0);
        let hi = format!("{:.1},{:.1}", arc_mid[0], arc_mid[1] + 25.0);
        assert!(
            text.contains(&format!(
                "<clipPath id=\"mclip0\"><polygon points=\"100.0,75.0 {lo} {hi} 100.0,125.0\"/></clipPath>"
            )),
            "first half clip wrong:\n{text}"
        );
        assert!(
            text.contains(&format!(
                "<clipPath id=\"mclip1\"><polygon points=\"{lo} 200.0,75.0 200.0,125.0 {hi}\"/></clipPath>"
            )),
            "second half clip wrong:\n{text}"
        );
        // Half 1's matrix: the a-component is still the
        // quad-edge over the source-half width (50 / (0.5*4)
        // = 25), the b-component carries the bend slope, and
        // the translation is the first corner (the source
        // rect's origin folds to zero for the left half).
        let slope = matrix_num((arc_mid[1] - 100.0) / 2.0);
        assert!(
            text.contains(&format!(
                "<use href=\"#asset0\" transform=\"matrix(25 {slope} 0 25 100.0 75.0)\"/>"
            )),
            "first half matrix wrong:\n{text}"
        );
        // The straight full-quad use must NOT appear.
        assert!(
            !text.contains("<use href=\"#asset0\" transform=\"matrix(25 0 0 25 100.0 75.0)\"/>"),
            "bent segment still emitted as one straight use:\n{text}"
        );
    }

    #[test]
    fn svg_bent_segment_image_emits_one_clipped_use_per_strip() {
        // Wave Y (Y9, GAP P-11): at bend 2.0 the funnel warps
        // the arm's image into K = 4 strips; this writer emits
        // one clipped <use> per strip over the one shared
        // asset def, so the SVG strip count equals the
        // funnel's item count for the same project — the
        // surfaces share the stream and cannot drift.
        let mut project = clip_media_project(
            (Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), None),
            (None, None),
        );
        project.frames[0].bends = vec![vec![0.0, 2.0, 0.0]];
        let funnel_count = crate::media::media_items(&project, 0.0)
            .iter()
            .filter(|i| matches!(i, crate::media::MediaItem::Image { .. }))
            .count();
        assert_eq!(funnel_count, 4, "bend 2.0 warps into 4 funnel strips");
        let dir = std::path::Path::new("/tmp/styk-svg-strip-media-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(
            text.matches("<image id=").count(),
            1,
            "the strips share one asset def:\n{text}"
        );
        assert_eq!(
            text.matches("<use href=\"#asset0\"").count(),
            funnel_count,
            "one <use> per funnel strip:\n{text}"
        );
        assert_eq!(
            text.matches("<g clip-path=\"url(#mclip").count(),
            funnel_count,
            "each strip use is clipped to its quad:\n{text}"
        );
        for n in 0..funnel_count {
            assert!(
                text.contains(&format!("<clipPath id=\"mclip{n}\">")),
                "strip {n} has its own clip def:\n{text}"
            );
        }
    }

    #[test]
    fn svg_media_segment_text_carries_the_carrier_style_opacity() {
        // Wave R (R8 — P17's remainder): segment text used
        // to export fully opaque whatever the carrier node's
        // style opacity said, while the stage faded the
        // label with its segment. The Text item now carries
        // that opacity and this writer emits it under the
        // image arm's rule.
        let mut project = media_project();
        project.figures[0].def.nodes[2].media = Some(crate::model::SegmentMedia::Text {
            content: "HI".into(),
            size: 20.0,
        });
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(
            text.matches(" opacity=\"0.50\"").count(),
            1,
            "segment text must carry the carrier style opacity exactly once:\n{text}"
        );
        let line = text
            .lines()
            .find(|l| l.contains(">HI</text>"))
            .expect("the segment text element");
        assert!(
            line.contains(" opacity=\"0.50\""),
            "the opacity must ride the <text> element itself:\n{line}"
        );
    }

    #[test]
    fn svg_media_segment_text_ignores_figure_opacity() {
        // The stage's settled rule (stage_text's module
        // docs): figure opacity is a colour blend on the
        // stage and a segment-alpha factor in export — it
        // is NEVER multiplied into text, on any surface.
        // The text alpha is the carrier style opacity alone,
        // unlike the segment-image product beside it.
        let mut project = media_project();
        project.figures[0].def.nodes[2].media = Some(crate::model::SegmentMedia::Text {
            content: "HI".into(),
            size: 20.0,
        });
        project.figures[0].opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-fig-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let line = text
            .lines()
            .find(|l| l.contains(">HI</text>"))
            .expect("the segment text element");
        assert!(
            !line.contains("opacity"),
            "figure opacity leaked into segment text:\n{line}"
        );
        // Both factors at 0.5: the text still reads the
        // style's 0.50, not the 0.25 product.
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let line = text
            .lines()
            .find(|l| l.contains(">HI</text>"))
            .expect("the segment text element");
        assert!(
            line.contains(" opacity=\"0.50\""),
            "text alpha must be the style opacity alone:\n{line}"
        );
        assert!(
            !line.contains("opacity=\"0.25\""),
            "text alpha became the figure x style product:\n{line}"
        );
    }

    #[test]
    fn svg_media_segment_text_element_is_byte_identical_at_full_opacity() {
        // At the default style the alpha attribute must not
        // appear at all: projects whose text sits on
        // opacity-1 nodes (every pre-Wave-R file) export
        // their <text> elements byte-identically. Proven on
        // the element itself: the half-opacity element minus
        // its one attribute equals the full-opacity element
        // byte for byte. (The segment STROKE legitimately
        // differs between the two files — node style opacity
        // has always driven stroke-opacity in this writer —
        // so the claim is scoped to the text element, the
        // surface this wave changed.)
        let build = |opacity: f32| {
            let mut project = media_project();
            project.figures[0].def.nodes[2].media = Some(crate::model::SegmentMedia::Text {
                content: "HI".into(),
                size: 20.0,
            });
            project.figures[0].def.nodes[2].style.opacity = opacity;
            project
        };
        let text_line = |svg_text: &str| {
            svg_text
                .lines()
                .find(|l| l.contains(">HI</text>"))
                .expect("the segment text element")
                .to_string()
        };
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-alpha-diff-test");
        export_svg(&build(1.0), dir).unwrap();
        let full = text_line(&std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap());
        export_svg(&build(0.5), dir).unwrap();
        let half = text_line(&std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap());
        assert!(!full.contains("opacity"), "full-opacity text gained an attribute:\n{full}");
        assert_eq!(half.replace(" opacity=\"0.50\"", ""), full);
    }

    #[test]
    fn svg_media_segment_text_alpha_survives_save_load() {
        // The alpha's source is the stored node style, so a
        // save/load round trip re-derives the identical
        // item: the exported file is byte-identical after
        // the trip and the item alpha is still 0.5.
        let mut project = media_project();
        project.figures[0].def.nodes[2].media = Some(crate::model::SegmentMedia::Text {
            content: "HI".into(),
            size: 20.0,
        });
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-roundtrip-test");
        export_svg(&project, dir).unwrap();
        let before = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let json = serde_json::to_string(&project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        export_svg(&back, dir).unwrap();
        let after = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert_eq!(before, after, "save/load changed the segment-text alpha");
        let alpha = crate::media::media_items(&back, 0.0)
            .iter()
            .find_map(|i| match i {
                crate::media::MediaItem::Text { alpha, .. } => Some(*alpha),
                _ => None,
            })
            .expect("a text item");
        assert_eq!(alpha, 0.5);
    }

    #[test]
    fn svg_media_segment_text_legacy_project_without_style_loads_opaque() {
        // Legacy shape: a project file from before node
        // styles were stored carries no `style` object on
        // its nodes (NodeDef.style is serde-defaulted).
        // Such a file's segment text loads at alpha 1.0 and
        // its element exports exactly as it always did —
        // no attribute.
        let mut project = media_project();
        project.figures[0].def.nodes[2].media = Some(crate::model::SegmentMedia::Text {
            content: "HI".into(),
            size: 20.0,
        });
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        let mut value = serde_json::to_value(&project).unwrap();
        value["figures"][0]["def"]["nodes"][2]
            .as_object_mut()
            .unwrap()
            .remove("style");
        let back: Project = serde_json::from_value(value).unwrap();
        let alpha = crate::media::media_items(&back, 0.0)
            .iter()
            .find_map(|i| match i {
                crate::media::MediaItem::Text { alpha, .. } => Some(*alpha),
                _ => None,
            })
            .expect("a text item");
        assert_eq!(alpha, 1.0, "a style-less legacy node must load at alpha 1.0");
        let dir = std::path::Path::new("/tmp/styk-svg-media-text-legacy-test");
        export_svg(&back, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let line = text
            .lines()
            .find(|l| l.contains(">HI</text>"))
            .expect("the segment text element");
        assert!(!line.contains("opacity"), "legacy text gained an attribute:\n{line}");
    }

    #[test]
    fn svg_media_free_project_emits_no_media() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-media-free-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(!text.contains("<use"), "media-free project gained a <use>:\n{text}");
        assert!(!text.contains("asset0"), "media-free project gained an asset def:\n{text}");
        assert!(!text.contains("<text"), "media-free project gained a <text>:\n{text}");
    }

    // ---- Wave 13 (C1): z-order + S10 heads/shapes ----

    #[test]
    fn svg_strokes_emit_after_figures_and_before_media() {
        // Z-order: the rasterizer's segment pass composites
        // strokes OVER the figure segments and its media
        // pass composites over everything; this writer used
        // to emit its stroke block FIRST, putting every
        // stroke under the figures in an SVG renderer. Pin
        // the document order deliberately: head ring (in the
        // figure block) < stroke polyline < media <use>.
        let mut project = media_project();
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 600.0], [300.0, 600.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-zorder-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let head_pos = text.find("fill=\"none\"").expect("head ring missing");
        let stroke_pos = text.find("stroke=\"#123456\"").expect("stroke polyline missing");
        let media_pos = text.find("<use href=\"#asset0\"").expect("media use missing");
        assert!(
            head_pos < stroke_pos,
            "stroke still emits under the figures:\n{text}"
        );
        assert!(
            stroke_pos < media_pos,
            "media no longer composites last:\n{text}"
        );
    }

    #[test]
    fn svg_default_project_bytes_are_untouched_by_the_reorder() {
        // The zero-styled baseline both Wave 13 changes must
        // leave byte-identical: the demo project has NO
        // strokes (the moved stroke block emits zero bytes)
        // and every opacity product is 1.0 (the S10
        // composition emits no attribute), so its frames may
        // carry no stroke element and no opacity attribute
        // at all. (Exact segment bytes for this project are
        // pinned by the rotation test above.)
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-default-clean-test");
        export_svg(&project, dir).unwrap();
        for name in ["frame-0001.svg", "frame-0003.svg", "frame-0005.svg"] {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            assert!(!text.contains("<polyline"), "{name} gained a stroke:\n{text}");
            assert!(
                !text.contains("stroke-opacity"),
                "{name} gained a stroke-opacity:\n{text}"
            );
            assert!(
                !text.contains("fill-opacity"),
                "{name} gained a fill-opacity:\n{text}"
            );
        }
    }

    #[test]
    fn svg_node_style_opacity_reaches_head_ring_and_shapes() {
        // S10 SVG half: a head or shape node styled
        // translucent used to draw at the figure factor
        // alone (fully opaque at figure opacity 1.0). Node
        // 2 is the demo head; node 3 becomes a Circle shape
        // node, node 5 a Polygon — each styled to 0.5.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        project.figures[0].def.nodes[3].shape = crate::model::Shape::Circle;
        project.figures[0].def.nodes[3].style.opacity = 0.5;
        project.figures[0].def.nodes[5].shape = crate::model::Shape::Polygon(5);
        project.figures[0].def.nodes[5].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-s10-shapes-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        // Head ring: the fill="none" circle at the head
        // ring's own radius (17.0 at zoom 1).
        let head_r = format!("r=\"{:.1}\"", project.head_ring_radius());
        let head = text
            .lines()
            .find(|l| l.contains("<circle") && l.contains(&head_r))
            .expect("head ring missing");
        assert!(
            head.contains("stroke-opacity=\"0.50\""),
            "head ring lost the node style opacity: {head}"
        );
        // Circle shape: the fill="none" circle at
        // length / 2 = 19.0 (node 3's length is 38).
        let ring = text
            .lines()
            .find(|l| l.contains("<circle") && l.contains("r=\"19.0\""))
            .expect("circle shape missing");
        assert!(
            ring.contains("stroke-opacity=\"0.50\""),
            "circle shape lost the node style opacity: {ring}"
        );
        // Polygon shape: the only fill="none" polygon (no
        // segment in this project takes the polygon form).
        let poly = text
            .lines()
            .find(|l| l.contains("<polygon") && l.contains("fill=\"none\""))
            .expect("polygon shape missing");
        assert!(
            poly.contains("stroke-opacity=\"0.50\""),
            "polygon shape lost the node style opacity: {poly}"
        );
    }

    #[test]
    fn svg_node_style_opacity_composes_with_figure_opacity_on_heads() {
        // The product rule, the same as segments: a head
        // node at style 0.5 under a figure at 0.5 rides at
        // 0.25 — not at the figure factor alone (0.50, the
        // pre-Wave-13 byte) and not at the style alone.
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.figures[0].opacity = 0.5;
        project.figures[0].def.nodes[2].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-s10-compose-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let head_r = format!("r=\"{:.1}\"", project.head_ring_radius());
        let head = text
            .lines()
            .find(|l| l.contains("<circle") && l.contains(&head_r))
            .expect("head ring missing");
        assert!(
            head.contains("stroke-opacity=\"0.25\""),
            "head ring is not the style x figure product: {head}"
        );
    }

    // ---- Wave 14 (C4): clip-def media emission ----
    //
    // docs/wave13/CLIP_MEDIA.md §6: clip-def media reaches
    // every renderer through the ONE funnel — C1's per-node
    // fallback in `media::media_items` yields clip-sourced
    // items as ordinary `MediaItem`s, so `append_media`
    // needs (and has) no clip-specific branch. This section
    // pins that the emission treats them exactly like
    // figure-sourced media: same def + <use> + matrix, same
    // opacity product, same escaped <text>, media still
    // last in the Wave 13 z-order.

    fn node(
        name: &str,
        parent: Option<usize>,
        length: f32,
        media: Option<crate::model::SegmentMedia>,
    ) -> crate::model::NodeDef {
        crate::model::NodeDef {
            min_angle: None,
            max_angle: None,
            name: name.into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: crate::model::NodeStyle::default(),
            media,
        }
    }

    /// The media.rs C1 skeleton (root at (100, 100), node 1
    /// running +x for 100, node 2 running down for 80) as a
    /// movieclip instance of clip 0: the instance def and
    /// the clip def are built from the same skeleton with
    /// independent media, and the clip's single frame pose
    /// equals the timeline pose, so the geometry is the
    /// skeleton's own. Asset 0 is the shared 4×2 green PNG
    /// (aspect 2.0); the view is the identity camera.
    fn clip_media_project(
        inst_media: (
            Option<crate::model::SegmentMedia>,
            Option<crate::model::SegmentMedia>,
        ),
        clip_media: (
            Option<crate::model::SegmentMedia>,
            Option<crate::model::SegmentMedia>,
        ),
    ) -> Project {
        let skeleton = |m: (
            Option<crate::model::SegmentMedia>,
            Option<crate::model::SegmentMedia>,
        )| crate::model::FigureDef {
            nodes: vec![
                node("root", None, 0.0, None),
                node("arm", Some(0), 100.0, m.0),
                node("leg", Some(0), 80.0, m.1),
            ],
        };
        let pose = || crate::model::Pose {
            root: [100.0, 100.0],
            angles: vec![0.0, 90.0, 0.0],
        };
        let mut project = Project::new_demo();
        project.set_camera_key(0, 480.0, 360.0, 1.0);
        project.assets.push(crate::model::ImageAsset {
            name: "clipdot.png".into(),
            png: png_bytes(4, 2),
        });
        project.figures[0].color = 0xFF0000;
        project.figures[0].def = skeleton(inst_media);
        project.figures[0].clip = Some(0);
        project.frames = vec![crate::model::Frame {
            poses: vec![pose()],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }];
        project.clips = vec![crate::model::ClipDef {
            name: "clip".into(),
            def: skeleton(clip_media),
            frames: vec![pose()],
            loop_mode: crate::model::ClipLoop::Loop,
        }];
        project
    }

    #[test]
    fn svg_clip_def_image_emits_def_use_with_matrix_after_strokes() {
        // The media exists ONLY in the clip def; the
        // instance's own def is media-free. The funnel's
        // fallback yields one image item on node 1's
        // segment — (100,100) to (200,100), length 100,
        // asset aspect 2, so the quad is (100,75) to
        // (200,125) and the <use> matrix maps the 4×2
        // pixel space onto it: (25 0 0 25) about the
        // first corner.
        let mut project = clip_media_project(
            (None, None),
            (Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), None),
        );
        project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: vec![[100.0, 600.0], [300.0, 600.0]],
            color: 0x123456,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let dir = std::path::Path::new("/tmp/styk-svg-clip-img-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<image id=\"asset0\" width=\"4\" height=\"2\""),
            "clip asset def missing:\n{text}"
        );
        assert!(
            text.contains("href=\"data:image/png;base64,"),
            "clip asset bytes not embedded:\n{text}"
        );
        assert_eq!(
            text.matches("<use href=\"#asset0\"").count(),
            1,
            "expected exactly one clip media use:\n{text}"
        );
        assert!(
            text.contains("<use href=\"#asset0\" transform=\"matrix(25 0 0 25 100.0 75.0)\"/>"),
            "clip media placement matrix wrong:\n{text}"
        );
        // The asset's NAME is never emitted (as for
        // figure-sourced media) — bytes and generated
        // ids only.
        assert!(!text.contains("clipdot.png"), "asset name leaked:\n{text}");
        // The Wave 13 z-order holds for clip-sourced
        // media: figure segment < stroke < media use.
        let fig_pos = text.find("<line x1=\"").expect("figure segment missing");
        let stroke_pos = text.find("stroke=\"#123456\"").expect("stroke missing");
        let media_pos = text.find("<use href=\"#asset0\"").expect("media use missing");
        assert!(
            fig_pos < stroke_pos && stroke_pos < media_pos,
            "clip media broke the z-order:\n{text}"
        );
    }

    #[test]
    fn svg_clip_def_image_opacity_composes_instance_and_clip_node_style() {
        // §6.2's alpha product with the CARRIER's style:
        // instance figure opacity 0.5 × CLIP node style
        // 0.5 = 0.25 on the <use>. The instance node's
        // own style (0.9, a decoy) must not leak in —
        // its node carries no media, so it is not the
        // carrier.
        let mut project = clip_media_project(
            (None, None),
            (Some(crate::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), None),
        );
        project.figures[0].opacity = 0.5;
        project.figures[0].def.nodes[1].style.opacity = 0.9;
        project.clips[0].def.nodes[1].style.opacity = 0.5;
        let dir = std::path::Path::new("/tmp/styk-svg-clip-opacity-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        let use_line = text
            .lines()
            .find(|l| l.contains("<use href=\"#asset0\""))
            .expect("clip media use missing");
        assert!(
            use_line.contains("transform=\"matrix(25 0 0 25 100.0 75.0)\""),
            "clip media matrix changed: {use_line}"
        );
        assert!(
            use_line.ends_with(" opacity=\"0.25\"/>"),
            "clip media opacity is not the instance x clip-node product: {use_line}"
        );
    }

    #[test]
    fn svg_clip_def_text_emits_escaped_with_clip_colour_then_figure_fallback() {
        // Clip-def text on node 2 (the leg, (100,100) to
        // (100,180)): centred (100,140), direction 90°.
        // Content and size are the clip node's; the
        // colour is the clip node's colour, falling back
        // to the figure colour when the clip node has
        // none (§6.2). The content still routes through
        // xml_escape, as for figure-sourced text.
        let mut project = clip_media_project(
            (None, None),
            (
                None,
                Some(crate::model::SegmentMedia::Text {
                    content: "A<B".into(),
                    size: 25.0,
                }),
            ),
        );
        project.clips[0].def.nodes[2].color = Some(0x00FF00);
        let dir = std::path::Path::new("/tmp/styk-svg-clip-text-test");
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("<text x=\"100.0\" y=\"140.0\" font-family=\"monospace\" font-size=\"25.0\" fill=\"#00FF00\" text-anchor=\"middle\" dominant-baseline=\"central\" transform=\"rotate(90.0 100.0 140.0)\">A&lt;B</text>"),
            "clip text element wrong:\n{text}"
        );
        project.clips[0].def.nodes[2].color = None;
        export_svg(&project, dir).unwrap();
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(
            text.contains("fill=\"#FF0000\""),
            "clip text lost the figure-colour fallback:\n{text}"
        );
        assert!(
            !text.contains("#00FF00"),
            "clip node colour leaked after clearing:\n{text}"
        );
    }

    #[test]
    fn svg_media_free_clip_project_is_byte_identical_to_no_clip() {
        // A clip whose def carries no media (and whose
        // instance carries none) emits nothing: the
        // export is byte-identical to the same project
        // with no clip at all — the fallback only ever
        // ADDS items for media that exists.
        let with_clip = clip_media_project((None, None), (None, None));
        let mut no_clip = clip_media_project((None, None), (None, None));
        no_clip.figures[0].clip = None;
        no_clip.clips.clear();
        let dir_a = std::path::Path::new("/tmp/styk-svg-clip-clean-a-test");
        let dir_b = std::path::Path::new("/tmp/styk-svg-clip-clean-b-test");
        export_svg(&with_clip, dir_a).unwrap();
        export_svg(&no_clip, dir_b).unwrap();
        let a = std::fs::read_to_string(dir_a.join("frame-0001.svg")).unwrap();
        let b = std::fs::read_to_string(dir_b.join("frame-0001.svg")).unwrap();
        assert_eq!(a, b, "a media-free clip changed the SVG bytes");
        assert!(!a.contains("<use"), "media-free clip gained a <use>:\n{a}");
        assert!(
            !a.contains("asset0"),
            "media-free clip gained an asset def:\n{a}"
        );
    }

    // ---- Wave P (P27): subtitle emission ----

    /// A demo project on a known 800×600 stage with the
    /// given cues and an explicitly-set style (bottom,
    /// scale 1.0, white on black unless a test overrides),
    /// so the pins below measure THIS writer's layout maths
    /// and not the style type's `Default` values.
    fn subtitled_project(
        cues: Vec<crate::subtitles::SubtitleCue>,
    ) -> Project {
        let mut project = Project::new_demo();
        project.stage = [800.0, 600.0];
        project.subtitles = cues;
        project.subtitle_style = crate::subtitles::SubtitleStyle {
            position: crate::subtitles::SubtitlePosition::Bottom,
            font_scale: 1.0,
            text_color: 0xFF_FF_FF,
            box_color: 0x00_00_00,
        };
        project
    }

    fn cue(start: usize, end: usize, text: &str) -> crate::subtitles::SubtitleCue {
        crate::subtitles::SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    fn frame_svg(project: &Project, frame: usize, name: &str) -> String {
        let path = std::path::Path::new(name);
        write_frame_svg(project, frame, path).unwrap();
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn svg_subtitle_bottom_cue_emits_one_text_at_the_pinned_band() {
        // Layout contract on an 800×600 stage, scale 1.0:
        // font 20, line height 24, edge margin 16 — the
        // single bottom line's centre sits at
        // 600 − 16 − 0×24 − 24/2 = 572, centred at x = 400.
        let project = subtitled_project(vec![cue(0, 48, "Hello world")]);
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-bottom.svg");
        assert_eq!(
            text.matches("<text").count(),
            1,
            "one live cue must emit exactly one <text>:\n{text}"
        );
        assert!(
            text.contains("<text x=\"400.0\" y=\"572.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">Hello world</text>"),
            "bottom subtitle element wrong:\n{text}"
        );
    }

    #[test]
    fn svg_subtitle_top_cue_mirrors_the_bottom_band() {
        // Top anchor: centre at 16 + 0×24 + 24/2 = 28 — the
        // exact mirror of the bottom band (600 − 572 = 28).
        let mut project = subtitled_project(vec![cue(0, 48, "Hello world")]);
        project.subtitle_style.position = crate::subtitles::SubtitlePosition::Top;
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-top.svg");
        assert_eq!(text.matches("<text").count(), 1, "{text}");
        assert!(
            text.contains("<text x=\"400.0\" y=\"28.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">Hello world</text>"),
            "top subtitle element is not the bottom mirror:\n{text}"
        );
    }

    #[test]
    fn svg_subtitle_stack_grows_away_from_the_anchor_edge_in_stored_order() {
        // Two live cues, bottom: the FIRST cue stays closest
        // to the edge (572); the second stacks one line
        // height further in (548). Top mirrors: 28 then 52.
        let project = subtitled_project(vec![cue(0, 48, "First"), cue(0, 48, "Second")]);
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-stack.svg");
        assert_eq!(text.matches("<text").count(), 2, "{text}");
        assert!(
            text.contains("y=\"572.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">First</text>"),
            "first cue left the edge band:\n{text}"
        );
        assert!(
            text.contains("y=\"548.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">Second</text>"),
            "second cue did not stack inward:\n{text}"
        );
        let mut project = subtitled_project(vec![cue(0, 48, "First"), cue(0, 48, "Second")]);
        project.subtitle_style.position = crate::subtitles::SubtitlePosition::Top;
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-stack-top.svg");
        assert!(
            text.contains("y=\"28.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">First</text>"),
            "top stack: first cue wrong:\n{text}"
        );
        assert!(
            text.contains("y=\"52.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">Second</text>"),
            "top stack: second cue wrong:\n{text}"
        );
    }

    #[test]
    fn svg_subtitle_multiline_cue_puts_its_last_line_at_the_edge() {
        // One two-line cue at the bottom anchor: the LAST
        // line owns the edge band (572); the first line
        // stacks above it (548). One <text> per line.
        let project = subtitled_project(vec![cue(0, 48, "one\ntwo")]);
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-multiline.svg");
        assert_eq!(text.matches("<text").count(), 2, "{text}");
        assert!(
            text.contains("y=\"572.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">two</text>"),
            "last line not at the edge:\n{text}"
        );
        assert!(
            text.contains("y=\"548.0\" font-family=\"sans-serif\" font-size=\"20.0\" fill=\"#FFFFFF\" stroke=\"#000000\" stroke-width=\"3\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">one</text>"),
            "first line not stacked above:\n{text}"
        );
    }

    #[test]
    fn svg_subtitle_no_live_cue_emits_nothing_and_bytes_match_cue_free() {
        // Half-open liveness: cue 10..20 is dead at frame 0
        // and at frame 20, live at 19. A dead frame's
        // document carries no <text> and is byte-identical
        // to the cue-free project.
        let project = subtitled_project(vec![cue(10, 20, "Later")]);
        let dead = frame_svg(&project, 0, "/tmp/styk-svg-sub-dead.svg");
        assert!(!dead.contains("<text"), "dead frame gained a <text>:\n{dead}");
        let mut cue_free = subtitled_project(vec![]);
        cue_free.subtitles.clear();
        let plain = frame_svg(&cue_free, 0, "/tmp/styk-svg-sub-plain.svg");
        assert_eq!(dead, plain, "a frame with no live cue changed the SVG bytes");
        let live = frame_svg(&project, 19, "/tmp/styk-svg-sub-live19.svg");
        assert_eq!(live.matches("<text").count(), 1, "{live}");
        let after = frame_svg(&project, 20, "/tmp/styk-svg-sub-after.svg");
        assert!(!after.contains("<text"), "end frame is exclusive:\n{after}");
    }

    #[test]
    fn svg_subtitle_text_is_xml_escaped() {
        let project = subtitled_project(vec![cue(0, 48, "Fish & <Chips>")]);
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-escape.svg");
        assert!(
            text.contains(">Fish &amp; &lt;Chips&gt;</text>"),
            "cue text not escaped:\n{text}"
        );
        assert!(
            !text.contains("Fish & <Chips>"),
            "raw cue text leaked into the document:\n{text}"
        );
    }

    #[test]
    fn svg_subtitle_style_scales_font_band_and_colours() {
        // Scale 2.0: font 40, line height 48, stroke width
        // 6; the bottom band centre moves to
        // 600 − 16 − 0×48 − 48/2 = 560. Style colours reach
        // fill (text) and stroke (backing) verbatim.
        let mut project = subtitled_project(vec![cue(0, 48, "Big")]);
        project.subtitle_style.font_scale = 2.0;
        project.subtitle_style.text_color = 0xFF_00_00;
        project.subtitle_style.box_color = 0x11_22_33;
        let text = frame_svg(&project, 0, "/tmp/styk-svg-sub-style.svg");
        assert!(
            text.contains("<text x=\"400.0\" y=\"560.0\" font-family=\"sans-serif\" font-size=\"40.0\" fill=\"#FF0000\" stroke=\"#112233\" stroke-width=\"6\" paint-order=\"stroke\" text-anchor=\"middle\" dominant-baseline=\"central\">Big</text>"),
            "styled subtitle element wrong:\n{text}"
        );
    }
}
