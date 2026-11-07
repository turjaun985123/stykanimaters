//! PNG sequence export: a small CPU rasterizer so projects can be
//! rendered to images without a GPU or a display. Draws each figure's
//! evaluated segments as round-capped thick lines and the head as a
//! ring, with 2x supersampling for anti-aliasing.

use crate::model::{dist, evaluate, CapKind, ParticleKind, Project, Shape, V2};
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
    let mut segments: Vec<StyledSeg> = Vec::new();
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
            let color_hex = node.color.unwrap_or(fig.color);
            let color = rgb(color_hex);
            let half = node.thickness / 2.0 * zoom * ss as f32;
            if let Some(p) = node.parent {
                let style = node.style;
                // A point cap forces the far width to zero whatever
                // the taper says.
                let half_far = if style.cap == CapKind::Point {
                    0.0
                } else {
                    half * style.taper.max(0.0)
                };
                segments.push(StyledSeg {
                    a: view(pos[p]),
                    b: view(pos[i]),
                    half_near: half,
                    half_far,
                    color_near: color_hex,
                    color_far: style.color2.unwrap_or(color_hex),
                    opacity: style.opacity,
                    cap: style.cap,
                });
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
            let pressured = stroke.widths.len() == stroke.points.len();
            for (i, w) in stroke.points.windows(2).enumerate() {
                let width = if pressured {
                    (stroke.widths[i] + stroke.widths[i + 1]) / 2.0
                } else {
                    stroke.width
                };
                segments.push(StyledSeg::solid(
                    view(w[0]),
                    view(w[1]),
                    stroke.color,
                    width / 2.0 * zoom * ss as f32,
                ));
            }
        }
    }
    let mut particles: Vec<([f32; 2], f32, [u8; 3], f32)> = particles_at(project, frame_f)
        .into_iter()
        .map(|(p, r, c, a)| (view(p), r * zoom * ss as f32, c, a))
        .collect();
    for entry in &project.fx_clips {
        if entry.figure < project.figures.len() {
            let fig = &project.figures[entry.figure];
            if !fig.hidden {
                let pos = evaluate(&fig.def, &poses[entry.figure]);
                if let Some(&anchor) = pos.get(entry.node) {
                    for (p, r, c, a) in crate::vfx::clip_particles(&entry.clip, frame_f, anchor) {
                        particles.push((view(p), r * zoom * ss as f32, c, a));
                    }
                }
            }
        }
    }

    for y in 0..h {
        for x in 0..w {
            let p = [x as f32, y as f32];
            let mut px = bg;
            for seg in &segments {
                if let Some(c) = styled_seg_color(p, seg) {
                    px = crate::style::blend_over(px, rgb(c), seg.opacity);
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
    // Media segments (sprites / text) composite on the final image.
    let has_media = project
        .figures
        .iter()
        .any(|f| f.def.nodes.iter().any(|n| n.media.is_some()));
    if has_media {
        let mut rgba = image::DynamicImage::ImageRgb8(out).to_rgba8();
        let ssf = ss as f32;
        crate::media::composite_media(project, frame_f, &mut rgba, &|p| {
            let v = view(p);
            [v[0] / ssf, v[1] / ssf]
        });
        out = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
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
    let silent = unique_temp("silent").with_extension("mp4");
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
static ENCODE_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn unique_temp(tag: &str) -> std::path::PathBuf {
    let seq = ENCODE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("styk-{tag}-{}-{seq}", std::process::id()))
}

fn ffmpeg_encode(
    project: &Project,
    path: &Path,
    video_args: &[&str],
) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = unique_temp("enc");
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

/// Export every authored frame as `frame-0001.jpg`, `frame-0002.jpg`,
/// ... in `dir` (JPEG, quality 90).
pub fn export_jpeg_sequence(project: &Project, dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for i in 0..project.frames.len() {
        let img = render_frame(project, i);
        let path = dir.join(format!("frame-{:04}.jpg", i + 1));
        let mut file = std::fs::File::create(&path)?;
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90);
        encoder.encode_image(&img).map_err(std::io::Error::other)?;
    }
    Ok(project.frames.len())
}

/// Export the animation as an APNG (animated PNG) at the project
/// fps, looping forever. Uses the `png` crate's native APNG writer
/// (`set_animated`): the first frame is stored as IDAT and the rest
/// as fdAT. Frames are the same full-canvas samples the GIF/MP4
/// exports use, converted to RGBA.
pub fn export_apng(project: &Project, path: &Path) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let times = sample_times(project);
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


    /// One-figure test project: a single horizontal segment from
    /// (50, 100) to (150, 100) on a 200x200 black stage, thickness
    /// 20 (half-width 10), coloured `seg_color`, styled `style`.
    fn styled_project(style: crate::model::NodeStyle, seg_color: u32) -> Project {
        use crate::model::{
            Easing, FigureDef, FigureInstance, Frame, NodeDef, Pose, Shape,
        };
        let node = |parent: Option<usize>, length: f32, thickness: f32| NodeDef {
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
            }],
            frames: vec![Frame {
                poses: vec![Pose {
                    root: [50.0, 100.0],
                    angles: vec![0.0, 90.0],
                }],
                duration: 1.0,
                easing: None,
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
            Some(crate::model::SegmentMedia::Image { asset: 0 });
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
}
