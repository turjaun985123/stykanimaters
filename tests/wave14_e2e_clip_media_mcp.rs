//! Wave 14, Squad C (agent C10) — integration tests:
//! movieclip-definition segment media, driven end-to-end
//! through real MCP dispatch (`McpServer::handle`) and
//! ONLY through MCP. No GUI, no core calls: if a step
//! cannot be done with tools, this suite is where that
//! fact surfaces.
//!
//! The entering truth (Wave 13, E5's audit in
//! `docs/wave13/CLIP_MEDIA.md`): a clip definition's nodes
//! can carry `SegmentMedia`, it is stored, serialised and
//! counted by the asset census — but no renderer sampled
//! it, and NO tool could attach, clear or read it. Wave 14
//! squad C closes both halves: C1–C4 make it render (the
//! per-node fallback in `media::media_items`, the raster
//! gate, the SVG emission), C5 adds the MCP tools.
//!
//! The session this suite proves, step by step:
//!   import an asset (shipped `asset_import`)
//!   → create a movieclip def (shipped `clip_create`)
//!   → attach the asset as DEF media (C5's tool)
//!   → read it back at every step (C5's read tool, plus
//!     the shipped witnesses: `get_node`, `asset_info`,
//!     `assets_list`, `clip_list`, the saved file)
//!   → export PNG + SVG (shipped `render`) and assert the
//!     media is IN the outputs: the PNG carries the media
//!     image's pixels in the segment's region where the
//!     no-media export has none, the SVG embeds the asset
//!     and places it with a `<use>`
//!   → save, reload in a FRESH server, and find the media
//!     still reported and still rendering
//!   → undo after attach restores the no-media state,
//!     pixel for pixel, through tools only.
//!
//! A second session reaches clip-def-only media by the
//! audit's §4 provenance path using shipped tools alone
//! (attach to the figure, make the clip, clear the
//! figure's own copy) and pins that the fallback renders
//! that media too.
//!
//! DEPENDS — what this suite needs from parallel agents,
//! funnelled through the "WAVE14-DEPENDS funnels" section
//! below (the Wave 13 GAP convention: the funnels are the
//! ONLY tolerant code in the file; the witnesses are
//! strict):
//!
//!   * C5 — the clip media tools. NOT in the tree when
//!     this file was written. Names and argument spellings
//!     are C9's TOOL ENTRY SPECs in
//!     `docs/wave14/CLIP_MEDIA_ROWS.md` §0, written against
//!     C5's remit: `clip_asset_attach` {clip, node, asset},
//!     `clip_clear_node_media` {clip, node},
//!     `clip_get_node_media` {clip, node} returning the
//!     media in `get_node`'s shape (null, or
//!     {kind: "image", asset} / {kind: "text", content,
//!     size}). If C5 registers different names, the
//!     integrator renames them IN THE FUNNELS. C9 also
//!     documents one alternative shape — the read folded
//!     into `clip_list` as per-node media — and the read
//!     funnel tolerates exactly that one alternative.
//!   * C3 — the raster gate: `export.rs`'s `has_media`
//!     must gain the clip-def term of CLIP_MEDIA.md §6.4.
//!     Verified ABSENT on the entering tree (the gate at
//!     src/export.rs checks sprites / texts / figure-def
//!     media only), so without C3 a project whose ONLY
//!     media is clip-def media — exactly what this suite
//!     builds — never composites it into the PNG, and the
//!     PNG assertions below fail for that reason alone.
//!   * C1 — the `media_items` fallback itself. Present in
//!     the working tree when this file was written
//!     (src/media.rs, "Wave 14, C1" comments); recorded
//!     here because the SVG assertions ride on it too
//!     (C4 needs no clip-specific branch: `append_media`
//!     consumes the funnel unconditionally).
//!
//! The standing contracts pinned alongside (the house
//! rules every wave re-pins): every mutating tool is
//! exactly ONE undo step (observed through
//! `get_editor_status`'s undo depth); a refused call is
//! an `isError` tool result that spends no undo step and
//! changes no state; an unknown tool name is a protocol
//! error, `-32602`.
//!
//! The media fixture is a 64×64 solid magenta PNG,
//! generated in-test with the `image` crate (a regular
//! dependency, as the Wave 13 suite's `serde_json` use
//! establishes for integration tests). Magenta is chosen
//! because nothing in the default scene produces it: the
//! no-media export must contain ZERO magenta-ish pixels,
//! and every magenta-ish pixel in the media export is
//! therefore the attached image's own paint. The image
//! quad a segment carries spans the segment's length in
//! both axes for a square asset (media.rs's geometry:
//! half-height = len / aspect / 2), so the chest segment
//! (node 1, length 46) stamps a ≈46×46 quad centred on
//! the segment's midpoint — the region assertion compares
//! the magenta centroid against the midpoint computed
//! from the saved project through the model's own
//! `poses_at` + `evaluate`, never against a hand-copied
//! coordinate.
//!
//! Every step goes through MCP messages; direct
//! filesystem access is used only for what an MCP client
//! itself would observe from outside the protocol:
//! writing the import fixture, and reading back the saved
//! project and the exported files. Each test builds its
//! own server and its own temp dir and shares nothing, so
//! tests never depend on execution order. No test changes
//! the process current directory; every path passed to a
//! tool is absolute.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::{evaluate, Project, SegmentMedia};

/// The media target throughout: node 1 of the default
/// humanoid, the "chest" segment (parent 0, length 46,
/// pointing up in the rest pose). Long enough to stamp a
/// clearly countable quad, central enough that no view
/// transform can push it off-frame.
const MEDIA_NODE: u64 = 1;

/// The fixture image's colour: full magenta, a colour the
/// default scene (dark blue-grey background, near-white
/// figure) never produces on its own.
const MAGENTA: [u8; 4] = [255, 0, 255, 255];

/// A pixel counts as the fixture's paint when it is
/// unambiguously magenta; the tolerance absorbs the
/// raster's anti-aliased quad edges and any supersample
/// downscale, neither of which can INVENT magenta in the
/// no-media export.
fn is_magentaish(px: &image::Rgba<u8>) -> bool {
    px[0] >= 200 && px[1] <= 90 && px[2] >= 200
}

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 11/13 pattern)
// ---------------------------------------------------------------------

/// A per-test scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front
/// and the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave14-e2e-clip-media-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// The 64×64 solid-magenta PNG, encoded once per test:
/// real PNG bytes the `image` decoder accepts (asset
/// import validates by decoding), small and exact.
fn magenta_png_bytes() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(64, 64, image::Rgba(MAGENTA));
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        )
        .expect("encode the fixture PNG");
    buf
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
// Shared session reads — strict witnesses (no DEPENDS tolerance)
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

/// Save through the shipped IO tool and return the project
/// exactly as it crossed the file boundary — the
/// ground-truth witness for everything the tools wrote,
/// independent of any reply shape.
fn saved_project(mcp: &mut Dispatch, dir: &Path, file: &str) -> (PathBuf, Project) {
    let path = dir.join(file);
    mcp.call_ok(
        "save_project",
        json!({ "path": path.to_string_lossy() }),
    );
    let project = read_project(&path);
    (path, project)
}

/// Export one frame as a PNG through the shipped `render`
/// tool and decode it: the raster witness. What a client
/// receives is the file; decoding it here is the same
/// outside-the-protocol read as parsing the saved project.
fn render_png(mcp: &mut Dispatch, dir: &Path, file: &str) -> image::RgbaImage {
    let path = dir.join(file);
    mcp.call_ok(
        "render",
        json!({
            "path": path.to_string_lossy(),
            "format": "png",
            "frame": 0,
        }),
    );
    let bytes = fs::read(&path).expect("the render tool wrote the PNG");
    image::load_from_memory(&bytes)
        .expect("the exported PNG decodes")
        .to_rgba8()
}

/// Export the timeline as SVG through the shipped `render`
/// tool (one file per frame into a directory) and return
/// frame 1's document: the vector witness.
fn render_svg_frame1(mcp: &mut Dispatch, dir: &Path, sub: &str) -> String {
    let out_dir = dir.join(sub);
    mcp.call_ok(
        "render",
        json!({
            "path": out_dir.to_string_lossy(),
            "format": "svg",
        }),
    );
    fs::read_to_string(out_dir.join("frame-0001.svg"))
        .expect("the render tool wrote frame-0001.svg")
}

/// Count the fixture's paint in a rendered frame, and the
/// paint's centroid in pixel coordinates.
fn magenta_stats(img: &image::RgbaImage) -> (u64, [f32; 2]) {
    let (w, h) = img.dimensions();
    let mut count: u64 = 0;
    let mut sum = [0.0f32; 2];
    for y in 0..h {
        for x in 0..w {
            if is_magentaish(img.get_pixel(x, y)) {
                count += 1;
                sum[0] += x as f32;
                sum[1] += y as f32;
            }
        }
    }
    let centroid = if count > 0 {
        [sum[0] / count as f32, sum[1] / count as f32]
    } else {
        [0.0, 0.0]
    };
    (count, centroid)
}

/// The media segment's midpoint in IMAGE pixel
/// coordinates, computed from the witnessed project
/// through the model's own sampling (`poses_at` applies
/// the clip pose, `evaluate` places the joints) and scaled
/// from stage units by the rendered size — the expected
/// centre of the stamped quad, derived from the file the
/// tools wrote, never hand-copied.
fn segment_midpoint_px(project: &Project, img: &image::RgbaImage) -> [f32; 2] {
    let poses = project.poses_at(0.0);
    let pos = evaluate(&project.figures[0].def, &poses[0]);
    let node = MEDIA_NODE as usize;
    let parent = project.figures[0].def.nodes[node]
        .parent
        .expect("the media node has a parent segment");
    let mid = [
        (pos[parent][0] + pos[node][0]) / 2.0,
        (pos[parent][1] + pos[node][1]) / 2.0,
    ];
    let (w, h) = img.dimensions();
    [
        mid[0] * w as f32 / project.stage[0],
        mid[1] * h as f32 / project.stage[1],
    ]
}

/// Standard-alphabet base64 with padding, written out
/// because no base64 crate is among the project's
/// dependencies. The SVG writer embeds the asset's stored
/// PNG bytes base64-encoded (src/svg.rs `append_media`,
/// via `mcp_io::base64_encode`); asset import stores the
/// imported file's bytes verbatim, so the expected string
/// is computable from the fixture bytes alone.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| -> u32 { chunk.get(i).copied().unwrap_or(0) as u32 };
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Assert an SVG document carries the fixture asset as
/// clip/segment media: the embedded per-asset PNG (the
/// exact imported bytes) plus a `<use>` placing it.
fn assert_svg_carries_media(svg: &str, fixture_png: &[u8], what: &str) {
    assert!(
        svg.contains("<image id=\"asset0\""),
        "{what}: the SVG embeds the asset as <image id=\"asset0\">: {}…",
        &svg[..svg.len().min(200)]
    );
    let expected = format!("data:image/png;base64,{}", base64_encode(fixture_png));
    assert!(
        svg.contains(&expected),
        "{what}: the SVG embeds the imported asset's exact bytes"
    );
    assert!(
        svg.contains("<use href=\"#asset0\""),
        "{what}: the SVG places the asset with a <use> element"
    );
    assert!(
        svg.contains("matrix("),
        "{what}: the placement carries its matrix transform"
    );
}

/// Assert an SVG document carries NO embedded media at all.
fn assert_svg_media_free(svg: &str, what: &str) {
    assert!(
        !svg.contains("data:image/png;base64,"),
        "{what}: the SVG embeds no image data"
    );
    assert!(
        !svg.contains("<use href=\"#asset"),
        "{what}: the SVG places no media asset"
    );
}

// ---------------------------------------------------------------------
// WAVE14-DEPENDS funnels — the ONLY tolerant code in this file
// ---------------------------------------------------------------------
//
// C5's tools land after this file. Everything whose final
// spelling they own is concentrated here:
//
//   * the three tool names (clip_asset_attach,
//     clip_clear_node_media, clip_get_node_media) and their
//     argument spellings (clip / node / asset), from C9's
//     TOOL ENTRY SPECs in docs/wave14/CLIP_MEDIA_ROWS.md
//     §0 — the integrator renames here if C5 registered
//     differently;
//   * the read reply's wrapping: the payload AS the media
//     value, or under a `media` key, or (C9's documented
//     alternative) no dedicated read tool at all, with
//     clip_list carrying per-node media instead.
//
// The flip: when C5's shipped shapes are known, tighten
// each funnel to the one true spelling and delete the
// alternates; the test bodies below do not change.

// WAVE14-DEPENDS: C5 clip_asset_attach — the call itself
// (unregistered on the entering surface, so this fails
// loudly through call_ok until the splice; deliberate —
// no DEPENDS test can pass green against a missing tool).
fn clip_attach_media(mcp: &mut Dispatch, clip: u64, node: u64, asset: u64) -> Value {
    mcp.call_ok(
        "clip_asset_attach",
        json!({ "clip": clip, "node": node, "asset": asset }),
    )
}

// WAVE14-DEPENDS: C5 clip_asset_attach — the refusal path.
fn clip_attach_media_err(mcp: &mut Dispatch, clip: u64, node: u64, asset: u64) -> String {
    mcp.call_err(
        "clip_asset_attach",
        json!({ "clip": clip, "node": node, "asset": asset }),
    )
}

// WAVE14-DEPENDS: C5 clip_clear_node_media — the call.
fn clip_clear_media(mcp: &mut Dispatch, clip: u64, node: u64) -> Value {
    mcp.call_ok(
        "clip_clear_node_media",
        json!({ "clip": clip, "node": node }),
    )
}

// WAVE14-DEPENDS: C5 clip_clear_node_media — the refusal path.
fn clip_clear_media_err(mcp: &mut Dispatch, clip: u64, node: u64) -> String {
    mcp.call_err(
        "clip_clear_node_media",
        json!({ "clip": clip, "node": node }),
    )
}

// WAVE14-DEPENDS: C5 clip_get_node_media — the read.
// Returns the media value in get_node's shape: Value::Null
// for none, else the {kind, …} object. Extraction tolerates
// the payload being the media itself or wrapping it under
// `media`; if the tool name itself is unknown (-32602),
// falls back to C9's documented alternative — per-node
// media inside clip_list's reply. A clip_list that carries
// NO per-node media is the parity failure this suite
// exists to catch, and the funnel says so in those words.
fn clip_read_media(mcp: &mut Dispatch, clip: u64, node: u64) -> Value {
    let outcome = mcp.call(
        "clip_get_node_media",
        json!({ "clip": clip, "node": node }),
    );
    if outcome.is_error && outcome.text.contains("unknown tool") {
        return clip_list_media(mcp, clip, node);
    }
    assert!(
        !outcome.is_error,
        "clip_get_node_media failed: {}",
        outcome.text
    );
    extract_media(&outcome.payload)
}

// WAVE14-DEPENDS: C5 clip_get_node_media — the refusal path
// (a bad clip/node must refuse, whichever read shape C5
// shipped; the clip_list fallback answers a bad clip the
// same way, so this goes through the plain call).
fn clip_read_media_err(mcp: &mut Dispatch, clip: u64, node: u64) -> String {
    mcp.call_err(
        "clip_get_node_media",
        json!({ "clip": clip, "node": node }),
    )
}

/// Pull the media value out of a read reply: the payload
/// itself when it is null or a {kind, …} object, else its
/// `media` member.
fn extract_media(payload: &Value) -> Value {
    if payload.is_null() || payload.get("kind").is_some() {
        return payload.clone();
    }
    if let Some(media) = payload.get("media") {
        return media.clone();
    }
    panic!("read reply carries no media value: {payload}");
}

/// The clip_list fallback for the read: find the clip's
/// entry and its per-node media, under any of the field
/// spellings C9's clause admits (a `media` array, a
/// `node_media` array, or per-node objects).
fn clip_list_media(mcp: &mut Dispatch, clip: u64, node: u64) -> Value {
    let list = mcp.call_ok("clip_list", json!({}));
    let entry = list["clips"]
        .as_array()
        .and_then(|clips| {
            clips
                .iter()
                .find(|c| c["index"].as_u64() == Some(clip))
                .cloned()
        })
        .unwrap_or_else(|| panic!("clip_list carries no entry for clip {clip}: {list}"));
    for field in ["media", "node_media"] {
        if let Some(per_node) = entry[field].as_array() {
            return per_node
                .get(node as usize)
                .cloned()
                .unwrap_or(Value::Null);
        }
    }
    if let Some(nodes) = entry["nodes"].as_array() {
        if let Some(node_entry) = nodes.get(node as usize) {
            if node_entry.get("kind").is_some() {
                return node_entry.clone();
            }
            if let Some(media) = node_entry.get("media") {
                return media.clone();
            }
        }
    }
    panic!(
        "clip_list carries no per-node media for clip {clip} — \
         no read tool can see clip-def media, the parity gap \
         this suite exists to catch: {entry}"
    );
}

/// Assert a media value (from the read funnel) is "no media".
fn assert_no_media(media: &Value, what: &str) {
    assert!(
        media.is_null(),
        "{what}: expected no media, got {media}"
    );
}

/// Assert a media value is the image of `asset`.
fn assert_image_media(media: &Value, asset: u64, what: &str) {
    assert_eq!(
        media["kind"].as_str(),
        Some("image"),
        "{what}: the media kind is image: {media}"
    );
    assert_eq!(
        media["asset"].as_u64(),
        Some(asset),
        "{what}: the media names asset {asset}: {media}"
    );
}

// ---------------------------------------------------------------------
// Shared session builders
// ---------------------------------------------------------------------

/// The session's opening moves, all shipped tools: a fresh
/// project, the magenta fixture imported as asset 0, and a
/// movieclip created from figure 0 (which then loops it).
/// Returns the fixture PNG's exact bytes.
fn build_project_with_clip(mcp: &mut Dispatch, dir: &Path, name: &str) -> Vec<u8> {
    mcp.call_ok("new_project", json!({ "name": name }));
    let png = magenta_png_bytes();
    let png_path = write_fixture(dir, "badge.png", &png);
    let imported = mcp.call_ok("asset_import", json!({ "path": png_path }));
    assert_eq!(
        imported["index"].as_u64(),
        Some(0),
        "the fixture imports as asset 0: {imported}"
    );
    let created = mcp.call_ok("clip_create", json!({ "figure": 0 }));
    assert_eq!(
        created["clip"].as_u64(),
        Some(0),
        "the clip is created at index 0: {created}"
    );
    assert_eq!(
        created["loops_on_figure"],
        json!(true),
        "figure 0 loops the new clip: {created}"
    );
    png
}

/// The shipped asset-census witness: `asset_info`'s live
/// reference counts for asset 0 — (figure segments,
/// clip-def segments, sprites).
fn asset_references(mcp: &mut Dispatch) -> (u64, u64, u64) {
    let info = mcp.call_ok("asset_info", json!({ "index": 0 }));
    let refs = &info["references"];
    (
        refs["segments"].as_u64().expect("segments count"),
        refs["clip_segments"].as_u64().expect("clip_segments count"),
        refs["sprites"].as_u64().expect("sprites count"),
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
    // The floor is the Wave 13 census, NOT the Wave 14
    // total: C5's clip media tools take the surface to
    // 291, but the splice may land after this suite, and a
    // floor assertion stays true on both sides of it.
    assert!(
        tools.len() >= 288,
        "the entering surface is the Wave 13 census of 288 tools, got {}",
        tools.len()
    );
    let tool_names: std::collections::BTreeSet<String> = tools
        .iter()
        .map(|t| {
            t["name"]
                .as_str()
                .expect("every tool carries a name")
                .to_string()
        })
        .collect();
    // The shipped surface this suite drives around C5's
    // tools: session setup, the asset family, the clip
    // family, the media read witnesses, selection (for the
    // selection-scoped clear), render and the IO witnesses.
    for required in [
        "new_project",
        "asset_import",
        "assets_list",
        "asset_info",
        "asset_remove",
        "asset_attach",
        "clip_create",
        "clip_list",
        "clip_insert_figure",
        "get_node",
        "set_selection",
        "clear_node_media",
        "render",
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
    assert!(
        response["error"]["message"]
            .as_str()
            .expect("the protocol error carries a message")
            .contains("unknown tool"),
        "the error names the problem: {response}"
    );
}

// ---------------------------------------------------------------------
// The clip media sessions — WAVE14-DEPENDS steps, flipped at the splice
// ---------------------------------------------------------------------

/// The headline session: attach the asset as clip-def
/// media via C5's tool, watch every read surface report it,
/// and find its pixels in the PNG export (in the segment's
/// region) and its bytes in the SVG export — where the
/// same project exported BEFORE the attach carries none.
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_get_node_media;
// C3's raster gate (the PNG half); C1's funnel (both halves).
#[test]
fn clip_media_attach_read_and_export_session() {
    let dir = temp_dir("attach");
    let mut mcp = Dispatch::new();
    let png = build_project_with_clip(&mut mcp, &dir, "Wave14 Clip Media");

    // Read parity BEFORE the attach: no surface sees media.
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_no_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        "the clip def's chest node starts media-free",
    );
    let node = mcp.call_ok("get_node", json!({ "figure": 0, "node": MEDIA_NODE }));
    assert!(
        node["media"].is_null(),
        "the figure's own chest node starts media-free: {node}"
    );
    assert_eq!(
        asset_references(&mut mcp),
        (0, 0, 0),
        "the asset census counts no references yet"
    );

    // The no-media exports, same project, same session:
    // the PNG carries no magenta anywhere, the SVG embeds
    // and places nothing.
    let baseline_png = render_png(&mut mcp, &dir, "baseline.png");
    let (baseline_count, _) = magenta_stats(&baseline_png);
    assert_eq!(
        baseline_count, 0,
        "the no-media export contains no fixture paint"
    );
    let baseline_svg = render_svg_frame1(&mut mcp, &dir, "svg-baseline");
    assert_svg_media_free(&baseline_svg, "the no-media SVG");

    // The attach: exactly one undo step.
    let depth_before = undo_depth(&mut mcp);
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media(&mut mcp, 0, MEDIA_NODE, 0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "clip_asset_attach is exactly one undo step"
    );

    // Read parity AFTER the attach — every surface agrees:
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "the clip read tool reports the attached media",
    );
    // …the figure's OWN def stays media-free (this is
    // def-only media; get_node reads the figure's def and
    // must NOT see it — a read that found it here would
    // mean the attach wrote to the wrong def)…
    let node = mcp.call_ok("get_node", json!({ "figure": 0, "node": MEDIA_NODE }));
    assert!(
        node["media"].is_null(),
        "the media lives on the clip def, not the figure's def: {node}"
    );
    // …the shipped census counts exactly one clip-def
    // reference and nothing else…
    assert_eq!(
        asset_references(&mut mcp),
        (0, 1, 0),
        "the census counts the clip-def reference"
    );
    // …clip_list still describes the def the media hangs on…
    let clips = mcp.call_ok("clip_list", json!({}));
    assert_eq!(clips["count"].as_u64(), Some(1), "one clip def: {clips}");
    assert_eq!(
        clips["clips"][0]["nodes"].as_u64(),
        Some(11),
        "the def carries the humanoid's 11 nodes: {clips}"
    );
    assert_eq!(
        clips["clips"][0]["used_by"],
        json!([0]),
        "figure 0 uses the clip: {clips}"
    );
    // …and the file witness agrees with all of them.
    let (_, project) = saved_project(&mut mcp, &dir, "attached.styk.json");
    assert_eq!(
        project.clips[0].def.nodes[MEDIA_NODE as usize].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
        "the saved clip def carries the image media"
    );
    assert_eq!(
        project.figures[0].def.nodes[MEDIA_NODE as usize].media,
        None,
        "the saved figure def carries none — def-only media"
    );
    assert_eq!(
        project.figures[0].clip,
        Some(0),
        "the saved figure loops clip 0"
    );

    // The PNG export now carries the media's pixels, in
    // the segment's region: a solid count of fixture paint
    // whose centroid sits on the segment's midpoint (the
    // stamped quad is ≈46px wide; a centroid 70px away
    // would mean the paint landed somewhere else entirely).
    // WAVE14-DEPENDS: C3's has_media gate extension — a
    // clip-def-only project reaches composite_media at all
    // only once the gate knows clip defs.
    let media_png = render_png(&mut mcp, &dir, "with-media.png");
    let (count, centroid) = magenta_stats(&media_png);
    assert!(
        count >= 800,
        "the media export stamps the segment's quad (got {count} magenta-ish pixels)"
    );
    let expected = segment_midpoint_px(&project, &media_png);
    let dist = ((centroid[0] - expected[0]).powi(2)
        + (centroid[1] - expected[1]).powi(2))
    .sqrt();
    assert!(
        dist <= 70.0,
        "the paint's centroid {centroid:?} sits on the segment midpoint {expected:?} (distance {dist:.1})"
    );

    // The SVG export embeds the asset's exact bytes and
    // places them.
    let media_svg = render_svg_frame1(&mut mcp, &dir, "svg-media");
    assert_svg_carries_media(&media_svg, &png, "the media SVG");

    let _ = fs::remove_dir_all(&dir);
}

/// Undo after the attach restores the no-media state —
/// the read tools say so, and the PNG export is pixel-for
/// pixel the export taken before the attach. Redo brings
/// the media back.
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_get_node_media.
#[test]
fn clip_media_undo_restores_the_no_media_state() {
    let dir = temp_dir("undo");
    let mut mcp = Dispatch::new();
    build_project_with_clip(&mut mcp, &dir, "Wave14 Undo");

    let before_png = render_png(&mut mcp, &dir, "before.png");
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media(&mut mcp, 0, MEDIA_NODE, 0);
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "the attach landed before the undo",
    );

    // ONE undo undoes the whole attach…
    mcp.call_ok("undo", json!({}));
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_no_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        "one undo removed the clip-def media",
    );
    assert_eq!(
        asset_references(&mut mcp),
        (0, 0, 0),
        "the census agrees: no references after the undo"
    );
    // …and the export is the pre-attach export again,
    // pixel for pixel.
    let undone_png = render_png(&mut mcp, &dir, "undone.png");
    assert_eq!(
        undone_png.dimensions(),
        before_png.dimensions(),
        "the undone export has the same size"
    );
    assert_eq!(
        undone_png.as_raw(),
        before_png.as_raw(),
        "the undone export is pixel-identical to the pre-attach export"
    );

    // Redo restores the media and its paint.
    mcp.call_ok("redo", json!({}));
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "redo restored the clip-def media",
    );
    let redone_png = render_png(&mut mcp, &dir, "redone.png");
    let (redone_count, _) = magenta_stats(&redone_png);
    assert!(
        redone_count >= 800,
        "the redone export stamps the quad again (got {redone_count} magenta-ish pixels)"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Clear via C5's tool detaches the media: the reads go
/// back to none, the exports lose the media — and the
/// asset the media was locking (the Wave 13 audit's §3
/// symptom: a clip-def-only reference refused
/// `asset_remove`) can finally be removed through the
/// shipped tool.
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_clear_node_media
// + clip_get_node_media.
#[test]
fn clip_media_clear_detaches_and_frees_the_asset() {
    let dir = temp_dir("clear");
    let mut mcp = Dispatch::new();
    build_project_with_clip(&mut mcp, &dir, "Wave14 Clear");
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media(&mut mcp, 0, MEDIA_NODE, 0);

    // While the clip def references the asset, the shipped
    // removal refuses — honestly now, because the media
    // renders and a detach path exists.
    let depth_before = undo_depth(&mut mcp);
    let err = mcp.call_err("asset_remove", json!({ "index": 0 }));
    assert!(!err.is_empty(), "the refusal says why: {err}");
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "the refused removal spends no undo step"
    );

    // The clear: exactly one undo step (media was actually
    // cleared — C9's row wording).
    let depth_before = undo_depth(&mut mcp);
    // WAVE14-DEPENDS: C5 clip_clear_node_media
    clip_clear_media(&mut mcp, 0, MEDIA_NODE);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "clearing real media is exactly one undo step"
    );
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_no_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        "the clear detached the media",
    );
    assert_eq!(
        asset_references(&mut mcp),
        (0, 0, 0),
        "the census counts no references after the clear"
    );

    // The exports lose the media.
    let cleared_png = render_png(&mut mcp, &dir, "cleared.png");
    let (count, _) = magenta_stats(&cleared_png);
    assert_eq!(count, 0, "the cleared export carries no fixture paint");
    let cleared_svg = render_svg_frame1(&mut mcp, &dir, "svg-cleared");
    assert_svg_media_free(&cleared_svg, "the cleared SVG");

    // Clearing an already-clear node is reported, not an
    // error (C9's row text), and changes nothing.
    // WAVE14-DEPENDS: C5 clip_clear_node_media
    clip_clear_media(&mut mcp, 0, MEDIA_NODE);
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_no_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        "the empty clear left the node media-free",
    );

    // And now the asset removes cleanly.
    mcp.call_ok("asset_remove", json!({ "index": 0 }));
    let assets = mcp.call_ok("assets_list", json!({}));
    assert_eq!(
        assets["count"].as_u64(),
        Some(0),
        "the freed asset was removed: {assets}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The C5 refusals: a bad clip, a bad node, a bad asset —
/// each an isError result quoting the problem, spending no
/// undo step and changing no state.
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_clear_node_media
// + clip_get_node_media.
#[test]
fn clip_media_refusals_spend_no_undo() {
    let dir = temp_dir("refusals");
    let mut mcp = Dispatch::new();
    build_project_with_clip(&mut mcp, &dir, "Wave14 Refusals");

    let depth_before = undo_depth(&mut mcp);
    // WAVE14-DEPENDS: C5 clip_asset_attach
    let err = clip_attach_media_err(&mut mcp, 99, MEDIA_NODE, 0);
    assert!(!err.is_empty(), "the bad-clip refusal says why: {err}");
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media_err(&mut mcp, 0, 99, 0);
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media_err(&mut mcp, 0, MEDIA_NODE, 99);
    // WAVE14-DEPENDS: C5 clip_clear_node_media
    clip_clear_media_err(&mut mcp, 99, MEDIA_NODE);
    // WAVE14-DEPENDS: C5 clip_get_node_media
    clip_read_media_err(&mut mcp, 99, MEDIA_NODE);
    // WAVE14-DEPENDS: C5 clip_get_node_media
    clip_read_media_err(&mut mcp, 0, 99);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "refused clip media calls spend no undo step"
    );
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_no_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        "the refused calls attached nothing",
    );
    let (_, project) = saved_project(&mut mcp, &dir, "after-refusals.styk.json");
    assert_eq!(
        project.clips[0].def.nodes[MEDIA_NODE as usize].media,
        None,
        "the saved clip def agrees: no media"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Save the attached project, reload it in a FRESH server
/// (a new McpServer, the way a new client session would),
/// and find the media still reported by the read tools and
/// still present in both exports.
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_get_node_media.
#[test]
fn clip_media_survives_save_and_reload_in_a_fresh_server() {
    let dir = temp_dir("reload");
    let png = {
        let mut mcp = Dispatch::new();
        let png = build_project_with_clip(&mut mcp, &dir, "Wave14 Reload");
        // WAVE14-DEPENDS: C5 clip_asset_attach
        clip_attach_media(&mut mcp, 0, MEDIA_NODE, 0);
        let (path, _) = saved_project(&mut mcp, &dir, "saved.styk.json");
        // Reload into the SAME server first would prove
        // little; the fresh server below is the witness.
        let _ = path;
        png
    };

    let mut mcp = Dispatch::new();
    let path = dir.join("saved.styk.json");
    mcp.call_ok(
        "load_project",
        json!({ "path": path.to_string_lossy() }),
    );

    // Read parity survives the round trip.
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "the reloaded project's clip def still carries the media",
    );
    assert_eq!(
        asset_references(&mut mcp),
        (0, 1, 0),
        "the census still counts the clip-def reference"
    );
    let (_, project) = saved_project(&mut mcp, &dir, "resaved.styk.json");
    assert_eq!(
        project.clips[0].def.nodes[MEDIA_NODE as usize].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
        "the resaved clip def carries the media"
    );

    // And it still renders in the fresh session.
    let media_png = render_png(&mut mcp, &dir, "reloaded.png");
    let (count, _) = magenta_stats(&media_png);
    assert!(
        count >= 800,
        "the reloaded project still stamps the quad (got {count} magenta-ish pixels)"
    );
    let media_svg = render_svg_frame1(&mut mcp, &dir, "svg-reloaded");
    assert_svg_carries_media(&media_svg, &png, "the reloaded SVG");

    let _ = fs::remove_dir_all(&dir);
}

/// A second instance of the same clip, placed through the
/// shipped `clip_insert_figure`, draws the def's media
/// too: the def is shared, so its media is every
/// instance's media. (Pixel COUNT growth is deliberately
/// not asserted: `apply_clips` replaces an instance's
/// whole pose — root included — with the clip's pose, so
/// both instances draw at the clip pose's root and their
/// quads coincide; the witnesses are the placement, the
/// clip table and the paint's continued presence.)
// WAVE14-DEPENDS: C5 clip_asset_attach + clip_get_node_media.
#[test]
fn clip_media_second_instance_renders_from_the_same_def() {
    let dir = temp_dir("second");
    let mut mcp = Dispatch::new();
    build_project_with_clip(&mut mcp, &dir, "Wave14 Second Instance");
    // WAVE14-DEPENDS: C5 clip_asset_attach
    clip_attach_media(&mut mcp, 0, MEDIA_NODE, 0);

    // Place the second instance: one undo step.
    let depth_before = undo_depth(&mut mcp);
    let placed = mcp.call_ok("clip_insert_figure", json!({ "index": 0 }));
    assert_eq!(
        placed["figure"].as_u64(),
        Some(1),
        "the second instance is figure 1: {placed}"
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "clip_insert_figure is exactly one undo step"
    );

    // The clip table names both users; the read still
    // reports the one shared def's media.
    let clips = mcp.call_ok("clip_list", json!({}));
    assert_eq!(
        clips["clips"][0]["used_by"],
        json!([0, 1]),
        "both figures use clip 0: {clips}"
    );
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "the shared def still carries the media",
    );
    let (_, project) = saved_project(&mut mcp, &dir, "two-instances.styk.json");
    assert_eq!(project.figures.len(), 2, "two figures saved");
    assert_eq!(project.figures[1].clip, Some(0), "figure 1 loops clip 0");
    assert_eq!(
        project.figures[0].def.nodes[MEDIA_NODE as usize].media,
        None,
        "figure 0's own def is still media-free — the paint is the def's"
    );

    // The paint is on the export with both instances live.
    let media_png = render_png(&mut mcp, &dir, "two-instances.png");
    let (count, _) = magenta_stats(&media_png);
    assert!(
        count >= 800,
        "the media renders with both instances placed (got {count} magenta-ish pixels)"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The provenance path (CLIP_MEDIA.md §4), driven with
/// SHIPPED tools only: attach the image to the FIGURE,
/// make the clip (the def clone inherits the media),
/// then clear the figure's own copy through the
/// selection-scoped shipped clear — leaving media that
/// exists ONLY in the clip def. That media renders via
/// the fallback, and the one read that can see it is
/// C5's.
// WAVE14-DEPENDS: C5 clip_get_node_media (the read only —
// every write here is shipped); C1's funnel + C3's gate
// (the render half).
#[test]
fn clip_media_clone_provenance_renders_via_the_fallback() {
    let dir = temp_dir("provenance");
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave14 Provenance" }));
    let png = magenta_png_bytes();
    let png_path = write_fixture(&dir, "badge.png", &png);
    mcp.call_ok("asset_import", json!({ "path": png_path }));

    // Attach to the figure's own def (shipped, by index).
    mcp.call_ok(
        "asset_attach",
        json!({ "figure": 0, "node": MEDIA_NODE, "asset": 0 }),
    );
    let node = mcp.call_ok("get_node", json!({ "figure": 0, "node": MEDIA_NODE }));
    assert_eq!(
        node["media"]["kind"].as_str(),
        Some("image"),
        "the figure's node carries the media before the clip: {node}"
    );
    // Figure-def media renders — the entering truth.
    let figure_png = render_png(&mut mcp, &dir, "figure-media.png");
    let (count, _) = magenta_stats(&figure_png);
    assert!(
        count >= 800,
        "figure-def media renders (got {count} magenta-ish pixels)"
    );

    // Make the clip: the def clone inherits the media, the
    // figure keeps its own copy.
    mcp.call_ok("clip_create", json!({ "figure": 0 }));
    assert_eq!(
        asset_references(&mut mcp),
        (1, 1, 0),
        "both defs now reference the asset"
    );

    // Clear the FIGURE's copy (the shipped clear is
    // selection-scoped; steer the selection with the
    // shipped built-in first).
    mcp.call_ok(
        "set_selection",
        json!({ "figure": 0, "node": MEDIA_NODE }),
    );
    mcp.call_ok("clear_node_media", json!({}));
    let node = mcp.call_ok("get_node", json!({ "figure": 0, "node": MEDIA_NODE }));
    assert!(
        node["media"].is_null(),
        "the figure's own copy is cleared: {node}"
    );

    // What remains is clip-def-ONLY media: the census
    // counts exactly the clip-def reference…
    assert_eq!(
        asset_references(&mut mcp),
        (0, 1, 0),
        "only the clip-def reference remains"
    );
    // …C5's read sees it…
    // WAVE14-DEPENDS: C5 clip_get_node_media
    assert_image_media(
        &clip_read_media(&mut mcp, 0, MEDIA_NODE),
        0,
        "the clip read tool sees the orphaned media",
    );
    // …and it RENDERS, on both export surfaces, where
    // before Wave 14 it rendered on neither.
    let fallback_png = render_png(&mut mcp, &dir, "fallback.png");
    let (count, _) = magenta_stats(&fallback_png);
    assert!(
        count >= 800,
        "clip-def-only media renders via the fallback (got {count} magenta-ish pixels)"
    );
    let fallback_svg = render_svg_frame1(&mut mcp, &dir, "svg-fallback");
    assert_svg_carries_media(&fallback_svg, &png, "the fallback SVG");

    let _ = fs::remove_dir_all(&dir);
}
