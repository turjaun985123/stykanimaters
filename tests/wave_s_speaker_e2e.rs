//! Wave S (S12) — speaker rendering proof: the display
//! prefix Wave R's IA-R21-4 wired into the renderers,
//! pinned on the two lib surfaces an integration test
//! can reach.
//!
//! The contract under proof: a credited cue
//! (`SubtitleCue::speaker = Some(name)`) draws as
//! `Name: <first line>` — the prefix rides the FIRST
//! line only, through the one shared helper
//! `subtitles::subtitle_display_lines`, so line counts,
//! stack offsets and band geometry are unchanged; the
//! stored text and the file formats never carry the
//! prefix (SRT least of all — it has no speaker syntax,
//! so a credited cue's SRT export is the raw text, the
//! documented Wave R loss).
//!
//! Surfaces covered here:
//! - SVG (`svg::write_frame_svg`): the `<text>` element
//!   content is the prefixed line, byte-exact.
//! - Raster (`export::render_frame`): the credited
//!   render's subtitle ink band is wider than the
//!   uncredited render of the same text (the prefix's
//!   ink), in exactly the same rows.
//!
//! NOT covered here, by construction: the stage bands
//! live in the bin crate (`rune_sec/stage_text.rs`) and
//! are not nameable from an integration test — the R12
//! boundary. The stage surface's speaker proof is S2's
//! headless subtitle census (a credited seeded scene
//! against the uncredited control), not this file.

use std::path::PathBuf;

use stykanimate_rs::export::render_frame;
use stykanimate_rs::model::{Frame, Project};
use stykanimate_rs::subtitles::{
    subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt, SubtitleCue, SubtitleStyle,
};
use stykanimate_rs::svg;

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
/// figures or any other ink source, one empty frame)
/// carrying exactly `cues` under the default subtitle
/// style, at an explicit 24 fps — the wave_r_e2e
/// burn-in fixture shape. On this stage the ONLY
/// `<text>` elements in an SVG and the ONLY non-
/// background pixels in a raster are the subtitles'.
fn bare_project(cues: Vec<SubtitleCue>) -> Project {
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
    project.subtitles = cues;
    project.subtitle_style = SubtitleStyle::default();
    project
}

/// A per-process, per-test scratch file under the OS
/// temp dir (the tree's established fixture shape).
fn temp_svg(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "styk-wave-s-speaker-{}-{tag}.svg",
        std::process::id()
    ))
}

/// The contents of every `<text>` element in frame 0's
/// SVG, in document order.
fn svg_text_contents(project: &Project, tag: &str) -> Vec<String> {
    let path = temp_svg(tag);
    svg::write_frame_svg(project, 0, &path).expect("write frame SVG");
    let text = std::fs::read_to_string(&path).expect("read frame SVG");
    let _ = std::fs::remove_file(&path);
    let mut out = Vec::new();
    let mut rest = text.as_str();
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

const BACKGROUND: [u8; 3] = [0x33, 0x33, 0x33];

/// The bounding box (min_x, min_y, max_x, max_y) of
/// every non-background pixel — on the bare stage,
/// exactly the subtitle band's ink box (glyph faces
/// plus the one-cell backing outline the burn-in
/// stamps around them, present identically in both
/// renders of a pair).
fn ink_bbox(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 != BACKGROUND {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    any.then_some((min_x, min_y, max_x, max_y))
}

// ---------------------------------------------------------------------
// 1. SVG — the prefix is in the <text> content, first line only
// ---------------------------------------------------------------------

#[test]
fn svg_credited_cue_prefixes_the_first_line_only() {
    // Two-line cue: line one carries the prefix, line
    // two is the raw text — the prefix rides the first
    // line rather than adding one, so the element count
    // is the line count.
    let project = bare_project(vec![credited(0, 48, "First line\nSecond line", "MARA")]);
    assert_eq!(
        svg_text_contents(&project, "svg-two-line"),
        vec!["MARA: First line".to_string(), "Second line".to_string()],
        "a credited cue's SVG lines are `Name: <first line>` then the raw rest"
    );

    // Single-line cue: the whole content is the
    // prefixed line, exactly.
    let project = bare_project(vec![credited(0, 48, "We go now.", "MARA")]);
    assert_eq!(
        svg_text_contents(&project, "svg-one-line"),
        vec!["MARA: We go now.".to_string()],
    );
}

#[test]
fn svg_uncredited_cue_is_its_raw_text() {
    let project = bare_project(vec![uncredited(0, 48, "We go now.")]);
    assert_eq!(
        svg_text_contents(&project, "svg-uncredited"),
        vec!["We go now.".to_string()],
        "an uncredited cue renders its stored text verbatim — no prefix, no decoration"
    );
}

// ---------------------------------------------------------------------
// 2. Raster — the prefix widens the band; the rows do not move
// ---------------------------------------------------------------------

#[test]
fn raster_prefix_widens_the_band_in_the_same_rows() {
    let plain = render_frame(&bare_project(vec![uncredited(0, 48, "We go now.")]), 0);
    let named = render_frame(
        &bare_project(vec![credited(0, 48, "We go now.", "MARA")]),
        0,
    );
    let (px0, py0, px1, py1) = ink_bbox(&plain).expect("the uncredited cue paints ink");
    let (nx0, ny0, nx1, ny1) = ink_bbox(&named).expect("the credited cue paints ink");

    // Same rows, exactly: the prefix rides the existing
    // first (only) line, so the band's vertical extent —
    // face rows plus the shared outline — is unchanged.
    assert_eq!(
        (py0, py1),
        (ny0, ny1),
        "the credited band must sit in exactly the uncredited band's rows"
    );

    // Wider: "MARA: " is six glyph advances in front of
    // the same text — at the house font (20-unit glyphs,
    // 4-unit cells, 4 px per cell at this stage size)
    // that is ~96 px of extra run. Half of that is
    // demanded, so the pin is about the prefix's ink,
    // not about a rounding edge.
    let plain_w = px1 - px0;
    let named_w = nx1 - nx0;
    assert!(
        named_w >= plain_w + 48,
        "the credited band ({named_w}px wide) must be materially wider than \
         the uncredited band ({plain_w}px) — the prefix's ink"
    );
}

// ---------------------------------------------------------------------
// 3. Round-trip composition — a WebVTT voice tag renders prefixed
// ---------------------------------------------------------------------

#[test]
fn vtt_voice_tag_import_renders_with_the_prefix() {
    let vtt = "\
WEBVTT

00:00:00.000 --> 00:00:02.000
<v MARA>We go now.</v>
";
    let cues = subtitles_from_vtt(vtt, 24.0).expect("the voice-tagged cue imports");
    assert_eq!(
        cues,
        vec![credited(0, 48, "We go now.", "MARA")],
        "the voice tag lands in the speaker field, the payload stays raw"
    );

    // Field -> display path, end to end: the imported
    // cue renders through the SVG writer with the
    // prefix composed at draw time.
    let project = bare_project(cues);
    assert_eq!(
        svg_text_contents(&project, "vtt-round-trip"),
        vec!["MARA: We go now.".to_string()],
    );
}

// ---------------------------------------------------------------------
// 4. SRT non-regression — the prefix never reaches stored/file text
// ---------------------------------------------------------------------

#[test]
fn srt_export_of_a_credited_cue_carries_raw_text_only() {
    let project = bare_project(vec![credited(0, 48, "We go now.", "MARA")]);
    let srt = subtitles_to_srt(&project);

    // The raw text is there; the speaker's name is
    // NOWHERE in the document — no prefix baked into
    // the text, no compensating annotation. SRT has
    // no speaker syntax, and the display prefix is a
    // render-time composition, never stored text.
    assert!(
        srt.contains("We go now."),
        "the SRT carries the cue's text verbatim:\n{srt}"
    );
    assert!(
        !srt.contains("MARA"),
        "the SRT must not mention the speaker at all:\n{srt}"
    );

    // And the round trip proves nothing was baked in:
    // re-importing yields the uncredited cue with the
    // text byte-identical (the documented SRT speaker
    // loss — the field drops, the text does not move).
    assert_eq!(
        subtitles_from_srt(&srt, 24.0).expect("own SRT re-imports"),
        vec![uncredited(0, 48, "We go now.")],
    );
}
