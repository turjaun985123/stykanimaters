//! SVG export: one vector file per authored frame, drawn with the
//! same camera, shapes, strokes and particles as the rasterizer.
//!
//! Wave 11 (E8) parity pass: filled strokes, FX-clip particles
//! (including Trail segments) and weather particles now emit
//! here too — previously this writer silently dropped all
//! three, so an MCP client exporting SVG got a different
//! picture from the rasterizer/stage for the same project.
//! See the module's fidelity notes in the Wave 11 report for
//! what still differs (figure opacity, bends, z-order).
//!
//! XML safety: no user-controlled string is interpolated into
//! the document — every `format!` argument below is a number,
//! a `hex()` colour (u32-masked), or a fixed literal — so a
//! hostile figure/project name cannot break the markup today.
//! [`xml_escape`] is the pinned helper any future text
//! emission (text instances, names) MUST route through.
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

/// Escape the five XML-significant characters for safe
/// interpolation into SVG text content or a quoted attribute
/// value: `&` first (it introduces every other entity), then
/// `<`, `>`, `"` and `'`. Nothing in `write_frame_svg`
/// interpolates user text today (see the module docs), but
/// text instances and figure names are user-controlled —
/// any future emission of them MUST pass through here, and
/// the tests below pin the mapping so that day is safe.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\'' => out += "&apos;",
            _ => out.push(ch),
        }
    }
    out
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
    let opacity = opacity.clamp(0.0, 1.0);
    if !(opacity < 1.0) {
        return String::new();
    }
    let text = format!("{opacity:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!(" stroke-opacity=\"{text}\"")
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
    let poses = project.poses_at(frame as f32);
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
                svg += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" fill-opacity=\"{:.2}\"/>\n",
                    q[0],
                    q[1],
                    r,
                    hex(stroke.color),
                    alpha
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
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let pos = evaluate(&fig.def, &poses[fi]);
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let color = hex(node.color.unwrap_or(fig.color));
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
                match style.color2 {
                    Some(color2) => {
                        // Styled segment: a polygon spanning the
                        // near/far widths, filled with a linear
                        // gradient along the segment axis.
                        let id = format!("grad{grad_n}");
                        grad_n += 1;
                        defs += &format!(
                            "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>\n",
                            id, a[0], a[1], b[0], b[1], color,
                            hex(color2)
                        );
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
                        let opacity = if style.opacity < 1.0 {
                            format!(" fill-opacity=\"{:.2}\"", style.opacity)
                        } else {
                            String::new()
                        };
                        svg += &format!(
                            "<polygon points=\"{}\" fill=\"url(#{})\"{}/>\n",
                            pts, id, opacity
                        );
                    }
                    None => {
                        let cap = match style.cap {
                            CapKind::Round => "round",
                            CapKind::Square => "square",
                            // A point is not expressible as a
                            // line cap; butt ends the stroke
                            // flush at the joint.
                            CapKind::Point => "butt",
                        };
                        let opacity = if style.opacity < 1.0 {
                            format!(" stroke-opacity=\"{:.2}\"", style.opacity)
                        } else {
                            String::new()
                        };
                        svg += &format!(
                            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"{}\"{}/>\n",
                            a[0], a[1], b[0], b[1], color, w, cap, opacity
                        );
                    }
                }
            }
            if node.head {
                let c = view(pos[i]);
                svg += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"/>\n",
                    c[0], c[1], project.head_ring_radius() * zoom, color, w
                );
            }
            match node.shape {
                Shape::Circle => {
                    let c = view(pos[i]);
                    svg += &format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"/>\n",
                        c[0], c[1], node.length / 2.0 * zoom, color, w
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
                        "<polygon points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\"/>\n",
                        pts.join(" "),
                        color,
                        w
                    );
                }
                Shape::Line => {}
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
        let pos = evaluate(&fig.def, pose);
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
    svg += "</svg>\n";
    let svg = svg.replace("<!--DEFS-->", &defs);
    std::fs::write(path, svg)
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
}
