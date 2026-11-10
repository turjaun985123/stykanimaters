//! Wave S (S18) — cross-feature end-to-end: Wave R's
//! features combined the way a real project combines
//! them. The sibling suites prove each feature in
//! isolation (R18/R19 for Wave R's, S10/S11/S12 for the
//! pixel proofs); these legs prove the combinations do
//! not interfere:
//!
//! - **Credited cue + frame range + export scale.** A
//!   credited cue exports through the scaled PNG-sequence
//!   range entry: in-range frames carry the prefixed
//!   burn-in at scaled geometry (face height, band
//!   position, and the prefix widening the line at the
//!   scaled size), out-of-range frames are byte-identical
//!   to the cue-free control, and every exported file is
//!   the same render the direct scaled entry produces.
//! - **JPEG asset + subtitled project.** One fixture
//!   holding a JPEG asset (imported through the funnel),
//!   credited cues and a non-default subtitle style
//!   round-trips through save→load byte-identically (the
//!   P22 method), through the real `projects::save` /
//!   `projects::load` file path too.
//! - **Speaker across the file formats.** WebVTT import
//!   reads voice tags into the field; SRT export drops
//!   the speaker (no syntax for it) while keeping the
//!   text verbatim; WebVTT export restores the voice tag
//!   from the field. The field is the single source of
//!   truth across the lossy hop — nothing is ever
//!   smuggled through the cue text.
//! - **Segment-text alpha + speaker subtitle in one
//!   frame.** Both compositing paths in a single render:
//!   the alpha leg's blend pixels are asserted on their
//!   own population, the speaker leg's on the subtitle
//!   band's ink, and each population is proven invariant
//!   under the other feature's toggle.
//!
//! Fixtures set fps explicitly (24.0) per the wave's
//! timing contract; every test builds its own fixtures
//! and its own temp directory — no test reads another
//! test's files.

use std::io::Cursor;
use std::path::PathBuf;

use serde_json::json;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export::{
    export_png_sequence_range_scaled, render_frame, render_frame_scaled,
};
use stykanimate_rs::media::tool_asset_import;
use stykanimate_rs::model::{Frame, InstanceKey, Project, SegmentMedia, SpriteInstance};
use stykanimate_rs::subtitles::{
    subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt, subtitles_to_vtt, SubtitleCue,
    SubtitlePosition, SubtitleStyle,
};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

fn credited(start_frame: usize, end_frame: usize, text: &str, speaker: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: Some(speaker.to_string()),
    }
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-s-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// An empty stage (background 0x333333, no figures) with
/// `frames` identical empty authored frames, carrying
/// exactly `cues` under `style`, at 24 fps — the
/// wave_r_e2e `bare_stage` shape with a frame count.
fn bare_stage_n(
    stage: [f32; 2],
    frames: usize,
    cues: Vec<SubtitleCue>,
    style: SubtitleStyle,
) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = stage;
    project.background = 0x333333;
    project.figures.clear();
    project.frames = (0..frames)
        .map(|_| Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    project.subtitles = cues;
    project.subtitle_style = style;
    project
}

/// Solid-colour JPEG bytes, encoded with the image
/// crate exactly as the media module's own fixtures
/// build them.
fn jpeg_bytes(w: u32, h: u32, px: image::Rgb<u8>) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .expect("encode JPEG fixture");
    buf
}

const WHITE: [u8; 3] = [255, 255, 255];
const BG: [u8; 3] = [51, 51, 51]; // 0x333333

/// Rows of `img` that contain at least one pixel of
/// exactly `color`, in ascending order.
fn rows_with_color(img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>, color: [u8; 3]) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| img.get_pixel(x, y).0 == color) {
            rows.push(y);
        }
    }
    rows
}

/// Pixels within `tol` of `target` on every channel,
/// counted over `rows` only — leg 4 counts its blend
/// population above the subtitle band, so the two
/// features' pixel populations are disjoint by
/// construction, not by luck.
fn count_near_rows(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    target: [u8; 3],
    tol: u8,
    rows: std::ops::Range<u32>,
) -> usize {
    let mut n = 0;
    for y in rows {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y).0;
            if p.iter().zip(target).all(|(a, b)| a.abs_diff(b) <= tol) {
                n += 1;
            }
        }
    }
    n
}

/// The (min, max) x columns carrying non-background ink
/// in `rows` — the horizontal extent of whatever was
/// stamped there.
fn ink_extents(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    rows: std::ops::Range<u32>,
) -> Option<(u32, u32)> {
    let mut min = u32::MAX;
    let mut max = 0u32;
    for y in rows {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 != BG {
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    if min == u32::MAX {
        None
    } else {
        Some((min, max))
    }
}

/// How many non-background pixels sit in `rows`.
fn ink_count(img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>, rows: std::ops::Range<u32>) -> usize {
    let mut n = 0;
    for y in rows {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 != BG {
                n += 1;
            }
        }
    }
    n
}

/// Rows in which `a` and `b` differ in at least one
/// pixel (same-sized images).
fn differing_rows(
    a: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    b: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
) -> Vec<u32> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut rows = Vec::new();
    for y in 0..a.height() {
        if (0..a.width()).any(|x| a.get_pixel(x, y) != b.get_pixel(x, y)) {
            rows.push(y);
        }
    }
    rows
}

// ---------------------------------------------------------------------
// Leg 1 — credited cue + frame range + export scale
// ---------------------------------------------------------------------

#[test]
fn credited_cue_range_export_at_scale_2_carries_the_prefixed_burn_in_only_inside_the_cue() {
    // Six authored frames; the credited cue is live on
    // frames 2 and 3 only (half-open [2, 4)).
    let project = bare_stage_n(
        [480.0, 400.0],
        6,
        vec![credited(2, 4, "We go at dawn.", "Ana")],
        SubtitleStyle::default(),
    );
    let control = bare_stage_n([480.0, 400.0], 6, Vec::new(), SubtitleStyle::default());
    let uncredited = bare_stage_n(
        [480.0, 400.0],
        6,
        vec![cue(2, 4, "We go at dawn.")],
        SubtitleStyle::default(),
    );
    let dir = temp_dir("credited-range-scale");
    let out = dir.join("seq");
    let n = export_png_sequence_range_scaled(&project, 0, 5, &out, 2.0)
        .expect("scaled range export writes");
    assert_eq!(n, 6, "the whole range exports, one file per frame");

    // The scaled subtitle band: at 2.0 the face centre is
    // 2 * (400 - 24 - 12) = 728 and the face spans ~40
    // rows; all burn-in ink (face + one-cell outline)
    // stays in the bottom 160 rows of the 800-tall frame.
    let band = 640..800u32;
    for i in 0..6usize {
        let file = out.join(format!("frame-{:04}.png", i + 1));
        let img = image::open(&file)
            .expect("exported frame reads back")
            .to_rgb8();
        assert_eq!(
            (img.width(), img.height()),
            (960, 800),
            "frame {i}: the exported frame is a true 2x render"
        );
        // The range entry and the direct scaled entry are
        // the same render.
        let direct = render_frame_scaled(&project, i, 2.0).expect("direct scaled render");
        assert_eq!(
            img.as_raw(),
            direct.as_raw(),
            "frame {i}: the exported file is the direct scaled render, byte for byte"
        );
        let ctrl = render_frame_scaled(&control, i, 2.0).expect("control render");
        if (2..4).contains(&i) {
            // In the cue's range: the burn-in is present,
            // confined to the scaled band, at scaled
            // geometry.
            assert_ne!(
                img.as_raw(),
                ctrl.as_raw(),
                "frame {i}: a live credited cue must burn in"
            );
            let diff = differing_rows(&img, &ctrl);
            assert!(
                diff.iter().all(|&y| band.contains(&y)),
                "frame {i}: burn-in paint outside the scaled band: {diff:?}"
            );
            let face_rows = rows_with_color(&img, WHITE);
            assert!(
                (36..=44).contains(&face_rows.len()),
                "frame {i}: face spans {} rows, expected the 40-row scaled house size",
                face_rows.len()
            );
            // The prefix widens the centred line BOTH
            // ways at the scaled size: the credited
            // render's band ink reaches further left and
            // further right than the uncredited render's.
            let plain = render_frame_scaled(&uncredited, i, 2.0).expect("uncredited render");
            let (cmin, cmax) = ink_extents(&img, band.clone()).expect("credited band has ink");
            let (umin, umax) = ink_extents(&plain, band.clone()).expect("uncredited band has ink");
            assert!(
                cmin < umin && cmax > umax,
                "frame {i}: the 'Ana: ' prefix must widen the line at scale 2 \
                 (credited {cmin}..{cmax}, uncredited {umin}..{umax})"
            );
        } else {
            // Outside the cue's range the scaled range
            // export is the cue-free render, byte for byte
            // — the cue leaves no trace on frames it does
            // not cover.
            assert_eq!(
                img.as_raw(),
                ctrl.as_raw(),
                "frame {i}: no cue is live, so no burn-in may appear"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — JPEG asset + credited cues + style: one document,
// byte-identical round trip
// ---------------------------------------------------------------------

#[test]
fn jpeg_asset_credited_cues_and_style_round_trip_byte_identically() {
    let dir = temp_dir("jpeg-subtitled-roundtrip");
    let mut core = EditorCore::new();
    core.project.fps = 24.0;

    // The JPEG enters through the one import funnel
    // (content decides; the file name says nothing).
    let bytes = jpeg_bytes(24, 18, image::Rgb([200, 30, 30]));
    let file = dir.join("photo.bin");
    std::fs::write(&file, &bytes).expect("write JPEG file");
    let out = tool_asset_import(&mut core, &json!({ "path": file.to_string_lossy() }))
        .expect("a valid JPEG imports through the funnel");
    let idx = out["index"].as_u64().expect("import returns an index") as usize;

    // The asset is a document citizen: a sprite instance
    // places it on the stage.
    core.project.sprites.push(SpriteInstance {
        asset: idx,
        keys: vec![InstanceKey {
            frame: 0.0,
            x: 100.0,
            y: 75.0,
            scale: 2.0,
            angle: 0.0,
        }],
    });

    // Credited cues through the ops (the speaker is
    // normalised and stored as a field, never in text).
    assert_eq!(
        core.subtitle_add(0, 48, "We go at dawn.", Some("Ana")),
        Some(0)
    );
    assert_eq!(
        core.subtitle_add(48, 96, "Then the long road home.", Some("Bo")),
        Some(1)
    );

    // A non-default style rides along in the same
    // document.
    core.project.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 1.5,
        text_color: 0xFFD94D,
        box_color: 0x201008,
    };

    // Non-vacuity: the serialised document visibly holds
    // all three Wave R citizens at once.
    let text1 = serde_json::to_string_pretty(&core.project).expect("serialise fixture");
    assert!(
        text1.contains("\"speaker\": \"Ana\"") && text1.contains("\"speaker\": \"Bo\""),
        "the document must carry both speakers as fields"
    );
    assert!(
        text1.contains("\"font_scale\": 1.5"),
        "the document must carry the non-default style"
    );

    // The P22 method: serialise → load → re-serialise is
    // byte-identical, and the loaded values are the
    // fixture's values.
    let back: Project = serde_json::from_str(&text1).expect("reload fixture");
    let text2 = serde_json::to_string_pretty(&back).expect("re-serialise loaded fixture");
    assert_eq!(
        text2, text1,
        "JPEG asset + credited cues + style: save→load→re-serialise is not byte-identical"
    );
    assert_eq!(
        back.assets[idx].png, bytes,
        "the JPEG bytes survive the round trip verbatim"
    );
    assert_eq!(back.subtitles, core.project.subtitles, "the cues survive exactly");
    assert_eq!(
        back.subtitle_style, core.project.subtitle_style,
        "the style survives exactly"
    );

    // The same identity through the real file path.
    let path = stykanimate_rs::projects::save(&core.project, &dir).expect("save via projects::save");
    let loaded = stykanimate_rs::projects::load(&path).expect("load via projects::load");
    assert_eq!(
        serde_json::to_string_pretty(&loaded).expect("re-serialise file-loaded project"),
        text1,
        "the file path round trip is byte-identical too"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — speaker across VTT -> project -> SRT -> VTT
// ---------------------------------------------------------------------

#[test]
fn speaker_survives_the_vtt_srt_vtt_flow_with_srt_as_the_only_lossy_hop() {
    let vtt_src = "WEBVTT\n\
        \n\
        00:00:01.000 --> 00:00:03.000\n\
        <v Ana>We go at dawn.</v>\n\
        \n\
        00:00:03.000 --> 00:00:05.000\n\
        <v Bo>Then the long road home.</v>\n";
    let cues = subtitles_from_vtt(vtt_src, 24.0).expect("the VTT source parses");
    assert_eq!(
        cues,
        vec![
            credited(24, 72, "We go at dawn.", "Ana"),
            credited(72, 120, "Then the long road home.", "Bo"),
        ],
        "voice tags land in the speaker field, text verbatim"
    );

    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.subtitles = cues.clone();

    // The SRT hop: text verbatim, speaker gone — SRT has
    // no speaker syntax and none is invented into the
    // text.
    let srt = subtitles_to_srt(&project);
    assert!(srt.contains("We go at dawn."), "SRT carries the first text: {srt}");
    assert!(
        srt.contains("Then the long road home."),
        "SRT carries the second text: {srt}"
    );
    assert!(
        !srt.contains("Ana") && !srt.contains("<v"),
        "SRT must not represent the speaker anywhere: {srt}"
    );
    assert_eq!(
        subtitles_from_srt(&srt, 24.0).expect("the SRT export re-imports"),
        vec![
            cue(24, 72, "We go at dawn."),
            cue(72, 120, "Then the long road home."),
        ],
        "the SRT hop drops exactly the speakers and nothing else"
    );

    // The VTT hop restores the voice tags from the field
    // — the field, not the text, was the source of truth
    // all along.
    let vtt = subtitles_to_vtt(&project);
    assert!(
        vtt.contains("<v Ana>We go at dawn.</v>"),
        "VTT export restores the first voice tag: {vtt}"
    );
    assert!(
        vtt.contains("<v Bo>Then the long road home.</v>"),
        "VTT export restores the second voice tag: {vtt}"
    );
    assert_eq!(
        subtitles_from_vtt(&vtt, 24.0).expect("the VTT export re-imports"),
        cues,
        "the VTT round trip is exact, speakers included"
    );
}

// ---------------------------------------------------------------------
// Leg 4 — segment-text alpha + speaker subtitle in ONE frame
// ---------------------------------------------------------------------

/// The demo project dressed for the combined leg: the
/// longest segment's node carries white text at the
/// given carrier opacity (R18's alpha fixture shape),
/// everything else painted into the 0x333333 background,
/// plus one subtitle cue (credited or not) live at
/// frame 0 under the default style.
fn combined_project(alpha: f32, speaker: Option<&str>) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.background = 0x333333;
    project.sprites.clear();
    project.texts.clear();
    project.strokes.clear();
    project.figures[0].color = 0x333333;
    let carrier = project.figures[0]
        .def
        .nodes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.length.partial_cmp(&b.1.length).expect("lengths are finite"))
        .map(|(i, _)| i)
        .expect("the demo figure has nodes");
    for (i, node) in project.figures[0].def.nodes.iter_mut().enumerate() {
        node.color = Some(if i == carrier { 0xFFFFFF } else { 0x333333 });
    }
    project.figures[0].def.nodes[carrier].media = Some(SegmentMedia::Text {
        content: "HELLO".to_string(),
        size: 40.0,
    });
    project.figures[0].def.nodes[carrier].style.opacity = alpha;
    project.subtitles = vec![SubtitleCue {
        start_frame: 0,
        end_frame: 30,
        text: "We go at dawn.".to_string(),
        speaker: speaker.map(str::to_string),
    }];
    project.subtitle_style = SubtitleStyle::default();
    project
}

#[test]
fn segment_text_alpha_and_speaker_subtitle_composite_in_one_frame_independently() {
    let credited_half = render_frame(&combined_project(0.5, Some("Ana")), 0);
    let plain_half = render_frame(&combined_project(0.5, None), 0);
    let credited_full = render_frame(&combined_project(1.0, Some("Ana")), 0);
    let height = credited_half.height();
    // The subtitle band: the default style's face centre
    // sits 36 units above the bottom edge at scale 1;
    // the bottom 70 rows hold the whole band (face +
    // outline) and nothing of the segment text, which
    // rides the figure's longest segment mid-stage.
    let band = (height - 70)..height;

    // The alpha population: white segment text (and its
    // carrier's stroke) at carrier opacity 0.5 over the
    // 0x333333 background blends to the halfway colour
    // (255 + 51) / 2 = 153 per channel. Counted above
    // the band only, so the subtitle stamps (solid white
    // and black, in the band) can neither add to nor
    // cover this population.
    let blend = [153, 153, 153];
    let above_band = 0..(height - 70);
    let blend_credited = count_near_rows(&credited_half, blend, 4, above_band.clone());
    let blend_plain = count_near_rows(&plain_half, blend, 4, above_band.clone());
    let blend_full = count_near_rows(&credited_full, blend, 4, above_band);
    assert!(
        blend_credited >= 200,
        "alpha 0.5 in the combined frame: only {blend_credited} px at the halfway blend — \
         the segment text must blend, not stamp solid"
    );
    assert_eq!(
        blend_credited, blend_plain,
        "the speaker toggle must not move the alpha population \
         (credited {blend_credited}, uncredited {blend_plain})"
    );
    assert!(
        blend_full * 3 < blend_credited,
        "alpha 1.0 leaves only anti-aliasing accidents near the blend \
         ({blend_full} vs the 0.5 frame's {blend_credited})"
    );

    // The speaker population: the subtitle band's ink.
    // The credited line ("Ana: We go at dawn.") is longer
    // than the uncredited one, so its band carries more
    // ink — in the SAME frame where the alpha blend
    // above is being asserted.
    let ink_credited = ink_count(&credited_half, band.clone());
    let ink_plain = ink_count(&plain_half, band);
    assert!(
        ink_credited > ink_plain,
        "the 'Ana: ' prefix must widen the subtitle band in the combined frame \
         (credited {ink_credited} ink px, uncredited {ink_plain})"
    );
}
