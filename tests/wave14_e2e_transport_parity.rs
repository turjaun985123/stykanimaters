//! Wave 14, Squad D (session D2) — integration tests: stdio↔live
//! DISPATCH parity, driven end-to-end through the two public
//! dispatch surfaces of `McpServer`.
//!
//! The two surfaces under test:
//!   (a) `McpServer::handle` — the stdio path: the server routes
//!       against its OWN core (this is what `src/bin/mcp_server.rs`
//!       calls per line);
//!   (b) `McpServer::handle_on` — the live path's dispatch: the
//!       server routes against an EXTERNAL core (this is what the
//!       GUI's drain calls per queued request, `src/live_mcp.rs`:
//!       `router.handle_on(core, &req.msg)`).
//!
//! The HTTP/socket layer above (b) has its own end-to-end suite
//! already; this file's target is the dispatch itself. The two
//! framing wrappers are likewise out of scope and are named here
//! only to mark the boundary: the stdio binary adds JSON parse
//! errors (`-32700`, id `null`) and a panic net around `handle`
//! (`src/bin/mcp_server.rs`), and the live HTTP layer adds its
//! own parse-error body in the same `-32700` convention plus the
//! `GET /health` endpoint (`src/live_mcp.rs`) — the wrappers
//! mirror each other by design and neither is exercised here.
//!
//! The parity truth, read from source before writing (the claim
//! this suite exists to PIN, not to discover): both entry points
//! are thin delegates over the one router, `McpServer::dispatch`
//! (`src/mcp.rs`), parameterised only over which core is the edit
//! target. Consequences the tests below assert end-to-end:
//!   * there are NO per-transport fields in any dispatch response
//!     — no transport name, no mode flag, nothing to allow-list;
//!   * `tools/list` returns `all_tools()` in its own order on
//!     both paths (the one `tools_list_result` maps the vec in
//!     order), so list equality is asserted INCLUDING order;
//!   * the single by-design asymmetry is the edit TARGET:
//!     `handle` mutates the server's own core, `handle_on`
//!     mutates the caller's core and leaves the router's own core
//!     untouched (pinned in its own test below, so nobody
//!     "repairs" it later).
//!
//! The only non-identical bytes in the scripted conversation are
//! the harness's own, not the transports': each side gets its
//! own temp directory, so the fixture PNG path (asset_import)
//! and the save destination (save_project) differ by directory.
//! Responses are compared after replacing the side's directory
//! string with a fixed placeholder wherever it appears (the save
//! result echoes the path, and `get_editor_status`'s status line
//! quotes it after a save). Everything else — every result value,
//! every error text, every id echo — must match exactly.
//!
//! The conversation itself is one fixed `Vec` of request values
//! (built once per side by `conversation`, parameterised only by
//! the two filesystem paths): initialize, tools/list, ping, then
//! a mutation sequence across families — frame add, figure add,
//! asset import, sprite add + sprite key, FX clip add — with
//! read-backs, a project save to the side's temp dir, and final
//! state/status snapshots. Fresh cores per side; each test
//! builds its own sides and its own temp dirs and shares
//! nothing, so tests never depend on execution order.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;

/// The canonical 1×1 transparent PNG (67 bytes) — the same
/// fixture the Wave 12/13 suites drive: small enough to embed,
/// real enough that the `image` decoder accepts it (a sprite's
/// quad geometry derives from the asset's decoded size).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR, 13 bytes
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1 x 1
    0x08, 0x06, 0x00, 0x00, 0x00, // 8-bit, RGBA, deflate
    0x1F, 0x15, 0xC4, 0x89, // IHDR CRC
    0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, // IDAT, 10 bytes
    0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, // zlib data
    0x0D, 0x0A, 0x2D, 0xB4, // IDAT CRC
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
];

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// A per-test, per-side scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front and
/// the caller removes the directory again when done. The side
/// tag keeps the two transports' files apart (the save targets
/// must never collide — and the directory string is exactly
/// what the response normaliser masks, see `mask_paths`).
fn temp_dir(test: &str, side: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave14-e2e-parity-{}-{test}-{side}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write `bytes` as `dir/name` and return the absolute path as
/// the string an MCP client would pass to a tool.
fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write fixture file");
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------
// The two dispatch sides
// ---------------------------------------------------------------------

/// One transport under test. Both variants wrap a router; they
/// differ ONLY in which core a `tools/call` edits — the whole
/// point of the suite.
enum Side {
    /// (a) stdio: `handle` against the server's own core.
    Stdio { server: McpServer },
    /// (b) live: `handle_on` against an external core; the
    /// router's own core must stay untouched throughout.
    Live { router: McpServer, core: EditorCore },
}

impl Side {
    fn stdio() -> Self {
        Side::Stdio {
            server: McpServer::new(),
        }
    }

    fn live() -> Self {
        Side::Live {
            router: McpServer::new(),
            core: EditorCore::new(),
        }
    }

    /// Send one parsed JSON-RPC message through this side's
    /// dispatch surface; `None` is the notification answer.
    fn send(&mut self, msg: &Value) -> Option<Value> {
        match self {
            Side::Stdio { server } => server.handle(msg),
            Side::Live { router, core } => router.handle_on(core, msg),
        }
    }

    /// A request (it carries an id) must always be answered.
    fn request(&mut self, msg: &Value) -> Value {
        self.send(msg).expect("a request with an id must be answered")
    }
}

// ---------------------------------------------------------------------
// The scripted conversation
// ---------------------------------------------------------------------

/// One JSON-RPC request envelope with a pre-stamped id.
fn req(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// One `tools/call` request envelope.
fn call(id: u64, name: &str, args: Value) -> Value {
    req(id, "tools/call", json!({ "name": name, "arguments": args }))
}

/// The fixed conversation, identical for both sides except the
/// two filesystem paths (the imported fixture and the save
/// destination live under the side's own temp dir). Step labels
/// ride alongside each request so a parity failure names the
/// exact step that diverged.
fn conversation(fixture_png: &str, save_path: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "initialize",
            req(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "parity-harness", "version": "0" },
                }),
            ),
        ),
        ("tools/list", req(2, "tools/list", json!({}))),
        ("ping", req(3, "ping", json!({}))),
        ("get_state (entering)", call(4, "get_state", json!({}))),
        ("add_frame", call(5, "add_frame", json!({}))),
        (
            "add_figure",
            call(
                6,
                "add_figure",
                json!({ "kind": "mini", "name": "Parity", "color": 0x3366CC }),
            ),
        ),
        (
            "asset_import",
            call(7, "asset_import", json!({ "path": fixture_png })),
        ),
        ("sprite_add", call(8, "sprite_add", json!({ "asset": 0 }))),
        (
            "sprite_set_key",
            call(
                9,
                "sprite_set_key",
                json!({
                    "sprite": 0, "frame": 1,
                    "x": 42.5, "y": 24.0, "rotation": 15.0, "scale": 1.5,
                }),
            ),
        ),
        (
            "add_fx_clip",
            call(
                10,
                "add_fx_clip",
                json!({
                    "figure": 0, "node": 0, "kind": "sparkle",
                    "start": 0.0, "duration": 2.0,
                }),
            ),
        ),
        ("sprite_list", call(11, "sprite_list", json!({}))),
        ("fx_list", call(12, "fx_list", json!({}))),
        (
            "save_project",
            call(13, "save_project", json!({ "path": save_path })),
        ),
        ("get_state (after)", call(14, "get_state", json!({}))),
        (
            "get_editor_status (after)",
            call(15, "get_editor_status", json!({})),
        ),
    ]
}

// ---------------------------------------------------------------------
// Response normalisation: mask the side's own paths, canonicalise
// ---------------------------------------------------------------------

/// Replace every occurrence of the side's temp-dir string in
/// any string value with a fixed placeholder. This masks the
/// harness's own per-side paths (fixture + save destination,
/// and anything quoting them, like the post-save status line) —
/// it must never mask a transport difference, and by the source
/// reading in the module doc there are none to mask.
fn mask_paths(value: &Value, dir: &Path) -> Value {
    let needle = dir.to_string_lossy().into_owned();
    fn walk(value: &Value, needle: &str) -> Value {
        match value {
            Value::String(s) => Value::String(s.replace(needle, "<PARITY_TMPDIR>")),
            Value::Array(items) => {
                Value::Array(items.iter().map(|item| walk(item, needle)).collect())
            }
            Value::Object(map) => {
                let mut out = Map::new();
                for (key, item) in map {
                    out.insert(key.clone(), walk(item, needle));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }
    walk(value, &needle)
}

/// Rebuild a value with every object's keys in sorted order, so
/// two responses can be compared byte-for-byte (via their
/// serialisation) regardless of map insertion order. Array
/// order is preserved — it is meaningful (the tool list).
fn canonical(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = Map::new();
            for (key, item) in entries {
                out.insert(key.clone(), canonical(item));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Run the whole scripted conversation down one side and return
/// the responses, masked and canonicalised, in step order.
/// Every response's envelope (jsonrpc version + id echo) is
/// asserted here, on both sides alike.
fn run_conversation(side: &mut Side, dir: &Path) -> Vec<(String, Value)> {
    let fixture = write_fixture(dir, "badge.png", PNG_1X1);
    let save_path = dir
        .join("parity-project.styk.json")
        .to_string_lossy()
        .into_owned();
    let steps = conversation(&fixture, &save_path);
    let mut responses = Vec::with_capacity(steps.len());
    for (label, request) in &steps {
        let response = side.request(request);
        assert_eq!(response["jsonrpc"], "2.0", "{label}: bad envelope");
        assert_eq!(
            response["id"], request["id"],
            "{label}: response does not echo the request id"
        );
        responses.push((
            label.to_string(),
            canonical(&mask_paths(&response, dir)),
        ));
    }
    responses
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// The headline harness: the same scripted conversation —
/// handshake, tool list, and a mutation sequence across the
/// frame, figure, asset/sprite and FX families, ending in a
/// save and two state snapshots — must produce byte-identical
/// responses on the stdio dispatch (`handle`) and the live
/// dispatch (`handle_on`), once each side's own temp-dir paths
/// are masked. Any divergence names its step.
#[test]
fn scripted_conversation_replies_identically_on_stdio_and_live_dispatch() {
    let stdio_dir = temp_dir("conversation", "stdio");
    let live_dir = temp_dir("conversation", "live");

    let stdio_responses = run_conversation(&mut Side::stdio(), &stdio_dir);
    let live_responses = run_conversation(&mut Side::live(), &live_dir);

    assert_eq!(
        stdio_responses.len(),
        live_responses.len(),
        "the two runs answered a different number of steps"
    );
    for ((label, stdio_response), (_, live_response)) in
        stdio_responses.iter().zip(live_responses.iter())
    {
        assert_eq!(
            stdio_response,
            live_response,
            "dispatch divergence at step '{label}':\nstdio: {}\nlive:  {}",
            serde_json::to_string_pretty(stdio_response).unwrap_or_default(),
            serde_json::to_string_pretty(live_response).unwrap_or_default(),
        );
    }

    // Both saves really happened, to each side's own file.
    for dir in [&stdio_dir, &live_dir] {
        let saved = dir.join("parity-project.styk.json");
        assert!(
            saved.exists(),
            "save_project left no file at {}",
            saved.display()
        );
    }

    let _ = fs::remove_dir_all(&stdio_dir);
    let _ = fs::remove_dir_all(&live_dir);
}

/// The tool list itself: same tools, same order, same schemas —
/// the full `tools/list` result must be equal as a value (array
/// order is significant in the comparison), and its length must
/// be the library's own census, so the pin cannot quietly rot
/// as tools are added: new tools extend both sides together or
/// the count assert names the drift.
#[test]
fn tools_list_is_identical_in_content_and_order_on_both_paths() {
    let request = req(1, "tools/list", json!({}));

    let stdio_response = Side::stdio().request(&request);
    let live_response = Side::live().request(&request);
    assert_eq!(
        canonical(&stdio_response),
        canonical(&live_response),
        "tools/list differs between the stdio and live dispatch"
    );

    let names: Vec<&str> = stdio_response["result"]["tools"]
        .as_array()
        .expect("tools/list result carries a tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("every tool carries a name"))
        .collect();
    assert_eq!(
        names.len(),
        stykanimate_rs::mcp::all_tools().len(),
        "tools/list length disagrees with the all_tools() census"
    );
    // The families the scripted conversation exercises are on
    // the list (guards against a list that is equal but empty).
    for expected in [
        "get_state",
        "add_frame",
        "add_figure",
        "asset_import",
        "sprite_add",
        "sprite_set_key",
        "add_fx_clip",
        "save_project",
    ] {
        assert!(
            names.contains(&expected),
            "tools/list is missing {expected}"
        );
    }
    // No duplicate names: a duplicated entry would still
    // compare equal across paths while breaking clients.
    let unique: std::collections::BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "tools/list carries a duplicate name");
}

/// Error shapes, part 1: an unknown tool name is a PROTOCOL
/// error — code `-32602`, message `unknown tool: {name}` (the
/// standing Wave 11 contract) — never an `isError` tool result;
/// an unknown METHOD is `-32601`. Both responses must be
/// identical across the two dispatches, in full.
#[test]
fn unknown_tool_and_unknown_method_errors_match_on_both_paths() {
    let unknown_tool = call(1, "no_such_tool_wave14", json!({}));
    let stdio_response = Side::stdio().request(&unknown_tool);
    let live_response = Side::live().request(&unknown_tool);
    assert_eq!(
        canonical(&stdio_response),
        canonical(&live_response),
        "unknown-tool error differs between dispatches"
    );
    assert_eq!(stdio_response["error"]["code"], json!(-32602));
    assert_eq!(
        stdio_response["error"]["message"],
        json!("unknown tool: no_such_tool_wave14")
    );
    assert!(
        stdio_response.get("result").is_none(),
        "an unknown tool must not produce a result object"
    );

    let unknown_method = req(2, "no/such/method", json!({}));
    let stdio_response = Side::stdio().request(&unknown_method);
    let live_response = Side::live().request(&unknown_method);
    assert_eq!(
        canonical(&stdio_response),
        canonical(&live_response),
        "unknown-method error differs between dispatches"
    );
    assert_eq!(stdio_response["error"]["code"], json!(-32601));
    assert_eq!(
        stdio_response["error"]["message"],
        json!("Method not found: no/such/method")
    );
}

/// Error shapes, part 2: a KNOWN tool called with bad arguments
/// (a missing required argument; a wrong-typed argument) is an
/// `isError` TOOL RESULT — the tool ran and refused — with the
/// refusal text identical on both dispatches, and it changes no
/// state: the project snapshot after the refused calls equals
/// the one before them, on each side.
#[test]
fn bad_arguments_errors_match_on_both_paths_and_change_nothing() {
    let mut stdio = Side::stdio();
    let mut live = Side::live();

    let state_before_stdio = stdio.request(&call(1, "get_state", json!({})));
    let state_before_live = live.request(&call(1, "get_state", json!({})));
    assert_eq!(canonical(&state_before_stdio), canonical(&state_before_live));

    let bad_calls: Vec<(&str, Value)> = vec![
        // Missing the one required argument ('kind').
        ("add_figure without kind", call(2, "add_figure", json!({}))),
        // A string where an integer index is required.
        (
            "sprite_add with a string asset",
            call(3, "sprite_add", json!({ "asset": "zero" })),
        ),
    ];
    for (label, request) in &bad_calls {
        let stdio_response = stdio.request(request);
        let live_response = live.request(request);
        assert_eq!(
            canonical(&stdio_response),
            canonical(&live_response),
            "{label}: bad-arguments error differs between dispatches"
        );
        let result = &stdio_response["result"];
        assert_eq!(
            result["isError"],
            json!(true),
            "{label}: a refused call must be an isError tool result, not a protocol error"
        );
        assert!(
            stdio_response.get("error").is_none(),
            "{label}: a refused call must not be a protocol error"
        );
        let text = result["content"][0]["text"]
            .as_str()
            .expect("the refusal carries one text block");
        assert!(!text.is_empty(), "{label}: the refusal text is empty");
    }

    // Nothing moved: post-refusal state equals pre-refusal
    // state on BOTH sides (no partial edit, no spent undo step
    // visible in the project snapshot).
    let state_after_stdio = stdio.request(&call(4, "get_state", json!({})));
    let state_after_live = live.request(&call(4, "get_state", json!({})));
    assert_eq!(canonical(&state_after_stdio), canonical(&state_before_stdio));
    assert_eq!(canonical(&state_after_live), canonical(&state_before_live));
}

/// Notifications (messages with no `id`) are never answered —
/// known method or not — on either dispatch, and a
/// notification-shaped `tools/call` performs no mutation: the
/// router's notification rule is checked BEFORE the method, in
/// the one shared `dispatch`, and this pins that both surfaces
/// inherit it.
#[test]
fn notifications_are_never_answered_on_either_path() {
    let mut stdio = Side::stdio();
    let mut live = Side::live();

    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    });
    assert!(
        stdio.send(&notification).is_none(),
        "stdio dispatch answered a notification"
    );
    assert!(
        live.send(&notification).is_none(),
        "live dispatch answered a notification"
    );

    // A tools/call with no id is still a notification: silence,
    // and the add_frame inside it must NOT have run.
    let silent_call = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": { "name": "add_frame", "arguments": {} },
    });
    assert!(stdio.send(&silent_call).is_none());
    assert!(live.send(&silent_call).is_none());

    let stdio_state = stdio.request(&call(1, "get_state", json!({})));
    let live_state = live.request(&call(1, "get_state", json!({})));
    assert_eq!(canonical(&stdio_state), canonical(&live_state));
    // Both still show the fresh demo project's frame count —
    // the silent add_frame added nothing on either side. The
    // expected count is read from a third fresh side rather
    // than hard-coded, so a demo-project change cannot make
    // this test lie.
    let fresh_state = Side::stdio().request(&call(1, "get_state", json!({})));
    assert_eq!(
        stdio_state["result"], fresh_state["result"],
        "a notification tools/call mutated the stdio core"
    );
    assert_eq!(
        live_state["result"], fresh_state["result"],
        "a notification tools/call mutated the live core"
    );
}

/// The one by-design asymmetry, pinned so it is never mistaken
/// for a divergence: `handle` edits the SERVER's own core;
/// `handle_on` edits the CALLER's core and leaves the router's
/// own core exactly as it was. After the same mutations down
/// both surfaces, each surface's edited core shows them — and
/// the live router, asked about its OWN core afterwards, still
/// reports a fresh project.
#[test]
fn the_two_paths_differ_only_in_which_core_they_edit() {
    let mut stdio_server = McpServer::new();
    let mut router = McpServer::new();
    let mut live_core = EditorCore::new();

    let mutation = call(1, "add_frame", json!({}));
    let stdio_response = stdio_server
        .handle(&mutation)
        .expect("stdio request answered");
    let live_response = router
        .handle_on(&mut live_core, &mutation)
        .expect("live request answered");
    assert_eq!(
        canonical(&stdio_response),
        canonical(&live_response),
        "the mutation's reply already differs between dispatches"
    );

    let state = call(2, "get_state", json!({}));
    let stdio_state = stdio_server.handle(&state).expect("stdio state answered");
    let live_state = router
        .handle_on(&mut live_core, &state)
        .expect("live state answered");
    assert_eq!(
        canonical(&stdio_state),
        canonical(&live_state),
        "both edited cores must show the same post-mutation state"
    );

    // The router's OWN core was never the target: asking the
    // router through `handle` shows the un-mutated fresh
    // project (one frame fewer than either edited core).
    let router_own_state = router.handle(&state).expect("router state answered");
    let edited_frames = stdio_state["result"]["content"][0]["text"]
        .as_str()
        .map(|text| {
            serde_json::from_str::<Value>(text).expect("get_state text is JSON")
        })
        .expect("get_state carries a text block")["frame_count"]
        .as_u64()
        .expect("frame_count is a count");
    let router_frames = router_own_state["result"]["content"][0]["text"]
        .as_str()
        .map(|text| {
            serde_json::from_str::<Value>(text).expect("get_state text is JSON")
        })
        .expect("get_state carries a text block")["frame_count"]
        .as_u64()
        .expect("frame_count is a count");
    assert_eq!(
        edited_frames,
        router_frames + 1,
        "handle_on must leave the router's own core untouched \
         (edited cores at {edited_frames} frames, router's own at {router_frames})"
    );
}
