//! Integration tests: the FX-clip lifecycle, driven end to end
//! through the real MCP dispatch (`McpServer::handle`) — Wave 11,
//! squad D, session "FX clips lifecycle, MCP only".
//!
//! Every mutation in this file arrives as a JSON-RPC
//! `tools/call` message and every assertion reads a JSON-RPC
//! reply: no `EditorCore` op is called directly and no project
//! field is written (or read) behind the server's back. That is
//! the point of the suite — it proves the surface an MCP client
//! actually sees, and where that surface is thinner than the
//! panel's, the test says so instead of routing around it.
//!
//! Gaps this suite asserted against the CORRECT behaviour
//! while they were open — all closed by the Wave 11 splice,
//! with the assertions below unchanged (they were written
//! for the fixed surface):
//!
//! - `fx_list` serialises the full FxInfo: colour, gradient
//!   end, seed, angle, length AND the hidden flag (B5 S1).
//! - `fx_set_hidden` and `fx_preset_list` are registered
//!   (fx_ops handlers, spliced in Wave 11).
//! - `add_fx_clip` refuses a start at or past `frame_count`,
//!   panel parity (such a clip is never sampled by any
//!   frame; see `nudge_clip_start` in `rune_sec/fx_clips.rs`).
//!
//! One naming inconsistency is pinned as-is, not fixed: the
//! add/preset replies report a clip's kind in Debug form
//! ("Burst") while `fx_list` uses snake_case ("burst") —
//! documented in docs/MCP.md.

use serde_json::{json, Value};
use stykanimate_rs::mcp::McpServer;

/// One MCP session: a server plus the JSON-RPC id counter.
/// The server's wrapped core is the session's project; the
/// only way in or out is `handle`.
struct Session {
    server: McpServer,
    next_id: i64,
}

impl Session {
    fn new() -> Self {
        Session {
            server: McpServer::new(),
            next_id: 1,
        }
    }

    /// Send one JSON-RPC request and return the response,
    /// asserting the envelope (version, matching id, and that
    /// a request — never a notification — is answered).
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let response = self
            .server
            .handle(&request)
            .expect("a request with an id must be answered");
        assert_eq!(response["jsonrpc"], "2.0", "{method}: bad envelope");
        assert_eq!(response["id"], id, "{method}: response id mismatch");
        response
    }

    /// Call a tool that must succeed and return its payload:
    /// the text content block parsed back into JSON (handlers
    /// return values; the router serialises them into text).
    fn call_ok(&mut self, name: &str, args: Value) -> Value {
        let response = self.rpc("tools/call", json!({ "name": name, "arguments": args }));
        assert!(
            response.get("error").is_none(),
            "{name}: unexpected protocol error: {}",
            response["error"]
        );
        let result = &response["result"];
        assert_eq!(
            result["isError"], false,
            "{name}: tool reported an error: {}",
            result["content"][0]["text"]
        );
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tool result text must be a string");
        serde_json::from_str(text).unwrap_or_else(|e| {
            panic!("{name}: tool result text is not the JSON payload expected: {e}: {text}")
        })
    }

    /// Call a tool that must FAIL at the tool level
    /// (`isError: true`, the handler's message as the text) and
    /// return that message. A protocol-level error object
    /// instead of an isError result is a different failure
    /// shape and fails the test.
    fn call_tool_error(&mut self, name: &str, args: Value) -> String {
        let response = self.rpc("tools/call", json!({ "name": name, "arguments": args }));
        assert!(
            response.get("error").is_none(),
            "{name}: expected a tool-level error, got a protocol error: {}",
            response["error"]
        );
        let result = &response["result"];
        assert_eq!(
            result["isError"], true,
            "{name}: expected the tool to refuse, got success: {}",
            result["content"][0]["text"]
        );
        result["content"][0]["text"]
            .as_str()
            .expect("tool error text must be a string")
            .to_string()
    }

    /// The registered tool names, via `tools/list`.
    fn tool_names(&mut self) -> Vec<String> {
        let response = self.rpc("tools/list", json!({}));
        response["result"]["tools"]
            .as_array()
            .expect("tools/list must return a tools array")
            .iter()
            .map(|t| {
                t["name"]
                    .as_str()
                    .expect("every tool entry has a name")
                    .to_string()
            })
            .collect()
    }

    /// The `fx_list` payload: `{ ok, count, clips }`.
    fn fx_list(&mut self) -> Value {
        self.call_ok("fx_list", json!({}))
    }

    /// Just the `clips` array of an `fx_list` payload.
    fn clips(&mut self) -> Vec<Value> {
        self.fx_list()["clips"]
            .as_array()
            .expect("fx_list payload must carry a clips array")
            .clone()
    }

    /// A fresh project with `frames` authored frames, built
    /// the way a client would: `new_project` (one frame) plus
    /// `add_frame` calls. Asserts the frame counts the replies
    /// report, so the session's project end is known exactly.
    fn new_project_with_frames(&mut self, frames: usize) {
        let r = self.call_ok(
            "new_project",
            json!({ "name": "Wave 11 FX E2E", "fps": 12, "width": 960, "height": 720 }),
        );
        assert_eq!(r["ok"], true);
        assert_eq!(r["project"]["name"], "Wave 11 FX E2E");
        assert_eq!(r["project"]["figures"], 1, "new project has the one figure");
        assert_eq!(r["project"]["frames"], 1, "new project starts with one frame");
        for want in 2..=frames {
            let r = self.call_ok("add_frame", json!({}));
            assert_eq!(
                r["frames"].as_u64(),
                Some(want as u64),
                "add_frame must report the new count"
            );
        }
    }
}

/// Float assertion for JSON numbers (clip times are f32 on the
/// engine side, f64 in JSON).
fn assert_num(actual: &Value, expected: f32, what: &str) {
    let got = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{what}: expected a number, got {actual}"));
    assert!(
        (got - expected as f64).abs() < 1e-6,
        "{what}: expected {expected}, got {got}"
    );
}

/// Assert every field of one `fx_list` row that the entering
/// reply shape carries: identity (index), the figure/node
/// binding, the snake-case kind, and the window
/// (start / duration / end).
fn assert_clip_row(
    clip: &Value,
    index: usize,
    figure: usize,
    node: usize,
    kind: &str,
    start: f32,
    duration: f32,
) {
    assert_eq!(clip["index"], index, "row {index}: index field");
    assert_eq!(clip["figure"], figure, "row {index}: figure binding");
    assert_eq!(clip["node"], node, "row {index}: node binding");
    assert_eq!(clip["kind"], kind, "row {index}: kind");
    assert_num(&clip["start"], start, "row start");
    assert_num(&clip["duration"], duration, "row duration");
    assert_num(&clip["end"], start + duration, "row end (= start + duration)");
}

#[test]
fn fx_clips_full_lifecycle_through_mcp_dispatch() {
    let mut s = Session::new();

    // Handshake: the server identifies itself before any tool
    // runs, exactly as a stdio or live-control client sees it.
    let init = s.rpc("initialize", json!({}));
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");

    // The whole FX lifecycle surface is registered.
    let names = s.tool_names();
    for want in [
        "add_fx_clip",
        "fx_list",
        "fx_move",
        "fx_resize",
        "fx_duplicate",
        "fx_delete",
        "fx_split_clip",
        "fx_set_color",
        "fx_set_color2",
        "fx_set_kind",
        "fx_set_anchor",
        "fx_preset_save",
        "fx_preset_add",
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "tools/list must register {want}"
        );
    }

    // A 24-frame project: every clip start used below is a
    // real, sampled frame coordinate inside it.
    s.new_project_with_frames(24);
    assert_eq!(s.fx_list()["count"], 0, "a new project has no FX clips");

    // --- Add three clips of different kinds ----------------
    // Note the reply shape: `kind` comes back in Debug form
    // ("Burst") while fx_list below reports snake_case
    // ("burst") — and the reply carries color2 but NOT the
    // primary color. The assertions pin what is actually
    // there; the report flags the inconsistency.
    let r = s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": 0.0, "duration": 4.0,
            "color": 0xFF0000, "color2": 0x0000FF, "length": 150.0,
        }),
    );
    assert_eq!(r["ok"], true);
    assert_eq!(r["fx_index"], 0);
    assert_eq!(r["kind"], "Burst", "add reply names the kind in Debug form");
    assert_eq!(r["figure"], 0);
    assert_eq!(r["node"], 0);
    assert_num(&r["start"], 0.0, "add reply start");
    assert_num(&r["duration"], 4.0, "add reply duration");
    assert_eq!(r["color2"], 0x0000FF, "add reply echoes the gradient end");

    let r = s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 2, "kind": "sparkle",
            "start": 6.0, "duration": 3.0,
        }),
    );
    assert_eq!(r["fx_index"], 1);
    assert_eq!(r["kind"], "Sparkle");

    let r = s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 6, "kind": "lightning",
            "start": 12.0, "duration": 5.0, "color": 0x33CCFF,
        }),
    );
    assert_eq!(r["fx_index"], 2);
    assert_eq!(r["kind"], "Lightning");

    // --- fx_list: the panel's view of the same clips -------
    let list = s.fx_list();
    assert_eq!(list["ok"], true);
    assert_eq!(list["count"], 3);
    let clips = s.clips();
    assert_clip_row(&clips[0], 0, 0, 0, "burst", 0.0, 4.0);
    assert_clip_row(&clips[1], 1, 0, 2, "sparkle", 6.0, 3.0);
    assert_clip_row(&clips[2], 2, 0, 6, "lightning", 12.0, 5.0);

    // --- Move and resize ------------------------------------
    let r = s.call_ok("fx_move", json!({ "index": 1, "start": 8.0 }));
    assert_eq!(r["fx_index"], 1);
    assert_num(&r["start"], 8.0, "fx_move reply start");
    let r = s.call_ok("fx_resize", json!({ "index": 2, "duration": 7.0 }));
    assert_eq!(r["fx_index"], 2);
    assert_num(&r["duration"], 7.0, "fx_resize reply duration");

    // A colour change and a re-anchor report through their
    // own replies (fx_list cannot show the colour — see the
    // gap test below — but the anchor IS a list field).
    let r = s.call_ok("fx_set_color", json!({ "index": 2, "color": 0xFFCC33 }));
    assert_eq!(r["color"], 0xFFCC33, "fx_set_color reply echoes the colour");
    let r = s.call_ok(
        "fx_set_anchor",
        json!({ "index": 2, "figure": 0, "node": 4 }),
    );
    assert_eq!(r["figure"], 0);
    assert_eq!(r["node"], 4);

    let clips = s.clips();
    assert_clip_row(&clips[0], 0, 0, 0, "burst", 0.0, 4.0);
    assert_clip_row(&clips[1], 1, 0, 2, "sparkle", 8.0, 3.0);
    assert_clip_row(&clips[2], 2, 0, 4, "lightning", 12.0, 7.0);

    // --- Duplicate: copy right after, in list and in time --
    let r = s.call_ok("fx_duplicate", json!({ "index": 0 }));
    assert_eq!(r["fx_index"], 1, "the copy is inserted right after the original");
    assert_eq!(r["duplicated_from"], 0);
    assert_num(&r["start"], 4.0, "the copy starts where the original ends");
    let clips = s.clips();
    assert_eq!(clips.len(), 4);
    assert_clip_row(&clips[0], 0, 0, 0, "burst", 0.0, 4.0);
    assert_clip_row(&clips[1], 1, 0, 0, "burst", 4.0, 4.0);
    assert_clip_row(&clips[2], 2, 0, 2, "sparkle", 8.0, 3.0);
    assert_clip_row(&clips[3], 3, 0, 4, "lightning", 12.0, 7.0);

    // --- Split the sparkle at frame 10 ----------------------
    let r = s.call_ok("fx_split_clip", json!({ "index": 2, "at_frame": 10.0 }));
    assert_eq!(r["ok"], true);
    assert_eq!(r["fx_index"], 2);
    assert!(
        r["status"].as_str().unwrap_or("").contains("split"),
        "split reply carries the op's status line: {}",
        r["status"]
    );
    let clips = s.clips();
    assert_eq!(clips.len(), 5, "a split adds exactly one clip");
    assert_clip_row(&clips[2], 2, 0, 2, "sparkle", 8.0, 2.0);
    assert_clip_row(&clips[3], 3, 0, 2, "sparkle", 10.0, 1.0);
    // The neighbours are untouched by the split.
    assert_clip_row(&clips[1], 1, 0, 0, "burst", 4.0, 4.0);
    assert_clip_row(&clips[4], 4, 0, 4, "lightning", 12.0, 7.0);

    // --- Delete the middle clip -----------------------------
    // Index 2 is the split's first half. The panel's delete
    // removes exactly that clip; everything after it shifts
    // down one index with its identity (kind/window/anchor)
    // intact.
    let r = s.call_ok("fx_delete", json!({ "index": 2 }));
    assert_eq!(r["deleted"], 2);
    assert_eq!(r["count"], 4);
    let clips = s.clips();
    assert_eq!(clips.len(), 4);
    assert_clip_row(&clips[0], 0, 0, 0, "burst", 0.0, 4.0);
    assert_clip_row(&clips[1], 1, 0, 0, "burst", 4.0, 4.0);
    assert_clip_row(&clips[2], 2, 0, 2, "sparkle", 10.0, 1.0);
    assert_clip_row(&clips[3], 3, 0, 4, "lightning", 12.0, 7.0);
}

#[test]
fn fx_list_reports_the_panel_visible_fields() {
    // A panel user looking at a clip row sees its kind,
    // window, anchor, colours, length and whether it is
    // hidden. fx_edit::FxInfo carries every one of those
    // fields; this test asserts the fx_list REPLY carries
    // them too. The entering tool serialises only
    // index/figure/node/kind/start/duration/end, so the
    // render-field and hidden assertions below fail until
    // the reply is widened to match FxInfo — that failure
    // is the gap report, not a test bug.
    let mut s = Session::new();
    s.new_project_with_frames(1);

    let r = s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "beam",
            "start": 0.0, "duration": 4.0,
            "color": 0xFF5D5D, "color2": 0x5DB2FF, "length": 200.0,
        }),
    );
    assert_eq!(r["fx_index"], 0);

    let clips = s.clips();
    assert_eq!(clips.len(), 1);
    let clip = &clips[0];
    // The fields the entering reply does carry.
    assert_clip_row(clip, 0, 0, 0, "beam", 0.0, 4.0);
    // The fields FxInfo carries and the panel shows, which
    // the reply must report with the values set at creation:
    // the primary colour and gradient end (settable via MCP
    // since long before Wave 11 — fx_set_color /
    // fx_set_color2 — but never readable back), the seed
    // (deterministic at creation: 11 + index * 7), the
    // angle (0 at creation) and the length.
    assert_eq!(
        clip["color"], 0xFF5D5D,
        "fx_list must report the clip's colour (FxInfo.color)"
    );
    assert_eq!(
        clip["color2"], 0x5DB2FF,
        "fx_list must report the gradient end colour (FxInfo.color2)"
    );
    assert_eq!(
        clip["seed"], 11,
        "fx_list must report the clip's seed (FxInfo.seed; first clip seeds at 11)"
    );
    assert_num(&clip["angle"], 0.0, "fx_list angle (FxInfo.angle)");
    assert_num(&clip["length"], 200.0, "fx_list length (FxInfo.length)");
    // FxInfo.hidden (landed in Wave 11):
    assert_eq!(
        clip["hidden"], false,
        "fx_list must report the clip's hidden flag (FxInfo.hidden): \
         a fresh clip is not hidden"
    );
}

#[test]
fn fx_hidden_toggle_round_trips_through_mcp() {
    // The panel's Hide/Show toggle sets FxClipEntry::hidden;
    // MCP can do the same and read it back — both halves are
    // Wave 11 surfaces (fx_set_hidden registered by the
    // splice; fx_list serialises the flag since B5 S1).
    let mut s = Session::new();

    let names = s.tool_names();
    // fx_set_hidden is registered (Wave 11 splice):
    assert!(
        names.iter().any(|n| n == "fx_set_hidden"),
        "tools/list must register fx_set_hidden (handler: fx_ops::tool_fx_set_hidden)"
    );
    // fx_preset_list is registered (Wave 11 splice):
    assert!(
        names.iter().any(|n| n == "fx_preset_list"),
        "tools/list must register fx_preset_list (handler: fx_ops::tool_fx_preset_list)"
    );

    s.new_project_with_frames(1);
    s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "aura",
            "start": 0.0, "duration": 4.0,
        }),
    );

    // Hide it: the reply reads the flag back from the
    // project, and fx_list must agree.
    let r = s.call_ok("fx_set_hidden", json!({ "index": 0, "hidden": true }));
    assert_eq!(r["ok"], true);
    assert_eq!(r["fx_index"], 0);
    assert_eq!(r["hidden"], true, "fx_set_hidden reply reads the flag back");
    let clips = s.clips();
    // FxInfo.hidden (landed in Wave 11):
    assert_eq!(
        clips[0]["hidden"], true,
        "fx_list must show the clip as hidden after fx_set_hidden"
    );

    // Show it again: hiding is not deleting — the clip is
    // still listed, still editable, flag cleared.
    let r = s.call_ok("fx_set_hidden", json!({ "index": 0, "hidden": false }));
    assert_eq!(r["hidden"], false);
    let clips = s.clips();
    assert_eq!(clips.len(), 1, "a hidden-then-shown clip stays in the list");
    // FxInfo.hidden (landed in Wave 11):
    assert_eq!(
        clips[0]["hidden"], false,
        "fx_list must show the clip as visible again"
    );

    // The preset catalogue is listable through MCP too
    // (read-only; its contents depend on the working
    // directory, so only the reply's shape is pinned).
    let r = s.call_ok("fx_preset_list", json!({}));
    assert_eq!(r["ok"], true);
    let presets = r["presets"]
        .as_array()
        .expect("fx_preset_list must return a presets array");
    assert_eq!(
        r["count"].as_u64(),
        Some(presets.len() as u64),
        "fx_preset_list count must match its presets array"
    );
}

#[test]
fn fx_preset_flow_never_overwrites_the_first_preset() {
    // Wave 10's rule, proven through MCP only: saving a
    // DIFFERENT clip under a name that already holds a
    // preset must not destroy the first preset — the new
    // one takes the next free suffix (<name>-2). Loading
    // the original name back must return the FIRST clip,
    // and the suffixed name the second.
    //
    // Presets live in ./fx-presets/ relative to the working
    // directory (the crate's documented convention), so the
    // preset name is made unique to this test process and
    // its files are removed before and after.
    let name = format!("wave11-e2e-{}-sparkle-1", std::process::id());
    let dir = std::path::Path::new("fx-presets");
    let first_file = dir.join(format!("{name}.json"));
    let second_file = dir.join(format!("{name}-2.json"));
    let _ = std::fs::remove_file(&first_file);
    let _ = std::fs::remove_file(&second_file);

    let mut s = Session::new();
    s.new_project_with_frames(24);

    // The clip that becomes the first preset: a 6-frame
    // sparkle.
    s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "sparkle",
            "start": 2.0, "duration": 6.0, "color": 0xFFC65D,
        }),
    );
    let r = s.call_ok("fx_preset_save", json!({ "index": 0, "name": name }));
    assert_eq!(r["ok"], true);
    assert_eq!(r["fx_index"], 0);
    assert_eq!(r["preset"], name, "the reply names the requested preset");
    assert_eq!(r["kind"], "Sparkle");

    // Change the clip under the same session: another kind,
    // another duration. Saving THAT under the same
    // auto-name is the collision Wave 10 fixed.
    s.call_ok("fx_set_kind", json!({ "index": 0, "kind": "galaxy" }));
    s.call_ok("fx_resize", json!({ "index": 0, "duration": 9.0 }));
    let r = s.call_ok("fx_preset_save", json!({ "index": 0, "name": name }));
    assert_eq!(r["ok"], true, "the colliding save still succeeds (suffixed)");
    // Reply-shape note: the reply reports only the
    // REQUESTED name — it never discloses that the clip
    // actually landed in <name>-2. The proof below is
    // therefore behavioural: load both names back.

    // Loading the original name returns the FIRST preset:
    // sparkle, 6 frames — the galaxy/9 save did not
    // overwrite it.
    let r = s.call_ok("fx_preset_add", json!({ "name": name, "frame": 20.0 }));
    assert_eq!(r["fx_index"], 1);
    assert_eq!(r["kind"], "Sparkle", "the first preset keeps its kind");
    let clips = s.clips();
    assert_clip_row(&clips[1], 1, 0, 0, "sparkle", 20.0, 6.0);

    // And the second save is intact under the suffix:
    // galaxy, 9 frames.
    let suffixed = format!("{name}-2");
    let r = s.call_ok("fx_preset_add", json!({ "name": suffixed, "frame": 22.0 }));
    assert_eq!(r["fx_index"], 2);
    assert_eq!(r["kind"], "Galaxy", "the suffixed preset holds the second clip");
    let clips = s.clips();
    assert_clip_row(&clips[2], 2, 0, 0, "galaxy", 22.0, 9.0);

    let _ = std::fs::remove_file(&first_file);
    let _ = std::fs::remove_file(&second_file);
}

#[test]
fn fx_negative_cases_are_refused_and_change_nothing() {
    let mut s = Session::new();
    // Five frames: frame_count 5, the last authored frame
    // index is 4 — the project end the panel guards.
    s.new_project_with_frames(5);
    s.call_ok(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": 1.0, "duration": 2.0,
        }),
    );
    // The whole fx_list payload is the project's FX state
    // as a client sees it; every refusal below must leave
    // it byte-for-byte the same.
    let before = s.fx_list();
    assert_eq!(before["count"], 1);

    // Bad kind string: refused, naming the problem.
    let msg = s.call_tool_error(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "nova",
            "start": 0.0, "duration": 2.0,
        }),
    );
    assert!(
        msg.contains("unknown fx kind"),
        "bad kind must be refused as an unknown kind: {msg}"
    );
    assert_eq!(s.fx_list(), before, "bad kind changed the project");

    // Zero duration: refused (a clip must cover something).
    let msg = s.call_tool_error(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": 0.0, "duration": 0.0,
        }),
    );
    assert!(
        msg.contains("duration"),
        "zero duration must be refused as a duration problem: {msg}"
    );
    assert_eq!(s.fx_list(), before, "zero duration changed the project");

    // Negative duration: refused the same way.
    let msg = s.call_tool_error(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": 0.0, "duration": -2.5,
        }),
    );
    assert!(
        msg.contains("duration"),
        "negative duration must be refused as a duration problem: {msg}"
    );
    assert_eq!(s.fx_list(), before, "negative duration changed the project");

    // Negative start: refused (starts clamp at frame 0 in
    // the panel; creation refuses outright).
    let msg = s.call_tool_error(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": -1.0, "duration": 2.0,
        }),
    );
    assert!(
        msg.contains("start"),
        "negative start must be refused as a start problem: {msg}"
    );
    assert_eq!(s.fx_list(), before, "negative start changed the project");

    // add_fx_clip project end (refusal landed in Wave 11):
    // A start AT the project end (start == frame_count) is
    // past the last authored frame: no frame ever samples
    // the clip (fx_edit::clips_at needs frame_f >= start
    // for a frame_f below frame_count) and the panel's
    // steppers refuse to land a start there — and now the
    // tool refuses too, panel parity.
    let msg = s.call_tool_error(
        "add_fx_clip",
        json!({
            "figure": 0, "node": 0, "kind": "burst",
            "start": 5.0, "duration": 2.0,
        }),
    );
    assert!(
        msg.contains("start"),
        "a start at the project end must be refused as a start problem: {msg}"
    );
    assert_eq!(
        s.fx_list(),
        before,
        "a start past the project end changed the project"
    );
}
