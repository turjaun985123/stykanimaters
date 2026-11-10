//! Wave T (T13) — the undo-census extension for Wave S's
//! MCP-surface behaviour changes.
//!
//! Wave R's R7 census (tests/wave_r_undo_census.rs) pinned
//! all 381 tools' undo-step behaviour as of Wave R. Wave S
//! then changed behaviour on exactly TWO existing tools
//! (audit of ~/workspace/wave-s/findings/, s1–s20 + f2 —
//! the per-slice evidence is in
//! ~/workspace/wave-t/findings/t13.md):
//!
//!   * `subtitle_export_srt` (S9) — the handler now runs
//!     the counted export (`subtitles_to_srt_counted`) and
//!     the reply gains `speakers_dropped: N`, present ONLY
//!     when N > 0. The tool was read-only (0 steps) and
//!     stays read-only; the reply-shape discipline is part
//!     of the changed contract, so shape and step count are
//!     pinned together, both ways.
//!   * `render` (S5) — the `scale` argument, which in
//!     Wave R routed only for png/png_sequence (R10),
//!     now also routes for gif, apng, spritesheet, mp4 and
//!     webm. `render` writes files and mutates no project
//!     state: 0 steps for the new scaled routes, and 0
//!     steps (plus nothing written) for their refusals.
//!
//! Nothing else at the MCP surface changed behaviour in
//! Wave S: S4's scaled entries are core functions the
//! render routing calls (no new tools; the surface stayed
//! 381 — S15's verified count), S19 changed docs only, and
//! the remaining slices are shell-side, core-only, or
//! tests/scripts/docs. Tools whose behaviour Wave S did
//! not change keep their R7 pins and are NOT re-pinned
//! here.
//!
//! Discipline mirrors the R7 census: invoke through the
//! real registry (`mcp::all_tools()`), setups run through
//! other tools' handlers, the undo depth is read AFTER
//! setup so only the measured call is counted, and timing
//! fixtures set fps 24.0 explicitly.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::all_tools;
use stykanimate_rs::subtitles::{subtitles_to_srt, subtitles_to_vtt};

// ---------------------------------------------------------------------
// Harness (the wave_r_undo_census idiom)
// ---------------------------------------------------------------------

/// A fresh demo core (1 humanoid figure, 5 frames) with the
/// project's fps set to 24.0 explicitly, per the board's
/// timing-fixture rule (subtitle exports convert timecodes
/// at this rate).
fn fresh_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core
}

/// Invoke one tool's handler by name through the real
/// registry (`mcp::all_tools()`), the surface this census
/// audits.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

/// Invoke during a setup: the call must succeed, or the leg
/// that needed the setup is meaningless.
fn must(core: &mut EditorCore, name: &str, args: Value) {
    if let Err(e) = invoke(core, name, &args) {
        panic!("setup call {name} failed: {e}");
    }
}

/// A per-test temp directory (created empty); each leg
/// writes its export targets inside its own.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wave_t_undo_census_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create per-test temp dir");
    dir
}

fn path_str(dir: &std::path::Path, file: &str) -> String {
    dir.join(file).to_string_lossy().into_owned()
}

/// The reply's key set, for exact-shape assertions: the
/// additive-field discipline (S9's `speakers_dropped`,
/// S5's no-scale-echo) is a statement about which keys
/// exist, not just what they hold.
fn reply_keys(reply: &Value) -> BTreeSet<String> {
    reply
        .as_object()
        .expect("tool replies are objects")
        .keys()
        .cloned()
        .collect()
}

fn key_set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------
// Subtitle setups (through the registry, like the R7 census)
// ---------------------------------------------------------------------

/// Two cues, no speakers.
fn uncredited_core() -> EditorCore {
    let mut core = fresh_core();
    must(
        &mut core,
        "subtitle_add",
        json!({ "start_frame": 0, "end_frame": 24, "text": "First line" }),
    );
    must(
        &mut core,
        "subtitle_add",
        json!({ "start_frame": 24, "end_frame": 48, "text": "Second line" }),
    );
    core
}

/// Three cues, two of them credited — the SRT export must
/// report exactly 2 dropped speakers.
fn credited_core() -> EditorCore {
    let mut core = fresh_core();
    must(
        &mut core,
        "subtitle_add",
        json!({
            "start_frame": 0, "end_frame": 24,
            "text": "First line", "speaker": "Mary Jane"
        }),
    );
    must(
        &mut core,
        "subtitle_add",
        json!({
            "start_frame": 24, "end_frame": 48,
            "text": "Second line", "speaker": "Bo"
        }),
    );
    must(
        &mut core,
        "subtitle_add",
        json!({ "start_frame": 48, "end_frame": 72, "text": "Third line" }),
    );
    core
}

// ---------------------------------------------------------------------
// subtitle_export_srt / subtitle_export_vtt (Wave S, S9)
// ---------------------------------------------------------------------

#[test]
fn srt_export_uncredited_spends_nothing_and_keeps_the_established_reply() {
    let dir = test_dir("srt_uncredited");
    let path = path_str(&dir, "cues.srt");
    let mut core = uncredited_core();
    let before = core.undo_depth();
    let reply = invoke(&mut core, "subtitle_export_srt", &json!({ "path": path }))
        .expect("an uncredited SRT export succeeds");
    // Read-only before Wave S, read-only after: the counted
    // export changed the reply's provenance, not its cost.
    assert_eq!(
        core.undo_depth(),
        before,
        "subtitle_export_srt must spend no undo step"
    );
    // No credited cues -> the additive field stays ABSENT
    // and the reply is exactly the established shape.
    assert!(
        reply.get("speakers_dropped").is_none(),
        "no speakers_dropped key at zero drops: {reply}"
    );
    assert_eq!(
        reply_keys(&reply),
        key_set(&["count", "ok", "path", "status"]),
        "the uncredited reply keeps its pre-Wave-S shape"
    );
    assert_eq!(reply["count"], json!(2));
    // The written file is the plain export, byte for byte.
    let written = std::fs::read_to_string(&path).expect("the SRT file was written");
    assert_eq!(written, subtitles_to_srt(&core.project));
}

#[test]
fn srt_export_credited_spends_nothing_and_counts_the_dropped_speakers() {
    let dir = test_dir("srt_credited");
    let path = path_str(&dir, "cues.srt");
    let mut core = credited_core();
    let before = core.undo_depth();
    let reply = invoke(&mut core, "subtitle_export_srt", &json!({ "path": path }))
        .expect("a credited SRT export succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "subtitle_export_srt must spend no undo step"
    );
    // Two of the three cues carry speakers: the reply
    // surfaces exactly those two unwritten credits.
    assert_eq!(reply["speakers_dropped"], json!(2));
    assert_eq!(
        reply_keys(&reply),
        key_set(&["count", "ok", "path", "speakers_dropped", "status"]),
        "the credited reply is the established shape plus the one field"
    );
    assert_eq!(reply["count"], json!(3));
    // The document itself is still the plain export — the
    // count rides the reply, never the file.
    let written = std::fs::read_to_string(&path).expect("the SRT file was written");
    assert_eq!(written, subtitles_to_srt(&core.project));
    assert!(
        !written.contains("Mary"),
        "no speaker name leaks into the SRT document: {written}"
    );
}

#[test]
fn vtt_export_credited_spends_nothing_and_never_carries_the_field() {
    let dir = test_dir("vtt_credited");
    let path = path_str(&dir, "cues.vtt");
    let mut core = credited_core();
    let before = core.undo_depth();
    let reply = invoke(&mut core, "subtitle_export_vtt", &json!({ "path": path }))
        .expect("a credited WebVTT export succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "subtitle_export_vtt must spend no undo step"
    );
    // WebVTT keeps speakers, so there is no loss to
    // surface: its reply shape is unchanged by Wave S.
    assert!(
        reply.get("speakers_dropped").is_none(),
        "the VTT reply has no speakers_dropped: {reply}"
    );
    assert_eq!(
        reply_keys(&reply),
        key_set(&["count", "ok", "path", "status"]),
        "the VTT reply keeps its established shape"
    );
    assert_eq!(reply["count"], json!(3));
    let written = std::fs::read_to_string(&path).expect("the VTT file was written");
    assert_eq!(written, subtitles_to_vtt(&core.project));
    assert!(
        written.contains("<v Mary Jane>"),
        "the voice tag is written: {written}"
    );
}

// ---------------------------------------------------------------------
// render scale routing (Wave S, S5)
// ---------------------------------------------------------------------

/// A fresh core on a tiny stage, so the scaled renders are
/// cheap; fps 24.0 per the timing-fixture rule.
fn small_stage_core() -> EditorCore {
    let mut core = fresh_core();
    core.project.stage = [96.0, 72.0];
    core
}

#[test]
fn render_gif_at_scale_spends_nothing() {
    let dir = test_dir("render_gif_scaled");
    let path = path_str(&dir, "out.gif");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "render",
        &json!({ "path": path, "format": "gif", "scale": 2.0 }),
    )
    .expect("a scaled GIF render succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "render writes files, never project state: no undo step"
    );
    // The scaled reply has exactly the unscaled shape —
    // no scale echo (S5's routing discipline).
    assert_eq!(
        reply_keys(&reply),
        key_set(&["format", "frames", "path"]),
        "the scaled render reply adds no fields"
    );
    assert_eq!(reply["frames"], json!(5));
    let bytes = std::fs::read(&path).expect("the GIF file was written");
    assert!(!bytes.is_empty(), "the scaled GIF is non-empty");
}

#[test]
fn render_spritesheet_at_scale_spends_nothing() {
    let dir = test_dir("render_sheet_scaled");
    let path = path_str(&dir, "sheet.png");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "render",
        &json!({ "path": path, "format": "spritesheet", "scale": 0.5 }),
    )
    .expect("a scaled spritesheet render succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "render writes files, never project state: no undo step"
    );
    assert_eq!(
        reply_keys(&reply),
        key_set(&["format", "frames", "path"]),
        "the scaled render reply adds no fields"
    );
    assert_eq!(reply["frames"], json!(5));
    let bytes = std::fs::read(&path).expect("the sheet file was written");
    assert!(!bytes.is_empty(), "the scaled sheet is non-empty");
}

#[test]
fn render_range_and_scale_combined_spends_nothing() {
    let dir = test_dir("render_range_scaled");
    let path = path_str(&dir, "out.gif");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "render",
        &json!({
            "path": path, "format": "gif",
            "from_frame": 1, "to_frame": 3, "scale": 2.0
        }),
    )
    .expect("a ranged, scaled GIF render succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "render writes files, never project state: no undo step"
    );
    assert_eq!(reply["frames"], json!(3));
    // The range echoes under the caller's spelling (R13),
    // and scale still adds no field of its own.
    assert_eq!(
        reply_keys(&reply),
        key_set(&["format", "frames", "from_frame", "path", "to_frame"]),
        "range echo present, no scale echo"
    );
    let bytes = std::fs::read(&path).expect("the GIF file was written");
    assert!(!bytes.is_empty(), "the ranged scaled GIF is non-empty");
}

#[test]
fn render_scale_refusal_spends_nothing_and_writes_nothing() {
    let dir = test_dir("render_scale_refusal");
    let path = path_str(&dir, "out.gif");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let err = invoke(
        &mut core,
        "render",
        &json!({ "path": path, "format": "gif", "scale": 0.0 }),
    )
    .expect_err("scale 0 is refused by the scaled entries");
    assert!(
        err.contains("positive finite"),
        "the refusal is the entries' shared scale validation: {err}"
    );
    assert_eq!(
        core.undo_depth(),
        before,
        "a refused render spends no undo step"
    );
    assert!(
        !std::path::Path::new(&path).exists(),
        "a refused render writes nothing"
    );
}

#[test]
fn render_range_past_end_refusal_spends_nothing_and_writes_nothing() {
    let dir = test_dir("render_range_refusal");
    let path = path_str(&dir, "out.gif");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let err = invoke(
        &mut core,
        "render",
        &json!({
            "path": path, "format": "gif",
            "from_frame": 0, "to_frame": 99, "scale": 2.0
        }),
    )
    .expect_err("a range past the timeline end is refused, not clamped");
    // R13's strict timeline check, in the caller's
    // 'to_frame' vocabulary; the demo timeline has 5 frames.
    assert_eq!(
        err,
        "argument 'to_frame' 99 is out of range (project has 5 frames)"
    );
    assert_eq!(
        core.undo_depth(),
        before,
        "a refused render spends no undo step"
    );
    assert!(
        !std::path::Path::new(&path).exists(),
        "a refused render writes nothing"
    );
}
