//! Wave R regression e2e — the wave's build outcomes
//! proven end to end through the public API (`EditorCore`
//! ops, the subtitle converters, the asset import funnel
//! and the raster export funnel), not through the
//! internals that implemented them:
//!
//! - R2 (subtitle validation hardening): ONE rule set at
//!   every boundary — the editing ops refuse an invalid
//!   cue (empty range, inverted range, blank text) and
//!   spend no undo step doing it; the SRT/VTT importers
//!   skip an inverted block as bad, clamp a zero-duration
//!   block to the one-frame minimum, and refuse a file
//!   that yields nothing; overlapping cues are valid and
//!   the queries return all live cues in stored order,
//!   `subtitle_at` being exactly the first of them; a
//!   stored list is read verbatim (never re-sorted, and
//!   a stored invalid cue can never be live).
//! - R9 (JPEG import): JPEG bytes import through the
//!   same funnel as PNG (content decides, never the file
//!   name), are stored verbatim, and reach a rendered
//!   frame through a sprite instance; corrupt bytes with
//!   a JPEG header are refused at decode and store
//!   nothing.
//! - R1 (raster subtitle sizing parity): the raster
//!   burn-in lays subtitles out with the shared-helper
//!   geometry (`subtitles::subtitle_font_size` /
//!   `subtitle_line_height` / `subtitle_line_center_y`)
//!   exactly as the stage and the SVG writer do — the
//!   glyph height is the absolute house size at any stage
//!   height (the retired sizing took 6% of the stage
//!   height), and stacked lines step by the shared line
//!   height. These legs assert that geometry through
//!   rendered pixels, calibrated to R1's landed shape
//!   (face span [centre − 8, centre + 11] at scale 1).
//! - R8 (segment-text alpha): the carrier node's
//!   `style.opacity` flows through the evaluated media
//!   stream to the rendered frame — 1.0 paints the full
//!   text colour, 0.5 blends it halfway toward what is
//!   underneath, 0.0 paints nothing at all. (Written
//!   against the board's stored-field contract and
//!   reconciled by the integrator to R8's landed
//!   settlement: the alpha is the carrier style opacity,
//!   r8.md.)
//! - R10 (export scale): a scale parameter on the PNG
//!   sequence and still renders multiplies the output
//!   dimensions (0.5 halves, 2.0 doubles), 1.0 stays
//!   byte-identical to the unscaled exports, and an
//!   invalid scale (zero, negative, non-finite) is
//!   refused before anything is written.
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
    export_png_sequence, export_png_sequence_scaled, render_frame, render_frame_scaled,
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

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-r-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// A core on the demo project at an explicit 24 fps with
/// the subtitle list emptied, so cue counts are exact.
fn subtitle_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core.project.subtitles.clear();
    core
}

fn cue_texts(core: &EditorCore, frame: usize) -> Vec<&str> {
    core.subtitles_at(frame)
        .iter()
        .map(|c| c.text.as_str())
        .collect()
}

/// An empty stage (background 0x333333, no figures, one
/// empty frame) of the given size carrying exactly
/// `cues` under `style`, at 24 fps — the P26 burn-in
/// fixture shape, one level up.
fn bare_stage(stage: [f32; 2], cues: Vec<SubtitleCue>, style: SubtitleStyle) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = stage;
    project.background = 0x333333;
    project.figures.clear();
    project.frames = vec![Frame {
        poses: Vec::new(),
        duration: 1.0,
        easing: None,
        bends: Vec::new(),
    }];
    project.subtitles = cues;
    project.subtitle_style = style;
    project
}

const WHITE: [u8; 3] = [255, 255, 255];

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

/// Group sorted rows into runs of consecutive rows.
fn row_clusters(rows: &[u32]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &y in rows {
        match out.last_mut() {
            Some((_, last)) if *last + 1 == y => *last = y,
            _ => out.push((y, y)),
        }
    }
    out
}

/// How many pixels of exactly `color` sit in `rows`.
fn count_in_rows(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    rows: std::ops::RangeInclusive<u32>,
    color: [u8; 3],
) -> usize {
    let mut n = 0;
    for y in rows {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 == color {
                n += 1;
            }
        }
    }
    n
}

// ---------------------------------------------------------------------
// R2 — the ops enforce the one rule set
// ---------------------------------------------------------------------

#[test]
fn r2_ops_refuse_invalid_cues_and_spend_no_undo_step() {
    let mut core = subtitle_core();
    let depth = core.undo_depth();

    // Zero-length and inverted ranges, and blank text,
    // are the invalid cue under the one rule set: refused,
    // nothing stored, no snapshot taken.
    assert_eq!(core.subtitle_add(5, 5, "zero length", None), None);
    assert_eq!(core.subtitle_add(6, 5, "inverted", None), None);
    assert_eq!(core.subtitle_add(0, 4, "   ", None), None);
    assert_eq!(core.subtitle_add(0, 4, "", None), None);
    assert_eq!(core.subtitle_count(), 0, "refusals store nothing");
    assert_eq!(core.undo_depth(), depth, "refusals spend no undo step");
    assert!(
        core.status.contains("Subtitle add failed"),
        "the refusal speaks on the status line: {}",
        core.status
    );

    // A valid cue lands at exactly one undo step.
    assert_eq!(core.subtitle_add(0, 4, "ok", None), Some(0));
    assert_eq!(core.undo_depth(), depth + 1, "one step for the add");

    // An update to an invalid range is refused the same
    // way: the stored cue is untouched, no step spent.
    assert_eq!(core.subtitle_update(0, 4, 4, "zero", None), None);
    assert_eq!(core.subtitle_update(0, 9, 4, "inverted", None), None);
    assert_eq!(core.subtitle_update(0, 0, 4, "  ", None), None);
    assert_eq!(core.subtitle_cues(), [cue(0, 4, "ok")].as_slice());
    assert_eq!(core.undo_depth(), depth + 1, "refused updates spend nothing");

    // A missing index is refused before anything moves
    // (update and delete alike).
    assert_eq!(core.subtitle_update(9, 0, 4, "ok", None), None);
    assert!(!core.subtitle_delete(9));
    assert_eq!(core.subtitle_count(), 1);
    assert_eq!(core.undo_depth(), depth + 1);

    // A real delete is exactly one step.
    assert!(core.subtitle_delete(0));
    assert_eq!(core.subtitle_count(), 0);
    assert_eq!(core.undo_depth(), depth + 2, "one step for the delete");
}

#[test]
fn r2_overlapping_cues_are_valid_and_first_live_wins() {
    let mut core = subtitle_core();
    // Overlap is a legitimate authoring choice: the ops
    // accept it, and the sorted list keeps (0,10) ahead
    // of (2,6).
    assert_eq!(core.subtitle_add(0, 10, "outer", None), Some(0));
    assert_eq!(core.subtitle_add(2, 6, "inner", None), Some(1));
    assert_eq!(
        core.subtitle_cues(),
        [cue(0, 10, "outer"), cue(2, 6, "inner")].as_slice()
    );

    // Both live: ALL of them, in stored order; the
    // singular query is exactly the list's first element.
    assert_eq!(cue_texts(&core, 3), vec!["outer", "inner"]);
    assert_eq!(core.subtitle_at(3).map(|c| c.text.as_str()), Some("outer"));
    assert_eq!(core.subtitle_at(3), core.subtitles_at(3).first().copied());

    // Half-open liveness at the edges: the inner cue is
    // gone at its end frame, the outer at its own.
    assert_eq!(cue_texts(&core, 0), vec!["outer"]);
    assert_eq!(cue_texts(&core, 2), vec!["outer", "inner"]);
    assert_eq!(cue_texts(&core, 6), vec!["outer"]);
    assert!(cue_texts(&core, 10).is_empty());
    assert_eq!(core.subtitle_at(10), None);
}

#[test]
fn r2_stored_lists_are_read_verbatim_and_invalid_stored_cues_never_live() {
    let mut core = subtitle_core();
    // A list the ops would never build (out of order),
    // as a hand-edited project file could carry: reads
    // follow the STORED order — never re-sorted at read.
    core.project.subtitles = vec![cue(4, 8, "late"), cue(0, 10, "early")];
    assert_eq!(cue_texts(&core, 5), vec!["late", "early"]);
    assert_eq!(core.subtitle_at(5).map(|c| c.text.as_str()), Some("late"));

    // A stored invalid cue (inverted or zero-length) can
    // never be live: the half-open liveness test is
    // unsatisfiable for it at every frame.
    core.project.subtitles = vec![cue(5, 3, "inverted"), cue(6, 6, "zero")];
    for frame in 0..12 {
        assert!(
            core.subtitles_at(frame).is_empty(),
            "frame {frame}: an invalid stored cue must never be live"
        );
        assert_eq!(core.subtitle_at(frame), None);
    }
}

// ---------------------------------------------------------------------
// R2 — the importers enforce the same rule set
// ---------------------------------------------------------------------

#[test]
fn r2_srt_import_skips_inverted_blocks_and_clamps_zero_duration() {
    // Block 1 is inverted (end before start): the same
    // invalid cue as the ops refuse, so it is skipped
    // like any other bad block. Block 2 is zero-duration
    // (a non-inverted range naming a live instant): NOT
    // invalid — it imports as the one-frame minimum cue.
    // Block 3 is ordinary.
    let srt = "\
1
00:00:02,000 --> 00:00:01,000
inverted

2
00:00:01,000 --> 00:00:01,000
instant

3
00:00:03,000 --> 00:00:04,000
fine
";
    let cues = subtitles_from_srt(srt, 24.0).expect("the good blocks import");
    assert_eq!(cues, vec![cue(24, 25, "instant"), cue(72, 96, "fine")]);

    // The clamped one-frame cue is live at exactly its
    // single frame.
    let mut core = subtitle_core();
    core.project.subtitles = cues;
    assert_eq!(cue_texts(&core, 24), vec!["instant"]);
    assert!(cue_texts(&core, 25).is_empty());
    assert_eq!(cue_texts(&core, 72), vec!["fine"]);

    // WebVTT shares the block parser, so the same
    // discipline holds there.
    let vtt = "\
WEBVTT

00:00:02.000 --> 00:00:01.000
inverted

00:00:01.000 --> 00:00:01.000
instant
";
    let cues = subtitles_from_vtt(vtt, 24.0).expect("the good VTT block imports");
    assert_eq!(cues, vec![cue(24, 25, "instant")]);
}

#[test]
fn r2_import_refuses_a_file_that_yields_nothing() {
    // Every block inverted: zero yield. An empty list
    // would silently wipe a project's subtitles on
    // import, so the import refuses outright.
    let srt = "\
1
00:00:02,000 --> 00:00:01,000
inverted

2
00:00:05,000 --> 00:00:03,000
also inverted
";
    assert!(subtitles_from_srt(srt, 24.0).is_err());
    // A missing WEBVTT header refuses before any mining.
    assert!(subtitles_from_vtt("1\n00:00:00,000 --> 00:00:01,000\nx\n", 24.0).is_err());
    // A non-positive frame rate converts nothing.
    assert!(subtitles_from_srt("1\n00:00:00,000 --> 00:00:01,000\nx\n", 0.0).is_err());
}

#[test]
fn r2_converter_round_trip_is_frame_exact_and_import_keeps_file_order() {
    // Export -> import at the project's own rate returns
    // the stored list exactly, multi-line text included.
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.subtitles = vec![cue(0, 25, "alpha"), cue(25, 49, "beta\ngamma")];
    let srt = subtitles_to_srt(&project);
    assert_eq!(
        subtitles_from_srt(&srt, 24.0).expect("own SRT re-imports"),
        project.subtitles
    );
    let vtt = subtitles_to_vtt(&project);
    assert_eq!(
        subtitles_from_vtt(&vtt, 24.0).expect("own VTT re-imports"),
        project.subtitles
    );

    // The converters return cues in FILE order and never
    // sort: normalising is the caller's job.
    let srt = "\
1
00:00:02,000 --> 00:00:03,000
second

2
00:00:00,000 --> 00:00:01,000
first
";
    assert_eq!(
        subtitles_from_srt(srt, 24.0).expect("out-of-order file imports"),
        vec![cue(48, 72, "second"), cue(0, 24, "first")]
    );
}

// ---------------------------------------------------------------------
// R9 — JPEG bytes reach a rendered frame through the import funnel
// ---------------------------------------------------------------------

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

/// Solid-colour PNG bytes (the funnel's original
/// accepted format — the control).
fn png_bytes(w: u32, h: u32, px: image::Rgba<u8>) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, px);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode PNG fixture");
    buf
}

/// A core on a bare 200x150 stage (background 0x101010,
/// no figures, one empty frame), at 24 fps.
fn bare_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core.project.stage = [200.0, 150.0];
    core.project.background = 0x101010;
    core.project.figures.clear();
    core.project.frames = vec![Frame {
        poses: Vec::new(),
        duration: 1.0,
        easing: None,
        bends: Vec::new(),
    }];
    core.project.sprites.clear();
    core.project.texts.clear();
    core
}

#[test]
fn r9_jpeg_asset_imports_and_reaches_a_rendered_frame() {
    let dir = temp_dir("jpeg-render");
    let mut core = bare_core();
    let depth = core.undo_depth();

    // The file's name says nothing (deliberately not a
    // .jpg name): content decides.
    let bytes = jpeg_bytes(24, 18, image::Rgb([200, 30, 30]));
    assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "fixture is a real JPEG");
    let file = dir.join("photo.bin");
    std::fs::write(&file, &bytes).expect("write JPEG file");
    let out = tool_asset_import(&mut core, &json!({ "path": file.to_string_lossy() }))
        .expect("a valid JPEG imports through the funnel");
    assert_eq!(out["width"].as_u64(), Some(24));
    assert_eq!(out["height"].as_u64(), Some(18));
    let idx = out["index"].as_u64().expect("import returns an index") as usize;
    assert_eq!(
        core.project.assets[idx].png, bytes,
        "the stored bytes are the file's bytes, verbatim"
    );
    assert_eq!(core.undo_depth(), depth + 1, "the import is one undo step");

    // Place the asset as a sprite instance centred on
    // the stage (24x18 at scale 2 = a 48x36 quad) and
    // render: the JPEG's colour must be on the frame.
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
    let img = render_frame(&core.project, 0);
    assert_eq!((img.width(), img.height()), (200, 150));
    let centre = img.get_pixel(100, 75).0;
    for (channel, want) in centre.iter().zip([200u8, 30, 30]) {
        assert!(
            channel.abs_diff(want) <= 8,
            "centre pixel {centre:?} must carry the JPEG's colour (JPEG is lossy; solid fields stay close)"
        );
    }
    assert_eq!(
        img.get_pixel(4, 4).0,
        [16, 16, 16],
        "outside the quad the stage background is untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r9_png_still_imports_and_corrupt_jpeg_is_refused() {
    let dir = temp_dir("jpeg-refusals");
    let mut core = bare_core();
    let depth = core.undo_depth();
    let assets_before = core.project.assets.len();

    // The original format still imports through the
    // widened funnel.
    let png = png_bytes(10, 10, image::Rgba([0, 200, 0, 255]));
    let png_file = dir.join("still.png");
    std::fs::write(&png_file, &png).expect("write PNG file");
    let out = tool_asset_import(&mut core, &json!({ "path": png_file.to_string_lossy() }))
        .expect("a valid PNG still imports");
    assert_eq!(out["width"].as_u64(), Some(10));
    assert_eq!(core.project.assets.len(), assets_before + 1);
    assert_eq!(core.undo_depth(), depth + 1);

    // JPEG magic over a garbage body is refused at the
    // decode step: nothing stored, no step spent.
    let mut junk = vec![0xFF, 0xD8, 0xFF];
    junk.extend_from_slice(b"garbagegarbage");
    let junk_file = dir.join("junk.jpg");
    std::fs::write(&junk_file, &junk).expect("write junk file");
    let err = tool_asset_import(&mut core, &json!({ "path": junk_file.to_string_lossy() }))
        .expect_err("JPEG magic over garbage must be refused");
    assert!(err.contains("does not decode"), "err: {err}");
    assert_eq!(core.project.assets.len(), assets_before + 1);
    assert_eq!(core.undo_depth(), depth + 1, "the refusal spent no step");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// R1 — the raster burn-in lays out through the shared helpers
// ---------------------------------------------------------------------

#[test]
fn r1_glyph_height_is_the_house_size_at_any_stage_height() {
    // The shared layout's glyph height is the absolute
    // house size (SUBTITLE_BASE_FONT_SIZE = 20 at scale
    // 1.0), wherever the stage is anchored. The retired
    // sizing took 6% of the stage height, so the same cue
    // grew from ~25 face rows at 400 tall to ~37 at 600.
    let short = bare_stage([480.0, 400.0], vec![cue(0, 30, "HELLO")], SubtitleStyle::default());
    let tall = bare_stage([480.0, 600.0], vec![cue(0, 30, "HELLO")], SubtitleStyle::default());
    let rows_short = rows_with_color(&render_frame(&short, 0), WHITE);
    let rows_tall = rows_with_color(&render_frame(&tall, 0), WHITE);

    // The face span at scale 1 is [centre - 8, centre + 11]
    // = 20 rows at either height (centres 372 and 572).
    assert!(
        (18..=22).contains(&rows_short.len()),
        "400-tall stage: face spans {} rows, expected the 20-row house size",
        rows_short.len()
    );
    assert!(
        (18..=22).contains(&rows_tall.len()),
        "600-tall stage: face spans {} rows — the glyph height must not grow with the stage",
        rows_tall.len()
    );
    // Anchoring still follows the stage: at 600 tall the
    // whole face sits in the bottom band of the taller
    // frame, and nowhere else.
    assert!(
        rows_tall.iter().all(|&y| y >= 540),
        "600-tall stage: subtitle paint outside the bottom band: {rows_tall:?}"
    );
    assert!(
        rows_short.iter().all(|&y| (350..=395).contains(&y)),
        "400-tall stage: subtitle paint outside the bottom band: {rows_short:?}"
    );
}

#[test]
fn r1_stacked_cues_step_by_the_shared_line_height_first_cue_nearest_the_edge() {
    // Two cues live at once, stored in this order: the
    // sparse "I" first, the dense "WWWW" second. The
    // shared layout stacks them one line height (24 at
    // scale 1) apart with the FIRST cue closest to the
    // bottom edge — so the lower face cluster is the
    // sparse one. The retired step was 32.4, at which the
    // two faces (20 rows each) also sit measurably
    // further apart.
    let project = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "I"), cue(0, 30, "WWWW")],
        SubtitleStyle::default(),
    );
    let img = render_frame(&project, 0);
    let clusters = row_clusters(&rows_with_color(&img, WHITE));
    assert_eq!(
        clusters.len(),
        2,
        "two stacked cues must paint two disjoint face clusters: {clusters:?}"
    );
    let (lower, upper) = (clusters[1], clusters[0]); // rows ascend: upper cluster first
    let centre = |c: (u32, u32)| (c.0 + c.1) as f32 / 2.0;
    let step = centre(lower) - centre(upper);
    assert!(
        (20.0..=28.0).contains(&step),
        "stacked face centres are {step} rows apart, expected the shared 24-row line height"
    );
    let lower_count = count_in_rows(&img, lower.0..=lower.1, WHITE);
    let upper_count = count_in_rows(&img, upper.0..=upper.1, WHITE);
    assert!(
        lower_count < upper_count,
        "the first cue (\"I\", {lower_count} white px) sits closest to the edge, \
         below the second cue (\"WWWW\", {upper_count} white px)"
    );
}

#[test]
fn r1_top_anchor_and_font_scale_follow_the_helpers() {
    // Top anchor: the face hangs from the shared margin
    // — centre 28 at scale 1, face rows [20, 39].
    let top = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "HELLO")],
        SubtitleStyle {
            position: SubtitlePosition::Top,
            ..SubtitleStyle::default()
        },
    );
    let rows = rows_with_color(&render_frame(&top, 0), WHITE);
    assert!(
        (18..=22).contains(&rows.len()),
        "top anchor: face spans {} rows, expected 20",
        rows.len()
    );
    assert!(
        rows.iter().all(|&y| (12..=48).contains(&y)),
        "top anchor: face rows {rows:?} must hang from the top margin"
    );

    // Font scale 2.0 doubles the house size (20 -> 40):
    // the face spans 40 rows. The retired sizing gave
    // 400 * 0.06 * 2 = 48.
    let scaled = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "HELLO")],
        SubtitleStyle {
            font_scale: 2.0,
            ..SubtitleStyle::default()
        },
    );
    let rows = rows_with_color(&render_frame(&scaled, 0), WHITE);
    assert!(
        (36..=44).contains(&rows.len()),
        "scale 2.0: face spans {} rows, expected the doubled 40-row house size",
        rows.len()
    );
}

// ---------------------------------------------------------------------
// R8 — segment-text alpha flows to the rendered frame.
// Reconciled by the integrator to R8's landed settlement
// (r8.md): there is no stored alpha field — the text item's
// alpha IS the carrier node's `style.opacity`, the same
// value that fades the carrier's own stroke, so the 0.0
// leg compares against a control built at the SAME
// opacity with the media stripped.
// ---------------------------------------------------------------------

/// The demo project dressed so only the segment text can
/// leave a visible mark: background 0x333333, the figure
/// and every non-carrier node painted in the background
/// colour, the carrier node (the longest segment) white
/// so its text reads white. The carrier's own stroke is
/// present in both the media and no-media renders at the
/// same opacity, so the comparisons below isolate the text.
fn alpha_fixture(alpha: f32) -> Project {
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
    project
}

/// Pixels within `tol` of `target` on every channel.
fn count_near(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    target: [u8; 3],
    tol: u8,
) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y).0;
            if p.iter().zip(target).all(|(a, b)| a.abs_diff(b) <= tol) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn r8_segment_text_alpha_blends_the_rendered_frame() {
    let full = render_frame(&alpha_fixture(1.0), 0);
    let half = render_frame(&alpha_fixture(0.5), 0);
    let mut none_project = alpha_fixture(1.0);
    none_project.figures[0].def.nodes.iter_mut().for_each(|n| n.media = None);
    let none = render_frame(&none_project, 0);
    let gone = render_frame(&alpha_fixture(0.0), 0);
    let mut gone_control_project = alpha_fixture(0.0);
    gone_control_project
        .figures[0]
        .def
        .nodes
        .iter_mut()
        .for_each(|n| n.media = None);
    let gone_control = render_frame(&gone_control_project, 0);

    // alpha 1.0 is the pre-R8 look: the text colour
    // paints at full strength (well beyond the carrier
    // segment's own stroke, which the no-media control
    // also carries).
    let white_full = count_near(&full, WHITE, 0);
    let white_none = count_near(&none, WHITE, 0);
    assert!(
        white_full > white_none + 100,
        "alpha 1.0: {white_full} white px vs the no-media control's {white_none} — the text must paint"
    );

    // alpha 0.5: the glyph face blends halfway toward
    // the 0x333333 background — (255 + 51) / 2 = 153 per
    // channel. The blend colour must cover the face,
    // dwarfing the anti-aliasing accidents the 1.0 render
    // produces near that value.
    let blend = [153, 153, 153];
    let blend_half = count_near(&half, blend, 4);
    let blend_full = count_near(&full, blend, 4);
    assert!(
        blend_half >= 200,
        "alpha 0.5: only {blend_half} px at the halfway blend — the text must blend, not stamp solid"
    );
    assert!(
        blend_half > blend_full * 3,
        "alpha 0.5 blend px ({blend_half}) must dwarf the alpha 1.0 render's \
         anti-aliasing accidents near that value ({blend_full})"
    );

    // alpha 0.0 paints nothing at all: the frame is the
    // no-media render AT THE SAME CARRIER OPACITY, byte
    // for byte (the carrier's stroke fades with the same
    // style opacity, so the control is built at 0.0 too).
    assert_eq!(
        gone.as_raw(),
        gone_control.as_raw(),
        "alpha 0.0 must leave the frame exactly as if the text were not there"
    );
}

// ---------------------------------------------------------------------
// R10 — export scale multiplies output size; 1.0 is unchanged
// ---------------------------------------------------------------------

#[test]
fn r10_still_render_scales_dimensions_and_1_0_is_byte_identical() {
    let project = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "HELLO")],
        SubtitleStyle::default(),
    );
    let half = render_frame_scaled(&project, 0, 0.5).expect("scale 0.5 renders");
    assert_eq!((half.width(), half.height()), (240, 200));
    let double = render_frame_scaled(&project, 0, 2.0).expect("scale 2.0 renders");
    assert_eq!((double.width(), double.height()), (960, 800));

    let one = render_frame_scaled(&project, 0, 1.0).expect("scale 1.0 renders");
    assert_eq!(
        one.as_raw(),
        render_frame(&project, 0).as_raw(),
        "scale 1.0 must be byte-identical to the unscaled still"
    );

    // The scale rule: positive and finite, nothing else.
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            render_frame_scaled(&project, 0, bad).is_err(),
            "scale {bad} must be refused"
        );
    }
}

#[test]
fn r10_png_sequence_scales_and_its_1_0_matches_the_unscaled_export() {
    let project = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "HELLO")],
        SubtitleStyle::default(),
    );
    let dir = temp_dir("export-scale");

    let x2 = dir.join("x2");
    let n = export_png_sequence_scaled(&project, &x2, 2.0).expect("scaled sequence exports");
    assert_eq!(n, 1, "one authored frame exports one file");
    let written = image::open(x2.join("frame-0001.png")).expect("the scaled frame reads back");
    assert_eq!(
        (written.width(), written.height()),
        (960, 800),
        "the sequence's frames are true 2x renders"
    );

    // At 1.0 the scaled export writes exactly what the
    // unscaled export writes, byte for byte.
    let plain = dir.join("plain");
    export_png_sequence(&project, &plain).expect("unscaled sequence exports");
    let x1 = dir.join("x1");
    export_png_sequence_scaled(&project, &x1, 1.0).expect("1.0 sequence exports");
    assert_eq!(
        std::fs::read(plain.join("frame-0001.png")).expect("read unscaled frame"),
        std::fs::read(x1.join("frame-0001.png")).expect("read 1.0 frame"),
        "scale 1.0 sequence output must be byte-identical"
    );

    // A refused scale writes nothing and creates no
    // directory.
    let ghost = dir.join("ghost");
    assert!(export_png_sequence_scaled(&project, &ghost, 0.0).is_err());
    assert!(!ghost.exists(), "a refused scale must not create its directory");
    let _ = std::fs::remove_dir_all(&dir);
}
