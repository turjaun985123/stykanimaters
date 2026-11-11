//! Wave T (T19) — cross-feature e2e for Wave S's scaled
//! ANIMATED exports: the format-level proof S4's entries
//! were landed without.
//!
//! Wave S's S4 built `export_gif_scaled`,
//! `export_apng_scaled` and `export_sheet_scaled` (1.0
//! delegates to the unscaled entries; scaled paths
//! re-render every frame through
//! `render_frame_at_scaled`, so the subtitle burn-in
//! scales with the picture — S11 pinned that at the
//! FRAME level). S4's own pins prove dimensions, decoded
//! GIF frame counts and tile bytes on an UNSUBTITLED
//! fixture. What no leg proved: that a scaled GIF's
//! actual decoded frames carry the burn-in at scaled
//! geometry, that a scaled APNG's frames do, and that
//! the sheet's SUBTITLED tile is the scaled render. Those
//! are these legs, on one fixture shape throughout: a
//! bare 480×400 stage, background 0x333333, no figures,
//! four identical empty authored frames at fps 24.0
//! (tween playback off, so the animated exports sample
//! exactly the four authored frames), default subtitle
//! style (white text over black backing).
//!
//! Measurement derivations are S11's, from the shared
//! helpers in src/subtitles.rs (house font 20, line
//! height 24, edge margin 16): at scale s the shaped
//! face spans ~14·s rows (real-font cap ink, ~0.7 of
//! the em) and its centre sits 28·s units from the
//! anchored (bottom) edge — at 2.0 a span of 25..=33
//! rows and an offset of 50..=62; at 0.5 a span of
//! 5..=9 and an offset of 11..=17. The APNG 0.5 leg
//! measures the face near-white (>= 200 per channel):
//! at em 10 the ~1 px strokes never reach full
//! coverage, so no pixel is exact white. A speaker prefix
//! rides the first line, so a credited cue ("NARRATOR:
//! HELLO", 15 chars, run 59 cells) paints a line ~3.1×
//! wider than the same text uncredited ("HELLO", 5
//! chars, run 19 cells).
//!
//! Format decode scope, stated exactly:
//! - GIF is palette-quantised by the encoder, so its
//!   frames are measured with a NEAR-white predicate
//!   (>= 240 per channel — the background is 51 and the
//!   backing box 0, so the predicate isolates face ink)
//!   and geometry ranges, never byte equality.
//! - APNG is lossless PNG: the first frame IS the PNG
//!   default image in files this encoder writes, so it
//!   decodes through `image::open` and is compared
//!   byte-for-byte with the direct scaled render. The
//!   image crate's PNG decoder does not surface APNG
//!   frame iteration here, so the animation's frame
//!   count is asserted on the wire instead: the acTL
//!   header's num_frames and the fcTL chunk count,
//!   read by a small chunk walker below.
//! - The spritesheet is a plain PNG: tiles crop out and
//!   compare byte-for-byte with `render_frame_scaled`,
//!   S4's witness extended to a subtitled frame.

use std::path::{Path, PathBuf};

use stykanimate_rs::export::{
    export_apng, export_apng_range_scaled, export_apng_scaled, export_gif, export_gif_range_scaled,
    export_gif_scaled, export_sheet_range_scaled, export_sheet_scaled, export_spritesheet,
    render_frame_scaled,
};
use stykanimate_rs::model::{Frame, Project};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleStyle};

// ---------------------------------------------------------------------
// Fixtures (wave_s_e2e's bare_stage_n shape)
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

/// An empty stage (background 0x333333, no figures) with
/// `frames` identical empty authored frames, carrying
/// exactly `cues` under the default style, at 24 fps.
fn bare_stage_n(stage: [f32; 2], frames: usize, cues: Vec<SubtitleCue>) -> Project {
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
    project.subtitle_style = SubtitleStyle::default();
    project
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-t-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// ---------------------------------------------------------------------
// Pixel helpers (S11's, plus the GIF near-white variant)
// ---------------------------------------------------------------------

type Rgb8 = image::ImageBuffer<image::Rgb<u8>, Vec<u8>>;

/// A GIF face pixel: the encoder palette-quantises, so
/// the white face is near-white rather than exact. The
/// background (51) and the backing box (0) sit far below
/// the threshold, so this isolates face ink.
fn is_face_white(px: [u8; 3]) -> bool {
    px[0] >= 240 && px[1] >= 240 && px[2] >= 240
}

/// Rows of a decoded GIF frame holding face ink.
fn rows_with_face(img: &Rgb8) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| is_face_white(img.get_pixel(x, y).0)) {
            rows.push(y);
        }
    }
    rows
}

/// A face-ink pixel at the 0.5 scale: at em 10 the strokes are ~1 px
/// wide and unhinted swash anti-aliasing leaves no pixel at full
/// coverage, so the APNG 0.5 leg measures near-white — at >= 200,
/// not the GIF legs' >= 240: at em 10 the regular faces' brightest
/// pixel is 243..=253 and >= 240 counts only 3..=4 rows for
/// Liberation/Noto Regular, below the (5..=9) band. The background
/// (51) and the backing box (0) sit far below 200, so this still
/// isolates face ink.
fn is_face_white_200(px: [u8; 3]) -> bool {
    px[0] >= 200 && px[1] >= 200 && px[2] >= 200
}

/// Rows of a decoded frame holding face ink at the >= 200 predicate.
fn rows_with_face_200(img: &Rgb8) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| is_face_white_200(img.get_pixel(x, y).0)) {
            rows.push(y);
        }
    }
    rows
}

/// How many face-ink pixels a decoded GIF frame holds.
fn count_face(img: &Rgb8) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                n += 1;
            }
        }
    }
    n
}

/// The horizontal extent (min..=max) of face ink in a
/// decoded GIF frame. Only called on frames known to
/// carry a face.
fn face_col_extent(img: &Rgb8) -> (u32, u32) {
    let mut lo = u32::MAX;
    let mut hi = 0u32;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo, hi)
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

/// Centre of a row cluster.
fn centre(cluster: (u32, u32)) -> f32 {
    (cluster.0 + cluster.1) as f32 / 2.0
}

/// Distance from the frame's BOTTOM edge to a cluster's
/// centre — the anchored-edge offset for the default
/// bottom style.
fn edge_offset(img: &Rgb8, cluster: (u32, u32)) -> f32 {
    img.height() as f32 - centre(cluster)
}

/// The single face cluster of a one-line frame, asserted
/// to be exactly one cluster.
fn single_face(rows: &[u32], what: &str) -> (u32, u32) {
    let clusters = row_clusters(rows);
    assert_eq!(
        clusters.len(),
        1,
        "{what}: a one-line cue must paint exactly one face cluster: {clusters:?}"
    );
    clusters[0]
}

// ---------------------------------------------------------------------
// Format decoding
// ---------------------------------------------------------------------

/// Decode every frame of a GIF file to RGB (the S4
/// count pin's decoder path, buffers kept).
fn decode_gif_frames(path: &Path) -> Vec<Rgb8> {
    use image::AnimationDecoder as _;
    let file = std::fs::File::open(path).expect("open GIF");
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file))
        .expect("GIF decoder");
    decoder
        .into_frames()
        .map(|f| {
            let frame = f.expect("GIF frame decodes");
            image::DynamicImage::ImageRgba8(frame.buffer().clone()).to_rgb8()
        })
        .collect()
}

/// What the APNG wire header states: the IHDR size, the
/// acTL frame count, and how many fcTL chunks follow.
struct ApngWire {
    dims: (u32, u32),
    num_frames: u32,
    fctl_chunks: usize,
}

/// Walk a PNG's chunks (pure std — the wire layout is
/// the assertion target, not any decoder's reading of
/// it). Panics on a malformed file: a bad chunk walk is
/// a failed export, not a soft result.
fn apng_wire(bytes: &[u8]) -> ApngWire {
    assert!(
        bytes.len() > 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n",
        "APNG file must start with the PNG signature"
    );
    let mut pos = 8usize;
    let mut wire = ApngWire {
        dims: (0, 0),
        num_frames: 0,
        fctl_chunks: 0,
    };
    while pos + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &bytes[pos + 4..pos + 8];
        let data = &bytes[pos + 8..pos + 8 + len];
        match kind {
            b"IHDR" => {
                wire.dims = (
                    u32::from_be_bytes(data[0..4].try_into().unwrap()),
                    u32::from_be_bytes(data[4..8].try_into().unwrap()),
                );
            }
            b"acTL" => {
                wire.num_frames = u32::from_be_bytes(data[0..4].try_into().unwrap());
            }
            b"fcTL" => wire.fctl_chunks += 1,
            b"IEND" => break,
            _ => {}
        }
        pos += 12 + len;
    }
    wire
}

// ---------------------------------------------------------------------
// GIF at 2.0 — decoded frames carry the scaled burn-in
// ---------------------------------------------------------------------

#[test]
fn t19_gif_at_2_0_frames_carry_the_scaled_burn_in() {
    // The credited cue is live on authored frames 1..3
    // (half-open [1, 3)) of four — frames 0 and 3 of the
    // GIF must be clean, frames 1 and 2 burned in.
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(1, 3, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("gif2x");
    let scaled_path = dir.join("anim-2x.gif");
    let control_path = dir.join("anim-1x.gif");
    let plain_path = dir.join("anim-2x-uncredited.gif");

    let n = export_gif_scaled(&project, &scaled_path, 2.0).expect("scaled GIF exports");
    assert_eq!(n, 4, "the scaled GIF reports the four sampled frames");
    let n = export_gif(&project, &control_path).expect("control GIF exports");
    assert_eq!(n, 4);

    let frames = decode_gif_frames(&scaled_path);
    let control = decode_gif_frames(&control_path);
    assert_eq!(
        frames.len(),
        control.len(),
        "scaled and unscaled GIFs carry the same frame count"
    );
    assert_eq!(frames.len(), 4, "one GIF frame per authored frame");
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(
            (f.width(), f.height()),
            (960, 800),
            "scaled GIF frame {i} is the doubled stage size"
        );
    }
    for (i, f) in control.iter().enumerate() {
        assert_eq!(
            (f.width(), f.height()),
            (480, 400),
            "control GIF frame {i} is the stage size"
        );
    }

    // Out-of-cue frames are clean: no face ink anywhere.
    for i in [0usize, 3] {
        assert_eq!(
            count_face(&frames[i]),
            0,
            "scaled GIF frame {i} is outside the cue's range and must carry no burn-in"
        );
    }

    // Frame 1, scaled: one face cluster at the doubled
    // geometry — span ~28 rows, centre ~56 from the
    // bottom edge (S11's derivation).
    let face2 = single_face(&rows_with_face(&frames[1]), "scaled GIF frame 1");
    let span2 = face2.1 - face2.0 + 1;
    assert!(
        (25..=33).contains(&span2),
        "scaled GIF: face spans {span2} rows, expected the doubled ~28-row shaped cap-ink"
    );
    let edge2 = edge_offset(&frames[1], face2);
    assert!(
        (50.0..=62.0).contains(&edge2),
        "scaled GIF: face centre sits {edge2} px from the bottom edge, expected ~56"
    );

    // The same frame of the 1.0 control, same predicate:
    // the relational half — spans and offsets double.
    let face1 = single_face(&rows_with_face(&control[1]), "control GIF frame 1");
    let span1 = face1.1 - face1.0 + 1;
    assert!(
        (12..=16).contains(&span1),
        "control GIF: face spans {span1} rows, expected the ~14-row shaped cap-ink"
    );
    let ratio = span2 as f32 / span1 as f32;
    assert!(
        (1.7..=2.3).contains(&ratio),
        "scaled/control face span ratio is {ratio}, expected 2.0"
    );
    let edge1 = edge_offset(&control[1], face1);
    assert!(
        (edge2 - 2.0 * edge1).abs() <= 6.0,
        "scaled edge offset {edge2} must double the control's {edge1}"
    );

    // Frame 2 is the cue's second live frame: burned in
    // at the same geometry (the range is [1, 3), not a
    // single frame).
    let face2b = single_face(&rows_with_face(&frames[2]), "scaled GIF frame 2");
    assert_eq!(
        face2b, face2,
        "both live frames of the scaled GIF carry the face on the same rows"
    );

    // The prefix is inside the GIF frame, not just any
    // text: an uncredited export of the same cue at the
    // same scale paints the bare 5-char line; the
    // credited line's extent is ~3.1x wider (derived),
    // floored at 2x for edge-glyph slack.
    let uncredited = bare_stage_n([480.0, 400.0], 4, vec![cue(1, 3, "HELLO")]);
    export_gif_scaled(&uncredited, &plain_path, 2.0).expect("uncredited GIF exports");
    let plain_frames = decode_gif_frames(&plain_path);
    let (lo_c, hi_c) = face_col_extent(&frames[1]);
    let (lo_p, hi_p) = face_col_extent(&plain_frames[1]);
    let width_credited = hi_c - lo_c + 1;
    let width_plain = hi_p - lo_p + 1;
    assert!(
        width_credited >= 2 * width_plain,
        "credited GIF line is {width_credited} px wide vs the uncredited \
         {width_plain} px — the scaled prefix must widen the line (~3.1x derived)"
    );
}

// ---------------------------------------------------------------------
// APNG at 0.5 — halved frames, burn-in in the first frame
// ---------------------------------------------------------------------

#[test]
fn t19_apng_at_0_5_halves_dimensions_and_keeps_the_burn_in() {
    // The credited cue is live on every frame, frame 0
    // included, so the APNG's first frame (its PNG
    // default image) carries the burn-in.
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(0, 4, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("apng05");
    let scaled_path = dir.join("anim-05.png");
    let control_path = dir.join("anim-1x.png");

    let n = export_apng_scaled(&project, &scaled_path, 0.5).expect("scaled APNG exports");
    assert_eq!(n, 4, "the scaled APNG reports the four sampled frames");
    export_apng(&project, &control_path).expect("control APNG exports");

    // Wire level: IHDR halved, acTL counts four frames,
    // one fcTL per frame.
    let bytes = std::fs::read(&scaled_path).expect("read scaled APNG");
    let wire = apng_wire(&bytes);
    assert_eq!(
        wire.dims,
        (240, 200),
        "APNG IHDR is the halved stage size"
    );
    assert_eq!(
        image::image_dimensions(&scaled_path).expect("APNG dimensions read"),
        (240, 200),
        "the image crate reads the same halved IHDR"
    );
    assert_eq!(wire.num_frames, 4, "acTL num_frames counts every frame");
    assert_eq!(wire.fctl_chunks, 4, "one fcTL chunk per frame");

    // The 1.0 control's wire states the unscaled truth.
    let control_bytes = std::fs::read(&control_path).expect("read control APNG");
    let control_wire = apng_wire(&control_bytes);
    assert_eq!(control_wire.dims, (480, 400), "control APNG IHDR");
    assert_eq!(control_wire.num_frames, 4, "control acTL num_frames");

    // Size class: a quarter of the pixels is a smaller
    // file (both are near-flat images; the dims above
    // carry the precise claim, this is the coarse one).
    assert!(
        bytes.len() < control_bytes.len(),
        "scaled APNG ({} bytes) must be smaller than the 1.0 control ({} bytes)",
        bytes.len(),
        control_bytes.len()
    );

    // First frame, decoded: PNG is lossless and the
    // first frame is the default image, so the decoded
    // pixels are the direct 0.5 render byte-for-byte —
    // burn-in included in the compared bytes.
    let first = image::open(&scaled_path)
        .expect("APNG first frame decodes")
        .to_rgb8();
    assert_eq!((first.width(), first.height()), (240, 200));
    let direct = render_frame_scaled(&project, 0, 0.5).expect("direct 0.5 render");
    assert_eq!(
        first.as_raw(),
        direct.as_raw(),
        "the APNG's first frame is the true 0.5 re-render of frame 0"
    );

    // And its face sits at the halved geometry (S11's
    // 0.5 derivation: span ~7 rows, centre ~14 from the
    // bottom edge), measured on the decoded frame — near-white
    // (>= 200): at em 10 no pixel reaches exact white.
    let face = single_face(&rows_with_face_200(&first), "APNG first frame");
    let span = face.1 - face.0 + 1;
    assert!(
        (5..=9).contains(&span),
        "APNG first frame: face spans {span} rows, expected the halved ~7-row shaped cap-ink"
    );
    let edge = edge_offset(&first, face);
    assert!(
        (11.0..=17.0).contains(&edge),
        "APNG first frame: face centre sits {edge} px from the bottom edge, expected ~14"
    );
}

// ---------------------------------------------------------------------
// Spritesheet at 2.0 — tiles are the scaled renders,
// burn-in inside the compared bytes
// ---------------------------------------------------------------------

#[test]
fn t19_sheet_at_2_0_tiles_are_the_scaled_renders_burn_in_included() {
    // Same liveness shape as the GIF leg: the credited
    // cue is live on authored frames 1..3 only, so the
    // sheet's four tiles split into two clean renders
    // (frames 0 and 3) and two burned-in ones (1 and 2).
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(1, 3, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("sheet2x");
    let path = dir.join("sheet-2x.png");

    let n = export_sheet_scaled(&project, &path, 2.0).expect("scaled sheet exports");
    assert_eq!(n, 4, "the scaled sheet reports the four tiled frames");
    let sheet = image::open(&path).expect("sheet decodes").to_rgb8();
    // Four 960x800 tiles on the square-root grid: 2x2.
    assert_eq!(
        (sheet.width(), sheet.height()),
        (1920, 1600),
        "2.0 sheet size: 2 cols x 2 rows of 960x800 tiles"
    );

    let tile = |k: usize| -> Vec<u8> {
        let (ox, oy) = ((k as u32 % 2) * 960, (k as u32 / 2) * 800);
        image::imageops::crop_imm(&sheet, ox, oy, 960, 800)
            .to_image()
            .into_raw()
    };

    // The subtitled tile (frame 1, top-right): its bytes
    // are the direct 2.0 render of that frame — the
    // scaled burn-in is inside the compared bytes, not
    // inferred from dimensions.
    let tile1 = tile(1);
    assert_eq!(
        tile1,
        render_frame_scaled(&project, 1, 2.0)
            .expect("direct render of frame 1")
            .into_raw(),
        "tile 1 must be the 2.0 re-render of subtitled frame 1"
    );
    // Frame 2 is the cue's other live frame: same story.
    assert_eq!(
        tile(2),
        render_frame_scaled(&project, 2, 2.0)
            .expect("direct render of frame 2")
            .into_raw(),
        "tile 2 must be the 2.0 re-render of subtitled frame 2"
    );
    // The clean tiles are the direct renders too, and
    // they differ from the burned-in tile — on this bare
    // stage the burn-in is the ONLY difference between
    // the frames, so the inequality is the non-vacuity
    // witness for the equalities above.
    let tile0 = tile(0);
    assert_eq!(
        tile0,
        render_frame_scaled(&project, 0, 2.0)
            .expect("direct render of frame 0")
            .into_raw(),
        "tile 0 must be the 2.0 re-render of frame 0 (cue not live)"
    );
    assert_eq!(
        tile(3),
        render_frame_scaled(&project, 3, 2.0)
            .expect("direct render of frame 3")
            .into_raw(),
        "tile 3 must be the 2.0 re-render of frame 3 (cue range is half-open)"
    );
    assert_ne!(
        tile0, tile1,
        "the burned-in tile must differ from the clean tile"
    );
}

// ---------------------------------------------------------------------
// 1.0 identity on a subtitled fixture — the scaled
// entries delegate, whatever the frames carry
// ---------------------------------------------------------------------

#[test]
fn t19_scaled_entries_at_1_0_are_byte_identical_on_a_subtitled_fixture() {
    // S4 pinned these trios on an unsubtitled fixture;
    // here every frame carries the credited burn-in, so
    // the identity covers files whose content includes
    // the subtitle stamps. The cue is live throughout.
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(0, 4, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("one-x");

    // GIF trio.
    let (plain, scaled, ranged) = (
        dir.join("gif-plain.gif"),
        dir.join("gif-scaled.gif"),
        dir.join("gif-ranged.gif"),
    );
    export_gif(&project, &plain).expect("plain GIF");
    export_gif_scaled(&project, &scaled, 1.0).expect("scaled-at-1.0 GIF");
    export_gif_range_scaled(&project, 0, 3, &ranged, 1.0).expect("range-scaled-at-1.0 GIF");
    let plain_bytes = std::fs::read(&plain).expect("read plain GIF");
    assert_eq!(
        plain_bytes,
        std::fs::read(&scaled).expect("read scaled GIF"),
        "GIF: scaled entry at 1.0 is byte-identical"
    );
    assert_eq!(
        plain_bytes,
        std::fs::read(&ranged).expect("read ranged GIF"),
        "GIF: range entry at 1.0 is byte-identical"
    );

    // APNG trio — the png encoder is deterministic (the
    // delegation makes both sides the same code path in
    // any case), so the identity holds for APNG too.
    let (plain, scaled, ranged) = (
        dir.join("apng-plain.png"),
        dir.join("apng-scaled.png"),
        dir.join("apng-ranged.png"),
    );
    export_apng(&project, &plain).expect("plain APNG");
    export_apng_scaled(&project, &scaled, 1.0).expect("scaled-at-1.0 APNG");
    export_apng_range_scaled(&project, 0, 3, &ranged, 1.0).expect("range-scaled-at-1.0 APNG");
    let plain_bytes = std::fs::read(&plain).expect("read plain APNG");
    assert_eq!(
        plain_bytes,
        std::fs::read(&scaled).expect("read scaled APNG"),
        "APNG: scaled entry at 1.0 is byte-identical"
    );
    assert_eq!(
        plain_bytes,
        std::fs::read(&ranged).expect("read ranged APNG"),
        "APNG: range entry at 1.0 is byte-identical"
    );

    // Spritesheet trio.
    let (plain, scaled, ranged) = (
        dir.join("sheet-plain.png"),
        dir.join("sheet-scaled.png"),
        dir.join("sheet-ranged.png"),
    );
    export_spritesheet(&project, &plain).expect("plain sheet");
    export_sheet_scaled(&project, &scaled, 1.0).expect("scaled-at-1.0 sheet");
    export_sheet_range_scaled(&project, 0, 3, &ranged, 1.0).expect("range-scaled-at-1.0 sheet");
    let plain_bytes = std::fs::read(&plain).expect("read plain sheet");
    assert_eq!(
        plain_bytes,
        std::fs::read(&scaled).expect("read scaled sheet"),
        "sheet: scaled entry at 1.0 is byte-identical"
    );
    assert_eq!(
        plain_bytes,
        std::fs::read(&ranged).expect("read ranged sheet"),
        "sheet: range entry at 1.0 is byte-identical"
    );
}
