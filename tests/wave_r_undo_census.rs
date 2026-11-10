//! Wave R (R7) — the MCP undo-contract FULL census.
//!
//! The standing contract (Wave 11, carried since; sampled by
//! P41 in tests/wave_p_undo_contract.rs for 14 tools): every
//! MUTATING MCP tool spends exactly ONE undo step (one
//! `EditorCore::snapshot` on its path); read-only tools spend
//! none; refusals spend none. This suite extends the sample to
//! the ENTIRE surface — all 392 tools, classified by handler
//! behaviour (static call-path trace + handler reads), not by
//! name:
//!
//!   MUTATING (211)   — a leg per tool below: fresh demo core,
//!                      invoke the handler through
//!                      `mcp::all_tools()`, assert the result
//!                      is Ok and the undo depth moves by
//!                      EXACTLY 1. Setups use other tools'
//!                      handlers, then the baseline is read —
//!                      only the leg call is measured.
//!   EXCEPTION (21)   — project-mutating tools that spend ZERO
//!                      steps BY DESIGN: the Wave 13 dirty-
//!                      census seam (`mark_dirty`, no snapshot)
//!                      with panel parity — the panel caller
//!                      writes through the same op/seam and
//!                      also spends no step. Each leg asserts
//!                      Ok + depth unchanged; the per-tool
//!                      evidence is in
//!                      ~/workspace/wave-r/findings/r7.md.
//!                      `figure_set_color` is the member P41
//!                      flagged (F1) and Q18 declared.
//!   NON-MUTATING (160) — read-only (53), session/runtime
//!                      state (47), library/file state (20),
//!                      project swaps (4, standing exclusion),
//!                      the undo/redo tools themselves (2) and
//!                      the live-only shell stubs (34), which
//!                      refuse on a bare core. The completeness
//!                      test pins the classification table
//!                      against `all_tools()` itself; most of
//!                      these classes also get behavioural
//!                      legs (depth must not move).
//!
//! Timing fixtures set fps 24.0 explicitly (fresh_core). The
//! undo stack caps at 60; every leg measures ONE call on a
//! fresh core, so the cap cannot distort a measurement.

use serde_json::{json, Value};
use std::collections::BTreeSet;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::all_tools;

// ---------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------

/// Serial guard: this suite's legs share the CWD-relative
/// stores (./library, ./library3d, ./projects, ./import),
/// and cargo runs a binary's tests on parallel threads —
/// left unserialised, one test's library delete/rename
/// lands inside another's save→list window (CI rounds
/// 42761122064/42761838001). Every test below holds this
/// guard for its whole body (the wave_p_close_guard
/// precedent). Poison-tolerant: one panicking test must
/// not cascade-fail the rest.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// A fresh demo core (1 humanoid figure, 5 frames) with the
/// project's fps set to 24.0 explicitly, per the board's
/// timing-fixture rule (subtitle import legs convert
/// timecodes at this rate).
fn fresh_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core
}

/// Invoke one tool's handler by name through the real
/// registry (`mcp::all_tools()`), the surface this census
/// audits.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

/// Invoke during a setup: the call must succeed, or the leg
/// that needed the setup is meaningless.
fn must(core: &mut EditorCore, name: &str, args: Value) {
    if let Err(e) = invoke(core, name, &args) {
        panic!("setup call {name} failed: {e}");
    }
}

/// One census leg: a setup that brings the demo project to
/// the state the tool acts on, then the measured call.
struct Leg {
    name: &'static str,
    group: &'static str,
    setup: fn(&mut EditorCore),
    args: fn(&EditorCore) -> Value,
}

fn run_legs(legs: &[Leg], group: &str, expect_delta: usize) {
    for leg in legs.iter().filter(|l| l.group == group) {
        let mut core = fresh_core();
        (leg.setup)(&mut core);
        let args = (leg.args)(&core);
        let before = core.undo_depth();
        let res = invoke(&mut core, leg.name, &args);
        assert!(
            res.is_ok(),
            "{}: expected Ok, got Err({:?})",
            leg.name,
            res.err()
        );
        assert_eq!(
            core.undo_depth(),
            before + expect_delta,
            "{}: undo depth must move by exactly {expect_delta}",
            leg.name
        );
    }
}

// ---------------------------------------------------------------------
// Fixture files (temp dir): WAV / PNG / SRT / VTT
// ---------------------------------------------------------------------

fn tmp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn write_wav(path: &str, samples: Vec<f32>) {
    let data = stykanimate_rs::audio_edit::WavData {
        sample_rate: 22050,
        channels: 1,
        samples,
    };
    std::fs::write(path, stykanimate_rs::audio_edit::write_wav(&data))
        .expect("write temp wav");
}

/// 0.5 s of loud square wave (peak 0.5).
fn tone_samples() -> Vec<f32> {
    (0..11025)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect()
}

/// 1.0 s of loud square wave.
fn tone_samples_1s() -> Vec<f32> {
    (0..22050)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect()
}

/// Silence / loud / silence / loud / silence — the pattern
/// the trim and split-on-silence legs need (0.3 s silences at
/// 22050 Hz = 6615 samples; loud runs of 0.5 s).
fn padded_samples() -> Vec<f32> {
    let sil = vec![0.0f32; 6615];
    let loud: Vec<f32> = (0..11025)
        .map(|i| if i % 2 == 0 { 0.6 } else { -0.6 })
        .collect();
    [sil.clone(), loud.clone(), sil.clone(), loud, sil].concat()
}

/// The canonical 1x1 RGBA PNG.
fn write_png(path: &str) {
    // A real encoder-made 1x1 PNG (the wave_r_e2e idiom).
    // The hand-rolled byte constant this replaced carried a
    // bad IDAT CRC: it passed while asset import checked
    // magic only, and Wave R's decode-checked funnel (R9)
    // rightly refuses it (CI 42761122064).
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        )
        .expect("encode census PNG fixture");
    std::fs::write(path, buf).expect("write temp png");
}

// ---------------------------------------------------------------------
// Shared setups
// ---------------------------------------------------------------------

fn s_none(_: &mut EditorCore) {}

fn s_add_figure(core: &mut EditorCore) {
    must(core, "add_figure", json!({ "kind": "humanoid" }));
}

fn s_fx(core: &mut EditorCore) {
    must(
        core,
        "add_fx_clip",
        json!({ "figure": 0, "node": 0, "kind": "burst", "start": 0, "duration": 3 }),
    );
}

fn s_particle(core: &mut EditorCore) {
    must(
        core,
        "particle_add_event",
        json!({ "frame": 0, "x": 100.0, "y": 100.0 }),
    );
}

fn s_selection_node(core: &mut EditorCore) {
    must(core, "set_selection", json!({ "figure": 0, "node": 1 }));
}

/// A root_x channel on figure 0 with keys at frames 0 and 4.
fn s_channel(core: &mut EditorCore) {
    must(
        core,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 0, "value": 10 }),
    );
    must(
        core,
        "graph_key",
        json!({ "figure": 0, "target": "root_x", "frame": 4, "value": 20 }),
    );
}

/// A root_x channel with keys at frames 0, 2 and 4.
fn s_channel3(core: &mut EditorCore) {
    for (f, v) in [(0, 10), (2, 20), (4, 30)] {
        must(
            core,
            "graph_key",
            json!({ "figure": 0, "target": "root_x", "frame": f, "value": v }),
        );
    }
}

/// A root_x channel with three collinear keys (frames 0,1,2,
/// values 0,5,10) — the middle key is removable by reduce.
fn s_channel_collinear(core: &mut EditorCore) {
    for (f, v) in [(0, 0), (1, 5), (2, 10)] {
        must(
            core,
            "graph_key",
            json!({ "figure": 0, "target": "root_x", "frame": f, "value": v }),
        );
    }
}

fn s_stroke(core: &mut EditorCore) {
    must(
        core,
        "draw_stroke",
        json!({ "frame": 0, "points": [[100, 100], [150, 130], [200, 100], [250, 120]] }),
    );
}

fn s_stroke_small(core: &mut EditorCore) {
    must(
        core,
        "draw_stroke",
        json!({ "frame": 0, "points": [[120, 120], [150, 140], [180, 120]] }),
    );
}

fn s_shape(core: &mut EditorCore) {
    must(
        core,
        "draw_shape",
        json!({ "kind": "rect", "x1": 100, "y1": 100, "x2": 220, "y2": 200, "frame": 0 }),
    );
}

fn s_seg_text(core: &mut EditorCore) {
    must(
        core,
        "set_segment_text",
        json!({ "figure": 0, "node": 1, "content": "Hello" }),
    );
}

fn s_text1(core: &mut EditorCore) {
    must(core, "add_text_instance", json!({ "content": "Title" }));
}

fn s_text2(core: &mut EditorCore) {
    s_text1(core);
    must(core, "add_text_instance", json!({ "content": "Second" }));
}

fn s_clip(core: &mut EditorCore) {
    must(core, "clip_create", json!({ "figure": 0 }));
}

fn s_two_clips(core: &mut EditorCore) {
    s_clip(core);
    s_add_figure(core);
    must(core, "clip_create", json!({ "figure": 1 }));
}

fn s_clip_session(core: &mut EditorCore) {
    s_clip(core);
    must(core, "clip_enter", json!({ "index": 0 }));
}

// Wave X (X3) state setups: a pool clip exists and the
// figure's link state is pinned either way — the assign /
// unlink handlers report already-linked / already-unlinked
// as success no-ops, so these setups end in the named state
// whether or not clip_create linked the figure itself.
fn s_state_key(core: &mut EditorCore) {
    s_clip(core);
    must(core, "figure_set_state", json!({ "figure": 0, "frame": 2, "clip": 0 }));
}

fn s_linked_clip(core: &mut EditorCore) {
    s_clip(core);
    must(core, "figure_assign_clip", json!({ "figure": 0, "clip": 0 }));
}

fn s_unlinked_clip(core: &mut EditorCore) {
    s_clip(core);
    must(core, "figure_unlink_clip", json!({ "figure": 0 }));
}

// Wave X (X5) sprite setups: a sprite plus a SECOND asset
// (set_asset needs somewhere real to point), and two
// sprites (reorder needs somewhere real to move).
fn s_sprite_two_assets(core: &mut EditorCore) {
    s_sprite(core);
    write_png(&tmp_path("wave_r_census_asset2.png"));
    must(
        core,
        "asset_import",
        json!({ "path": tmp_path("wave_r_census_asset2.png") }),
    );
}

fn s_two_sprites(core: &mut EditorCore) {
    s_sprite(core);
    must(core, "sprite_add", json!({ "asset": 0 }));
}

// Wave Y (Y7): set_rest_pose needs the current pose OFF
// the rest pose, or the op is an honest no-op — pose
// every frame from the rest angles with one node moved.
fn s_rest_displaced(core: &mut EditorCore) {
    let mut angles = core.project.figures[0].def.rest_angles();
    if angles.len() > 5 {
        angles[5] += 30.0;
    }
    must(core, "pose_apply_all_frames", json!({ "figure": 0, "angles": angles }));
}

// Wave Y (Y6): a layered-set folder fixture on disk —
// one part that attaches by stem (head) and one that
// degrades to a sprite (cape).
fn s_layered_folder(core: &mut EditorCore) {
    let _ = core;
    let dir = tmp_path("wave_r_census_layered");
    std::fs::create_dir_all(&dir).expect("layered fixture dir");
    write_png(&format!("{dir}/head.png"));
    write_png(&format!("{dir}/cape.png"));
}

fn s_camera_key(core: &mut EditorCore) {
    must(
        core,
        "set_camera",
        json!({ "frame": 0, "x": 480, "y": 360, "zoom": 1.2 }),
    );
}

fn s_cue(core: &mut EditorCore) {
    must(
        core,
        "subtitle_add",
        json!({ "start_frame": 0, "end_frame": 48, "text": "Hello census" }),
    );
}

fn s_fig3(core: &mut EditorCore) {
    must(core, "fig3_enter", json!({}));
}

fn s_png_file(core: &mut EditorCore) {
    let _ = core;
    write_png(&tmp_path("wave_r_census_asset.png"));
}

fn s_asset(core: &mut EditorCore) {
    s_png_file(core);
    must(
        core,
        "asset_import",
        json!({ "path": tmp_path("wave_r_census_asset.png") }),
    );
}

fn s_sprite(core: &mut EditorCore) {
    s_asset(core);
    must(core, "sprite_add", json!({ "asset": 0 }));
}

fn s_sprite_key3(core: &mut EditorCore) {
    s_sprite(core);
    must(
        core,
        "sprite_set_key",
        json!({ "sprite": 0, "frame": 3, "x": 520 }),
    );
}

fn s_asset_attach(core: &mut EditorCore) {
    s_asset(core);
    must(
        core,
        "asset_attach",
        json!({ "figure": 0, "node": 1, "asset": 0 }),
    );
}

fn s_clip_asset(core: &mut EditorCore) {
    s_clip(core);
    s_asset(core);
    must(
        core,
        "clip_asset_attach",
        json!({ "clip": 0, "node": 0, "asset": 0 }),
    );
}

fn s_wav_tone(core: &mut EditorCore) {
    let _ = core;
    write_wav(&tmp_path("wave_r_census_a.wav"), tone_samples());
}

fn s_audio1(core: &mut EditorCore) {
    s_wav_tone(core);
    must(
        core,
        "add_audio_clip",
        json!({ "path": tmp_path("wave_r_census_a.wav"), "start_frame": 0 }),
    );
}

fn s_audio1_long(core: &mut EditorCore) {
    write_wav(&tmp_path("wave_r_census_long.wav"), tone_samples_1s());
    must(
        core,
        "add_audio_clip",
        json!({ "path": tmp_path("wave_r_census_long.wav"), "start_frame": 0 }),
    );
}

fn s_audio_padded(core: &mut EditorCore) {
    write_wav(&tmp_path("wave_r_census_padded.wav"), padded_samples());
    must(
        core,
        "add_audio_clip",
        json!({ "path": tmp_path("wave_r_census_padded.wav"), "start_frame": 0 }),
    );
}

fn s_audio2(core: &mut EditorCore) {
    write_wav(&tmp_path("wave_r_census_a.wav"), tone_samples());
    write_wav(&tmp_path("wave_r_census_b.wav"), tone_samples());
    must(
        core,
        "add_audio_clip",
        json!({ "path": tmp_path("wave_r_census_a.wav"), "start_frame": 0 }),
    );
    must(
        core,
        "add_audio_clip",
        json!({ "path": tmp_path("wave_r_census_b.wav"), "start_frame": 12 }),
    );
}

fn s_fx_preset(core: &mut EditorCore) {
    s_fx(core);
    must(
        core,
        "fx_preset_save",
        json!({ "index": 0, "name": "wave-r-census" }),
    );
}

fn s_pose_save(core: &mut EditorCore) {
    must(core, "pose_library_save", json!({ "name": "wave-r-census-apply" }));
}

fn s_figlib_save(core: &mut EditorCore) {
    must(core, "figure_library_save", json!({}));
}

/// attach_image_to_node reads the first PNG in ./import —
/// plant one there (and take it away again in the leg's own
/// core Drop-free flow: the file is removed by the test
/// after the measured call; see the bespoke leg).
fn s_import_dir(core: &mut EditorCore) {
    let _ = core;
    std::fs::create_dir_all("import").expect("create ./import");
    write_png("import/wave_r_census.png");
    s_selection_node(core);
}

// ---------------------------------------------------------------------
// MUTATING legs — every one must spend exactly one undo step
// ---------------------------------------------------------------------

fn a_legs() -> Vec<Leg> {
    vec![
        // ---- frames / timeline (group "core") ----
        Leg { name: "add_frame", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "insert_inbetween", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "delete_frame", group: "core", setup: s_none, args: |_| json!({ "frame": 4 }) },
        Leg { name: "duplicate_frame", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "swap_frames", group: "core", setup: s_none, args: |_| json!({ "a": 0, "b": 1 }) },
        Leg { name: "paste_frame", group: "core", setup: |c| must(c, "copy_frame", json!({})), args: |_| json!({}) },
        Leg { name: "shift_frames", group: "core", setup: s_none, args: |_| json!({ "from": 0, "count": 2, "delta": 1 }) },
        Leg { name: "reverse_range", group: "core", setup: s_none, args: |_| json!({ "from": 0, "count": 3 }) },
        Leg { name: "delete_range", group: "core", setup: s_none, args: |_| json!({ "from": 1, "count": 1 }) },
        Leg { name: "paste_range", group: "core", setup: |c| must(c, "copy_range", json!({ "from": 0, "count": 2 })), args: |_| json!({ "at": 3 }) },
        Leg { name: "set_frame_easing", group: "core", setup: s_none, args: |_| json!({ "easing": "smooth", "frame": 0 }) },
        Leg { name: "insert_frames", group: "core", setup: s_none, args: |_| json!({ "count": 2 }) },
        Leg { name: "swap_frame_with_neighbour", group: "core", setup: s_none, args: |_| json!({ "direction": "next" }) },
        Leg { name: "delete_keyframe", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "frame": 1 }) },
        // ---- project settings setters (deliberate, one step on change) ----
        Leg { name: "set_project_fps", group: "core", setup: s_none, args: |_| json!({ "fps": 30 }) },
        Leg { name: "set_stage_size", group: "core", setup: s_none, args: |_| json!({ "width": 800, "height": 600 }) },
        Leg { name: "set_head_ring_scale", group: "core", setup: s_none, args: |_| json!({ "scale": 1.5 }) },
        Leg { name: "set_project_background", group: "core", setup: s_none, args: |_| json!({ "color": 0x101418 }) },
        Leg { name: "set_tween_playback", group: "core", setup: s_none, args: |_| json!({ "enabled": true }) },
        Leg { name: "set_camera_follow", group: "core", setup: s_none, args: |_| json!({ "figure": 0 }) },
        // ---- figures & poses ----
        Leg { name: "add_figure", group: "core", setup: s_none, args: |_| json!({ "kind": "humanoid" }) },
        Leg { name: "add_figure", group: "core", setup: s_none, args: |_| json!({ "kind": "quadruped" }) },
        Leg { name: "figure_duplicate", group: "core", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "delete_figure", group: "core", setup: s_add_figure, args: |_| json!({ "figure": 1 }) },
        Leg { name: "move_figure", group: "core", setup: s_add_figure, args: |_| json!({ "figure": 0, "direction": 1 }) },
        Leg { name: "figure_recolor", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "color": 0x336699 }) },
        Leg { name: "figure_scale", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "factor": 1.1 }) },
        Leg { name: "figure_centre", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "figure_ground", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "copy_pose_between", group: "core", setup: s_add_figure, args: |_| json!({ "from_figure": 0, "to_figure": 1 }) },
        Leg { name: "reset_pose", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "set_rest_pose", group: "core", setup: s_rest_displaced, args: |_| json!({ "figure": 0 }) },
        Leg { name: "pose_apply_all_frames", group: "core", setup: s_none, args: |c| {
            let n = c.project.figures[0].def.nodes.len();
            let mut angles = vec![0.0; n];
            if n > 5 { angles[5] = 12.0; }
            json!({ "figure": 0, "angles": angles })
        } },
        Leg { name: "set_pose", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "frame": 0, "root": [520, 460] }) },
        Leg { name: "mirror_pose", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "mirror_figure", group: "core", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "set_node", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "length": 57 }) },
        Leg { name: "set_node_cap", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "cap": "square" }) },
        Leg { name: "set_node_color2", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "color2": 0x334455 }) },
        Leg { name: "set_node_opacity", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "opacity": 0.5 }) },
        Leg { name: "add_node", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "parent": 0, "length": 33 }) },
        Leg { name: "delete_node", group: "core", setup: |c| must(c, "add_node", json!({ "figure": 0, "parent": 0, "length": 33 })), args: |c| json!({ "figure": 0, "node": c.project.figures[0].def.nodes.len() - 1 }) },
        Leg { name: "set_bend", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "frame": 0, "node": 1, "radians": 0.3 }) },
        Leg { name: "nudge_path_point", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "frame": 0, "dx": 5, "dy": -3 }) },
        Leg { name: "smooth_path_roots", group: "core", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "cycle_node_taper", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1 }) },
        Leg { name: "set_node_taper", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "taper": 0.5 }) },
        Leg { name: "set_node_style", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "color": 0x445566, "cap": "square", "opacity": 0.7 }) },
        Leg { name: "apply_style_preset", group: "core", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "preset": "Ink" }) },
        // ---- camera ----
        Leg { name: "set_camera", group: "core", setup: s_none, args: |_| json!({ "frame": 0, "x": 480, "y": 360, "zoom": 1.2 }) },
        Leg { name: "delete_camera_key", group: "core", setup: s_camera_key, args: |_| json!({ "frame": 0 }) },
        Leg { name: "set_camera_key_here", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "hold_camera", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "frame_all_figures", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "centre_on_selection", group: "core", setup: s_none, args: |_| json!({}) },
        Leg { name: "camera_zoom_by", group: "core", setup: s_none, args: |_| json!({ "factor": 1.25 }) },
        Leg { name: "rotate_camera", group: "core", setup: s_none, args: |_| json!({ "degrees": 15 }) },
        // ---- FX clips (group "fxpa") ----
        Leg { name: "add_fx_clip", group: "fxpa", setup: s_none, args: |_| json!({ "figure": 0, "node": 0, "kind": "burst", "start": 0, "duration": 3 }) },
        Leg { name: "fx_move", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "start": 2 }) },
        Leg { name: "fx_resize", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "duration": 5 }) },
        Leg { name: "fx_duplicate", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0 }) },
        Leg { name: "fx_delete", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0 }) },
        Leg { name: "fx_split_clip", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "at_frame": 1 }) },
        Leg { name: "fx_set_color", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "color": 0x00FF00 }) },
        Leg { name: "fx_set_kind", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "kind": "sparkle" }) },
        Leg { name: "fx_set_color2", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "color2": 0x0000FF }) },
        Leg { name: "fx_set_anchor", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "figure": 0, "node": 2 }) },
        Leg { name: "fx_set_hidden", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "hidden": true }) },
        Leg { name: "fx_set_seed", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "seed": 42 }) },
        Leg { name: "fx_set_angle", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "angle": 45 }) },
        Leg { name: "fx_set_length", group: "fxpa", setup: s_fx, args: |_| json!({ "index": 0, "length": 120 }) },
        Leg { name: "fx_preset_add", group: "fxpa", setup: s_fx_preset, args: |_| json!({ "name": "wave-r-census", "frame": 2 }) },
        // ---- particles ----
        Leg { name: "particle_add_event", group: "fxpa", setup: s_none, args: |_| json!({ "frame": 0, "x": 100.0, "y": 120.0 }) },
        Leg { name: "particle_add_anchored", group: "fxpa", setup: s_selection_node, args: |_| json!({}) },
        Leg { name: "particle_duplicate_event", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0 }) },
        Leg { name: "particle_delete_event", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0 }) },
        Leg { name: "particle_set_kind", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "kind": "sparkle" }) },
        Leg { name: "particle_set_color", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "color": 0xFF8800 }) },
        Leg { name: "particle_set_frame", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "frame": 2 }) },
        Leg { name: "particle_nudge_frame", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "delta": 1 }) },
        Leg { name: "particle_set_position", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "x": 200, "y": 180 }) },
        Leg { name: "particle_set_anchor", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "figure": 0, "node": 1 }) },
        Leg { name: "particle_set_seed", group: "fxpa", setup: s_particle, args: |_| json!({ "index": 0, "seed": 7 }) },
        Leg { name: "clear_particles", group: "fxpa", setup: s_particle, args: |_| json!({}) },
        // ---- audio ----
        Leg { name: "add_audio_clip", group: "fxpa", setup: s_wav_tone, args: |_| json!({ "path": tmp_path("wave_r_census_a.wav"), "start_frame": 0 }) },
        Leg { name: "audio_delete_clip", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_move_clip", group: "fxpa", setup: s_audio1, args: |_| json!({ "start_frame": 3 }) },
        Leg { name: "audio_set_gain", group: "fxpa", setup: s_audio1, args: |_| json!({ "gain": 0.5 }) },
        Leg { name: "audio_set_muted", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0, "muted": true }) },
        Leg { name: "audio_fade", group: "fxpa", setup: s_audio1, args: |_| json!({}) },
        Leg { name: "audio_reverse", group: "fxpa", setup: s_audio1, args: |_| json!({}) },
        Leg { name: "audio_echo", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_lowpass", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_trim_silence", group: "fxpa", setup: s_audio_padded, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_pitch", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_compress", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_split_silences", group: "fxpa", setup: s_audio_padded, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_normalise_all", group: "fxpa", setup: s_audio1, args: |_| json!({}) },
        Leg { name: "audio_clip_fade", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_clip_reverse", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_clip_set_gain", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0, "gain": 0.5 }) },
        Leg { name: "audio_clip_gain_step", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0, "delta": 0.25 }) },
        Leg { name: "audio_clip_move", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0, "start_frame": 3 }) },
        Leg { name: "audio_clip_duplicate", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_clip_normalise", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_clip_trim", group: "fxpa", setup: s_audio1_long, args: |_| json!({ "index": 0, "start_secs": 0.1, "end_secs": 0.4 }) },
        Leg { name: "audio_clip_toggle_muted", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_split", group: "fxpa", setup: s_audio1, args: |_| json!({ "index": 0, "at_frame": 2 }) },
        Leg { name: "audio_crossfade", group: "fxpa", setup: s_audio2, args: |_| json!({ "index": 0 }) },
        // ---- graph channels (group "graph") ----
        Leg { name: "graph_key", group: "graph", setup: s_none, args: |_| json!({ "figure": 0, "target": "root_x", "frame": 0, "value": 10 }) },
        Leg { name: "smooth_graph_tangents", group: "graph", setup: s_channel, args: |_| json!({}) },
        Leg { name: "bake_graph", group: "graph", setup: s_channel, args: |_| json!({ "from": 0, "to": 2 }) },
        Leg { name: "graph_tangent_preset", group: "graph", setup: s_channel, args: |_| json!({ "preset": "smooth" }) },
        Leg { name: "mirror_channel_time", group: "graph", setup: s_channel, args: |_| json!({ "figure": 0, "target": "root_x" }) },
        Leg { name: "scale_channel_values", group: "graph", setup: s_channel, args: |_| json!({ "figure": 0, "target": "root_x", "factor": 2 }) },
        Leg { name: "graph_paste_keys", group: "graph", setup: s_channel, args: |_| json!({ "figure": 0, "target": "root_x", "keys": [{ "frame": 0, "value": 5, "in_tan": 0, "out_tan": 0, "interp": "hermite" }], "at_frame": 2 }) },
        Leg { name: "duplicate_graph_channel", group: "graph", setup: |c| { s_channel(c); s_add_figure(c); }, args: |_| json!({ "channel": 0, "to_figure": 1 }) },
        Leg { name: "delete_graph_channel", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0 }) },
        Leg { name: "set_key_interp", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "key": 0, "interp": "step" }) },
        Leg { name: "graph_delete_key", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "frame": 4 }) },
        Leg { name: "graph_move_key", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "from_frame": 0, "to_frame": 2 }) },
        Leg { name: "graph_set_tangents", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "frame": 0, "in_tan": 1.5, "out_tan": 1.5 }) },
        Leg { name: "graph_shift_keys", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "delta": 1 }) },
        Leg { name: "graph_fill_keys", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "from": 1, "to": 3 }) },
        Leg { name: "graph_delete_keys_in_range", group: "graph", setup: s_channel3, args: |_| json!({ "channel": 0, "from": 2, "to": 2 }) },
        Leg { name: "graph_clear_channel_keys", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0 }) },
        Leg { name: "graph_reduce_keys", group: "graph", setup: s_channel_collinear, args: |_| json!({ "channel": 0 }) },
        Leg { name: "graph_mirror_channel", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0 }) },
        Leg { name: "graph_scale_channel", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "factor": 2 }) },
        Leg { name: "graph_tangent_preset_channel", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "preset": "linear" }) },
        Leg { name: "graph_key_all_channels", group: "graph", setup: s_channel, args: |_| json!({}) },
        Leg { name: "graph_key_angle", group: "graph", setup: s_selection_node, args: |_| json!({}) },
        Leg { name: "graph_clear_channels", group: "graph", setup: s_channel, args: |_| json!({}) },
        Leg { name: "graph_paste_keys_at", group: "graph", setup: s_channel, args: |_| json!({ "channel": 0, "keys": [{ "frame": 0, "value": 7, "in_tan": 0, "out_tan": 0, "interp": "hermite" }], "at_frame": 3 }) },
        Leg { name: "graph_set_interp_at_playhead", group: "graph", setup: |c| { s_selection_node(c); must(c, "graph_key", json!({ "figure": 0, "target": "angle:1", "frame": 0, "value": 10 })); }, args: |_| json!({ "interp": "step" }) },
        // ---- strokes ----
        Leg { name: "draw_stroke", group: "graph", setup: s_none, args: |_| json!({ "frame": 0, "points": [[100, 100], [160, 140], [220, 110]] }) },
        Leg { name: "draw_shape", group: "graph", setup: s_none, args: |_| json!({ "kind": "rect", "x1": 100, "y1": 100, "x2": 220, "y2": 200, "frame": 0 }) },
        Leg { name: "draw_polygon", group: "graph", setup: s_none, args: |_| json!({ "cx": 300, "cy": 200, "radius": 60, "frame": 0 }) },
        Leg { name: "draw_star", group: "graph", setup: s_none, args: |_| json!({ "cx": 300, "cy": 200, "radius": 60, "frame": 0 }) },
        Leg { name: "draw_arrow", group: "graph", setup: s_none, args: |_| json!({ "x1": 100, "y1": 300, "x2": 260, "y2": 300, "frame": 0 }) },
        Leg { name: "draw_spray", group: "graph", setup: s_none, args: |_| json!({ "frame": 0, "points": [[150, 150], [180, 160]] }) },
        Leg { name: "smooth_strokes", group: "graph", setup: s_stroke, args: |_| json!({ "frame": 0 }) },
        Leg { name: "erase_strokes", group: "graph", setup: s_stroke, args: |_| json!({ "frame": 0, "x": 150, "y": 130, "radius": 30 }) },
        Leg { name: "simplify_strokes", group: "graph", setup: s_stroke, args: |_| json!({ "frame": 0, "epsilon": 50 }) },
        Leg { name: "delete_stroke", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0 }) },
        Leg { name: "apply_nib", group: "graph", setup: s_stroke, args: |_| json!({ "frame": 0, "nib": "square" }) },
        Leg { name: "apply_nib_stroke", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0, "nib": "square" }) },
        Leg { name: "smooth_stroke", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0 }) },
        Leg { name: "simplify_stroke", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0, "epsilon": 50 }) },
        Leg { name: "set_stroke_style", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0, "color": 0xFF0000, "width": 6 }) },
        Leg { name: "set_stroke_spray", group: "graph", setup: s_stroke, args: |_| json!({ "index": 0, "radius": 20, "opacity": 0.8 }) },
        Leg { name: "fill_stroke", group: "graph", setup: s_shape, args: |_| json!({}) },
        Leg { name: "lasso_erase", group: "graph", setup: s_stroke_small, args: |_| json!({ "points": [[80, 80], [220, 80], [220, 180], [80, 180]] }) },
        Leg { name: "mirror_strokes", group: "graph", setup: s_stroke, args: |_| json!({}) },
        Leg { name: "mirror_all_strokes", group: "graph", setup: s_stroke, args: |_| json!({}) },
        Leg { name: "clear_frame_strokes", group: "graph", setup: s_stroke, args: |_| json!({}) },
        Leg { name: "clear_strokes_on_frame", group: "graph", setup: s_stroke, args: |_| json!({ "frame": 0 }) },
        Leg { name: "delete_all_strokes", group: "graph", setup: s_stroke, args: |_| json!({}) },
        Leg { name: "recolor_strokes", group: "graph", setup: s_stroke, args: |_| json!({ "color": 0x00FF00, "scope": "all" }) },
        // ---- texts, clips, assets, sprites (group "misc") ----
        Leg { name: "set_segment_text", group: "misc", setup: s_none, args: |_| json!({ "figure": 0, "node": 1, "content": "Hello" }) },
        Leg { name: "set_segment_text_size", group: "misc", setup: s_seg_text, args: |_| json!({ "figure": 0, "node": 1, "size": 44 }) },
        Leg { name: "clear_segment_text", group: "misc", setup: s_seg_text, args: |_| json!({ "figure": 0, "node": 1 }) },
        Leg { name: "add_text_instance", group: "misc", setup: s_none, args: |_| json!({ "content": "Title" }) },
        Leg { name: "update_text_instance", group: "misc", setup: s_text1, args: |_| json!({ "index": 0, "content": "New title" }) },
        Leg { name: "delete_text_instance", group: "misc", setup: s_text1, args: |_| json!({ "index": 0 }) },
        Leg { name: "duplicate_text_instance", group: "misc", setup: s_text1, args: |_| json!({ "index": 0 }) },
        Leg { name: "nudge_text_instance", group: "misc", setup: s_text1, args: |_| json!({ "index": 0, "dx": 5, "dy": 5 }) },
        Leg { name: "set_text_instance_size", group: "misc", setup: s_text1, args: |_| json!({ "index": 0, "size": 44 }) },
        Leg { name: "rotate_text_instance", group: "misc", setup: s_text1, args: |_| json!({ "index": 0, "degrees": 15 }) },
        Leg { name: "set_text_instance_color", group: "misc", setup: s_text1, args: |_| json!({ "index": 0, "color": 0xFFD94D }) },
        Leg { name: "move_text_instance", group: "misc", setup: s_text2, args: |_| json!({ "index": 0, "to_front": true }) },
        Leg { name: "clip_create", group: "misc", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "set_clip_loop", group: "misc", setup: s_clip, args: |_| json!({ "index": 0, "mode": "once" }) },
        Leg { name: "clip_delete", group: "misc", setup: s_clip, args: |_| json!({ "index": 0 }) },
        Leg { name: "clip_rename", group: "misc", setup: s_clip, args: |_| json!({ "index": 0, "name": "Renamed" }) },
        Leg { name: "clip_duplicate", group: "misc", setup: s_clip, args: |_| json!({ "index": 0 }) },
        Leg { name: "clip_move", group: "misc", setup: s_two_clips, args: |_| json!({ "index": 0, "direction": "down" }) },
        Leg { name: "clip_insert_figure", group: "misc", setup: s_clip, args: |_| json!({ "index": 0 }) },
        Leg { name: "clip_add_frame", group: "misc", setup: s_clip_session, args: |_| json!({}) },
        Leg { name: "clip_delete_frame", group: "misc", setup: s_clip_session, args: |_| json!({}) },
        // Wave X (X3): animation-state tools — the ranged
        // create, the base-link pair and the state-key pair.
        Leg { name: "clip_create_range", group: "misc", setup: s_none, args: |_| json!({ "figure": 0, "start": 0, "end": 2 }) },
        Leg { name: "figure_assign_clip", group: "misc", setup: s_unlinked_clip, args: |_| json!({ "figure": 0, "clip": 0 }) },
        Leg { name: "figure_unlink_clip", group: "misc", setup: s_linked_clip, args: |_| json!({ "figure": 0 }) },
        Leg { name: "figure_set_state", group: "misc", setup: s_clip, args: |_| json!({ "figure": 0, "frame": 2, "clip": 0 }) },
        Leg { name: "figure_clear_state", group: "misc", setup: s_state_key, args: |_| json!({ "figure": 0, "frame": 2 }) },
        Leg { name: "asset_import", group: "misc", setup: s_png_file, args: |_| json!({ "path": tmp_path("wave_r_census_asset.png") }) },
        Leg { name: "asset_rename", group: "misc", setup: s_asset, args: |_| json!({ "index": 0, "name": "hero" }) },
        Leg { name: "asset_remove", group: "misc", setup: s_asset, args: |_| json!({ "index": 0 }) },
        Leg { name: "asset_attach", group: "misc", setup: s_asset, args: |_| json!({ "figure": 0, "node": 1, "asset": 0 }) },
        Leg { name: "clip_asset_attach", group: "misc", setup: |c| { s_clip(c); s_asset(c); }, args: |_| json!({ "clip": 0, "node": 0, "asset": 0 }) },
        Leg { name: "clip_clear_node_media", group: "misc", setup: s_clip_asset, args: |_| json!({ "clip": 0, "node": 0 }) },
        Leg { name: "clear_node_media", group: "misc", setup: |c| { s_asset_attach(c); s_selection_node(c); }, args: |_| json!({}) },
        Leg { name: "sprite_add", group: "misc", setup: s_asset, args: |_| json!({ "asset": 0 }) },
        Leg { name: "sprite_remove", group: "misc", setup: s_sprite, args: |_| json!({ "sprite": 0 }) },
        Leg { name: "sprite_set_key", group: "misc", setup: s_sprite, args: |_| json!({ "sprite": 0, "frame": 2, "x": 500 }) },
        Leg { name: "sprite_delete_key", group: "misc", setup: s_sprite, args: |_| json!({ "sprite": 0, "frame": 0 }) },
        Leg { name: "sprite_move_key", group: "misc", setup: s_sprite, args: |_| json!({ "sprite": 0, "from_frame": 0, "to_frame": 2 }) },
        Leg { name: "sprite_delete_key_at", group: "misc", setup: s_sprite_key3, args: |_| json!({ "sprite": 0, "frame": 3 }) },
        // Wave X (X5): the sprite small-win pair.
        Leg { name: "sprite_set_asset", group: "misc", setup: s_sprite_two_assets, args: |_| json!({ "sprite": 0, "asset": 1 }) },
        Leg { name: "sprite_reorder", group: "misc", setup: s_two_sprites, args: |_| json!({ "sprite": 0, "to": 1 }) },
        Leg { name: "import_layered_set", group: "misc", setup: s_layered_folder, args: |_| json!({ "path": tmp_path("wave_r_census_layered"), "figure": 0 }) },
        // ---- fig3 bakes, libraries, subtitles ----
        Leg { name: "fig3_bake", group: "misc", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_bake_animation", group: "misc", setup: |c| { s_fig3(c); must(c, "fig3_rotate", json!({ "axis": 0, "degrees": 25 })); must(c, "fig3_mark_b", json!({})); must(c, "fig3_rotate", json!({ "axis": 0, "degrees": -10 })); }, args: |_| json!({ "frames": 3 }) },
        // The library stores the figure's name inside its
        // file (Wave V, V1): the saved "Styk" figure is
        // listed under its exact name, "Styk".
        Leg { name: "figure_library_insert", group: "misc", setup: s_figlib_save, args: |_| json!({ "name": "Styk" }) },
        Leg { name: "pose_library_apply", group: "misc", setup: s_pose_save, args: |_| json!({ "name": "wave-r-census-apply", "figure": 0, "frame": 1 }) },
        Leg { name: "subtitle_add", group: "misc", setup: s_none, args: |_| json!({ "start_frame": 0, "end_frame": 48, "text": "Hello census" }) },
        Leg { name: "subtitle_update", group: "misc", setup: s_cue, args: |_| json!({ "index": 0, "start_frame": 0, "end_frame": 24, "text": "Updated" }) },
        Leg { name: "subtitle_delete", group: "misc", setup: s_cue, args: |_| json!({ "index": 0 }) },
        Leg { name: "subtitle_set_style", group: "misc", setup: s_none, args: |_| json!({ "position": "top", "font_scale": 1.5, "text_color": 0xFFFFFF, "box_color": 0x000000 }) },
        Leg { name: "subtitle_import_srt", group: "misc", setup: |_| std::fs::write(tmp_path("wave_r_census.srt"), "1\n00:00:00,000 --> 00:00:02,000\nHello census\n").expect("write srt"), args: |_| json!({ "path": tmp_path("wave_r_census.srt") }) },
        Leg { name: "subtitle_import_vtt", group: "misc", setup: |_| std::fs::write(tmp_path("wave_r_census.vtt"), "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello census\n").expect("write vtt"), args: |_| json!({ "path": tmp_path("wave_r_census.vtt") }) },
    ]
}

#[test]
fn census_mutating_core_spend_one_step() {
    let _serial = serial_guard();
    run_legs(&a_legs(), "core", 1);
}

#[test]
fn census_mutating_fx_particles_audio_spend_one_step() {
    let _serial = serial_guard();
    run_legs(&a_legs(), "fxpa", 1);
}

#[test]
fn census_mutating_graph_strokes_spend_one_step() {
    let _serial = serial_guard();
    run_legs(&a_legs(), "graph", 1);
}

#[test]
fn census_mutating_misc_spend_one_step() {
    let _serial = serial_guard();
    run_legs(&a_legs(), "misc", 1);
}

/// attach_image_to_node is selection-scoped and reads the
/// first PNG in ./import (its panel file-picker shape) — a
/// bespoke leg so the planted file is removed afterwards.
#[test]
fn census_attach_image_to_node_spends_one_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    s_import_dir(&mut core);
    let before = core.undo_depth();
    let res = invoke(&mut core, "attach_image_to_node", &json!({}));
    let _ = std::fs::remove_file("import/wave_r_census.png");
    assert!(res.is_ok(), "attach_image_to_node: {:?}", res.err());
    assert_eq!(core.undo_depth(), before + 1, "attach_image_to_node");
}

// ---------------------------------------------------------------------
// EXCEPTION legs — project writes that spend ZERO steps by
// design (the Wave 13 mark_dirty seam, panel parity). Each
// leg asserts Ok + depth unchanged; several also assert the
// write really landed and dirty moved, so a leg cannot pass
// vacuously. Evidence per tool: findings/r7.md.
// ---------------------------------------------------------------------

fn exception_legs() -> Vec<Leg> {
    vec![
        Leg { name: "figure_set_color", group: "exc", setup: s_none, args: |_| json!({ "figure": 0, "color": 0x224466 }) },
        Leg { name: "figure_rename", group: "exc", setup: s_none, args: |_| json!({ "figure": 0, "name": "Hero" }) },
        Leg { name: "set_project_easing", group: "exc", setup: s_none, args: |_| json!({ "easing": "smooth" }) },
        Leg { name: "set_frame_duration", group: "exc", setup: s_none, args: |_| json!({ "frame": 0, "duration": 2.0 }) },
        Leg { name: "set_frame_durations", group: "exc", setup: s_none, args: |_| json!({ "durations": [2.0, 1.0] }) },
        Leg { name: "adjust_frame_duration", group: "exc", setup: s_none, args: |_| json!({ "factor": 2.0 }) },
        Leg { name: "set_weather", group: "exc", setup: s_none, args: |_| json!({ "kind": "rain" }) },
        Leg { name: "set_weather_kind", group: "exc", setup: s_none, args: |_| json!({ "kind": "snow" }) },
        Leg { name: "cycle_weather", group: "exc", setup: s_none, args: |_| json!({}) },
        Leg { name: "set_weather_amount", group: "exc", setup: |c| must(c, "set_weather_kind", json!({ "kind": "rain" })), args: |_| json!({ "amount": 120 }) },
        Leg { name: "set_weather_wind", group: "exc", setup: |c| must(c, "set_weather_kind", json!({ "kind": "rain" })), args: |_| json!({ "wind": 40 }) },
        Leg { name: "layers", group: "exc", setup: s_none, args: |_| json!({ "figure": 0, "hidden": true }) },
        Leg { name: "layers_set_all_visible", group: "exc", setup: s_none, args: |_| json!({ "visible": false }) },
        Leg { name: "layers_invert_visibility", group: "exc", setup: s_none, args: |_| json!({}) },
        Leg { name: "figure_cycle_opacity", group: "exc", setup: s_none, args: |_| json!({}) },
        Leg { name: "figure_step_opacity", group: "exc", setup: s_none, args: |_| json!({ "delta": -0.2 }) },
        Leg { name: "figure_toggle_hidden", group: "exc", setup: s_none, args: |_| json!({}) },
        Leg { name: "figure_toggle_lock", group: "exc", setup: s_none, args: |_| json!({}) },
        Leg { name: "clear_camera_key", group: "exc", setup: s_camera_key, args: |_| json!({}) },
        Leg { name: "clip_set_angle", group: "exc", setup: s_clip_session, args: |_| json!({ "node": 1, "degrees": 33 }) },
        Leg { name: "clip_set_root", group: "exc", setup: s_clip_session, args: |_| json!({ "root": [400, 300] }) },
    ]
}

#[test]
fn census_exceptions_spend_no_step_but_really_write() {
    let _serial = serial_guard();
    run_legs(&exception_legs(), "exc", 0);

    // Non-vacuity witnesses for three representatives: the
    // colour write lands in the project, the duration write
    // lands in the frame, the clip tweak lands in the clip.
    let mut core = fresh_core();
    must(&mut core, "figure_set_color", json!({ "figure": 0, "color": 0x224466 }));
    assert_eq!(core.project.figures[0].color, 0x224466);
    must(&mut core, "set_frame_duration", json!({ "frame": 0, "duration": 2.0 }));
    assert_eq!(core.project.frames[0].duration, 2.0);
    let mut core = fresh_core();
    s_clip_session(&mut core);
    // The setup itself spends a step (clip_create is a
    // project mutation, counted in its own group above);
    // the exception's contract is that the measured call
    // adds none — measured relatively, like the fig3 and
    // figlib witnesses below.
    let before = core.undo_depth();
    must(&mut core, "clip_set_angle", json!({ "node": 1, "degrees": 33 }));
    assert_eq!(core.project.clips[0].frames[0].angles[1], 33.0);
    assert_eq!(core.undo_depth(), before, "exceptions never touch the undo stack");
}

// ---------------------------------------------------------------------
// SESSION-state legs — these mutate only editor/session
// state (playhead, selection, clipboards, loop range, brush
// selection, fig3 session, clip-session cursor): never the
// undo stack. All assert Ok + depth unchanged.
// ---------------------------------------------------------------------

fn session_legs() -> Vec<Leg> {
    vec![
        Leg { name: "set_frame", group: "ses", setup: s_none, args: |_| json!({ "frame": 2 }) },
        Leg { name: "copy_frame", group: "ses", setup: s_none, args: |_| json!({}) },
        Leg { name: "copy_range", group: "ses", setup: s_none, args: |_| json!({ "from": 0, "count": 2 }) },
        Leg { name: "set_loop_range", group: "ses", setup: s_none, args: |_| json!({ "start": 0, "end": 2 }) },
        Leg { name: "clear_loop_range", group: "ses", setup: |c| must(c, "set_loop_range", json!({ "start": 0, "end": 2 })), args: |_| json!({}) },
        Leg { name: "set_playback", group: "ses", setup: s_none, args: |_| json!({ "action": "play" }) },
        Leg { name: "set_selection", group: "ses", setup: s_none, args: |_| json!({ "figure": 0, "node": 1 }) },
        Leg { name: "goto_next_keyframe", group: "ses", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "goto_prev_keyframe", group: "ses", setup: |c| must(c, "set_frame", json!({ "frame": 2 })), args: |_| json!({ "figure": 0 }) },
        Leg { name: "shell_play", group: "ses", setup: s_none, args: |_| json!({}) },
        Leg { name: "shell_pause", group: "ses", setup: s_none, args: |_| json!({}) },
        Leg { name: "shell_set_playhead", group: "ses", setup: s_none, args: |_| json!({ "frame": 2 }) },
        Leg { name: "set_brush", group: "ses", setup: s_none, args: |_| json!({ "index": 0 }) },
        Leg { name: "cycle_brush", group: "ses", setup: s_none, args: |_| json!({}) },
        Leg { name: "set_draw_tool", group: "ses", setup: s_none, args: |_| json!({ "tool": "eraser" }) },
        Leg { name: "set_draw_mode", group: "ses", setup: s_none, args: |_| json!({ "enabled": true }) },
        Leg { name: "set_brush_nib", group: "ses", setup: s_none, args: |_| json!({ "nib": "square" }) },
        Leg { name: "step_brush_param", group: "ses", setup: s_none, args: |_| json!({ "param": "size", "direction": 1 }) },
        Leg { name: "set_onion_skin", group: "ses", setup: s_none, args: |_| json!({ "enabled": false }) },
        Leg { name: "figure_toggle_solo", group: "ses", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "clip_enter", group: "ses", setup: s_clip, args: |_| json!({ "index": 0 }) },
        Leg { name: "clip_exit", group: "ses", setup: s_clip_session, args: |_| json!({}) },
        Leg { name: "clip_goto", group: "ses", setup: s_clip_session, args: |_| json!({ "frame": 1 }) },
        Leg { name: "fig3_enter", group: "ses", setup: s_none, args: |_| json!({}) },
        Leg { name: "fig3_select", group: "ses", setup: s_fig3, args: |_| json!({ "joint": 1 }) },
        Leg { name: "fig3_select_next", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_rotate", group: "ses", setup: s_fig3, args: |_| json!({ "axis": 0, "degrees": 20 }) },
        Leg { name: "fig3_orbit", group: "ses", setup: s_fig3, args: |_| json!({ "yaw": 10, "pitch": 5 }) },
        Leg { name: "fig3_mark_b", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_camera_preset", group: "ses", setup: s_fig3, args: |_| json!({ "name": "front" }) },
        Leg { name: "fig3_frame_camera", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_frame_preview", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_camera_zoom", group: "ses", setup: s_fig3, args: |_| json!({ "dir": 1 }) },
        Leg { name: "fig3_ik", group: "ses", setup: s_fig3, args: |_| json!({ "limb": "arm_l" }) },
        Leg { name: "fig3_pose_mirror", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_set_camera", group: "ses", setup: s_fig3, args: |_| json!({ "yaw": 30 }) },
        Leg { name: "fig3_set_rotation", group: "ses", setup: s_fig3, args: |_| json!({ "x": 15 }) },
        Leg { name: "fig3_set_root", group: "ses", setup: s_fig3, args: |_| json!({ "position": [0, 100, 0] }) },
        Leg { name: "fig3_rename", group: "ses", setup: s_fig3, args: |_| json!({ "name": "Hero3D" }) },
        Leg { name: "fig3_exit", group: "ses", setup: s_fig3, args: |_| json!({}) },
        Leg { name: "fig3_pose_apply", group: "ses", setup: |c| { s_fig3(c); must(c, "fig3_pose_save", json!({ "name": "wave-r-census-3d" })); }, args: |_| json!({ "name": "wave-r-census-3d" }) },
    ]
}

#[test]
fn census_session_state_spends_no_step() {
    let _serial = serial_guard();
    run_legs(&session_legs(), "ses", 0);
}

/// The fig3 library chain: save the session figure to the
/// 3D library (library/file state), then load and cycle —
/// session state throughout, never an undo step.
#[test]
fn census_fig3_library_chain_spends_no_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    s_fig3(&mut core);
    let before = core.undo_depth();
    must(&mut core, "fig3_save_to_library", json!({}));
    let listed = invoke(&mut core, "fig3_library_list", &json!({})).expect("library list");
    let name = listed["figures"][0]
        .as_str()
        .expect("the saved figure is listed by name")
        .to_string();
    must(&mut core, "fig3_load_from_library", json!({ "name": name }));
    must(&mut core, "fig3_cycle_library", json!({}));
    assert_eq!(core.undo_depth(), before, "fig3 library chain");
}

/// The figure-library selection cycle is session state.
#[test]
fn census_figure_library_cycle_spends_no_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    s_figlib_save(&mut core);
    let before = core.undo_depth();
    must(&mut core, "figure_library_cycle", json!({}));
    assert_eq!(core.undo_depth(), before, "figure_library_cycle");
}

// ---------------------------------------------------------------------
// READ-only legs — a behavioural sweep: every read below
// must succeed on the demo state it is given and must not
// move the undo depth.
// ---------------------------------------------------------------------

fn read_legs() -> Vec<Leg> {
    vec![
        Leg { name: "server_info", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_state", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_project_stats", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "list_figures", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_pose", group: "read", setup: s_none, args: |_| json!({ "figure": 0, "frame": 0 }) },
        Leg { name: "fx_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "clip_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "figure_list_states", group: "read", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "clip_compatible", group: "read", setup: s_clip, args: |_| json!({ "figure": 0, "clip": 0 }) },
        Leg { name: "get_keyframes", group: "read", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "list_frames", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "particle_list_events", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "list_strokes", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_drawing_stats", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "list_brushes", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_node_style", group: "read", setup: s_none, args: |_| json!({ "figure": 0, "node": 1 }) },
        Leg { name: "list_texts", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_stroke_style", group: "read", setup: s_stroke, args: |_| json!({ "index": 0 }) },
        Leg { name: "audio_list_clips", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "audio_clip_info", group: "read", setup: s_audio1, args: |_| json!({ "index": 0 }) },
        Leg { name: "graph_list_channels", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "graph_get_channel", group: "read", setup: s_channel, args: |_| json!({ "channel": 0 }) },
        Leg { name: "graph_key_interp", group: "read", setup: s_channel, args: |_| json!({ "channel": 0, "frame": 0 }) },
        Leg { name: "graph_count_keys_in_range", group: "read", setup: s_channel, args: |_| json!({ "channel": 0, "from": 0, "to": 4 }) },
        Leg { name: "get_figure", group: "read", setup: s_none, args: |_| json!({ "figure": 0 }) },
        Leg { name: "get_node", group: "read", setup: s_none, args: |_| json!({ "figure": 0, "node": 1 }) },
        Leg { name: "figure_library_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "assets_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "asset_info", group: "read", setup: s_asset, args: |_| json!({ "index": 0 }) },
        Leg { name: "sprite_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "sprite_sample", group: "read", setup: s_sprite, args: |_| json!({ "sprite": 0, "frame": 0 }) },
        Leg { name: "get_onion_skin", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_editor_settings", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_clipboard_state", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "fig3_get", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "fig3_pose_library_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "list_projects", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "project_summary", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "subtitle_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_camera", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_editor_status", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "get_selection", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "shell_get_transport", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "fig3_library_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "clip_get_pose", group: "read", setup: s_clip_session, args: |_| json!({}) },
        Leg { name: "clip_get_node_media", group: "read", setup: s_clip, args: |_| json!({ "clip": 0, "node": 0 }) },
        Leg { name: "get_brush", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "pose_library_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "fx_preset_list", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "render_frame_base64", group: "read", setup: s_none, args: |_| json!({}) },
        Leg { name: "subtitle_export_srt", group: "read", setup: s_cue, args: |_| json!({ "path": tmp_path("wave_r_census_out.srt") }) },
        Leg { name: "subtitle_export_vtt", group: "read", setup: s_cue, args: |_| json!({ "path": tmp_path("wave_r_census_out.vtt") }) },
    ]
}

#[test]
fn census_read_only_spends_no_step() {
    let _serial = serial_guard();
    run_legs(&read_legs(), "read", 0);
}

// ---------------------------------------------------------------------
// LIBRARY / FILE legs — writes land in library stores or
// files on disk, never in the session project's undo
// history. All assert Ok + depth unchanged.
// ---------------------------------------------------------------------

fn libfile_legs() -> Vec<Leg> {
    vec![
        Leg { name: "save_project", group: "lib", setup: s_none, args: |_| json!({ "path": tmp_path("wave_r_census_save.styk.json") }) },
        Leg { name: "export_file", group: "lib", setup: s_none, args: |_| json!({ "path": tmp_path("wave_r_census_export.piv"), "format": "piv5" }) },
        Leg { name: "render", group: "lib", setup: s_none, args: |_| json!({ "path": tmp_path("wave_r_census_render.png"), "format": "png", "frame": 0 }) },
        Leg { name: "render_contact_sheet", group: "lib", setup: s_none, args: |_| json!({ "path": tmp_path("wave_r_census_sheet.png") }) },
        Leg { name: "export_bundle", group: "lib", setup: s_none, args: |_| json!({ "path": tmp_path("wave_r_census_bundle"), "kind": "images" }) },
        Leg { name: "save_named_project", group: "lib", setup: |_| std::fs::create_dir_all("projects").expect("create ./projects"), args: |_| json!({}) },
        Leg { name: "save_project_copy", group: "lib", setup: |_| std::fs::create_dir_all("projects").expect("create ./projects"), args: |_| json!({}) },
        Leg { name: "figure_library_save", group: "lib", setup: s_none, args: |_| json!({}) },
        Leg { name: "fx_preset_save", group: "lib", setup: s_fx, args: |_| json!({ "index": 0, "name": "wave-r-census-fx" }) },
        Leg { name: "fx_preset_delete", group: "lib", setup: |c| { s_fx(c); must(c, "fx_preset_save", json!({ "index": 0, "name": "wave-r-census-del" })); }, args: |_| json!({ "name": "wave-r-census-del" }) },
        Leg { name: "fig3_pose_save", group: "lib", setup: s_fig3, args: |_| json!({ "name": "wave-r-census-3d-lib" }) },
        Leg { name: "pose_library_save", group: "lib", setup: s_none, args: |_| json!({ "name": "wave-r-census-lib" }) },
    ]
}

#[test]
fn census_library_file_spends_no_step() {
    let _serial = serial_guard();
    run_legs(&libfile_legs(), "lib", 0);
}

/// The pose-library rename/delete pair are file operations
/// on the entry the save leg planted.
#[test]
fn census_pose_library_file_ops_spend_no_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    must(&mut core, "pose_library_save", json!({ "name": "wave-r-census" }));
    let before = core.undo_depth();
    must(
        &mut core,
        "pose_library_rename",
        json!({ "name": "wave-r-census", "new_name": "wave-r-census-renamed" }),
    );
    must(
        &mut core,
        "pose_library_delete",
        json!({ "name": "wave-r-census-renamed" }),
    );
    assert_eq!(core.undo_depth(), before, "pose library file ops");
}

/// figure_library_rename renames the entry's backing file
/// (P20 pinned its zero-undo as designed).
#[test]
fn census_figure_library_rename_spends_no_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    s_figlib_save(&mut core);
    let listed = invoke(&mut core, "figure_library_list", &json!({})).expect("library list");
    let idx = listed["entries"]
        .as_array()
        .and_then(|es| {
            es.iter()
                .position(|e| e["name"] == json!("Styk"))
                .map(|p| p as i64)
        })
        .expect("the saved Styk entry is listed (under its exact stored name)");
    let before = core.undo_depth();
    must(
        &mut core,
        "figure_library_rename",
        json!({ "index": idx, "new_name": "StykRenamed" }),
    );
    assert_eq!(core.undo_depth(), before, "figure_library_rename");
}

/// delete_project_file removes a saved file from
/// ./projects — a file operation, not an undo step.
#[test]
fn census_delete_project_file_spends_no_step() {
    let _serial = serial_guard();
    std::fs::create_dir_all("projects").expect("create ./projects");
    let mut core = fresh_core();
    must(
        &mut core,
        "save_project",
        json!({ "path": "projects/wave_r_census_del.styk.json" }),
    );
    let before = core.undo_depth();
    must(
        &mut core,
        "delete_project_file",
        json!({ "path": "projects/wave_r_census_del.styk.json" }),
    );
    assert_eq!(core.undo_depth(), before, "delete_project_file");
}

// ---------------------------------------------------------------------
// REFUSALS on a bare core — the live-only shell stubs always
// refuse here (on the live endpoint they are routed to the
// shell queue and never run against the core), and the two
// saved-file tools refuse when no saved file exists. All
// must spend nothing.
// ---------------------------------------------------------------------

#[test]
fn census_live_stubs_and_file_refusals_spend_no_step() {
    let _serial = serial_guard();
    for (name, class) in NON_MUTATING {
        if *class != "live-only" {
            continue;
        }
        let mut core = fresh_core();
        let res = invoke(&mut core, name, &json!({}));
        let err = res.expect_err(&format!("{name} must refuse on a bare core"));
        assert!(
            err.contains("needs the live editor"),
            "{name}: canonical live refusal, got: {err}"
        );
        assert_eq!(core.undo_depth(), 0, "{name} refusal spends nothing");
    }
    for (name, args) in [
        ("project_rename", json!({ "new_name": "nope" })),
        ("duplicate_project", json!({})),
    ] {
        let mut core = fresh_core();
        // Both tools resolve the current file through the
        // core's cached selector list; clear it so the
        // refusal is deterministic even if a parallel test
        // has planted files in ./projects.
        core.project_files.clear();
        let res = invoke(&mut core, name, &args);
        assert!(res.is_err(), "{name} refuses with no saved file");
        assert_eq!(core.undo_depth(), 0, "{name} refusal spends nothing");
    }
}

// ---------------------------------------------------------------------
// HISTORY tools — undo/redo are the mechanism itself: they
// move the depth by restoring, never by spending a step.
// ---------------------------------------------------------------------

#[test]
fn census_history_tools_restore_not_spend() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    must(&mut core, "add_figure", json!({ "kind": "humanoid" }));
    assert_eq!(core.undo_depth(), 1);
    assert_eq!(core.project.figures.len(), 2);
    must(&mut core, "undo", json!({}));
    assert_eq!(core.undo_depth(), 0, "undo consumed the step");
    assert_eq!(core.project.figures.len(), 1, "undo restored the project");
    must(&mut core, "redo", json!({}));
    assert_eq!(core.undo_depth(), 1, "redo restored the step");
    assert_eq!(core.project.figures.len(), 2);
}

// ---------------------------------------------------------------------
// SWAP tools — project replacement sits outside undo
// accounting by standing exclusion (the swap restarts the
// history). The one merge branch of import_file (.stk adds
// a figure) is the exception: it spends exactly one step,
// pinned here.
// ---------------------------------------------------------------------

#[test]
fn census_swap_tools_replace_the_project() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    must(&mut core, "add_frame", json!({}));
    assert_eq!(core.project.frames.len(), 6);
    must(&mut core, "new_project", json!({}));
    assert_eq!(core.project.frames.len(), 1, "new_project replaces wholesale");

    let path = tmp_path("wave_r_census_swap.styk.json");
    must(&mut core, "save_project", json!({ "path": path }));
    must(&mut core, "add_frame", json!({}));
    must(
        &mut core,
        "load_project",
        json!({ "path": tmp_path("wave_r_census_swap.styk.json") }),
    );
    assert_eq!(core.project.frames.len(), 1, "load_project replaces wholesale");
}

#[test]
fn census_import_file_stk_branch_spends_one_step() {
    let _serial = serial_guard();
    let mut core = fresh_core();
    let path = tmp_path("wave_r_census_fig.stk");
    must(
        &mut core,
        "export_file",
        json!({ "path": path, "format": "stk", "figure": 0 }),
    );
    let before = core.undo_depth();
    must(
        &mut core,
        "import_file",
        json!({ "path": tmp_path("wave_r_census_fig.stk") }),
    );
    assert_eq!(core.project.figures.len(), 2, "the .stk import added a figure");
    assert_eq!(core.undo_depth(), before + 1, "the .stk import is one undo step");
}
/// The non-mutating classification: every tool NOT in
/// the mutating legs or the exception legs, with its
/// class. Pinned against all_tools() by the
/// completeness test below.
const NON_MUTATING: &[(&str, &str)] = &[
    ("asset_info", "read"),
    ("assets_list", "read"),
    ("audio_clip_info", "read"),
    ("audio_list_clips", "read"),
    ("audio_mixdown", "library/file"),
    ("clear_loop_range", "session"),
    ("clip_compatible", "read"),
    ("clip_enter", "session"),
    ("clip_exit", "session"),
    ("clip_get_node_media", "read"),
    ("clip_get_pose", "read"),
    ("clip_goto", "session"),
    ("clip_list", "read"),
    ("copy_frame", "session"),
    ("copy_range", "session"),
    ("cycle_brush", "session"),
    ("cycle_project_file", "session"),
    ("delete_project_file", "library/file"),
    ("duplicate_project", "library/file"),
    ("export_bundle", "library/file"),
    ("export_file", "library/file"),
    ("fig3_camera_preset", "session"),
    ("fig3_camera_zoom", "session"),
    ("fig3_cycle_library", "session"),
    ("fig3_enter", "session"),
    ("fig3_exit", "session"),
    ("fig3_frame_camera", "session"),
    ("fig3_frame_preview", "session"),
    ("fig3_get", "read"),
    ("fig3_ik", "session"),
    ("fig3_library_list", "read"),
    ("fig3_load_from_library", "session"),
    ("fig3_mark_b", "session"),
    ("fig3_orbit", "session"),
    ("fig3_pose_apply", "session"),
    ("fig3_pose_library_list", "read"),
    ("fig3_pose_mirror", "session"),
    ("fig3_pose_save", "library/file"),
    ("fig3_rename", "session"),
    ("fig3_rotate", "session"),
    ("fig3_save_to_library", "library/file"),
    ("fig3_select", "session"),
    ("fig3_select_next", "session"),
    ("fig3_set_camera", "session"),
    ("fig3_set_root", "session"),
    ("fig3_set_rotation", "session"),
    ("figure_library_cycle", "session"),
    ("figure_library_list", "read"),
    ("figure_list_states", "read"),
    ("figure_library_rename", "library/file"),
    ("figure_library_save", "library/file"),
    ("figure_toggle_solo", "session"),
    ("fx_list", "read"),
    ("fx_preset_delete", "library/file"),
    ("fx_preset_list", "read"),
    ("fx_preset_save", "library/file"),
    ("get_brush", "read"),
    ("get_camera", "read"),
    ("get_clipboard_state", "read"),
    ("get_drawing_stats", "read"),
    ("get_editor_settings", "read"),
    ("get_editor_status", "read"),
    ("get_figure", "read"),
    ("get_keyframes", "read"),
    ("get_node", "read"),
    ("get_node_style", "read"),
    ("get_onion_skin", "read"),
    ("get_pose", "read"),
    ("get_project_stats", "read"),
    ("get_selection", "read"),
    ("get_state", "read"),
    ("get_stroke_style", "read"),
    ("goto_next_key", "session"),
    ("goto_next_keyframe", "session"),
    ("goto_prev_key", "session"),
    ("goto_prev_keyframe", "session"),
    ("graph_copy_keys", "read"),
    ("graph_count_keys_in_range", "read"),
    ("graph_get_channel", "read"),
    ("graph_key_interp", "read"),
    ("graph_list_channels", "read"),
    ("import_file", "swap"),
    ("list_brushes", "read"),
    ("list_figures", "read"),
    ("list_frames", "read"),
    ("list_projects", "read"),
    ("list_strokes", "read"),
    ("list_texts", "read"),
    ("load_project", "swap"),
    ("new_project", "swap"),
    ("open_project_file", "swap"),
    ("particle_list_events", "read"),
    ("pose_library_delete", "library/file"),
    ("pose_library_list", "read"),
    ("pose_library_rename", "library/file"),
    ("pose_library_save", "library/file"),
    ("project_rename", "library/file"),
    ("project_summary", "read"),
    ("redo", "history"),
    ("render", "library/file"),
    ("render_contact_sheet", "library/file"),
    ("render_frame_base64", "read"),
    ("save_named_project", "library/file"),
    ("save_project", "library/file"),
    ("save_project_copy", "library/file"),
    ("server_info", "read"),
    ("set_brush", "session"),
    ("set_brush_nib", "session"),
    ("set_draw_mode", "session"),
    ("set_draw_tool", "session"),
    ("set_frame", "session"),
    ("set_loop_range", "session"),
    ("set_onion_skin", "session"),
    ("set_playback", "session"),
    ("set_selection", "session"),
    ("shell_delete_project_file", "live-only"),
    ("shell_duplicate_project_file", "live-only"),
    ("shell_get_layout", "live-only"),
    ("shell_get_recent_colours", "live-only"),
    ("shell_get_selection", "live-only"),
    ("shell_get_settings", "live-only"),
    ("shell_get_state", "live-only"),
    ("shell_get_transport", "read"),
    ("shell_get_view", "live-only"),
    ("shell_list_recents", "live-only"),
    ("shell_open_recent", "live-only"),
    ("shell_pause", "session"),
    ("shell_play", "session"),
    ("shell_recover_snapshot", "live-only"),
    ("shell_reload_project", "live-only"),
    ("shell_select_audio_clip", "live-only"),
    ("shell_select_clip", "live-only"),
    ("shell_select_figure", "live-only"),
    ("shell_select_frame", "live-only"),
    ("shell_select_fx_clip", "live-only"),
    ("shell_select_graph_channel", "live-only"),
    ("shell_select_node", "live-only"),
    ("shell_select_particle", "live-only"),
    ("shell_select_sprite", "live-only"),
    ("shell_select_stroke", "live-only"),
    ("shell_set_column_widths", "live-only"),
    ("shell_set_onion_depths", "live-only"),
    ("shell_set_playhead", "session"),
    ("shell_set_section", "live-only"),
    ("shell_set_setting", "live-only"),
    ("shell_set_sidebar", "live-only"),
    ("shell_set_theme", "live-only"),
    ("shell_set_timeline", "live-only"),
    ("shell_set_workspace", "live-only"),
    ("shell_view_centre", "live-only"),
    ("shell_view_fit", "live-only"),
    ("shell_view_reset", "live-only"),
    ("shell_view_zoom", "live-only"),
    ("sprite_list", "read"),
    ("sprite_sample", "read"),
    ("step_brush_param", "session"),
    ("subtitle_export_srt", "read"),
    ("subtitle_export_vtt", "read"),
    ("subtitle_list", "read"),
    ("undo", "history"),
];

// ---------------------------------------------------------------------
// COMPLETENESS — the census covers every tool exactly once.
// all_tools() is the ground truth: a tool added later fails
// here until it is classified and given a leg.
// ---------------------------------------------------------------------

/// attach_image_to_node's leg is bespoke (it needs a planted
/// ./import file and cleans it up), so its name joins the
/// mutating set from here.
const EXTRA_MUTATING: &[&str] = &["attach_image_to_node"];

#[test]
fn census_covers_every_tool_exactly_once() {
    let _serial = serial_guard();
    let surface: BTreeSet<String> = all_tools().iter().map(|t| t.name.to_string()).collect();

    let mutating: BTreeSet<String> = a_legs()
        .iter()
        .map(|l| l.name.to_string())
        .chain(EXTRA_MUTATING.iter().map(|s| s.to_string()))
        .collect();
    let exceptions: BTreeSet<String> = exception_legs()
        .iter()
        .map(|l| l.name.to_string())
        .collect();
    let non_mutating: BTreeSet<String> = NON_MUTATING.iter().map(|(n, _)| n.to_string()).collect();

    assert_eq!(
        mutating.len(),
        211,
        "211 mutating tools (legs may repeat a tool with different args)"
    );
    assert_eq!(exceptions.len(), 21, "21 declared exceptions");
    assert_eq!(non_mutating.len(), 160, "160 non-mutating tools");
    assert!(
        mutating.is_disjoint(&exceptions),
        "a tool is both mutating and exception"
    );
    assert!(
        mutating.is_disjoint(&non_mutating),
        "a tool is both mutating and non-mutating"
    );
    assert!(
        exceptions.is_disjoint(&non_mutating),
        "a tool is both exception and non-mutating"
    );
    let covered: BTreeSet<String> = mutating
        .union(&exceptions)
        .cloned()
        .collect::<BTreeSet<_>>()
        .union(&non_mutating)
        .cloned()
        .collect();
    assert_eq!(
        covered, surface,
        "the census classification must cover the whole surface exactly"
    );

    // The behavioural leg groups must agree with the
    // classification table: session legs are "session",
    // read legs are "read", library/file legs are
    // "library/file".
    let class_of = |name: &str| {
        NON_MUTATING
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| *c)
    };
    for leg in session_legs() {
        assert_eq!(class_of(leg.name), Some("session"), "{}", leg.name);
    }
    for leg in read_legs() {
        assert_eq!(class_of(leg.name), Some("read"), "{}", leg.name);
    }
    for leg in libfile_legs() {
        assert_eq!(class_of(leg.name), Some("library/file"), "{}", leg.name);
    }
}
