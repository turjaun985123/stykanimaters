//! Wave R (R12): export determinism pins, second suite —
//! the SVG funnel and the stage draw's evaluated stream.
//!
//! P47's suite (`tests/wave_p_determinism.rs`) pinned the
//! RASTER path: `export::render_frame` is a pure function of
//! (project, frame) under repeat, A-B-A and interleave. This
//! suite carries the same claim to the two remaining drawn
//! surfaces:
//!
//! 1. THE SVG FUNNEL — `svg::write_frame_svg` (the writer
//!    `svg::export_svg` applies to every frame). The output
//!    is text, so identity is byte-identity of the emitted
//!    string: repeat renders, and A-B-A renders with a
//!    different project in between, must match exactly. Two
//!    further legs give the funnel's Wave P additions the
//!    same treatment: a live subtitle cue (P27's `<text>`
//!    emission) and graph channels (P17's funnel through
//!    `graph::apply_channels`).
//! 2. THE STAGE DRAW — with one reachability settlement,
//!    stated plainly: `stage_view::stage_shape` (the bin
//!    crate's Shape builder) is NOT reachable from an
//!    integration test — it lives behind `src/main.rs`'s
//!    module tree, and Wave P+Q's CI round 1 proved the
//!    boundary the hard way (E0433, `stykanimate_rs::
//!    stage_view` does not exist). What this suite pins is
//!    the stage draw's complete lib-side evaluated stream —
//!    everything `stage_shape` is a pure function of:
//!    per-figure joint positions from `EditorCore::
//!    positions_f` (the stage's own evaluation, graph
//!    channels included), the frame's `media::media_items`
//!    stream, and the subtitle band geometry composed from
//!    the `subtitles` layout helpers the stage stamps its
//!    glyphs from (`live_subtitle_cues` order, stack
//!    offsets, line centres, font size). Serialised in
//!    order and compared whole, repeat and A-B-A. The
//!    bin-side Shape assembly itself stays pixel-proven by
//!    the headless subtitle census (Wave Q, Q13).
//!
//! Failure messages report the first differing byte index
//! and the differing-byte count rather than dumping whole
//! outputs, so a CI failure carries the measured difference
//! with it (P47's discipline).
//!
//! Everything uses only the public API and `std` — no
//! dev-dependencies. Timing fixtures set fps 24.0
//! explicitly (board contract); neither funnel evaluates
//! by wall-clock time, fps is carried for the same reason
//! P47 carried it.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
use stykanimate_rs::media::media_items;
use stykanimate_rs::model::Project;
use stykanimate_rs::subtitles::{
    live_subtitle_cues, subtitle_font_size, subtitle_line_center_y, subtitle_stack_line_offset,
    SubtitleCue, SubtitlePosition, SubtitleStyle,
};
use stykanimate_rs::svg;

/// The demo project cut down to 3 authored frames so CI
/// stays fast (P47's fixture, unchanged). Stage size is
/// left at the demo's 960x720.
fn fixture() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.frames.truncate(3);
    assert_eq!(project.frames.len(), 3);
    project
}

/// The fixture with an explicit subtitle style (P27's
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
    };
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

/// The fixture with two 2-key graph channels on the demo
/// figure (root X and node 1's angle), so channel
/// evaluation interpolates between keys at frame 1 instead
/// of sampling a constant. Values are chosen well away
/// from the demo's authored pose, so a channelled render
/// can never coincide with the plain one.
fn channelled_fixture() -> Project {
    let mut project = fixture();
    project.channels = vec![
        Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 613.0), Key::flat(2.0, 217.0)],
        },
        Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 33.0), Key::flat(2.0, -20.0)],
        },
    ];
    project
}

/// Render one frame through the SVG funnel and own the
/// emitted text. `write_frame_svg` does not create parent
/// directories; `/tmp` exists, and each caller passes a
/// name unique to its leg so parallel tests never share a
/// path (the svg.rs unit tests' idiom).
fn svg_string(project: &Project, frame: usize, name: &str) -> String {
    let path = std::path::Path::new(name);
    svg::write_frame_svg(project, frame, path).unwrap();
    std::fs::read_to_string(path).unwrap()
}

/// Byte-identity with a compact measured difference on
/// failure: first differing index + how many bytes differ
/// + both lengths (P47's `assert_identical`, for text).
fn assert_identical(a: &str, b: &str, context: &str) {
    if a == b {
        return;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let shared = a.len().min(b.len());
    let first_diff = (0..shared).find(|&i| a[i] != b[i]);
    let differing =
        (0..shared).filter(|&i| a[i] != b[i]).count() + a.len().abs_diff(b.len());
    panic!(
        "{context}: outputs differ — first differing byte at {first_diff:?}, \
         {differing} of {} bytes differ (lengths {} vs {})",
        shared.max(a.len().max(b.len())),
        a.len(),
        b.len()
    );
}

/// The stage draw's evaluated stream at `frame`, serialised
/// in draw order: every figure's joint positions via the
/// stage's own evaluation (`EditorCore::positions_f`,
/// graph channels applied), the frame's media items, and
/// the subtitle band geometry (live cues in stored order,
/// each line's stack offset and centre from the shared
/// layout helpers, at the style's font size).
fn stage_stream(project: &Project, frame: usize) -> String {
    let mut core = EditorCore::new();
    core.project = project.clone();
    let mut out = String::new();
    out.push_str(&format!(
        "stage {:?} style {:?}\n",
        project.stage, project.subtitle_style
    ));
    for fi in 0..project.figures.len() {
        let joints = core.positions_f(frame as f32, fi);
        out.push_str(&format!("figure {fi} joints {joints:?}\n"));
    }
    let items = media_items(project, frame as f32);
    out.push_str(&format!("media {items:?}\n"));
    // Subtitle bands: the geometry the stage's glyph stamps
    // are placed from (stage_text composes purely from
    // these helpers — Wave P, P28).
    let style = project.subtitle_style;
    let mut lines_before = 0usize;
    for live in live_subtitle_cues(project, frame) {
        let lines_in_cue = live.text.split('\n').count();
        for line_index in 0..lines_in_cue {
            let from_edge = subtitle_stack_line_offset(
                lines_before,
                lines_in_cue,
                line_index,
                style.position,
            );
            let center_y = subtitle_line_center_y(style, project.stage[1], from_edge);
            out.push_str(&format!(
                "band cue {:?} line {line_index} from_edge {from_edge} \
                 center_y {center_y} font {}\n",
                live.text,
                subtitle_font_size(style)
            ));
        }
        lines_before += lines_in_cue;
    }
    out
}

// ---------------------------------------------------------------------
// SVG funnel
// ---------------------------------------------------------------------

#[test]
fn svg_repeat_and_aba_are_byte_identical() {
    let base = fixture();

    // Repeat: the same frame written twice is byte-identical.
    let a1 = svg_string(&base, 0, "/tmp/styk-det2-svg-repeat-a1.svg");
    let a2 = svg_string(&base, 0, "/tmp/styk-det2-svg-repeat-a2.svg");
    assert_identical(&a1, &a2, "SVG frame 0 written twice");

    // A-B-A: a different project rendered in between (the
    // only figure recoloured — the SVG writer resolves
    // segment colour through the instance colour, as the
    // rasterizer does) must change the output, and must
    // leave no residue in the writer.
    let mut changed = base.clone();
    changed.figures[0].color = 0xFF2A2A;
    let b = svg_string(&changed, 0, "/tmp/styk-det2-svg-aba-b.svg");
    assert_ne!(
        a1, b,
        "recolouring the only figure changed no SVG output — the \
         changed project is not reaching the SVG writer"
    );
    let a3 = svg_string(&base, 0, "/tmp/styk-det2-svg-aba-a3.svg");
    assert_identical(&a1, &a3, "SVG A-B-A: frame 0 before vs after the changed render");
}

#[test]
fn svg_aba_subtitle_cue_is_byte_identical_and_sensitive() {
    // One cue live across the fixture's whole span (P27's
    // <text> path).
    let base = subtitled_fixture(vec![cue(0, 3, "Hello world")]);

    let a1 = svg_string(&base, 0, "/tmp/styk-det2-svg-sub-a1.svg");
    // Non-vacuity: the cue reaches the writer as exactly
    // one <text> carrying its words.
    assert_eq!(
        a1.matches("<text").count(),
        1,
        "one live cue must emit exactly one <text>:\n{a1}"
    );
    assert!(
        a1.contains(">Hello world</text>"),
        "the cue's words are not in the SVG text element:\n{a1}"
    );

    // B: one word changed. The emitted element must move
    // with the word.
    let mut changed = base.clone();
    changed.subtitles[0].text = "Hello there".to_string();
    let b = svg_string(&changed, 0, "/tmp/styk-det2-svg-sub-b.svg");
    assert_ne!(
        a1, b,
        "changing one word of a live cue changed no SVG output — the \
         cue text is not reaching the SVG writer"
    );
    assert!(
        b.contains(">Hello there</text>"),
        "the changed cue's words are not in the SVG text element:\n{b}"
    );

    // Back to A: byte-identical, no residue.
    let a2 = svg_string(&base, 0, "/tmp/styk-det2-svg-sub-a2.svg");
    assert_identical(&a1, &a2, "SVG subtitle A-B-A: frame 0 before vs after the changed-cue render");
}

#[test]
fn svg_aba_graph_channels_are_byte_identical_and_move_the_frame() {
    let channelled = channelled_fixture();
    let plain = fixture();

    // Frame 1: both channels interpolate between their
    // keys, so the funnel evaluates `graph::apply_channels`
    // mid-segment (P17's path), not at a key.
    let a1 = svg_string(&channelled, 1, "/tmp/styk-det2-svg-chan-a1.svg");
    let repeat = svg_string(&channelled, 1, "/tmp/styk-det2-svg-chan-repeat.svg");
    assert_identical(&a1, &repeat, "SVG channelled frame 1 written twice");

    // Non-vacuity: the channels move the frame — the plain
    // project at the same frame differs.
    let b = svg_string(&plain, 1, "/tmp/styk-det2-svg-chan-b.svg");
    assert_ne!(
        a1, b,
        "graph channels changed no SVG output at frame 1 — the \
         channelled evaluation is not reaching the SVG writer"
    );

    // Back to A after the plain render in between.
    let a2 = svg_string(&channelled, 1, "/tmp/styk-det2-svg-chan-a2.svg");
    assert_identical(&a1, &a2, "SVG channels A-B-A: frame 1 before vs after the plain render");
}

// ---------------------------------------------------------------------
// Stage draw — the evaluated stream (see the header for the
// reachability settlement)
// ---------------------------------------------------------------------

#[test]
fn stage_stream_repeat_and_aba_are_identical() {
    let base = fixture();

    let a1 = stage_stream(&base, 0);
    let a2 = stage_stream(&base, 0);
    assert_identical(&a1, &a2, "stage stream frame 0 built twice");

    // A-B-A with a geometrically different project in
    // between: the channelled fixture's root and angle
    // channels move every joint the stage would draw.
    let b = stage_stream(&channelled_fixture(), 0);
    assert_ne!(
        a1, b,
        "graph channels changed no stage-stream positions — the \
         channelled evaluation is not reaching the stage stream"
    );

    let a3 = stage_stream(&base, 0);
    assert_identical(&a1, &a3, "stage stream A-B-A: frame 0 before vs after the channelled build");
}

#[test]
fn stage_stream_subtitle_cue_is_sensitive_and_leaves_no_residue() {
    let bare = stage_stream(&subtitled_fixture(vec![]), 0);

    // A not-live cue contributes no band: its stream equals
    // the cue-less stream (Q6's "nothing otherwise"
    // discipline, at band level).
    let not_live = stage_stream(&subtitled_fixture(vec![cue(5, 9, "Too late")]), 0);
    assert_identical(&bare, &not_live, "stage stream: a cue outside frame 0 adds no band");

    // One live cue: exactly one band line joins the stream.
    let base = subtitled_fixture(vec![cue(0, 3, "Hello world")]);
    let a1 = stage_stream(&base, 0);
    assert_eq!(
        a1.matches("band cue").count(),
        1,
        "one live one-line cue must add exactly one band:\n{a1}"
    );
    assert_ne!(
        a1, bare,
        "a live cue changed no stage-stream band — the subtitle \
         layout is not reaching the stage stream"
    );

    // B: one word changed — the band's cue text is part of
    // the stream, so the stream must move with the word.
    let mut changed = base.clone();
    changed.subtitles[0].text = "Hello there".to_string();
    let b = stage_stream(&changed, 0);
    assert_ne!(
        a1, b,
        "changing one word of a live cue changed no stage-stream \
         band — the cue text is not reaching the stage stream"
    );

    // Back to A: identical, no residue.
    let a2 = stage_stream(&base, 0);
    assert_identical(&a1, &a2, "stage stream subtitle A-B-A: frame 0 before vs after the changed-cue build");
}
