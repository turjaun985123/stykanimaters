//! Wave 14, Squad C (session C7) — integration tests:
//! movieclip-def media as a SYSTEM, driven end-to-end
//! through real MCP dispatch (`McpServer::handle`).
//!
//! The defect this suite pins shut is the Wave 13 E5 audit
//! (docs/wave13/CLIP_MEDIA.md): a movieclip DEFINITION's
//! nodes can carry `SegmentMedia` (image / text) — it is
//! stored, serialised, counted by the asset census and
//! remapped on asset removal — but entering Wave 14 no
//! renderer ever sampled it, because the single sampling
//! funnel (`media::media_items`) iterated `project.figures`
//! only. Media that existed ONLY in a clip def (the state
//! reached when the originating figure's own copy is
//! cleared, or when a project file carrying clip-def media
//! is loaded) was invisible on every surface while still
//! locking its asset against `asset_remove`.
//!
//! Wave 14's C squad lands the fix per the audit's §6:
//! C1 puts the per-node fallback in the funnel (instance
//! media wins, the clip fills gaps, never both), C3
//! extends the raster's `has_media` gate, C4 covers the
//! SVG half, C5 adds direct clip-def media authoring to
//! MCP, C6 keeps the asset census honest. This suite is
//! the end-to-end witness for the five system behaviours:
//!
//!   1. a clip def with media, placed on two figures and
//!      sampled at two timeline frames — the media follows
//!      the def everywhere (two placements in one SVG
//!      frame, at two different frames);
//!   2. clip-def-only media survives a save -> load
//!      round-trip AND renders into a PNG export (a real
//!      pixel assertion on the exported frame);
//!   3. editing the def's media through MCP is exactly
//!      ONE undo step, and one undo removes it everywhere;
//!   4. `asset_remove` refuses while a clip def references
//!      the asset — with the clip-segment count named in
//!      the refusal — and succeeds once the clip is gone;
//!   5. a project whose only media lives in a clip def
//!      exports SVG that embeds the asset's PNG def and
//!      places it.
//!
//! DEPENDS convention: siblings C1/C3/C4/C5 land in
//! parallel with this file, so every assertion that rests
//! on a sibling's piece carries a `// DEPENDS:` comment
//! naming the sibling and the piece. At the time of
//! writing, C1's funnel fallback was already in the tree
//! (src/media.rs, "Wave 14, C1" comments); C3's gate
//! extension, C4's SVG audit and C5's clip-media tools
//! were not. Assertions with no DEPENDS marker are true
//! on the entering tree already (the model witnesses, the
//! round-trip storage half, and test 4's refusal — the
//! census was always honest; only the rendering was not).
//!
//! The setup trick that reaches the clip-def-ONLY state
//! through shipped tools alone (no C5 dependency):
//! `asset_attach` writes the figure's own def, then
//! `clip_create` clones that def into the new clip
//! (CLIP_MEDIA.md §4 path 1), then the selection-scoped
//! `clear_node_media` removes the figure's own copy —
//! leaving the media resident only in `clips[0].def`,
//! with figure 0 still looping the clip.
//!
//! Witness discipline (the Wave 12/13 house pattern):
//! value witnesses are strict and read the saved project
//! through the model types (`Project.clips[].def`),
//! whose shape no splice can change; rendered witnesses
//! read the actual export products — the exported PNG's
//! decoded pixels and the exported SVG's text — through
//! the shipped `render` io tool. The only tolerant code
//! is the C5 funnel in test 3, where the tool's NAME is
//! the sibling's to choose.
//!
//! Every step goes through MCP messages; direct
//! filesystem access is used only for what an MCP client
//! itself would observe from outside the protocol:
//! writing the import fixture and reading back saved /
//! exported files. Each test builds its own server and
//! its own temp dir and shares nothing, so tests never
//! depend on execution order. No test changes the process
//! current directory; every path passed to a tool is
//! absolute.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::{Project, SegmentMedia};

/// The node the whole suite attaches media to: node 1 of
/// the default humanoid is "chest" — a real segment
/// (parent = hips, node 0; length 46), so the media item
/// gets a real quad: 46 stage units long and, for the
/// square fixture below, 46 units tall.
const MEDIA_NODE: usize = 1;

/// The fixture's edge length in pixels (square, so the
/// media quad's aspect is 1 and its on-stage height
/// equals the segment length).
const FIXTURE_PX: u32 = 16;

/// A solid opaque red PNG, encoded at runtime with the
/// `png` crate: small, real enough for `asset_import`'s
/// decode validation, and a colour nothing else in a
/// default project paints (dark background, light figure
/// lines), so the exported-frame pixel count is an
/// unambiguous media witness.
fn red_png_bytes() -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, FIXTURE_PX, FIXTURE_PX);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header");
        let pixels = vec![255u8, 0, 0].repeat((FIXTURE_PX * FIXTURE_PX) as usize);
        writer.write_image_data(&pixels).expect("PNG pixels");
        writer.finish().expect("PNG finish");
    }
    buf
}

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 13 pattern)
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

    /// The registered tool names, from a live `tools/list`.
    fn tool_names(&mut self) -> BTreeSet<String> {
        let list = self.request("tools/list", json!({}));
        list["result"]["tools"]
            .as_array()
            .expect("tools/list returns an array")
            .iter()
            .map(|t| {
                t["name"]
                    .as_str()
                    .expect("every tool carries a name")
                    .to_string()
            })
            .collect()
    }
}

// ---------------------------------------------------------------------
// Shared session reads — strict witnesses (no tolerance)
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

/// Save through the shipped IO tool and return the whole
/// project exactly as it crossed the file boundary — the
/// ground-truth witness for everything the tools wrote,
/// independent of any tool's reply shape.
fn saved_project(mcp: &mut Dispatch, dir: &Path, file: &str) -> Project {
    let path = dir.join(file);
    mcp.call_ok(
        "save_project",
        json!({ "path": path.to_string_lossy() }),
    );
    read_project(&path)
}

/// The media on clip `clip`'s node `node`, witnessed
/// through the saved project (see `saved_project`).
fn clip_def_media(project: &Project, clip: usize, node: usize) -> Option<SegmentMedia> {
    project.clips[clip].def.nodes[node].media.clone()
}

/// The media on figure `figure`'s OWN def at `node`,
/// witnessed the same way.
fn figure_def_media(project: &Project, figure: usize, node: usize) -> Option<SegmentMedia> {
    project.figures[figure].def.nodes[node].media.clone()
}

/// Export every frame as SVG through the shipped `render`
/// io tool and return one frame's text. The writer names
/// its files `frame-NNNN.svg`, 1-based (src/svg.rs
/// `export_svg`), so timeline frame `frame` (0-based) is
/// `frame-(frame+1).svg` in the fresh output directory.
fn svg_frame(mcp: &mut Dispatch, dir: &Path, frame: usize) -> String {
    let out_dir = dir.join(format!("svg-{frame}"));
    mcp.call_ok(
        "render",
        json!({
            "format": "svg",
            "path": out_dir.to_string_lossy(),
        }),
    );
    let file = out_dir.join(format!("frame-{:04}.svg", frame + 1));
    fs::read_to_string(&file).expect("the SVG frame file exists")
}

/// How many placements of asset 0 an SVG frame carries:
/// the writer emits one `<use href="#asset0"` per placed
/// image item (one `<image id="asset0">` def per asset,
/// however many items reference it).
fn svg_use_count(svg: &str) -> usize {
    svg.matches("<use href=\"#asset0\"").count()
}

/// Render one timeline frame to a PNG file through the
/// shipped `render` io tool and count the fixture-red
/// pixels in the decoded image: the exported-frame
/// witness that the media actually composited.
fn red_pixel_count(mcp: &mut Dispatch, dir: &Path, frame: usize) -> usize {
    let path = dir.join(format!("frame-{frame}.png"));
    mcp.call_ok(
        "render",
        json!({
            "format": "png",
            "frame": frame,
            "path": path.to_string_lossy(),
        }),
    );
    let bytes = fs::read(&path).expect("the exported PNG exists");
    let img = image::load_from_memory(&bytes)
        .expect("the exported PNG decodes")
        .to_rgb8();
    img.pixels()
        .filter(|p| p[0] >= 200 && p[1] <= 90 && p[2] <= 90)
        .count()
}

/// A minimal standard-base64 decoder (padding-tolerant)
/// for the SVG data-URI payload: the suite compares the
/// EMBEDDED bytes with the fixture bytes, which is the
/// strictest form of "the SVG carries the media def".
fn base64_decode(text: &str) -> Vec<u8> {
    fn value_of(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let Some(v) = value_of(byte) else { continue };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// The base64 payload of the first PNG data URI in an
/// SVG text, if the document embeds one.
fn svg_embedded_png(svg: &str) -> Option<Vec<u8>> {
    let marker = "data:image/png;base64,";
    let start = svg.find(marker)? + marker.len();
    let tail = &svg[start..];
    let end = tail.find('"').expect("the data URI is quoted");
    Some(base64_decode(&tail[..end]))
}

// ---------------------------------------------------------------------
// The shared setup — a clip-def-ONLY media project, built
// from shipped tools alone (see the module doc)
// ---------------------------------------------------------------------

/// Build the suite's canonical state and return the
/// fixture PNG bytes:
///
///   * a fresh 4-frame project;
///   * asset 0 = the red fixture, attached to figure 0's
///     chest node (its own def);
///   * `clip_create` from figure 0 — clip 0's def inherits
///     the media by clone, and figure 0 loops the clip;
///   * figure 0's own copy cleared again through the
///     selection-scoped `clear_node_media`.
///
/// Net state: the ONLY media in the project lives in
/// `clips[0].def.nodes[MEDIA_NODE]`, and figure 0 is a
/// clip instance whose own def is media-free — exactly
/// the state the Wave 13 audit proved invisible. The
/// closing model witnesses are asserted HERE (they are
/// true on the entering tree) so every test starts from
/// a proven-identical setup.
fn clip_def_only_project(mcp: &mut Dispatch, dir: &Path) -> Vec<u8> {
    let fixture = red_png_bytes();
    mcp.call_ok("new_project", json!({ "name": "Wave14 Clip Media" }));
    for _ in 0..3 {
        mcp.call_ok("add_frame", json!({}));
    }
    let png_path = write_fixture(dir, "red.png", &fixture);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    mcp.call_ok(
        "asset_attach",
        json!({ "figure": 0, "node": MEDIA_NODE, "asset": 0 }),
    );
    let created = mcp.call_ok("clip_create", json!({ "figure": 0 }));
    assert_eq!(
        created["clip"].as_u64(),
        Some(0),
        "the first clip is index 0: {created}"
    );
    mcp.call_ok(
        "set_selection",
        json!({ "figure": 0, "node": MEDIA_NODE }),
    );
    mcp.call_ok("clear_node_media", json!({}));

    // The setup's contract, witnessed through the model:
    // the media lives in the clip def ONLY.
    let project = saved_project(mcp, dir, "setup.styk.json");
    assert_eq!(project.clips.len(), 1, "exactly one clip was created");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the clip def carries the image on its chest node"
    );
    assert_eq!(
        figure_def_media(&project, 0, MEDIA_NODE),
        None,
        "figure 0's own def is media-free after the clear"
    );
    assert_eq!(
        project.figures[0].clip,
        Some(0),
        "figure 0 loops clip 0"
    );
    fixture
}

// ---------------------------------------------------------------------
// The entering baseline — live on today's surface, no flip needed
// ---------------------------------------------------------------------

#[test]
fn entering_baseline_surface_and_unknown_tool_contract() {
    let mut mcp = Dispatch::new();

    let init = mcp.request("initialize", json!({}));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");

    // The floor is the Wave 13 census: Wave 14 only adds.
    let names = mcp.tool_names();
    assert!(
        names.len() >= 288,
        "the entering surface is the Wave 13 census of 288 tools, got {}",
        names.len()
    );
    // The shipped surface this suite drives: session setup,
    // the asset pair and its attach, the clip lifecycle,
    // the selection + clear pair the setup rides, and the
    // IO witnesses (save/load/render).
    for required in [
        "new_project",
        "add_frame",
        "asset_import",
        "assets_list",
        "asset_attach",
        "asset_remove",
        "clip_create",
        "clip_list",
        "clip_insert_figure",
        "clip_delete",
        "set_selection",
        "clear_node_media",
        "get_editor_status",
        "undo",
        "redo",
        "save_project",
        "load_project",
        "render",
    ] {
        assert!(
            names.contains(required),
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
// (1) The def's media follows it to every instance, at
//     every frame the clip plays on
// ---------------------------------------------------------------------

/// Place the media-carrying clip on a SECOND figure
/// (`clip_insert_figure` clones the clip def into the new
/// instance and points it at the clip): one SVG frame
/// then carries TWO placements of the def's asset — one
/// per instance — and it does so at two different
/// timeline frames, because the clip loops its media
/// with its poses.
#[test]
fn clip_def_media_follows_the_def_across_figures_and_frames() {
    let dir = temp_dir("follows");
    let mut mcp = Dispatch::new();
    clip_def_only_project(&mut mcp, &dir);

    // The second placement.
    let inserted = mcp.call_ok("clip_insert_figure", json!({ "index": 0 }));
    assert_eq!(
        inserted["figure"].as_u64(),
        Some(1),
        "the inserted instance is figure 1: {inserted}"
    );

    // Model witnesses (no DEPENDS — true on the entering
    // tree): both figures loop clip 0; the clip def holds
    // the only authored media, and the inserted figure's
    // own def carries the clone it was created with.
    let project = saved_project(&mut mcp, &dir, "two-figures.styk.json");
    assert_eq!(project.figures.len(), 2, "two figures after the insert");
    assert_eq!(project.figures[0].clip, Some(0), "figure 0 loops clip 0");
    assert_eq!(project.figures[1].clip, Some(0), "figure 1 loops clip 0");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the def still carries the media"
    );
    assert_eq!(
        figure_def_media(&project, 1, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the inserted instance was cloned from the def, media included"
    );
    assert_eq!(
        figure_def_media(&project, 0, MEDIA_NODE),
        None,
        "figure 0's own def is still media-free — any placement \
         it makes can only come from the clip def"
    );

    // DEPENDS: C1 — the per-node fallback in
    // `media::media_items` (docs/wave13/CLIP_MEDIA.md
    // §6.1). Figure 1 places the asset through its own
    // cloned def on any tree; figure 0 — whose own def is
    // media-free — places it ONLY through the clip-def
    // fallback. Two `<use>` placements in one frame is
    // therefore the fallback's signature: the entering
    // funnel yields exactly one.
    for frame in [0usize, 2] {
        let svg = svg_frame(&mut mcp, &dir, frame);
        assert!(
            svg.contains("<image id=\"asset0\""),
            "frame {frame}: the SVG embeds the asset def: {svg}"
        );
        assert_eq!(
            svg_use_count(&svg),
            2,
            "frame {frame}: BOTH instances place the def's media \
             (figure 0 via the clip-def fallback, figure 1 via its clone)"
        );
    }

    // The raster agrees the media is on stage at frame 0
    // (a presence witness only — on its own it cannot say
    // which instance painted, which is why the SVG count
    // above is the discriminating pin).
    let reds = red_pixel_count(&mut mcp, &dir, 0);
    assert!(
        reds > 100,
        "frame 0's PNG export paints the media (red pixels: {reds})"
    );

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// (2) Round-trip + the raster: stored faithfully, and
//     actually composited into an exported PNG
// ---------------------------------------------------------------------

/// Save the clip-def-only project, load it back through
/// the shipped IO tools, and prove both halves of the
/// fix: the media is still in the def after the file
/// round-trip (the storage half — always true), and a
/// PNG exported from the LOADED project contains the
/// image's pixels at two different frames (the render
/// half — the audit's whole point).
#[test]
fn clip_def_media_survives_save_load_and_renders_to_png() {
    let dir = temp_dir("roundtrip");
    let mut mcp = Dispatch::new();
    clip_def_only_project(&mut mcp, &dir);

    // Round-trip through the file boundary.
    let path = dir.join("roundtrip.styk.json");
    mcp.call_ok(
        "save_project",
        json!({ "path": path.to_string_lossy() }),
    );
    mcp.call_ok(
        "load_project",
        json!({ "path": path.to_string_lossy() }),
    );

    // Storage half (no DEPENDS): the loaded project is the
    // saved project — def media, figure assignment and
    // asset table intact.
    let project = saved_project(&mut mcp, &dir, "after-load.styk.json");
    assert_eq!(project.clips.len(), 1, "the clip survives the round-trip");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the def media survives save -> load"
    );
    assert_eq!(
        figure_def_media(&project, 0, MEDIA_NODE),
        None,
        "figure 0's own def is still media-free after the load"
    );
    assert_eq!(project.figures[0].clip, Some(0), "figure 0 still loops clip 0");
    assert_eq!(project.assets.len(), 1, "the asset survives the round-trip");

    // DEPENDS: C1 — the funnel fallback (as test 1): with
    // the figure's own def media-free, the funnel yields
    // the image only if it samples the clip def.
    // DEPENDS: C3 — the raster gate extension
    // (CLIP_MEDIA.md §6.4): this project has NO figure-def
    // media, no sprites and no text instances, so the
    // entering `has_media` gate in export.rs skips
    // `composite_media` entirely and the exported frames
    // would be media-free even with the funnel fixed.
    // Together: red pixels in the export are reachable
    // only when both pieces have landed.
    for frame in [0usize, 2] {
        let reds = red_pixel_count(&mut mcp, &dir, frame);
        assert!(
            reds > 100,
            "frame {frame}: the loaded project's PNG export composites \
             the clip-def image (red pixels: {reds})"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// (3) The def-media edit: one undo step, and undo takes
//     it back everywhere
// ---------------------------------------------------------------------

// DEPENDS: C5 — the clip-def media attach tool. Entering
// Wave 14 there is NO MCP path that writes media onto a
// clip def's node directly (CLIP_MEDIA.md §4: every
// production media writer targets figure defs; clip-def
// media arises only by clone). C5's remit is to close
// exactly that. The tool's NAME is C5's to choose, so the
// funnel below probes the plausible spellings against a
// live `tools/list` and drives the first that is
// registered; the argument funnel tries the clip index
// under `clip` first, then under `index` (the clip
// family uses both: `clip_create`'s reply says `clip`,
// `clip_insert_figure` / `set_clip_loop` take `index`).
// Until C5 lands, `pick_clip_media_attach` panics with a
// clear message — deliberately: no test in this suite
// may pass green against a missing surface.
const CLIP_MEDIA_ATTACH_CANDIDATES: &[&str] = &[
    "clip_asset_attach",
    "clip_media_attach",
    "clip_attach_media",
    "asset_attach_clip",
    "clip_set_node_media",
    "clip_set_media",
];

fn pick_clip_media_attach(mcp: &mut Dispatch) -> String {
    let names = mcp.tool_names();
    CLIP_MEDIA_ATTACH_CANDIDATES
        .iter()
        .find(|c| names.contains(**c))
        .map(|c| c.to_string())
        .unwrap_or_else(|| {
            panic!(
                "no clip-def media attach tool is registered \
                 (DEPENDS: C5 — probed {CLIP_MEDIA_ATTACH_CANDIDATES:?})"
            )
        })
}

// DEPENDS: C5 — see the candidate list above. Attaches
// asset `asset` to clip `clip`'s node `node` and returns
// the success payload.
fn clip_media_attach(mcp: &mut Dispatch, tool: &str, clip: u64, node: u64, asset: u64) -> Value {
    let outcome = mcp.call(
        tool,
        json!({ "clip": clip, "node": node, "asset": asset }),
    );
    if !outcome.is_error {
        return outcome.payload;
    }
    // The clip family's other spelling for the clip index.
    if outcome.text.contains("missing") || outcome.text.contains("index") {
        let retried = mcp.call(
            tool,
            json!({ "index": clip, "node": node, "asset": asset }),
        );
        assert!(
            !retried.is_error,
            "tool {tool} failed under both clip-index spellings: {}",
            retried.text
        );
        return retried.payload;
    }
    panic!("tool {tool} failed: {}", outcome.text);
}

/// Author media DIRECTLY on a clip def (a clip made from
/// a media-free figure, so the def starts clean): the
/// attach is exactly one undo step; the def then carries
/// the media and the clip's instance renders it; one
/// undo removes the media from the def and from every
/// surface at once; redo restores it.
// DEPENDS: C5 for the edit itself (funnel above);
// DEPENDS: C1 for the rendered witness (the instance's
// own def never carries the media in this test, so its
// SVG placement exists only through the fallback).
#[test]
fn clip_def_media_edit_is_exactly_one_undo_step() {
    let dir = temp_dir("edit-undo");
    let mut mcp = Dispatch::new();

    // A clip whose def starts media-free: import AFTER
    // the create, so no clone path can smuggle media in.
    mcp.call_ok("new_project", json!({ "name": "Wave14 Clip Media Edit" }));
    for _ in 0..3 {
        mcp.call_ok("add_frame", json!({}));
    }
    let created = mcp.call_ok("clip_create", json!({ "figure": 0 }));
    assert_eq!(created["clip"].as_u64(), Some(0), "the clip: {created}");
    let fixture = red_png_bytes();
    let png_path = write_fixture(&dir, "red.png", &fixture);
    mcp.call_ok("asset_import", json!({ "path": png_path }));
    let project = saved_project(&mut mcp, &dir, "before-edit.styk.json");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        None,
        "the def starts media-free"
    );

    // DEPENDS: C5 — the attach tool (name probed).
    let tool = pick_clip_media_attach(&mut mcp);
    let depth_before = undo_depth(&mut mcp);
    clip_media_attach(&mut mcp, &tool, 0, MEDIA_NODE as u64, 0);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before + 1,
        "the clip-def media attach is exactly one undo step"
    );
    let project = saved_project(&mut mcp, &dir, "after-edit.styk.json");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the def now carries the media"
    );
    assert_eq!(
        figure_def_media(&project, 0, MEDIA_NODE),
        None,
        "the edit wrote the DEF, not the instance's own def"
    );

    // DEPENDS: C1 — the instance renders the def's media
    // through the fallback (its own def is media-free).
    let svg = svg_frame(&mut mcp, &dir, 0);
    assert_eq!(
        svg_use_count(&svg),
        1,
        "the instance places the def's media in the SVG: {svg}"
    );

    // ONE undo removes it — from the def (model witness)
    // and from the rendered surface alike.
    mcp.call_ok("undo", json!({}));
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "the undo consumed exactly the attach's step"
    );
    let project = saved_project(&mut mcp, &dir, "after-undo.styk.json");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        None,
        "one undo removed the media from the def"
    );
    let svg = svg_frame(&mut mcp, &dir, 0);
    assert!(
        !svg.contains("data:image/png;base64,"),
        "one undo removed the media from the SVG surface too: {svg}"
    );
    assert_eq!(svg_use_count(&svg), 0, "no placement survives the undo");

    // Redo restores the def media in one step.
    mcp.call_ok("redo", json!({}));
    let project = saved_project(&mut mcp, &dir, "after-redo.styk.json");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "redo restored the def media"
    );

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// (4) The census stays honest: a clip-def-only reference
//     blocks asset_remove until the clip is gone
// ---------------------------------------------------------------------

/// The sharpest symptom from the audit, pinned at the
/// system level: in the clip-def-only state, the asset
/// is referenced by NOTHING visible in the figure/sprite
/// census — yet `asset_remove` must refuse, naming the
/// clip segment in its counts, and must spend no undo
/// step doing so. Deleting the clip clears the last
/// reference, and the same removal then succeeds.
///
/// No DEPENDS marker: the census (`asset_references`)
/// counted clip-def media correctly even while nothing
/// rendered it, so this behaviour is true on the
/// entering tree; C6's remit is to KEEP it true now
/// that the media renders.
#[test]
fn asset_remove_refuses_a_clip_def_only_reference() {
    let dir = temp_dir("refusal");
    let mut mcp = Dispatch::new();
    clip_def_only_project(&mut mcp, &dir);

    // The refusal: an isError result (never a panic, never
    // a silent success) whose counts name the clip
    // segment — the one reference class in play.
    let depth_before = undo_depth(&mut mcp);
    let err = mcp.call_err("asset_remove", json!({ "index": 0 }));
    assert!(
        err.contains("1 clip segment(s)"),
        "the refusal names the clip-def reference in its counts: {err}"
    );
    assert!(
        err.contains("0 segment image(s)"),
        "no figure segment references the asset in this state: {err}"
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "the refused removal spends no undo step"
    );
    let project = saved_project(&mut mcp, &dir, "still-referenced.styk.json");
    assert_eq!(project.assets.len(), 1, "the refused removal kept the asset");
    assert_eq!(
        clip_def_media(&project, 0, MEDIA_NODE),
        Some(SegmentMedia::Image { asset: 0 }),
        "the refused removal kept the def media"
    );

    // Delete the clip: the last reference goes with it,
    // and the same removal now succeeds.
    mcp.call_ok("clip_delete", json!({ "index": 0 }));
    mcp.call_ok("asset_remove", json!({ "index": 0 }));
    let project = saved_project(&mut mcp, &dir, "after-removal.styk.json");
    assert_eq!(project.clips.len(), 0, "the clip is gone");
    assert_eq!(
        project.assets.len(),
        0,
        "the unreferenced asset was removed"
    );

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// (5) The SVG half: the export embeds the def and places
//     the clip-sourced item
// ---------------------------------------------------------------------

/// In the clip-def-only state (single figure), the SVG
/// export of frame 0 embeds the asset's PNG as a
/// document def — byte-for-byte the imported fixture —
/// and places it exactly once, for the one instance,
/// whose own def carries nothing.
// DEPENDS: C1 — the funnel fallback produces the item
// (the instance's own def is media-free; the entering
// funnel yields no item at all here).
// DEPENDS: C4 — the SVG emission of clip-sourced items
// (the writer consumes the funnel unconditionally, so
// this rides C1's items; C4's audit owns the verdict
// that nothing in the SVG path filters them back out).
#[test]
fn svg_export_embeds_the_clip_media_asset_def() {
    let dir = temp_dir("svg");
    let mut mcp = Dispatch::new();
    let fixture = clip_def_only_project(&mut mcp, &dir);

    let svg = svg_frame(&mut mcp, &dir, 0);
    assert!(
        svg.contains("<image id=\"asset0\""),
        "the SVG carries the asset's image def: {svg}"
    );
    assert!(
        svg.contains("data:image/png;base64,"),
        "the def embeds the PNG as a data URI: {svg}"
    );
    assert_eq!(
        svg_embedded_png(&svg),
        Some(fixture),
        "the embedded bytes are the imported asset's bytes, verbatim"
    );
    assert_eq!(
        svg_use_count(&svg),
        1,
        "the single clip instance places the def's media exactly once"
    );

    let _ = fs::remove_dir_all(&dir);
}
