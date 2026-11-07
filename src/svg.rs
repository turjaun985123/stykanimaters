//! SVG export: one vector file per authored frame, drawn with the
//! same camera, shapes, strokes and particles as the rasterizer.

use crate::export::particles_at;
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
        let view = |p: V2| -> V2 {
            [
                (p[0] - cam_x) * zoom + project.stage[0] / 2.0,
                (p[1] - cam_y) * zoom + project.stage[1] / 2.0,
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
            if stroke.frame == frame_idx && stroke.points.len() > 1 {
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
    fn svg_frames_are_real_vector_files() {
        let project = Project::new_demo();
        let dir = std::path::Path::new("/tmp/styk-svg-test");
        let n = export_svg(&project, dir).unwrap();
        assert_eq!(n, 5);
        let text = std::fs::read_to_string(dir.join("frame-0001.svg")).unwrap();
        assert!(text.starts_with("<svg"), "not an svg: {}", &text[..40.min(text.len())]);
        assert!(text.contains("<line"), "figure segments missing");
    }
}
