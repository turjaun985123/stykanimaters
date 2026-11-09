//! Wave 14, Squad E (session E1) — THE MCP-ONLY BUILD: the
//! wave's flagship proof. One complete small project is
//! built using NOTHING but MCP tool calls (`McpServer::
//! handle`, the real dispatch both transports share), every
//! subsystem that has tools is touched, and at each stage
//! the state is asserted through READ tools only — a write
//! whose effect no read tool can see is a parity failure,
//! and this suite is written to catch exactly that.
//!
//! The build script, in order (the tool per step is named
//! in the stage comments of `mcp_only_full_project_build`):
//!
//!   1. `new_project`            — project create
//!   2. `add_frame` × 3          — a 4-frame timeline
//!   3. `add_figure`             — a second figure
//!   4. `set_pose` × 3           — pose keys across frames
//!   5. `graph_key` × 2 +
//!      `set_key_interp`         — graph channel + easing
//!   6. `draw_stroke`            — a drawn stroke
//!   7. `add_fx_clip` +
//!      `particle_add_event`     — FX clip + particle event
//!   8. `add_audio_clip`         — a generated WAV fixture
//!   9. `asset_import` +
//!      `sprite_add` /
//!      `sprite_set_key`         — asset + placed sprite
//!  10. `fig3_enter` /
//!      `fig3_select` /
//!      `fig3_rotate`            — a 3D figure touch
//!  11. `clip_create`            — a movieclip placed on
//!                                 (looping) figure 0
//!  12. `set_camera`             — a camera key
//!  13. `set_weather`            — scene weather
//!  14. `save_project`           — save via tools
//!  15. fresh server +
//!      `load_project`           — load via tools, then the
//!                                 whole read-back repeats
//!  16. `render`                 — PNG frame, SVG set and a
//!                                 PNG frame-range, via tools
//!
//! Weather and the roster: A9's remit includes scene
//! weather tools, but the weather step does NOT depend on
//! A9 — `set_weather` already ships in the entering
//! registry (`src/mcp_tools.rs`, with `kind` / `amount` /
//! `wind`), so the step is written against the shipped
//! tool. No step in this build is left depending on an
//! unlanded spec.
//!
//! READ-VISIBILITY FINDINGS (the parity failures this
//! suite was built to hunt — both are pinned in the test
//! at the strength the surface actually supports, and
//! named here so the integrator and E10 see them):
//!
//!   F1. `set_weather`'s values are invisible to
//!       `get_state`. MCP.md's `get_state` row promises
//!       "weather state (kind, amount, wind)", but the
//!       handler serialises `project.weather.is_some()`
//!       (`src/mcp.rs`, the get_state reply) — a bare
//!       boolean. The kind / amount / wind ARE readable,
//!       but only via `project_summary`, whose summary
//!       carries the full weather object
//!       (`src/mcp_io.rs` `summary()`). The build asserts
//!       both: the boolean in `get_state`, the values in
//!       `project_summary`. The fix is one line in the
//!       `get_state` handler (serialise the object, as
//!       `summary()` does) — E10 / A9 territory, not this
//!       file's.
//!   F2. CLOSED BY A7, recorded so the history is honest:
//!       before Wave 14, `fig3_rotate`'s effect was not
//!       readable — `fig3_get` reported joint positions
//!       only. A7's in-tree change adds each joint's
//!       stored Euler `rotation` to the summary
//!       (`src/fig3d.rs`, `fig3mcp_summary`), and the
//!       build asserts the rotated joint's rotation
//!       strictly. On a tree without A7's change this
//!       stage fails — that is the dependency, and it has
//!       landed.
//!
//! Standing contracts pinned along the way (the house
//! rules, carried from the Wave 12/13 suites):
//!   * every mutating tool is exactly ONE undo step where
//!     this suite spot-checks it (observed through
//!     `get_editor_status`'s undo depth);
//!   * an unknown tool name is a protocol error, `-32602`;
//!   * value witnesses are strict where the reply shape is
//!     pinned by source read at writing time (`get_state`,
//!     `get_pose`, `add_figure`, `graph_key`, `clip_create`,
//!     `clip_list`, `fig3_get`, `project_summary`); the
//!     list-style reads whose field spellings are the
//!     panels' own (`fx_list`, `particle_list_events`,
//!     `list_strokes`, `audio_list_clips`, `get_camera`)
//!     are funnelled through the tolerant extractors in
//!     the "Funnels" section — the ONLY tolerant code in
//!     the file, same doctrine as the Wave 13 suite.
//!
//! Every step goes through MCP messages; direct filesystem
//! access is used only for what an MCP client itself
//! would observe from outside the protocol: writing the
//! import fixtures (PNG + WAV) and reading the exported
//! files the tools produced. Each test builds its own
//! server and temp dir and shares nothing. No test
//! changes the process current directory; every path
//! passed to a tool is absolute.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;

/// The canonical 1×1 transparent PNG (67 bytes) — the same
/// fixture the Wave 12/13 suites drive: real enough that
/// the image decoder accepts it.
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

/// A tiny but real WAV: PCM16 mono, 8000 Hz, 0.3 s of a
/// 440 Hz sine at moderate amplitude. `add_audio_clip`
/// refuses anything that is not a WAV, and the audio
/// reads probe the file's health and duration — a header
/// alone would not do; this must decode.
fn wav_fixture() -> Vec<u8> {
    let sample_rate: u32 = 8000;
    let samples: u32 = 2400; // 0.3 s
    let data_len = samples * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let v = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        out.extend_from_slice(&((v * 12000.0) as i16).to_le_bytes());
    }
    out
}

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 11 D-suite pattern,
// carried through Wave 12/13 unchanged)
// ---------------------------------------------------------------------

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave14-e2e-mcp-only-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write fixture file");
    path.to_string_lossy().into_owned()
}

struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

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
}

// ---------------------------------------------------------------------
// Shared witnesses
// ---------------------------------------------------------------------

fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "{what}: expected {expected}, got {actual}"
    );
}

fn f32_at(v: &Value, what: &str) -> f32 {
    v.as_f64()
        .unwrap_or_else(|| panic!("{what}: expected a number, got {v}")) as f32
}

// ---------------------------------------------------------------------
// Funnels — the ONLY tolerant code in this file
// ---------------------------------------------------------------------
//
// List-style reads whose entry spellings belong to the
// panels (fx_list, particle_list_events, list_strokes,
// audio_list_clips, get_camera, graph channel listing)
// are extracted here, so a spelling reconciliation
// happens in one place. Everything else in the file is
// strict, against shapes read from source.

// Sprite reads (the Wave 13 post-flip spellings, proven
// by that suite): the entry array under `sprites`, the
// asset under `asset`, the key count as a `keys` array's
// length or a count field.
fn sprite_entries(mcp: &mut Dispatch) -> Vec<Value> {
    let payload = mcp.call_ok("sprite_list", json!({}));
    if let Some(entries) = payload["sprites"].as_array() {
        return entries.clone();
    }
    if let Some(entries) = payload.as_array() {
        return entries.to_vec();
    }
    panic!("sprite_list payload carries no entry array: {payload}");
}

fn sprite_key_count(entry: &Value) -> u64 {
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

/// The graph channel for (figure, angle node): its index
/// in the project's channel list, from graph_list_channels.
/// The target may be spelled "angle:2" (string) or
/// {"angle": 2} (object); the key count may ride as a
/// `keys` array or a count field.
fn graph_channel_index(mcp: &mut Dispatch, figure: u64, node: u64) -> u64 {
    let payload = mcp.call_ok("graph_list_channels", json!({}));
    let channels = payload["channels"]
        .as_array()
        .unwrap_or_else(|| panic!("graph_list_channels carries no channels array: {payload}"));
    for (pos, ch) in channels.iter().enumerate() {
        if ch["figure"].as_u64() != Some(figure) {
            continue;
        }
        let target = &ch["target"];
        // The handler's real shape (graph_keys::gk_target_json):
        // { "target": "angle", "node": n, "target_label": … }.
        let matches = target["target"].as_str() == Some("angle")
            && target["node"].as_u64() == Some(node);
        if matches {
            return ch["index"].as_u64().unwrap_or(pos as u64);
        }
    }
    panic!("no graph channel for figure {figure} angle:{node} in {payload}");
}

/// One channel's keys, from graph_get_channel.
fn graph_channel_keys(mcp: &mut Dispatch, channel: u64) -> Vec<Value> {
    let payload = mcp.call_ok("graph_get_channel", json!({ "channel": channel }));
    payload["keys"]
        .as_array()
        .unwrap_or_else(|| panic!("graph_get_channel carries no keys array: {payload}"))
        .clone()
}

fn key_field(key: &Value, names: &[&str], what: &str) -> Value {
    for name in names {
        if !key[*name].is_null() {
            return key[*name].clone();
        }
    }
    panic!("{what}: none of {names:?} present in {key}");
}

/// The FX clip list, from fx_list (array under `clips` /
/// `fx`, or the payload as a bare array).
fn fx_entries(mcp: &mut Dispatch) -> Vec<Value> {
    let payload = mcp.call_ok("fx_list", json!({}));
    for field in ["clips", "fx", "entries"] {
        if let Some(entries) = payload[field].as_array() {
            return entries.clone();
        }
    }
    if let Some(entries) = payload.as_array() {
        return entries.to_vec();
    }
    panic!("fx_list payload carries no entry array: {payload}");
}

/// The particle event list, from particle_list_events.
fn particle_entries(mcp: &mut Dispatch) -> Vec<Value> {
    let payload = mcp.call_ok("particle_list_events", json!({}));
    for field in ["events", "particles", "entries"] {
        if let Some(entries) = payload[field].as_array() {
            return entries.clone();
        }
    }
    if let Some(entries) = payload.as_array() {
        return entries.to_vec();
    }
    panic!("particle_list_events payload carries no entry array: {payload}");
}

/// An event's stage position, as (x, y): flat fields or a
/// `position` pair.
fn entry_position(entry: &Value) -> (f32, f32) {
    if !entry["position"].is_null() {
        let p = &entry["position"];
        if let Some(arr) = p.as_array() {
            return (f32_at(&arr[0], "event position x"), f32_at(&arr[1], "event position y"));
        }
        return (f32_at(&p["x"], "event position x"), f32_at(&p["y"], "event position y"));
    }
    (f32_at(&entry["x"], "event x"), f32_at(&entry["y"], "event y"))
}

/// The audio clip list, from audio_list_clips.
fn audio_entries(mcp: &mut Dispatch) -> Vec<Value> {
    let payload = mcp.call_ok("audio_list_clips", json!({}));
    for field in ["clips", "audio", "entries"] {
        if let Some(entries) = payload[field].as_array() {
            return entries.clone();
        }
    }
    if let Some(entries) = payload.as_array() {
        return entries.to_vec();
    }
    panic!("audio_list_clips payload carries no entry array: {payload}");
}

/// A clip's start frame: `start_frame` or `start`.
/// FX starts are fractional frames (f32 in the model),
/// audio starts are whole frames — read both as f64.
fn entry_start(entry: &Value) -> f64 {
    key_field(entry, &["start_frame", "start"], "audio/fx entry start")
        .as_f64()
        .expect("entry start is a frame number")
}

/// The camera keys, from get_camera.
fn camera_keys(mcp: &mut Dispatch, frame: u64) -> Vec<Value> {
    let payload = mcp.call_ok("get_camera", json!({ "frame": frame }));
    payload["keys"]
        .as_array()
        .unwrap_or_else(|| panic!("get_camera carries no keys array: {payload}"))
        .clone()
}

/// The interpolated camera at a frame, from get_camera:
/// the {x, y, zoom} object under `camera`.
fn camera_at(mcp: &mut Dispatch, frame: u64) -> (f32, f32, f32) {
    let payload = mcp.call_ok("get_camera", json!({ "frame": frame }));
    let cam = &payload["camera"];
    assert!(cam.is_object(), "get_camera carries a camera object: {payload}");
    (
        f32_at(&cam["x"], "camera x"),
        f32_at(&cam["y"], "camera y"),
        f32_at(&cam["zoom"], "camera zoom"),
    )
}

// ---------------------------------------------------------------------
// The entering baseline — live on today's surface
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
        tools.len() >= 288,
        "the entering surface is the Wave 13 census of 288 tools, got {}",
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
    // Every tool the MCP-only build below drives must be
    // on the shipped surface — the build is a proof about
    // THIS surface, not a wish list.
    for required in [
        "new_project",
        "add_frame",
        "set_frame",
        "add_figure",
        "list_figures",
        "set_pose",
        "get_pose",
        "graph_key",
        "graph_list_channels",
        "graph_get_channel",
        "set_key_interp",
        "draw_stroke",
        "list_strokes",
        "add_fx_clip",
        "fx_list",
        "particle_add_event",
        "particle_list_events",
        "add_audio_clip",
        "audio_list_clips",
        "audio_clip_info",
        "asset_import",
        "assets_list",
        "sprite_add",
        "sprite_list",
        "sprite_set_key",
        "fig3_enter",
        "fig3_select",
        "fig3_rotate",
        "fig3_get",
        "fig3_exit",
        "clip_create",
        "clip_list",
        "set_camera",
        "get_camera",
        "set_weather",
        "get_state",
        "get_editor_status",
        "project_summary",
        "save_project",
        "load_project",
        "render",
        "render_frame_base64",
    ] {
        assert!(
            tool_names.contains(required),
            "tools/list is missing {required}, which the MCP-only build drives"
        );
    }

    // The standing unknown-tool contract: a protocol
    // error -32602, never a tool result.
    let response = mcp.request(
        "tools/call",
        json!({ "name": "wave14_no_such_tool", "arguments": {} }),
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
}

// ---------------------------------------------------------------------
// THE MCP-ONLY BUILD — one project, every subsystem,
// nothing but MCP calls
// ---------------------------------------------------------------------

#[test]
fn mcp_only_full_project_build() {
    let dir = temp_dir("build");
    let mut mcp = Dispatch::new();

    // -- Stage 1: project create (new_project) ----------
    mcp.call_ok(
        "new_project",
        json!({ "name": "Wave14 MCP-Only Build", "fps": 12, "width": 960, "height": 540 }),
    );
    let state = mcp.call_ok("get_state", json!({}));
    assert_eq!(state["project"], "Wave14 MCP-Only Build", "get_state: {state}");
    assert_eq!(state["frame_count"].as_u64(), Some(1));
    assert_eq!(state["stage"], json!([960.0, 540.0]), "get_state: {state}");
    assert_eq!(
        state["figures"].as_array().expect("get_state figures").len(),
        1,
        "a fresh project has the one default figure"
    );
    assert_eq!(state["weather"], false, "no weather on a fresh project");

    // -- Stage 2: the timeline (add_frame) ---------------
    for _ in 0..3 {
        mcp.call_ok("add_frame", json!({}));
    }
    mcp.call_ok("set_frame", json!({ "frame": 0 }));
    let state = mcp.call_ok("get_state", json!({}));
    assert_eq!(state["frame_count"].as_u64(), Some(4), "four frames after three adds");

    // -- Stage 3: a second figure (add_figure) -----------
    let added = mcp.call_ok(
        "add_figure",
        json!({ "kind": "mini", "name": "Sidekick", "color": 0x3399FF }),
    );
    assert_eq!(added["figure"].as_u64(), Some(1), "add_figure reply: {added}");
    assert_eq!(added["name"], "Sidekick");
    let figures = mcp.call_ok("list_figures", json!({}));
    let figure_rows = figures["figures"]
        .as_array()
        .unwrap_or_else(|| panic!("list_figures carries no figures array: {figures}"));
    assert_eq!(figure_rows.len(), 2, "two figures after the add");
    assert_eq!(figure_rows[1]["name"], "Sidekick");

    // -- Stage 4: pose keys across frames (set_pose) -----
    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok(
        "set_pose",
        json!({
            "figure": 0, "frame": 0,
            "root": [480.0, 300.0],
            "angles": { "1": -30.0, "2": 25.0 },
        }),
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "set_pose is exactly one undo step"
    );
    mcp.call_ok(
        "set_pose",
        json!({
            "figure": 0, "frame": 2,
            "root": [520.0, 300.0],
            "angles": { "1": 15.0, "2": -10.0 },
        }),
    );
    mcp.call_ok(
        "set_pose",
        json!({ "figure": 1, "frame": 0, "angles": { "1": 40.0 } }),
    );
    // Read back through get_pose only.
    let pose = mcp.call_ok("get_pose", json!({ "figure": 0, "frame": 0 }));
    let root = pose["root"].as_array().expect("get_pose root");
    assert_close(f32_at(&root[0], "pose root x"), 480.0, "frame-0 root x");
    assert_close(f32_at(&root[1], "pose root y"), 300.0, "frame-0 root y");
    let angles = pose["angles"].as_array().expect("get_pose angles");
    assert_close(f32_at(&angles[1], "pose angle 1"), -30.0, "frame-0 angle 1");
    assert_close(f32_at(&angles[2], "pose angle 2"), 25.0, "frame-0 angle 2");
    let pose2 = mcp.call_ok("get_pose", json!({ "figure": 0, "frame": 2 }));
    let angles2 = pose2["angles"].as_array().expect("get_pose angles (frame 2)");
    assert_close(f32_at(&angles2[1], "frame-2 angle 1"), 15.0, "frame-2 angle 1");
    assert_close(f32_at(&angles2[2], "frame-2 angle 2"), -10.0, "frame-2 angle 2");
    let pose_b = mcp.call_ok("get_pose", json!({ "figure": 1, "frame": 0 }));
    let angles_b = pose_b["angles"].as_array().expect("get_pose angles (figure 1)");
    assert_close(f32_at(&angles_b[1], "figure-1 angle 1"), 40.0, "figure-1 angle 1");

    // -- Stage 5: graph channel + easing -----------------
    // (graph_key, set_key_interp; read via
    // graph_list_channels / graph_get_channel)
    let g1 = mcp.call_ok(
        "graph_key",
        json!({ "figure": 0, "target": "angle:2", "frame": 0, "value": 25.0 }),
    );
    assert_eq!(g1["channel_keys"].as_u64(), Some(1), "graph_key reply: {g1}");
    let g2 = mcp.call_ok(
        "graph_key",
        json!({ "figure": 0, "target": "angle:2", "frame": 3, "value": 60.0 }),
    );
    assert_eq!(g2["channel_keys"].as_u64(), Some(2), "graph_key reply: {g2}");
    let channel = graph_channel_index(&mut mcp, 0, 2);
    let keys = graph_channel_keys(&mut mcp, channel);
    assert_eq!(keys.len(), 2, "the channel carries both keys: {keys:?}");
    let key_at = |frame: f64| {
        keys.iter()
            .find(|k| (k["frame"].as_f64().unwrap_or(-1.0) - frame).abs() < 1e-4)
            .unwrap_or_else(|| panic!("no graph key at frame {frame} in {keys:?}"))
            .clone()
    };
    assert_close(f32_at(&key_at(0.0)["value"], "graph key value"), 25.0, "graph key at 0");
    assert_close(f32_at(&key_at(3.0)["value"], "graph key value"), 60.0, "graph key at 3");
    // The easing write: key 0 (frame order) becomes linear…
    mcp.call_ok(
        "set_key_interp",
        json!({ "channel": channel, "key": 0, "interp": "linear" }),
    );
    // …and the read tool sees it.
    let keys = graph_channel_keys(&mut mcp, channel);
    let first = keys
        .iter()
        .find(|k| k["frame"].as_f64().map_or(false, |f| f.abs() < 1e-4))
        .expect("the frame-0 key survives the interp write");
    let interp = key_field(first, &["interp", "interpolation"], "graph key interp");
    assert_eq!(
        interp.as_str().unwrap_or_default(),
        "linear",
        "set_key_interp is visible in graph_get_channel: {first}"
    );

    // -- Stage 6: a drawn stroke (draw_stroke) -----------
    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok(
        "draw_stroke",
        json!({
            "frame": 0,
            "points": [[120.0, 420.0], [300.0, 400.0], [480.0, 430.0]],
            "color": 0xFF0000,
            "width": 6,
        }),
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "draw_stroke is exactly one undo step"
    );
    let strokes = mcp.call_ok("list_strokes", json!({ "frame": 0 }));
    let stroke_rows = strokes["strokes"]
        .as_array()
        .unwrap_or_else(|| panic!("list_strokes carries no strokes array: {strokes}"));
    assert_eq!(stroke_rows.len(), 1, "one stroke on frame 0: {stroke_rows:?}");
    assert_eq!(
        stroke_rows[0]["color"].as_u64(),
        Some(0xFF0000),
        "the stroke's colour reads back: {stroke_rows:?}"
    );
    // The entry's `points` field IS the count when the
    // polyline itself was not requested (draw_ops'
    // stroke JSON, read from source).
    let points_field = &stroke_rows[0]["points"];
    let point_count = points_field
        .as_u64()
        .or_else(|| points_field.as_array().map(|a| a.len() as u64))
        .unwrap_or_else(|| panic!("stroke entry carries no point count: {stroke_rows:?}"));
    assert_eq!(point_count, 3, "the stroke's three points read back");

    // -- Stage 7: FX clip + particle event ---------------
    mcp.call_ok(
        "add_fx_clip",
        json!({ "figure": 0, "node": 1, "kind": "burst", "start": 0, "duration": 3 }),
    );
    let fx = fx_entries(&mut mcp);
    assert_eq!(fx.len(), 1, "one FX clip after the add: {fx:?}");
    assert_eq!(entry_start(&fx[0]), 0.0, "the FX clip starts at frame 0");
    assert_eq!(fx[0]["duration"].as_f64(), Some(3.0), "the FX clip's duration reads back");
    let kind = fx[0]["kind"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(kind.contains("burst"), "the FX clip's kind reads back: {fx:?}");

    mcp.call_ok(
        "particle_add_event",
        json!({ "frame": 1, "x": 200.0, "y": 180.0 }),
    );
    let events = particle_entries(&mut mcp);
    let event = events
        .iter()
        .find(|e| e["frame"].as_u64() == Some(1))
        .unwrap_or_else(|| panic!("no particle event at frame 1 in {events:?}"));
    let (ex, ey) = entry_position(event);
    assert_close(ex, 200.0, "the particle event's x");
    assert_close(ey, 180.0, "the particle event's y");

    // -- Stage 8: audio (add_audio_clip, WAV fixture) ----
    let wav_path = write_fixture(&dir, "tone.wav", &wav_fixture());
    mcp.call_ok(
        "add_audio_clip",
        json!({ "path": wav_path, "start_frame": 1 }),
    );
    let clips = audio_entries(&mut mcp);
    assert_eq!(clips.len(), 1, "one audio clip after the add: {clips:?}");
    assert_eq!(entry_start(&clips[0]), 1.0, "the audio clip starts at frame 1");
    assert!(
        clips[0]["path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("tone.wav"),
        "the audio clip's path reads back: {clips:?}"
    );
    if let Some(health) = clips[0]["health"].as_str() {
        assert_eq!(health, "healthy", "the generated WAV probes healthy: {clips:?}");
    }
    let info = mcp.call_ok("audio_clip_info", json!({ "index": 0 }));
    let duration = key_field(
        &info,
        &["duration_secs", "duration_seconds", "duration"],
        "audio duration",
    );
    assert!(
        duration.as_f64().unwrap_or(0.0) > 0.1,
        "the WAV's duration is readable and real: {info}"
    );

    // -- Stage 9: asset + sprite -------------------------
    let png_path = write_fixture(&dir, "badge.png", PNG_1X1);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    let assets = mcp.call_ok("assets_list", json!({}));
    let asset_rows = assets["assets"]
        .as_array()
        .unwrap_or_else(|| panic!("assets_list carries no assets array: {assets}"));
    assert_eq!(asset_rows.len(), 1, "one asset after the import");

    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok("sprite_add", json!({ "asset": 0 }));
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "sprite_add is exactly one undo step"
    );
    mcp.call_ok(
        "sprite_set_key",
        json!({
            "sprite": 0, "frame": 2,
            "x": 150.0, "y": 120.0, "rotation": 20.0, "scale": 1.25,
        }),
    );
    let sprites = sprite_entries(&mut mcp);
    assert_eq!(sprites.len(), 1, "one sprite after the add");
    assert_eq!(sprites[0]["asset"].as_u64(), Some(0), "the sprite references asset 0");
    assert_eq!(sprite_key_count(&sprites[0]), 2, "the sprite carries both keys");

    // -- Stage 10: a 3D figure touch ---------------------
    // (fig3_enter / fig3_select / fig3_rotate; read via
    // fig3_get — the rotation read is A7's landed field,
    // finding F2 in the header)
    mcp.call_ok("fig3_enter", json!({}));
    let fig3 = mcp.call_ok("fig3_get", json!({}));
    assert_eq!(fig3["active"], true, "fig3_get reports the session: {fig3}");
    mcp.call_ok("fig3_select", json!({ "joint": 1 }));
    mcp.call_ok("fig3_rotate", json!({ "axis": 2, "degrees": 30.0 }));
    let fig3 = mcp.call_ok("fig3_get", json!({}));
    assert_eq!(fig3["selected"].as_u64(), Some(1), "the selected joint reads back");
    let joints = fig3["joints"].as_array().expect("fig3_get joints");
    let rotation = joints[1]["rotation"]
        .as_array()
        .unwrap_or_else(|| panic!("fig3 joint carries no rotation (finding F2): {joints:?}"));
    assert_close(
        f32_at(&rotation[2], "joint rotation z"),
        30.0,
        "the rotated joint's Z rotation reads back",
    );
    mcp.call_ok("fig3_exit", json!({}));
    let fig3 = mcp.call_ok("fig3_get", json!({}));
    assert_eq!(fig3["active"], false, "fig3_exit ends the session");

    // -- Stage 11: the movieclip (clip_create) -----------
    let created = mcp.call_ok("clip_create", json!({ "figure": 0 }));
    assert_eq!(
        created["loops_on_figure"], true,
        "clip_create sets the figure looping: {created}"
    );
    assert_eq!(created["frames"].as_u64(), Some(4), "the clip spans all four frames");
    let clip_list = mcp.call_ok("clip_list", json!({}));
    assert_eq!(clip_list["count"].as_u64(), Some(1), "one clip definition");
    let clip0 = &clip_list["clips"][0];
    assert_eq!(clip0["frames"].as_u64(), Some(4), "clip_list agrees on the frame count");
    assert!(
        clip0["used_by"]
            .as_array()
            .expect("clip used_by")
            .iter()
            .any(|u| u.as_u64() == Some(0)),
        "figure 0 loops the clip: {clip0}"
    );

    // -- Stage 12: a camera key (set_camera) -------------
    let depth_before = undo_depth(&mut mcp);
    mcp.call_ok(
        "set_camera",
        json!({ "frame": 0, "x": 480.0, "y": 270.0, "zoom": 1.25 }),
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "set_camera is exactly one undo step"
    );
    let (cx, cy, cz) = camera_at(&mut mcp, 0);
    assert_close(cx, 480.0, "the camera key's x");
    assert_close(cy, 270.0, "the camera key's y");
    assert_close(cz, 1.25, "the camera key's zoom");
    assert!(
        !camera_keys(&mut mcp, 0).is_empty(),
        "get_camera lists the written key"
    );

    // -- Stage 13: weather (set_weather) -----------------
    // NOT depends-on-A9: set_weather already ships. The
    // values are asserted through project_summary — see
    // finding F1 in the header for why get_state cannot
    // carry them.
    mcp.call_ok(
        "set_weather",
        json!({ "kind": "rain", "amount": 120, "wind": 15 }),
    );
    let state = mcp.call_ok("get_state", json!({}));
    assert_eq!(
        state["weather"], true,
        "get_state sees that weather exists (a boolean — finding F1)"
    );
    let summary = mcp.call_ok("project_summary", json!({}));
    let weather = &summary["weather"];
    assert!(weather.is_object(), "project_summary carries the weather object: {summary}");
    let wkind = weather["kind"].as_str().unwrap_or_default().to_lowercase();
    assert!(wkind.contains("rain"), "the weather kind reads back: {weather}");
    assert_close(f32_at(&weather["amount"], "weather amount"), 120.0, "the weather amount");
    assert_close(f32_at(&weather["wind"], "weather wind"), 15.0, "the weather wind");
    assert_eq!(summary["has_audio"], true, "the summary sees the audio clip");
    assert!(
        summary["camera_keys"].as_u64().unwrap_or(0) >= 1,
        "the summary counts the camera key: {summary}"
    );

    // -- Stage 14: save via tools (save_project) ---------
    let project_path = dir.join("mcp-only-build.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": project_path.to_string_lossy() }),
    );
    let saved_len = fs::metadata(&project_path)
        .expect("the saved project exists")
        .len();
    assert!(saved_len > 1000, "the saved project is a real document ({saved_len} bytes)");

    // -- Stage 15: fresh server, load via tools ----------
    let mut mcp = Dispatch::new();
    let loaded = mcp.call_ok(
        "load_project",
        json!({ "path": project_path.to_string_lossy() }),
    );
    assert_eq!(loaded["name"], "Wave14 MCP-Only Build", "load summary: {loaded}");

    // The whole build, re-read on the fresh session
    // through read tools only.
    let state = mcp.call_ok("get_state", json!({}));
    assert_eq!(state["frame_count"].as_u64(), Some(4), "the timeline survived the round trip");
    assert_eq!(
        state["figures"].as_array().expect("get_state figures").len(),
        2,
        "both figures survived the round trip"
    );
    assert_eq!(state["weather"], true, "the weather survived the round trip");
    let pose = mcp.call_ok("get_pose", json!({ "figure": 0, "frame": 0 }));
    let angles = pose["angles"].as_array().expect("get_pose angles after load");
    assert_close(f32_at(&angles[1], "loaded angle 1"), -30.0, "the pose survived the round trip");
    let channel = graph_channel_index(&mut mcp, 0, 2);
    assert_eq!(
        graph_channel_keys(&mut mcp, channel).len(),
        2,
        "the graph channel survived the round trip"
    );
    let strokes = mcp.call_ok("list_strokes", json!({ "frame": 0 }));
    assert_eq!(
        strokes["strokes"].as_array().expect("strokes after load").len(),
        1,
        "the stroke survived the round trip"
    );
    assert_eq!(fx_entries(&mut mcp).len(), 1, "the FX clip survived the round trip");
    assert_eq!(
        particle_entries(&mut mcp).len(),
        1,
        "the particle event survived the round trip"
    );
    assert_eq!(audio_entries(&mut mcp).len(), 1, "the audio clip survived the round trip");
    let sprites = sprite_entries(&mut mcp);
    assert_eq!(sprites.len(), 1, "the sprite survived the round trip");
    assert_eq!(sprite_key_count(&sprites[0]), 2, "the sprite's keys survived the round trip");
    let clip_list = mcp.call_ok("clip_list", json!({}));
    assert_eq!(clip_list["count"].as_u64(), Some(1), "the movieclip survived the round trip");
    assert!(
        !camera_keys(&mut mcp, 0).is_empty(),
        "the camera key survived the round trip"
    );

    // -- Stage 16: exports via tools (render) ------------
    // One PNG frame: exists, decodes at stage size, and
    // is NOT blank — the figures, the red stroke, the FX
    // and the rain guarantee real coverage over the
    // background.
    let png_path = dir.join("frame0.png");
    mcp.call_ok(
        "render",
        json!({ "path": png_path.to_string_lossy(), "format": "png", "frame": 0 }),
    );
    let img = image::open(&png_path).expect("the exported PNG decodes");
    assert_eq!(img.width(), 960, "the PNG is stage width");
    assert_eq!(img.height(), 540, "the PNG is stage height");
    let rgba = img.to_rgba8();
    let bg = rgba.get_pixel(0, 0).0;
    let non_bg = rgba
        .pixels()
        .filter(|p| {
            let q = p.0;
            (q[0] as i32 - bg[0] as i32).abs() > 40
                || (q[1] as i32 - bg[1] as i32).abs() > 40
                || (q[2] as i32 - bg[2] as i32).abs() > 40
        })
        .count();
    assert!(
        non_bg > 200,
        "the exported PNG is not blank: {non_bg} non-background pixels"
    );
    // The base64 read agrees on the frame's size.
    let shot = mcp.call_ok("render_frame_base64", json!({ "frame": 0 }));
    assert_eq!(shot["width"].as_u64(), Some(960), "render_frame_base64: {shot}");
    assert_eq!(shot["height"].as_u64(), Some(540));

    // The SVG set: one file per frame into a directory;
    // across the set, the figure geometry AND the media
    // (the placed sprite, as an embedded image) appear.
    let svg_dir = dir.join("svg");
    mcp.call_ok(
        "render",
        json!({ "path": svg_dir.to_string_lossy(), "format": "svg" }),
    );
    let mut svg_text = String::new();
    let mut svg_files = 0;
    for entry in fs::read_dir(&svg_dir).expect("the SVG directory exists") {
        let path = entry.expect("dir entry").path();
        if path.extension().map_or(false, |e| e == "svg") {
            svg_files += 1;
            svg_text.push_str(&fs::read_to_string(&path).expect("read SVG frame"));
        }
    }
    assert!(svg_files >= 1, "the SVG export wrote frame files");
    assert!(svg_text.contains("<svg"), "the SVG files are SVG documents");
    assert!(
        ["<line", "<path", "<circle", "<polyline"]
            .iter()
            .any(|m| svg_text.contains(m)),
        "the SVG set draws the figures"
    );
    assert!(
        svg_text.contains("<image") || svg_text.contains("data:image"),
        "the SVG set references the placed sprite's media"
    );

    // A frame-range export: png_sequence over frames
    // 1..=2 writes exactly those two frames.
    let seq_dir = dir.join("seq");
    mcp.call_ok(
        "render",
        json!({
            "path": seq_dir.to_string_lossy(),
            "format": "png_sequence",
            "from": 1,
            "to": 2,
        }),
    );
    let seq_pngs = fs::read_dir(&seq_dir)
        .expect("the sequence directory exists")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "png"))
        .count();
    assert_eq!(seq_pngs, 2, "the range export wrote exactly frames 1 and 2");

    let _ = fs::remove_dir_all(&dir);
}
