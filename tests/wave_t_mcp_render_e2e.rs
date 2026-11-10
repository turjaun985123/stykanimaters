//! Wave T (T10) — the MCP `render` tool end to end,
//! driven through the real registry
//! (`stykanimate_rs::mcp::all_tools`, the undo census's
//! invoke idiom). Wave S's S18 proved the combination
//! credited cue + frame range + export scale at the CORE
//! entry level; these legs prove the same combination
//! survives the layer that actually sits between an MCP
//! caller and the renderer — argument parsing, R13's
//! range validation, and S5's scale routing:
//!
//! - **Range + scale + speaker through the registry.**
//!   `render(format="png_sequence", from_frame, to_frame,
//!   scale=2.0)` on a 6-frame credited project writes
//!   exactly the requested range at doubled dimensions;
//!   in-cue frames carry the prefixed ("Ana: ") burn-in
//!   at scaled geometry; out-of-cue frames in the range
//!   are the cue-free render byte for byte.
//! - **Range refusal through the registry.** A
//!   `to_frame` past the end of the timeline is refused
//!   with R13's sentence before anything is written,
//!   and spends no undo step.
//! - **Scale refusal through the registry.** `scale=0`
//!   is refused by the scaled entries' shared validation
//!   (wrapped in the tool's voice) before anything is
//!   written, and spends no undo step.
//! - **Legacy spelling.** The Wave 11 `from`/`to` keys
//!   drive the same ranged, scaled, credited render and
//!   echo under the spelling used.
//!
//! Fixtures set fps explicitly (24.0) per the wave's
//! timing contract; every test builds its own core and
//! its own temp directory.

use std::path::PathBuf;

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export::render_frame_scaled;
use stykanimate_rs::mcp::all_tools;
use stykanimate_rs::model::{Frame, Project};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleStyle};

// ---------------------------------------------------------------------
// Harness (the wave_r_undo_census invoke idiom)
// ---------------------------------------------------------------------

/// Invoke one tool's handler by name through the real
/// registry (`mcp::all_tools()`), the surface an MCP
/// caller actually reaches.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

// ---------------------------------------------------------------------
// Fixtures (the wave_s_e2e bare-stage shape)
// ---------------------------------------------------------------------

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

fn credited(start_frame: usize, end_frame: usize, text: &str, speaker: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: Some(speaker.to_string()),
    }
}

/// An empty stage (background 0x333333, no figures) with
/// `frames` identical empty authored frames, carrying
/// exactly `cues` under the default subtitle style, at
/// 24 fps — the wave_s_e2e `bare_stage_n` shape.
fn bare_stage_n(frames: usize, cues: Vec<SubtitleCue>) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = [480.0, 400.0];
    project.background = 0x333333;
    project.figures.clear();
    project.frames = (0..frames)
        .map(|_| Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    project.subtitles = cues;
    project.subtitle_style = SubtitleStyle::default();
    project
}

/// The credited fixture: six authored frames, the cue
/// credited to Ana live on frames 2 and 3 only
/// (half-open [2, 4)).
fn credited_project() -> Project {
    bare_stage_n(6, vec![credited(2, 4, "We go at dawn.", "Ana")])
}

/// A fresh core whose project is `project` — the
/// registry handlers read `core.project`, so installing
/// the fixture there is the whole setup.
fn core_with(project: Project) -> EditorCore {
    let mut core = EditorCore::new();
    core.project = project;
    core
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-t-mcp-render-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// ---------------------------------------------------------------------
// Pixel helpers (the wave_s_e2e measurement approach)
// ---------------------------------------------------------------------

const WHITE: [u8; 3] = [255, 255, 255];
const BG: [u8; 3] = [51, 51, 51]; // 0x333333

/// Rows of `img` that contain at least one pixel of
/// exactly `color`, in ascending order.
fn rows_with_color(img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>, color: [u8; 3]) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| img.get_pixel(x, y).0 == color) {
            rows.push(y);
        }
    }
    rows
}

/// Rows in which `a` and `b` differ in at least one
/// pixel (same-sized images).
fn differing_rows(
    a: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    b: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
) -> Vec<u32> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut rows = Vec::new();
    for y in 0..a.height() {
        if (0..a.width()).any(|x| a.get_pixel(x, y) != b.get_pixel(x, y)) {
            rows.push(y);
        }
    }
    rows
}

/// The (min, max) x columns carrying non-background ink
/// in `rows` — the horizontal extent of whatever was
/// stamped there.
fn ink_extents(
    img: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    rows: std::ops::Range<u32>,
) -> Option<(u32, u32)> {
    let mut min = u32::MAX;
    let mut max = 0u32;
    for y in rows {
        for x in 0..img.width() {
            if img.get_pixel(x, y).0 != BG {
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    if min == u32::MAX {
        None
    } else {
        Some((min, max))
    }
}

/// The scaled subtitle band of the fixture: at 2.0 the
/// 400-tall stage renders 800 tall and all burn-in ink
/// (face + one-cell outline) stays in the bottom 160
/// rows (S18's calibration of the same fixture).
const BAND: std::ops::Range<u32> = 640..800;

// ---------------------------------------------------------------------
// Leg 1 — range + scale + speaker through the registry
// ---------------------------------------------------------------------

#[test]
fn render_range_scale_2_credited_png_sequence_through_the_registry() {
    let project = credited_project();
    let control = bare_stage_n(6, Vec::new());
    let uncredited = bare_stage_n(6, vec![cue(2, 4, "We go at dawn.")]);
    let mut core = core_with(project.clone());
    let depth_before = core.undo_depth();

    let dir = temp_dir("range-scale-credited");
    let target = dir.join("seq");
    let out = invoke(
        &mut core,
        "render",
        &json!({
            "path": target.to_string_lossy(),
            "format": "png_sequence",
            "from_frame": 1,
            "to_frame": 4,
            "scale": 2.0,
        }),
    )
    .expect("ranged scaled render through the registry failed");

    // The reply counts the range, names the directory,
    // and echoes the bounds under the spelling used.
    assert_eq!(out["format"], "png_sequence");
    assert_eq!(out["frames"], 4, "exactly the requested range: {out}");
    assert_eq!(
        out["directory"].as_str().expect("directory is a string"),
        target.to_string_lossy().as_ref()
    );
    assert_eq!(out["from_frame"], 1);
    assert_eq!(out["to_frame"], 4);
    assert!(
        out.get("from").is_none() && out.get("to").is_none(),
        "legacy echo keys must not appear: {out}"
    );
    assert_eq!(
        core.undo_depth(),
        depth_before,
        "a render is read-only: it spends no undo step"
    );

    // Exactly the range's files were written, each named
    // for its authored frame (frame i -> frame-XXXX with
    // XXXX = i + 1, the sequence entries' naming).
    let mut names: Vec<String> = std::fs::read_dir(&target)
        .expect("the sequence directory exists")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "frame-0002.png".to_string(),
            "frame-0003.png".to_string(),
            "frame-0004.png".to_string(),
            "frame-0005.png".to_string(),
        ],
        "the written frames are exactly the requested range"
    );

    for i in 1..=4usize {
        let file = target.join(format!("frame-{:04}.png", i + 1));
        let img = image::open(&file)
            .expect("exported frame reads back")
            .to_rgb8();
        assert_eq!(
            (img.width(), img.height()),
            (960, 800),
            "frame {i}: the exported frame is a true 2x render"
        );
        // What the registry wrote is the direct scaled
        // render of that authored frame, byte for byte —
        // routing added no resampling of its own.
        let direct = render_frame_scaled(&project, i, 2.0).expect("direct scaled render");
        assert_eq!(
            img.as_raw(),
            direct.as_raw(),
            "frame {i}: the registry's file is the direct scaled render"
        );
        let ctrl = render_frame_scaled(&control, i, 2.0).expect("control render");
        if (2..4).contains(&i) {
            // In the cue's range: the burn-in is present,
            // confined to the scaled band, at scaled
            // geometry.
            assert_ne!(
                img.as_raw(),
                ctrl.as_raw(),
                "frame {i}: a live credited cue must burn in"
            );
            let diff = differing_rows(&img, &ctrl);
            assert!(
                diff.iter().all(|&y| BAND.contains(&y)),
                "frame {i}: burn-in paint outside the scaled band: {diff:?}"
            );
            let face_rows = rows_with_color(&img, WHITE);
            assert!(
                (36..=44).contains(&face_rows.len()),
                "frame {i}: face spans {} rows, expected the 40-row scaled house size",
                face_rows.len()
            );
            // The prefix widens the centred line BOTH
            // ways at the scaled size: the credited
            // render's band ink reaches further left and
            // further right than the uncredited render's
            // (S18's measurement, now through the
            // registry's files).
            let plain = render_frame_scaled(&uncredited, i, 2.0).expect("uncredited render");
            let (cmin, cmax) = ink_extents(&img, BAND.clone()).expect("credited band has ink");
            let (umin, umax) = ink_extents(&plain, BAND.clone()).expect("uncredited band has ink");
            assert!(
                cmin < umin && cmax > umax,
                "frame {i}: the 'Ana: ' prefix must widen the line at scale 2 \
                 (credited {cmin}..{cmax}, uncredited {umin}..{umax})"
            );
        } else {
            // In the export range but outside the cue's
            // range: the cue-free render, byte for byte —
            // the cue leaves no trace on frames it does
            // not cover.
            assert_eq!(
                img.as_raw(),
                ctrl.as_raw(),
                "frame {i}: no cue is live, so no burn-in may appear"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — range refusal through the registry
// ---------------------------------------------------------------------

#[test]
fn render_range_past_the_end_is_refused_through_the_registry_and_writes_nothing() {
    let mut core = core_with(credited_project());
    let depth_before = core.undo_depth();
    let dir = temp_dir("range-refused");
    let target = dir.join("seq");
    let err = invoke(
        &mut core,
        "render",
        &json!({
            "path": target.to_string_lossy(),
            "format": "png_sequence",
            "from_frame": 0,
            "to_frame": 6,
            "scale": 2.0,
        }),
    )
    .expect_err("a range past the end of the timeline must be refused");
    // R13's sentence, verbatim: the bound is refused,
    // never clamped to a shorter export.
    assert_eq!(
        err,
        "argument 'to_frame' 6 is out of range (project has 6 frames)"
    );
    assert!(
        !target.exists(),
        "a refused range wrote {target:?} — validation runs before any write"
    );
    assert_eq!(
        core.undo_depth(),
        depth_before,
        "a refused render spends no undo step"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — scale refusal through the registry
// ---------------------------------------------------------------------

#[test]
fn render_scale_zero_is_refused_through_the_registry_and_writes_nothing() {
    let mut core = core_with(credited_project());
    let depth_before = core.undo_depth();
    let dir = temp_dir("scale-refused");
    let target = dir.join("seq");
    let err = invoke(
        &mut core,
        "render",
        &json!({
            "path": target.to_string_lossy(),
            "format": "png_sequence",
            "scale": 0,
        }),
    )
    .expect_err("scale 0 must be refused");
    // The scaled entries' shared validation sentence,
    // wrapped in the render tool's voice (S5's routing
    // reaches check_export_scale before any write).
    assert_eq!(
        err,
        format!(
            "render to {} failed: export scale must be a positive finite number, got 0",
            target.to_string_lossy()
        )
    );
    assert!(
        !target.exists(),
        "a refused scale wrote {target:?} — the output directory is untouched"
    );
    assert_eq!(
        core.undo_depth(),
        depth_before,
        "a refused render spends no undo step"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 4 — the Wave 11 from/to spelling drives the same render
// ---------------------------------------------------------------------

#[test]
fn render_range_legacy_from_to_spelling_drives_the_same_scaled_credited_render() {
    let project = credited_project();
    let control = bare_stage_n(6, Vec::new());
    let mut core = core_with(project.clone());
    let dir = temp_dir("range-legacy");
    let target = dir.join("seq");
    let out = invoke(
        &mut core,
        "render",
        &json!({
            "path": target.to_string_lossy(),
            "format": "png_sequence",
            "from": 1,
            "to": 4,
            "scale": 2.0,
        }),
    )
    .expect("legacy-spelling ranged scaled render failed");
    // Same render, echoed under the Wave 11 spelling.
    assert_eq!(out["frames"], 4);
    assert_eq!(out["from"], 1);
    assert_eq!(out["to"], 4);
    assert!(
        out.get("from_frame").is_none() && out.get("to_frame").is_none(),
        "framed echo keys must not appear: {out}"
    );
    assert_eq!(
        std::fs::read_dir(&target).expect("sequence dir").count(),
        4,
        "the legacy spelling writes exactly the range"
    );
    // An in-cue frame written under the legacy spelling
    // is the same prefixed, scaled render: byte-equal to
    // the direct scaled render and visibly burned in
    // against the cue-free control.
    let img = image::open(target.join("frame-0004.png"))
        .expect("in-cue frame reads back")
        .to_rgb8();
    let direct = render_frame_scaled(&project, 3, 2.0).expect("direct scaled render");
    assert_eq!(img.as_raw(), direct.as_raw());
    let ctrl = render_frame_scaled(&control, 3, 2.0).expect("control render");
    assert_ne!(
        img.as_raw(),
        ctrl.as_raw(),
        "frame 3 is in the cue's range: the burn-in must be present"
    );
    let face_rows = rows_with_color(&img, WHITE);
    assert!(
        (36..=44).contains(&face_rows.len()),
        "frame 3: face spans {} rows, expected the 40-row scaled house size",
        face_rows.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
