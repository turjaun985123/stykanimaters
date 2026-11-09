//! Integration tests: an MCP-only undo/redo + selection + status
//! session, driven through the real JSON-RPC dispatch
//! (`McpServer::handle`) from start to finish.
//!
//! Undo/redo exposure verdict (censused at the source before this
//! file was written): undo and redo ARE exposed to MCP clients,
//! but not as `mcp.rs` built-ins. They are registry tools in
//! `mcp_tools::tools()` (names `undo` / `redo`), whose handlers
//! call `EditorCore::undo` / `EditorCore::redo` and answer
//! `{ ok, status, project }`. The Wave 10 built-in
//! `get_editor_status` complements them with `can_undo` /
//! `can_redo` and the two history depths, so an MCP client can
//! observe and drive the same history the GUI's Ctrl+Z uses.
//! The loop is therefore not half-open at the dispatch layer.
//!
//! The history contract these tests pin is the one
//! `EditorCore::snapshot` / `undo` / `redo` actually implement:
//!
//! - `snapshot()` serialises the pre-edit `Project` onto the
//!   undo stack (capped at 60), and CLEARS the redo stack — so a
//!   new edit after an undo makes the undone edit unreachable.
//! - `undo()` / `redo()` restore only the `Project`. The
//!   playhead and the selection are session state: they are not
//!   rewound, only clamped into the restored project's bounds.
//! - Stepping past either end is an honest no-op, never an
//!   error and never a panic: the core sets the status line to
//!   "Nothing to undo." / "Nothing to redo." and the tool still
//!   answers `ok: true` carrying that status.
//!
//! Two prescribed session mutations entered Wave 11 NOT
//! honouring the one-edit-one-undo-step contract, because
//! their registry handlers wrote the project directly
//! without snapshotting:
//!
//! - `set_pose` (`mcp_tools::tool_set_pose`) mutated through
//!   `pose_mut` with no `core.snapshot()` call, and
//! - `add_fx_clip` (`mcp_tools::tool_add_fx_clip`) pushed onto
//!   `project.fx_clips` with no `core.snapshot()` call.
//!
//! Both handlers snapshot now (the wave's undo-discipline
//! ruling: every mutating MCP tool is exactly one undo
//! step), and the two tests at the end of this file —
//! written against the correct behaviour from the start —
//! pin the fixed depths. (`add_figure` and `add_frame` were
//! always fine: the core ops they call snapshot first.)
//!
//! Everything below speaks only MCP messages — no test touches
//! the server's core directly.

use serde_json::{json, Value};

use stykanimate_rs::mcp::McpServer;

/// One answered `tools/call`: the decoded payload (the tool's
/// JSON result, or the failure text as a string when the tool
/// answered `isError`) plus the error flag itself.
struct Reply {
    payload: Value,
    is_error: bool,
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Send one `tools/call` through the real dispatch and decode
/// the text content block the router wraps tool outcomes in —
/// the same envelope `mcp.rs`'s own tests unwrap.
fn call(server: &mut McpServer, name: &str, args: Value) -> Reply {
    let resp = server
        .handle(&request(
            1,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        ))
        .expect("tools/call is a request and must be answered");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp.get("error").is_none(),
        "{name}: unexpected JSON-RPC protocol error: {resp}"
    );
    let result = &resp["result"];
    let text = result["content"][0]["text"]
        .as_str()
        .expect("content block carries text");
    Reply {
        payload: serde_json::from_str(text)
            .unwrap_or_else(|_| Value::String(text.to_string())),
        is_error: result["isError"].as_bool().unwrap_or(false),
    }
}

/// A `tools/call` that must succeed; returns its payload.
fn call_ok(server: &mut McpServer, name: &str, args: Value) -> Value {
    let reply = call(server, name, args);
    assert!(
        !reply.is_error,
        "{name} unexpectedly failed: {}",
        reply.payload
    );
    reply.payload
}

/// Every tool name the server advertises over `tools/list`.
fn tool_names(server: &mut McpServer) -> Vec<String> {
    let resp = server
        .handle(&request(2, "tools/list", json!({})))
        .expect("tools/list is a request and must be answered");
    resp["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array")
        .iter()
        .map(|t| {
            t["name"]
                .as_str()
                .expect("each tool carries a name")
                .to_string()
        })
        .collect()
}

/// The Wave 10 status built-in's snapshot of the session.
fn editor_status(server: &mut McpServer) -> Value {
    call_ok(server, "get_editor_status", json!({}))
}

/// Assert the session fields the status built-in reports:
/// playback, playhead, selected figure and the history depths.
fn assert_status(
    server: &mut McpServer,
    playing: bool,
    current_frame: u64,
    selected_figure: u64,
    undo_depth: u64,
    redo_depth: u64,
) {
    let status = editor_status(server);
    assert_eq!(status["playing"], json!(playing), "playing: {status}");
    assert_eq!(
        status["current_frame"],
        json!(current_frame),
        "current_frame: {status}"
    );
    assert_eq!(
        status["selected_figure"],
        json!(selected_figure),
        "selected_figure: {status}"
    );
    assert_eq!(
        status["undo_depth"],
        json!(undo_depth),
        "undo_depth: {status}"
    );
    assert_eq!(
        status["redo_depth"],
        json!(redo_depth),
        "redo_depth: {status}"
    );
    assert_eq!(
        status["can_undo"],
        json!(undo_depth > 0),
        "can_undo: {status}"
    );
    assert_eq!(
        status["can_redo"],
        json!(redo_depth > 0),
        "can_redo: {status}"
    );
}

/// Assert the selection the selection built-in reports: figure
/// index, figure name (`None` = out of range / null) and node
/// index (`None` = no node selected).
fn assert_selection(
    server: &mut McpServer,
    figure: u64,
    figure_name: Option<&str>,
    node: Option<u64>,
) {
    let selection = call_ok(server, "get_selection", json!({}));
    assert_eq!(
        selection["selected_figure"],
        json!(figure),
        "selected_figure: {selection}"
    );
    match figure_name {
        Some(name) => assert_eq!(
            selection["figure_name"],
            json!(name),
            "figure_name: {selection}"
        ),
        None => assert_eq!(
            selection["figure_name"],
            Value::Null,
            "figure_name: {selection}"
        ),
    }
    match node {
        Some(node) => assert_eq!(
            selection["selected_node"],
            json!(node),
            "selected_node: {selection}"
        ),
        None => assert_eq!(
            selection["selected_node"],
            Value::Null,
            "selected_node: {selection}"
        ),
    }
}

/// The project-shape oracle (`get_state` built-in).
fn project_state(server: &mut McpServer) -> Value {
    call_ok(server, "get_state", json!({}))
}

/// A figure's pose root in a frame, via `get_pose`.
fn pose_root(server: &mut McpServer, figure: u64, frame: u64) -> Value {
    let pose = call_ok(
        server,
        "get_pose",
        json!({ "figure": figure, "frame": frame }),
    );
    pose["root"].clone()
}

/// The FX clip list, via `fx_list`.
fn fx_list(server: &mut McpServer) -> Value {
    call_ok(server, "fx_list", json!({}))
}

/// Start the prescribed session: a fresh project through the
/// `new_project` registry tool on a fresh server. Its undo
/// stack is empty (a new server has no history, and the tool
/// replaces the project wholesale without snapshotting), so the
/// returned project summary is the baseline every later undo
/// assertion is measured against.
fn start_new_project(server: &mut McpServer, name: &str) -> Value {
    let out = call_ok(
        server,
        "new_project",
        json!({ "name": name, "fps": 12, "width": 960, "height": 720 }),
    );
    let summary = out["project"].clone();
    assert_eq!(summary["name"], json!(name), "summary: {summary}");
    assert_eq!(summary["figures"], json!(1), "summary: {summary}");
    assert_eq!(summary["frames"], json!(1), "summary: {summary}");
    assert_eq!(summary["current_frame"], json!(0), "summary: {summary}");
    assert_status(server, false, 0, 0, 0, 0);
    assert_selection(server, 0, Some("Styk"), None);
    let status = editor_status(server);
    assert_eq!(
        status["status"],
        json!("New project (via MCP)."),
        "status line: {status}"
    );
    summary
}

#[test]
fn undo_redo_and_session_tools_are_all_listed_and_reachable() {
    let mut server = McpServer::new();
    let names = tool_names(&mut server);
    for want in [
        // The history pair under test (registry tools).
        "undo",
        "redo",
        // The Wave 10 session built-ins this session reads.
        "get_editor_status",
        "set_playback",
        "get_selection",
        "set_selection",
        // The mutation and oracle tools the session drives.
        "new_project",
        "add_figure",
        "set_pose",
        "add_frame",
        "add_fx_clip",
        "get_state",
        "get_pose",
        "fx_list",
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "tools/list is missing {want:?}"
        );
    }

    // A fresh server answers the status built-in with an empty
    // history before any session starts.
    let status = editor_status(&mut server);
    assert_eq!(status["can_undo"], json!(false), "status: {status}");
    assert_eq!(status["can_redo"], json!(false), "status: {status}");
    assert_eq!(status["undo_depth"], json!(0), "status: {status}");
    assert_eq!(status["redo_depth"], json!(0), "status: {status}");

    start_new_project(&mut server, "Wave 11 Exposure");
}

#[test]
fn status_selection_and_playback_track_each_mcp_mutation() {
    let mut server = McpServer::new();
    start_new_project(&mut server, "Wave 11 Tracking");

    // Selection is session state the built-ins drive directly:
    // selecting figure 0's root node is not an undoable edit.
    let out = call_ok(
        &mut server,
        "set_selection",
        json!({ "figure": 0, "node": 0 }),
    );
    assert_eq!(out["selected_node"], json!(0));
    assert_selection(&mut server, 0, Some("Styk"), Some(0));
    assert_status(&mut server, false, 0, 0, 0, 0);

    // Playback flips the same flag the transport button flips,
    // and the status built-in tracks every transition. None of
    // this touches the undo history.
    let out = call_ok(
        &mut server,
        "set_playback",
        json!({ "action": "play" }),
    );
    assert_eq!(out["playing"], json!(true));
    assert_status(&mut server, true, 0, 0, 0, 0);
    let out = call_ok(
        &mut server,
        "set_playback",
        json!({ "action": "toggle" }),
    );
    assert_eq!(out["playing"], json!(false));
    let out = call_ok(
        &mut server,
        "set_playback",
        json!({ "action": "toggle" }),
    );
    assert_eq!(out["playing"], json!(true));
    let out = call_ok(
        &mut server,
        "set_playback",
        json!({ "action": "pause" }),
    );
    assert_eq!(out["playing"], json!(false));
    assert_status(&mut server, false, 0, 0, 0, 0);

    // Mutation 1 — add a figure. The core op selects the new
    // figure, exactly as the figures panel's add does, and
    // records one undo step. It does NOT clear the node
    // selection (that is `select_figure`'s contract, via
    // set_selection): the node index carries over and now
    // addresses the NEW figure's node 0. (This suite's first
    // CI execution was this run — cargo's fail-fast had
    // skipped the binary behind earlier suites' failures —
    // and the entering expectation of a cleared node was
    // wrong about the shared op; the real contract is
    // pinned here.)
    let out = call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "humanoid", "name": "Second" }),
    );
    assert_eq!(out["figure"], json!(1));
    assert_eq!(out["name"], json!("Second"));
    assert_status(&mut server, false, 0, 1, 1, 0);
    assert_selection(&mut server, 1, Some("Second"), Some(0));
    assert_eq!(project_state(&mut server)["figures"][1]["name"], json!("Second"));

    // Select the new figure with its root node, then mutate
    // something else: the selection must survive untouched, as
    // it does in the GUI.
    call_ok(
        &mut server,
        "set_selection",
        json!({ "figure": 1, "node": 0 }),
    );

    // Mutation 2 — add a frame. The playhead moves onto the new
    // frame; the selection (figure AND node) is preserved.
    let out = call_ok(&mut server, "add_frame", json!({}));
    assert_eq!(out["frames"], json!(2));
    assert_eq!(out["current_frame"], json!(1));
    assert_status(&mut server, false, 1, 1, 2, 0);
    assert_selection(&mut server, 1, Some("Second"), Some(0));

    // Restart rewinds the playhead and plays; pause stops it.
    // Neither is an edit: the history depths do not move.
    let out = call_ok(
        &mut server,
        "set_playback",
        json!({ "action": "restart" }),
    );
    assert_eq!(out["current_frame"], json!(0));
    assert_eq!(out["playing"], json!(true));
    call_ok(&mut server, "set_playback", json!({ "action": "pause" }));
    assert_status(&mut server, false, 0, 1, 2, 0);
    assert_selection(&mut server, 1, Some("Second"), Some(0));

    // Mutation 3 — move figure 0's pose in frame 0. The pose
    // oracle reads the new root back; the selection (on the
    // OTHER figure) and the playhead stay where they were.
    // (Whether this edit is undoable is a separate question —
    // see the undo-step tests below — so no depth is asserted
    // here.)
    let rest_root = pose_root(&mut server, 0, 0);
    // The default figure's rest root is [480, 468] by
    // construction, computed in f32 — the read-back carries
    // the f32 rounding (467.99997), so compare with a
    // tolerance rather than exact JSON equality.
    let rr = rest_root.as_array().expect("root is an array");
    assert!(
        (rr[0].as_f64().unwrap() - 480.0).abs() < 0.01
            && (rr[1].as_f64().unwrap() - 468.0).abs() < 0.01,
        "rest root near [480, 468]: {rest_root}"
    );
    let out = call_ok(
        &mut server,
        "set_pose",
        json!({ "figure": 0, "frame": 0, "root": [430.0, 450.0] }),
    );
    assert_eq!(out["pose"]["root"], json!([430.0, 450.0]));
    assert_eq!(pose_root(&mut server, 0, 0), json!([430.0, 450.0]));
    assert_selection(&mut server, 1, Some("Second"), Some(0));
    let status = editor_status(&mut server);
    assert_eq!(status["current_frame"], json!(0), "status: {status}");
    assert_eq!(status["playing"], json!(false), "status: {status}");

    // Mutation 4 — add an FX clip anchored to figure 0. The
    // clip oracle lists it with its authored span; selection,
    // playhead and playback are again untouched. (Its undo
    // coverage is the other undo-step test below.)
    let out = call_ok(
        &mut server,
        "add_fx_clip",
        json!({
            "figure": 0,
            "node": 0,
            "kind": "burst",
            "start": 0.0,
            "duration": 1.5,
        }),
    );
    assert_eq!(out["fx_index"], json!(0));
    assert_eq!(out["kind"], json!("Burst"));
    let list = fx_list(&mut server);
    assert_eq!(list["count"], json!(1), "fx list: {list}");
    assert_eq!(list["clips"][0]["figure"], json!(0), "fx list: {list}");
    assert_eq!(list["clips"][0]["node"], json!(0), "fx list: {list}");
    assert_eq!(list["clips"][0]["kind"], json!("burst"), "fx list: {list}");
    assert_eq!(list["clips"][0]["start"], json!(0.0), "fx list: {list}");
    assert_eq!(list["clips"][0]["duration"], json!(1.5), "fx list: {list}");
    assert_eq!(list["clips"][0]["end"], json!(1.5), "fx list: {list}");
    assert_selection(&mut server, 1, Some("Second"), Some(0));
    let status = editor_status(&mut server);
    assert_eq!(status["current_frame"], json!(0), "status: {status}");
    assert_eq!(status["playing"], json!(false), "status: {status}");
}

#[test]
fn undo_redo_round_trip_unwinds_in_reverse_and_stops_at_the_ends() {
    let mut server = McpServer::new();
    let initial_summary = start_new_project(&mut server, "Wave 11 Round Trip");

    // Two snapshotting mutations: A adds a figure, B adds a
    // frame. (The pose/FX mutations are covered by their own
    // tests — their handlers' missing snapshots are the
    // undo gaps documented at the top of this file, both closed in Wave 11.)
    call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "humanoid", "name": "Second" }),
    );
    call_ok(
        &mut server,
        "set_selection",
        json!({ "figure": 1, "node": 0 }),
    );
    call_ok(&mut server, "add_frame", json!({}));
    assert_status(&mut server, false, 1, 1, 2, 0);

    // Undo B: the frame goes, the figure stays. The playhead
    // was on the removed frame and clamps back to frame 0;
    // the selection is session state and is NOT rewound.
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["status"], json!("Undone."));
    assert_eq!(out["project"]["figures"], json!(2), "undo B: {out}");
    assert_eq!(out["project"]["frames"], json!(1), "undo B: {out}");
    assert_eq!(out["project"]["current_frame"], json!(0), "undo B: {out}");
    assert_status(&mut server, false, 0, 1, 1, 1);
    assert_selection(&mut server, 1, Some("Second"), Some(0));

    // Undo A: the project is back to the exact summary the
    // session started from. The selected figure index clamps
    // into the restored project (figure 1 no longer exists);
    // the node index is left as it was — the core clamps the
    // figure only, and node 0 exists on figure 0 too.
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["status"], json!("Undone."));
    assert_eq!(
        out["project"], initial_summary,
        "undo A returns to the initial summary"
    );
    assert_status(&mut server, false, 0, 0, 0, 2);
    assert_selection(&mut server, 0, Some("Styk"), Some(0));

    // Undo past the beginning: an honest no-op. The tool still
    // answers ok, the status line says there is nothing to
    // undo, and the project does not move.
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["status"], json!("Nothing to undo."));
    assert_eq!(out["project"], initial_summary, "boundary undo: {out}");
    assert_status(&mut server, false, 0, 0, 0, 2);

    // Redo A: the figure returns. The selection is NOT
    // restored to figure 1 — history holds projects, not
    // session state — it stays clamped on figure 0.
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["status"], json!("Redone."));
    assert_eq!(out["project"]["figures"], json!(2), "redo A: {out}");
    assert_eq!(out["project"]["frames"], json!(1), "redo A: {out}");
    assert_status(&mut server, false, 0, 0, 1, 1);
    assert_selection(&mut server, 0, Some("Styk"), Some(0));

    // Redo B: the frame returns. The playhead is not rewound
    // either — it stays on frame 0 rather than returning to
    // the frame B had made current.
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["status"], json!("Redone."));
    assert_eq!(out["project"]["figures"], json!(2), "redo B: {out}");
    assert_eq!(out["project"]["frames"], json!(2), "redo B: {out}");
    assert_eq!(out["project"]["current_frame"], json!(0), "redo B: {out}");
    assert_status(&mut server, false, 0, 0, 2, 0);
    let state = project_state(&mut server);
    assert_eq!(state["frame_count"], json!(2), "state: {state}");
    assert_eq!(state["figures"][1]["name"], json!("Second"), "state: {state}");
    let redone_summary = out["project"].clone();

    // Redo past the end: the same honest no-op contract.
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["status"], json!("Nothing to redo."));
    assert_eq!(out["project"], redone_summary, "boundary redo: {out}");
    assert_status(&mut server, false, 0, 0, 2, 0);
}

#[test]
fn a_new_mutation_after_undo_makes_the_undone_edit_unreachable() {
    let mut server = McpServer::new();
    start_new_project(&mut server, "Wave 11 Interleave");

    // A, then B, then undo B — one redo is now available.
    call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "humanoid", "name": "First Added" }),
    );
    call_ok(&mut server, "add_frame", json!({}));
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["project"]["frames"], json!(1), "undo B: {out}");
    assert_status(&mut server, false, 0, 1, 1, 1);

    // C: a NEW mutation on the branch. `EditorCore::snapshot`
    // clears the redo stack, so B's frame must be gone for
    // good — standard undo semantics, and the contract the
    // core's snapshot code promises.
    let out = call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "mini", "name": "Branch Added" }),
    );
    assert_eq!(out["figure"], json!(2));
    assert_status(&mut server, false, 0, 2, 2, 0);
    let state = project_state(&mut server);
    assert_eq!(state["frame_count"], json!(1), "state: {state}");
    assert_eq!(state["figures"][1]["name"], json!("First Added"), "state: {state}");
    assert_eq!(state["figures"][2]["name"], json!("Branch Added"), "state: {state}");

    // Redo now reports the empty stack and changes nothing:
    // the undone frame does not come back.
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["status"], json!("Nothing to redo."));
    assert_eq!(out["project"]["figures"], json!(3), "redo: {out}");
    assert_eq!(out["project"]["frames"], json!(1), "redo: {out}");
    assert_status(&mut server, false, 0, 2, 2, 0);
}

#[test]
fn set_pose_is_a_single_undo_step() {
    let mut server = McpServer::new();
    start_new_project(&mut server, "Wave 11 Pose Gap");
    let rest_root = pose_root(&mut server, 0, 0);

    call_ok(
        &mut server,
        "set_pose",
        json!({ "figure": 0, "frame": 0, "root": [430.0, 450.0] }),
    );
    assert_eq!(pose_root(&mut server, 0, 0), json!([430.0, 450.0]));

    // Undo discipline (landed in Wave 11): `tool_set_pose`
    // snapshots after validation, before `pose_mut` — the
    // move is exactly one undo step, like every other MCP
    // mutation. Entering the wave it wrote through with no
    // snapshot at all and the change was invisible to
    // history; this test was the gap pin for that.
    let status = editor_status(&mut server);
    assert_eq!(
        status["undo_depth"],
        json!(1),
        "set_pose must record exactly one undo step: {status}"
    );
    assert_eq!(status["can_undo"], json!(true), "status: {status}");

    // One undo reverts exactly the pose move; one redo
    // reapplies exactly it.
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["status"], json!("Undone."));
    assert_eq!(pose_root(&mut server, 0, 0), rest_root);
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["status"], json!("Redone."));
    assert_eq!(pose_root(&mut server, 0, 0), json!([430.0, 450.0]));
}

#[test]
fn add_fx_clip_is_a_single_undo_step() {
    let mut server = McpServer::new();
    start_new_project(&mut server, "Wave 11 FX Gap");
    assert_eq!(fx_list(&mut server)["count"], json!(0));

    call_ok(
        &mut server,
        "add_fx_clip",
        json!({
            "figure": 0,
            "node": 0,
            "kind": "burst",
            "start": 0.0,
            "duration": 1.5,
        }),
    );
    assert_eq!(fx_list(&mut server)["count"], json!(1));

    // Undo discipline (landed in Wave 11): `tool_add_fx_clip`
    // snapshots before the push, so adding a clip is exactly
    // one undo step, like `fx_move` and the other FX tools
    // whose descriptions promise one. Entering the wave it
    // pushed with no snapshot and the clip could not be
    // undone at all; this test was the gap pin for that.
    let status = editor_status(&mut server);
    assert_eq!(
        status["undo_depth"],
        json!(1),
        "add_fx_clip must record exactly one undo step: {status}"
    );
    assert_eq!(status["can_undo"], json!(true), "status: {status}");

    // One undo removes exactly the clip; one redo restores it
    // with its authored span.
    let out = call_ok(&mut server, "undo", json!({}));
    assert_eq!(out["status"], json!("Undone."));
    assert_eq!(fx_list(&mut server)["count"], json!(0));
    let out = call_ok(&mut server, "redo", json!({}));
    assert_eq!(out["status"], json!("Redone."));
    let list = fx_list(&mut server);
    assert_eq!(list["count"], json!(1), "fx list: {list}");
    assert_eq!(list["clips"][0]["kind"], json!("burst"), "fx list: {list}");
    assert_eq!(list["clips"][0]["duration"], json!(1.5), "fx list: {list}");
}
