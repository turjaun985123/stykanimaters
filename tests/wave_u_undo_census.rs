//! Wave U (U13) — the undo-census extension for Wave T's
//! MCP-surface behaviour changes.
//!
//! Wave R's R7 census pinned all 381 tools' undo-step
//! behaviour as of Wave R; Wave T's T13 extension pinned
//! Wave S's two behaviour changes. Wave T then changed
//! behaviour on exactly two more existing tools (the
//! per-slice evidence is in ~/workspace/wave-t/findings/
//! t7.md and t8.md; the audit is repeated in
//! ~/workspace/wave-u/findings/u13.md):
//!
//!   * `render` (T7) — the `scale` argument, which before
//!     Wave T routed for every raster format EXCEPT 'jpeg',
//!     now routes for 'jpeg' too: a scaled call reaches the
//!     true-scaled JPEG-sequence entries
//!     (`export_jpeg_sequence_scaled` /
//!     `export_jpeg_sequence_range_scaled`), and an invalid
//!     scale is refused by the entries' shared validation
//!     BEFORE the target directory is created. `render`
//!     writes files and mutates no project state: 0 steps
//!     for the new scaled routes, and 0 steps (plus
//!     nothing written) for their refusals.
//!   * `export_bundle` (T8) — the tool already accepted
//!     `scale` (the Wave R routing), but validated it only
//!     inside the PNG member, after other members were
//!     written: a bad scale produced a partial bundle
//!     behind a successful call, and a non-number scale
//!     was silently treated as absent. Wave T validates a
//!     present `scale` up front — before `create_dir_all`,
//!     before any member — so an invalid scale (zero,
//!     negative, non-finite after the f32 cast, or not a
//!     number at all) refuses the WHOLE call with nothing
//!     written, and a valid scale still scales only the
//!     'images' bundle's PNG-sequence member.
//!
//! Nothing else at the MCP surface changed behaviour in
//! Wave T: T7/T8 touched no other tool, and the surface
//! stayed 381 (the S+T integration census). Tools whose
//! behaviour Wave T did not change keep their R7/T13
//! pins and are NOT re-pinned here.
//!
//! Discipline mirrors the earlier censuses: invoke
//! through the real registry (`mcp::all_tools()`), the
//! undo depth is read AFTER fixture setup so only the
//! measured call is counted, timing fixtures set fps
//! 24.0 explicitly, and every leg writes only inside its
//! own per-test temp dir — no leg touches a CWD-relative
//! store, so no serial guard is needed. The suite has no
//! input image fixtures at all: the JPEG/PNG bytes it
//! inspects are produced by the app's own encoders.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::all_tools;

// ---------------------------------------------------------------------
// Harness (the wave_r / wave_t undo-census idiom)
// ---------------------------------------------------------------------

/// A fresh demo core (1 humanoid figure, 5 frames) with
/// the project's fps set to 24.0 explicitly, per the
/// board's timing-fixture rule.
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

/// A per-test temp directory (created empty); each leg
/// writes its export targets inside its own.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wave_u_undo_census_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create per-test temp dir");
    dir
}

fn path_str(dir: &std::path::Path, file: &str) -> String {
    dir.join(file).to_string_lossy().into_owned()
}

/// The reply's key set, for exact-shape assertions: the
/// no-scale-echo discipline is a statement about which
/// keys exist, not just what they hold.
fn reply_keys(reply: &Value) -> BTreeSet<String> {
    reply
        .as_object()
        .expect("tool replies are objects")
        .keys()
        .cloned()
        .collect()
}

fn key_set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// A fresh core on a tiny stage, so the scaled renders
/// are cheap (the wave_t census fixture); 5 frames.
fn small_stage_core() -> EditorCore {
    let mut core = fresh_core();
    core.project.stage = [96.0, 72.0];
    core
}

/// T8's in-crate bundle fixture: the demo project at
/// fps 24.0 on a 120x90 stage, truncated to 3 frames so
/// every bundle member stays tiny.
fn bundle_core() -> EditorCore {
    let mut core = fresh_core();
    core.project.stage = [120.0, 90.0];
    core.project.frames.truncate(3);
    core
}

/// Every file under `dir` as (relative path, bytes),
/// sorted by path — T8's member-wise bundle comparison.
fn bundle_tree(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("read bundle dir") {
            let p = entry.expect("bundle entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir)
                        .expect("member under the bundle dir")
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(&p).expect("read bundle member"),
                ));
            }
        }
    }
    out.sort();
    out
}

/// The (width, height) out of a PNG's IHDR header.
fn png_dims_of(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n", "not a PNG file");
    (
        u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width")),
        u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height")),
    )
}

// ---------------------------------------------------------------------
// render: jpeg scale routing (Wave T, T7)
// ---------------------------------------------------------------------

#[test]
fn render_jpeg_sequence_at_scale_spends_nothing() {
    let dir = test_dir("render_jpeg_scaled");
    let target = path_str(&dir, "seq");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "render",
        &json!({ "path": target, "format": "jpeg", "scale": 2.0 }),
    )
    .expect("a scaled JPEG-sequence render succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "render writes files, never project state: no undo step"
    );
    // The scaled reply has exactly the unscaled shape —
    // the directory echo, and no scale echo (T7's routing
    // discipline, the png_sequence arm's shape).
    assert_eq!(
        reply_keys(&reply),
        key_set(&["directory", "format", "frames", "path"]),
        "the scaled jpeg reply adds no fields"
    );
    assert_eq!(reply["frames"], json!(5));
    assert_eq!(reply["directory"], json!(target));
    // All five frames written at twice the stage size
    // (96x72 at 2.0), as real JPEGs.
    for n in 1..=5 {
        let file = dir.join("seq").join(format!("frame-{n:04}.jpg"));
        let bytes = std::fs::read(&file).expect("each frame file was written");
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "frame {n} carries the JPEG SOI marker");
    }
    let img = image::open(dir.join("seq").join("frame-0001.jpg")).expect("frame 1 decodes");
    assert_eq!((img.width(), img.height()), (192, 144), "2.0x JPEG frame dims");
}

#[test]
fn render_jpeg_sequence_ranged_at_scale_spends_nothing() {
    let dir = test_dir("render_jpeg_ranged_scaled");
    let target = path_str(&dir, "seq");
    let mut core = small_stage_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "render",
        &json!({
            "path": target, "format": "jpeg",
            "from_frame": 1, "to_frame": 3, "scale": 0.5
        }),
    )
    .expect("a ranged, scaled JPEG-sequence render succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "render writes files, never project state: no undo step"
    );
    assert_eq!(reply["frames"], json!(3));
    // The range echoes under the caller's spelling (R13);
    // scale still adds no field of its own.
    assert_eq!(
        reply_keys(&reply),
        key_set(&["directory", "format", "frames", "from_frame", "path", "to_frame"]),
        "range echo present, no scale echo"
    );
    // Authored-index naming: the range 1..=3 writes
    // frames 2-4, never frame 1.
    assert!(
        !dir.join("seq").join("frame-0001.jpg").exists(),
        "the range must not write frame 1"
    );
    let img = image::open(dir.join("seq").join("frame-0002.jpg")).expect("frame 2 decodes");
    assert_eq!((img.width(), img.height()), (48, 36), "0.5x JPEG frame dims");
}

#[test]
fn render_jpeg_sequence_scale_refusals_spend_nothing_and_write_nothing() {
    let dir = test_dir("render_jpeg_refused");
    let mut core = small_stage_core();
    // Scale 0, full timeline: the refusal is the scaled
    // entries' shared validation, wrapped in the render
    // tool's voice — asserted verbatim (the T10 idiom).
    let target = path_str(&dir, "seq-zero");
    let before = core.undo_depth();
    let err = invoke(
        &mut core,
        "render",
        &json!({ "path": target, "format": "jpeg", "scale": 0 }),
    )
    .expect_err("scale 0 is refused by the scaled entries");
    assert_eq!(
        err,
        format!(
            "render to {target} failed: export scale must be a positive finite number, got 0"
        )
    );
    assert_eq!(core.undo_depth(), before, "a refused render spends no undo step");
    assert!(
        !std::path::Path::new(&target).exists(),
        "a refused render creates no directory"
    );
    // Negative and f32-overflowing scales refuse the same
    // way (the message carries the f32 the entry
    // received: -2.5 verbatim, 1e300 as its cast, inf).
    for (name, scale, got) in [
        ("seq-neg", json!(-2.5), "-2.5"),
        ("seq-huge", json!(1e300), "inf"),
    ] {
        let target = path_str(&dir, name);
        let before = core.undo_depth();
        let err = invoke(
            &mut core,
            "render",
            &json!({ "path": target, "format": "jpeg", "scale": scale }),
        )
        .expect_err("an invalid scale is refused");
        assert_eq!(
            err,
            format!(
                "render to {target} failed: export scale must be a positive finite number, got {got}"
            )
        );
        assert_eq!(core.undo_depth(), before, "a refused render spends no undo step");
        assert!(
            !std::path::Path::new(&target).exists(),
            "a refused render creates no directory"
        );
    }
    // Ranged + scale: the validation still fires before
    // anything is written.
    let target = path_str(&dir, "seq-ranged");
    let before = core.undo_depth();
    let err = invoke(
        &mut core,
        "render",
        &json!({
            "path": target, "format": "jpeg",
            "from_frame": 0, "to_frame": 2, "scale": 0
        }),
    )
    .expect_err("a ranged scale 0 is refused");
    assert_eq!(
        err,
        format!(
            "render to {target} failed: export scale must be a positive finite number, got 0"
        )
    );
    assert_eq!(core.undo_depth(), before, "a refused render spends no undo step");
    assert!(
        !std::path::Path::new(&target).exists(),
        "a refused ranged render creates no directory"
    );
}

// ---------------------------------------------------------------------
// export_bundle: refuse-before-any-write validation (Wave T, T8)
// ---------------------------------------------------------------------

#[test]
fn bundle_valid_scale_spends_nothing_and_scales_only_the_png_member() {
    let dir = test_dir("bundle_scaled");
    let plain_dir = dir.join("plain");
    let scaled_dir = dir.join("scaled");
    let mut core = bundle_core();
    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "export_bundle",
        &json!({ "path": plain_dir.to_string_lossy(), "kind": "images" }),
    )
    .expect("an unscaled images bundle succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "export_bundle writes files, never project state: no undo step"
    );
    assert_eq!(
        reply_keys(&reply),
        key_set(&["directory", "failed", "kind", "members", "ok", "succeeded"]),
        "the bundle reply keeps its established shape"
    );
    assert_eq!(reply["succeeded"], json!(4));
    assert_eq!(reply["failed"], json!(0));

    let before = core.undo_depth();
    let reply = invoke(
        &mut core,
        "export_bundle",
        &json!({
            "path": scaled_dir.to_string_lossy(), "kind": "images", "scale": 2.0
        }),
    )
    .expect("a scaled images bundle succeeds");
    assert_eq!(
        core.undo_depth(),
        before,
        "a scaled bundle spends no undo step either"
    );
    assert_eq!(reply["succeeded"], json!(4));
    assert_eq!(reply["failed"], json!(0));

    // Member-wise, T8's in-crate contract at registry
    // level: the same member set; every png/ member's
    // dims double (120x90 at 2.0); the jpeg/, svg/ and
    // spritesheet members are byte-identical — the
    // bundle's scale honours the PNG member only.
    let plain = bundle_tree(&plain_dir);
    let scaled = bundle_tree(&scaled_dir);
    let plain_paths: Vec<&str> = plain.iter().map(|(p, _)| p.as_str()).collect();
    let scaled_paths: Vec<&str> = scaled.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(plain_paths, scaled_paths, "the member sets must agree");
    assert!(
        plain_paths.iter().any(|p| p.starts_with("png/")),
        "the PNG member must be present: {plain_paths:?}"
    );
    for ((p, plain_bytes), (_, scaled_bytes)) in plain.iter().zip(&scaled) {
        if p.starts_with("png/") {
            assert_eq!(png_dims_of(plain_bytes), (120, 90), "{p} at 1.0");
            assert_eq!(png_dims_of(scaled_bytes), (240, 180), "{p} at 2.0");
        } else {
            assert_eq!(
                plain_bytes, scaled_bytes,
                "member {p} must be byte-unaffected by the PNG member's scale"
            );
        }
    }
}

#[test]
fn bundle_explicit_scale_1_0_matches_absent_and_spends_nothing() {
    let dir = test_dir("bundle_one");
    let absent_dir = dir.join("absent");
    let one_dir = dir.join("one");
    let mut core = bundle_core();
    let before = core.undo_depth();
    invoke(
        &mut core,
        "export_bundle",
        &json!({ "path": absent_dir.to_string_lossy(), "kind": "images" }),
    )
    .expect("an absent-scale bundle succeeds");
    assert_eq!(core.undo_depth(), before, "no undo step");
    let before = core.undo_depth();
    invoke(
        &mut core,
        "export_bundle",
        &json!({ "path": one_dir.to_string_lossy(), "kind": "images", "scale": 1.0 }),
    )
    .expect("an explicit-1.0 bundle succeeds");
    assert_eq!(core.undo_depth(), before, "no undo step");
    assert_eq!(
        bundle_tree(&absent_dir),
        bundle_tree(&one_dir),
        "an explicit 1.0 writes the absent-scale bundle, byte for byte"
    );
}

#[test]
fn bundle_invalid_scales_refuse_before_any_write_and_spend_nothing() {
    let dir = test_dir("bundle_refused");
    let mut core = bundle_core();
    // Zero, negative and f32-overflowing scales, for BOTH
    // kinds (the validation precedes the member split):
    // the whole call refuses — in check_export_scale's
    // sentence, carrying the f32 the member would have
    // received — and the target directory is never even
    // created. No partial bundle, no undo step.
    for kind in ["images", "video"] {
        for (scale, got) in [(json!(0), "0"), (json!(-2.5), "-2.5"), (json!(1e300), "inf")] {
            let target = dir.join(format!("out-{kind}-{got}"));
            let before = core.undo_depth();
            let err = invoke(
                &mut core,
                "export_bundle",
                &json!({
                    "path": target.to_string_lossy(), "kind": kind, "scale": scale
                }),
            )
            .expect_err("an invalid scale refuses the whole call");
            assert_eq!(
                err,
                format!("export scale must be a positive finite number, got {got}"),
                "{kind} scale {scale}"
            );
            assert_eq!(
                core.undo_depth(),
                before,
                "a refused bundle spends no undo step"
            );
            assert!(
                !target.exists(),
                "{kind} scale {scale}: a refused scale left a bundle behind"
            );
        }
    }
}

#[test]
fn bundle_string_scale_refuses_in_the_helpers_voice_and_spends_nothing() {
    let dir = test_dir("bundle_string_scale");
    let target = dir.join("out");
    let mut core = bundle_core();
    let before = core.undo_depth();
    let err = invoke(
        &mut core,
        "export_bundle",
        &json!({ "path": target.to_string_lossy(), "kind": "images", "scale": "2" }),
    )
    .expect_err("a non-number scale refuses the whole call (Wave T: it was silently ignored before)");
    // The MCP helpers' voice, verbatim — the handler
    // returns this refusal unwrapped.
    assert_eq!(err, "argument 'scale' must be a number");
    assert_eq!(
        core.undo_depth(),
        before,
        "a refused bundle spends no undo step"
    );
    assert!(
        !target.exists(),
        "a refused scale left a bundle behind"
    );
}
