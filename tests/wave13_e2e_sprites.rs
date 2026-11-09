//! Wave 13, Squad C (session C10) — integration tests: the
//! free-standing sprite surface, driven end-to-end through
//! real MCP dispatch (`McpServer::handle`).
//!
//! Wave 12's B5 audit proved the entering truth: sprite
//! instances (`Project.sprites`) existed in the model and
//! rendered in exports, but NO surface could create or edit
//! one — they arrived only via `.nodes` import. Wave 13's
//! A squad builds the surface; A4's handlers live in
//! `media.rs` beside the Wave 12 asset handlers:
//!
//!   sprite_list                          (read-only)
//!   sprite_add          {asset}          (mutating)
//!   sprite_remove       {index}          (mutating)
//!   sprite_set_key      {index, frame, x, y, rotation, scale} (mutating)
//!   sprite_delete_key   {index, frame}   (mutating)
//!
//! The standing contracts this suite pins, beyond the
//! per-tool behaviour (the Wave 12 asset suite's contracts,
//! carried over unchanged):
//!   * every mutating tool is exactly ONE undo step
//!     (observed through `get_editor_status`'s undo depth);
//!   * a refused call (bad asset index, bad sprite index,
//!     a key write on a sprite that does not exist) is an
//!     `isError` tool result — never a panic, never a
//!     protocol error — and spends no undo step and changes
//!     no state;
//!   * an unknown tool name is a protocol error, `-32602`;
//!   * `sprite_add` seeds exactly one key at frame 0 (A1's
//!     charter: the instance lands at stage centre with an
//!     identity transform — scale 1, angle 0);
//!   * `sprite_set_key` at a frame that already carries a
//!     key REPLACES that key (the house keyframe semantic,
//!     cf. `set_camera_key`); at a new frame it inserts one;
//!   * the Wave 12 cross-surface rule still holds with the
//!     new writers in play: `asset_remove` refuses while a
//!     sprite instance references the asset, and succeeds
//!     once the last referencing sprite is removed.
//!
//! Gap convention (the Wave 12 B10 pattern, pre-flip): at
//! the time this file was written, A4's handlers were not
//! yet written and the registry splice had not landed, so
//! every step that calls a sprite tool carries a
//! `// WAVE13-GAP:` marker naming the tool, and everything
//! whose spelling A4's report must still fix — argument
//! names, the list reply's field names — is funnelled
//! through the helpers in the "WAVE13-GAP funnels" section
//! below, so the integrator's flip reconciles spellings
//! THERE, not at every call site. The funnels are the ONLY
//! tolerant code in the file: the value witnesses are
//! strict, because they read the saved project through the
//! model types (`Project.sprites`, `InstanceKey`), whose
//! shape no splice can change. One test needs no flip —
//! the entering-baseline census is live on today's surface.
//!
//! The read-back path for key values is twofold: the file
//! witness (save the project, parse it back, inspect
//! `sprites[i].keys` directly), and the model's own
//! sampling (`InstanceKey::sample` over the witnessed keys)
//! — together they pin what `sprite_set_key` wrote without
//! depending on any reply shape at all.
//!
//! Every step goes through MCP messages; direct filesystem
//! access is used only for what an MCP client itself would
//! observe from outside the protocol: writing the import
//! fixture, and reading back the saved project's bytes.
//! Each test builds its own server and its own temp dir and
//! shares nothing, so tests never depend on execution
//! order. No test changes the process current directory;
//! every path passed to a tool is absolute, so running in
//! parallel with the other suites is safe.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::{InstanceKey, Project, SpriteInstance};

/// The canonical 1×1 transparent PNG (67 bytes): signature,
/// IHDR (1×1, 8-bit RGBA), one IDAT, IEND. The same fixture
/// the Wave 12 asset suite drives — small enough to embed,
/// real enough that the `image` decoder accepts it, which
/// matters here because a sprite's quad geometry derives
/// from the asset's decoded size.
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

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 11 D-suite pattern)
// ---------------------------------------------------------------------

/// A per-test scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front
/// and the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave13-e2e-sprites-{}-{tag}",
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
// Shared session reads — strict witnesses (no GAP tolerance)
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

/// Save through the shipped IO tool and return the sprite
/// table exactly as it crossed the file boundary — the
/// ground-truth witness for everything the sprite tools
/// wrote, independent of any sprite reply shape.
fn saved_sprites(mcp: &mut Dispatch, dir: &Path, file: &str) -> Vec<SpriteInstance> {
    let path = dir.join(file);
    mcp.call_ok(
        "save_project",
        json!({ "path": path.to_string_lossy() }),
    );
    read_project(&path).sprites
}

/// Float comparison for key values: the values this suite
/// writes are exactly representable, but the JSON and f32
/// round trips earn a small epsilon rather than an
/// assert_eq on raw bits.
fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{what}: expected {expected}, got {actual}"
    );
}

/// One witnessed key, field by field. NOTE the model's
/// spelling: the rotation lives in `InstanceKey::angle`;
/// `rotation` is only the tool argument's name (see the
/// set-key funnel below).
fn assert_key(key: &InstanceKey, frame: f32, x: f32, y: f32, scale: f32, angle: f32, what: &str) {
    assert_close(key.frame, frame, &format!("{what}: key frame"));
    assert_close(key.x, x, &format!("{what}: key x"));
    assert_close(key.y, y, &format!("{what}: key y"));
    assert_close(key.scale, scale, &format!("{what}: key scale"));
    assert_close(key.angle, angle, &format!("{what}: key angle"));
}

// ---------------------------------------------------------------------
// WAVE13-GAP funnels — the ONLY tolerant code in this file
// ---------------------------------------------------------------------
//
// A4's handlers (media.rs) and the registry splice land
// after this file. Everything whose spelling they must
// still fix is concentrated here:
//
//   * the list reply's entry array: `sprites`, or
//     `instances`, or the payload as a bare array
//     (sprite_entries);
//   * a list entry's asset field (entry_asset) and key
//     count — a `keys` array's length, or a `key_count` /
//     `keys_count` number (entry_key_count);
//   * every sprite call's argument object: the sprite is
//     addressed as `index` (the house convention the asset
//     tools set), the asset as `asset`, and the rotation
//     argument is spelled `rotation` where the model field
//     is `angle` (the sprite_* wrappers).
//
// The flip: when A4's shipped shapes are known, tighten
// each funnel to the one true spelling and delete the
// alternates; the test bodies below do not change.

// WAVE13-GAP: sprite_list — the reply's entry array under
// any of the plausible field spellings.
fn sprite_entries(list_payload: &Value) -> Vec<Value> {
    if let Some(entries) = list_payload["sprites"].as_array() {
        return entries.clone();
    }
    if let Some(entries) = list_payload["instances"].as_array() {
        return entries.clone();
    }
    if let Some(entries) = list_payload.as_array() {
        return entries.to_vec();
    }
    panic!("sprite_list payload carries no entry array: {list_payload}");
}

// WAVE13-GAP: sprite_list — an entry's asset index.
fn entry_asset(entry: &Value) -> u64 {
    entry["asset"]
        .as_u64()
        .unwrap_or_else(|| panic!("sprite entry carries no asset index: {entry}"))
}

// WAVE13-GAP: sprite_list — an entry's key count, as a
// `keys` array's length or an explicit count field.
fn entry_key_count(entry: &Value) -> u64 {
    if let Some(keys) = entry["keys"].as_array() {
        return keys.len() as u64;
    }
    for field in ["key_count", "keys_count"] {
        if let Some(n) = entry[field].as_u64() {
            return n;
        }
    }
    panic!("sprite entry carries no key count: {entry}");
}

// WAVE13-GAP: sprite_list — the call itself (the tool is
// unregistered until the splice, so this fails loudly
// through call_ok until then; that is deliberate — no
// GAP test can pass green against a missing surface).
fn sprite_list(mcp: &mut Dispatch) -> Vec<Value> {
    sprite_entries(&mcp.call_ok("sprite_list", json!({})))
}

// WAVE13-GAP: sprite_add — the asset argument's spelling.
fn sprite_add(mcp: &mut Dispatch, asset: u64) -> Value {
    mcp.call_ok("sprite_add", json!({ "asset": asset }))
}

// WAVE13-GAP: sprite_add — the refusal path.
fn sprite_add_err(mcp: &mut Dispatch, asset: u64) -> String {
    mcp.call_err("sprite_add", json!({ "asset": asset }))
}

// WAVE13-GAP: sprite_remove — the index argument's spelling.
fn sprite_remove(mcp: &mut Dispatch, index: u64) -> Value {
    mcp.call_ok("sprite_remove", json!({ "sprite": index }))
}

// WAVE13-GAP: sprite_remove — the refusal path.
fn sprite_remove_err(mcp: &mut Dispatch, index: u64) -> String {
    mcp.call_err("sprite_remove", json!({ "sprite": index }))
}

// WAVE13-GAP: sprite_set_key — the argument spellings:
// sprite as `sprite` (A4's landed spelling), the transform as frame / x / y /
// rotation / scale (rotation lands in InstanceKey::angle).
#[allow(clippy::too_many_arguments)]
fn sprite_set_key(
    mcp: &mut Dispatch,
    index: u64,
    frame: f64,
    x: f64,
    y: f64,
    rotation: f64,
    scale: f64,
) -> Value {
    mcp.call_ok(
        "sprite_set_key",
        json!({
            "sprite": index,
            "frame": frame,
            "x": x,
            "y": y,
            "rotation": rotation,
            "scale": scale,
        }),
    )
}

// WAVE13-GAP: sprite_set_key — the refusal path.
#[allow(clippy::too_many_arguments)]
fn sprite_set_key_err(
    mcp: &mut Dispatch,
    index: u64,
    frame: f64,
    x: f64,
    y: f64,
    rotation: f64,
    scale: f64,
) -> String {
    mcp.call_err(
        "sprite_set_key",
        json!({
            "sprite": index,
            "frame": frame,
            "x": x,
            "y": y,
            "rotation": rotation,
            "scale": scale,
        }),
    )
}

// WAVE13-GAP: sprite_delete_key — the argument spellings.
fn sprite_delete_key(mcp: &mut Dispatch, index: u64, frame: f64) -> Value {
    mcp.call_ok(
        "sprite_delete_key",
        json!({ "sprite": index, "frame": frame }),
    )
}

// WAVE13-GAP: sprite_delete_key — the refusal path.
fn sprite_delete_key_err(mcp: &mut Dispatch, index: u64, frame: f64) -> String {
    mcp.call_err(
        "sprite_delete_key",
        json!({ "sprite": index, "frame": frame }),
    )
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
    // The floor is the Wave 12 census, NOT the Wave 13
    // total: the five sprite tools take the surface to
    // 288, but the splice may land after this suite, and a
    // floor assertion stays true on both sides of it.
    assert!(
        tools.len() >= 283,
        "the entering surface is the Wave 12 census of 283 tools, got {}",
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
    // The shipped surface this suite drives around the
    // sprite tools: the Wave 12 asset pair (a sprite needs
    // an asset, and asset_remove is the cross-surface
    // witness), session setup, the undo pair, status and
    // the IO witness.
    for required in [
        "new_project",
        "add_frame",
        "asset_import",
        "assets_list",
        "asset_remove",
        "get_editor_status",
        "undo",
        "redo",
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
        json!({ "name": "wave13_no_such_tool", "arguments": {} }),
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

// ---------------------------------------------------------------------
// The sprite surface — WAVE13-GAP steps, flipped at the splice
// ---------------------------------------------------------------------

/// Add a sprite over an imported asset: the list shows it,
/// the seeded key is the frame-0 identity key, and the
/// add is exactly one undo step — one undo removes the
/// instance whole, redo restores it.
// WAVE13-GAP: sprite_add, sprite_list — unregistered on
// the entering surface until A4's splice.
#[test]
fn sprite_add_list_undo_redo_session() {
    let dir = temp_dir("add");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Sprites" }));

    // A fresh project lists no sprites.
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 0, "a fresh project has no sprites");

    // A sprite needs an asset: import the fixture through
    // the shipped Wave 12 tool.
    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));

    // The add: exactly one undo step.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_add
    sprite_add(&mut mcp, 0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "sprite_add is exactly one undo step"
    );

    // The list shows the instance: asset 0, one key.
    // WAVE13-GAP: sprite_list
    let entries = sprite_list(&mut mcp);
    assert_eq!(entries.len(), 1, "one sprite after one add: {entries:?}");
    assert_eq!(entry_asset(&entries[0]), 0, "the sprite references asset 0");
    assert_eq!(entry_key_count(&entries[0]), 1, "the sprite seeds exactly one key");

    // The file witness: one instance over asset 0 whose
    // only key sits at frame 0 with the identity transform
    // (A1's charter — scale 1, angle 0; the centre position
    // itself is the stage's business and is not pinned).
    let sprites = saved_sprites(&mut mcp, &dir, "after-add.styk.json");
    assert_eq!(sprites.len(), 1, "the saved project carries the sprite");
    assert_eq!(sprites[0].asset, 0);
    assert_eq!(sprites[0].keys.len(), 1, "exactly one seeded key");
    let seed = sprites[0].keys[0];
    assert_close(seed.frame, 0.0, "the seeded key sits at frame 0");
    assert_close(seed.scale, 1.0, "the seeded key's scale is the identity 1");
    assert_close(seed.angle, 0.0, "the seeded key's angle is the identity 0");

    // ONE undo removes the whole add: the instance leaves
    // the table, the imported asset stays.
    mcp.call_ok("undo", json!({}));
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 0, "one undo removed the sprite");
    let undone = saved_sprites(&mut mcp, &dir, "after-undo.styk.json");
    assert_eq!(undone.len(), 0, "the saved project agrees: no sprites");

    // Redo puts the instance back, key and all.
    mcp.call_ok("redo", json!({}));
    let redone = saved_sprites(&mut mcp, &dir, "after-redo.styk.json");
    assert_eq!(redone.len(), 1, "redo restored the sprite");
    assert_eq!(redone[0].asset, 0);
    assert_eq!(redone[0].keys.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

/// The add refusals: no asset at all, and indices past the
/// end of the asset table — each an isError result that
/// spends no undo step and creates nothing.
// WAVE13-GAP: sprite_add, sprite_list — unregistered on
// the entering surface until the splice.
#[test]
fn sprite_add_refusals_spend_no_undo() {
    let dir = temp_dir("refusals");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Refusals" }));

    // With an EMPTY asset table every add refuses — there
    // is no asset 0 to reference.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_add
    let err = sprite_add_err(&mut mcp, 0);
    assert!(!err.is_empty(), "the refusal says why: {err}");
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a refused add spends no undo step"
    );
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 0, "a refused add created nothing");

    // Import one asset; indices past it refuse the same way.
    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_add
    sprite_add_err(&mut mcp, 1);
    // WAVE13-GAP: sprite_add
    sprite_add_err(&mut mcp, 99);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "out-of-range adds spend no undo step"
    );
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 0, "the table is still empty");
    let sprites = saved_sprites(&mut mcp, &dir, "after-refusals.styk.json");
    assert_eq!(sprites.len(), 0, "the saved project agrees: no sprites");

    let _ = fs::remove_dir_all(&dir);
}

/// Set a key at a new frame, read the values back through
/// the file witness AND through the model's own sampling;
/// re-setting the same frame replaces the key; deleting
/// the key removes exactly it. Writes addressed to a
/// sprite that does not exist refuse and spend nothing.
// WAVE13-GAP: sprite_add, sprite_set_key, sprite_delete_key
// — unregistered on the entering surface until the splice.
#[test]
fn sprite_set_key_sample_and_delete_key_session() {
    let dir = temp_dir("keys");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Keys" }));
    // Six frames, so frame 4 is in range under any sane
    // frame validation the op applies.
    for _ in 0..5 {
        mcp.call_ok("add_frame", json!({}));
    }
    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    // WAVE13-GAP: sprite_add
    sprite_add(&mut mcp, 0);

    // The set: one undo step, a second key at frame 4 with
    // the exact transform written.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_set_key
    sprite_set_key(&mut mcp, 0, 4.0, 120.5, -30.25, 30.0, 1.5);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "sprite_set_key is exactly one undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-set.styk.json");
    assert_eq!(sprites.len(), 1);
    assert_eq!(sprites[0].keys.len(), 2, "the set inserted a second key");
    let seed = sprites[0].keys[0];
    assert_close(seed.frame, 0.0, "the seeded key is still at frame 0");
    let written = sprites[0]
        .keys
        .iter()
        .find(|k| (k.frame - 4.0).abs() < 1e-4)
        .expect("a key exists at frame 4");
    assert_key(written, 4.0, 120.5, -30.25, 1.5, 30.0, "the frame-4 key");

    // Read-back through the model's sampling: at frame 4
    // the sample IS the written key; halfway between the
    // keys the interpolated fields are the exact midpoints
    // of the witnessed endpoints (the position midpoint is
    // computed from the witnessed seed, whose centre this
    // suite deliberately never pins).
    let keys = &sprites[0].keys;
    let at_key = InstanceKey::sample(keys, 4.0).expect("sampling at a keyed frame");
    assert_key(&at_key, 4.0, 120.5, -30.25, 1.5, 30.0, "the sample at frame 4");
    let mid = InstanceKey::sample(keys, 2.0).expect("sampling between keys");
    assert_close(mid.x, (seed.x + 120.5) / 2.0, "the frame-2 sample x is the midpoint");
    assert_close(mid.y, (seed.y + -30.25) / 2.0, "the frame-2 sample y is the midpoint");
    assert_close(mid.scale, 1.25, "the frame-2 sample scale is the midpoint");
    assert_close(mid.angle, 15.0, "the frame-2 sample angle is the midpoint");

    // Re-setting the SAME frame replaces the key — the
    // count does not grow, the values are the new ones,
    // and it is still exactly one undo step.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_set_key
    sprite_set_key(&mut mcp, 0, 4.0, 200.0, 40.0, -15.0, 0.5);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "replacing a key is exactly one undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-reset.styk.json");
    assert_eq!(sprites[0].keys.len(), 2, "replacement did not add a key");
    let replaced = InstanceKey::sample(&sprites[0].keys, 4.0).expect("sampling the replaced key");
    assert_key(&replaced, 4.0, 200.0, 40.0, 0.5, -15.0, "the replaced frame-4 key");

    // Writes addressed to a sprite that does not exist
    // refuse: no undo step, no state change.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_set_key
    sprite_set_key_err(&mut mcp, 99, 4.0, 0.0, 0.0, 0.0, 1.0);
    // WAVE13-GAP: sprite_delete_key
    sprite_delete_key_err(&mut mcp, 99, 4.0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "refused key writes spend no undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-key-refusals.styk.json");
    assert_eq!(sprites[0].keys.len(), 2, "the refused writes changed nothing");

    // Delete the frame-4 key: one undo step, exactly that
    // key goes, the seeded key remains.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_delete_key
    sprite_delete_key(&mut mcp, 0, 4.0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "sprite_delete_key is exactly one undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-delete.styk.json");
    assert_eq!(sprites[0].keys.len(), 1, "exactly the frame-4 key was deleted");
    assert_close(sprites[0].keys[0].frame, 0.0, "the seeded key remains");

    // One undo restores the deleted key.
    mcp.call_ok("undo", json!({}));
    let sprites = saved_sprites(&mut mcp, &dir, "after-key-undo.styk.json");
    assert_eq!(sprites[0].keys.len(), 2, "one undo restored the deleted key");

    let _ = fs::remove_dir_all(&dir);
}

/// Remove sprites one by one, and the cross-surface rule:
/// while any sprite references the asset, the shipped
/// `asset_remove` refuses; once the last sprite goes, the
/// same call succeeds.
// WAVE13-GAP: sprite_add, sprite_remove, sprite_list —
// unregistered on the entering surface until the splice
// (asset_remove itself is shipped Wave 12 surface).
#[test]
fn sprite_remove_and_asset_reference_session() {
    let dir = temp_dir("remove");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Remove" }));

    let png_path = write_fixture(&dir, "dot.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    // WAVE13-GAP: sprite_add
    sprite_add(&mut mcp, 0);
    // WAVE13-GAP: sprite_add
    sprite_add(&mut mcp, 0);
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 2, "two sprites over one asset");

    // The Wave 12 rule, with sprite references in play:
    // the asset cannot be removed from under its sprites.
    let err = mcp.call_err("asset_remove", json!({ "index": 0 }));
    assert!(!err.is_empty(), "the refusal says why: {err}");
    let project = {
        let path = dir.join("still-referenced.styk.json");
        mcp.call_ok("save_project", json!({ "path": path.to_string_lossy() }));
        read_project(&path)
    };
    assert_eq!(project.assets.len(), 1, "the refused removal kept the asset");
    assert_eq!(project.sprites.len(), 2, "the refused removal kept the sprites");

    // Remove one sprite: one undo step, one remains, and
    // the survivor still references asset 0.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_remove
    sprite_remove(&mut mcp, 0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "sprite_remove is exactly one undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-remove.styk.json");
    assert_eq!(sprites.len(), 1, "one sprite remains");
    assert_eq!(sprites[0].asset, 0, "the survivor still references asset 0");

    // A removal at an index that does not exist refuses
    // and spends nothing.
    let depth_before = undo_depth(&mut mcp);
    // WAVE13-GAP: sprite_remove
    sprite_remove_err(&mut mcp, 99);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a refused removal spends no undo step"
    );
    let sprites = saved_sprites(&mut mcp, &dir, "after-remove-refusal.styk.json");
    assert_eq!(sprites.len(), 1, "the refused removal changed nothing");

    // The asset is STILL referenced by the survivor, so
    // asset_remove still refuses…
    mcp.call_err("asset_remove", json!({ "index": 0 }));

    // …until the last sprite goes; then the same call
    // succeeds and the table is empty.
    // WAVE13-GAP: sprite_remove
    sprite_remove(&mut mcp, 0);
    // WAVE13-GAP: sprite_list
    assert_eq!(sprite_list(&mut mcp).len(), 0, "no sprites remain");
    mcp.call_ok("asset_remove", json!({ "index": 0 }));
    let project = {
        let path = dir.join("all-removed.styk.json");
        mcp.call_ok("save_project", json!({ "path": path.to_string_lossy() }));
        read_project(&path)
    };
    assert_eq!(project.assets.len(), 0, "the unreferenced asset was removed");
    assert_eq!(project.sprites.len(), 0, "the saved project agrees: no sprites");

    let _ = fs::remove_dir_all(&dir);
}
