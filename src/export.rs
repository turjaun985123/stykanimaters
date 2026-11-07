//! PNG sequence export: a small CPU rasterizer so projects can be
//! rendered to images without a GPU or a display. Draws each figure's
//! evaluated segments as round-capped thick lines and the head as a
//! ring, with 2x supersampling for anti-aliasing.

use crate::model::{dist, evaluate, ParticleKind, Project, Shape, V2};
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

/// Render one frame index to an RGB buffer at the project stage size.
pub fn render_frame(project: &Project, frame_idx: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    render_frame_at(project, frame_idx as f32)
}

/// Render at a fractional frame time (tween poses, camera and
/// particles all evaluate at `f`). The camera (centre + zoom,
/// interpolated between keys) is applied, as are per-segment
/// colours, thicknesses and shape segments.
pub fn render_frame_at(project: &Project, frame_f: f32) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let ss = 2u32; // supersample factor
    let (w, h) = (
        project.stage[0] as u32 * ss,
        project.stage[1] as u32 * ss,
    );
    let bg = rgb(project.background);
    let mut buf = ImageBuffer::from_pixel(w, h, Rgb(bg));
    let poses = project.poses_at(frame_f);
    let (cam_x, cam_y, zoom) = project.camera_at(frame_f);
    let view = |p: [f32; 2]| -> [f32; 2] {
        [
            ((p[0] - cam_x) * zoom + project.stage[0] / 2.0) * ss as f32,
            ((p[1] - cam_y) * zoom + project.stage[1] / 2.0) * ss as f32,
        ]
    };
    let head_r = 17.0f32 * zoom * ss as f32;

    // Collect drawables: segments (a, b, color, half-width), head
    // rings (centre, color, half-width), shape circles and polygons.
    let mut segments: Vec<([f32; 2], [f32; 2], [u8; 3], f32)> = Vec::new();
    let mut heads: Vec<([f32; 2], [u8; 3], f32)> = Vec::new();
    let mut rings: Vec<([f32; 2], f32, [u8; 3], f32)> = Vec::new();
    let mut polys: Vec<(Vec<[f32; 2]>, [u8; 3], f32)> = Vec::new();
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let pose = &poses[fi];
        let pos = evaluate(&fig.def, pose);
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let color = rgb(node.color.unwrap_or(fig.color));
            let half = node.thickness / 2.0 * zoom * ss as f32;
            if let Some(p) = node.parent {
                segments.push((view(pos[p]), view(pos[i]), color, half));
            }
            if node.head {
                heads.push((view(pos[i]), color, half));
            }
            match node.shape {
                Shape::Circle => {
                    rings.push((view(pos[i]), node.length / 2.0 * zoom * ss as f32, color, half));
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
                    polys.push((verts, color, half));
                }
                Shape::Line => {}
            }
        }
    }
    // Brush strokes belonging to the authored frame under `frame_f`.
    let authored = (frame_f.floor() as usize).min(project.frames.len().saturating_sub(1));
    for stroke in &project.strokes {
        if stroke.frame == authored {
            for w in stroke.points.windows(2) {
                segments.push((
                    view(w[0]),
                    view(w[1]),
                    rgb(stroke.color),
                    stroke.width / 2.0 * zoom * ss as f32,
                ));
            }
        }
    }
    let particles: Vec<([f32; 2], f32, [u8; 3], f32)> = particles_at(project, frame_f)
        .into_iter()
        .map(|(p, r, c, a)| (view(p), r * zoom * ss as f32, c, a))
        .collect();

    for y in 0..h {
        for x in 0..w {
            let p = [x as f32, y as f32];
            let mut px = bg;
            for (a, b, color, half) in &segments {
                if seg_dist(p, *a, *b) <= *half {
                    px = *color;
                }
            }
            for (c, color, half) in &heads {
                let d = dist(p, *c);
                if (d - head_r).abs() <= *half {
                    px = *color;
                }
            }
            for (c, r, color, half) in &rings {
                let d = dist(p, *c);
                if (d - r).abs() <= *half {
                    px = *color;
                }
            }
            for (verts, color, half) in &polys {
                for k in 0..verts.len() {
                    if seg_dist(p, verts[k], verts[(k + 1) % verts.len()]) <= *half {
                        px = *color;
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

/// Export the animation as a looping GIF at the project fps.
pub fn export_gif(project: &Project, path: &Path) -> std::io::Result<usize> {
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
    let times = sample_times(project);
    let mut frames = Vec::new();
    for t in &times {
        let img = image::DynamicImage::ImageRgb8(render_frame_at(project, *t)).to_rgba8();
        frames.push(image::Frame::from_parts(img, 0, 0, delay));
    }
    let n = frames.len();
    encoder.encode_frames(frames).map_err(std::io::Error::other)?;
    Ok(n)
}

/// Export an MP4 (H.264, yuv420p) via the system `ffmpeg`. When
/// the project has an audio clip whose file exists, it is muxed in
/// at its start frame. Errors if ffmpeg is missing.
pub fn export_mp4(project: &Project, path: &Path) -> std::io::Result<usize> {
    let has_audio = project
        .audio
        .first()
        .map(|c| std::path::Path::new(&c.path).exists())
        .unwrap_or(false);
    if !has_audio {
        return ffmpeg_encode(project, path, &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
    }
    let silent = std::env::temp_dir().join(format!("styk-silent-{}.mp4", std::process::id()));
    let n = ffmpeg_encode(project, &silent, &["-c:v", "libx264", "-pix_fmt", "yuv420p"])?;
    let muxed = mux_audio(project, &silent, path)?;
    let _ = std::fs::remove_file(&silent);
    if muxed {
        Ok(n)
    } else {
        Err(std::io::Error::other("audio mux failed"))
    }
}

/// Encode rendered frames with ffmpeg using the given video args.
fn ffmpeg_encode(
    project: &Project,
    path: &Path,
    video_args: &[&str],
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = std::env::temp_dir().join(format!("styk-enc-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let times = sample_times(project);
    for (k, t) in times.iter().enumerate() {
        render_frame_at(project, *t)
            .save(tmp.join(format!("frame-{:04}.png", k + 1)))
            .map_err(std::io::Error::other)?;
    }
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-framerate".into(),
        format!("{}", project.fps.max(1.0) as i32),
        "-i".into(),
        tmp.join("frame-%04d.png").to_string_lossy().into_owned(),
    ];
    args.extend(video_args.iter().map(|a| a.to_string()));
    args.push(path.to_string_lossy().into_owned());
    let result = std::process::Command::new("ffmpeg")
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = std::fs::remove_dir_all(&tmp);
    match result {
        Ok(status) if status.success() => Ok(times.len()),
        Ok(status) => Err(std::io::Error::other(format!("ffmpeg exited with {status}"))),
        Err(e) => Err(std::io::Error::other(format!("could not run ffmpeg: {e}"))),
    }
}

/// Export a WebM (VP9). Frames are opaque; the alpha channel is
/// present but fully opaque.
pub fn export_webm(project: &Project, path: &Path) -> std::io::Result<usize> {
    ffmpeg_encode(project, path, &["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "2M"])
}

/// Mux the project's first audio clip (if its file exists) into an
/// already-encoded silent MP4, offset by its start frame.
pub fn mux_audio(project: &Project, video: &Path, out: &Path) -> std::io::Result<bool> {
    let Some(clip) = project.audio.first() else {
        return Ok(false);
    };
    if !std::path::Path::new(&clip.path).exists() {
        return Ok(false);
    }
    let offset = clip.start_frame as f32 / project.fps.max(1.0);
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            video.to_str().unwrap_or(""),
            "-itsoffset",
            &format!("{offset:.3}"),
            "-i",
            &clip.path,
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

/// Export all authored frames as one spritesheet PNG (grid layout).
pub fn export_spritesheet(project: &Project, path: &Path) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let n = project.frames.len();
    let (w, h) = (project.stage[0] as u32, project.stage[1] as u32);
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = (n as u32 + cols - 1) / cols;
    let mut sheet = ImageBuffer::new(w * cols, h * rows);
    for i in 0..n {
        let img = render_frame(project, i);
        let (ox, oy) = ((i as u32 % cols) * w, (i as u32 / cols) * h);
        image::imageops::overlay(&mut sheet, &img, ox as i64, oy as i64);
    }
    sheet.save(path).map_err(std::io::Error::other)?;
    Ok(n)
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
}
