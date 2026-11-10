//! Wave P (P47): export determinism pins for the Rune raster
//! path.
//!
//! The claim under pin: the rasterizer is a pure function of
//! (project, frame). Four legs, all through the exporter's
//! public frame render (`export::render_frame`, the integer-frame
//! entry the PNG sequence / GIF / spritesheet exporters funnel
//! through, plus one pass through `export::render_frame_at`):
//!
//! 1. REPEAT-RENDER — the same fixture rendered twice at the
//!    same frame is byte-identical: no leaked state, no time or
//!    random dependence between calls.
//! 2. A-B-A SENSITIVITY — render A, render a clone with ONE
//!    figure property changed (the figure's colour), render A
//!    again: the changed render MUST differ (the property
//!    reaches the rasterizer), and A2 MUST equal A1 byte for
//!    byte (rendering the changed project in between left no
//!    residue in the pipeline).
//! 2b. A-B-A SUBTITLE — the same pattern with the changed
//!    property inside a subtitle cue: one word of a cue live
//!    at frame 0 changes, and the render must change with it
//!    (P26's burn-in reaches the rasterizer), then restoring
//!    the word restores byte-identity. Added by Wave Q (Q20)
//!    once P23's subtitle core and P26's burn-in had landed
//!    — the follow-up p47.md named.
//! 3. INTERLEAVE — renders of other frames (including a
//!    fractional tween frame) between two renders of frame 0
//!    do not perturb the second frame-0 render: no per-frame
//!    cache or cursor leaks across calls.
//!
//! The subtitle leg (2b) was added by Wave Q (Q20): when
//! the first three legs were written, `src/subtitles.rs`
//! had not landed (P23 in flight), so leg 2 used a figure
//! property; p47.md named the cue leg as the follow-up
//! for once P23's core and P26's raster burn-in were both
//! in the tree. Both are, so the leg sits beside leg 2
//! below, unchanged in shape.
//!
//! Failure messages report the first differing byte index and
//! the differing-byte count rather than dumping whole buffers,
//! so a CI failure carries the measured difference with it.
//!
//! Everything uses only the public API (`Project::new_demo`
//! plus pub fields) and `std` — no dev-dependencies.

use stykanimate_rs::export;
use stykanimate_rs::model::Project;
use stykanimate_rs::subtitles::SubtitleCue;

/// The demo project cut down to 3 authored frames so CI stays
/// fast. Stage size is left at the demo's 960x720. fps is set
/// explicitly (board contract 3); it does not drive the raster
/// path, which evaluates by frame index.
fn fixture() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.frames.truncate(3);
    assert_eq!(project.frames.len(), 3);
    project
}

/// Render one integer frame and own the RGB bytes.
fn render(project: &Project, frame: usize) -> Vec<u8> {
    export::render_frame(project, frame).as_raw().clone()
}

/// Byte-identity with a compact measured difference on failure:
/// first differing index + how many bytes differ + both lengths.
fn assert_identical(a: &[u8], b: &[u8], context: &str) {
    if a == b {
        return;
    }
    let shared = a.len().min(b.len());
    let first_diff = (0..shared).find(|&i| a[i] != b[i]);
    let differing = (0..shared).filter(|&i| a[i] != b[i]).count()
        + a.len().abs_diff(b.len());
    panic!(
        "{context}: renders differ — first differing byte at {first_diff:?}, \
         {differing} of {} bytes differ (lengths {} vs {})",
        shared.max(a.len().max(b.len())),
        a.len(),
        b.len()
    );
}

#[test]
fn repeat_render_is_byte_identical() {
    let project = fixture();
    // render_frame supersamples 2x internally but returns the
    // downsampled stage-size frame: stage_w * stage_h * 3 bytes.
    let expected_len = project.stage[0] as usize * project.stage[1] as usize * 3;

    for frame in [0usize, 1, 2] {
        let a = render(&project, frame);
        let b = render(&project, frame);
        assert_eq!(
            a.len(),
            expected_len,
            "frame {frame}: unexpected buffer size"
        );
        assert_identical(&a, &b, &format!("frame {frame} rendered twice"));
    }

    // Non-vacuity: frames 0 and 2 are different poses of the
    // demo wave, so the equality above is not comparing two
    // blank buffers.
    let first = render(&project, 0);
    let third = render(&project, 2);
    assert_ne!(
        first, third,
        "frames 0 and 2 render identically — the demo animation is not reaching the rasterizer"
    );
}

#[test]
fn aba_one_figure_property_change_is_sensitive_and_leaves_no_residue() {
    let base = fixture();
    assert_eq!(base.figures.len(), 1, "fixture premise: one figure");
    assert_eq!(base.figures[0].color, 0xE8E8EC, "fixture premise: demo colour");

    let a1 = render(&base, 0);

    // ONE figure property changed: the figure's colour. The
    // rasterizer resolves segment colour as
    // node colour falling back to the instance colour
    // (export.rs), so every segment of the figure recolours.
    let mut changed = base.clone();
    changed.figures[0].color = 0xFF2A2A;
    let c = render(&changed, 0);
    assert_ne!(
        a1, c,
        "recolouring the only figure changed no pixels — the changed \
         project is not reaching the rasterizer"
    );

    // Back to A: rendering the changed project in between must
    // not have perturbed the pipeline in any way.
    let a2 = render(&base, 0);
    assert_identical(&a1, &a2, "A-B-A: frame 0 before vs after the changed render");
}

#[test]
fn aba_one_word_cue_change_is_sensitive_and_leaves_no_residue() {
    // Wave Q (Q20): the subtitle leg p47.md named as its
    // follow-up, written once P23's subtitle core and P26's
    // raster burn-in were both in the tree. One cue live
    // across the fixture's whole span, under the demo's
    // initialised style (the plain default look: bottom,
    // unscaled, white on black).
    let mut base = fixture();
    base.subtitles.push(SubtitleCue {
        start_frame: 0,
        end_frame: 3,
        text: "Hello world".to_string(),
    });
    assert_eq!(base.subtitles.len(), 1, "fixture premise: one cue");

    let a1 = render(&base, 0);

    // Non-vacuity: the cue itself reaches the rasterizer —
    // the same frame without the cue renders differently.
    let bare = render(&fixture(), 0);
    assert_ne!(
        a1, bare,
        "a live cue changed no pixels — the subtitle burn-in is not \
         reaching the rasterizer"
    );

    // ONE word of the cue's text changed. The burn-in
    // stamps the cue's glyphs at the tail of
    // `render_frame_at` (export.rs), so the rendered band
    // must move with the word.
    let mut changed = base.clone();
    changed.subtitles[0].text = "Hello there".to_string();
    let c = render(&changed, 0);
    assert_ne!(
        a1, c,
        "changing one word of a live cue changed no pixels — the cue \
         text is not reaching the rasterizer"
    );

    // Back to A: rendering the changed-cue project in
    // between must not have perturbed the pipeline.
    let a2 = render(&base, 0);
    assert_identical(
        &a1,
        &a2,
        "subtitle A-B-A: frame 0 before vs after the changed-cue render",
    );
}

#[test]
fn interleaved_renders_do_not_leak_state() {
    let project = fixture();

    let a1 = render(&project, 0);
    // Interleave: a later integer frame and a fractional tween
    // frame through the other public entry point.
    let _other = render(&project, 2);
    let _tween = export::render_frame_at(&project, 0.5);
    let a2 = render(&project, 0);

    assert_identical(
        &a1,
        &a2,
        "frame 0 before vs after interleaved frame-2 and tween-0.5 renders",
    );
}
