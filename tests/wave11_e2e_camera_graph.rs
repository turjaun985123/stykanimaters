//! Integration tests (Wave 11, Squad D6): camera + graph
//! authoring driven end-to-end through the real MCP dispatch
//! (`McpServer::handle`) — no editor shortcut: every mutation
//! in these sessions is a `tools/call` message, and every
//! read-back goes through a tool as well.
//!
//! The session, in order: new project -> grow the timeline ->
//! camera keys with pan/zoom/rotation (including rotations
//! past 360 and below -180, the wrapping cases) -> read them
//! back -> delete a camera key -> graph channels (root
//! position + a joint angle) -> tangents and interpolation ->
//! a key shift via copy/paste -> `bake_graph` -> read the
//! baked poses back. A second session covers the negative
//! cases: out-of-range frames, channels, keys and targets
//! must be refused with the project left unchanged.
//!
//! Panel truths these tests hold MCP to (Wave 10 audits):
//! camera angle READOUTS are wrapped with `model::norm_deg`
//! (the panel's `angle_deg`), graph channels are addressed
//! by their TRUE index in `Project::channels`, and an
//! interpolation edit must land on the channel it addresses
//! — so the assertions below read every channel back
//! individually after each edit.
//!
//! Gaps found while writing this file were marked inline
//! with `// WAVE11-GAP:`; all four closed in the Wave 11
//! splice and the pins flipped the same wave: `get_camera`
//! reads keys back, `set_camera` stores angles WRAPPED to
//! -180..180 (the C2 ruling: one storage convention, panel
//! parity — the verbatim-storage pins below now assert the
//! wrapped values), `graph_set_tangents` sets one key's
//! tangents directly, and `graph_shift_keys` /
//! `graph_fill_keys` are the dedicated shift/fill tools
//! (unit-proven in graph_keys; the cross-channel composite
//! below remains as the end-to-end shift proof).

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::norm_deg;

// ---------------------------------------------------------------------
// Dispatch helpers (mirrors the in-crate `call_payload` in src/mcp.rs)
// ---------------------------------------------------------------------

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// One `tools/call` round trip. Returns the parsed payload the
/// tool returned (its result value serialised into the single
/// text content block) and the `isError` flag; a tool error's
/// message text comes back as a plain string payload.
fn call(server: &mut McpServer, name: &str, args: Value) -> (Value, bool) {
    let resp = server
        .handle(&request(1, "tools/call", json!({ "name": name, "arguments": args })))
        .expect("tools/call is a request and must be answered");
    let result = &resp["result"];
    let text = result["content"][0]["text"]
        .as_str()
        .expect("content block carries text");
    let is_error = result["isError"].as_bool().unwrap_or(false);
    (
        serde_json::from_str(text).unwrap_or(Value::String(text.to_string())),
        is_error,
    )
}

fn call_ok(server: &mut McpServer, name: &str, args: Value) -> Value {
    let (payload, is_error) = call(server, name, args);
    assert!(!is_error, "{name} unexpectedly failed: {payload}");
    payload
}

fn call_err(server: &mut McpServer, name: &str, args: Value) -> String {
    let (payload, is_error) = call(server, name, args);
    assert!(is_error, "{name} unexpectedly succeeded: {payload}");
    match payload {
        Value::String(s) => s,
        other => other.to_string(),
    }
}

fn num(v: &Value) -> f64 {
    v.as_f64().expect("expected a JSON number")
}

fn assert_close(actual: f64, expected: f64, tol: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tol,
        "{what}: expected {expected} ± {tol}, got {actual}"
    );
}

/// A per-process, per-test scratch directory (same pattern as
/// tests/export_determinism.rs).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave11-e2e-camera-graph-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// A fresh server holding a 6-frame, 1-figure project built
/// entirely through MCP calls.
fn six_frame_server() -> McpServer {
    let mut server = McpServer::new();
    let made = call_ok(
        &mut server,
        "new_project",
        json!({ "name": "Wave11 Camera Graph", "fps": 12, "width": 960, "height": 720 }),
    );
    assert_eq!(made["project"]["frames"], 1, "new_project starts at one frame");
    for want in 2..=6 {
        let added = call_ok(&mut server, "add_frame", json!({}));
        assert_eq!(added["frames"], want, "add_frame grows the timeline");
    }
    let state = call_ok(&mut server, "get_state", json!({}));
    assert_eq!(state["frame_count"], 6);
    assert_eq!(state["figures"].as_array().map(Vec::len), Some(1));
    server
}

/// The keys of one graph channel, read back through
/// `graph_copy_keys` (the registry's channel read-back).
fn copy_keys(server: &mut McpServer, figure: u64, target: &str) -> Vec<Value> {
    let out = call_ok(
        server,
        "graph_copy_keys",
        json!({ "figure": figure, "target": target }),
    );
    out["keys"]
        .as_array()
        .expect("graph_copy_keys returns a keys array")
        .clone()
}

// ---------------------------------------------------------------------
// Registry surface
// ---------------------------------------------------------------------

#[test]
fn registry_lists_the_camera_and_graph_session_tools() {
    let mut server = McpServer::new();
    let resp = server
        .handle(&request(1, "tools/list", json!({})))
        .expect("tools/list must be answered");
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool entry has a name"))
        .collect();
    for want in [
        "new_project",
        "add_frame",
        "add_figure",
        "get_state",
        "project_summary",
        "save_project",
        "set_camera",
        "delete_camera_key",
        "graph_key",
        "graph_copy_keys",
        "graph_paste_keys",
        "set_key_interp",
        "smooth_graph_tangents",
        "graph_tangent_preset",
        "bake_graph",
        "get_pose",
        "delete_graph_channel",
    ] {
        assert!(names.contains(&want), "tools/list is missing {want}");
    }
    // server_info counts the same registry tools/list serves.
    let info = call_ok(&mut server, "server_info", json!({}));
    assert_eq!(num(&info["tools"]) as usize, names.len());
}

// ---------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------

#[test]
fn camera_and_graph_authoring_end_to_end() {
    let mut server = six_frame_server();

    // ---- Camera keys -------------------------------------------
    // Plain key, no rotation surprises.
    let k0 = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 0, "x": 480.0, "y": 360.0, "zoom": 1.0, "angle": 45.0 }),
    );
    assert_eq!(k0["camera"]["x"], 480.0);
    assert_eq!(k0["camera"]["angle"], 45.0);

    // Rotation past 360: 725 degrees.
    let k2 = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 2, "x": 500.0, "y": 300.0, "zoom": 1.5, "angle": 725.0 }),
    );
    // Rotation below -180: -190 degrees.
    let k4 = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 4, "x": 320.0, "y": 240.0, "zoom": 0.75, "angle": -190.0 }),
    );
    // No angle argument: a fresh key starts unrotated.
    let k5 = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 5, "x": 600.0, "y": 400.0, "zoom": 2.0 }),
    );
    assert_eq!(k5["camera"]["angle"], 0.0);

    // Camera read-back (landed in Wave 11): get_camera lists
    // every key verbatim (plus the interpolated camera at a
    // frame) — the in-protocol read this suite originally had
    // to route around through save_project's file. The file
    // check below stays as the storage proof.
    let summary = call_ok(&mut server, "project_summary", json!({}));
    assert_eq!(summary["camera_keys"], 4);
    assert_eq!(summary["frames"], 6);

    // STORAGE CONVENTION (Wave 11, C2 ruling): set_camera
    // stores the angle WRAPPED to -180..180 via norm_deg —
    // the same convention every panel write uses — so 725 is
    // stored as 5 and -190 as 170, in the echo, in get_camera
    // and in the project file alike. (Entering the wave, MCP
    // stored angles verbatim and only the panel's display
    // wrapped; the pins below asserted that split then, and
    // assert the unified convention now.)
    assert_eq!(k2["camera"]["angle"], 5.0, "725 is stored wrapped to 5");
    assert_eq!(k4["camera"]["angle"], 170.0, "-190 is stored wrapped to 170");
    assert_close(norm_deg(725.0) as f64, 5.0, 1e-4, "norm_deg of 725");
    assert_close(norm_deg(-190.0) as f64, 170.0, 1e-4, "norm_deg of -190");

    // Upsert on an existing frame WITHOUT an angle: pan/zoom
    // update, the stored angle is preserved (the model's
    // set_camera_key never touches an existing key's angle).
    let up = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 2, "x": 520.0, "y": 310.0, "zoom": 1.25 }),
    );
    assert_eq!(up["camera"]["x"], 520.0);
    assert_eq!(up["camera"]["zoom"], 1.25);
    assert_eq!(up["camera"]["angle"], 5.0, "the wrapped angle survives a pan/zoom upsert");
    // ... and with an explicit angle it is replaced (wrapped).
    let up2 = call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 2, "x": 520.0, "y": 310.0, "zoom": 1.25, "angle": -30.0 }),
    );
    assert_eq!(up2["camera"]["angle"], -30.0);

    // get_camera reads the same stored state in-protocol:
    // four keys, sorted by frame, angles as stored (wrapped),
    // none stale on a 6-frame project.
    let cam = call_ok(&mut server, "get_camera", json!({}));
    let keys = cam["keys"].as_array().expect("get_camera keys array");
    assert_eq!(keys.len(), 4, "get_camera lists all four keys");
    assert_eq!(keys[1]["frame"], 2);
    assert_eq!(keys[1]["angle"], -30.0);
    assert_eq!(keys[2]["frame"], 4);
    assert_eq!(keys[2]["angle"], 170.0, "the -190 key reads back wrapped");
    assert!(
        keys.iter().all(|k| k["stale"] == false),
        "no key is stale inside the timeline: {keys:?}"
    );

    // Stored state, read back through save_project's file.
    let dir = temp_dir("save");
    let path = dir.join("session.styk.json");
    call_ok(
        &mut server,
        "save_project",
        json!({ "path": path.to_string_lossy() }),
    );
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read saved project"))
            .expect("saved project is JSON");
    let cams = saved["camera"].as_array().expect("saved camera array");
    assert_eq!(cams.len(), 4, "four camera keys are stored");
    // (frame, x, y, zoom, stored angle) — keys sorted by frame.
    let want_cams = [
        (0.0, 480.0, 360.0, 1.0, 45.0),
        (2.0, 520.0, 310.0, 1.25, -30.0),
        (4.0, 320.0, 240.0, 0.75, 170.0),
        (5.0, 600.0, 400.0, 2.0, 0.0),
    ];
    for (cam, want) in cams.iter().zip(want_cams) {
        assert_close(num(&cam["frame"]), want.0, 0.0, "stored camera frame");
        assert_close(num(&cam["x"]), want.1, 1e-3, "stored camera x");
        assert_close(num(&cam["y"]), want.2, 1e-3, "stored camera y");
        assert_close(num(&cam["zoom"]), want.3, 1e-3, "stored camera zoom");
        assert_close(num(&cam["angle"]), want.4, 1e-3, "stored camera angle (verbatim)");
    }
    let _ = std::fs::remove_dir_all(&dir);

    // Delete a camera key; the count drops and a second delete
    // of the same frame is refused.
    let del = call_ok(&mut server, "delete_camera_key", json!({ "frame": 4 }));
    assert_eq!(del["frame"], 4);
    let summary = call_ok(&mut server, "project_summary", json!({}));
    assert_eq!(summary["camera_keys"], 3);
    let err = call_err(&mut server, "delete_camera_key", json!({ "frame": 4 }));
    assert!(
        err.contains("No camera key at frame 5."),
        "delete refusal names the 1-based frame: {err}"
    );

    // ---- Graph channels ----------------------------------------
    // Channel creation order fixes the TRUE channel indices
    // used below: root_x = 0, root_y = 1, angle:2 = 2.
    let g = call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 0, "value": 400.0 }),
    );
    assert_eq!(g["channels"], 1);
    assert_eq!(g["channel_keys"], 1);
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 5, "value": 640.0 }),
    );
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_y", "frame": 0, "value": 500.0 }),
    );
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_y", "frame": 5, "value": 420.0 }),
    );
    let g = call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "angle:2", "frame": 0, "value": 0.0 }),
    );
    assert_eq!(g["channels"], 3, "three channels after the angle channel");
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "angle:2", "frame": 5, "value": 90.0 }),
    );

    // Read-back: freshly authored keys are flat Hermite keys.
    let keys = copy_keys(&mut server, 0, "root_x");
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["frame"], 0.0);
    assert_eq!(keys[0]["value"], 400.0);
    assert_eq!(keys[0]["in_tan"], 0.0);
    assert_eq!(keys[0]["out_tan"], 0.0);
    assert_eq!(keys[0]["interp"], "hermite");
    assert_eq!(keys[1]["frame"], 5.0);
    assert_eq!(keys[1]["value"], 640.0);

    // Smoothing derives every tangent from the key secants:
    // root_y falls 500 -> 420 over 5 frames (slope -16) and
    // root_x rises 400 -> 640 (slope 48); two-key channels
    // give both keys the segment slope on both sides.
    let smooth = call_ok(&mut server, "smooth_graph_tangents", json!({}));
    assert!(
        smooth["status"].as_str().unwrap_or("").contains("Smoothed tangents on 3"),
        "smooth status: {smooth}"
    );
    let keys = copy_keys(&mut server, 0, "root_y");
    for key in &keys {
        assert_close(num(&key["in_tan"]), -16.0, 1e-3, "smoothed root_y in_tan");
        assert_close(num(&key["out_tan"]), -16.0, 1e-3, "smoothed root_y out_tan");
    }
    let keys = copy_keys(&mut server, 0, "root_x");
    for key in &keys {
        assert_close(num(&key["in_tan"]), 48.0, 1e-3, "smoothed root_x in_tan");
        assert_close(num(&key["out_tan"]), 48.0, 1e-3, "smoothed root_x out_tan");
    }

    // Per-key tangents: authored here through
    // graph_paste_keys, whose key objects carry
    // in_tan/out_tan/interp — pasting over the same frames
    // replaces those keys in place. (The direct setter,
    // graph_set_tangents, landed in the same wave and is
    // exercised after the preset pass below; graph_key still
    // writes flat keys, and the global smooth/preset passes
    // remain the broad-stroke authors.)
    let pasted = call_ok(
        &mut server,
        "graph_paste_keys",
        json!({
            "figure": 0,
            "target": "root_x",
            "at_frame": 0,
            "keys": [
                { "frame": 0, "value": 400.0, "in_tan": 0.0, "out_tan": 12.0, "interp": "hermite" },
                { "frame": 5, "value": 640.0, "in_tan": -12.0, "out_tan": 0.0, "interp": "linear" }
            ]
        }),
    );
    assert_eq!(pasted["channel_keys"], 2, "paste replaced, not appended");
    let keys = copy_keys(&mut server, 0, "root_x");
    assert_eq!(keys[0]["out_tan"], 12.0);
    assert_eq!(keys[1]["in_tan"], -12.0);
    assert_eq!(keys[1]["interp"], "linear");

    // Interpolation by TRUE channel index: channel 0 is
    // root_x (created first). Setting its first key to Step
    // must not touch root_y (channel 1) or the angle channel.
    let set = call_ok(
        &mut server,
        "set_key_interp",
        json!({ "channel": 0, "key": 0, "interp": "step" }),
    );
    assert_eq!(set["interp"], "step");
    let keys = copy_keys(&mut server, 0, "root_x");
    assert_eq!(keys[0]["interp"], "step", "interp landed on channel 0 key 0");
    assert_eq!(keys[1]["interp"], "linear", "the other key is untouched");
    // Channel 2 is the angle channel: set its LAST key.
    call_ok(
        &mut server,
        "set_key_interp",
        json!({ "channel": 2, "key": 1, "interp": "step" }),
    );
    let keys = copy_keys(&mut server, 0, "angle:2");
    assert_eq!(keys[0]["interp"], "hermite");
    assert_eq!(keys[1]["interp"], "step", "interp landed on channel 2 key 1");
    let keys = copy_keys(&mut server, 0, "root_y");
    assert!(
        keys.iter().all(|k| k["interp"] == "hermite"),
        "channel 1 (root_y) was not hit by either interp edit: {keys:?}"
    );

    // Tangent preset: ease-inout scales each key's secants by
    // 0.35 on the selected figure's channels. Angle channel:
    // secant (90 - 0) / 5 = 18 -> tangents 6.3. root_x:
    // secant 48 -> 16.8. Interp modes are NOT a preset's
    // business (except Linear), so the Step above survives.
    let preset = call_ok(
        &mut server,
        "graph_tangent_preset",
        json!({ "preset": "ease-inout" }),
    );
    assert!(
        preset["status"].as_str().unwrap_or("").contains("preset to 3 graph channel(s)"),
        "preset status: {preset}"
    );
    let keys = copy_keys(&mut server, 0, "angle:2");
    for key in &keys {
        assert_close(num(&key["in_tan"]), 6.3, 0.01, "ease-inout angle in_tan");
        assert_close(num(&key["out_tan"]), 6.3, 0.01, "ease-inout angle out_tan");
    }
    let keys = copy_keys(&mut server, 0, "root_x");
    for key in &keys {
        assert_close(num(&key["in_tan"]), 16.8, 0.01, "ease-inout root_x in_tan");
        assert_close(num(&key["out_tan"]), 16.8, 0.01, "ease-inout root_x out_tan");
    }
    assert_eq!(keys[0]["interp"], "step", "preset leaves interp modes alone");

    // The direct per-key tangent setter (landed in Wave 11):
    // one call sets ONE key's two tangents, read back through
    // the copy surface — no paste composite needed.
    let set_tan = call_ok(
        &mut server,
        "graph_set_tangents",
        json!({ "channel": 0, "frame": 0, "in_tan": 3.0, "out_tan": -3.0 }),
    );
    assert_eq!(set_tan["ok"], true);
    let keys = copy_keys(&mut server, 0, "root_x");
    assert_eq!(keys[0]["in_tan"], 3.0, "graph_set_tangents wrote the in tangent");
    assert_eq!(keys[0]["out_tan"], -3.0, "graph_set_tangents wrote the out tangent");

    // ---- Shift keys --------------------------------------------
    // The shift below is the cross-channel composite: copy
    // one channel's keys and paste them into another channel
    // at a new frame — paste rebases the earliest key onto
    // at_frame, which is a true +1 shift here. (Dedicated
    // same-channel tools — graph_shift_keys and
    // graph_fill_keys — landed in the Wave 11 splice and are
    // unit-proven in graph_keys.)
    let fig = call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "humanoid", "name": "Second" }),
    );
    assert_eq!(fig["figure"], 1);
    // Seed figure 1's root_x channel with the key the paste
    // will land on (same frame/value, so the paste replaces
    // it rather than adding a neighbour).
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 1, "target": "root_x", "frame": 1, "value": 400.0 }),
    );
    let source = copy_keys(&mut server, 0, "root_x");
    let shifted = call_ok(
        &mut server,
        "graph_paste_keys",
        json!({ "figure": 1, "target": "root_x", "at_frame": 1, "keys": source }),
    );
    assert_eq!(shifted["channel_keys"], 2, "shift lands exactly two keys");
    let keys = copy_keys(&mut server, 1, "root_x");
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["frame"], 1.0, "earliest key shifted 0 -> 1");
    assert_eq!(keys[0]["value"], 400.0);
    assert_eq!(keys[1]["frame"], 6.0, "last key shifted 5 -> 6");
    assert_eq!(keys[1]["value"], 640.0);

    // ---- Bake ---------------------------------------------------
    // bake_graph samples every channel into the frame POSES
    // (it does not add channel keys). Read-back is get_pose.
    let baked = call_ok(
        &mut server,
        "bake_graph",
        json!({ "from": 0, "to": 5 }),
    );
    assert!(
        baked["status"].as_str().unwrap_or("").contains("Baked 4 graph channel(s)"),
        "bake status: {baked}"
    );
    let pose0 = call_ok(&mut server, "get_pose", json!({ "figure": 0, "frame": 0 }));
    assert_close(num(&pose0["root"][0]), 400.0, 1e-3, "baked frame 0 root x");
    assert_close(num(&pose0["root"][1]), 500.0, 1e-3, "baked frame 0 root y");
    assert_close(num(&pose0["angles"][2]), 0.0, 1e-3, "baked frame 0 angle:2");
    let pose5 = call_ok(&mut server, "get_pose", json!({ "figure": 0, "frame": 5 }));
    assert_close(num(&pose5["root"][0]), 640.0, 1e-3, "baked frame 5 root x");
    assert_close(num(&pose5["root"][1]), 420.0, 1e-3, "baked frame 5 root y");
    assert_close(num(&pose5["angles"][2]), 90.0, 1e-3, "baked frame 5 angle:2");
    // Mid-timeline: root_x key 0 is a Step key, so its value
    // holds across the whole segment — frame 2 still reads
    // 400 — while the Hermite root_y and angle channels are
    // strictly between their endpoint values. This is the
    // behavioural proof the interp edit reached evaluation.
    let pose2 = call_ok(&mut server, "get_pose", json!({ "figure": 0, "frame": 2 }));
    assert_close(num(&pose2["root"][0]), 400.0, 1e-3, "step key holds root x at frame 2");
    let mid_y = num(&pose2["root"][1]);
    assert!(mid_y > 420.0 && mid_y < 500.0, "frame 2 root y interpolates: {mid_y}");
    let mid_a = num(&pose2["angles"][2]);
    assert!(mid_a > 0.0 && mid_a < 90.0, "frame 2 angle:2 interpolates: {mid_a}");
}

// ---------------------------------------------------------------------
// Negative cases
// ---------------------------------------------------------------------

#[test]
fn negative_cases_refuse_and_leave_the_project_unchanged() {
    let mut server = six_frame_server();
    // Seed one camera key and one channel with two keys.
    call_ok(
        &mut server,
        "set_camera",
        json!({ "frame": 1, "x": 100.0, "y": 100.0, "zoom": 1.0, "angle": 10.0 }),
    );
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 0, "value": 400.0 }),
    );
    call_ok(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 5, "value": 640.0 }),
    );
    let baseline_keys = copy_keys(&mut server, 0, "root_x");
    let state = call_ok(&mut server, "get_state", json!({}));
    assert_eq!(state["current_frame"], 5, "playhead ends on the last added frame");

    // Camera key on a nonexistent frame (the project has
    // frames 0..=5): refused by the same check_frame the
    // panel-side ops use.
    let err = call_err(
        &mut server,
        "set_camera",
        json!({ "frame": 6, "x": 0.0, "y": 0.0, "zoom": 1.0 }),
    );
    assert!(err.contains("frame index 6 out of range"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "set_camera",
        json!({ "frame": 99, "x": 0.0, "y": 0.0, "zoom": 1.0 }),
    );
    assert!(err.contains("frame index 99 out of range"), "refusal: {err}");
    // A non-positive zoom on a REAL frame is refused too.
    let err = call_err(
        &mut server,
        "set_camera",
        json!({ "frame": 0, "x": 0.0, "y": 0.0, "zoom": 0.0 }),
    );
    assert!(err.contains("must be positive"), "refusal: {err}");
    // Deleting a camera key that does not exist.
    let err = call_err(&mut server, "delete_camera_key", json!({ "frame": 3 }));
    assert!(err.contains("No camera key at frame 4."), "refusal: {err}");

    // Graph authoring refusals: figure, node, target, time.
    let err = call_err(
        &mut server,
        "graph_key",
        json!({ "figure": 9, "target": "root_x", "frame": 0, "value": 0.0 }),
    );
    assert!(err.contains("figure index 9 out of range"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "angle:99", "frame": 0, "value": 0.0 }),
    );
    assert!(err.contains("node index 99 out of range"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "bogus", "frame": 0, "value": 0.0 }),
    );
    assert!(err.contains("unknown graph target"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": -1.0, "value": 0.0 }),
    );
    assert!(err.contains("must not be negative"), "refusal: {err}");
    // BY DESIGN (Wave 11 ruling): graph_key has NO upper
    // bound on frame — unlike set_camera's check_frame, a
    // key time past the timeline end (e.g. frame 42 on this
    // 6-frame project) is accepted, because channel keys are
    // raw f32 timeline coordinates (the stale-key philosophy:
    // keys may outlive the frames they were authored for).
    // Not asserted either way here.

    // Channel index and key index out of range.
    let err = call_err(
        &mut server,
        "set_key_interp",
        json!({ "channel": 99, "key": 0, "interp": "step" }),
    );
    assert!(err.contains("no graph channel at index 99"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "set_key_interp",
        json!({ "channel": 0, "key": 99, "interp": "step" }),
    );
    assert!(err.contains("No key 100 on graph channel 1"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "set_key_interp",
        json!({ "channel": 0, "key": 0, "interp": "bounce" }),
    );
    assert!(err.contains("unknown interp 'bounce'"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "delete_graph_channel",
        json!({ "channel": 99 }),
    );
    assert!(err.contains("no graph channel at index 99"), "refusal: {err}");
    let err = call_err(
        &mut server,
        "graph_copy_keys",
        json!({ "figure": 0, "target": "root_y" }),
    );
    assert!(!err.is_empty(), "copying a channel that does not exist is refused");

    // BY DESIGN (Wave 11, B8 §4.3): bake_graph does NOT
    // surface the op's refusal as an isError result — a
    // reversed range comes back ok:true with the refusal in
    // the status text (set_key_interp, by contrast, errs).
    // The status-only refusal pattern was reviewed and left
    // as-is deliberately (bake_graph / graph_smooth_keys /
    // graph_mirror_channel share it). The behaviour pinned
    // here is the honest one: status reports the refusal and
    // nothing is baked.
    let bad_bake = call_ok(&mut server, "bake_graph", json!({ "from": 5, "to": 2 }));
    assert!(
        bad_bake["status"].as_str().unwrap_or("").contains("invalid frame range"),
        "reversed bake reports its refusal in status: {bad_bake}"
    );
    let pose0 = call_ok(&mut server, "get_pose", json!({ "figure": 0, "frame": 0 }));
    assert_close(
        num(&pose0["root"][0]),
        480.0,
        1e-3,
        "reversed bake left the rest-pose root x untouched",
    );

    // Everything above left the project exactly as seeded.
    let summary = call_ok(&mut server, "project_summary", json!({}));
    assert_eq!(summary["frames"], 6);
    assert_eq!(summary["camera_keys"], 1);
    let after_keys = copy_keys(&mut server, 0, "root_x");
    assert_eq!(after_keys, baseline_keys, "channel keys unchanged by refusals");
    let state = call_ok(&mut server, "get_state", json!({}));
    assert_eq!(state["current_frame"], 5, "playhead unmoved by refusals");
    let stats = call_ok(&mut server, "get_project_stats", json!({}));
    assert_eq!(stats["channels"], 1, "no phantom channels from refused calls");
}
