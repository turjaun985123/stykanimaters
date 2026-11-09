//! Integration tests: Wave 11 end-to-end MCP session — 3D figures.
//!
//! One full session driven ONLY through real `McpServer`
//! JSON-RPC messages (`tools/list` / `tools/call` via
//! `McpServer::handle`, plus the live-control `handle_on`
//! route): list the 3D library, load a figure by name, orbit
//! the camera past a full turn, bake two library figures into
//! the project, delete one, and probe the negative space
//! (no-session calls, absent names, out-of-range indices,
//! clamped extremes). Assertions on the resulting core state
//! read `server.core` directly, the pattern `mcp.rs`'s own
//! tests use.
//!
//! Scope honesty: integration tests cannot render the Rune
//! shell (`rune_sec::stage_fig3` lives in the binary crate),
//! so "the stage can draw this figure" is asserted at the
//! level the stage itself consumes — `fig3d::evaluate3` +
//! `fig3d::project` + `fig3d::segments2d` over the session
//! state, which is exactly the pipeline
//! `stage_fig3_prims` is built on.
//!
//! The 3D library is a real directory (`./library3d` beside
//! the working directory — `EditorCore::fig3_library_dir`),
//! shared mutable state, so the whole session is ONE test:
//! parallel tests must not race over it. The test removes
//! the files it creates (and any stale copy from a crashed
//! earlier run) so the directory is left as it was found.
//!
//! Two fixture notes:
//!
//! - The session figure a fresh `fig3_enter` spawns is
//!   `fig3d::humanoid3()`: name "Humanoid3", 15 nodes
//!   (hips/chest/head, both arms, both legs — fig3d.rs).
//! - Fixture note: `fig3_save_to_library` saves under the
//!   session figure's own name; since the Wave 11 splice a
//!   session figure CAN be renamed first (`fig3_rename`,
//!   exercised in the live-control test below), so a second
//!   distinctly-named entry is reachable through MCP. The
//!   second entry below is still seeded with the crate's
//!   public `fig3d::save_library_figure` — it also needs a
//!   specific colour and a posed head, which is fixture
//!   setup, not a session step; every session step after
//!   that goes through MCP messages only.

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fig3d;
use stykanimate_rs::mcp::McpServer;

const BOT_NAME: &str = "Wave11 Bot";
const BOT_COLOR: u32 = 0x3366FF;
/// Node count of `fig3d::humanoid3()` at source: hips,
/// chest, head, upperArm/foreArm/hand ×2, thigh/shin/foot ×2.
const HUMANOID_NODES: usize = 15;

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Drive one `tools/call` through `McpServer::handle` and
/// return the parsed payload of its text content block plus
/// the `isError` flag — the `mcp.rs` test helper, verbatim
/// in spirit. Error payloads are plain text, so they come
/// back as `Value::String`.
fn call_payload(server: &mut McpServer, name: &str, args: Value) -> (Value, bool) {
    let resp = server
        .handle(&request(1, "tools/call", json!({ "name": name, "arguments": args })))
        .expect("tools/call is a request and must be answered");
    let result = &resp["result"];
    let text = result["content"][0]["text"]
        .as_str()
        .expect("content block carries text");
    let is_error = result["isError"].as_bool().unwrap_or(false);
    (
        serde_json::from_str(text).unwrap_or(Value::String(text.to_string())),
        is_error,
    )
}

/// Same, routed through `McpServer::handle_on` against a
/// caller-owned core (the live-control path).
fn call_payload_on(
    server: &mut McpServer,
    core: &mut EditorCore,
    name: &str,
    args: Value,
) -> (Value, bool) {
    let resp = server
        .handle_on(
            core,
            &request(1, "tools/call", json!({ "name": name, "arguments": args })),
        )
        .expect("tools/call is a request and must be answered");
    let result = &resp["result"];
    let text = result["content"][0]["text"]
        .as_str()
        .expect("content block carries text");
    let is_error = result["isError"].as_bool().unwrap_or(false);
    (
        serde_json::from_str(text).unwrap_or(Value::String(text.to_string())),
        is_error,
    )
}

/// The error text of a failed call (error payloads are not
/// JSON, so `call_payload` returns them as a string value).
fn err_text(out: &Value) -> &str {
    out.as_str().expect("error payload is plain text")
}

/// A JSON number as f64. Reply floats are f32s serialised
/// through JSON, so exact equality is the wrong tool for
/// them; callers compare with a tolerance.
fn num(v: &Value) -> f64 {
    v.as_f64().expect("expected a JSON number")
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: expected {expected}, got {actual}"
    );
}

/// The directory `EditorCore` scans for the 3D library:
/// `./library3d` under the process working directory
/// (`EditorCore::fig3_library_dir` is private; this mirrors
/// it the way the panel docs describe it).
fn library_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("library3d")
}

/// Display names stored in the library directory right now,
/// read straight from the files (each `fig3d-*.json` holds
/// `{name, figure, pose}`) — the independent cross-check for
/// what `fig3_library_list` reports, not a re-call of the
/// code under test.
fn library_names_on_disk(dir: &PathBuf) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file.starts_with("fig3d-") && file.ends_with(".json") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                        if let Some(name) = parsed["name"].as_str() {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}

#[test]
fn fig3_tools_are_listed_with_their_schemas() {
    let mut server = McpServer::new();
    let resp = server
        .handle(&request(2, "tools/list", json!({})))
        .expect("tools/list must be answered");
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let find = |name: &str| {
        tools
            .iter()
            .find(|t| (*t)["name"] == name)
            .unwrap_or_else(|| panic!("tools/list is missing {name}"))
    };
    for name in [
        "fig3_enter",
        "fig3_select",
        "fig3_rotate",
        "fig3_orbit",
        "fig3_bake",
        "fig3_mark_b",
        "fig3_bake_animation",
        "fig3_library_list",
        "fig3_load_from_library",
        "fig3_save_to_library",
    ] {
        find(name);
    }
    // The name argument belongs to the LOAD tool.
    // `fig3_library_list` itself takes no arguments — the
    // session brief's "library_list with a name arg" is the
    // load tool's schema, verified here at the surface.
    let load = find("fig3_load_from_library");
    assert_eq!(load["inputSchema"]["required"], json!(["name"]));
    assert_eq!(load["inputSchema"]["properties"]["name"]["type"], "string");
    let list = find("fig3_library_list");
    let list_required = &list["inputSchema"]["required"];
    assert!(
        list_required.is_null() || list_required == &json!([]),
        "fig3_library_list takes no arguments, got required {list_required}"
    );
    // Orbit is a two-delta call: both are required (there is
    // no single-axis orbit call).
    let orbit = find("fig3_orbit");
    let required = orbit["inputSchema"]["required"]
        .as_array()
        .expect("orbit required array");
    assert!(required.contains(&json!("yaw")), "orbit requires yaw");
    assert!(required.contains(&json!("pitch")), "orbit requires pitch");
    // fig3_exit ends a session (EditorCore::exit_3d, the
    // GUI's mode toggle) — registered in the Wave 11 splice;
    // before it, a session started over MCP could only be
    // replaced (load) or outlived. fig3_rename and
    // fig3_set_camera landed in the same wave.
    for name in ["fig3_exit", "fig3_rename", "fig3_set_camera"] {
        assert!(
            tools.iter().any(|t| t["name"] == name),
            "{name} must be registered"
        );
    }
}

#[test]
fn fig3_live_control_route_drives_an_external_core() {
    let mut server = McpServer::new();
    let mut external = EditorCore::new();

    let (out, is_error) = call_payload_on(&mut server, &mut external, "fig3_enter", json!({}));
    assert!(!is_error);
    assert_eq!(out["figure"], "Humanoid3");
    assert!(
        external.fig3.is_some(),
        "the passed core holds the session"
    );

    // fig3_rename (Wave 11): the session figure takes a new
    // name — the name a later fig3_save_to_library would file
    // it under. Before this tool, library entries were stuck
    // with the spawn name.
    let (out, is_error) = call_payload_on(
        &mut server,
        &mut external,
        "fig3_rename",
        json!({ "name": "Live Figure" }),
    );
    assert!(!is_error, "fig3_rename works on the live core: {out}");
    assert_eq!(out["figure"], "Live Figure");
    assert_eq!(external.fig3.as_ref().unwrap().figure.name, "Live Figure");
    assert!(
        server.core.fig3.is_none(),
        "the router's own core must stay sessionless"
    );

    let (out, is_error) = call_payload_on(
        &mut server,
        &mut external,
        "fig3_orbit",
        json!({ "yaw": 400.0, "pitch": 0.0 }),
    );
    assert!(!is_error);
    let st = external.fig3.as_ref().expect("session still active");
    assert_close(st.camera.yaw as f64, 400.0, "external core stored yaw");
    assert!(
        external.status.contains("yaw 40"),
        "status reports the normalised yaw: {}",
        external.status
    );
    assert_close(num(&out["camera"]["yaw"]), 400.0, "live reply yaw");
}

#[test]
fn fig3_session_end_to_end_via_mcp() {
    let mut server = McpServer::new();
    let demo_figures = server.core.project.figures.len();
    let demo_frames = server.core.project.frames.len();
    assert_eq!(demo_figures, 1, "demo project starts with one figure");
    assert!(server.core.fig3.is_none(), "no session yet");

    let dir = library_dir();
    let humanoid_path = dir.join(fig3d::library_file_name("Humanoid3"));
    let humanoid_existed = humanoid_path.exists();
    let bot_path = dir.join(fig3d::library_file_name(BOT_NAME));
    // A crashed earlier run may have left the seeded entry.
    let _ = std::fs::remove_file(&bot_path);

    // ---- Phase 0: no session — every session tool refuses
    // cleanly, and nothing panics or half-starts.
    for (name, args) in [
        ("fig3_orbit", json!({ "yaw": 10.0, "pitch": 5.0 })),
        ("fig3_select", json!({ "joint": 0 })),
        ("fig3_rotate", json!({ "axis": 0, "degrees": 10.0 })),
        ("fig3_bake", json!({})),
        ("fig3_mark_b", json!({})),
        ("fig3_bake_animation", json!({ "frames": 4 })),
    ] {
        let (_out, is_error) = call_payload(&mut server, name, args);
        assert!(is_error, "{name} without a session refuses");
        assert!(server.core.fig3.is_none(), "{name} started no session");
    }
    let (out, is_error) = call_payload(&mut server, "fig3_save_to_library", json!({}));
    assert!(is_error, "saving without a session refuses");
    assert!(
        err_text(&out).contains("Enter 3D mode first"),
        "save refusal names the fix: {}",
        err_text(&out)
    );
    // Spawning by a name the library does not hold refuses,
    // with the name in the message, and starts nothing.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_load_from_library",
        json!({ "name": "Wave11 No Such Figure" }),
    );
    assert!(is_error, "loading an absent library figure refuses");
    assert!(
        err_text(&out).contains("No 3D library figure named 'Wave11 No Such Figure'"),
        "load refusal names the figure: {}",
        err_text(&out)
    );
    assert!(server.core.fig3.is_none());
    // A missing name argument is an argument error, not a
    // silent default.
    let (out, is_error) = call_payload(&mut server, "fig3_load_from_library", json!({}));
    assert!(is_error);
    assert!(
        err_text(&out).contains("missing required argument 'name'"),
        "missing-arg wording: {}",
        err_text(&out)
    );

    // ---- Phase 1: enter, save the spawn to the library,
    // seed the second entry (fixture — see module docs),
    // then list.
    let (out, is_error) = call_payload(&mut server, "fig3_enter", json!({}));
    assert!(!is_error);
    assert_eq!(out["active"], true);
    assert_eq!(out["figure"], "Humanoid3");
    assert_eq!(out["joints"].as_u64(), Some(HUMANOID_NODES as u64));
    {
        let st = server.core.fig3.as_ref().expect("session started");
        assert_eq!(st.figure.name, "Humanoid3");
        assert_eq!(st.figure.nodes.len(), HUMANOID_NODES);
        assert_eq!(st.selected, 0);
        assert!(st.pose_b.is_none());
        // The spawn path's camera is fig3d::default_camera.
        assert_close(st.camera.yaw as f64, 0.0, "spawn yaw");
        assert_close(st.camera.pitch as f64, 0.0, "spawn pitch");
        assert_close(st.camera.distance as f64, 400.0, "spawn distance");
        assert_close(st.camera.fov_deg as f64, 40.0, "spawn fov");
        assert_eq!(st.camera.target, [0.0, 0.0, 0.0]);
        assert_eq!(st.pose.rotations.len(), HUMANOID_NODES);
        assert!(st.pose.rotations.iter().all(|r| *r == [0.0, 0.0, 0.0]));
    }

    let (out, is_error) = call_payload(&mut server, "fig3_save_to_library", json!({}));
    assert!(!is_error, "saving the session figure works: {out}");
    assert!(
        humanoid_path.exists(),
        "the save wrote {}",
        humanoid_path.display()
    );

    // Fixture seed (see the module docs: this entry needs a
    // specific colour and posed head, so it is seeded
    // directly). A renamed, recoloured humanoid in a
    // turned-head pose, written by the same public save
    // fig3_save_to_library wraps.
    let mut bot_fig = fig3d::humanoid3();
    bot_fig.name = BOT_NAME.to_string();
    bot_fig.color = BOT_COLOR;
    let mut bot_pose = fig3d::Pose3::rest(&bot_fig, [0.0, 0.0, 0.0]);
    bot_pose.rotations[2] = [0.0, 30.0, 0.0]; // head, Y
    fig3d::save_library_figure(&dir, BOT_NAME, &bot_fig, &bot_pose)
        .expect("seed the second library entry");
    assert!(bot_path.exists());

    let (out, is_error) = call_payload(&mut server, "fig3_library_list", json!({}));
    assert!(!is_error);
    assert_eq!(out["ok"], true);
    let listed: Vec<String> = out["figures"]
        .as_array()
        .expect("figures is an array")
        .iter()
        .map(|v| {
            // The panel's library rows show a name and a
            // Load button — nothing else — so the reply
            // carrying bare name strings is exact parity,
            // not a thin reply.
            v.as_str().expect("library entries are bare names").to_string()
        })
        .collect();
    assert!(
        listed.contains(&"Humanoid3".to_string()),
        "saved figure listed: {listed:?}"
    );
    assert!(
        listed.contains(&BOT_NAME.to_string()),
        "seeded figure listed: {listed:?}"
    );
    // Sorted, deduplicated (scan_library's contract).
    let mut sorted = listed.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(listed, sorted, "list is sorted without duplicates");
    // Every reported name is a real entry on disk.
    let on_disk = library_names_on_disk(&dir);
    for name in &listed {
        assert!(
            on_disk.contains(name),
            "listed name {name} has no library file on disk"
        );
    }

    // ---- Phase 2: load ("spawn") the second figure by
    // name and read the session back at core/model level.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_load_from_library",
        json!({ "name": BOT_NAME }),
    );
    assert!(!is_error);
    assert_eq!(out["name"], BOT_NAME);
    assert!(
        out["status"].as_str().unwrap_or("").contains("Loaded 3D figure"),
        "load status: {out}"
    );
    let stage = server.core.project.stage;
    {
        let st = server.core.fig3.as_ref().expect("load keeps a session");
        assert_eq!(st.figure.name, BOT_NAME);
        assert_eq!(st.figure.color, BOT_COLOR);
        assert_eq!(st.figure.nodes.len(), HUMANOID_NODES);
        // The load path's session state (fig3_load_from_
        // library): selection resets, pose B clears, the
        // saved pose comes back whole.
        assert_eq!(st.selected, 0, "load resets the selection");
        assert!(st.pose_b.is_none(), "load clears pose B");
        assert_eq!(st.pose.rotations.len(), HUMANOID_NODES);
        assert_eq!(st.pose.rotations[2], [0.0, 30.0, 0.0], "saved head turn");
        // The session already existed, so its camera is
        // KEPT — still the untouched default here.
        assert_close(st.camera.yaw as f64, 0.0, "kept camera yaw");
        assert_close(st.camera.distance as f64, 400.0, "kept camera distance");

        // Projection readiness — the exact pipeline the
        // stage consumes (evaluate3 -> project ->
        // segments2d): every joint projects in front of
        // the camera with a usable scale, and every
        // parented node yields a segment.
        let world = fig3d::evaluate3(&st.figure, &st.pose);
        assert_eq!(world.len(), HUMANOID_NODES);
        assert!(
            world.iter().all(|p| p.iter().all(|c| c.is_finite())),
            "world points are finite"
        );
        let proj = fig3d::project(&world, &st.camera, stage);
        assert_eq!(proj.len(), HUMANOID_NODES);
        for p in &proj {
            assert!(p.xy[0].is_finite() && p.xy[1].is_finite(), "proj xy finite");
            assert!(
                p.depth < fig3d::BEHIND_DEPTH,
                "no joint is behind the camera (depth {})",
                p.depth
            );
            assert!(p.scale.is_finite() && p.scale > 0.0, "proj scale usable");
        }
        let segments = fig3d::segments2d(&st.figure, &proj);
        assert_eq!(
            segments.len(),
            HUMANOID_NODES - 1,
            "one segment per parented node"
        );
    }

    // ---- Phase 3: orbit — past a full turn, then to the
    // pitch clamps. Stored yaw accumulates RAW (fig3d's
    // view frame consumes yaw through sin/cos, so the
    // stored value is periodic by construction); the
    // editor's status line reports norm_deg of it.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_orbit",
        json!({ "yaw": 450.0, "pitch": 10.0 }),
    );
    assert!(!is_error);
    assert_close(
        server.core.fig3.as_ref().unwrap().camera.yaw as f64,
        450.0,
        "stored yaw accumulates past 360"
    );
    assert!(
        out["status"].as_str().unwrap_or("").contains("yaw 90"),
        "status reports the normalised angle: {out}"
    );
    // The tool reply's camera.yaw reports the RAW
    // accumulated value (450) while the same reply's status
    // reports the normalised angle (90) — and since the
    // Wave 11 splice the reply ALSO carries yaw_wrapped, so
    // a structured-reading client gets the angle the user
    // sees named without applying norm_deg itself. The raw
    // field stays (accumulation is real state: orbit deltas
    // build on it).
    assert_close(num(&out["camera"]["yaw"]), 450.0, "reply yaw is raw");
    assert_close(
        num(&out["camera"]["yaw_wrapped"]),
        90.0,
        "reply yaw_wrapped is the normalised angle"
    );
    assert_close(num(&out["camera"]["pitch"]), 10.0, "reply pitch");

    let (out, is_error) = call_payload(
        &mut server,
        "fig3_orbit",
        json!({ "yaw": -180.0, "pitch": 200.0 }),
    );
    assert!(!is_error);
    {
        let st = server.core.fig3.as_ref().unwrap();
        assert_close(st.camera.yaw as f64, 270.0, "stored yaw 450 - 180");
        assert_close(st.camera.pitch as f64, 89.9, "pitch clamps at +89.9");
    }
    assert!(
        out["status"].as_str().unwrap_or("").contains("yaw -90"),
        "270 normalises to -90 in the status: {out}"
    );
    assert_close(num(&out["camera"]["pitch"]), 89.9, "reply pitch clamped");

    let (_out, is_error) = call_payload(
        &mut server,
        "fig3_orbit",
        json!({ "yaw": 0.0, "pitch": -500.0 }),
    );
    assert!(!is_error);
    {
        let st = server.core.fig3.as_ref().unwrap();
        assert_close(st.camera.pitch as f64, -89.9, "pitch clamps at -89.9");
        assert_close(st.camera.yaw as f64, 270.0, "yaw untouched by pitch-only orbit");
        // Periodicity of the stored semantics: projecting
        // with the stored 270 must match projecting with
        // its normalised twin -90.
        let world = fig3d::evaluate3(&st.figure, &st.pose);
        let a = fig3d::project(&world, &st.camera, stage);
        let mut twin = st.camera.clone();
        twin.yaw = -90.0;
        let b = fig3d::project(&world, &twin, stage);
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert!(
                (pa.xy[0] - pb.xy[0]).abs() < 0.5 && (pa.xy[1] - pb.xy[1]).abs() < 0.5,
                "yaw 270 projects like yaw -90: {pa:?} vs {pb:?}"
            );
        }
    }
    // One delta cannot be omitted: both are required.
    let (out, is_error) = call_payload(&mut server, "fig3_orbit", json!({ "yaw": 5.0 }));
    assert!(is_error, "orbit without pitch refuses");
    assert!(
        err_text(&out).contains("missing required argument 'pitch'"),
        "missing-pitch wording: {}",
        err_text(&out)
    );

    // Joint selection bounds: the figure index space of
    // the session. 15 is one past the last joint.
    let (out, is_error) = call_payload(&mut server, "fig3_select", json!({ "joint": 15 }));
    assert!(is_error, "joint 15 is out of range for 15 joints");
    assert!(
        err_text(&out).contains("out of range"),
        "select refusal: {}",
        err_text(&out)
    );
    let (out, is_error) = call_payload(&mut server, "fig3_select", json!({ "joint": 14 }));
    assert!(!is_error);
    assert_eq!(out["selected"].as_u64(), Some(14));
    assert_eq!(out["selected_name"], "foot.R");
    // A bad rotation axis refuses before anything moves.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_rotate",
        json!({ "axis": 3, "degrees": 10.0 }),
    );
    assert!(is_error);
    assert!(
        err_text(&out).contains("must be 0 (X), 1 (Y) or 2 (Z)"),
        "axis refusal: {}",
        err_text(&out)
    );
    // Baking an animation with no pose B marked refuses
    // and adds nothing.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_bake_animation",
        json!({ "frames": 4 }),
    );
    assert!(is_error, "bake_animation without pose B refuses");
    assert!(
        err_text(&out).contains("Mark a pose B first"),
        "pose-B refusal: {}",
        err_text(&out)
    );
    assert_eq!(server.core.project.figures.len(), demo_figures);

    // ---- Phase 4: bake both library figures into the
    // project, read the project back through MCP, delete
    // one through MCP.
    let bot_index = demo_figures;
    let (out, is_error) = call_payload(&mut server, "fig3_bake", json!({}));
    assert!(!is_error);
    assert_eq!(out["figure"].as_u64(), Some(bot_index as u64));
    assert_eq!(out["name"], "Wave11 Bot (3D)");
    assert_eq!(out["figures"].as_u64(), Some((demo_figures + 1) as u64));
    {
        let fig = &server.core.project.figures[bot_index];
        assert_eq!(fig.name, "Wave11 Bot (3D)");
        assert_eq!(fig.color, BOT_COLOR);
        assert_eq!(fig.def.nodes.len(), HUMANOID_NODES);
        assert!(!fig.hidden);
        // Held on all frames: every frame gained a pose
        // row for the new figure.
        for frame in &server.core.project.frames {
            assert_eq!(frame.poses.len(), demo_figures + 1);
        }
    }
    let (state, is_error) = call_payload(&mut server, "get_state", json!({}));
    assert!(!is_error);
    let state_figures = state["figures"].as_array().expect("state figures");
    assert_eq!(state_figures.len(), demo_figures + 1);
    assert_eq!(state_figures[bot_index]["name"], "Wave11 Bot (3D)");
    assert_eq!(state_figures[bot_index]["hidden"], false);

    // The second library figure, applied over the live
    // session: the orbited camera is kept (the load
    // contract), the figure and pose are replaced.
    let (out, is_error) = call_payload(
        &mut server,
        "fig3_load_from_library",
        json!({ "name": "Humanoid3" }),
    );
    assert!(!is_error);
    {
        let st = server.core.fig3.as_ref().expect("session survives load");
        assert_eq!(st.figure.name, "Humanoid3");
        assert_close(
            st.camera.yaw as f64,
            270.0,
            "load keeps the active session's camera"
        );
        assert!(st.pose_b.is_none());
    }
    let (out, is_error) = call_payload(&mut server, "fig3_bake", json!({}));
    assert!(!is_error);
    assert_eq!(out["name"], "Humanoid3 (3D)");
    assert_eq!(server.core.project.figures.len(), demo_figures + 2);

    // Delete the first baked figure; the second stays.
    let (out, is_error) = call_payload(
        &mut server,
        "delete_figure",
        json!({ "figure": bot_index }),
    );
    assert!(!is_error);
    assert_eq!(out["figures"].as_u64(), Some((demo_figures + 1) as u64));
    assert_eq!(server.core.project.figures.len(), demo_figures + 1);
    assert_eq!(
        server.core.project.figures[bot_index].name,
        "Humanoid3 (3D)",
        "the other baked figure shifted down, intact"
    );
    let (state, is_error) = call_payload(&mut server, "get_state", json!({}));
    assert!(!is_error);
    assert_eq!(
        state["figures"].as_array().unwrap().len(),
        demo_figures + 1,
        "get_state agrees after the delete"
    );
    // Deleting a figure index that does not exist refuses.
    let (out, is_error) = call_payload(
        &mut server,
        "delete_figure",
        json!({ "figure": 99 }),
    );
    assert!(is_error);
    assert!(
        err_text(&out).contains("out of range"),
        "delete refusal: {}",
        err_text(&out)
    );
    assert_eq!(server.core.project.figures.len(), demo_figures + 1);

    // ---- Phase 5: the project is still consistent, and
    // the library directory is left as it was found.
    assert_eq!(server.core.project.frames.len(), demo_frames);
    for frame in &server.core.project.frames {
        assert_eq!(
            frame.poses.len(),
            server.core.project.figures.len(),
            "every frame holds one pose per figure"
        );
    }
    assert!(
        server.core.selected_figure < server.core.project.figures.len(),
        "selection is in range"
    );
    let _ = std::fs::remove_file(&bot_path);
    if !humanoid_existed {
        let _ = std::fs::remove_file(&humanoid_path);
    }
    // Succeeds only if the directory is now empty (i.e. it
    // was created by this test's saves and nothing else
    // lives there) — exactly the leave-no-trace case.
    let _ = std::fs::remove_dir(&dir);
}
