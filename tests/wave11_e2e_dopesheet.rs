//! Integration tests: the DOPESHEET workflow as one MCP-only
//! user session (Wave 11, suite D2).
//!
//! Everything below drives the real MCP dispatch —
//! `McpServer::handle` with full JSON-RPC envelopes — exactly
//! as the stdio binary and the live-control HTTP path route
//! them. No editor core is touched directly: the session is
//! built, authored, navigated, edited and verified through
//! `tools/call` messages alone, and every step asserts the
//! reply's JSON shape, not just a success flag.
//!
//! The session mirrors what the GUI's dopesheet lane (Wave
//! 10) is for:
//!
//! 1. new project -> second figure -> five frames;
//! 2. distinct poses authored per frame: figure 0 keyed on
//!    every frame {0,1,2,3,4}, figure 1 keyed on {0,2,4}
//!    with frames 1 and 3 authored as exact holds;
//! 3. poses read back through `get_pose` and checked field
//!    by field;
//! 4. keyframe navigation (`goto_next/prev_keyframe`) from
//!    keyed frames, from non-key stands, from before the
//!    first key and from after the last — refusals included,
//!    with the playhead proven unmoved by `get_editor_status`;
//! 5. `delete_keyframe` on the middle key: the frame must
//!    afterwards HOLD the previous key's pose exactly, the
//!    following hold must become the new key, and the undo
//!    depth must grow by exactly one;
//! 6. the honest refusals: deleting frame 0's base key and
//!    deleting at a non-key frame are errors, and the
//!    project is proven unchanged afterwards;
//! 7. playback and selection state, driven and read back
//!    through `set_playback` / `get_editor_status` /
//!    `get_selection`.
//!
//! Keyframe-list evidence: a figure's key list is carried
//! inside the replies of `goto_next_keyframe`,
//! `goto_prev_keyframe` and `delete_keyframe` (each carries
//! `key_frames`) — tools that move the playhead or mutate —
//! and, since the Wave 11 splice landed `get_keyframes`, by
//! a dedicated read-only query for the very data the
//! dopesheet lane renders. That read is asserted by
//! `read_only_figure_keyframes_query` below (it began life
//! as the WAVE11-GAP absence pin).

use serde_json::{json, Value};
use stykanimate_rs::mcp::McpServer;

// ---------------------------------------------------------------------
// Dispatch helpers — the full JSON-RPC envelope, asserted every call
// ---------------------------------------------------------------------

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Send one `tools/call` and return `(payload, is_error)`,
/// where the payload is the parsed JSON document the result's
/// single text block carries — or the raw text as a string
/// when it is not JSON (tool error messages are plain text).
/// The envelope itself is asserted on every call: JSON-RPC
/// version, id echo, one text content block, boolean isError.
fn call(server: &mut McpServer, id: u64, name: &str, args: Value) -> (Value, bool) {
    let resp = server
        .handle(&request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        ))
        .expect("tools/call is a request and must be answered");
    assert_eq!(resp["jsonrpc"], "2.0", "{name}: bad jsonrpc version");
    assert_eq!(resp["id"], id, "{name}: response id does not echo the request");
    let result = &resp["result"];
    let content = result["content"].as_array().expect("content is an array");
    assert_eq!(content.len(), 1, "{name}: exactly one content block");
    assert_eq!(content[0]["type"], "text", "{name}: content block is text");
    let is_error = result["isError"].as_bool().expect("isError is a boolean");
    let text = content[0]["text"].as_str().expect("content block carries text");
    let payload = serde_json::from_str(text).unwrap_or(Value::String(text.to_string()));
    (payload, is_error)
}

/// A call that must succeed; returns its JSON payload.
fn call_ok(server: &mut McpServer, id: u64, name: &str, args: Value) -> Value {
    let (payload, is_error) = call(server, id, name, args);
    assert!(!is_error, "{name} unexpectedly failed: {payload}");
    payload
}

/// A call that must fail as a tool error (isError result, not
/// a protocol error); returns the plain-text message.
fn call_err(server: &mut McpServer, id: u64, name: &str, args: Value) -> String {
    let (payload, is_error) = call(server, id, name, args);
    assert!(is_error, "{name} unexpectedly succeeded: {payload}");
    payload
        .as_str()
        .expect("tool errors are plain-text messages")
        .to_string()
}

// ---------------------------------------------------------------------
// Session vocabulary — reads and moves the session leans on
// ---------------------------------------------------------------------

/// `get_editor_status` payload, shape-asserted.
fn editor_status(server: &mut McpServer, id: u64) -> Value {
    let s = call_ok(server, id, "get_editor_status", json!({}));
    for field in [
        "playing",
        "current_frame",
        "playhead",
        "loop_range",
        "selected_figure",
        "selected_node",
        "can_undo",
        "can_redo",
        "undo_depth",
        "redo_depth",
        "status",
    ] {
        assert!(
            s.get(field).is_some(),
            "get_editor_status is missing field {field}: {s}"
        );
    }
    s
}

/// `get_pose` payload ({root: [x, y], angles: [degrees...]}).
fn pose(server: &mut McpServer, id: u64, figure: usize, frame: usize) -> Value {
    let p = call_ok(
        server,
        id,
        "get_pose",
        json!({ "figure": figure, "frame": frame }),
    );
    assert!(p["root"].is_array(), "pose has a root array: {p}");
    assert!(p["angles"].is_array(), "pose has an angles array: {p}");
    p
}

/// Assert a pose's root and one node's angle against the
/// authored values (the session controls exactly these two
/// fields per frame; every other angle stays at the rest
/// value in every frame, so root + node 2 decide keyframe
/// equality — `stats::pose_changed` compares root within
/// 0.5 units and any angle within 0.01 degrees).
fn assert_pose(
    server: &mut McpServer,
    id: u64,
    figure: usize,
    frame: usize,
    root: [f64; 2],
    node2_deg: f64,
) {
    let p = pose(server, id, figure, frame);
    assert_eq!(
        p["root"],
        json!(root),
        "figure {figure} frame {frame}: root"
    );
    assert_eq!(
        p["angles"][2].as_f64().expect("angle is a number"),
        node2_deg,
        "figure {figure} frame {frame}: node 2 angle"
    );
}

// Authored poses. Figure 0: a new pose on every frame.
// Figure 1: keys at 0 / 2 / 4, exact holds at 1 and 3.
// Roots step by >= 30 units and node-2 angles by >= 45
// degrees between keys — far past the keyframe detector's
// epsilons, so the authored key sets are exact, not fuzzy.
const FIG0_ROOT_X: [f64; 5] = [480.0, 510.0, 540.0, 570.0, 600.0];
const FIG0_NODE2: [f64; 5] = [0.0, 15.0, 30.0, 45.0, 60.0];
const FIG1_ROOT_X: [f64; 5] = [390.0, 390.0, 450.0, 450.0, 510.0];
const FIG1_NODE2: [f64; 5] = [10.0, 10.0, 55.0, 55.0, 100.0];
const ROOT_Y: f64 = 470.0; // figure 1's row; figure 0's authored y is 468.0

/// Build the shared session: a named project, a second
/// humanoid figure, five frames, and both figures' poses
/// authored as above. Returns the server mid-session with
/// the playhead wherever `add_frame` left it (frame 4).
fn build_session() -> McpServer {
    let mut server = McpServer::new();
    let mut id = 0u64;
    let mut next = move || {
        id += 1;
        id
    };

    // -- Step 1: a fresh project -------------------------------
    let created = call_ok(
        &mut server,
        next(),
        "new_project",
        json!({ "name": "Dopesheet Session", "fps": 12 }),
    );
    assert_eq!(created["ok"], true);
    let summary = &created["project"];
    assert_eq!(summary["name"], "Dopesheet Session");
    assert_eq!(summary["fps"].as_f64(), Some(12.0));
    assert_eq!(summary["stage"], json!([960.0, 720.0]));
    assert_eq!(summary["figures"], 1, "a new project starts with one figure");
    assert_eq!(summary["frames"], 1, "a new project starts with one frame");
    assert_eq!(summary["current_frame"], 0);

    // -- Step 2: the second figure ------------------------------
    let added = call_ok(
        &mut server,
        next(),
        "add_figure",
        json!({ "kind": "humanoid", "name": "Partner", "color": 0xFF5D5D }),
    );
    assert_eq!(added["ok"], true);
    assert_eq!(added["figure"], 1, "the second figure takes index 1");
    assert_eq!(added["name"], "Partner");
    let nodes1 = added["nodes"].as_u64().expect("node count reported");

    let listed = call_ok(&mut server, next(), "list_figures", json!({}));
    let figures = listed["figures"].as_array().expect("figures is an array");
    assert_eq!(figures.len(), 2);
    assert_eq!(figures[0]["index"], 0);
    assert_eq!(figures[0]["name"], "Styk");
    assert_eq!(figures[1]["index"], 1);
    assert_eq!(figures[1]["name"], "Partner");
    assert_eq!(
        figures[0]["nodes"].as_u64().expect("node count reported"),
        nodes1,
        "both figures are humanoid: same node count"
    );
    assert!(nodes1 >= 3, "the session authors node 2: the rig must have one");

    // get_state sees the same project shape.
    let state = call_ok(&mut server, next(), "get_state", json!({}));
    assert_eq!(state["project"], "Dopesheet Session");
    assert_eq!(state["frame_count"], 1);
    assert_eq!(state["current_frame"], 0);
    assert_eq!(state["weather"], false);
    assert_eq!(state["weather_kind"], Value::Null);
    assert_eq!(state["figures"][1]["name"], "Partner");

    // -- Step 3: five frames ------------------------------------
    for want in 2..=5u64 {
        let r = call_ok(&mut server, next(), "add_frame", json!({}));
        assert_eq!(r["ok"], true);
        assert_eq!(
            r["frames"].as_u64(),
            Some(want),
            "frame count after add_frame"
        );
        assert_eq!(
            r["current_frame"].as_u64(),
            Some(want - 1),
            "add_frame moves to the frame it appended"
        );
    }
    let stats = call_ok(&mut server, next(), "get_project_stats", json!({}));
    assert_eq!(stats["figures"], 2);
    assert_eq!(stats["frames"], 5);
    assert_eq!(stats["fps"].as_f64(), Some(12.0));

    // -- Step 4: author the poses -------------------------------
    for f in 0..5usize {
        let r = call_ok(
            &mut server,
            next(),
            "set_pose",
            json!({
                "figure": 0,
                "frame": f,
                "root": [FIG0_ROOT_X[f], 468.0],
                "angles": { "2": FIG0_NODE2[f] },
            }),
        );
        assert_eq!(r["ok"], true);
        assert_eq!(r["figure"], 0);
        assert_eq!(r["frame"].as_u64(), Some(f as u64));
        assert_eq!(r["angles_changed"], 1, "one node angle per set_pose");
        // set_pose echoes the pose it wrote.
        assert_eq!(r["pose"]["root"], json!([FIG0_ROOT_X[f], 468.0]));
        assert_eq!(r["pose"]["angles"][2].as_f64(), Some(FIG0_NODE2[f]));

        let r = call_ok(
            &mut server,
            next(),
            "set_pose",
            json!({
                "figure": 1,
                "frame": f,
                "root": [FIG1_ROOT_X[f], ROOT_Y],
                "angles": { "2": FIG1_NODE2[f] },
            }),
        );
        assert_eq!(r["ok"], true);
        assert_eq!(r["pose"]["root"], json!([FIG1_ROOT_X[f], ROOT_Y]));
        assert_eq!(r["pose"]["angles"][2].as_f64(), Some(FIG1_NODE2[f]));
    }

    server
}

// ---------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------

#[test]
fn dopesheet_workflow_end_to_end() {
    let mut server = build_session();
    let mut id = 1000u64;
    let mut next = move || {
        id += 1;
        id
    };

    // -- Step 5: every authored pose reads back ------------------
    for f in 0..5usize {
        assert_pose(&mut server, next(), 0, f, [FIG0_ROOT_X[f], 468.0], FIG0_NODE2[f]);
        assert_pose(&mut server, next(), 1, f, [FIG1_ROOT_X[f], ROOT_Y], FIG1_NODE2[f]);
    }
    // The holds are exact: figure 1's frame 1 IS frame 0's
    // pose, and frame 3 IS frame 2's, as whole documents.
    assert_eq!(
        pose(&mut server, next(), 1, 0),
        pose(&mut server, next(), 1, 1),
        "frame 1 must be an exact hold of frame 0"
    );
    assert_eq!(
        pose(&mut server, next(), 1, 2),
        pose(&mut server, next(), 1, 3),
        "frame 3 must be an exact hold of frame 2"
    );
    // Non-vacuity: figure 0 really does change every frame.
    assert_ne!(
        pose(&mut server, next(), 0, 0),
        pose(&mut server, next(), 0, 1),
        "figure 0 frames 0 and 1 must differ — it is keyed on both"
    );

    // -- Step 6: playback state drives and reads back ------------
    let r = call_ok(
        &mut server,
        next(),
        "set_playback",
        json!({ "action": "restart" }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["playing"], true, "restart plays");
    assert_eq!(r["current_frame"], 0, "restart goes to frame 0");
    let s = editor_status(&mut server, next());
    assert_eq!(s["playing"], true);
    assert_eq!(s["current_frame"], 0);

    let r = call_ok(
        &mut server,
        next(),
        "set_playback",
        json!({ "action": "pause" }),
    );
    assert_eq!(r["playing"], false);
    let s = editor_status(&mut server, next());
    assert_eq!(s["playing"], false, "the pause must reach the session state");

    // -- Step 7: keyframe navigation, figure 1 (keys {0,2,4}) ---
    // Before the first key: at frame 0 there is no earlier key.
    let msg = call_err(
        &mut server,
        next(),
        "goto_prev_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(msg, "No earlier keyframe for figure 2.");
    assert_eq!(
        editor_status(&mut server, next())["current_frame"],
        0,
        "a refused jump must not move the playhead"
    );

    // The forward walk enumerates the key list through the
    // replies' `key_frames` field — also available read-only
    // via get_keyframes (see the query test at the end).
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["figure"], 1);
    assert_eq!(r["current_frame"], 2);
    assert_eq!(
        r["key_frames"],
        json!([0, 2, 4]),
        "figure 1's authored key list, reported by the jump"
    );
    assert_eq!(r["status"], "Figure 2 keyframe at frame 3.");

    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 4);
    assert_eq!(r["key_frames"], json!([0, 2, 4]));

    // After the last key: no later key, playhead unmoved.
    let msg = call_err(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(msg, "No later keyframe for figure 2.");
    assert_eq!(editor_status(&mut server, next())["current_frame"], 4);

    // The walk back down.
    let r = call_ok(
        &mut server,
        next(),
        "goto_prev_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 2);
    let r = call_ok(
        &mut server,
        next(),
        "goto_prev_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 0);

    // From NON-KEY stands: frame 3 is a hold for figure 1 —
    // next key is strictly after (4), prev strictly before (2).
    let r = call_ok(&mut server, next(), "set_frame", json!({ "frame": 3 }));
    assert_eq!(r["current_frame"], 3);
    assert_eq!(r["frame_count"], 5);
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 4, "from hold frame 3, next key is 4");
    let r = call_ok(&mut server, next(), "set_frame", json!({ "frame": 3 }));
    assert_eq!(r["current_frame"], 3);
    let r = call_ok(
        &mut server,
        next(),
        "goto_prev_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 2, "from hold frame 3, prev key is 2");
    // And from hold frame 1: next is 2, prev is 0.
    call_ok(&mut server, next(), "set_frame", json!({ "frame": 1 }));
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 2);
    call_ok(&mut server, next(), "set_frame", json!({ "frame": 1 }));
    let r = call_ok(
        &mut server,
        next(),
        "goto_prev_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 0);

    // Figure 0 is keyed everywhere: its list is the full range.
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 0 }),
    );
    assert_eq!(r["current_frame"], 1);
    assert_eq!(r["key_frames"], json!([0, 1, 2, 3, 4]));

    // Out-of-range figure is an honest error, not a jump.
    let msg = call_err(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 9 }),
    );
    assert_eq!(msg, "figure index 9 out of range (project has 2 figure(s))");

    // -- Step 8: selection follows the dopesheet's figure --------
    let r = call_ok(
        &mut server,
        next(),
        "set_selection",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["selected_figure"], 1);
    assert_eq!(r["selected_node"], Value::Null);
    let sel = call_ok(&mut server, next(), "get_selection", json!({}));
    assert_eq!(sel["selected_figure"], 1);
    assert_eq!(sel["figure_name"], "Partner");
    assert_eq!(sel["selected_node"], Value::Null);
    assert_eq!(editor_status(&mut server, next())["selected_figure"], 1);

    // -- Step 9: delete the middle key — the pose must HOLD ------
    let undo_before = editor_status(&mut server, next())["undo_depth"]
        .as_u64()
        .expect("undo depth is a count");
    let held_pose = pose(&mut server, next(), 1, 1); // the previous key's pose
    let r = call_ok(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 1, "frame": 2 }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["figure"], 1);
    assert_eq!(r["frame"], 2);
    assert_eq!(
        r["key_frames"],
        json!([0, 3, 4]),
        "after the delete, the old frame-3 hold becomes the frame-3 key"
    );
    assert_eq!(r["status"], "Figure 2 keyframe at frame 3 deleted.");

    // The hold, proven by reading the poses back: frame 2 now
    // IS frame 1's pose as a whole document, frame 3 keeps
    // the pose that used to key at frame 2, frame 4 is
    // untouched.
    assert_eq!(
        pose(&mut server, next(), 1, 2),
        held_pose,
        "frame 2 must hold the previous key's pose exactly"
    );
    assert_pose(&mut server, next(), 1, 3, [450.0, ROOT_Y], 55.0);
    assert_pose(&mut server, next(), 1, 4, [510.0, ROOT_Y], 100.0);
    assert_eq!(
        editor_status(&mut server, next())["undo_depth"].as_u64(),
        Some(undo_before + 1),
        "the delete is exactly one undo step"
    );

    // The new key list, walked: 0 -> 3 -> 4 -> refused.
    call_ok(&mut server, next(), "set_frame", json!({ "frame": 0 }));
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 3);
    assert_eq!(r["key_frames"], json!([0, 3, 4]));
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(r["current_frame"], 4);
    let msg = call_err(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(msg, "No later keyframe for figure 2.");

    // -- Step 10: the honest refusals change NOTHING -------------
    // Frame 0's base key, for both figures.
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 1, "frame": 0 }),
    );
    assert_eq!(
        msg,
        "Frame 1 is the base pose \u{2014} its keyframe cannot be deleted."
    );
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 0, "frame": 0 }),
    );
    assert_eq!(
        msg,
        "Frame 1 is the base pose \u{2014} its keyframe cannot be deleted."
    );
    // A non-key frame: figure 1's frame 1 is a hold, and
    // figure 0's frame 2 key was never figure 1's business —
    // frame 2 for figure 1 is now a hold too (post-delete).
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 1, "frame": 1 }),
    );
    assert_eq!(msg, "No keyframe for figure 2 at frame 2.");
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 1, "frame": 2 }),
    );
    assert_eq!(msg, "No keyframe for figure 2 at frame 3.");
    // Out-of-range indices refuse before anything else.
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 9, "frame": 1 }),
    );
    assert_eq!(msg, "figure index 9 out of range (project has 2 figure(s))");
    let msg = call_err(
        &mut server,
        next(),
        "delete_keyframe",
        json!({ "figure": 1, "frame": 9 }),
    );
    assert_eq!(msg, "frame index 9 out of range (project has 5 frame(s))");

    // Project unchanged by every refusal above: poses and
    // both key lists exactly as after the one real delete.
    assert_pose(&mut server, next(), 1, 0, [390.0, ROOT_Y], 10.0);
    assert_eq!(pose(&mut server, next(), 1, 1), held_pose);
    assert_eq!(pose(&mut server, next(), 1, 2), held_pose);
    assert_pose(&mut server, next(), 1, 3, [450.0, ROOT_Y], 55.0);
    assert_pose(&mut server, next(), 1, 4, [510.0, ROOT_Y], 100.0);
    for f in 0..5usize {
        assert_pose(&mut server, next(), 0, f, [FIG0_ROOT_X[f], 468.0], FIG0_NODE2[f]);
    }
    call_ok(&mut server, next(), "set_frame", json!({ "frame": 0 }));
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 1 }),
    );
    assert_eq!(
        r["key_frames"],
        json!([0, 3, 4]),
        "figure 1's key list survived the refusals"
    );
    call_ok(&mut server, next(), "set_frame", json!({ "frame": 0 }));
    let r = call_ok(
        &mut server,
        next(),
        "goto_next_keyframe",
        json!({ "figure": 0 }),
    );
    assert_eq!(
        r["key_frames"],
        json!([0, 1, 2, 3, 4]),
        "figure 0's key list survived the refusals"
    );

    // -- Step 11: playback reflects the session's end state ------
    let r = call_ok(
        &mut server,
        next(),
        "set_playback",
        json!({ "action": "toggle" }),
    );
    assert_eq!(r["playing"], true);
    assert_eq!(r["current_frame"], 1, "toggle does not move the playhead");
    let s = editor_status(&mut server, next());
    assert_eq!(s["playing"], true);
    assert_eq!(s["current_frame"], 1);
    let r = call_ok(
        &mut server,
        next(),
        "set_playback",
        json!({ "action": "toggle" }),
    );
    assert_eq!(r["playing"], false);
    assert_eq!(editor_status(&mut server, next())["playing"], false);

    // A bad action is refused with the valid set named.
    let msg = call_err(
        &mut server,
        next(),
        "set_playback",
        json!({ "action": "spin" }),
    );
    assert!(
        msg.contains("play") && msg.contains("play_range"),
        "the refusal names the valid actions: {msg}"
    );
}

/// The dopesheet's own data — a figure's keyframe list, the
/// exact vector the lane renders from
/// `stats::figure_keyframes` — is readable on its own via
/// `get_keyframes` { figure } -> { figure, key_frames, .. }.
///
/// This began as the WAVE11-GAP absence pin: the list used
/// to leak out only inside the success replies of
/// `goto_next_keyframe` / `goto_prev_keyframe` (which move
/// the playhead) and `delete_keyframe` (which mutates). The
/// splice landed the read (frame_swap's handler, registered
/// in the Wave 11 census), so the pin flipped to positive
/// assertions of the tool's presence and reply.
#[test]
fn read_only_figure_keyframes_query() {
    // The read-only query exists and is listed alongside the
    // keyframe tools that move or mutate.
    let mut server = McpServer::new();
    let resp = server
        .handle(&request(1, "tools/list", json!({})))
        .expect("tools/list must be answered");
    let tools = resp["result"]["tools"].as_array().expect("tools is an array");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("every tool has a name"))
        .collect();
    assert!(
        names.contains(&"get_keyframes"),
        "the read-only keyframe query must be registered"
    );
    let mut keyframe_named: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| n.contains("keyframe"))
        .collect();
    keyframe_named.sort_unstable();
    assert_eq!(
        keyframe_named,
        vec![
            "delete_keyframe",
            "get_keyframes",
            "goto_next_keyframe",
            "goto_prev_keyframe"
        ],
        "the keyframe-named surface: one read, two jumps, one delete"
    );

    // Its reply carries the dopesheet's own census for the
    // authored session (figure 1 keys at 0, 2, 4) and does
    // not move the playhead or spend an undo step.
    let mut server = build_session();
    let mut id = 5000u64;
    let mut next = move || {
        id += 1;
        id
    };
    let undo_before = editor_status(&mut server, next())["undo_depth"]
        .as_u64()
        .expect("undo depth is a count");
    let frame_before = editor_status(&mut server, next())["current_frame"].clone();
    let r = call_ok(&mut server, next(), "get_keyframes", json!({ "figure": 1 }));
    assert_eq!(r["ok"], true);
    assert_eq!(r["figure"], 1);
    assert_eq!(r["key_frames"], json!([0, 2, 4]));
    assert_eq!(r["frame_count"], 5);
    let s = editor_status(&mut server, next());
    assert_eq!(s["current_frame"], frame_before, "a read must not move the playhead");
    assert_eq!(
        s["undo_depth"].as_u64(),
        Some(undo_before),
        "a read must not spend an undo step"
    );

    // Out-of-range figure is an honest error.
    let msg = call_err(&mut server, next(), "get_keyframes", json!({ "figure": 9 }));
    assert!(
        msg.contains("out of range"),
        "unexpected refusal wording: {msg}"
    );
}
