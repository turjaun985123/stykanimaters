//! Wave 11, Squad D (session D1) — integration tests: the
//! foundation session, "zero to exported image, MCP only".
//!
//! Every step of the session goes through the real MCP
//! dispatch (`McpServer::handle`, and `handle_on` for the
//! live-control path), with JSON-RPC messages built exactly
//! as a client would build them: `initialize`, the
//! `notifications/initialized` notification, `tools/list`,
//! then `tools/call` for everything else. State is read back
//! through MCP query tools wherever one exists — that is the
//! point of the exercise: the chain is only proven if a
//! client holding nothing but the protocol can observe each
//! effect. (`EditorCore` is never touched directly in the
//! session test; the one place a core is constructed by hand
//! is the `handle_on` test, where owning a second core IS
//! the scenario.)
//!
//! Reply-shape facts these tests pin (all read from the
//! dispatch source, `src/mcp.rs`): a `tools/call` success
//! wraps the handler's JSON value, serialised, in a single
//! text content block (`result.content[0].text`, to be
//! parsed as JSON) with `result.isError == false`; a
//! handler failure is the same envelope with `isError ==
//! true` and the error message as the text; an unknown tool
//! name is a protocol error instead (`error.code ==
//! -32602`, message `unknown tool: <name>`); an unknown
//! method is `-32601`; notifications are never answered.
//!
//! Gap convention: an assertion written against the CORRECT
//! behaviour that today's surface cannot satisfy carries a
//! `// WAVE11-GAP:` comment naming what is missing.

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;

// ---------------------------------------------------------------------
// A minimal MCP client: request ids, envelope checks, payload parsing.
// ---------------------------------------------------------------------

struct Client {
    next_id: u64,
}

impl Client {
    fn new() -> Self {
        Client { next_id: 0 }
    }

    fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// One JSON-RPC round trip through `McpServer::handle`.
    fn rpc(&mut self, server: &mut McpServer, method: &str, params: Value) -> Value {
        let id = self.alloc_id();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let reply = server
            .handle(&msg)
            .expect("a request with an id must be answered");
        assert_eq!(reply["jsonrpc"], "2.0", "reply must be JSON-RPC 2.0");
        assert_eq!(reply["id"].as_u64(), Some(id), "reply id must echo the request id");
        reply
    }

    /// The same round trip through the live-control path,
    /// `McpServer::handle_on`, against a host-owned core.
    fn rpc_on(
        &mut self,
        server: &mut McpServer,
        core: &mut EditorCore,
        method: &str,
        params: Value,
    ) -> Value {
        let id = self.alloc_id();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let reply = server
            .handle_on(core, &msg)
            .expect("a request with an id must be answered");
        assert_eq!(reply["jsonrpc"], "2.0", "reply must be JSON-RPC 2.0");
        assert_eq!(reply["id"].as_u64(), Some(id), "reply id must echo the request id");
        reply
    }

    /// `tools/call` — returns the raw reply so tests can
    /// assert on either the result or the error object.
    fn call_raw(&mut self, server: &mut McpServer, name: &str, args: Value) -> Value {
        self.rpc(
            server,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        )
    }

    /// `tools/call` on the live path — raw reply.
    fn call_raw_on(
        &mut self,
        server: &mut McpServer,
        core: &mut EditorCore,
        name: &str,
        args: Value,
    ) -> Value {
        self.rpc_on(
            server,
            core,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        )
    }

    /// Unwrap a `tools/call` reply that must be a tool
    /// result: checks the envelope, then parses the text
    /// content block as JSON. Returns (payload, is_error).
    fn unwrap_result(reply: &Value) -> (Value, bool) {
        let result = reply
            .get("result")
            .unwrap_or_else(|| panic!("expected a result, got: {reply}"));
        let is_error = result["isError"].as_bool().expect("isError flag present");
        let block = &result["content"][0];
        assert_eq!(block["type"], "text", "first content block must be text");
        let text = block["text"].as_str().expect("text block carries a string");
        if is_error {
            (json!({ "__error_text": text }), true)
        } else {
            let payload: Value = serde_json::from_str(text)
                .unwrap_or_else(|e| panic!("tool result text is not JSON ({e}): {text}"));
            (payload, false)
        }
    }

    /// A tool call that must succeed; returns its payload.
    fn call_ok(&mut self, server: &mut McpServer, name: &str, args: Value) -> Value {
        let reply = self.call_raw(server, name, args);
        let (payload, is_error) = Self::unwrap_result(&reply);
        assert!(
            !is_error,
            "tool {name} failed unexpectedly: {}",
            payload["__error_text"]
        );
        payload
    }

    /// A tool call that must succeed, on the live path.
    fn call_ok_on(
        &mut self,
        server: &mut McpServer,
        core: &mut EditorCore,
        name: &str,
        args: Value,
    ) -> Value {
        let reply = self.call_raw_on(server, core, name, args);
        let (payload, is_error) = Self::unwrap_result(&reply);
        assert!(
            !is_error,
            "tool {name} (live path) failed unexpectedly: {}",
            payload["__error_text"]
        );
        payload
    }

    /// A tool call that must fail as a tool error
    /// (`isError`), returning the handler's message.
    fn call_err(&mut self, server: &mut McpServer, name: &str, args: Value) -> String {
        let reply = self.call_raw(server, name, args);
        let (payload, is_error) = Self::unwrap_result(&reply);
        assert!(is_error, "tool {name} should have failed, got: {payload}");
        payload["__error_text"]
            .as_str()
            .expect("error payload carries the message")
            .to_string()
    }
}

// ---------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------

/// Per-process scratch directory (the `tests/` convention:
/// removed at the start, removed again by the caller).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave11-e2e-foundation-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn assert_f64(actual: &Value, expected: f64, what: &str) {
    let got = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{what}: expected a number, got {actual}"));
    assert!(
        (got - expected).abs() < 0.001,
        "{what}: expected {expected}, got {got}"
    );
}

/// Standard-alphabet base64 decoding (the mirror of the
/// encoder in `mcp_io`), so the base64 render payload from
/// `render_frame_base64` can be checked for real PNG bytes
/// rather than just for being a long string.
fn base64_decode(text: &str) -> Vec<u8> {
    fn value_of(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a') as u32 + 26),
            b'0'..=b'9' => Some((b - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut n: u32 = 0;
        let mut pad = 0;
        for (i, b) in chunk.iter().enumerate() {
            if *b == b'=' {
                pad += 1;
                continue;
            }
            let v = value_of(*b).expect("base64 payload uses the standard alphabet");
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    out
}

const WAVE_COLOR: u64 = 0x00FF_5D5D;
const STYK_COLOR: u64 = 0x00E8_E8EC;

// ---------------------------------------------------------------------
// The foundation session: zero to exported image, MCP only.
// ---------------------------------------------------------------------

#[test]
fn mcp_only_zero_to_exported_image() {
    let mut server = McpServer::new();
    let mut client = Client::new();

    // -- Handshake -------------------------------------------------
    let reply = client.rpc(
        &mut server,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "wave11-e2e-foundation", "version": "1.0" },
        }),
    );
    let init = &reply["result"];
    assert_eq!(init["protocolVersion"], "2024-11-05");
    assert_eq!(init["serverInfo"]["name"], "stykanimate-rs");
    assert!(init["serverInfo"]["version"].is_string());
    assert!(init["capabilities"]["tools"].is_object());

    // The follow-up notification must be swallowed silently.
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert!(
        server.handle(&note).is_none(),
        "notifications must never be answered"
    );

    // -- Tools list: everything this session needs is on it --------
    let reply = client.rpc(&mut server, "tools/list", json!({}));
    let tools = reply["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array");
    assert!(!tools.is_empty(), "the registry must not be empty");
    for tool in tools {
        assert!(tool["name"].is_string(), "every tool has a name");
        assert!(
            tool["inputSchema"].is_object(),
            "every tool publishes an input schema"
        );
    }
    for needed in [
        "new_project",
        "add_figure",
        "list_figures",
        "get_pose",
        "set_pose",
        "add_frame",
        "get_project_stats",
        "get_state",
        "project_summary",
        "render",
        "render_frame_base64",
    ] {
        assert!(
            tools.iter().any(|t| t["name"] == needed),
            "tools/list is missing {needed}, which this session needs"
        );
    }
    // server_info agrees with the list it ships in.
    let info = client.call_ok(&mut server, "server_info", json!({}));
    assert_eq!(info["name"], "stykanimate-rs");
    assert_eq!(info["tools"].as_u64(), Some(tools.len() as u64));

    // -- A fresh project --------------------------------------------
    let created = client.call_ok(
        &mut server,
        "new_project",
        json!({ "name": "Wave11 E2E", "fps": 12, "width": 960, "height": 720 }),
    );
    assert_eq!(created["ok"], true);
    let summary = &created["project"];
    assert_eq!(summary["name"], "Wave11 E2E");
    assert_f64(&summary["fps"], 12.0, "new_project fps");
    assert_eq!(summary["figures"].as_u64(), Some(1), "a new project starts with one figure");
    assert_eq!(summary["frames"].as_u64(), Some(1), "a new project starts with one frame");
    assert_eq!(summary["current_frame"].as_u64(), Some(0));
    let stage = summary["stage"].as_array().expect("stage is [w, h]");
    assert_f64(&stage[0], 960.0, "stage width");
    assert_f64(&stage[1], 720.0, "stage height");

    // -- Add a figure, with a colour of our choosing ----------------
    let added = client.call_ok(
        &mut server,
        "add_figure",
        json!({ "kind": "humanoid", "name": "Wave", "color": WAVE_COLOR }),
    );
    assert_eq!(added["ok"], true);
    assert_eq!(added["figure"].as_u64(), Some(1), "the second figure takes index 1");
    assert_eq!(added["name"], "Wave");
    assert_eq!(
        added["color"].as_u64(),
        Some(WAVE_COLOR),
        "add_figure echoes the colour it applied"
    );
    let node_count = added["nodes"].as_u64().expect("add_figure reports a node count");
    assert!(node_count > 1, "a humanoid has more than one node");

    // -- list_figures: the roster as a client sees it ---------------
    let list = client.call_ok(&mut server, "list_figures", json!({}));
    let figures = list["figures"].as_array().expect("figures is an array");
    assert_eq!(figures.len(), 2, "two figures after add_figure");
    assert_eq!(figures[0]["index"].as_u64(), Some(0));
    assert_eq!(figures[0]["name"], "Styk");
    assert_eq!(figures[0]["color"].as_u64(), Some(STYK_COLOR));
    assert_eq!(figures[1]["index"].as_u64(), Some(1));
    assert_eq!(figures[1]["name"], "Wave");
    assert_eq!(figures[1]["color"].as_u64(), Some(WAVE_COLOR));
    assert_eq!(
        figures[1]["nodes"].as_u64(),
        Some(node_count),
        "list_figures agrees with add_figure on the node count"
    );
    assert_eq!(figures[1]["hidden"], false);
    assert_eq!(figures[1]["locked"], false);

    // -- Pose frame 0: read, write, read back ------------------------
    let pose0 = client.call_ok(
        &mut server,
        "get_pose",
        json!({ "figure": 1, "frame": 0 }),
    );
    let angles0 = pose0["angles"].as_array().expect("pose carries angles");
    assert_eq!(
        angles0.len() as u64,
        node_count,
        "one angle per node in the pose"
    );
    let root0 = pose0["root"].as_array().expect("pose carries a root");
    assert_eq!(root0.len(), 2, "root is [x, y]");

    let written = client.call_ok(
        &mut server,
        "set_pose",
        json!({ "figure": 1, "frame": 0, "angles": { "1": 45.0 } }),
    );
    assert_eq!(written["ok"], true);
    assert_eq!(written["angles_changed"].as_u64(), Some(1));
    assert_f64(&written["pose"]["angles"][1], 45.0, "set_pose echoes the new angle");

    let moved = client.call_ok(
        &mut server,
        "set_pose",
        json!({ "figure": 1, "frame": 0, "root": [500.0, 400.0] }),
    );
    assert_f64(&moved["pose"]["root"][0], 500.0, "set_pose echoes the new root x");
    assert_f64(&moved["pose"]["root"][1], 400.0, "set_pose echoes the new root y");

    // Read back through get_pose: both writes stuck, together.
    let pose0_after = client.call_ok(
        &mut server,
        "get_pose",
        json!({ "figure": 1, "frame": 0 }),
    );
    assert_f64(&pose0_after["angles"][1], 45.0, "frame 0 angle persists");
    assert_f64(&pose0_after["root"][0], 500.0, "frame 0 root x persists");
    assert_f64(&pose0_after["root"][1], 400.0, "frame 0 root y persists");

    // -- A second frame, posed differently ---------------------------
    let framed = client.call_ok(&mut server, "add_frame", json!({}));
    assert_eq!(framed["ok"], true);
    assert_eq!(framed["frames"].as_u64(), Some(2));
    assert_eq!(
        framed["current_frame"].as_u64(),
        Some(1),
        "add_frame moves the playhead to the new frame"
    );

    let written1 = client.call_ok(
        &mut server,
        "set_pose",
        json!({ "figure": 1, "frame": 1, "angles": { "1": -30.0 } }),
    );
    assert_f64(&written1["pose"]["angles"][1], -30.0, "frame 1 takes its own angle");

    // Frame independence, proven in both directions through MCP.
    let pose1 = client.call_ok(
        &mut server,
        "get_pose",
        json!({ "figure": 1, "frame": 1 }),
    );
    assert_f64(&pose1["angles"][1], -30.0, "frame 1 angle reads back");
    let pose0_again = client.call_ok(
        &mut server,
        "get_pose",
        json!({ "figure": 1, "frame": 0 }),
    );
    assert_f64(
        &pose0_again["angles"][1],
        45.0,
        "posing frame 1 must not disturb frame 0"
    );

    // -- Whole-project reads: two independent summaries agree --------
    let stats = client.call_ok(&mut server, "get_project_stats", json!({}));
    assert_eq!(stats["ok"], true);
    assert_eq!(stats["figures"].as_u64(), Some(2));
    assert_eq!(stats["frames"].as_u64(), Some(2));
    assert_f64(&stats["fps"], 12.0, "stats fps");
    assert_f64(
        &stats["duration_secs"],
        2.0 / 12.0,
        "duration is frames / fps",
    );
    assert_eq!(stats["fx_clips"].as_u64(), Some(0));
    assert_eq!(stats["audio_clips"].as_u64(), Some(0));

    let state = client.call_ok(&mut server, "get_state", json!({}));
    assert_eq!(state["project"], "Wave11 E2E");
    assert_eq!(state["frame_count"].as_u64(), Some(2));
    assert_eq!(state["current_frame"].as_u64(), Some(1));
    let state_figs = state["figures"].as_array().expect("get_state lists figures");
    assert_eq!(state_figs.len(), 2);
    assert_eq!(state_figs[1]["name"], "Wave");

    let summary2 = client.call_ok(&mut server, "project_summary", json!({}));
    assert_eq!(summary2["name"], "Wave11 E2E");
    assert_eq!(summary2["figure_count"].as_u64(), Some(2));
    assert_eq!(summary2["frames"].as_u64(), Some(2));
    let names: Vec<&str> = summary2["figures"]
        .as_array()
        .expect("project_summary lists figure names")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(names, ["Styk", "Wave"], "project_summary names the roster in order");
    assert_eq!(summary2["has_audio"], false);
    assert_eq!(summary2["camera_keys"].as_u64(), Some(0));

    // -- Export: PNG (a promised path must be a real file) -----------
    let dir = temp_dir("session");
    let png_path = dir.join("frame-1.png");
    let png_arg = png_path.to_string_lossy().into_owned();
    let rendered = client.call_ok(
        &mut server,
        "render",
        json!({ "path": png_arg, "format": "png", "frame": 1 }),
    );
    assert_eq!(rendered["format"], "png");
    assert_eq!(rendered["path"], png_arg.as_str());
    assert_eq!(rendered["frame"].as_u64(), Some(1));
    assert_eq!(rendered["width"].as_u64(), Some(960), "PNG renders at stage width");
    assert_eq!(rendered["height"].as_u64(), Some(720), "PNG renders at stage height");
    let promised_bytes = rendered["bytes"].as_u64().expect("render reports its byte count");
    let png_bytes = std::fs::read(&png_path).expect("the promised PNG path exists");
    assert_eq!(
        png_bytes.len() as u64,
        promised_bytes,
        "the file on disk is exactly the size the reply promised"
    );
    assert!(
        png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "the exported file is a real PNG (signature bytes)"
    );

    // The base64 twin must agree on the frame's dimensions and
    // decode to the same kind of document.
    let b64 = client.call_ok(
        &mut server,
        "render_frame_base64",
        json!({ "frame": 0 }),
    );
    assert_eq!(b64["frame"].as_u64(), Some(0));
    assert_eq!(b64["width"].as_u64(), Some(960));
    assert_eq!(b64["height"].as_u64(), Some(720));
    let encoded = b64["png_base64"].as_str().expect("base64 payload is text");
    assert!(!encoded.is_empty(), "base64 payload is not empty");
    let decoded = base64_decode(encoded);
    assert!(
        decoded.starts_with(b"\x89PNG\r\n\x1a\n"),
        "the base64 payload decodes to a real PNG"
    );

    // -- Export: SVG (one document per frame, in a directory) --------
    let svg_dir = dir.join("svg");
    let svg_arg = svg_dir.to_string_lossy().into_owned();
    let svg_out = client.call_ok(
        &mut server,
        "render",
        json!({ "path": svg_arg, "format": "svg" }),
    );
    assert_eq!(svg_out["format"], "svg");
    assert_eq!(svg_out["frames"].as_u64(), Some(2), "one SVG per authored frame");
    assert_eq!(
        svg_out["directory"],
        svg_arg.as_str(),
        "for svg the reply names the output directory"
    );
    for frame_name in ["frame-0001.svg", "frame-0002.svg"] {
        assert!(
            svg_dir.join(frame_name).is_file(),
            "{frame_name} exists in the export directory"
        );
    }
    let svg_text =
        std::fs::read_to_string(svg_dir.join("frame-0001.svg")).expect("read exported SVG");
    assert!(svg_text.starts_with("<svg"), "the export is an SVG document");
    assert!(svg_text.contains("<line"), "figure segments are drawn");
    assert!(
        svg_text.contains("#FF5D5D"),
        "the added figure's colour reaches the SVG (uppercase #RRGGBB)"
    );
    assert!(
        svg_text.contains("#E8E8EC"),
        "the default figure's colour reaches the SVG"
    );

    // -- Negative basics, in-session ----------------------------------
    // Unknown tool name: a protocol error, not a tool result.
    let reply = client.call_raw(&mut server, "wave11_no_such_tool", json!({}));
    assert!(
        reply.get("result").is_none(),
        "an unknown tool must not produce a result: {reply}"
    );
    assert_eq!(
        reply["error"]["code"].as_i64(),
        Some(-32602),
        "an unknown tool is a -32602 protocol error"
    );
    let message = reply["error"]["message"]
        .as_str()
        .expect("the protocol error carries a message");
    assert!(
        message.contains("unknown tool"),
        "the error names the problem: {message}"
    );

    // Unknown method: also a protocol error (-32601).
    let reply = client.rpc(&mut server, "wave11/bogus_method", json!({}));
    assert_eq!(reply["error"]["code"].as_i64(), Some(-32601));

    // Missing required argument: a tool error, and the
    // project is exactly as it was before the bad calls.
    let err = client.call_err(&mut server, "set_pose", json!({ "figure": 1 }));
    assert!(
        err.contains("missing required argument 'frame'"),
        "the error names the missing argument: {err}"
    );
    let err = client.call_err(&mut server, "render", json!({ "format": "png" }));
    assert!(
        err.contains("missing required string argument: path"),
        "the error names the missing argument: {err}"
    );
    let err = client.call_err(&mut server, "add_figure", json!({}));
    assert!(
        err.contains("missing required argument 'kind'"),
        "the error names the missing argument: {err}"
    );

    let stats_after = client.call_ok(&mut server, "get_project_stats", json!({}));
    assert_eq!(
        stats_after["figures"].as_u64(),
        Some(2),
        "failed calls must not add figures"
    );
    assert_eq!(
        stats_after["frames"].as_u64(),
        Some(2),
        "failed calls must not add frames"
    );
    let pose1_after = client.call_ok(
        &mut server,
        "get_pose",
        json!({ "figure": 1, "frame": 1 }),
    );
    assert_f64(
        &pose1_after["angles"][1],
        -30.0,
        "failed calls must not disturb existing poses"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// The live-control path: handle_on edits the HOST's core, never the
// router's own — this is the chain the GUI's live MCP endpoint rides.
// ---------------------------------------------------------------------

#[test]
fn handle_on_drives_the_hosts_core_not_the_servers() {
    let mut server = McpServer::new();
    let mut host = EditorCore::new();
    let mut client = Client::new();

    let reply = client.rpc_on(
        &mut server,
        &mut host,
        "initialize",
        json!({ "protocolVersion": "2024-11-05", "capabilities": {} }),
    );
    assert_eq!(reply["result"]["serverInfo"]["name"], "stykanimate-rs");

    // Build a recognisable project in the HOST's core, via MCP.
    let created = client.call_ok_on(
        &mut server,
        &mut host,
        "new_project",
        json!({ "name": "Live Host Project" }),
    );
    assert_eq!(created["project"]["name"], "Live Host Project");
    let framed = client.call_ok_on(&mut server, &mut host, "add_frame", json!({}));
    assert_eq!(framed["frames"].as_u64(), Some(2));

    // The host core shows it through the live path…
    let host_state = client.call_ok_on(&mut server, &mut host, "get_state", json!({}));
    assert_eq!(host_state["project"], "Live Host Project");
    assert_eq!(host_state["frame_count"].as_u64(), Some(2));

    // …while the server's own core, asked through `handle`,
    // still holds its untouched starting project.
    let own_state = client.call_ok(&mut server, "get_state", json!({}));
    assert_ne!(
        own_state["project"], "Live Host Project",
        "handle_on must never leak edits into the router's own core"
    );

    // And the plain path still works independently afterwards:
    // a different project built via `handle` does not reach
    // the host.
    let own_created = client.call_ok(
        &mut server,
        "new_project",
        json!({ "name": "Stdio Project" }),
    );
    assert_eq!(own_created["project"]["name"], "Stdio Project");
    let host_state_after = client.call_ok_on(&mut server, &mut host, "get_state", json!({}));
    assert_eq!(
        host_state_after["project"], "Live Host Project",
        "handle must never leak edits into a host core"
    );
    assert_eq!(host_state_after["frame_count"].as_u64(), Some(2));
}
