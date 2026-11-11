//! Wave S (S6): raster determinism pins — A-B-A byte
//! identity through the export funnel.
//!
//! What the earlier suites pinned, and what this one adds:
//!
//! - P47 (`tests/export_determinism.rs`) pinned the raster
//!   REPEAT claim: the same frame rendered twice back to
//!   back is byte-identical (its frame test compares two
//!   consecutive renders; R12's header describes it as
//!   covering A-B-A too, but the file itself is repeat-only
//!   for frames — this suite is the A-B-A leg it did not
//!   write).
//! - R12 (`tests/wave_r_determinism2.rs`) carried the claim
//!   to the SVG funnel and the stage's evaluated stream —
//!   the two NON-raster surfaces.
//!
//! This suite pins the raster path itself
//! (`export::render_frame` and
//! `export::render_frame_at_scaled`, through the
//! supersampled funnel, the subtitle burn-in and the
//! media composite) under A-B-A: render A, render an
//! unrelated B (a different project that provably renders
//! differently), render A again — the two A frames must
//! be byte-identical. Repeat identity alone cannot catch
//! state that one render leaves behind for the next
//! (caches, thread-locals, leaked scratch); the interleave
//! is the point. Five legs, one per content class Wave R
//! left without a raster A-B-A pin:
//!
//! 1. a plain demo frame;
//! 2. a subtitled frame (two stacked cues, one multi-line)
//!    — the burn-in's overlay path;
//! 3. a credited cue (speaker set) — the rendered
//!    `Name: ` prefix from `subtitle_display_lines`;
//! 4. segment text at carrier opacity 0.35 — the
//!    IA-R8-1 evaluated-alpha media path;
//! 5. scaled renders (2.0 and 0.5) of a subtitled frame —
//!    the R10 funnel with the IA-R10-3 scaled burn-in.
//!
//! Failure messages report the first differing byte index
//! and the differing-byte count rather than dumping whole
//! frames, so a CI failure carries the measured difference
//! with it (P47's discipline).
//!
//! Everything uses only the public API and `std` — the
//! `image` buffer type is named for the render results,
//! as the sibling e2e suites name it. Timing fixtures set
//! fps 24.0 explicitly (board contract); the funnel does
//! not evaluate by wall-clock time.

use stykanimate_rs::export;
use stykanimate_rs::model::{Project, SegmentMedia};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleFont, SubtitlePosition, SubtitleStyle};

/// The demo project cut down to 3 authored frames so CI
/// stays fast (the sibling determinism suites' fixture).
/// Stage size is left at the demo's 960x720.
fn fixture() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.frames.truncate(3);
    assert_eq!(project.frames.len(), 3);
    project
}

fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame: start,
        end_frame: end,
        text: text.to_string(),
        speaker: None,
    }
}

fn credited_cue(start: usize, end: usize, text: &str, speaker: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame: start,
        end_frame: end,
        text: text.to_string(),
        speaker: Some(speaker.to_string()),
    }
}

/// The fixture with an explicit subtitle style (the R12
/// idiom: bottom, scale 1.0, white on black, set — not
/// inherited from the type's defaults).
fn subtitled_fixture(cues: Vec<SubtitleCue>) -> Project {
    let mut project = fixture();
    project.subtitles = cues;
    project.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Bottom,
        font_scale: 1.0,
        text_color: 0xFF_FF_FF,
        box_color: 0x00_00_00,
        font: SubtitleFont::Shaped,
    };
    project
}

/// The two-cue subtitled project of legs 2 and 5: one
/// two-line cue and one single-line cue, both live across
/// the fixture's whole span, so frame 0 stacks both (the
/// burn-in's stack path, not just its single-cue path).
fn stacked_cues_fixture() -> Project {
    subtitled_fixture(vec![
        cue(0, 3, "First line\nsecond line"),
        cue(0, 3, "Stacked below"),
    ])
}

/// Byte-identity with a compact measured difference on
/// failure: first differing index + how many bytes differ
/// + both lengths (P47's discipline, for pixel buffers).
fn assert_bytes_identical(a: &[u8], b: &[u8], context: &str) {
    if a == b {
        return;
    }
    let shared = a.len().min(b.len());
    let first_diff = (0..shared).find(|&i| a[i] != b[i]);
    let differing = (0..shared).filter(|&i| a[i] != b[i]).count() + a.len().abs_diff(b.len());
    panic!(
        "{context}: frames differ — first differing byte at {first_diff:?}, \
         {differing} bytes differ (lengths {} vs {})",
        a.len(),
        b.len()
    );
}

// ---------------------------------------------------------------------
// Leg 1 — plain demo frame
// ---------------------------------------------------------------------

#[test]
fn raster_aba_plain_demo_frame_is_byte_identical() {
    let base = fixture();

    let a1 = export::render_frame(&base, 0);

    // B: the same project with its only figure recoloured
    // — a different project through the same funnel, and
    // it must render differently or the interleave proves
    // nothing.
    let mut changed = base.clone();
    changed.figures[0].color = 0xFF2A2A;
    let b = export::render_frame(&changed, 0);
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "recolouring the only figure changed no raster output — \
         the changed project is not reaching the rasterizer"
    );

    // Back to A: byte-identical, no residue from B.
    let a2 = export::render_frame(&base, 0);
    assert_eq!(
        (a1.width(), a1.height()),
        (a2.width(), a2.height()),
        "plain A-B-A: frame dimensions changed across the interleave"
    );
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A plain frame: frame 0 before vs after the recoloured render",
    );
}

// ---------------------------------------------------------------------
// Leg 2 — subtitled frame (two stacked cues, one multi-line)
// ---------------------------------------------------------------------

#[test]
fn raster_aba_subtitled_frame_is_byte_identical() {
    let base = stacked_cues_fixture();

    let a1 = export::render_frame(&base, 0);

    // Non-vacuity: the burn-in reaches the frame — the
    // same project without cues renders differently.
    let bare = export::render_frame(&subtitled_fixture(vec![]), 0);
    assert_ne!(
        a1.as_raw(),
        bare.as_raw(),
        "two live cues changed no raster output — the subtitle \
         burn-in is not reaching the rasterizer"
    );

    // B: one word of the second cue changed. The burn-in
    // must move with the word.
    let mut changed = base.clone();
    changed.subtitles[1].text = "Stacked above".to_string();
    let b = export::render_frame(&changed, 0);
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "changing one word of a live cue changed no raster output — \
         the cue text is not reaching the burn-in"
    );

    // Back to A: byte-identical, no residue.
    let a2 = export::render_frame(&base, 0);
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A subtitled frame: frame 0 before vs after the changed-cue render",
    );
}

// ---------------------------------------------------------------------
// Leg 3 — credited cue (speaker prefix renders identically)
// ---------------------------------------------------------------------

#[test]
fn raster_aba_credited_cue_is_byte_identical() {
    let base = subtitled_fixture(vec![credited_cue(0, 3, "We go home now", "Narrator")]);

    let a1 = export::render_frame(&base, 0);

    // Non-vacuity: the credit reaches the frame — the
    // same cue uncredited renders differently (the
    // `Narrator: ` prefix paints glyphs the plain line
    // does not have).
    let uncredited = subtitled_fixture(vec![cue(0, 3, "We go home now")]);
    let plain = export::render_frame(&uncredited, 0);
    assert_ne!(
        a1.as_raw(),
        plain.as_raw(),
        "crediting the cue changed no raster output — the speaker \
         prefix is not reaching the burn-in"
    );

    // B: a different speaker name. The prefix must move
    // with the name.
    let mut changed = base.clone();
    changed.subtitles[0].speaker = Some("Guide".to_string());
    let b = export::render_frame(&changed, 0);
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "changing the speaker name changed no raster output — \
         the prefix is not composed from the cue's speaker"
    );

    // Back to A: byte-identical, no residue.
    let a2 = export::render_frame(&base, 0);
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A credited cue: frame 0 before vs after the renamed-speaker render",
    );
}

// ---------------------------------------------------------------------
// Leg 4 — segment-text alpha (carrier style opacity 0.35)
// ---------------------------------------------------------------------

/// The demo project with a text label on the longest
/// segment's carrier node, the carrier's style opacity
/// set to `opacity` — the IA-R8-1 fixture shape (the
/// evaluated `MediaItem::Text` alpha IS the carrier's
/// style opacity; there is no stored alpha field).
fn alpha_fixture(opacity: f32) -> Project {
    let mut project = fixture();
    let carrier = project.figures[0]
        .def
        .nodes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.length.partial_cmp(&b.1.length).expect("lengths are finite"))
        .map(|(i, _)| i)
        .expect("the demo figure has nodes");
    project.figures[0].def.nodes[carrier].media = Some(SegmentMedia::Text {
        content: "HELLO".to_string(),
        size: 40.0,
    });
    project.figures[0].def.nodes[carrier].style.opacity = opacity;
    project
}

#[test]
fn raster_aba_segment_text_alpha_is_byte_identical() {
    let base = alpha_fixture(0.35);

    let a1 = export::render_frame(&base, 0);

    // B: the same fixture at full carrier opacity. The
    // evaluated text alpha differs (0.35 vs 1.0), so the
    // frame must differ — the alpha is reaching the media
    // composite, and this render is the interleave.
    let full = alpha_fixture(1.0);
    let b = export::render_frame(&full, 0);
    assert_ne!(
        a1.as_raw(),
        b.as_raw(),
        "carrier opacity 0.35 vs 1.0 changed no raster output — \
         the segment-text alpha is not reaching the rasterizer"
    );

    // Back to A: byte-identical, no residue.
    let a2 = export::render_frame(&base, 0);
    assert_bytes_identical(
        a1.as_raw(),
        a2.as_raw(),
        "raster A-B-A segment-text alpha: frame 0 before vs after the full-opacity render",
    );
}

// ---------------------------------------------------------------------
// Leg 5 — scaled renders (2.0 and 0.5) of a subtitled frame
// ---------------------------------------------------------------------

#[test]
fn raster_aba_scaled_subtitled_frame_is_byte_identical() {
    let base = stacked_cues_fixture();

    // B is the same interleave at every scale: one word of
    // the second cue changed (leg 2's B).
    let mut changed = base.clone();
    changed.subtitles[1].text = "Stacked above".to_string();

    for (scale, dims) in [(2.0f32, (1920u32, 1440u32)), (0.5, (480, 360))] {
        let a1 = export::render_frame_at_scaled(&base, 0.0, scale)
            .unwrap_or_else(|e| panic!("scale {scale}: the A render was refused: {e}"));
        assert_eq!(
            (a1.width(), a1.height()),
            dims,
            "scale {scale}: output dimensions are not the demo stage scaled"
        );

        let b = export::render_frame_at_scaled(&changed, 0.0, scale)
            .unwrap_or_else(|e| panic!("scale {scale}: the B render was refused: {e}"));
        assert_ne!(
            a1.as_raw(),
            b.as_raw(),
            "scale {scale}: changing one word of a live cue changed no \
             raster output — the scaled burn-in is not reaching the rasterizer"
        );

        let a2 = export::render_frame_at_scaled(&base, 0.0, scale)
            .unwrap_or_else(|e| panic!("scale {scale}: the second A render was refused: {e}"));
        assert_bytes_identical(
            a1.as_raw(),
            a2.as_raw(),
            &format!(
                "raster A-B-A scaled subtitled frame (scale {scale}): \
                 frame 0 before vs after the changed-cue render"
            ),
        );
    }
}
