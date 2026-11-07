//! PNG sequence export: a small CPU rasterizer so projects can be
//! rendered to images without a GPU or a display. Draws each figure's
//! evaluated segments as round-capped thick lines and the head as a
//! ring, with 2x supersampling for anti-aliasing.

use crate::model::{dist, evaluate, Project};
use image::{ImageBuffer, Rgb};
use std::path::Path;

fn rgb(hex: u32) -> [u8; 3] {
    [
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    ]
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

/// Render one frame index to an RGB buffer at the project stage size.
pub fn render_frame(project: &Project, frame_idx: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let ss = 2u32; // supersample factor
    let (w, h) = (
        project.stage[0] as u32 * ss,
        project.stage[1] as u32 * ss,
    );
    let bg = rgb(project.background);
    let mut buf = ImageBuffer::from_pixel(w, h, Rgb(bg));
    let frame = &project.frames[frame_idx];
    let line_half = 5.0f32 * ss as f32;
    let head_r = 17.0f32 * ss as f32;

    // Collect drawables first: (segment a, b, color) and (head center, color).
    let mut segments: Vec<([f32; 2], [f32; 2], [u8; 3])> = Vec::new();
    let mut heads: Vec<([f32; 2], [u8; 3])> = Vec::new();
    for (fi, fig) in project.figures.iter().enumerate() {
        let pose = &frame.poses[fi];
        let pos = evaluate(&fig.def, pose);
        let color = rgb(fig.color);
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                let a = [pos[p][0] * ss as f32, pos[p][1] * ss as f32];
                let b = [pos[i][0] * ss as f32, pos[i][1] * ss as f32];
                segments.push((a, b, color));
            }
            if node.head {
                heads.push(([pos[i][0] * ss as f32, pos[i][1] * ss as f32], color));
            }
        }
    }

    for y in 0..h {
        for x in 0..w {
            let p = [x as f32, y as f32];
            let mut px = bg;
            for (a, b, color) in &segments {
                if seg_dist(p, *a, *b) <= line_half {
                    px = *color;
                }
            }
            for (c, color) in &heads {
                let d = dist(p, *c);
                if (d - head_r).abs() <= line_half {
                    px = *color;
                }
            }
            buf.put_pixel(x, y, Rgb(px));
        }
    }
    // Downsample by the supersample factor (box average).
    let (ow, oh) = (project.stage[0] as u32, project.stage[1] as u32);
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
    out
}

/// Export every frame as `frame-0001.png`, `frame-0002.png`, ... in `dir`.
pub fn export_png_sequence(project: &Project, dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for i in 0..project.frames.len() {
        let img = render_frame(project, i);
        img.save(dir.join(format!("frame-{:04}.png", i + 1)))
            .map_err(std::io::Error::other)?;
    }
    Ok(project.frames.len())
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
    fn renders_non_background_pixels() {
        let project = Project::new_demo();
        let img = render_frame(&project, 0);
        let bg = rgb(project.background);
        let drawn = img.pixels().filter(|p| p.0 != bg).count();
        assert!(drawn > 500, "expected the figure to be drawn, got {drawn} px");
    }
}
