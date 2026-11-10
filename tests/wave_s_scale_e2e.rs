//! Wave S (S11) — pixel proof for the SCALED subtitle
//! burn-in: the pin IA-R10-3 was landed without.
//!
//! Wave R's R10 built the export-scale funnel with
//! deliberately cue-free content pins; the integrator's
//! IA-R10-3 then made the raster subtitle burn-in
//! multiply its shared-helper layout by the render scale
//! (src/export.rs: `subtitle_font_size(style) * scale`,
//! `cx * scale`, `subtitle_line_center_y(..) * scale`).
//! Until this file, nothing measured a scaled subtitle
//! pixel: a burn-in that silently kept stamping at stage
//! size on a 2.0 frame (or double-scaled, or anchored to
//! the unscaled edge) passed every existing leg.
//!
//! Derivations (from src/subtitles.rs's helpers, not
//! copied measurements): the house font size is
//! SUBTITLE_BASE_FONT_SIZE = 20 at font_scale 1.0, the
//! line step is SUBTITLE_BASE_LINE_HEIGHT = 24, and the
//! edge margin is SUBTITLE_EDGE_MARGIN = 16. On a
//! 400-tall stage, bottom anchor, one line, the shared
//! centre is 400 − 16 − 24/2 = 372 — 28 units from the
//! anchored edge — and the glyph face spans the 20-row
//! house size (R18's 1.0 calibration: face rows
//! [centre − 8, centre + 11], the half-cell quirk
//! included). The burn-in layout is that geometry
//! multiplied by the render scale, and the output frame
//! is the stage multiplied by the same scale, so at
//! scale s:
//!
//! - face span = 20·s rows (40 at 2.0, 10 at 0.5);
//! - centre sits 28·s units from the anchored edge of
//!   the scaled frame (56 at 2.0, 14 at 0.5);
//! - two stacked lines step 24·s centre-to-centre
//!   (48 at 2.0);
//! - a speaker prefix rides the FIRST line
//!   (`subtitle_display_lines`), so a credited cue's
//!   face keeps the uncredited line's rows and grows
//!   WIDER: "NARRATOR: HELLO" is 15 chars against
//!   "HELLO"'s 5, and a line's run is (4n − 1) glyph
//!   cells, so the face extent grows ~59/19 ≈ 3.1×.
//!
//! Every leg asserts the scaled render against BOTH
//! the derived absolute range and the same project's
//! 1.0 render measured in the same test (the relational
//! half is what catches a layout that scales by the
//! wrong factor while staying plausible in isolation).
//! Fixtures follow R18's shape: bare 480×400 stage,
//! background 0x333333, default style (white text over
//! black backing), fps 24.0 explicit.

use stykanimate_rs::export::render_frame_scaled;
use stykanimate_rs::model::{Frame, Project};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleStyle};

// ---------------------------------------------------------------------
// Fixtures (R18's shape)
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

/// An empty stage (background 0x333333, no figures, one
/// empty frame) of the given size carrying exactly
/// `cues` under the default style, at 24 fps.
fn bare_stage(stage: [f32; 2], cues: Vec<SubtitleCue>) -> Project {
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
    project.subtitle_style = SubtitleStyle::default();
    project
}

const WHITE: [u8; 3] = [255, 255, 255];

type Rgb8 = image::ImageBuffer<image::Rgb<u8>, Vec<u8>>;

/// Rows of `img` that contain at least one pixel of
/// exactly `color`, in ascending order.
fn rows_with_color(img: &Rgb8, color: [u8; 3]) -> Vec<u32> {
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

/// The horizontal extent (min..=max) of `color` pixels.
fn col_extent(img: &Rgb8, color: [u8; 3]) -> (u32, u32) {
    let mut lo = u32::MAX;
    let mut hi = 0u32;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 == color {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo, hi)
}

/// How many pixels of exactly `color` `img` holds.
fn count_color(img: &Rgb8, color: [u8; 3]) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 == color {
                n += 1;
            }
        }
    }
    n
}

/// How many pixels of exactly `color` sit in `rows`.
fn count_in_rows(img: &Rgb8, rows: std::ops::RangeInclusive<u32>, color: [u8; 3]) -> usize {
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

/// The single white face cluster of a one-line render,
/// asserted to be exactly one cluster (nothing else on
/// a bare stage paints white).
fn single_face(img: &Rgb8) -> (u32, u32) {
    let clusters = row_clusters(&rows_with_color(img, WHITE));
    assert_eq!(
        clusters.len(),
        1,
        "a one-line cue must paint exactly one white face cluster: {clusters:?}"
    );
    clusters[0]
}

// ---------------------------------------------------------------------
// Scale 2.0 — face height and anchored position double
// ---------------------------------------------------------------------

#[test]
fn s11_burn_in_face_doubles_at_scale_2() {
    let project = bare_stage([480.0, 400.0], vec![cue(0, 30, "HELLO")]);

    // The 1.0 baseline, measured here: the 20-row house
    // size, centre 28 units from the bottom edge.
    let img1 = render_frame_scaled(&project, 0, 1.0).expect("scale 1.0 renders");
    assert_eq!((img1.width(), img1.height()), (480, 400));
    let face1 = single_face(&img1);
    let span1 = face1.1 - face1.0 + 1;
    assert!(
        (18..=22).contains(&span1),
        "1.0: face spans {span1} rows, expected the 20-row house size"
    );
    let edge1 = edge_offset(&img1, face1);
    assert!(
        (22.0..=32.0).contains(&edge1),
        "1.0: face centre sits {edge1} px from the bottom edge, expected ~28 \
         (margin 16 + half the 24 line height)"
    );

    // At 2.0 the frame doubles and so does the burn-in:
    // 40 face rows, centre 56 from the edge. A burn-in
    // still stamping at stage size would span ~20 rows
    // sitting ~28 (or ~428, if anchored to the unscaled
    // stage) from the edge — both fail here.
    let img2 = render_frame_scaled(&project, 0, 2.0).expect("scale 2.0 renders");
    assert_eq!((img2.width(), img2.height()), (960, 800));
    let face2 = single_face(&img2);
    let span2 = face2.1 - face2.0 + 1;
    assert!(
        (36..=44).contains(&span2),
        "2.0: face spans {span2} rows, expected the doubled 40-row house size"
    );
    let ratio = span2 as f32 / span1 as f32;
    assert!(
        (1.7..=2.3).contains(&ratio),
        "2.0/1.0 face span ratio is {ratio}, expected 2.0"
    );
    let edge2 = edge_offset(&img2, face2);
    assert!(
        (50.0..=62.0).contains(&edge2),
        "2.0: face centre sits {edge2} px from the bottom edge, expected ~56"
    );
    assert!(
        (edge2 - 2.0 * edge1).abs() <= 6.0,
        "2.0 edge offset {edge2} must double the measured 1.0 offset {edge1}"
    );
}

// ---------------------------------------------------------------------
// Scale 0.5 — face height and anchored position halve
// ---------------------------------------------------------------------

#[test]
fn s11_burn_in_face_halves_at_scale_0_5() {
    let project = bare_stage([480.0, 400.0], vec![cue(0, 30, "HELLO")]);

    let img1 = render_frame_scaled(&project, 0, 1.0).expect("scale 1.0 renders");
    let face1 = single_face(&img1);
    let span1 = face1.1 - face1.0 + 1;
    let edge1 = edge_offset(&img1, face1);

    // At 0.5: a 240x200 frame, a 10-row face, centre 14
    // from the edge. A burn-in ignoring the scale would
    // span the full 20 rows at the 1.0 offset.
    let img = render_frame_scaled(&project, 0, 0.5).expect("scale 0.5 renders");
    assert_eq!((img.width(), img.height()), (240, 200));
    let face = single_face(&img);
    let span = face.1 - face.0 + 1;
    assert!(
        (8..=13).contains(&span),
        "0.5: face spans {span} rows, expected the halved 10-row house size"
    );
    let ratio = span as f32 / span1 as f32;
    assert!(
        (0.38..=0.65).contains(&ratio),
        "0.5/1.0 face span ratio is {ratio}, expected 0.5"
    );
    let edge = edge_offset(&img, face);
    assert!(
        (11.0..=17.0).contains(&edge),
        "0.5: face centre sits {edge} px from the bottom edge, expected ~14"
    );
    assert!(
        (edge - edge1 / 2.0).abs() <= 4.0,
        "0.5 edge offset {edge} must halve the measured 1.0 offset {edge1}"
    );
}

// ---------------------------------------------------------------------
// Credited cue at 2.0 — the prefix rides the scaled line
// ---------------------------------------------------------------------

#[test]
fn s11_credited_cue_prefix_scales_with_the_line_at_2_0() {
    let plain = bare_stage([480.0, 400.0], vec![cue(0, 30, "HELLO")]);
    let credited_project = bare_stage(
        [480.0, 400.0],
        vec![credited(0, 30, "HELLO", "NARRATOR")],
    );
    let img_plain = render_frame_scaled(&plain, 0, 2.0).expect("plain 2.0 renders");
    let img_credited =
        render_frame_scaled(&credited_project, 0, 2.0).expect("credited 2.0 renders");

    // Same line, same band: the prefix is composed onto
    // the first (only) line, so the credited face keeps
    // the plain face's rows — no second line, no shift.
    let face_plain = single_face(&img_plain);
    let face_credited = single_face(&img_credited);
    let span_plain = face_plain.1 - face_plain.0 + 1;
    let span_credited = face_credited.1 - face_credited.0 + 1;
    assert!(
        (36..=44).contains(&span_credited),
        "credited 2.0: face spans {span_credited} rows, expected the doubled 40"
    );
    assert!(
        (span_credited as i32 - span_plain as i32).abs() <= 4,
        "credited face span {span_credited} must match the plain span {span_plain} \
         (the prefix adds no line)"
    );
    assert!(
        (centre(face_credited) - centre(face_plain)).abs() <= 4.0,
        "credited face centre {} must sit on the plain face's line ({})",
        centre(face_credited),
        centre(face_plain)
    );

    // Wider by the prefix: the rendered line is
    // "NARRATOR: HELLO" (15 chars, run 59 cells) against
    // "HELLO" (5 chars, run 19 cells) — the white extent
    // grows ~3.1×; the floor below leaves generous slack
    // for edge glyphs while still failing an unprefixed
    // or stage-width render.
    let (lo_p, hi_p) = col_extent(&img_plain, WHITE);
    let (lo_c, hi_c) = col_extent(&img_credited, WHITE);
    let width_plain = hi_p - lo_p + 1;
    let width_credited = hi_c - lo_c + 1;
    assert!(
        width_credited >= 2 * width_plain,
        "credited face is {width_credited} px wide vs the plain {width_plain} px — \
         the scaled prefix must widen the line (~3.1x derived)"
    );
    // Both lines centre on the frame's horizontal centre
    // (each line is centred as a whole string).
    let mid = img_credited.width() as f32 / 2.0;
    let centre_c = (lo_c + hi_c) as f32 / 2.0;
    assert!(
        (centre_c - mid).abs() <= 8.0,
        "credited line centre {centre_c} must sit at the frame centre {mid}"
    );
    // And the wider line paints more face pixels.
    assert!(
        count_color(&img_credited, WHITE) * 2 > count_color(&img_plain, WHITE) * 3,
        "credited render must paint well over 1.5x the plain face's white pixels"
    );
}

// ---------------------------------------------------------------------
// Two-cue stack at 2.0 — the shared line step scales too
// ---------------------------------------------------------------------

#[test]
fn s11_stack_step_doubles_at_scale_2() {
    // R18's stack fixture: the sparse "I" stored first
    // (closest to the bottom edge), the dense "WWWW"
    // second, one shared line height apart.
    let project = bare_stage(
        [480.0, 400.0],
        vec![cue(0, 30, "I"), cue(0, 30, "WWWW")],
    );

    let img1 = render_frame_scaled(&project, 0, 1.0).expect("scale 1.0 renders");
    let clusters1 = row_clusters(&rows_with_color(&img1, WHITE));
    assert_eq!(
        clusters1.len(),
        2,
        "1.0: two stacked cues must paint two disjoint face clusters: {clusters1:?}"
    );
    let step1 = centre(clusters1[1]) - centre(clusters1[0]);
    assert!(
        (20.0..=28.0).contains(&step1),
        "1.0: stacked centres are {step1} rows apart, expected the shared 24-row line height"
    );

    // At 2.0 the faces double (40 rows) and the step
    // doubles with them (48): the clusters stay disjoint
    // (48 > 40) and the first cue still sits nearest the
    // edge. An unscaled step would read ~24 and the two
    // 40-row faces would collide into one cluster.
    let img2 = render_frame_scaled(&project, 0, 2.0).expect("scale 2.0 renders");
    let clusters2 = row_clusters(&rows_with_color(&img2, WHITE));
    assert_eq!(
        clusters2.len(),
        2,
        "2.0: two stacked cues must paint two disjoint face clusters: {clusters2:?}"
    );
    let (lower2, upper2) = (clusters2[1], clusters2[0]); // rows ascend: upper first
    let step2 = centre(lower2) - centre(upper2);
    assert!(
        (40.0..=56.0).contains(&step2),
        "2.0: stacked centres are {step2} rows apart, expected the doubled 48-row line height"
    );
    let ratio = step2 / step1;
    assert!(
        (1.7..=2.3).contains(&ratio),
        "2.0/1.0 stack step ratio is {ratio}, expected 2.0"
    );
    for (i, cluster) in clusters2.iter().enumerate() {
        let span = cluster.1 - cluster.0 + 1;
        assert!(
            (36..=44).contains(&span),
            "2.0: stacked face {i} spans {span} rows, expected the doubled 40"
        );
    }
    // First-stored cue ("I", sparse) still nearest the
    // edge: the lower cluster is the sparse one.
    let lower_count = count_in_rows(&img2, lower2.0..=lower2.1, WHITE);
    let upper_count = count_in_rows(&img2, upper2.0..=upper2.1, WHITE);
    assert!(
        lower_count < upper_count,
        "2.0: the first cue (\"I\", {lower_count} white px) sits closest to the edge, \
         below the second cue (\"WWWW\", {upper_count} white px)"
    );
}
