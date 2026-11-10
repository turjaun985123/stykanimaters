//! Wave P (P25) — WebVTT discipline pins.
//!
//! Board contract 1 (Wave P): `src/subtitles.rs` carries
//! `subtitles_to_vtt` / `subtitles_from_vtt`, fps-
//! parameterised, mirroring the SRT pair's discipline —
//! bad cue blocks are skipped, and zero yield is the one
//! whole-file refusal — plus the one refusal SRT has no
//! analogue for: a missing `WEBVTT` header refuses
//! whole-file up front, because without the signature
//! the text is not WebVTT at all. Metadata blocks
//! (`STYLE` / `REGION` / `NOTE`) are skipped silently:
//! not cues, not errors, and not yield.
//!
//! fps is 24.0 explicitly everywhere (contract 3: never
//! assume a demo rate). Frame↔timestamp conversion is
//! rounding on both sides of the wire (export:
//! `round(frame * 1000 / fps)` milliseconds; import:
//! `round(seconds * fps)` frames), which is what makes
//! the off-grid cue below (frame 7 → 292 ms → frame 7)
//! round-trip exactly.
//!
//! INTEGRATOR SEQUENCING NOTE: these pins compile only
//! once P23's `src/subtitles.rs` module (and the
//! `pub mod subtitles;` line) lands. P25 must not edit
//! src/subtitles.rs or src/lib.rs, so if CI reaches this
//! file before P23, the failure is a sequencing error,
//! not a behaviour regression.

use stykanimate_rs::model::Project;
use stykanimate_rs::subtitles::{
    subtitles_from_srt, subtitles_from_vtt, subtitles_to_srt, subtitles_to_vtt, SubtitleCue,
};

/// The frame rate every pin in this file runs at,
/// named once so no test can drift onto a demo rate.
const FPS: f32 = 24.0;

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

/// The shared fixture: an on-grid cue, an off-grid cue
/// (frame 7 is not a whole number of milliseconds at
/// 24 fps), and a multi-line cue — text is stored and
/// exported verbatim, so the lines must survive.
fn fixture_cues() -> Vec<SubtitleCue> {
    vec![
        cue(0, 24, "First cue"),
        cue(7, 30, "Off-grid cue"),
        cue(48, 73, "Line one\nLine two"),
    ]
}

/// A project carrying `cues` at this file's frame rate.
/// Both exporters read the rate from the project, so it
/// is set here, explicitly, alongside the cue list.
fn fixture_project(cues: Vec<SubtitleCue>) -> Project {
    let mut project = Project::new_demo();
    project.fps = FPS;
    project.subtitles = cues;
    project
}

#[test]
fn vtt_round_trip_is_frame_exact_including_an_off_grid_cue() {
    let cues = fixture_cues();
    let project = fixture_project(cues.clone());

    let exported = subtitles_to_vtt(&project);
    assert!(
        exported.starts_with("WEBVTT\n\n"),
        "the export opens with the signature and a blank line:\n{exported}"
    );
    // The wire format itself is pinned, not just round-
    // trip self-consistency: dot millisecond separators,
    // rounded from the frame (7 frames at 24 fps is
    // 291.67 ms, written as 292).
    assert!(
        exported.contains("00:00:00.292 --> 00:00:01.250"),
        "the off-grid cue's timing line is written with dot \
         separators and rounded milliseconds:\n{exported}"
    );

    let imported = subtitles_from_vtt(&exported, FPS).expect("own export re-imports");
    assert_eq!(
        imported, cues,
        "export → import at 24 fps returns every cue frame-exact, \
         off-grid frames and multi-line text included"
    );
}

#[test]
fn a_file_without_the_webvtt_header_refuses_whole_file() {
    // SRT-shaped text whose cues are perfectly parseable
    // as blocks: the refusal is about the missing
    // signature, not about cue quality — nothing is
    // mined from a file that is not WebVTT. (Behaviour
    // is pinned, not the refusal's wording: the contract
    // fixes the refusal, not its voice — the same
    // deliberate non-assertion as P24's SRT pins.)
    let srt_shaped = "1\n00:00:00,000 --> 00:00:01,000\nFirst cue\n\n\
                      2\n00:00:01,000 --> 00:00:02,000\nSecond cue\n";
    subtitles_from_vtt(srt_shaped, FPS)
        .expect_err("headerless text refuses even when its cues parse");

    // Dot-timestamp cues without the signature refuse
    // the same way — the header check precedes any cue
    // parsing, so this is the header refusal, not a
    // zero-yield refusal.
    let dot_shaped = "00:00:00.000 --> 00:00:01.000\nFirst cue\n";
    subtitles_from_vtt(dot_shaped, FPS)
        .expect_err("headerless dot-timestamp text refuses whole-file");

    // The empty string has no first line to sign it:
    // same refusal, reached before zero-yield can apply.
    subtitles_from_vtt("", FPS).expect_err("empty text has no header");
}

#[test]
fn a_style_block_is_skipped_not_a_cue_and_not_an_error() {
    // A STYLE block carries presentation data this model
    // does not keep. It sits between the header and the
    // cues, is skipped silently, and the real cue after
    // it imports untouched.
    let text = "WEBVTT\n\n\
                STYLE\n::cue {\n  color: lime;\n}\n\n\
                1\n00:00:01.000 --> 00:00:03.000\nStyled cue\n";
    let imported = subtitles_from_vtt(text, FPS).expect("a STYLE block is tolerated");
    assert_eq!(
        imported,
        vec![cue(24, 72, "Styled cue")],
        "exactly the real cue imports — the STYLE block is \
         neither a cue nor an error"
    );

    // A file of header + STYLE only yields nothing:
    // metadata does not count as yield, so the import
    // still refuses (zero yield).
    subtitles_from_vtt("WEBVTT\n\nSTYLE\n::cue { color: lime; }\n", FPS)
        .expect_err("metadata alone is zero yield");
}

#[test]
fn srt_and_vtt_imports_of_the_same_cues_agree() {
    // Cross-format consistency: one cue list, exported
    // through both writers, re-imported through both
    // parsers, must land on the same cues — the two
    // disciplines differ only on the wire (signature,
    // comma vs dot), never in what a cue means.
    let cues = fixture_cues();
    let project = fixture_project(cues.clone());

    let from_srt = subtitles_from_srt(&subtitles_to_srt(&project), FPS)
        .expect("own SRT export re-imports");
    let from_vtt = subtitles_from_vtt(&subtitles_to_vtt(&project), FPS)
        .expect("own VTT export re-imports");

    assert_eq!(from_srt, cues, "the SRT round trip is frame-exact");
    assert_eq!(from_vtt, cues, "the WebVTT round trip is frame-exact");
    assert_eq!(
        from_srt, from_vtt,
        "both parsers land on identical cue lists for the same cues"
    );
}

#[test]
fn an_empty_project_exports_the_bare_header_and_refuses_reimport() {
    let project = fixture_project(Vec::new());

    // An empty WebVTT file is the bare signature — not
    // SRT's empty string: a WebVTT file without its
    // header is not a WebVTT file at all.
    let exported = subtitles_to_vtt(&project);
    assert_eq!(
        exported, "WEBVTT\n\n",
        "a project with no cues exports as exactly the bare header"
    );

    // …and the format has no way to say "zero cues"
    // back: re-importing the bare header is the one
    // whole-file refusal, zero yield.
    subtitles_from_vtt(&exported, FPS)
        .expect_err("the bare header refuses re-import (zero yield)");
}
