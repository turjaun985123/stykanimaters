//! Text + sprite segment rendering.
//!
//! A node may carry [`SegmentMedia`](crate::model::SegmentMedia):
//! an image from the project's asset table, or a text label. This
//! module turns those into concrete draw items for a frame
//! ([`media_items`]) and rasterises them into a rendered frame
//! ([`composite_media`]).
//!
//! Geometry: an image becomes a quad centred on the node's segment
//! (parent joint -> node joint). The quad's long axis follows the
//! segment, its length is the segment length, and its height is
//! `length / aspect`, where the aspect is the decoded PNG's
//! width / height, so sprites are never stretched. Text sits at the
//! segment midpoint, rotated along the segment direction.
//!
//! Free-standing instances (`Project.sprites` / `Project.texts`,
//! keyframed with `InstanceKey`s — e.g. imported from Stick Nodes
//! animation files) are placed by the same [`media_items`] pass,
//! after the segment media, so instances draw on top. A sprite
//! instance's `InstanceKey` gives its centre, rotation and scale:
//! the drawn quad is the image's pixel size times the key scale,
//! rotated about the centre. A text instance draws with glyph
//! height `size * key.scale` rotated by the key angle.
//!
//! Text is drawn with a built-in 3x5 bitmap font — no font files, no
//! platform dependencies, fully deterministic. The charset is
//! deliberately small: `A-Z` (lowercase is folded to uppercase),
//! `0-9`, space, and `. , ! ? ' " - : ; ( ) + / & % * = < > _ # $ @`.
//! Any other character renders as `?`.
//!
//! Conventions and limits:
//! - Hidden figures contribute no media (same as the rasterizer).
//! - A node with no parent has no segment; its media is placed at
//!   the joint, extending `length` units along +x. (Media normally
//!   belongs on real segments.)
//! - An image whose asset index is out of range, or whose PNG
//!   bytes fail to decode, is skipped silently.
//! - PNG assets are decoded on each call (no cache): fine for
//!   export-time compositing, not a per-frame editor hot path.

use crate::model::{evaluate, InstanceKey, Project, SegmentMedia, V2};
use image::{Rgba, RgbaImage};

/// One piece of segment media placed in stage space for a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaItem {
    /// A sprite quad. Corners are ordered so the quad is
    /// `c0 + u*(c1-c0) + v*(c3-c0)` for u, v in [0, 1]: c0/c1 are
    /// the parent-end/node-end corners on one side, c3 mirrors c0.
    Image { corners: [V2; 4], asset: usize },
    /// A text label centred at `pos`, running along `angle_deg`
    /// (stage-space degrees, y-down like the rest of the model).
    /// `size` is the glyph height in stage units.
    Text {
        pos: V2,
        angle_deg: f32,
        content: String,
        size: f32,
        color: u32,
    },
}

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: V2, s: f32) -> V2 {
    [a[0] * s, a[1] * s]
}

/// Pixel dimensions of asset `idx`, or None when it is missing
/// or its PNG bytes do not decode.
fn png_dims(project: &Project, idx: usize) -> Option<(u32, u32)> {
    let asset = project.assets.get(idx)?;
    let img = image::load_from_memory(&asset.png).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Width / height of asset `idx`, or None when it is missing or
/// its PNG bytes do not decode.
fn png_aspect(project: &Project, idx: usize) -> Option<f32> {
    let (w, h) = png_dims(project, idx)?;
    Some(w as f32 / h as f32)
}

/// All media items visible at fractional frame time `frame_f`:
/// segment media in figure order, then node order; then sprite
/// instances; then text instances (instances draw on top).
pub fn media_items(project: &Project, frame_f: f32) -> Vec<MediaItem> {
    let mut out = Vec::new();
    let poses = project.poses_at(frame_f);
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let Some(pose) = poses.get(fi) else { continue };
        let pos = evaluate(&fig.def, pose);
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            let Some(media) = &node.media else { continue };
            let (a, b) = match node.parent {
                Some(p) => (pos[p], pos[ni]),
                None => (pos[ni], add(pos[ni], [node.length, 0.0])),
            };
            let ab = sub(b, a);
            let len = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
            if len <= f32::EPSILON {
                continue;
            }
            let d = scale(ab, 1.0 / len);
            let mid = scale(add(a, b), 0.5);
            match media {
                SegmentMedia::Image { asset } => {
                    let Some(aspect) = png_aspect(project, *asset) else {
                        continue;
                    };
                    let n = [-d[1], d[0]];
                    let hl = scale(d, len / 2.0);
                    let hh = scale(n, len / aspect / 2.0);
                    out.push(MediaItem::Image {
                        corners: [
                            sub(sub(mid, hl), hh),
                            sub(add(mid, hl), hh),
                            add(add(mid, hl), hh),
                            add(sub(mid, hl), hh),
                        ],
                        asset: *asset,
                    });
                }
                SegmentMedia::Text { content, size } => {
                    out.push(MediaItem::Text {
                        pos: mid,
                        angle_deg: d[1].atan2(d[0]).to_degrees(),
                        content: content.clone(),
                        size: *size,
                        color: node.color.unwrap_or(fig.color),
                    });
                }
            }
        }
    }
    // Free-standing sprite instances: the image's pixel size times
    // the sampled key scale, centred at the key position, rotated
    // by the key angle (y-down rotation, as for text).
    for inst in &project.sprites {
        let Some(key) = InstanceKey::sample(&inst.keys, frame_f) else {
            continue;
        };
        if key.scale <= 0.0 {
            continue;
        }
        let Some((w, h)) = png_dims(project, inst.asset) else {
            continue;
        };
        let c = [key.x, key.y];
        let rad = key.angle.to_radians();
        let u = [rad.cos(), rad.sin()];
        let v = [-rad.sin(), rad.cos()];
        let hu = scale(u, w as f32 * key.scale / 2.0);
        let hv = scale(v, h as f32 * key.scale / 2.0);
        out.push(MediaItem::Image {
            corners: [
                sub(sub(c, hu), hv),
                sub(add(c, hu), hv),
                add(add(c, hu), hv),
                add(sub(c, hu), hv),
            ],
            asset: inst.asset,
        });
    }
    // Free-standing text instances: `size` is the unscaled glyph
    // height; the key scale multiplies it.
    for text in &project.texts {
        let Some(key) = InstanceKey::sample(&text.keys, frame_f) else {
            continue;
        };
        let size = text.size * key.scale;
        if size <= 0.0 {
            continue;
        }
        out.push(MediaItem::Text {
            pos: [key.x, key.y],
            angle_deg: key.angle,
            content: text.content.clone(),
            size,
            color: text.color,
        });
    }
    out
}

/// Draw every media item for `frame_f` into `img`. `view` maps
/// stage coordinates to image pixel coordinates (the same closure
/// shape the export rasterizer builds). Images are sampled
/// nearest-neighbour through the quad's affine map and alpha-blended
/// over the destination; text is stamped as solid glyph blocks.
pub fn composite_media(
    project: &Project,
    frame_f: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    // Stage-units -> pixels scale, probed from the view: the
    // length of the mapped x basis vector. Views may also rotate
    // (camera angle, Wave 6) — a basis vector's length is the
    // uniform scale with or without rotation, so this probe
    // stays exact.
    let o = view([0.0, 0.0]);
    let ex = view([1.0, 0.0]);
    let view_scale = ((ex[0] - o[0]).powi(2) + (ex[1] - o[1]).powi(2)).sqrt();
    let view_scale = if view_scale > f32::EPSILON {
        view_scale
    } else {
        1.0
    };

    for item in media_items(project, frame_f) {
        match item {
            MediaItem::Image { corners, asset } => {
                composite_image(project, asset, corners, img, view);
            }
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
            } => composite_text(pos, angle_deg, &content, size, color, img, view, view_scale),
        }
    }
}

fn composite_image(
    project: &Project,
    asset: usize,
    corners: [V2; 4],
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    let Some(asset) = project.assets.get(asset) else {
        return;
    };
    let Ok(decoded) = image::load_from_memory(&asset.png) else {
        return;
    };
    let src = decoded.to_rgba8();
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    if sw < 1.0 || sh < 1.0 {
        return;
    }
    let c: [[f32; 2]; 4] = [
        view(corners[0]),
        view(corners[1]),
        view(corners[2]),
        view(corners[3]),
    ];
    let e1 = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
    let e2 = [c[3][0] - c[0][0], c[3][1] - c[0][1]];
    let det = e1[0] * e2[1] - e1[1] * e2[0];
    if det.abs() <= f32::EPSILON {
        return;
    }
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    let min_x = c.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_x = c.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    let min_y = c.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_y = c.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    for y in min_y.max(0)..=max_y.min(ih - 1) {
        for x in min_x.max(0)..=max_x.min(iw - 1) {
            let rel = [x as f32 + 0.5 - c[0][0], y as f32 + 0.5 - c[0][1]];
            let u = (rel[0] * e2[1] - rel[1] * e2[0]) / det;
            let v = (e1[0] * rel[1] - e1[1] * rel[0]) / det;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            let sx = ((u * sw) as u32).min(src.width() - 1);
            let sy = ((v * sh) as u32).min(src.height() - 1);
            let sp = *src.get_pixel(sx, sy);
            let sa = sp[3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let dp = img.get_pixel_mut(x as u32, y as u32);
            let da = dp[3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }
            for ch in 0..3 {
                let blended =
                    (sp[ch] as f32 * sa + dp[ch] as f32 * da * (1.0 - sa)) / out_a;
                dp[ch] = blended.round().clamp(0.0, 255.0) as u8;
            }
            dp[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_text(
    pos: V2,
    angle_deg: f32,
    content: &str,
    size: f32,
    color: u32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
    view_scale: f32,
) {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() || size <= 0.0 {
        return;
    }
    let cell = size / 5.0; // stage units per glyph pixel
    let total_units = chars.len() as f32 * 4.0 - 1.0; // 3 wide + 1 gap
    let start = -total_units / 2.0;
    let rad = angle_deg.to_radians();
    let (cs, sn) = (rad.cos(), rad.sin());
    let rgba = Rgba([
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        255,
    ]);
    let block = (cell * view_scale).round().max(1.0) as i32;
    for (ci, ch) in chars.iter().enumerate() {
        let glyph = glyph_rows(*ch);
        for row in 0..5usize {
            for col in 0..3usize {
                if (glyph[row] >> (2 - col)) & 1 == 0 {
                    continue;
                }
                let lx = (start + ci as f32 * 4.0 + col as f32 + 0.5) * cell;
                let ly = (row as f32 - 2.0 + 0.5) * cell;
                let world = [
                    pos[0] + lx * cs - ly * sn,
                    pos[1] + lx * sn + ly * cs,
                ];
                let px = view(world);
                let cx = px[0].round() as i32;
                let cy = px[1].round() as i32;
                for dy in 0..block {
                    for dx in 0..block {
                        let x = cx - block / 2 + dx;
                        let y = cy - block / 2 + dy;
                        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height()
                        {
                            img.put_pixel(x as u32, y as u32, rgba);
                        }
                    }
                }
            }
        }
    }
}

/// 3x5 bitmap glyph for `ch`: five rows, 3 bits each, bit 2 is the
/// leftmost pixel. Lowercase folds to uppercase; unknown printable
/// characters fall back to `?`.
fn glyph_rows(ch: char) -> [u8; 5] {
    let up = ch.to_ascii_uppercase();
    match up {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b010, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        ' ' => [0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0b010],
        ',' => [0, 0, 0, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '?' => [0b110, 0b001, 0b010, 0, 0b010],
        '\'' => [0b010, 0b010, 0, 0, 0],
        '"' => [0b101, 0b101, 0, 0, 0],
        '-' => [0, 0, 0b111, 0, 0],
        ':' => [0, 0b010, 0, 0b010, 0],
        ';' => [0, 0b010, 0, 0b010, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '+' => [0, 0b010, 0b111, 0b010, 0],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '*' => [0, 0b101, 0b010, 0b101, 0],
        '=' => [0, 0b111, 0, 0b111, 0],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '_' => [0, 0, 0, 0, 0b111],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        _ => {
            if ch.is_control() {
                [0, 0, 0, 0, 0]
            } else {
                glyph_rows('?')
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Easing, FigureDef, FigureInstance, Frame, ImageAsset, NodeDef, NodeStyle, Pose, Shape,
        SpriteInstance, TextInstance,
    };
    use std::io::Cursor;

    fn png_bytes(w: u32, h: u32, px: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, px);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn node(
        name: &str,
        parent: Option<usize>,
        length: f32,
        media: Option<SegmentMedia>,
    ) -> NodeDef {
        NodeDef {
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
            style: NodeStyle::default(),
            media,
        }
    }

    /// One figure, root at (100, 100): an image segment running
    /// +x for 100 units (pose angle 90 in this model's dir()), and
    /// a text segment running down (+y) for 80. Asset 0 is a 4x2
    /// green PNG (aspect 2.0).
    fn demo_project() -> Project {
        let fig = FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "Media".into(),
            color: 0xFF0000,
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node(
                        "sprite",
                        Some(0),
                        100.0,
                        Some(SegmentMedia::Image { asset: 0 }),
                    ),
                    node(
                        "label",
                        Some(0),
                        80.0,
                        Some(SegmentMedia::Text {
                            content: "HI".into(),
                            size: 25.0,
                        }),
                    ),
                ],
            },
            hidden: false,
            clip: None,
        };
        Project {
            name: "media-test".into(),
            fps: 12.0,
            stage: [800.0, 600.0],
            background: 0,
            figures: vec![fig],
            frames: vec![Frame {
                poses: vec![Pose {
                    root: [100.0, 100.0],
                    angles: vec![0.0, 90.0, 0.0],
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
            assets: vec![ImageAsset {
                name: "green".into(),
                png: png_bytes(4, 2, Rgba([0, 200, 0, 255])),
            }],
            channels: Vec::new(),
            sprites: Vec::new(),
            texts: Vec::new(),
            weather: None,
            head_ring_scale: 1.0,
        }
    }

    fn close(a: V2, b: V2) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    #[test]
    fn image_item_quad_follows_segment_with_png_aspect() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let img = items.iter().find_map(|i| match i {
            MediaItem::Image { corners, asset } => Some((corners, asset)),
            _ => None,
        });
        let (corners, asset) = img.expect("an image item");
        assert_eq!(*asset, 0);
        // Segment (100,100)->(200,100); aspect 2 -> height 50.
        assert!(close(corners[0], [100.0, 75.0]), "c0 {:?}", corners[0]);
        assert!(close(corners[1], [200.0, 75.0]), "c1 {:?}", corners[1]);
        assert!(close(corners[2], [200.0, 125.0]), "c2 {:?}", corners[2]);
        assert!(close(corners[3], [100.0, 125.0]), "c3 {:?}", corners[3]);
        // Long-edge midpoints lie exactly on the segment.
        let top_mid = scale(add(corners[0], corners[1]), 0.5);
        assert!(close(top_mid, [150.0, 75.0]));
    }

    #[test]
    fn text_item_sits_at_midpoint_along_segment() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let text = items.iter().find_map(|i| match i {
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
            } => Some((pos, angle_deg, content, size, color)),
            _ => None,
        });
        let (pos, angle, content, size, color) = text.expect("a text item");
        assert!(close(*pos, [100.0, 140.0]), "pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "angle {angle}");
        assert_eq!(content, "HI");
        assert_eq!(*size, 25.0);
        assert_eq!(*color, 0xFF0000, "falls back to the figure colour");
    }

    #[test]
    fn composite_paints_image_quad_and_text_pixels() {
        let project = demo_project();
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // Centre of the sprite quad is the asset's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([0, 200, 0, 255]));
        // Inside the quad but off-centre, too.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // A known "on" cell of the H: text runs downward from
        // (100, 140); the first glyph cell centre lands at
        // (107.5, 125), just below the sprite quad's bottom edge.
        assert_eq!(*img.get_pixel(108, 125), Rgba([255, 0, 0, 255]));
        // Far corner untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn missing_or_broken_assets_are_skipped() {
        let mut project = demo_project();
        project.assets.clear();
        project.assets.push(ImageAsset {
            name: "junk".into(),
            png: vec![1, 2, 3],
        });
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "only the text item survives");
        assert!(matches!(items[0], MediaItem::Text { .. }));
    }

    #[test]
    fn glyph_table_covers_charset_and_folds_case() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!?'-:;()+/&%*=<>_#$@\"".chars() {
            let g = glyph_rows(ch);
            let lit: u32 = g.iter().map(|r| r.count_ones()).sum();
            assert!(lit > 0, "glyph for {ch:?} is blank");
        }
        assert_eq!(glyph_rows('a'), glyph_rows('A'));
        assert_eq!(glyph_rows('~'), glyph_rows('?'), "unknown -> ?");
    }

    fn image_items(items: &[MediaItem]) -> Vec<([V2; 4], usize)> {
        items
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, asset } => Some((*corners, *asset)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn sprite_instance_item_geometry_and_sampling() {
        let mut project = demo_project();
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![
                InstanceKey {
                    frame: 0.0,
                    x: 300.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 0.0,
                },
                InstanceKey {
                    frame: 2.0,
                    x: 340.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 90.0,
                },
            ],
        });
        // Frame 0: asset is 4x2 px, scale 10 -> 40x20 quad at
        // (300, 200), unrotated.
        let items = media_items(&project, 0.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [280.0, 190.0]), "c0 {corners:?}");
        assert!(close(corners[1], [320.0, 190.0]), "c1 {corners:?}");
        assert!(close(corners[2], [320.0, 210.0]), "c2 {corners:?}");
        assert!(close(corners[3], [280.0, 210.0]), "c3 {corners:?}");
        // Frame 1: keys interpolate — centre (320, 200).
        let items = media_items(&project, 1.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        let centre = scale(add(corners[0], corners[2]), 0.5);
        assert!(close(centre, [320.0, 200.0]), "centre {centre:?}");
        // Frame 2: rotated 90 degrees — the quad is 20 wide, 40 tall.
        let items = media_items(&project, 2.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [350.0, 180.0]), "c0 {corners:?}");
        assert!(close(corners[1], [350.0, 220.0]), "c1 {corners:?}");
    }

    #[test]
    fn text_instance_item_uses_size_times_key_scale() {
        let mut project = demo_project();
        project.texts.push(TextInstance {
            content: "YO".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 400.0,
                scale: 2.0,
                angle: 0.0,
            }],
        });
        let items = media_items(&project, 0.0);
        // The instance text comes after the segment "HI" text.
        let text = items.iter().rev().find_map(|i| match i {
            MediaItem::Text {
                pos,
                content,
                size,
                color,
                ..
            } => Some((pos, content, size, color)),
            _ => None,
        });
        let (pos, content, size, color) = text.expect("a text instance item");
        assert!(close(*pos, [100.0, 400.0]), "pos {pos:?}");
        assert_eq!(content, "YO");
        assert_eq!(*size, 50.0, "25 * key scale 2");
        assert_eq!(*color, 0x0000FF);
    }

    #[test]
    fn composite_instances_draw_over_segment_media() {
        let mut project = demo_project();
        project.assets.push(ImageAsset {
            name: "red".into(),
            png: png_bytes(2, 2, Rgba([255, 0, 0, 255])),
        });
        // Red sprite instance (2x2 px * scale 10 = 20x20) centred
        // on the segment image's centre (150, 100).
        project.sprites.push(SpriteInstance {
            asset: 1,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 150.0,
                y: 100.0,
                scale: 10.0,
                angle: 0.0,
            }],
        });
        project.texts.push(TextInstance {
            content: "H".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 250.0,
                scale: 1.0,
                angle: 0.0,
            }],
        });
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // The instance sprite paints over the segment's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([255, 0, 0, 255]));
        // Green still visible outside the instance quad.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // The text instance stamps blue glyph pixels in its band.
        let mut blue = 0;
        for y in 238..262 {
            for x in 80..120 {
                if *img.get_pixel(x, y) == Rgba([0, 0, 255, 255]) {
                    blue += 1;
                }
            }
        }
        assert!(blue > 10, "text instance painted only {blue} blue pixels");
    }
}
