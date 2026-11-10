//! Wave 12, Squad B (session B10) — integration tests: the
//! project asset table, driven end-to-end through real MCP
//! dispatch (`McpServer::handle`).
//!
//! Wave 11's remainder list named the gap plainly: the
//! sprites/assets subsystem had NO MCP surface at all. Wave
//! 12's B squad closes it with six registry tools, handlers
//! in `media.rs`, spliced into the registry by the
//! integrator:
//!
//!   assets_list                         (read-only)
//!   asset_info   {index}                (read-only)
//!   asset_import {path}                 (mutating)
//!   asset_rename {index, name}          (mutating)
//!   asset_remove {index}                (mutating)
//!   asset_attach {figure, node, asset}  (mutating)
//!
//! The standing contracts this suite pins, beyond the
//! per-tool behaviour:
//!   * every mutating tool is exactly ONE undo step
//!     (observed through `get_editor_status`'s undo depth);
//!   * a refused call (bad bytes, missing file, bad index,
//!     removing a still-referenced asset) is an `isError`
//!     tool result — never a panic, never a protocol error —
//!     and spends no undo step and changes no state;
//!   * an unknown tool name is a protocol error, `-32602`;
//!   * `asset_remove` refuses while any segment media or
//!     sprite instance references the index (the B1 charter
//!     rule); removing an unreferenced asset leaves the
//!     surviving entries and their references consistent.
//!
//! Gap convention (the Wave 11 D-suite pattern), FLIPPED
//! at the Wave 12 integration: while the six tools were
//! written but not yet spliced, steps carried
//! `// WAVE12-GAP:` markers and the shared read helpers
//! tolerated alternate reply spellings; the splice has
//! landed (registry 230, total 283), so the markers are
//! gone and the helpers below assert the SHIPPED reply
//! shapes strictly — `assets_list`'s `assets` array, the
//! `bytes` field, `get_node`'s always-present `media`
//! member with its `kind` tag. Two tests never needed the
//! flip — the entering-baseline census and the legacy
//! `attach_image_to_node` flow were live throughout.
//!
//! Every step goes through MCP messages; direct filesystem
//! access is used only for what an MCP client itself would
//! observe from outside the protocol: writing the import
//! fixtures, and reading back the saved project's bytes.
//! Each test builds its own server and its own temp dir and
//! shares nothing, so tests never depend on execution
//! order. Only the legacy test changes the process current
//! directory (the `./import` folder is cwd-relative, like
//! `./projects` in the Wave 11 projects suite); every other
//! test passes absolute paths exclusively, so running in
//! parallel with it is safe.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::{Project, SegmentMedia};

/// The canonical 1×1 transparent PNG (67 bytes): signature,
/// IHDR (1×1, 8-bit RGBA), one IDAT, IEND. Small enough to
/// embed, real enough that the `image` decoder accepts it —
/// the asset table stores PNG bytes and the media renderer
/// decodes them, so a signature-only fake would not do.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR, 13 bytes
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1 x 1
    0x08, 0x06, 0x00, 0x00, 0x00, // 8-bit, RGBA, deflate, no interlace
    0x1F, 0x15, 0xC4, 0x89, // IHDR CRC
    0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, // IDAT, 10 bytes
    0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, // zlib data
    0x0D, 0x0A, 0x2D, 0xB4, // IDAT CRC
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
];

/// Bytes that are not a PNG in any way: no signature, no
/// decodable body. Written to a `.png`-named file so the
/// refusal provably keys on content, not on the extension.
const NOT_A_PNG: &[u8] = b"wave 12 fixture: this is not a png, whatever the file is called";

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 11 D-suite pattern)
// ---------------------------------------------------------------------

/// A per-test scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front
/// and the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave12-e2e-assets-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write `bytes` as `dir/name` and return the absolute path
/// as the string an MCP client would pass to a tool.
fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write fixture file");
    path.to_string_lossy().into_owned()
}

/// Restores the process current directory on drop, including
/// when the session panics. Only the legacy test needs it.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> Self {
        let old = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(dir).expect("enter temp dir");
        CwdGuard(old)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// One MCP `tools/call`, decoded the way a client decodes
/// it: the JSON document in the single text content block,
/// plus the protocol's error flag. A protocol-level error
/// response (for example an unknown tool) is folded into
/// the same shape with its message as the text.
struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

/// A small dispatch driver over one `McpServer`: sequential
/// request ids, JSON-RPC envelope assertions on every reply,
/// and ok/err helpers so the sessions below read as sessions.
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
// Shared session reads
// ---------------------------------------------------------------------

/// The session's undo depth — the observable edit history,
/// and so the witness for the one-undo-step contract.
fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

fn read_project(path: &Path) -> Project {
    let text = fs::read_to_string(path).expect("read saved project");
    serde_json::from_str(&text).expect("saved file is a project document")
}

// The asset reads (entry count, per-entry name,
// per-entry byte size) are funnelled through these
// helpers; post-splice they assert B1's shipped reply
// shape strictly: `assets_list` answers
// {ok, count, assets:[asset_json]} and each asset_json
// carries `bytes` (media.rs).
fn asset_entries(list_payload: &Value) -> Vec<Value> {
    list_payload["assets"]
        .as_array()
        .unwrap_or_else(|| panic!("assets_list payload carries no `assets` array: {list_payload}"))
        .clone()
}

fn asset_names(list_payload: &Value) -> Vec<String> {
    asset_entries(list_payload)
        .iter()
        .map(|e| {
            e["name"]
                .as_str()
                .expect("every asset entry carries a name")
                .to_string()
        })
        .collect()
}

/// The table's names, in order, against the expected list.
fn assert_asset_names(list_payload: &Value, expected: &[&str]) {
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(asset_names(list_payload), expected);
}

/// The entry's byte size — asset_json's `bytes` field.
fn byte_size(v: &Value) -> u64 {
    v["bytes"]
        .as_u64()
        .unwrap_or_else(|| panic!("no `bytes` size in: {v}"))
}

// The asset surface's read path is get_node's `media`
// member (added with this wave): always present — null
// when the segment carries nothing, an object
// {"kind":"image","asset":N} when it carries an image,
// {"kind":"text",…} for a text label. The shape is
// asserted only through these two helpers.
fn node_media(mcp: &mut Dispatch, figure: u64, node: u64) -> Value {
    let payload = mcp.call_ok("get_node", json!({ "figure": figure, "node": node }));
    payload
        .get("media")
        .cloned()
        .expect("get_node always carries a `media` member")
}

fn assert_no_media(media: &Value, what: &str) {
    assert!(
        media.is_null(),
        "{what}: expected no segment media, got {media}"
    );
}

fn assert_image_media(media: &Value, asset: u64, what: &str) {
    assert!(
        media.is_object(),
        "{what}: expected image media, got {media}"
    );
    assert_eq!(
        media["asset"].as_u64(),
        Some(asset),
        "{what}: the media references the attached asset"
    );
    assert_eq!(
        media["kind"].as_str(),
        Some("image"),
        "{what}: the media kind is an image"
    );
}

// ---------------------------------------------------------------------
// The entering baseline — live on today's surface, no flip needed
// ---------------------------------------------------------------------

#[test]
fn entering_baseline_census_and_unknown_tool_contract() {
    let mut mcp = Dispatch::new();

    let init = mcp.request("initialize", json!({}));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");

    let list = mcp.request("tools/list", json!({}));
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array");
    assert!(
        tools.len() >= 283,
        "the Wave 12 surface is 283 tools (277 entering + the six asset tools), got {}",
        tools.len()
    );
    let tool_names: BTreeSet<String> = tools
        .iter()
        .map(|t| {
            t["name"]
                .as_str()
                .expect("every tool carries a name")
                .to_string()
        })
        .collect();
    // The asset-adjacent surface that exists TODAY: the
    // selection-scoped legacy attach and its clear, the
    // selection/node/status reads, undo, and the IO pair
    // this suite's round trip rides on.
    for required in [
        "attach_image_to_node",
        "clear_node_media",
        "set_selection",
        "get_node",
        "get_editor_status",
        "undo",
        "save_project",
        "load_project",
    ] {
        assert!(
            tool_names.contains(required),
            "tools/list is missing {required}, which this suite drives"
        );
    }

    // The standing unknown-tool contract, asserted on a
    // name that is not — and must never become — a tool:
    // a protocol error -32602, never a tool result.
    let response = mcp.request(
        "tools/call",
        json!({ "name": "wave12_no_such_tool", "arguments": {} }),
    );
    assert!(
        response.get("result").is_none(),
        "an unknown tool must not produce a result: {response}"
    );
    assert_eq!(
        response["error"]["code"].as_i64(),
        Some(-32602),
        "an unknown tool is a -32602 protocol error"
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .expect("the protocol error carries a message")
            .contains("unknown tool"),
        "the error names the problem: {response}"
    );
}

/// The legacy attach, live today: `attach_image_to_node`
/// takes the first PNG in `./import` onto the selected
/// joint — one undo step for the whole op (asset + media),
/// proven by saving before and after a single undo.
#[test]
fn legacy_attach_image_flow_via_import_folder() {
    let root = temp_dir("legacy");
    let root = fs::canonicalize(&root).expect("canonicalise temp dir");
    let cwd_guard = CwdGuard::enter(&root);
    let mut mcp = Dispatch::new();

    mcp.call_ok("new_project", json!({ "name": "Wave12 Legacy" }));
    let selection = mcp.call_ok("set_selection", json!({ "figure": 0, "node": 1 }));
    assert_eq!(selection["selected_node"], 1);

    // With nothing in ./import the op refuses through the
    // status line, surfaced as a tool error, and no undo
    // step is spent.
    let depth_before = undo_depth(&mut mcp);
    let err = mcp.call_err("attach_image_to_node", json!({}));
    assert!(
        err.contains("import folder"),
        "the refusal names the missing import file: {err}"
    );
    assert_eq!(undo_depth(&mut mcp), depth_before, "a refusal spends no undo step");

    // Put the fixture where the op looks for it.
    let import_dir = root.join("import");
    fs::create_dir_all(&import_dir).expect("create import dir");
    fs::write(import_dir.join("dot.png"), PNG_1X1).expect("write import fixture");

    let attached = mcp.call_ok("attach_image_to_node", json!({}));
    assert_eq!(attached["figure"], 0);
    assert_eq!(attached["node"], 1);
    assert!(
        attached["status"]
            .as_str()
            .expect("the attach reports a status")
            .contains("Sprite attached"),
        "the attach names its success: {attached}"
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "the whole legacy attach (asset + media) is one undo step"
    );

    // The file witness: the saved project carries the asset
    // (named for the file, bytes verbatim) and the media on
    // node 1 of figure 0 — and nowhere else.
    let saved_path = root.join("legacy.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": saved_path.to_string_lossy() }),
    );
    let on_disk = read_project(&saved_path);
    assert_eq!(on_disk.assets.len(), 1);
    assert_eq!(on_disk.assets[0].name, "dot.png");
    assert_eq!(on_disk.assets[0].png, PNG_1X1.to_vec());
    assert_eq!(
        on_disk.figures[0].def.nodes[1].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
    );
    assert_eq!(on_disk.figures[0].def.nodes[2].media, None);

    // ONE undo removes the whole attach: the asset leaves
    // the table and the media leaves the segment, together.
    mcp.call_ok("undo", json!({}));
    let undone_path = root.join("legacy-undone.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": undone_path.to_string_lossy() }),
    );
    let undone = read_project(&undone_path);
    assert_eq!(undone.assets.len(), 0, "one undo removed the imported asset");
    assert_eq!(
        undone.figures[0].def.nodes[1].media, None,
        "the same undo removed the segment media"
    );

    drop(cwd_guard);
    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------
// The new asset surface — the six spliced tools, driven strictly
// ---------------------------------------------------------------------

/// Import, list, info, rename — and the two import refusals
/// (garbage bytes, missing file) changing nothing at all.
// The four read/manage tools of the asset surface:
// asset_import, assets_list, asset_info, asset_rename.
#[test]
fn asset_import_list_info_rename_session() {
    let dir = temp_dir("import");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave12 Assets" }));

    // An empty table lists as empty.
    let empty = mcp.call_ok("assets_list", json!({}));
    assert_eq!(asset_entries(&empty).len(), 0, "a fresh project has no assets");

    // (1) Import the tiny valid PNG: the list shows it with
    // the file's name and the fixture's exact byte size,
    // and the import is exactly one undo step.
    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "asset_import is exactly one undo step"
    );
    let list = mcp.call_ok("assets_list", json!({}));
    let entries = asset_entries(&list);
    assert_eq!(entries.len(), 1, "one asset after one import: {list}");
    assert_eq!(entries[0]["name"], "dot.png", "the asset is named for its file");
    assert_eq!(
        byte_size(&entries[0]),
        PNG_1X1.len() as u64,
        "the listed size is the fixture's byte count"
    );

    // asset_info agrees with the list, entry for entry.
    let info = mcp.call_ok("asset_info", json!({ "index": 0 }));
    assert_eq!(info["name"], "dot.png");
    assert_eq!(byte_size(&info), PNG_1X1.len() as u64);

    // Rename: one undo step, the list follows, and one undo
    // restores the imported name (the rename edits the
    // project, so the project's undo history owns it).
    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok(
        "asset_rename",
        json!({ "index": 0, "name": "hero.png" }),
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "asset_rename is exactly one undo step"
    );
    let renamed = mcp.call_ok("assets_list", json!({}));
    assert_asset_names(&renamed, &["hero.png"]);
    mcp.call_ok("undo", json!({}));
    let restored = mcp.call_ok("assets_list", json!({}));
    assert_eq!(
        asset_names(&restored),
        ["dot.png".to_string()],
        "one undo restored the imported name"
    );

    // (2) Garbage bytes under a .png name: an isError
    // result (not a panic, not a protocol error), and the
    // session is exactly as it was — same assets, same
    // undo depth.
    let junk_path = write_fixture(&dir, "junk.png", NOT_A_PNG);
    let depth_before = undo_depth(&mut mcp);
    let err = mcp.call_err("asset_import", json!({ "path": junk_path }));
    assert!(!err.is_empty(), "the refusal says why: {err}");
    let after_junk = mcp.call_ok("assets_list", json!({}));
    assert_eq!(
        asset_names(&after_junk),
        ["dot.png".to_string()],
        "a refused import added no asset"
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a refused import spends no undo step"
    );

    // A missing file refuses the same way.
    let missing = dir.join("no-such-file.png").to_string_lossy().into_owned();
    assert!(!Path::new(&missing).exists());
    mcp.call_err("asset_import", json!({ "path": missing }));
    let after_miss = mcp.call_ok("assets_list", json!({}));
    assert_asset_names(&after_miss, &["dot.png"]);
    assert_eq!(undo_depth(&mut mcp), depth_before);

    // Info on an index that does not exist is a tool error,
    // not a panic and not a phantom entry.
    mcp.call_err("asset_info", json!({ "index": 99 }));

    let _ = fs::remove_dir_all(&dir);
}

/// Attach to a segment, the one-undo contract, and the
/// removal refusal while the asset is referenced.
// asset_attach and asset_remove, with asset_import and
// assets_list setting the scene.
#[test]
fn asset_attach_undo_redo_and_remove_refusal_session() {
    let dir = temp_dir("attach");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave12 Attach" }));

    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    assert_no_media(&node_media(&mut mcp, 0, 1), "before the attach");

    // Bad indices refuse as tool errors and change nothing:
    // no media appears and no undo step is spent.
    let depth_before = undo_depth(&mut mcp);
    mcp.call_err(
        "asset_attach",
        json!({ "figure": 0, "node": 1, "asset": 99 }),
    );
    mcp.call_err(
        "asset_attach",
        json!({ "figure": 0, "node": 99, "asset": 0 }),
    );
    mcp.call_err(
        "asset_attach",
        json!({ "figure": 99, "node": 1, "asset": 0 }),
    );
    assert_no_media(&node_media(&mut mcp, 0, 1), "after the refused attaches");
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "refused attaches spend no undo step"
    );

    // (3) The attach itself: one undo step, and the node
    // read path shows the media referencing asset 0.
    mcp.call_ok(
        "asset_attach",
        json!({ "figure": 0, "node": 1, "asset": 0 }),
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "asset_attach is exactly one undo step"
    );
    assert_image_media(&node_media(&mut mcp, 0, 1), 0, "after the attach");

    // ONE undo removes the attach — and only the attach:
    // the imported asset stays in the table.
    mcp.call_ok("undo", json!({}));
    assert_no_media(&node_media(&mut mcp, 0, 1), "after one undo");
    let list = mcp.call_ok("assets_list", json!({}));
    assert_eq!(
        asset_names(&list),
        ["dot.png".to_string()],
        "undoing the attach did not un-import the asset"
    );

    // Redo puts the media back, restoring the referenced
    // state the removal refusal below needs.
    mcp.call_ok("redo", json!({}));
    assert_image_media(&node_media(&mut mcp, 0, 1), 0, "after the redo");

    // (4) Removal while a segment references the asset is
    // refused: an isError result, the table unchanged, no
    // undo step spent. (The charter rule covers sprite
    // instances too; no MCP tool on this suite's surface
    // creates one, so the segment-media reference is the
    // drivable case here.)
    let depth_before = undo_depth(&mut mcp);
    let err = mcp.call_err("asset_remove", json!({ "index": 0 }));
    assert!(!err.is_empty(), "the refusal says why: {err}");
    let list = mcp.call_ok("assets_list", json!({}));
    assert_asset_names(&list, &["dot.png"]);
    assert_image_media(&node_media(&mut mcp, 0, 1), 0, "after the refused removal");
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a refused removal spends no undo step"
    );

    // Clear the media through the surface that exists for
    // it today (selection-scoped, exactly one undo step)…
    mcp.call_ok("set_selection", json!({ "figure": 0, "node": 1 }));
    mcp.call_ok("clear_node_media", json!({}));
    assert_no_media(&node_media(&mut mcp, 0, 1), "after clearing the media");
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "clearing the media is exactly one undo step"
    );

    // …and now the removal succeeds, as one undo step,
    // leaving an empty table.
    mcp.call_ok("asset_remove", json!({ "index": 0 }));
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 2,
        "asset_remove is exactly one undo step"
    );
    let list = mcp.call_ok("assets_list", json!({}));
    assert_eq!(asset_entries(&list).len(), 0, "the table is empty after the removal");

    let _ = fs::remove_dir_all(&dir);
}

/// Removal of an UNREFERENCED asset: the surviving entries
/// keep their indices and their references — the half of
/// the B1 rule that can be pinned without a stated
/// renumbering contract for shifted references.
// asset_import, asset_attach, asset_remove, assets_list.
#[test]
fn asset_remove_keeps_surviving_indices_stable() {
    let dir = temp_dir("indices");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave12 Indices" }));

    let a_path = write_fixture(&dir, "dot.png", PNG_1X1);
    let b_path = write_fixture(&dir, "dot-b.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": a_path }));
    mcp.call_ok("asset_import", json!({ "path": b_path }));
    let list = mcp.call_ok("assets_list", json!({}));
    assert_asset_names(&list, &["dot.png", "dot-b.png"]);

    // Reference asset 0 from a segment; asset 1 stays
    // unreferenced.
    mcp.call_ok(
        "asset_attach",
        json!({ "figure": 0, "node": 1, "asset": 0 }),
    );
    assert_image_media(&node_media(&mut mcp, 0, 1), 0, "after the attach");

    // Removing the unreferenced tail entry succeeds, and
    // everything that survives is exactly where it was:
    // asset 0 still at index 0, the segment still pointing
    // at it.
    mcp.call_ok("asset_remove", json!({ "index": 1 }));
    let list = mcp.call_ok("assets_list", json!({}));
    assert_asset_names(&list, &["dot.png"]);
    let info = mcp.call_ok("asset_info", json!({ "index": 0 }));
    assert_eq!(info["name"], "dot.png");
    assert_image_media(
        &node_media(&mut mcp, 0, 1),
        0,
        "the surviving reference still resolves to asset 0",
    );

    // The referenced survivor still refuses removal, and a
    // removal at an index that no longer exists is a tool
    // error rather than a panic.
    mcp.call_err("asset_remove", json!({ "index": 0 }));
    mcp.call_err("asset_remove", json!({ "index": 99 }));

    let _ = fs::remove_dir_all(&dir);
}

/// (5) Save + reload: the asset bytes cross the file
/// round trip byte-identical, media reference included.
// asset_import, asset_attach, assets_list — with
// save_project / load_project as the IO pair.
#[test]
fn asset_save_load_round_trip_keeps_bytes() {
    let dir = temp_dir("roundtrip");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave12 Round Trip" }));

    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    mcp.call_ok(
        "asset_attach",
        json!({ "figure": 0, "node": 1, "asset": 0 }),
    );

    let saved_path = dir.join("round-trip.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": saved_path.to_string_lossy() }),
    );

    // The file witness, read the way a client holding the
    // file would: the stored asset bytes are the fixture's
    // bytes, verbatim, and the segment media survived.
    let on_disk = read_project(&saved_path);
    assert_eq!(on_disk.assets.len(), 1);
    assert_eq!(on_disk.assets[0].name, "dot.png");
    assert_eq!(
        on_disk.assets[0].png,
        PNG_1X1.to_vec(),
        "the saved asset bytes are byte-identical to the imported fixture"
    );
    assert_eq!(
        on_disk.figures[0].def.nodes[1].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
    );

    // Replace the session with an empty project, prove the
    // table is empty, then load the file back.
    mcp.call_ok("new_project", json!({ "name": "Wave12 Wipe" }));
    let wiped = mcp.call_ok("assets_list", json!({}));
    assert_eq!(asset_entries(&wiped).len(), 0, "the replacement project has no assets");

    let opened = mcp.call_ok(
        "load_project",
        json!({ "path": saved_path.to_string_lossy() }),
    );
    assert_eq!(opened["name"], "Wave12 Round Trip");

    // The reloaded session shows the same table and the
    // same media — and a second save writes the same asset
    // bytes again, so the round trip is stable, not lossy
    // in either direction.
    let list = mcp.call_ok("assets_list", json!({}));
    let entries = asset_entries(&list);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "dot.png");
    assert_eq!(byte_size(&entries[0]), PNG_1X1.len() as u64);
    assert_image_media(&node_media(&mut mcp, 0, 1), 0, "after the reload");

    let resaved_path = dir.join("round-trip-2.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": resaved_path.to_string_lossy() }),
    );
    let resaved = read_project(&resaved_path);
    assert_eq!(resaved.assets.len(), 1);
    assert_eq!(
        resaved.assets[0].png,
        PNG_1X1.to_vec(),
        "the reloaded asset re-saves byte-identical"
    );
    assert_eq!(
        resaved.figures[0].def.nodes[1].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
    );

    let _ = fs::remove_dir_all(&dir);
}
