//! Wave P, Squad 4 (agent P41) — the undo-contract SAMPLE audit.
//!
//! The standing contract (Wave 11, carried in every wave since):
//! every MUTATING MCP tool spends exactly ONE undo step (one
//! `EditorCore::snapshot` on its path); READ-ONLY tools spend
//! none; REFUSALS (an `isError` tool result) spend none and
//! change no state. This suite pins that contract for a SAMPLE
//! of three tool families, driven end-to-end through the real
//! MCP dispatch (`McpServer::handle`, the test-host idiom of
//! tests/wave14_e2e_clip_media_mcp.rs), with the undo depth
//! observed through `get_editor_status` — the only witness a
//! client has.
//!
//! COVERAGE — read this before trusting the suite: the registry
//! holds 370 tools (docs/MCP.md). This sample pins 14 of them
//! (~4%): five figure tools, five particle tools, four camera
//! tools, plus `get_editor_status` as the meter. It is a SAMPLE,
//! not a census: a green run proves the contract for these 14
//! tools only, and says nothing about the other 356.
//!
//! Sampled, per family (the board's classes):
//!   figures   — add: `add_figure`; update: `figure_recolor`;
//!               reorder: `move_figure` (a real reorder spends
//!               one; a clamped no-op move reports `moved: false`
//!               and spends none — the house "no phantom undo
//!               step" convention); read: `list_figures`;
//!               refusal: `delete_figure` on the last figure.
//!   particles — add: `particle_add_event`; set:
//!               `particle_set_kind`, `particle_set_color`;
//!               delete: `particle_delete_event`; read:
//!               `particle_list_events`; refusal:
//!               `particle_delete_event` at a bad index.
//!   camera    — key set: `set_camera`, `set_camera_key_here`;
//!               key delete: `delete_camera_key`; read:
//!               `get_camera`; refusal: `delete_camera_key` on a
//!               frame holding no key.
//!
//! Deliberately NOT pinned: `figure_set_color`. By static trace
//! it mutates the project but spends ZERO undo steps (handler
//! src/mcp_tools.rs `tool_figure_set_color` →
//! `EditorCore::set_figure_color`, src/figure_ops.rs, whose only
//! history effect is `mark_dirty`). The op's own doc calls the
//! zero-step write deliberate, but — unlike every sanctioned
//! panel-parity exception on this tree (`figure_rename`,
//! `figure_step_opacity`, `clear_camera_key`, …, each of which
//! declares "not an undo step" in its tool description) —
//! `figure_set_color`'s description declares nothing, so a
//! client cannot tell it apart from the one-step class. That is
//! FINDING F1 in ~/workspace/wave-p/findings/p41.md, left for
//! the integrator to disposition (declare the exception, or
//! take the snapshot); this suite asserts neither delta for it.
//! The declared exceptions `figure_rename` (figures) and
//! `clear_camera_key` (camera) are likewise outside the sample.
//!
//! DISPOSITION (Roadmap Wave 1, RM24, P-14 — audit A15 H3):
//! F1 was resolved by taking the snapshot: `figure_set_color`
//! now spends exactly one (coalesced) undo step, its tool
//! description says so, and the whole declared-exception
//! class — `figure_rename`, `clear_camera_key` and the rest
//! of the Wave 13 census's 21 — was promoted with it; the
//! full-surface proof lives in tests/wave_r_undo_census.rs
//! (MUTATING 232, EXCEPTION 0). This sample's scope is
//! unchanged: it still asserts no delta for those tools.

use serde_json::{json, Value};

use stykanimate_rs::mcp::McpServer;

// ---------------------------------------------------------------------
// The dispatch driver (the Wave 14 pattern, trimmed to this audit)
// ---------------------------------------------------------------------

/// One MCP `tools/call`, decoded the way a client decodes it:
/// the JSON document in the single text content block, plus the
/// protocol's error flag. A protocol-level error response is
/// folded into the same shape with its message as the text.
struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

/// A small dispatch driver over one `McpServer`: sequential
/// request ids and JSON-RPC envelope assertions on every reply.
struct Dispatch {
    server: McpServer,
    next_id: u64,
}

impl Dispatch {
    fn new() -> Self {
        Dispatch {
            server: McpServer::new(),
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let response = self
            .server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .expect("a request with an id must be answered");
        assert_eq!(response["jsonrpc"], "2.0", "{method}: bad envelope");
        assert_eq!(response["id"], json!(id), "{method}: id not echoed");
        response
    }

    fn call(&mut self, name: &str, args: Value) -> CallOutcome {
        let response = self.request(
            "tools/call",
            json!({ "name": name, "arguments": args }),
        );
        if let Some(error) = response.get("error") {
            return CallOutcome {
                payload: Value::Null,
                text: error["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                is_error: true,
            };
        }
        let result = &response["result"];
        let is_error = result["isError"]
            .as_bool()
            .expect("tools/call result carries isError");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tools/call result carries one text block")
            .to_string();
        let payload =
            serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.clone()));
        CallOutcome {
            payload,
            text,
            is_error,
        }
    }

    fn call_ok(&mut self, name: &str, args: Value) -> Value {
        let outcome = self.call(name, args);
        assert!(
            !outcome.is_error,
            "tool {name} failed: {}",
            outcome.text
        );
        outcome.payload
    }

    fn call_err(&mut self, name: &str, args: Value) -> String {
        let outcome = self.call(name, args);
        assert!(
            outcome.is_error,
            "tool {name} unexpectedly succeeded: {}",
            outcome.text
        );
        outcome.text
    }
}

// ---------------------------------------------------------------------
// The meter
// ---------------------------------------------------------------------

/// The session's undo depth — the observable edit history, and
/// so the witness for the one-undo-step contract.
fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

/// Assert the depth moved by exactly `delta` since `before`.
fn assert_delta(mcp: &mut Dispatch, before: u64, delta: u64, what: &str) {
    assert_eq!(
        undo_depth(mcp),
        before + delta,
        "{what}: undo depth must move by exactly {delta}"
    );
}

/// A fresh session on a fresh project (one default humanoid,
/// one frame), with the baseline depth read AFTER the project
/// swap — project-swap tools sit outside undo accounting by
/// standing exclusion, so the swap itself is never measured.
fn fresh_session() -> (Dispatch, u64) {
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({}));
    let baseline = undo_depth(&mut mcp);
    (mcp, baseline)
}

// ---------------------------------------------------------------------
// Figures family
// ---------------------------------------------------------------------

#[test]
fn figures_family_undo_contract() {
    let (mut mcp, d0) = fresh_session();

    // Read-only: list_figures spends nothing.
    let figures = mcp.call_ok("list_figures", json!({}));
    assert_eq!(figures["figures"].as_array().map(Vec::len), Some(1));
    assert_delta(&mut mcp, d0, 0, "list_figures (read-only)");

    // Refusal: the last figure cannot be deleted — an isError
    // result that spends nothing (the op's guard at
    // src/editor.rs `delete_figure` precedes its snapshot).
    let err = mcp.call_err("delete_figure", json!({ "figure": 0 }));
    assert!(
        err.contains("at least one figure"),
        "delete_figure refusal names its reason: {err}"
    );
    assert_delta(&mut mcp, d0, 0, "delete_figure refusal (last figure)");

    // Add class: add_figure spends exactly one step
    // (EditorCore::add_figure snapshots once, src/editor.rs).
    let added = mcp.call_ok("add_figure", json!({ "kind": "humanoid" }));
    assert_eq!(added["figure"], json!(1), "the new figure is index 1");
    assert_delta(&mut mcp, d0, 1, "add_figure");

    // Update class: figure_recolor spends exactly one step
    // (recolor_figure snapshots once, src/figure_ops.rs).
    let d = undo_depth(&mut mcp);
    mcp.call_ok(
        "figure_recolor",
        json!({ "figure": 1, "color": 0x224466 }),
    );
    assert_delta(&mut mcp, d, 1, "figure_recolor");

    // Reorder class: a real move spends exactly one step
    // (move_figure_full snapshots once, src/editor.rs).
    let d = undo_depth(&mut mcp);
    let moved = mcp.call_ok(
        "move_figure",
        json!({ "figure": 0, "direction": 1 }),
    );
    assert_eq!(moved["moved"], json!(true), "figure 0 really reordered");
    assert_delta(&mut mcp, d, 1, "move_figure (real reorder)");

    // A clamped move is a successful NO-OP: it reports
    // moved: false and spends nothing — no phantom undo step
    // for a change that never happened.
    let d = undo_depth(&mut mcp);
    let clamped = mcp.call_ok(
        "move_figure",
        json!({ "figure": 1, "direction": 1 }),
    );
    assert_eq!(
        clamped["moved"],
        json!(false),
        "figure 1 is already last; the move clamps to a no-op"
    );
    assert_delta(&mut mcp, d, 0, "move_figure (clamped no-op)");
}

// ---------------------------------------------------------------------
// Particles family
// ---------------------------------------------------------------------

#[test]
fn particles_family_undo_contract() {
    let (mut mcp, d0) = fresh_session();

    // Read-only: particle_list_events spends nothing.
    let listed = mcp.call_ok("particle_list_events", json!({}));
    assert_eq!(listed["count"], json!(0), "a fresh project has no events");
    assert_delta(&mut mcp, d0, 0, "particle_list_events (read-only)");

    // Refusal: deleting an event that does not exist is an
    // isError result that spends nothing (the handler's index
    // check errors before the op is reached).
    mcp.call_err("particle_delete_event", json!({ "index": 0 }));
    assert_delta(&mut mcp, d0, 0, "particle_delete_event refusal (bad index)");

    // Add class: particle_add_event spends exactly one step
    // (the op snapshots once, src/particle_ops.rs).
    let d = undo_depth(&mut mcp);
    let added = mcp.call_ok(
        "particle_add_event",
        json!({ "frame": 0, "x": 100.0, "y": 120.0 }),
    );
    assert_eq!(added["index"], json!(0), "the new event is index 0");
    assert_delta(&mut mcp, d, 1, "particle_add_event");

    // Set class: each setter spends exactly one step (the ops
    // guard the index, then snapshot once, src/particle_ops.rs).
    let d = undo_depth(&mut mcp);
    mcp.call_ok(
        "particle_set_kind",
        json!({ "index": 0, "kind": "sparkle" }),
    );
    assert_delta(&mut mcp, d, 1, "particle_set_kind");

    let d = undo_depth(&mut mcp);
    mcp.call_ok(
        "particle_set_color",
        json!({ "index": 0, "color": 0xFF8800 }),
    );
    assert_delta(&mut mcp, d, 1, "particle_set_color");

    // Delete class: particle_delete_event spends exactly one.
    let d = undo_depth(&mut mcp);
    mcp.call_ok("particle_delete_event", json!({ "index": 0 }));
    assert_delta(&mut mcp, d, 1, "particle_delete_event");
    let listed = mcp.call_ok("particle_list_events", json!({}));
    assert_eq!(listed["count"], json!(0), "the delete really landed");
}

// ---------------------------------------------------------------------
// Camera family
// ---------------------------------------------------------------------

#[test]
fn camera_family_undo_contract() {
    let (mut mcp, d0) = fresh_session();

    // Read-only: get_camera spends nothing.
    let cam = mcp.call_ok("get_camera", json!({}));
    assert_eq!(
        cam["keys"].as_array().map(Vec::len),
        Some(0),
        "a fresh project has no camera keys"
    );
    assert_delta(&mut mcp, d0, 0, "get_camera (read-only)");

    // Refusal: deleting a key from a frame that holds none is
    // an isError result that spends nothing (the handler at
    // src/mcp.rs `tool_delete_camera_key` returns before its
    // snapshot when no key matches).
    let err = mcp.call_err("delete_camera_key", json!({ "frame": 0 }));
    assert!(
        err.contains("No camera key"),
        "delete_camera_key refusal names its reason: {err}"
    );
    assert_delta(&mut mcp, d0, 0, "delete_camera_key refusal (no key)");

    // Key-set class: set_camera spends exactly one step (the
    // handler snapshots once, src/mcp_tools.rs `tool_set_camera`).
    let d = undo_depth(&mut mcp);
    mcp.call_ok(
        "set_camera",
        json!({ "frame": 0, "x": 12.0, "y": 34.0, "zoom": 1.25 }),
    );
    assert_delta(&mut mcp, d, 1, "set_camera");

    // Key-set class: set_camera_key_here spends exactly one
    // step (it rides EditorCore::rotate_camera, which snapshots
    // once, src/editor.rs).
    let d = undo_depth(&mut mcp);
    mcp.call_ok("set_camera_key_here", json!({}));
    assert_delta(&mut mcp, d, 1, "set_camera_key_here");

    // Key-delete class: delete_camera_key spends exactly one
    // step (one explicit snapshot in the handler, src/mcp.rs).
    let d = undo_depth(&mut mcp);
    mcp.call_ok("delete_camera_key", json!({ "frame": 0 }));
    assert_delta(&mut mcp, d, 1, "delete_camera_key");
    let cam = mcp.call_ok("get_camera", json!({}));
    assert_eq!(
        cam["keys"].as_array().map(Vec::len),
        Some(0),
        "the delete really landed"
    );
}
