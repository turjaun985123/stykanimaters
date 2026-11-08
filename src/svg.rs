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

pub fn export_svg(project: &Project, dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for frame_idx in 0..project.frames.len() {
        let poses = project.poses_at(frame_idx as f32);
        let (cam_x, cam_y, zoom) = project.camera_at(frame_idx as f32);
        let angle = project.camera_angle_at(frame_idx as f32);
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
        for stroke in &project.strokes {
            if stroke.frame != frame_idx {
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
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"/>\n",
                    pts.join(" "),
                    hex(stroke.color),
                    stroke.width * zoom
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
                        c[0], c[1], 17.0 * zoom, color, w
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
        for (p, r, c, a) in particles_at(project, frame_idx as f32) {
            let q = view(p);
            svg += &format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"#{:02X}{:02X}{:02X}\" fill-opacity=\"{:.2}\"/>\n",
                q[0], q[1], r * zoom, c[0], c[1], c[2], a
            );
        }
        svg += "</svg>\n";
        let svg = svg.replace("<!--DEFS-->", &defs);
        std::fs::write(dir.join(format!("frame-{:04}.svg", frame_idx + 1)), svg)?;
    }
    Ok(project.frames.len())
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
}
