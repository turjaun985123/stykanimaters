//! Wave V (V8) — the subtitle round trip at REGISTRY
//! level: the same journey Wave S's S18 proved against
//! the converters in `subtitles`, driven here entirely
//! through `mcp::all_tools()` — the surface an MCP
//! client actually touches.
//!
//! What was already proven, and where:
//!
//! * S18 (`tests/wave_s_e2e.rs`, leg 3) proved the
//!   converter level: WebVTT voice tags land in the
//!   speaker field, SRT drops exactly the speakers and
//!   nothing else, and the VTT round trip over
//!   `subtitles_to_vtt` / `subtitles_from_vtt` is exact.
//! * The two MCP modules pin their own handlers
//!   in-crate (`mcp_subtitle_tools`, `mcp_subtitle_io`):
//!   reply shapes, the one-snapshot discipline, the
//!   identical-import no-op.
//!
//! What no leg had done is compose those handlers the
//! way a client must: build a credited cue list with
//! `subtitle_add`, replace the style with
//! `subtitle_set_style`, carry the cues across cores
//! with `subtitle_export_vtt` / `subtitle_import_vtt`,
//! and read the result back with `subtitle_list` —
//! asserting at each hop the REPLY (indices after the
//! sorted insert, `changed`, the style as stored), not
//! just the core state underneath. That composition is
//! this file, plus the SRT leg's `speakers_dropped`
//! count read from the export reply (Wave S, S9's
//! surfacing, asserted here at registry level for the
//! first time).
//!
//! The style half of the claim needs its terms stated
//! plainly: a WebVTT document carries cues, not the
//! project style, so no format hop can transport it.
//! What the registry surface guarantees — and leg 1
//! asserts — is that the style is settable and readable
//! through the registry in its stored (post-clamp) form
//! (`subtitle_set_style`'s echo == `subtitle_list`'s
//! `style`), and that `subtitle_import_vtt` replaces
//! the cue list ONLY: a non-default style set on the
//! target core before the import is still in force
//! after it, word for word.
//!
//! Fixtures: every core is `EditorCore::new()` (the
//! demo project, 12 fps) with `project.fps` set to
//! 24.0 explicitly — timestamps convert at the project
//! rate, so the rate is part of the fixture, never
//! inherited. Cue boundaries sit on whole and half
//! seconds (frames 12/36/84/108 at 24 fps = 0.5/1.5/
//! 3.5/4.5 s), which are exact in the formats'
//! millisecond timestamps in both directions, so the
//! frame ranges must round-trip exactly — no rounding
//! tolerance is granted anywhere in this file. Files
//! live in a per-test temp dir; nothing touches a
//! CWD-relative store, so no serial guard is needed.

use std::path::PathBuf;

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::all_tools;

// ---------------------------------------------------------------------
// Helpers and fixture constants
// ---------------------------------------------------------------------

/// Drive one tool from the real registry by name — the
/// `wave_u_e2e` invoke idiom: a missing name is a
/// failure here, not a skip.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-v-subtitle-registry-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fresh core at this file's frame rate (see the
/// module doc: the demo's own 12 fps is never used).
fn fresh_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core
}

const TEXT_COLOR: u32 = 0xFF_E08A;
const BOX_COLOR: u32 = 0x10_2030;
const DEFAULT_TEXT_COLOR: u32 = 0xFF_FFFF;

/// The non-default style every leg sets: top-anchored,
/// 1.5× scale (inside the op's 0.5–3.0 clamp, so the
/// stored form equals the offered form), colours off
/// the defaults on both channels.
fn style_args() -> Value {
    json!({
        "position": "top",
        "font_scale": 1.5,
        "text_color": TEXT_COLOR,
        "box_color": BOX_COLOR,
    })
}

/// The style as stored, in `subtitle_style_json`'s
/// shape — what `subtitle_set_style` echoes and
/// `subtitle_list` reports.
fn expected_style() -> Value {
    json!({
        "position": "top",
        "font_scale": 1.5,
        "text_color": TEXT_COLOR,
        "box_color": BOX_COLOR,
    })
}

fn default_style() -> Value {
    json!({
        "position": "bottom",
        "font_scale": 1.0,
        "text_color": DEFAULT_TEXT_COLOR,
        "box_color": 0,
    })
}

/// The three fixture cues as `subtitle_list` reports
/// them, in kept (sorted) order: two credited, one
/// not — the uncredited cue's object carries NO
/// `speaker` key, which the equality below asserts by
/// construction.
fn expected_cues() -> Value {
    json!([
        {
            "index": 0,
            "start_frame": 12,
            "end_frame": 36,
            "text": "The light is still on.",
            "speaker": "NOVA",
        },
        {
            "index": 1,
            "start_frame": 36,
            "end_frame": 84,
            "text": "Then we walk home before dark.",
            "speaker": "REN",
        },
        {
            "index": 2,
            "start_frame": 84,
            "end_frame": 108,
            "text": "No credit on this line.",
        },
    ])
}

/// The same three cues with the speakers stripped —
/// what an SRT hop leaves behind.
fn expected_cues_uncredited() -> Value {
    json!([
        {
            "index": 0,
            "start_frame": 12,
            "end_frame": 36,
            "text": "The light is still on.",
        },
        {
            "index": 1,
            "start_frame": 36,
            "end_frame": 84,
            "text": "Then we walk home before dark.",
        },
        {
            "index": 2,
            "start_frame": 84,
            "end_frame": 108,
            "text": "No credit on this line.",
        },
    ])
}

/// Add the three fixture cues to `core` through the
/// registry, deliberately out of order: the kept list
/// sorts by (start_frame, end_frame) and each reply's
/// `index` is the new cue's position in it — the
/// middle cue first (index 0), the last cue (index 1),
/// then the earliest cue, which must sort to the
/// front (index 0 again).
fn add_fixture_cues(core: &mut EditorCore) {
    let reply = invoke(
        core,
        "subtitle_add",
        &json!({
            "start_frame": 36,
            "end_frame": 84,
            "text": "Then we walk home before dark.",
            "speaker": "REN",
        }),
    )
    .expect("add the REN cue");
    assert_eq!(reply["index"], json!(0), "the first add stands alone");

    let reply = invoke(
        core,
        "subtitle_add",
        &json!({
            "start_frame": 84,
            "end_frame": 108,
            "text": "No credit on this line.",
        }),
    )
    .expect("add the uncredited cue");
    assert_eq!(reply["index"], json!(1), "the later cue appends");

    let reply = invoke(
        core,
        "subtitle_add",
        &json!({
            "start_frame": 12,
            "end_frame": 36,
            "text": "The light is still on.",
            "speaker": "NOVA",
        }),
    )
    .expect("add the NOVA cue");
    assert_eq!(
        reply["index"],
        json!(0),
        "the earliest cue sorts to the front of the kept list"
    );
}

fn listed(core: &mut EditorCore) -> Value {
    invoke(core, "subtitle_list", &json!({})).expect("subtitle_list")
}

// ---------------------------------------------------------------------
// Leg 1 — the WebVTT round trip through the registry
// ---------------------------------------------------------------------

#[test]
fn vtt_round_trip_through_the_registry_preserves_cues_speakers_and_style() {
    let dir = temp_dir("vtt");
    let vtt_path = dir.join("round-trip.vtt");
    let vtt_path_str = vtt_path.to_str().unwrap().to_string();

    // -- The source core, built entirely through the
    // registry.
    let mut source = fresh_core();
    add_fixture_cues(&mut source);

    let reply = invoke(&mut source, "subtitle_set_style", &style_args())
        .expect("set the non-default style");
    assert_eq!(reply["changed"], json!(true));
    assert_eq!(
        reply["style"],
        expected_style(),
        "the set_style echo is the style as stored"
    );

    let list = listed(&mut source);
    assert_eq!(list["count"], json!(3));
    assert_eq!(list["subtitles"], expected_cues());
    assert_eq!(list["style"], expected_style());

    // -- Export through the registry; the document on
    // disk is the transport, so it is asserted too.
    let reply = invoke(
        &mut source,
        "subtitle_export_vtt",
        &json!({ "path": vtt_path_str }),
    )
    .expect("export the WebVTT document");
    assert_eq!(reply["count"], json!(3));
    let document = std::fs::read_to_string(&vtt_path).expect("read the exported document");
    assert!(
        document.starts_with("WEBVTT\n"),
        "the document opens with the signature: {document}"
    );
    assert!(
        document.contains("00:00:00.500 --> 00:00:01.500"),
        "frame 12..36 at 24 fps is 0.5s..1.5s: {document}"
    );
    assert!(
        document.contains("<v NOVA>The light is still on.</v>"),
        "the first credit rides as a voice tag: {document}"
    );
    assert!(
        document.contains("<v REN>Then we walk home before dark.</v>"),
        "the second credit rides as a voice tag: {document}"
    );
    assert_eq!(
        document.matches("<v ").count(),
        2,
        "only the credited cues carry voice tags: {document}"
    );

    // -- A fresh core: no cues, the default style.
    let mut target = fresh_core();
    let list = listed(&mut target);
    assert_eq!(list["count"], json!(0));
    assert_eq!(list["style"], default_style());

    // The style is set through the registry BEFORE the
    // import (see the module doc for the terms of the
    // style claim).
    let reply = invoke(&mut target, "subtitle_set_style", &style_args())
        .expect("set the target's style");
    assert_eq!(reply["changed"], json!(true));

    let reply = invoke(
        &mut target,
        "subtitle_import_vtt",
        &json!({ "path": vtt_path_str }),
    )
    .expect("import the WebVTT document into the fresh core");
    assert_eq!(reply["changed"], json!(true));
    assert_eq!(reply["count"], json!(3));

    let list = listed(&mut target);
    assert_eq!(
        list["subtitles"],
        expected_cues(),
        "frames, text and speakers survive the registry round trip exactly"
    );
    assert_eq!(
        list["style"],
        expected_style(),
        "the import replaced the cue list only — the style in force is untouched"
    );
    assert_eq!(
        target.project.subtitles, source.project.subtitles,
        "the two cores hold the same cue list, speakers included"
    );

    // Re-export from the target: the same cues at the
    // same rate serialise to the same document, byte
    // for byte.
    let vtt_path2 = dir.join("round-trip-2.vtt");
    invoke(
        &mut target,
        "subtitle_export_vtt",
        &json!({ "path": vtt_path2.to_str().unwrap() }),
    )
    .expect("re-export from the target core");
    assert_eq!(
        std::fs::read_to_string(&vtt_path2).expect("read the re-exported document"),
        document,
        "the round trip is a fixed point at the document level too"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — undo discipline at the registry surface
// ---------------------------------------------------------------------

#[test]
fn registry_undo_discipline_for_subtitle_mutations() {
    let dir = temp_dir("undo");
    let vtt_path = dir.join("cues.vtt");
    let vtt_path_str = vtt_path.to_str().unwrap().to_string();

    // -- Source core: every mutation spends exactly one
    // step; no-ops, read-only calls and refusals spend
    // none.
    let mut source = fresh_core();
    assert_eq!(source.undo_depth(), 0, "a fresh core has an empty undo stack");

    // Three adds, one step each (the adds' own reply
    // indices are leg 1's concern; here only the depth
    // moves).
    add_fixture_cues(&mut source);
    assert_eq!(source.undo_depth(), 3, "one undo step per subtitle_add");

    let reply = invoke(&mut source, "subtitle_set_style", &style_args())
        .expect("set the style");
    assert_eq!(reply["changed"], json!(true));
    assert_eq!(source.undo_depth(), 4, "the style replace is one step");

    // The same style again: the clamped result equals
    // the style in force, so the handler reports the
    // no-op and spends nothing (Wave R, R20's parity
    // rule, exercised at registry level).
    let reply = invoke(&mut source, "subtitle_set_style", &style_args())
        .expect("set the identical style");
    assert_eq!(reply["changed"], json!(false));
    assert_eq!(source.undo_depth(), 4, "an identical set_style spends no step");

    // Export is read-only: no step, and the core's
    // status line is not written (the sentence travels
    // in the reply only).
    let status_before = source.status.clone();
    invoke(
        &mut source,
        "subtitle_export_vtt",
        &json!({ "path": vtt_path_str }),
    )
    .expect("export for the import legs below");
    assert_eq!(source.undo_depth(), 4, "export spends no step");
    assert_eq!(
        source.status, status_before,
        "export leaves the core's status line alone"
    );

    // A refused add (empty range) changes nothing and
    // spends nothing.
    let err = invoke(
        &mut source,
        "subtitle_add",
        &json!({
            "start_frame": 50,
            "end_frame": 50,
            "text": "Never stored.",
        }),
    )
    .expect_err("an empty half-open range refuses");
    assert!(!err.is_empty(), "the refusal explains itself");
    assert_eq!(source.undo_depth(), 4, "a refused add spends no step");
    assert_eq!(source.project.subtitles.len(), 3, "a refused add stores nothing");

    // -- Target core: the import's fork, then the undo
    // walk back through it.
    let mut target = fresh_core();

    // A refused import (unreadable file) is refused
    // before any change and spends nothing.
    let missing = dir.join("no-such-file.vtt");
    assert!(
        invoke(
            &mut target,
            "subtitle_import_vtt",
            &json!({ "path": missing.to_str().unwrap() }),
        )
        .is_err(),
        "an unreadable import file refuses"
    );
    assert_eq!(target.undo_depth(), 0, "a refused import spends no step");
    assert!(target.project.subtitles.is_empty());

    invoke(&mut target, "subtitle_set_style", &style_args()).expect("set the target style");
    assert_eq!(target.undo_depth(), 1);

    let reply = invoke(
        &mut target,
        "subtitle_import_vtt",
        &json!({ "path": vtt_path_str.clone() }),
    )
    .expect("import the cues");
    assert_eq!(reply["changed"], json!(true));
    assert_eq!(target.undo_depth(), 2, "a changed import is exactly one step");

    // The identical import again: the parsed list
    // matches the current one, so nothing changes and
    // no step is spent.
    let reply = invoke(
        &mut target,
        "subtitle_import_vtt",
        &json!({ "path": vtt_path_str }),
    )
    .expect("import the same document again");
    assert_eq!(reply["changed"], json!(false));
    assert_eq!(target.undo_depth(), 2, "an identical import spends no step");

    // Undo the import: the cue list goes back to empty;
    // the style set BEFORE the import is untouched
    // (snapshots store the whole project, in order).
    target.undo();
    assert_eq!(target.undo_depth(), 1);
    let list = listed(&mut target);
    assert_eq!(list["count"], json!(0), "undoing the import empties the cue list");
    assert_eq!(
        list["style"],
        expected_style(),
        "undoing the import keeps the earlier style change"
    );

    // Undo the style set: the default style returns and
    // the stack is empty.
    target.undo();
    assert_eq!(target.undo_depth(), 0);
    let list = listed(&mut target);
    assert_eq!(
        list["style"],
        default_style(),
        "undoing the style set restores the default style"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — SRT at registry level: speakers_dropped
// ---------------------------------------------------------------------

#[test]
fn srt_leg_reports_speakers_dropped_and_imports_uncredited_cues() {
    let dir = temp_dir("srt");
    let srt_path = dir.join("cues.srt");
    let srt_path_str = srt_path.to_str().unwrap().to_string();

    let mut source = fresh_core();
    add_fixture_cues(&mut source);

    // The export reply counts what the format cannot
    // write: two of the three cues carry a credit
    // (Wave S, S9's surfacing).
    let reply = invoke(
        &mut source,
        "subtitle_export_srt",
        &json!({ "path": srt_path_str.clone() }),
    )
    .expect("export the SRT document");
    assert_eq!(reply["count"], json!(3));
    assert_eq!(
        reply["speakers_dropped"],
        json!(2),
        "two credited cues lose their credit to SRT"
    );

    let document = std::fs::read_to_string(&srt_path).expect("read the exported SRT");
    assert!(
        document.contains("00:00:00,500 --> 00:00:01,500"),
        "SRT timestamps use the comma separator: {document}"
    );
    for text in [
        "The light is still on.",
        "Then we walk home before dark.",
        "No credit on this line.",
    ] {
        assert!(document.contains(text), "the text is verbatim: {document}");
    }
    assert!(
        !document.contains("NOVA") && !document.contains("REN") && !document.contains("<v"),
        "SRT represents the speakers nowhere — not in tags, not in text: {document}"
    );

    // Import the SRT into a fresh core: frames and text
    // survive; every cue comes back uncredited.
    let mut target = fresh_core();
    let reply = invoke(
        &mut target,
        "subtitle_import_srt",
        &json!({ "path": srt_path_str }),
    )
    .expect("import the SRT document");
    assert_eq!(reply["changed"], json!(true));
    assert_eq!(reply["count"], json!(3));
    assert_eq!(target.undo_depth(), 1, "the SRT import is one step");

    let list = listed(&mut target);
    assert_eq!(
        list["subtitles"],
        expected_cues_uncredited(),
        "the SRT hop drops exactly the speakers and nothing else"
    );
    assert!(
        target.project.subtitles.iter().all(|cue| cue.speaker.is_none()),
        "no imported cue carries a speaker"
    );

    // Exporting SRT from an all-uncredited project: the
    // field is ABSENT, not zero — it is present only
    // above zero.
    let srt_path2 = dir.join("cues-2.srt");
    let reply = invoke(
        &mut target,
        "subtitle_export_srt",
        &json!({ "path": srt_path2.to_str().unwrap() }),
    )
    .expect("re-export SRT from the uncredited core");
    assert_eq!(reply["count"], json!(3));
    assert!(
        reply.get("speakers_dropped").is_none(),
        "speakers_dropped appears only when a credit was actually dropped: {reply}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
