//! Wave P (P24) — SRT discipline pins for the subtitles core.
//!
//! Written against the Wave P board's fixed contract 1, which P23
//! lands concurrently: `src/subtitles.rs` provides
//! `SubtitleCue { start_frame, end_frame, text }` (half-open frame
//! range), `subtitles_to_srt(project) -> String` and
//! `subtitles_from_srt(text, fps) -> Result<Vec<SubtitleCue>, String>`.
//! The design mirrors the proven Tauri-line shapes, so these pins
//! encode that line's settled SRT discipline:
//!
//! - a round trip is frame-exact at the project's frame rate, on
//!   grid AND off grid (timestamps are milliseconds; the writer
//!   rounds frame→ms to the nearest millisecond and the parser
//!   rounds back, so an off-grid frame like 7 at 24 fps — exported
//!   as 292 ms — lands back on frame 7);
//! - the parser returns cues in FILE order; sorting is the
//!   caller's normalisation, never the parser's;
//! - malformed discipline: a bad block among good ones is skipped
//!   and the good cues import; a file with nothing parseable is
//!   refused whole-file (zero yield is the one refusal);
//! - an empty project exports as the empty string, and the empty
//!   string refuses re-import — the format cannot say "zero cues"
//!   back.
//!
//! Everything below uses only the public API and `std` — no
//! dev-dependencies. Timing is pinned at 24.0 fps everywhere
//! (board contract 3); the demo project runs at 12 fps, so the
//! rate is always set explicitly. No error WORDING is asserted:
//! the contract fixes the refusal behaviour, not its voice.

use stykanimate_rs::model::Project;
use stykanimate_rs::subtitles::{subtitles_from_srt, subtitles_to_srt, SubtitleCue};

/// The frame rate every pin in this file runs at.
const FPS: f32 = 24.0;

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
    }
}

/// A project carrying exactly `cues`, timed at 24 fps. The demo
/// project supplies the rest of the (subtitle-irrelevant) state;
/// the exporter reads only `fps` and `subtitles`.
fn project_with(cues: Vec<SubtitleCue>) -> Project {
    let mut project = Project::new_demo();
    project.fps = FPS;
    project.subtitles = cues;
    project
}

#[test]
fn round_trip_is_frame_exact_for_on_grid_cues() {
    // Every boundary here is a whole number of milliseconds at
    // 24 fps (24 frames = 1000 ms), so nothing rounds at all.
    let cues = vec![
        cue(0, 24, "The gate is open."),
        cue(24, 48, "Now the yard."),
        cue(48, 96, "And the long road home."),
    ];
    let project = project_with(cues.clone());
    let srt = subtitles_to_srt(&project);
    let imported = subtitles_from_srt(&srt, FPS).expect("own export re-imports");
    assert_eq!(
        imported, cues,
        "an on-grid round trip is frame-exact, text and order preserved"
    );
}

#[test]
fn round_trip_is_frame_exact_for_an_off_grid_cue() {
    // Frame 7 at 24 fps is 291.66… ms — not representable in an
    // SRT timestamp. The writer rounds to the nearest millisecond
    // (292) and the parser rounds back, so the cue must return
    // to frame 7 exactly, not 6 or 8.
    let cues = vec![cue(7, 30, "Off grid, on purpose.")];
    let project = project_with(cues.clone());
    let srt = subtitles_to_srt(&project);
    assert!(
        srt.contains("00:00:00,292 --> 00:00:01,250"),
        "frame 7 exports as 292 ms and frame 30 as 1250 ms at 24 fps: {srt:?}"
    );
    let imported = subtitles_from_srt(&srt, FPS).expect("own export re-imports");
    assert_eq!(
        imported, cues,
        "the off-grid cue round trips onto its exact frames"
    );
}

#[test]
fn round_trip_preserves_text_and_order_for_a_multi_cue_project() {
    // Text is stored verbatim: punctuation, a comma (legal inside
    // cue text — only the timing line's comma is structural), an
    // embedded newline (a two-line cue is one SRT block), and
    // non-ASCII characters all survive the round trip.
    let cues = vec![
        cue(0, 24, "The gate is open."),
        cue(24, 48, "Wait — come back, please."),
        cue(48, 96, "First line\nSecond line"),
        cue(96, 120, "Café at noon."),
    ];
    let project = project_with(cues.clone());
    let srt = subtitles_to_srt(&project);
    let imported = subtitles_from_srt(&srt, FPS).expect("own export re-imports");
    assert_eq!(
        imported, cues,
        "a multi-cue round trip preserves every cue's frames, text and order"
    );
}

#[test]
fn parser_returns_file_order_and_the_caller_sorts() {
    // The second cue in the file starts earlier than the first.
    // The parser's contract is to return what the file says, in
    // the file's order — normalising the order is the caller's
    // job (the ops and the MCP import handlers sort after
    // parsing), so an out-of-order file is NOT silently reordered
    // by the parser.
    let srt = "1\n\
               00:00:02,000 --> 00:00:03,000\n\
               Later cue, first in the file\n\
               \n\
               2\n\
               00:00:00,000 --> 00:00:01,000\n\
               Earlier cue, second in the file\n";
    let parsed = subtitles_from_srt(srt, FPS).expect("both blocks are well-formed");
    assert_eq!(
        parsed,
        vec![
            cue(48, 72, "Later cue, first in the file"),
            cue(0, 24, "Earlier cue, second in the file"),
        ],
        "the parser returns cues in file order, unsorted"
    );

    // The caller's sort normalisation (by start, then end) then
    // yields the chronological list — the shape the project
    // stores.
    let mut sorted = parsed.clone();
    sorted.sort_by_key(|c| (c.start_frame, c.end_frame));
    assert_eq!(
        sorted,
        vec![
            cue(0, 24, "Earlier cue, second in the file"),
            cue(48, 72, "Later cue, first in the file"),
        ],
        "after the caller's sort the same cues are chronological"
    );
}

#[test]
fn import_skips_a_bad_block_and_keeps_the_good_cues() {
    // Malformed discipline: block 2 has no timing line at all.
    // It is skipped; blocks 1 and 3 import with exact frames.
    let srt = "1\n\
               00:00:00,000 --> 00:00:01,000\n\
               Good one\n\
               \n\
               2\n\
               this block is broken\n\
               \n\
               3\n\
               00:00:02,000 --> 00:00:03,500\n\
               Good two\n";
    let imported = subtitles_from_srt(srt, FPS).expect("a partial file imports");
    assert_eq!(
        imported,
        vec![cue(0, 24, "Good one"), cue(48, 84, "Good two")],
        "the bad block is skipped and both good cues survive, frame-exact"
    );
}

#[test]
fn import_refuses_when_nothing_parses() {
    // Zero yield is the one whole-file refusal: a file in which
    // no block parses must be an error, never an empty cue list
    // (an empty list would silently wipe a project's subtitles
    // on import).
    assert!(
        subtitles_from_srt("not a subtitle file at all\n", FPS).is_err(),
        "garbage text refuses"
    );
    assert!(
        subtitles_from_srt("1\nno timing line here\neither\n", FPS).is_err(),
        "a file whose only block is broken refuses"
    );
    assert!(
        subtitles_from_srt("  \n\n", FPS).is_err(),
        "whitespace only refuses"
    );
}

#[test]
fn empty_project_exports_empty_and_empty_text_refuses() {
    // The writer's zero-cue shape is the empty string (the
    // format has no header to emit), and that string is itself
    // a zero-yield refusal on the way back in — "no cues" cannot
    // be round-tripped through SRT, by design.
    let project = project_with(Vec::new());
    let exported = subtitles_to_srt(&project);
    assert_eq!(exported, "", "a project with no cues exports as empty SRT");
    assert!(
        subtitles_from_srt(&exported, FPS).is_err(),
        "the empty export refuses re-import (zero yield)"
    );
}

#[test]
fn import_refuses_a_non_positive_fps() {
    // A non-positive rate cannot convert timestamps to frames,
    // even for a perfectly good file.
    let project = project_with(vec![cue(0, 24, "Timed.")]);
    let srt = subtitles_to_srt(&project);
    assert!(
        subtitles_from_srt(&srt, 0.0).is_err(),
        "import at 0 fps is refused"
    );
    assert!(
        subtitles_from_srt(&srt, -24.0).is_err(),
        "import at a negative fps is refused"
    );
}
