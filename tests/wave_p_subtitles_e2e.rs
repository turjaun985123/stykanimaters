//! Wave P (P34) — subtitles end-to-end pins, through the
//! public API only.
//!
//! The unit suites pin the pieces (P24: SRT discipline,
//! P25: WebVTT discipline, P22: Project-level field
//! losslessness). This file pins the JOINS — the paths a
//! user's subtitles actually travel:
//!
//! 1. LEGACY LOAD — a project file written before the
//!    subtitle fields existed (the JSON simply has no
//!    `subtitles` / `subtitle_style` keys) loads through
//!    the app's own loader with an empty cue list and the
//!    default style: bottom-anchored, unscaled, white text
//!    over a black backing (board contract 1; the serde
//!    defaults on the `Project` fields).
//! 2. CORE ROUND TRIP — cues authored through the
//!    `EditorCore` ops (which keep the list sorted and
//!    spend exactly one undo step per mutation) plus a
//!    non-default style survive the app's own save path
//!    and a load into a FRESH core with cues and style
//!    identical — and the loaded core's undo depth is 0:
//!    opening a file is not an edit.
//! 3. EXPORT PRESENCE — a frame with a live cue renders
//!    through the raster exporter differently from the
//!    same frame cue-less, and identically on a frame
//!    where the cue is not live. The burn-in itself is
//!    P26's work; at the time this file was written
//!    `src/export.rs` carried no subtitle rendering (grep:
//!    its only subtitle mentions were exhaustive `Project`
//!    literals), so the presence assertion pins board
//!    contract 1 and is expected to fail until P26 lands.
//!    The other two assertions in that test (same-project
//!    determinism, not-live control) are P26-independent.
//!
//! 4. MCP ROUND TRIP — cues authored through the MCP
//!    cue tools survive an SRT export through the MCP
//!    IO tool and an import into a fresh core: identical
//!    cues, exactly one undo step, and an identical
//!    re-import reported as unchanged with no further
//!    step. This leg was P34's filed drop-in
//!    (IA-P34-1, in `~/workspace/wave-p/findings/p34.md`)
//!    and is appended at wiring time by Wave Q (Q19),
//!    once Q1's registration put P29's and P30's tools
//!    in `mcp::all_tools()`.
//!
//! Timing is pinned at 24.0 fps everywhere it matters
//! (board contract 3); the demo project runs at 12 fps,
//! so the rate is always set explicitly. No status or
//! error WORDING is asserted — the contracts fix
//! behaviour, not voice.

use std::path::PathBuf;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export;
use stykanimate_rs::model::Project;
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};

/// The frame rate every pin in this file runs at.
const FPS: f32 = 24.0;

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

/// A fully non-default style (the same values P22's
/// round-trip fixture uses, so the wave's suites agree on
/// what "non-default" looks like). Built fresh by each
/// call so no `Clone` assumption is needed.
fn non_default_style() -> SubtitleStyle {
    SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 1.5,
        text_color: 0xFFE680,
        box_color: 0x102040,
    }
}

/// Per-process, per-test scratch directory under the OS
/// temp dir (the P22 / export_determinism.rs idiom).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-p-subtitles-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn legacy_project_json_without_subtitle_fields_loads_with_defaults() {
    // Author a project that HAS cues and a non-default
    // style, serialise it, then strip the two subtitle
    // keys from the JSON — the exact shape of every
    // project file written before contract 1 existed.
    let mut authored = Project::new_demo();
    authored.name = "Legacy Subtitles".into();
    authored.fps = FPS;
    authored.subtitles = vec![
        cue(0, 24, "The gate is open."),
        cue(24, 48, "Now the yard."),
    ];
    authored.subtitle_style = non_default_style();

    let mut value = serde_json::to_value(&authored).expect("serialise authored project");
    let object = value.as_object_mut().expect("project serialises to an object");
    // Non-vacuity: the keys were really there to strip.
    assert!(
        object.remove("subtitles").is_some(),
        "the serialised project carries a subtitles key to strip"
    );
    assert!(
        object.remove("subtitle_style").is_some(),
        "the serialised project carries a subtitle_style key to strip"
    );
    let legacy_text = serde_json::to_string_pretty(&value).expect("serialise legacy shape");
    assert!(
        !legacy_text.contains("\"subtitles\""),
        "the legacy text has no subtitles key"
    );
    assert!(
        !legacy_text.contains("\"subtitle_style\""),
        "the legacy text has no subtitle_style key"
    );

    // Write it as a file and load it through the app's
    // own loader — the path a user's old project takes.
    let dir = temp_dir("legacy");
    let path = dir.join("legacy.json");
    std::fs::write(&path, &legacy_text).expect("write legacy project file");
    let loaded = projects::load(&path).expect("legacy project loads");

    assert!(
        loaded.subtitles.is_empty(),
        "a file without the subtitles field loads with an empty cue list"
    );
    assert_eq!(
        loaded.subtitle_style,
        SubtitleStyle::default(),
        "a file without the subtitle_style field loads with the default style"
    );
    // The default style is the documented plain look
    // (model.rs on the field): bottom-anchored, unscaled,
    // white text over a black backing.
    assert_eq!(
        loaded.subtitle_style.position,
        SubtitlePosition::Bottom,
        "the default style is bottom-anchored"
    );
    assert_eq!(
        loaded.subtitle_style.font_scale, 1.0,
        "the default style is unscaled"
    );
    assert_eq!(
        loaded.subtitle_style.text_color, 0xFF_FFFF,
        "the default style's text is white"
    );
    assert_eq!(
        loaded.subtitle_style.box_color, 0x00_0000,
        "the default style's backing is black"
    );

    // Non-vacuity: the rest of the project really loaded —
    // the defaults above are the subtitle fields' doing,
    // not an empty file's.
    assert_eq!(loaded.name, "Legacy Subtitles");
    assert_eq!(loaded.fps, FPS);
    assert_eq!(loaded.figures.len(), 1, "the demo figure loaded");
    assert_eq!(loaded.frames.len(), 5, "the demo frames loaded");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn core_authored_cues_and_style_survive_save_and_fresh_load() {
    let dir = temp_dir("core");

    let mut core = EditorCore::new();
    core.project.name = "Subtitled E2E".into();
    core.project.fps = FPS;
    assert_eq!(core.undo_depth(), 0, "a fresh core has no undo history");

    // Author three cues OUT of chronological order: the
    // ops keep the list sorted by (start, end) and report
    // each new cue's index in that sorted list. Exactly
    // one undo step per accepted add (contract 1).
    assert_eq!(
        core.subtitle_add(48, 96, "And the long road home.", None),
        Some(0),
        "the first add lands at index 0"
    );
    assert_eq!(
        core.subtitle_add(0, 24, "The gate is open.", None),
        Some(0),
        "an earlier cue inserts at the head of the sorted list"
    );
    assert_eq!(
        core.subtitle_add(24, 48, "Now the yard.", None),
        Some(1),
        "a middle cue inserts between its neighbours"
    );
    let expected = vec![
        cue(0, 24, "The gate is open."),
        cue(24, 48, "Now the yard."),
        cue(48, 96, "And the long road home."),
    ];
    assert_eq!(
        core.project.subtitles, expected,
        "the ops kept the cue list sorted by (start, end)"
    );
    assert_eq!(
        core.undo_depth(),
        3,
        "three accepted adds spend exactly three undo steps"
    );
    assert_eq!(
        core.subtitle_at(30).map(|c| c.text.as_str()),
        Some("Now the yard."),
        "the middle cue is the live one at frame 30"
    );
    assert_eq!(
        core.subtitles_at(30).len(),
        1,
        "exactly one cue is live at frame 30"
    );

    // A non-default style through the op: one more undo
    // step, and the project's style is the one given. The
    // op's return value is deliberately not matched on —
    // its EFFECTS are the contract, and they are asserted
    // here and after the round trip below.
    let _ = core.set_subtitle_style(non_default_style());
    assert_eq!(
        core.project.subtitle_style,
        non_default_style(),
        "the style op set the given style"
    );
    assert_eq!(
        core.undo_depth(),
        4,
        "the style set spends exactly one more undo step"
    );

    // Save through the app's own save path, load the file
    // back, and put it in a FRESH core — the editor's own
    // open shape (P22's idiom).
    let path = projects::save(&core.project, &dir).expect("save authored project");
    let loaded = projects::load(&path).expect("load authored project");
    let mut fresh = EditorCore::new();
    fresh.project = loaded;

    assert_eq!(
        fresh.undo_depth(),
        0,
        "the loaded core's undo depth is 0 — opening a file is not an edit"
    );
    assert_eq!(fresh.redo_depth(), 0, "the loaded core has no redo history");
    assert_eq!(
        fresh.project.subtitles, expected,
        "the cues are identical after save → load"
    );
    assert_eq!(
        fresh.project.subtitle_style,
        non_default_style(),
        "the style is identical after save → load"
    );
    assert_eq!(fresh.project.fps, FPS, "the frame rate survived the trip");
    assert_eq!(
        fresh.subtitle_at(30).map(|c| c.text.as_str()),
        Some("Now the yard."),
        "the loaded core answers live-cue queries"
    );
    assert!(
        fresh.subtitle_at(96).is_none(),
        "the last cue's range is half-open: nothing is live at its end frame"
    );
    // Byte-form closer (the writer is deterministic —
    // P22's premise): re-serialising the loaded project
    // reproduces the authored project's bytes exactly.
    assert_eq!(
        serde_json::to_string_pretty(&fresh.project).expect("re-serialise loaded project"),
        serde_json::to_string_pretty(&core.project).expect("serialise authored project"),
        "save → load is byte-identical for the subtitled project"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn raster_export_burns_in_a_live_cue() {
    // The demo project cut to 3 authored frames (the
    // export_determinism.rs idiom, so CI stays fast),
    // with and without one cue live over frames [1, 3).
    fn fixture(cued: bool) -> Project {
        let mut project = Project::new_demo();
        project.frames.truncate(3);
        project.fps = FPS;
        if cued {
            project.subtitles = vec![cue(1, 3, "The gate is open.")];
        }
        project
    }

    // Precondition (P26-independent): the raster path is
    // deterministic for a fixed project, so a byte
    // difference below can only be the cue's doing.
    let first = export::render_frame(&fixture(true), 2);
    let second = export::render_frame(&fixture(true), 2);
    assert_eq!(
        first.as_raw(),
        second.as_raw(),
        "two renders of the same subtitled frame are byte-identical"
    );

    // Presence: frame 2 is live for the cue, so the
    // burned-in frame must differ from the cue-less
    // render of the same frame. This is the assertion
    // that pins P26's burn-in (see the module docs: it is
    // expected to fail until the burn-in lands).
    let with_cue = export::render_frame(&fixture(true), 2);
    let without_cue = export::render_frame(&fixture(false), 2);
    assert_ne!(
        with_cue.as_raw(),
        without_cue.as_raw(),
        "a frame with a live cue renders differently from the same frame \
         cue-less (P26's raster burn-in, board contract 1)"
    );

    // Not-live control (P26-independent in shape): at
    // frame 0 the cue is not live, so carrying it must
    // change nothing — the difference above is the live
    // cue's rendering, not a project-identity effect.
    let idle_with = export::render_frame(&fixture(true), 0);
    let idle_without = export::render_frame(&fixture(false), 0);
    assert_eq!(
        idle_with.as_raw(),
        idle_without.as_raw(),
        "a frame where the cue is not live renders identically with and \
         without the cue in the project"
    );
}

// -----------------------------------------------------------------
// Leg 4 — the MCP round trip (Wave Q, Q19; IA-P34-1).
// P34 filed this test as a drop-in in its findings
// because, at its write time, P29's cue tools did not
// exist in the tree and P30's IO module was not yet
// declared or registered. Q1's wiring puts both in
// `mcp::all_tools()`, so the leg compiles here against
// the tree's real dispatch surface. Argument keys are
// P29's registered schema keys; the reply keys
// (`count`, `changed`) are P30's filed contract — both
// verified against the modules in tree by Q19.
// -----------------------------------------------------------------

/// Dispatch one MCP tool by name through the real
/// registry (the same `(tool.handler)(core, &args)`
/// call the modules' own tests use). A missing tool or
/// a refused call is a test failure with the tool named.
fn call(core: &mut EditorCore, name: &str, args: serde_json::Value) -> serde_json::Value {
    let tools = stykanimate_rs::mcp::all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} is registered"));
    (tool.handler)(core, &args).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn mcp_subtitle_round_trip() {
    let dir = temp_dir("mcp");
    let srt_path = dir.join("cues.srt");
    let mut core = EditorCore::new();
    core.project.fps = FPS;

    // Author through the cue tools (keys per P29's schemas).
    call(
        &mut core,
        "subtitle_add",
        serde_json::json!({
            "start_frame": 0, "end_frame": 24, "text": "The gate is open."
        }),
    );
    call(
        &mut core,
        "subtitle_add",
        serde_json::json!({
            "start_frame": 24, "end_frame": 48, "text": "Now the yard."
        }),
    );
    let listed = call(&mut core, "subtitle_list", serde_json::json!({}));
    // The list reports exactly the two authored cues, in
    // the kept (sorted) order (reply shape per P29).
    let listed_cues = listed["subtitles"]
        .as_array()
        .expect("the list reply carries a subtitles array");
    assert_eq!(
        listed_cues.len(),
        2,
        "the list reports exactly the two authored cues"
    );
    assert_eq!(listed_cues[0]["start_frame"], 0);
    assert_eq!(listed_cues[0]["end_frame"], 24);
    assert_eq!(listed_cues[0]["text"], "The gate is open.");
    assert_eq!(listed_cues[1]["start_frame"], 24);
    assert_eq!(listed_cues[1]["end_frame"], 48);
    assert_eq!(listed_cues[1]["text"], "Now the yard.");

    // Export through the IO tool; the file is the converter's
    // document for the authored cues.
    let exported = call(
        &mut core,
        "subtitle_export_srt",
        serde_json::json!({ "path": srt_path.to_string_lossy() }),
    );
    assert_eq!(exported["count"], 2);

    // Import into a fresh core: one undo step, changed: true,
    // cues identical to the authored ones.
    let mut fresh = EditorCore::new();
    fresh.project.fps = FPS;
    let imported = call(
        &mut fresh,
        "subtitle_import_srt",
        serde_json::json!({ "path": srt_path.to_string_lossy() }),
    );
    assert_eq!(imported["changed"], true);
    assert_eq!(fresh.project.subtitles, core.project.subtitles);
    assert_eq!(fresh.undo_depth(), 1);

    // Identical re-import: changed: false, no further undo step.
    let again = call(
        &mut fresh,
        "subtitle_import_srt",
        serde_json::json!({ "path": srt_path.to_string_lossy() }),
    );
    assert_eq!(again["changed"], false);
    assert_eq!(fresh.undo_depth(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}
