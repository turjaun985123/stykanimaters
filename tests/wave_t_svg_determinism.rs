//! Wave T (T9) — SVG determinism, the COMBINED paths.
//!
//! What the earlier suites pinned, and what this one adds:
//!
//! - Wave R's R12 (`tests/wave_r_determinism2.rs`) pinned
//!   SVG A-B-A byte identity for a plain project, ONE plain
//!   one-line subtitle cue, and graph channels — each path
//!   in isolation.
//! - Wave S's S6 (`tests/wave_s_determinism.rs`) gave the
//!   RASTER funnel its A-B-A suite (subtitles, speaker
//!   prefix, segment-text alpha, scaled renders).
//! - Wave S's S12 pinned the speaker prefix's SVG `<text>`
//!   CONTENT (byte-exact strings) and S10 the JPEG/PNG
//!   embed labelling — content pins, not determinism pins.
//!
//! The SVG writer's combined emission — subtitle text
//! (with the speaker prefix composed by
//! `subtitles::subtitle_display_lines`), media embeds
//! (JPEG data URIs sniffed by IA-R9-3, PNG control),
//! weather discs (`vfx::weather_particles`) and a
//! non-default subtitle style — had no A-B-A pin as a
//! combination. That is this file: three legs, each
//! exporting A, then an unrelated B that provably
//! differs, then A again, demanding byte identity of the
//! two A documents.
//!
//! Why each path is deterministic BY CONSTRUCTION (the
//! legs are the proof, this is the audit trail):
//! - `append_subtitles` walks `live_subtitle_cues` — the
//!   stored cue Vec in stored order — and each cue's
//!   display lines in order; positions come from the
//!   shared layout helpers (pure arithmetic).
//! - `append_media` walks the `media_items` stream in
//!   order and dedups assets through a Vec in first-use
//!   order; the embed payload is a base64 transport of
//!   the stored bytes, the MIME sniffed from them.
//! - `weather_particles` is a pure function of
//!   (weather, frame, stage, fps): positions derive from
//!   `prand`, a seeded integer hash — no RNG state, no
//!   wall clock anywhere in the writer.
//! - No HashMap iteration reaches the document (grep:
//!   svg.rs / media.rs / subtitles.rs hold none).
//!
//! Failure messages report the first differing byte
//! index and the differing-byte count rather than
//! dumping whole documents (the P47/R12 discipline), so
//! a CI failure carries the measured difference with it.
//!
//! Everything uses only the public API plus the `image`
//! crate's encoders for fixture bytes (the S10 idiom).
//! Timing fixtures set fps 24.0 explicitly (board
//! contract); the writer evaluates nothing by wall-clock
//! time.

use std::io::Cursor;
use std::path::PathBuf;

use stykanimate_rs::model::{Frame, ImageAsset, InstanceKey, Project, SpriteInstance};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleFont, SubtitlePosition, SubtitleStyle};
use stykanimate_rs::svg;
use stykanimate_rs::vfx::{weather_particles, Weather, WeatherKind};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

fn credited(start_frame: usize, end_frame: usize, text: &str, speaker: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: Some(speaker.to_string()),
    }
}

fn uncredited(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

/// A bare stage (480x400, background 0x333333, no
/// figures, sprites, texts, strokes, particles or
/// weather, one empty frame) at an explicit 24 fps —
/// the S12 fixture shape. On this stage the ONLY
/// `<text>` elements in an SVG are the subtitles', the
/// ONLY `<circle>` elements are weather discs, and the
/// ONLY embeds are the assets a leg places.
fn bare_project() -> Project {
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
    project.sprites.clear();
    project.texts.clear();
    project.strokes.clear();
    project.subtitles.clear();
    project.assets.clear();
    project.weather = None;
    project.subtitle_style = SubtitleStyle::default();
    project
}

/// The subtitle fixture shared by legs 1 and 3: a
/// credited TWO-LINE cue followed by a plain one-line
/// cue, both live across the whole span (frame 0
/// included). Display lines, in document order:
/// `MARA: First line`, `Second line`, `We go now.`
fn credited_cues() -> Vec<SubtitleCue> {
    vec![
        credited(0, 48, "First line\nSecond line", "MARA"),
        uncredited(0, 48, "We go now."),
    ]
}

/// Solid-colour JPEG bytes, encoded with the image
/// crate exactly as the media module's own fixtures
/// build them (the S10 idiom).
fn jpeg_bytes(w: u32, h: u32, px: image::Rgb<u8>) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .expect("encode JPEG fixture");
    buf
}

/// Solid-colour PNG bytes (the control format).
fn png_bytes(w: u32, h: u32, px: image::Rgba<u8>) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    buf
}

/// A sprite instance of `asset` centred at (x, y) from
/// frame 0 at the given scale, unrotated (S10's shape).
fn sprite_at(asset: usize, x: f32, y: f32, scale: f32) -> SpriteInstance {
    SpriteInstance {
        asset,
        keys: vec![InstanceKey {
            frame: 0.0,
            x,
            y,
            scale,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }],
    }
}

const JPEG_RGB: [u8; 3] = [200, 30, 30];
const PNG_RGB: [u8; 3] = [0, 200, 0];

/// The dual-asset media placement shared by legs 2 and
/// 3: a 24x18 JPEG (asset 0) and a 20x20 PNG control
/// (asset 1) stored directly in the asset table, each
/// instanced as a sprite at scale 2 on the bare stage —
/// the JPEG's 48x36 quad centred at (120, 200), the
/// PNG's 40x40 quad at (360, 200). Returns the two
/// stored byte strings.
fn place_media(project: &mut Project) -> (Vec<u8>, Vec<u8>) {
    let jpeg = jpeg_bytes(24, 18, image::Rgb(JPEG_RGB));
    assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]), "fixture is a real JPEG");
    let png = png_bytes(20, 20, image::Rgba([PNG_RGB[0], PNG_RGB[1], PNG_RGB[2], 255]));
    project.assets = vec![
        ImageAsset {
            name: "photo.jpg".to_string(),
            png: jpeg.clone(),
        },
        ImageAsset {
            name: "still.png".to_string(),
            png: png.clone(),
        },
    ];
    project.sprites = vec![sprite_at(0, 120.0, 200.0, 2.0), sprite_at(1, 360.0, 200.0, 2.0)];
    (jpeg, png)
}

/// The rain the combined leg flies: 40 drops, a light
/// wind. Particle count is `amount` rounded (the vfx
/// rule), positions a pure hash of the drop index.
fn rain() -> Weather {
    Weather {
        kind: WeatherKind::Rain,
        amount: 40.0,
        wind: 30.0,
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// A per-process, per-leg scratch SVG path under the OS
/// temp dir (the S12 fixture shape).
fn temp_svg(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "styk-wave-t-svg-det-{}-{tag}.svg",
        std::process::id()
    ))
}

/// Write frame 0 through the SVG funnel and own the
/// emitted text (the `svg::write_frame_svg` writer R12
/// pinned).
fn svg_string(project: &Project, tag: &str) -> String {
    let path = temp_svg(tag);
    svg::write_frame_svg(project, 0, &path).expect("write frame SVG");
    let text = std::fs::read_to_string(&path).expect("read frame SVG");
    let _ = std::fs::remove_file(&path);
    text
}

/// Byte-identity with a compact measured difference on
/// failure: first differing index + how many bytes
/// differ + both lengths (R12's `assert_identical`).
fn assert_identical(a: &str, b: &str, context: &str) {
    if a == b {
        return;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let shared = a.len().min(b.len());
    let first_diff = (0..shared).find(|&i| a[i] != b[i]);
    let differing = (0..shared).filter(|&i| a[i] != b[i]).count() + a.len().abs_diff(b.len());
    panic!(
        "{context}: outputs differ — first differing byte at {first_diff:?}, \
         {differing} of {} bytes differ (lengths {} vs {})",
        shared.max(a.len().max(b.len())),
        a.len(),
        b.len()
    );
}

/// The contents of every `<text>` element in the
/// document, in document order (S12's extractor).
fn svg_text_contents(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<text ") {
        let after_open = &rest[start..];
        let Some(gt) = after_open.find('>') else { break };
        let body = &after_open[gt + 1..];
        let Some(end) = body.find("</text>") else { break };
        out.push(body[..end].to_string());
        rest = &body[end..];
    }
    out
}

/// Decode a standard base64 payload (the SVG writer's
/// embed encoding) back to bytes (S10's decoder).
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(b: u8) -> u32 {
        match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a') as u32 + 26,
            b'0'..=b'9' => (b - b'0') as u32 + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let has2 = chunk.len() > 2 && chunk[2] != b'=';
        let has3 = chunk.len() > 3 && chunk[3] != b'=';
        let n = (val(chunk[0]) << 18)
            | (val(chunk[1]) << 12)
            | if has2 { val(chunk[2]) << 6 } else { 0 }
            | if has3 { val(chunk[3]) } else { 0 };
        out.push((n >> 16) as u8);
        if has2 {
            out.push((n >> 8) as u8);
        }
        if has3 {
            out.push(n as u8);
        }
    }
    out
}

/// The payload of the (single) `data:<mime>;base64,…`
/// embed in an SVG document (S10's extractor).
fn embed_payload<'a>(svg: &'a str, mime: &str) -> &'a str {
    let marker = format!("data:{mime};base64,");
    let start = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("the SVG carries no {mime} embed:\n{svg}"))
        + marker.len();
    let end = svg[start..]
        .find('"')
        .map(|i| start + i)
        .expect("the embed payload is quoted");
    &svg[start..end]
}

// ---------------------------------------------------------------------
// Leg 1 — credited multi-line cues: the full subtitle block
// ---------------------------------------------------------------------

#[test]
fn svg_aba_credited_multiline_cues_are_byte_identical() {
    let mut base = bare_project();
    base.subtitles = credited_cues();

    let a1 = svg_string(&base, "cues-a1");
    // Non-vacuity: the full subtitle block reaches the
    // writer — the credited cue's two display lines
    // (prefix on the first ONLY) then the plain cue's
    // line, in stored order. On the bare stage nothing
    // else emits <text>.
    assert_eq!(
        svg_text_contents(&a1),
        vec![
            "MARA: First line".to_string(),
            "Second line".to_string(),
            "We go now.".to_string()
        ],
        "the combined subtitle block must emit the prefixed first line, \
         the raw second line, then the plain cue:\n{a1}"
    );

    // Repeat: the same frame written again is identical.
    let a_repeat = svg_string(&base, "cues-a-repeat");
    assert_identical(&a1, &a_repeat, "SVG credited-cues frame 0 written twice");

    // B: the speaker renamed. Only the composed prefix
    // can move — B proves the A-B-A interleave actually
    // exercises the prefix path, not just generic text.
    let mut changed = base.clone();
    changed.subtitles[0].speaker = Some("JUNIPER".to_string());
    let b = svg_string(&changed, "cues-b");
    assert_ne!(
        a1, b,
        "renaming the speaker changed no SVG output — the prefix is \
         not reaching the SVG writer"
    );
    assert_eq!(
        svg_text_contents(&b)[0],
        "JUNIPER: First line",
        "the renamed speaker must head the first display line:\n{b}"
    );

    // Back to A: byte-identical, no residue.
    let a2 = svg_string(&base, "cues-a2");
    assert_identical(
        &a1,
        &a2,
        "SVG credited-cues A-B-A: frame 0 before vs after the renamed-speaker render",
    );
}

// ---------------------------------------------------------------------
// Leg 2 — a JPEG embed + a PNG control in the same project
// ---------------------------------------------------------------------

#[test]
fn svg_aba_jpeg_and_png_embeds_are_byte_identical() {
    let mut base = bare_project();
    let (jpeg, png) = place_media(&mut base);

    let a1 = svg_string(&base, "media-a1");
    // Non-vacuity: each asset embedded exactly once,
    // under the MIME its own bytes sniff as (IA-R9-3),
    // and each payload is the stored bytes themselves —
    // the embed is a transport, not a re-encode (S10's
    // content pin, restated here so this leg's A is
    // known to carry both embeds before identity is
    // demanded of it).
    assert_eq!(
        a1.matches("data:image/jpeg;base64,").count(),
        1,
        "exactly one JPEG embed, labelled image/jpeg:\n{a1}"
    );
    assert_eq!(
        a1.matches("data:image/png;base64,").count(),
        1,
        "exactly one PNG embed, labelled image/png:\n{a1}"
    );
    assert_eq!(
        base64_decode(embed_payload(&a1, "image/jpeg")),
        jpeg,
        "the JPEG embed's payload must decode to the stored JPEG bytes"
    );
    assert_eq!(
        base64_decode(embed_payload(&a1, "image/png")),
        png,
        "the PNG embed's payload must decode to the stored PNG bytes"
    );

    // Repeat.
    let a_repeat = svg_string(&base, "media-a-repeat");
    assert_identical(&a1, &a_repeat, "SVG media frame 0 written twice");

    // B: the PNG control recoloured (green -> blue, a
    // fresh encode). The document must move with the
    // payload — and only there: B still carries exactly
    // one embed of each kind.
    let mut changed = base.clone();
    changed.assets[1].png = png_bytes(20, 20, image::Rgba([0, 0, 200, 255]));
    let b = svg_string(&changed, "media-b");
    assert_ne!(
        a1, b,
        "recolouring the PNG asset changed no SVG output — the embed \
         payload is not reaching the SVG writer"
    );
    assert_eq!(b.matches("data:image/jpeg;base64,").count(), 1);
    assert_eq!(b.matches("data:image/png;base64,").count(), 1);
    assert_eq!(
        base64_decode(embed_payload(&b, "image/png")),
        changed.assets[1].png,
        "B's PNG payload must be B's stored bytes"
    );
    assert_eq!(
        base64_decode(embed_payload(&b, "image/jpeg")),
        jpeg,
        "B's JPEG payload must still be A's JPEG bytes — the recolour \
         touched one asset only"
    );

    // Back to A: byte-identical, no residue.
    let a2 = svg_string(&base, "media-a2");
    assert_identical(
        &a1,
        &a2,
        "SVG media A-B-A: frame 0 before vs after the recoloured-asset render",
    );
}

// ---------------------------------------------------------------------
// Leg 3 — everything combined + weather + a changed style
// ---------------------------------------------------------------------

#[test]
fn svg_aba_combined_weather_and_top_style_are_byte_identical() {
    let mut base = bare_project();
    base.subtitles = credited_cues();
    let (jpeg, png) = place_media(&mut base);
    base.weather = Some(rain());
    base.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 1.5,
        text_color: 0xFF_E6_80,
        box_color: 0x10_20_30,
        font: SubtitleFont::Shaped,
    };

    let a1 = svg_string(&base, "combo-a1");

    // Non-vacuity, one clause per combined path:
    // (a) the subtitle block, prefixed as in leg 1 —
    //     the Top anchor moves the lines, not their text.
    assert_eq!(
        svg_text_contents(&a1),
        vec![
            "MARA: First line".to_string(),
            "Second line".to_string(),
            "We go now.".to_string()
        ],
        "the combined document's subtitle block:\n{a1}"
    );
    // (b) the changed style reaches the writer: the
    //     text colour is the subtitle fill, and the
    //     font size is the house 20 at scale 1.5.
    assert!(
        a1.contains("fill=\"#FFE680\""),
        "the style's text colour must head the subtitle elements:\n{a1}"
    );
    assert!(
        a1.contains("font-size=\"30.0\""),
        "the style's 1.5 scale must size the subtitle text:\n{a1}"
    );
    // (c) both embeds, payloads verbatim.
    assert_eq!(a1.matches("data:image/jpeg;base64,").count(), 1);
    assert_eq!(a1.matches("data:image/png;base64,").count(), 1);
    assert_eq!(base64_decode(embed_payload(&a1, "image/jpeg")), jpeg);
    assert_eq!(base64_decode(embed_payload(&a1, "image/png")), png);
    // (d) the weather: on the bare stage every <circle>
    //     is a rain disc, and the disc count is exactly
    //     the particle function's count at frame 0.
    let expected_discs = weather_particles(&rain(), 0.0, base.stage, 24.0).len();
    assert_eq!(expected_discs, 40, "the rain fixture flies 40 drops");
    assert_eq!(
        a1.matches("<circle").count(),
        expected_discs,
        "every rain drop must emit exactly one disc:\n{a1}"
    );

    // Repeat.
    let a_repeat = svg_string(&base, "combo-a-repeat");
    assert_identical(&a1, &a_repeat, "SVG combined frame 0 written twice");

    // B: the rain thickens (40 -> 55 drops). The disc
    // count must track the particle function, and the
    // document must differ.
    let mut changed = base.clone();
    changed.weather = Some(Weather {
        kind: WeatherKind::Rain,
        amount: 55.0,
        wind: 30.0,
    });
    let b = svg_string(&changed, "combo-b");
    assert_ne!(
        a1, b,
        "thickening the rain changed no SVG output — the weather is \
         not reaching the SVG writer"
    );
    assert_eq!(
        b.matches("<circle").count(),
        weather_particles(&changed.weather.unwrap(), 0.0, changed.stage, 24.0).len(),
        "B's disc count must track B's particle count"
    );
    assert_eq!(
        svg_text_contents(&b),
        svg_text_contents(&a1),
        "the rain change must leave the subtitle block's text alone"
    );

    // Back to A: byte-identical, no residue.
    let a2 = svg_string(&base, "combo-a2");
    assert_identical(
        &a1,
        &a2,
        "SVG combined A-B-A: frame 0 before vs after the thicker-rain render",
    );
}
