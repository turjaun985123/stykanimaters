//! Integration tests: Wave 11 end-to-end MCP session — DRAWING.
//!
//! One full session, driven ONLY through `McpServer` JSON-RPC
//! messages (the same dispatch the stdio binary and the live
//! control path share): initialise, create a project, draw a
//! plain stroke, draw a scattered stroke, draw every shape
//! tool, then read the scene back out through the SVG and PNG
//! render tools and check the geometry that actually landed.
//!
//! The scattered stroke is the Wave 10 contract under test: a
//! stroke with `scatter > 0` renders as individual dabs —
//! `paint::stroke_dabs` placements at the shared
//! `paint::stroke_dab_spacing` — on the stage, in the
//! rasterizer and in the SVG exporter alike. The SVG check
//! therefore counts dab circles against `stroke_dabs` itself:
//! the export must agree with the single placement source.
//!
//! Gaps this suite pinned while they were open — all three
//! closed by the Wave 11 splice, and the pins below flipped
//! to positive assertions the same wave:
//!
//! - Stamps and text: the Draw panel's arrow stamp is
//!   `draw_arrow`; stage texts have a full tool family
//!   (`add_text_instance`, `list_texts`, ...). Both are in
//!   the registry now.
//! - Stroke read-back: `list_strokes` lists strokes with
//!   their geometry/colour/scatter; the session uses it.
//!   (`get_project_stats` still carries no stroke count —
//!   that absence is asserted as a fact, no longer a gap.)
//! - Non-finite arguments: `draw_stroke` (and every handler
//!   sharing the argument helpers) refuses a wire `1e999`
//!   (which serde_json parses as +inf) instead of storing
//!   `scatter: f32::INFINITY`.
//!
//! Everything below uses only the public API and `std` plus
//! the crate's own `serde_json` dependency — no
//! dev-dependencies, matching `export_determinism.rs`.

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::Stroke;
use stykanimate_rs::paint;

// ---------------------------------------------------------------------
// JSON-RPC plumbing: every interaction in this file goes through
// McpServer::handle as a parsed JSON-RPC message, exactly what
// the stdio shell feeds it line by line.
// ---------------------------------------------------------------------

struct Session {
    server: McpServer,
    next_id: i64,
}

struct ToolReply {
    /// The handler's return value, parsed back out of the text
    /// content block (handlers return JSON text). For error
    /// replies this is the raw message as a JSON string.
    value: Value,
    is_error: bool,
    text: String,
}

impl Session {
    fn new() -> Self {
        Session {
            server: McpServer::new(),
            next_id: 1,
        }
    }

    fn rpc(&mut self, msg: &Value) -> Value {
        self.server
            .handle(msg)
            .expect("a request (it carries an id) must be answered")
    }

    /// One `tools/call`: asserts the envelope (jsonrpc tag, id
    /// echo, a single text content block) and unwraps it.
    fn call(&mut self, name: &str, args: Value) -> ToolReply {
        let id = self.next_id;
        self.next_id += 1;
        let resp = self.rpc(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": args },
        }));
        assert_eq!(resp["jsonrpc"], "2.0", "response is not JSON-RPC 2.0");
        assert_eq!(resp["id"], id, "response id does not echo the request id");
        let result = resp
            .get("result")
            .unwrap_or_else(|| panic!("tools/call {name} returned no result: {resp}"));
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("tools/call {name} has no text content block: {resp}"))
            .to_string();
        let value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
        ToolReply {
            value,
            is_error,
            text,
        }
    }

    fn call_ok(&mut self, name: &str, args: Value) -> Value {
        let reply = self.call(name, args);
        assert!(
            !reply.is_error,
            "tools/call {name} unexpectedly failed: {}",
            reply.text
        );
        reply.value
    }
}

/// A per-process, per-test scratch directory under the OS temp
/// dir (same pattern as export_determinism.rs).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave11-e2e-draw-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// ---------------------------------------------------------------------
// SVG reading helpers (the exports are the session's read-back).
// ---------------------------------------------------------------------

/// The `points` attribute of the polyline drawn in
/// `stroke="#RRGGBB"`, split into "x,y" tokens.
fn polyline_points(svg: &str, color: &str) -> Vec<String> {
    let stroke_attr = format!("stroke=\"{color}\"");
    let line = svg
        .lines()
        .find(|l| l.contains("<polyline") && l.contains(&stroke_attr))
        .unwrap_or_else(|| panic!("no polyline with {stroke_attr} in the SVG:\n{svg}"));
    let start = line.find("points=\"").expect("polyline has points") + "points=\"".len();
    let end = line[start..].find('"').expect("points attribute closes") + start;
    line[start..end]
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// (cx, cy) of every dab circle filled with the given colour.
fn dab_centres(svg: &str, color: &str) -> Vec<(f32, f32)> {
    let fill_attr = format!("fill=\"{color}\"");
    svg.lines()
        .filter(|l| l.contains("<circle") && l.contains(&fill_attr))
        .map(|l| {
            let attr = |name: &str| -> f32 {
                let pat = format!("{name}=\"");
                let start = l.find(&pat).expect("circle attribute") + pat.len();
                let end = l[start..].find('"').expect("attribute closes") + start;
                l[start..end].parse().expect("circle attribute is a number")
            };
            (attr("cx"), attr("cy"))
        })
        .collect()
}

// ---------------------------------------------------------------------
// The session script. Run against a fresh server; asserts every
// step's reply and the final exports; returns the exported
// (svg, png) bytes so the determinism test can compare two
// independent runs of the identical script.
// ---------------------------------------------------------------------

fn run_draw_session(dir: &PathBuf) -> (Vec<u8>, Vec<u8>) {
    let mut s = Session::new();

    // -- Handshake -------------------------------------------------
    let init = s.rpc(&json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {},
    }));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");

    // The registry the session may use, proven against the
    // crate's own registry so this file cannot drift from it.
    let list = s.rpc(&json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "tools/list",
        "params": {},
    }));
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools/list returns a tools array");
    assert_eq!(
        tools.len(),
        stykanimate_rs::mcp::all_tools().len(),
        "tools/list disagrees with mcp::all_tools()"
    );
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool has a name"))
        .collect();
    for want in [
        "new_project",
        "draw_stroke",
        "draw_shape",
        "draw_polygon",
        "draw_star",
        "render",
        "get_project_stats",
    ] {
        assert!(names.contains(&want), "registry is missing {want}");
    }

    // -- New project ----------------------------------------------
    let created = s.call_ok(
        "new_project",
        json!({ "name": "Wave 11 Draw Session", "fps": 12, "width": 960, "height": 720 }),
    );
    assert_eq!(created["project"]["name"], "Wave 11 Draw Session");
    assert_eq!(created["project"]["frames"], 1, "a new project has one frame");
    assert_eq!(created["project"]["figures"], 1, "a new project has one figure");

    let state = s.call_ok("get_state", json!({}));
    assert_eq!(state["project"], "Wave 11 Draw Session");
    assert_eq!(state["frame_count"], 1);
    assert_eq!(state["figures"][0]["name"], "Styk");

    // -- 1. Plain stroke ------------------------------------------
    let plain = s.call_ok(
        "draw_stroke",
        json!({
            "frame": 0,
            "points": [[120, 120], [300, 90], [520, 150]],
            "color": 0x3366CC,
            "width": 5,
        }),
    );
    assert_eq!(plain["points"], 3);
    assert_eq!(plain["strokes_on_frame"], 1);

    // -- 2. Scattered stroke (the Wave 10 contract) ---------------
    // A 400-unit horizontal run at width 8: dab spacing is
    // width / 2 = 4, so stroke_dabs places 101 dabs, each
    // displaced by at most the 12-unit scatter radius.
    let scattered = s.call_ok(
        "draw_stroke",
        json!({
            "frame": 0,
            "points": [[100, 600], [500, 600]],
            "color": 0xFF00FF,
            "width": 8,
            "scatter": 12,
        }),
    );
    assert_eq!(scattered["points"], 2);
    assert_eq!(scattered["strokes_on_frame"], 2);

    // -- 3-6. The shape tools --------------------------------------
    // draw_shape and draw_star land on the CURRENT frame (0);
    // draw_polygon takes its frame explicitly.
    let rect = s.call_ok(
        "draw_shape",
        json!({ "kind": "rect", "x1": 640, "y1": 80, "x2": 880, "y2": 240, "color": 0x22CC88, "width": 4 }),
    );
    assert_eq!(rect["points"], 5, "a rect is a closed 5-point outline");
    assert_eq!(rect["strokes_on_frame"], 3);

    let ellipse = s.call_ok(
        "draw_shape",
        json!({ "kind": "ellipse", "x1": 60, "y1": 300, "x2": 260, "y2": 460, "color": 0xFFAA00, "width": 4 }),
    );
    assert_eq!(ellipse["points"], 25, "an ellipse is a closed 25-point outline");
    assert_eq!(ellipse["strokes_on_frame"], 4);

    let polygon = s.call_ok(
        "draw_polygon",
        json!({ "cx": 760, "cy": 420, "radius": 90, "sides": 6, "frame": 0, "color": 0x8888FF, "width": 4 }),
    );
    assert_eq!(polygon["points"], 7, "a 6-sided polygon is a closed 7-point outline");
    assert_eq!(polygon["strokes_on_frame"], 5);

    let star = s.call_ok(
        "draw_star",
        json!({ "cx": 300, "cy": 420, "radius": 80, "points": 5, "color": 0xFF5555, "width": 4 }),
    );
    assert_eq!(star["points"], 11, "a 5-spike star is a closed 11-point outline");
    assert_eq!(star["strokes_on_frame"], 6);

    // -- Read-back ---------------------------------------------------
    // list_strokes is the direct read-back: every stroke on
    // the frame, with geometry on request. get_project_stats
    // still carries no stroke count (asserted as a fact
    // below) — the list tool is the read surface now, and the
    // rendered exports below cross-check the geometry.
    let listed = s.call_ok("list_strokes", json!({ "frame": 0, "points": true }));
    assert_eq!(listed["count"], 6, "all six authored strokes are listed");
    assert_eq!(listed["total"], 6);
    let strokes = listed["strokes"].as_array().expect("strokes array");
    assert_eq!(strokes.len(), 6);
    assert!(
        strokes.iter().all(|st| st.get("points").is_some()),
        "points were requested and must ride along: {strokes:?}"
    );
    let stats = s.call_ok("get_project_stats", json!({}));
    assert_eq!(stats["frames"], 1);
    assert_eq!(stats["figures"], 1);
    assert!(
        stats.get("strokes").is_none(),
        "get_project_stats grew a strokes field — update the read-back note above"
    );

    // -- Export: SVG ----------------------------------------------
    let svg_dir = dir.join("svg");
    let svg_dir_str = svg_dir.to_string_lossy().into_owned();
    let svg_reply = s.call_ok(
        "render",
        json!({ "path": svg_dir_str, "format": "svg" }),
    );
    assert_eq!(svg_reply["frames"], 1, "one frame, one SVG file");
    assert_eq!(svg_reply["directory"], svg_dir_str.as_str());
    let svg = std::fs::read_to_string(svg_dir.join("frame-0001.svg")).expect("read exported SVG");

    // The plain stroke is a polyline through its exact points
    // (no camera keys, so the SVG view transform is identity).
    let plain_pts = polyline_points(&svg, "#3366CC");
    assert_eq!(
        plain_pts,
        vec!["120.0,120.0", "300.0,90.0", "520.0,150.0"],
        "the plain stroke's polyline must carry its recorded points verbatim"
    );

    // The scattered stroke is dabs, NOT a polyline — one
    // circle per paint::stroke_dabs placement for the stroke
    // as recorded (project-wide index 1: it was appended
    // second), each centre inside the scatter band.
    let recorded = Stroke {
        frame: 0,
        points: vec![[100.0, 600.0], [500.0, 600.0]],
        color: 0xFF00FF,
        width: 8.0,
        widths: Vec::new(),
        fill: None,
        spray: None,
        opacity: 1.0,
        scatter: 12.0,
    };
    let expected_dabs =
        paint::stroke_dabs(&recorded, 1, paint::stroke_dab_spacing(&recorded)).len();
    assert!(expected_dabs > 1, "the dab expectation itself must be plural");
    let dabs = dab_centres(&svg, "#FF00FF");
    assert_eq!(
        dabs.len(),
        expected_dabs,
        "the SVG must hold exactly one dab circle per stroke_dabs placement"
    );
    assert!(
        !svg.contains("stroke=\"#FF00FF\""),
        "a scattered stroke must not also be drawn as a polyline"
    );
    for (cx, cy) in &dabs {
        assert!(
            (588.0..=612.0).contains(cy),
            "dab centre y {cy} escapes the ±12 scatter band around y=600"
        );
        assert!(
            (88.0..=512.0).contains(cx),
            "dab centre x {cx} escapes the scattered run's span"
        );
    }
    assert!(
        dabs.iter().any(|(_, cy)| *cy != 600.0),
        "no dab was displaced off the recorded line — scatter did not reach the SVG"
    );

    // The shapes are polylines with their closed outlines.
    let rect_pts = polyline_points(&svg, "#22CC88");
    assert_eq!(
        rect_pts,
        vec!["640.0,80.0", "880.0,80.0", "880.0,240.0", "640.0,240.0", "640.0,80.0"],
        "the rect's outline must be its four corners, closed"
    );
    let ellipse_pts = polyline_points(&svg, "#FFAA00");
    assert_eq!(ellipse_pts.len(), 25, "ellipse outline point count");
    assert_eq!(ellipse_pts[0], ellipse_pts[24], "ellipse outline is closed");
    let polygon_pts = polyline_points(&svg, "#8888FF");
    assert_eq!(polygon_pts.len(), 7, "polygon outline point count");
    assert_eq!(polygon_pts[0], polygon_pts[6], "polygon outline is closed");
    let star_pts = polyline_points(&svg, "#FF5555");
    assert_eq!(star_pts.len(), 11, "star outline point count");
    assert_eq!(star_pts[0], star_pts[10], "star outline is closed");

    assert!(
        !svg.contains("inf") && !svg.contains("NaN"),
        "a clean session must export only finite coordinates"
    );

    // -- Export: PNG ------------------------------------------------
    let png_path = dir.join("frame0.png");
    let png_reply = s.call_ok(
        "render",
        json!({ "path": png_path.to_string_lossy(), "format": "png", "frame": 0 }),
    );
    assert_eq!(png_reply["width"], 960);
    assert_eq!(png_reply["height"], 720);
    let png_bytes = std::fs::read(&png_path).expect("read exported PNG");
    assert_eq!(
        png_reply["bytes"].as_u64(),
        Some(png_bytes.len() as u64),
        "the render reply's byte count must match the file it wrote"
    );
    assert!(
        png_bytes.starts_with(b"\x89PNG"),
        "the PNG export is missing its magic"
    );

    (svg.into_bytes(), png_bytes)
}

#[test]
fn draw_session_end_to_end() {
    let dir = temp_dir("session");
    let (svg, png) = run_draw_session(&dir);
    assert!(!svg.is_empty() && !png.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn draw_session_is_deterministic_across_servers() {
    // The identical message script against two independent
    // servers must produce byte-identical exports — the MCP
    // surface inherits the exporters' determinism contract
    // (the export_determinism.rs pattern, one level up).
    let dir_a = temp_dir("run-a");
    let dir_b = temp_dir("run-b");
    let (svg_a, png_a) = run_draw_session(&dir_a);
    let (svg_b, png_b) = run_draw_session(&dir_b);
    assert_eq!(svg_a, svg_b, "two runs of the draw session exported different SVGs");
    assert_eq!(png_a, png_b, "two runs of the draw session exported different PNGs");
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

// ---------------------------------------------------------------------
// Registry census: the stamp / text / stroke-read surface.
// These assertions began as ABSENCE pins (WAVE11-GAP); the
// splice landed all three, so they now assert presence.
// ---------------------------------------------------------------------

#[test]
fn registry_census_stamp_text_and_stroke_readback() {
    let mut s = Session::new();
    let list = s.rpc(&json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "tools/list",
        "params": {},
    }));
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("tool name").to_string())
        .collect();

    // The Draw panel's arrow stamp is a tool (the stamp's own
    // name is draw_arrow — it stamps the panel's arrow).
    assert!(
        names.iter().any(|n| n == "draw_arrow"),
        "the arrow stamp tool must be registered"
    );
    // Text objects are fully tooled: create + list at minimum.
    assert!(
        names.iter().any(|n| n == "add_text_instance"),
        "the text-instance create tool must be registered"
    );
    assert!(
        names.iter().any(|n| n == "list_texts"),
        "the text list tool must be registered"
    );
    // Strokes read back: the list tool exists.
    assert!(
        names.iter().any(|n| n == "list_strokes"),
        "the stroke list tool must be registered"
    );

    // A fresh project still reports zero texts — the count
    // the text tools now act on.
    s.call_ok("new_project", json!({ "name": "Census" }));
    let stats = s.call_ok("get_project_stats", json!({}));
    assert_eq!(stats["texts"], 0);
}

// ---------------------------------------------------------------------
// Negative cases: refused calls must not move the project.
// "Unchanged" is proven through MCP itself — save_project's
// file bytes before and after (the serialised project IS the
// project), plus the strokes_on_frame count of a probe stroke.
// ---------------------------------------------------------------------

#[test]
fn refused_draw_calls_leave_the_project_unchanged() {
    let dir = temp_dir("negatives");
    let mut s = Session::new();
    s.call_ok("new_project", json!({ "name": "Negatives" }));
    let seed = s.call_ok(
        "draw_stroke",
        json!({ "frame": 0, "points": [[10, 10], [90, 10]], "color": 0x101010 }),
    );
    assert_eq!(seed["strokes_on_frame"], 1);

    let save_bytes = |s: &mut Session, name: &str| -> Vec<u8> {
        let path = dir.join(name);
        s.call_ok(
            "save_project",
            json!({ "path": path.to_string_lossy() }),
        );
        std::fs::read(&path).expect("saved project file")
    };
    let baseline = save_bytes(&mut s, "baseline.styk.json");

    // Negative scatter: refused by draw_stroke's own contract.
    let neg = s.call(
        "draw_stroke",
        json!({ "frame": 0, "points": [[10, 20], [90, 20]], "scatter": -1 }),
    );
    assert!(neg.is_error, "negative scatter must be refused");
    assert!(
        neg.text.contains("scatter"),
        "the refusal should name the argument: {}",
        neg.text
    );
    assert_eq!(
        save_bytes(&mut s, "after-neg-scatter.styk.json"),
        baseline,
        "a refused negative-scatter call changed the project"
    );

    // Zero points: req_points yields an empty vec and the
    // handler refuses it.
    let empty = s.call(
        "draw_stroke",
        json!({ "frame": 0, "points": [] }),
    );
    assert!(empty.is_error, "a zero-point stroke must be refused");
    assert!(
        empty.text.contains("at least one point"),
        "the refusal should say why: {}",
        empty.text
    );
    assert_eq!(
        save_bytes(&mut s, "after-empty.styk.json"),
        baseline,
        "a refused zero-point call changed the project"
    );

    // A malformed point (one coordinate) is refused by
    // req_points before anything is recorded.
    let malformed = s.call(
        "draw_stroke",
        json!({ "frame": 0, "points": [[5.0]] }),
    );
    assert!(malformed.is_error, "a one-coordinate point must be refused");
    assert!(
        malformed.text.contains("[x, y] pair"),
        "the refusal should say why: {}",
        malformed.text
    );
    assert_eq!(
        save_bytes(&mut s, "after-malformed.styk.json"),
        baseline,
        "a refused malformed-point call changed the project"
    );

    // Out-of-range frame: refused by check_frame.
    let bad_frame = s.call(
        "draw_stroke",
        json!({ "frame": 7, "points": [[0, 0], [10, 10]] }),
    );
    assert!(bad_frame.is_error, "an out-of-range frame must be refused");
    assert!(
        bad_frame.text.contains("out of range"),
        "the refusal should say why: {}",
        bad_frame.text
    );
    assert_eq!(
        save_bytes(&mut s, "after-bad-frame.styk.json"),
        baseline,
        "a refused bad-frame call changed the project"
    );

    // The session is still usable and the count proves none of
    // the refusals recorded anything: the probe is stroke #2.
    let probe = s.call_ok(
        "draw_stroke",
        json!({ "frame": 0, "points": [[10, 30], [90, 30]] }),
    );
    assert_eq!(
        probe["strokes_on_frame"], 2,
        "a refusal leaked a stroke into the project"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_finite_scatter_is_refused() {
    // draw_stroke validates `scatter < 0.0` AND finiteness:
    // the argument helpers reject non-finite values before
    // AND after the f64 -> f32 cast — so a huge-but-finite
    // JSON number like 1e300, whose f32 cast overflows to
    // +infinity, is refused instead of being stored verbatim
    // as `scatter: f32::INFINITY` (the WAVE11-GAP behaviour
    // this test originally pinned). The same refusal covers
    // point coordinates and width via the same helpers.
    // (A literal 1e999 never reaches a handler at all:
    // serde_json refuses it at parse time — "number out of
    // range" — so the stdio framing answers -32700. The
    // cast-overflow case below is the one dispatch sees.)
    let dir = temp_dir("nonfinite");
    let mut s = Session::new();
    s.call_ok("new_project", json!({ "name": "NonFinite" }));
    let seed = s.call_ok(
        "draw_stroke",
        json!({ "frame": 0, "points": [[10, 10], [90, 10]] }),
    );
    assert_eq!(seed["strokes_on_frame"], 1);

    // Built as raw wire text, the way a client's line on the
    // stdio wire arrives — this is exactly what the server's
    // line parser hands to dispatch.
    let raw = r#"{"jsonrpc":"2.0","id":77,"method":"tools/call","params":{"name":"draw_stroke","arguments":{"frame":0,"points":[[10.0,50.0],[90.0,50.0]],"scatter":1e300}}}"#;
    let msg: Value = serde_json::from_str(raw).expect("the wire line parses");
    let resp = s
        .server
        .handle(&msg)
        .expect("the 1e300 call must be answered, never panic or hang the dispatch");
    assert_eq!(resp["id"], 77);
    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false);
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
        .to_string();
    assert!(is_error, "1e300 scatter must be refused, got: {text}");
    assert!(
        text.contains("scatter") && text.contains("finite"),
        "the refusal should name the argument and the reason: {text}"
    );

    // Nothing was recorded: the probe stroke lands at #2.
    let probe = s.call_ok(
        "draw_stroke",
        json!({ "frame": 0, "points": [[10, 90], [90, 90]] }),
    );
    assert_eq!(
        probe["strokes_on_frame"], 2,
        "a refused 1e999 call still recorded a stroke"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Dispatch shape: the protocol edges a drawing client hits
// before any tool runs.
// ---------------------------------------------------------------------

#[test]
fn dispatch_edges_notifications_and_unknown_tools() {
    let mut s = Session::new();

    // A notification (no id) is never answered, even a known
    // method — JSON-RPC's rule, and handle's contract.
    let note = s.server.handle(&json!({ "jsonrpc": "2.0", "method": "ping" }));
    assert!(note.is_none(), "a notification must not be answered");

    // An unknown tool is a PROTOCOL error (-32602), not an
    // isError tool result.
    let resp = s.rpc(&json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": { "name": "draw_everything", "arguments": {} },
    }));
    assert_eq!(resp["error"]["code"], -32602);
    assert!(
        resp["error"]["message"]
            .as_str()
            .expect("error message")
            .starts_with("unknown tool"),
        "the unknown-tool error keeps its historical text: {resp}"
    );

    // An unknown method is -32601.
    let resp = s.rpc(&json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/draw",
        "params": {},
    }));
    assert_eq!(resp["error"]["code"], -32601);
}
