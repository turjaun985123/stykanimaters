//! Integration tests: the audio lifecycle, driven end to end
//! through the real MCP dispatch (`McpServer::handle`) —
//! Wave 11, squad D, session 5 ("audio lifecycle, MCP only").
//!
//! One full session, messages only: new project -> add a
//! fixture WAV -> read it back -> move / trim / gain -> mute
//! and read back -> normalise -> split -> delete one half,
//! with the project state re-read through MCP after every
//! step. Negative sessions follow: a nonexistent path, an
//! unsupported extension, trims beyond the clip's bounds, a
//! non-finite gain, and out-of-range split/delete attempts.
//!
//! Environment note: every tool in this session decodes WAV
//! through the crate's own PCM16 codec
//! (`audio_edit::read_wav`) — none of them shells out to
//! ffmpeg / ffprobe / ffplay — so, unlike the MP4 encoders
//! `tests/export_determinism.rs` deliberately avoids, every
//! assertion here is unconditional. The fixture follows the
//! existing Rust test pattern (`audio_ops` / `editor` test
//! helpers): a `WavData` built in code and serialised with
//! `audio_edit::write_wav`. The tree's one environment-gating
//! pattern — the ffplay smoke test in `src/audio.rs`, which
//! prints "skipping audio playback smoke test: ffplay is not
//! available" and returns early — is therefore NOT needed
//! here; it is named only so a future step that does shell
//! out knows which pattern to copy.
//!
//! Gaps this suite pinned (see the markers at the
//! assertions themselves) — all closed by the Wave 11
//! splice; the assertions were written against the correct
//! behaviour and stand:
//!
//! - The Wave 11 (B6) audio parity handlers
//!   (`tool_audio_list_clips`, `tool_audio_clip_info`,
//!   `tool_audio_clip_move`, `tool_audio_clip_trim`,
//!   `tool_audio_clip_set_gain`, `tool_audio_clip_normalise`)
//!   are registered in `mcp_tools::tools()` under the
//!   conventional names this suite calls
//!   (`audio_list_clips`, ...).
//! - `add_audio_clip` validates the file: a nonexistent
//!   path or a non-WAV extension is refused and no phantom
//!   clip lands on the timeline (the panel adds-and-badges
//!   instead — the divergence is in the tool's description).
//! - `audio_edit::split_clip` carries the source clip's
//!   `muted` flag to both halves.

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::audio_edit::{WavData, write_wav};
use stykanimate_rs::mcp::McpServer;

// ---------------------------------------------------------------------
// Dispatch helpers (the `mcp.rs` unit-test harness shape)
// ---------------------------------------------------------------------

/// Send one `tools/call` request through the real router and
/// return `(payload, is_error)`: the payload is the parsed
/// JSON document the tool returned, or a `Value::String`
/// holding the error text when the call failed (tool errors
/// travel as plain text, per the MCP spec).
fn call(server: &mut McpServer, id: &mut u64, name: &str, args: Value) -> (Value, bool) {
    *id += 1;
    let request = json!({
        "jsonrpc": "2.0",
        "id": *id,
        "method": "tools/call",
        "params": { "name": name, "arguments": args },
    });
    let resp = server
        .handle(&request)
        .expect("tools/call is a request and must be answered");
    assert_eq!(resp["jsonrpc"], "2.0", "{name}: bad envelope");
    assert_eq!(resp["id"], *id, "{name}: response id mismatch");
    let result = &resp["result"];
    let text = result["content"][0]["text"]
        .as_str()
        .expect("content block carries text")
        .to_string();
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let payload = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (payload, is_error)
}

/// `call` for a step that must succeed; the failure text is
/// part of the panic message so CI logs name the step.
fn call_ok(server: &mut McpServer, id: &mut u64, name: &str, args: Value) -> Value {
    let (payload, is_error) = call(server, id, name, args);
    assert!(!is_error, "{name} must succeed, got error: {payload}");
    payload
}

/// `call` for a step that must be refused; returns the
/// error text for wording assertions.
fn call_err(server: &mut McpServer, id: &mut u64, name: &str, args: Value) -> String {
    let (payload, is_error) = call(server, id, name, args);
    assert!(is_error, "{name} must be refused, got success: {payload}");
    payload
        .as_str()
        .unwrap_or("<non-text error payload>")
        .to_string()
}

/// The audio clip count, read back through MCP
/// (`get_project_stats`) — the count assertion after steps
/// that add or remove clips.
fn stats_audio_clips(server: &mut McpServer, id: &mut u64) -> u64 {
    let stats = call_ok(server, id, "get_project_stats", json!({}));
    stats["audio_clips"]
        .as_u64()
        .expect("get_project_stats reports audio_clips")
}

/// Every clip as the MCP read surface reports it.
// Landed (B6, spliced): `audio_list_clips` is the B6 handler
// `tool_audio_list_clips` (src/audio_ops.rs); it is not yet
// registered in `all_tools()`, so until the integrator
// wires it this read fails with "unknown tool:
// audio_list_clips" and every read-back assertion below is
// blocked on that registration.
fn list_clips(server: &mut McpServer, id: &mut u64) -> Vec<Value> {
    let payload = call_ok(server, id, "audio_list_clips", json!({}));
    assert_eq!(payload["ok"], true, "audio_list_clips payload: {payload}");
    payload["clips"]
        .as_array()
        .expect("audio_list_clips returns a clips array")
        .clone()
}

fn assert_close(actual: &Value, expected: f64, tol: f64, what: &str) {
    let got = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{what}: expected a number, got {actual}"));
    assert!(
        (got - expected).abs() <= tol,
        "{what}: expected {expected} (+/- {tol}), got {got}"
    );
}

// ---------------------------------------------------------------------
// Fixture (the `audio_ops` tests' pattern: WavData + write_wav)
// ---------------------------------------------------------------------

/// A per-process, per-test scratch directory under the OS
/// temp dir (the `export_determinism` / `audio_ops` shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave11-e2e-audio-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// The session's fixture: 2.0 s of mono 8 kHz audio — a
/// quiet first half (constant +0.25) and a loud second half
/// (constant -0.5). Both levels are powers of two, so they
/// survive the PCM16 round trip exactly; the peak is 0.5
/// and the two halves leave distinguishable peak traces.
fn write_fixture_wav(dir: &PathBuf) -> String {
    let mut samples = vec![0.25f32; 8000];
    samples.extend(vec![-0.5f32; 8000]);
    let wav = WavData {
        sample_rate: 8000,
        channels: 1,
        samples,
    };
    let path = dir.join("clip.wav");
    std::fs::write(&path, write_wav(&wav)).expect("write fixture wav");
    path.to_string_lossy().into_owned()
}

/// A server holding a fresh MCP-created project (10 fps)
/// with the fixture clip placed at frame 0 — the shared
/// starting point of the negative sessions.
fn server_with_clip(tag: &str) -> (McpServer, u64, PathBuf, String) {
    let dir = temp_dir(tag);
    let fixture = write_fixture_wav(&dir);
    let mut server = McpServer::new();
    let mut id = 0u64;
    call_ok(
        &mut server,
        &mut id,
        "new_project",
        json!({ "name": "Wave 11 Audio Negatives", "fps": 10.0 }),
    );
    call_ok(
        &mut server,
        &mut id,
        "add_audio_clip",
        json!({ "path": fixture, "start_frame": 0 }),
    );
    (server, id, dir, fixture)
}

// ---------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------

#[test]
fn audio_lifecycle_end_to_end_via_mcp() {
    let dir = temp_dir("lifecycle");
    let fixture = write_fixture_wav(&dir);
    let mut server = McpServer::new();
    let mut id = 0u64;

    // -- Handshake and registry surface --------------------
    id += 1;
    let init = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {},
        }))
        .expect("initialize must be answered");
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");

    id += 1;
    let list = server
        .handle(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {},
        }))
        .expect("tools/list must be answered");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for want in [
        "new_project",
        "add_frame",
        "add_audio_clip",
        "audio_set_muted",
        "audio_split",
        "audio_delete_clip",
        "get_project_stats",
        "get_state",
        // Landed (B6, spliced): the next six are the B6 parity tools —
        // handlers written in src/audio_ops.rs, registration
        // in mcp_tools::tools() still owed by the integrator.
        "audio_list_clips",
        "audio_clip_info",
        "audio_clip_move",
        "audio_clip_trim",
        "audio_clip_set_gain",
        "audio_clip_normalise",
    ] {
        assert!(
            names.contains(&want),
            "tools/list must offer {want:?}; the session cannot run without it"
        );
    }
    // The trim tool's schema promises SECONDS in its
    // argument names — the units the trim step below
    // relies on.
    let trim_tool = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "audio_clip_trim")
        .expect("audio_clip_trim is listed");
    let props = &trim_tool["inputSchema"]["properties"];
    assert!(props.get("start_secs").is_some(), "trim schema: {props}");
    assert!(props.get("end_secs").is_some(), "trim schema: {props}");

    // -- New project ---------------------------------------
    let created = call_ok(
        &mut server,
        &mut id,
        "new_project",
        json!({ "name": "Wave 11 Audio Lifecycle", "fps": 10.0 }),
    );
    assert_eq!(created["project"]["name"], "Wave 11 Audio Lifecycle");
    assert_close(&created["project"]["fps"], 10.0, 0.0, "new_project fps");
    assert_eq!(stats_audio_clips(&mut server, &mut id), 0);

    // Grow the timeline to 25 frames (pure MCP) so the
    // clip's span sits inside the project.
    let mut frames = 0;
    for _ in 0..24 {
        let added = call_ok(&mut server, &mut id, "add_frame", json!({}));
        frames = added["frames"].as_u64().expect("add_frame reports frames");
    }
    assert_eq!(frames, 25, "24 add_frame calls on a 1-frame project");
    let state = call_ok(&mut server, &mut id, "get_state", json!({}));
    assert_eq!(state["frame_count"], 25);

    // -- Add the fixture clip ------------------------------
    let added = call_ok(
        &mut server,
        &mut id,
        "add_audio_clip",
        json!({ "path": fixture, "start_frame": 2 }),
    );
    assert_eq!(added["audio_index"], 0);
    assert_eq!(added["start_frame"], 2);
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);

    // -- Read it back: units first --------------------------
    // duration_secs is SECONDS and duration_frames is the
    // same span in project frames: at 10 fps a 2.0 s clip
    // must report 20 frames, and frames == secs * fps is
    // the cross-check that neither number silently changed
    // units.
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 1);
    let clip = &clips[0];
    assert_eq!(clip["path"], fixture.as_str());
    assert_eq!(clip["start_frame"], 2);
    assert_close(&clip["gain"], 1.0, 0.0, "initial gain");
    assert_eq!(clip["muted"], false);
    assert_eq!(clip["health"], "healthy");
    assert_eq!(clip["sample_rate"], 8000);
    assert_eq!(clip["channels"], 1);
    assert_close(&clip["duration_secs"], 2.0, 0.001, "duration_secs");
    assert_eq!(clip["duration_frames"], 20, "duration_frames: {clip}");
    assert_eq!(clip["end_frame"], 22, "end_frame: {clip}");
    assert_close(&clip["peak"], 0.5, 0.001, "fixture peak");
    assert_eq!(clip["starts_past_project_end"], false);
    assert_eq!(clip["ends_past_project_end"], false);

    // Waveform peaks via the info tool: 8 buckets over the
    // two halves — four quiet (0.25), four loud (0.5).
    let info = call_ok(
        &mut server,
        &mut id,
        "audio_clip_info",
        json!({ "index": 0, "buckets": 8 }),
    );
    let peaks = info["peaks"].as_array().expect("info returns peaks");
    assert_eq!(peaks.len(), 8);
    for (i, p) in peaks.iter().enumerate() {
        assert_close(p, if i < 4 { 0.25 } else { 0.5 }, 0.001, "info peak");
    }

    // -- Move ----------------------------------------------
    let moved = call_ok(
        &mut server,
        &mut id,
        "audio_clip_move",
        json!({ "index": 0, "start_frame": 5 }),
    );
    assert_eq!(moved["start_frame"], 5);
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips[0]["start_frame"], 5);
    assert_eq!(clips[0]["end_frame"], 25);
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);

    // -- Trim to [0.5 s, 1.5 s) -----------------------------
    // The surviving second straddles the fixture's halves;
    // a positive start advances the start frame by
    // round(0.5 s * 10 fps) = 5, preserving the surviving
    // audio's timeline position.
    let trimmed = call_ok(
        &mut server,
        &mut id,
        "audio_clip_trim",
        json!({ "index": 0, "start_secs": 0.5, "end_secs": 1.5 }),
    );
    assert_eq!(trimmed["start_frame"], 10);
    assert!(
        trimmed["status"].as_str().unwrap_or("").contains("trimmed to 1.00s"),
        "trim status: {trimmed}"
    );
    let clips = list_clips(&mut server, &mut id);
    let clip = &clips[0];
    assert!(
        clip["path"].as_str().unwrap_or("").ends_with("clip.trim.wav"),
        "trim repoints the clip at its product: {clip}"
    );
    assert_eq!(clip["start_frame"], 10);
    assert_close(&clip["duration_secs"], 1.0, 0.001, "trimmed duration_secs");
    assert_eq!(clip["duration_frames"], 10);
    assert_eq!(clip["end_frame"], 20);
    assert_close(&clip["peak"], 0.5, 0.001, "trimmed peak");
    let info = call_ok(
        &mut server,
        &mut id,
        "audio_clip_info",
        json!({ "index": 0, "buckets": 4 }),
    );
    let peaks = info["peaks"].as_array().expect("info returns peaks");
    assert_eq!(peaks.len(), 4);
    for (i, p) in peaks.iter().enumerate() {
        assert_close(p, if i < 2 { 0.25 } else { 0.5 }, 0.001, "trimmed peak");
    }

    // -- Gain: set, then clamp ------------------------------
    let gained = call_ok(
        &mut server,
        &mut id,
        "audio_clip_set_gain",
        json!({ "index": 0, "gain": 0.5 }),
    );
    assert_close(&gained["gain"], 0.5, 0.0, "set gain readback");
    let clips = list_clips(&mut server, &mut id);
    assert_close(&clips[0]["gain"], 0.5, 0.0, "gain after set");
    // The op clamps to 0.0..=2.0 and the B6 handler reports
    // the clamped value read back from the clip — a 5.0
    // request must report 2.0, not echo the request.
    let clamped = call_ok(
        &mut server,
        &mut id,
        "audio_clip_set_gain",
        json!({ "index": 0, "gain": 5.0 }),
    );
    assert_close(&clamped["gain"], 2.0, 0.0, "clamped gain readback");
    let clips = list_clips(&mut server, &mut id);
    assert_close(&clips[0]["gain"], 2.0, 0.0, "gain after clamp");

    // -- Mute, read back ------------------------------------
    let muted = call_ok(
        &mut server,
        &mut id,
        "audio_set_muted",
        json!({ "index": 0, "muted": true }),
    );
    assert_eq!(muted["muted"], true);
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips[0]["muted"], true, "mute read back via MCP: {clips:?}");
    assert_close(&clips[0]["gain"], 2.0, 0.0, "mute leaves the gain alone");

    // -- Normalise (per-clip, file level) -------------------
    // The panel's Norm op: the FILE is rewritten to peak
    // 0.90 and the clip repointed; the clip's gain field is
    // not part of it and must stay 2.0.
    let normed = call_ok(
        &mut server,
        &mut id,
        "audio_clip_normalise",
        json!({ "index": 0 }),
    );
    assert!(
        normed["status"]
            .as_str()
            .unwrap_or("")
            .contains("normalised to peak 0.90"),
        "normalise status: {normed}"
    );
    let clips = list_clips(&mut server, &mut id);
    let clip = &clips[0];
    assert!(
        clip["path"]
            .as_str()
            .unwrap_or("")
            .ends_with("clip.trim.norm.wav"),
        "normalise repoints the clip: {clip}"
    );
    assert_close(&clip["peak"], 0.9, 0.002, "normalised file peak");
    assert_close(&clip["duration_secs"], 1.0, 0.001, "normalised duration");
    assert_close(&clip["gain"], 2.0, 0.0, "file normalise keeps clip gain");
    assert_eq!(clip["muted"], true, "normalise keeps the mute flag");

    // -- Split at frame 15 ----------------------------------
    // The clip now spans frames [10, 20): 15 is strictly
    // inside. Both halves are materialised as real files.
    let split = call_ok(
        &mut server,
        &mut id,
        "audio_split",
        json!({ "clip": 0, "at_frame": 15 }),
    );
    assert_eq!(split["clips"], 2);
    assert_eq!(stats_audio_clips(&mut server, &mut id), 2);
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 2);
    let (first, second) = (&clips[0], &clips[1]);
    assert!(
        first["path"]
            .as_str()
            .unwrap_or("")
            .ends_with("clip.trim.norm.a.wav"),
        "first half path: {first}"
    );
    assert_eq!(first["start_frame"], 10);
    assert_close(&first["duration_secs"], 0.5, 0.001, "first half duration");
    assert_eq!(first["end_frame"], 15);
    assert_close(&first["gain"], 2.0, 0.0, "first half keeps the gain");
    assert!(
        second["path"]
            .as_str()
            .unwrap_or("")
            .ends_with("clip.trim.norm.b.wav"),
        "second half path: {second}"
    );
    assert_eq!(second["start_frame"], 15);
    assert_close(&second["duration_secs"], 0.5, 0.001, "second half duration");
    assert_eq!(second["end_frame"], 20);
    assert_close(&second["gain"], 2.0, 0.0, "second half keeps the gain");
    // Fixed (B6 B-B): splitting a MUTED clip must keep both
    // halves muted — the clip was muted above and nothing
    // since un-muted it. `audio_edit::split_clip` hardcodes
    // `muted: false` for both halves, so today both read
    // back un-muted: the split silently resurrects sound
    // the user had muted out of playback and exports.
    assert_eq!(first["muted"], true, "split must preserve the mute (first half)");
    assert_eq!(second["muted"], true, "split must preserve the mute (second half)");

    // -- Delete the second half ------------------------------
    let deleted = call_ok(
        &mut server,
        &mut id,
        "audio_delete_clip",
        json!({ "index": 1 }),
    );
    assert_eq!(deleted["count"], 1);
    assert!(
        deleted["path"]
            .as_str()
            .unwrap_or("")
            .ends_with("clip.trim.norm.b.wav"),
        "the deleted clip is the second half: {deleted}"
    );
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0]["start_frame"], 10);
    assert_close(&clips[0]["duration_secs"], 0.5, 0.001, "survivor duration");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Negative sessions
// ---------------------------------------------------------------------

#[test]
fn add_audio_clip_with_a_nonexistent_path_is_refused() {
    let dir = temp_dir("missing");
    let missing = dir.join("no-such-file.wav").to_string_lossy().into_owned();
    let mut server = McpServer::new();
    let mut id = 0u64;
    call_ok(
        &mut server,
        &mut id,
        "new_project",
        json!({ "name": "Wave 11 Missing", "fps": 10.0 }),
    );
    // Fixed: `add_audio_clip` (mcp_tools.rs) checks
    // only that the path is non-empty — it never stats or
    // probes the file, so today this call SUCCEEDS and a
    // phantom clip lands on the timeline (its panel health
    // would read "missing"). The honest behaviour is a
    // refusal: an error, no clip added, no panic.
    let (_payload, is_error) = call(
        &mut server,
        &mut id,
        "add_audio_clip",
        json!({ "path": missing, "start_frame": 0 }),
    );
    assert!(
        is_error,
        "add_audio_clip must refuse a path that does not exist"
    );
    assert_eq!(
        stats_audio_clips(&mut server, &mut id),
        0,
        "a refused add must leave the project clip-free"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_audio_clip_with_an_unsupported_extension_is_refused() {
    let dir = temp_dir("unsupported");
    // A real WAV's bytes under an .mp3 name: every decoder
    // in the app is WAV-only and the panel's probe
    // classifies by extension first ("unsupported format"),
    // so the extension alone decides.
    let fixture = write_fixture_wav(&dir);
    let mp3 = dir.join("song.mp3");
    std::fs::copy(&fixture, &mp3).expect("copy fixture under an mp3 name");
    let mp3 = mp3.to_string_lossy().into_owned();
    let mut server = McpServer::new();
    let mut id = 0u64;
    call_ok(
        &mut server,
        &mut id,
        "new_project",
        json!({ "name": "Wave 11 Unsupported", "fps": 10.0 }),
    );
    // Fixed: same hole as the nonexistent path —
    // `add_audio_clip` accepts any non-empty string, so
    // today this SUCCEEDS even though no surface in the
    // app can decode the clip it just placed.
    let (_payload, is_error) = call(
        &mut server,
        &mut id,
        "add_audio_clip",
        json!({ "path": mp3, "start_frame": 0 }),
    );
    assert!(
        is_error,
        "add_audio_clip must refuse a non-WAV extension"
    );
    assert_eq!(
        stats_audio_clips(&mut server, &mut id),
        0,
        "a refused add must leave the project clip-free"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trim_beyond_the_clips_bounds_clamps_or_refuses() {
    let (mut server, mut id, dir, _fixture) = server_with_clip("trim-bounds");

    // An end past the file's 2.0 s is CLAMPED to the data
    // (audio_edit::trim's documented rule): the call
    // succeeds and the clip keeps its full duration.
    let clamped = call_ok(
        &mut server,
        &mut id,
        "audio_clip_trim",
        json!({ "index": 0, "start_secs": 0.0, "end_secs": 99.0 }),
    );
    assert_eq!(clamped["start_frame"], 0);
    let clips = list_clips(&mut server, &mut id);
    assert_close(
        &clips[0]["duration_secs"],
        2.0,
        0.001,
        "clamped trim keeps the full duration",
    );
    let path_after_clamp = clips[0]["path"].as_str().unwrap_or("").to_string();
    assert!(
        path_after_clamp.ends_with("clip.trim.wav"),
        "clamped trim still materialises its product: {path_after_clamp}"
    );

    // A range ENTIRELY beyond the file leaves an empty
    // trim, which the engine refuses: an error, and the
    // clip is exactly as the clamped trim left it.
    let err = call_err(
        &mut server,
        &mut id,
        "audio_clip_trim",
        json!({ "index": 0, "start_secs": 5.0, "end_secs": 6.0 }),
    );
    assert!(
        err.contains("trim range is empty"),
        "out-of-bounds trim error names the empty range: {err}"
    );
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0]["path"], path_after_clamp.as_str());
    assert_close(
        &clips[0]["duration_secs"],
        2.0,
        0.001,
        "refused trim leaves the clip untouched",
    );
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_finite_and_negative_gains_are_refused() {
    let (mut server, mut id, dir, _fixture) = server_with_clip("gain-bounds");

    // 1e999 on the wire: JSON numbers that overflow f64
    // reach the Value layer as null (serde_json cannot
    // hold a non-finite number — `Value::from(f64)` maps
    // it to Null), which the handler must refuse as "not
    // a number" — never store, never panic.
    let mut gain_args = serde_json::Map::new();
    gain_args.insert("index".to_string(), json!(0));
    gain_args.insert("gain".to_string(), Value::from(f64::INFINITY));
    let err = call_err(
        &mut server,
        &mut id,
        "audio_clip_set_gain",
        Value::Object(gain_args),
    );
    assert!(
        err.contains("must be a number") || err.contains("finite"),
        "overflow gain error: {err}"
    );
    // A negative gain is refused by the handler's own
    // finite/non-negative check.
    let err = call_err(
        &mut server,
        &mut id,
        "audio_clip_set_gain",
        json!({ "index": 0, "gain": -1.0 }),
    );
    assert!(
        err.contains("finite, non-negative"),
        "negative gain error: {err}"
    );
    // Both refusals leave the clip at its initial gain.
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 1);
    assert_close(&clips[0]["gain"], 1.0, 0.0, "gain untouched by refusals");
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn split_and_delete_out_of_range_are_refused() {
    let (mut server, mut id, dir, _fixture) = server_with_clip("range-bounds");

    // The clip spans frames [0, 20) at 10 fps; a split at
    // frame 999 is not inside it — an error, no new clip,
    // and the original is untouched.
    let err = call_err(
        &mut server,
        &mut id,
        "audio_split",
        json!({ "clip": 0, "at_frame": 999 }),
    );
    assert!(
        err.contains("not inside clip"),
        "split error names the bad point: {err}"
    );
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);

    // Deleting an index the project does not have.
    let err = call_err(
        &mut server,
        &mut id,
        "audio_delete_clip",
        json!({ "index": 3 }),
    );
    assert!(
        err.contains("no audio clip at index 3"),
        "delete error names the index: {err}"
    );
    assert_eq!(stats_audio_clips(&mut server, &mut id), 1);
    let clips = list_clips(&mut server, &mut id);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0]["start_frame"], 0);
    assert_close(&clips[0]["duration_secs"], 2.0, 0.001, "clip untouched");

    let _ = std::fs::remove_dir_all(&dir);
}
