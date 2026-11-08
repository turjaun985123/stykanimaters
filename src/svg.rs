//! SVG export: one vector file per authored frame, drawn with the
//! same camera, shapes, strokes and particles as the rasterizer.
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

use crate::export::{camera_rotate_offset, particles_at};
use crate::model::{evaluate, CapKind, Project, Shape, V2};
use std::path::Path;

fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
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
            // exactly like polyline points do.
            let r = (stroke.width * 0.5).max(1.0);
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
        } else if stroke.scatter > 0.0 && !stroke.points.is_empty() {
            // Scattered stroke (Wave 10 adoption): dabs,
            // not a polyline — the same placements the
            // stage and the rasterizer compute
            // (paint::stroke_dabs at the shared spacing),
            // one circle per dab at the stroke's recorded
            // opacity, uniform across dabs.
            let r = (stroke.width * 0.5).max(1.0);
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
                            "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"><stop offset=\"0\" stop-color=\"{}\"><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>\n",
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
    for (p, r, c, a) in particles_at(project, frame as f32) {
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
}
