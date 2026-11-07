//! Integration tests: export determinism at the crate boundary.
//!
//! The port's selling point is "same input, same bytes": exports
//! are closed-form and deterministic. These tests pin that claim
//! for every exporter whose bytes this crate fully controls:
//!
//! - raw frame pixels (`export::render_frame_at`),
//! - SVG documents (`svg::export_svg`),
//! - GIF (`export::export_gif`, encoded in-crate via `image`),
//! - spritesheet PNG (`export::export_spritesheet`, in-crate).
//!
//! MP4 and WebM are deliberately NOT asserted here: they are
//! encoded by the system `ffmpeg` binary, so their bytes depend
//! on the local ffmpeg build and its encoder defaults, not on
//! this crate. Byte-equality for them could fail on a machine
//! whose renderer is perfectly deterministic.
//!
//! Everything below uses only the public API (`Project::new_demo`
//! plus pub fields) and `std` — no dev-dependencies.

use std::path::PathBuf;

use stykanimate_rs::export;
use stykanimate_rs::model::Project;
use stykanimate_rs::svg;

/// The demo project cut down to 3 authored frames so CI stays
/// fast. Stage size is left at the demo's 960x720.
fn tiny_project() -> Project {
    let mut project = Project::new_demo();
    project.frames.truncate(3);
    assert_eq!(project.frames.len(), 3);
    project
}

/// A per-process, per-test scratch directory under the OS temp
/// dir. Removed at the start (stale leftovers from a crashed run)
/// and the caller removes it again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-export-determinism-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn frame_pixels_are_byte_identical_across_renders() {
    let project = tiny_project();
    // render_frame_at supersamples 2x, so the RGB buffer is
    // (stage * 2)^2 * 3 bytes.
    let expected_len = (project.stage[0] as usize * 2) * (project.stage[1] as usize * 2) * 3;

    for frame_f in [0.0f32, 1.0, 2.0] {
        let a = export::render_frame_at(&project, frame_f);
        let b = export::render_frame_at(&project, frame_f);
        assert_eq!(
            a.as_raw().len(),
            expected_len,
            "frame {frame_f}: unexpected buffer size"
        );
        assert_eq!(
            a.as_raw(),
            b.as_raw(),
            "frame {frame_f}: two renders of the same project differ"
        );
    }

    // Non-vacuity: different frames of the wave animation must
    // actually differ, so the equality above is not comparing two
    // blank buffers.
    let first = export::render_frame_at(&project, 0.0);
    let third = export::render_frame_at(&project, 2.0);
    assert_ne!(
        first.as_raw(),
        third.as_raw(),
        "frames 0 and 2 render identically — the demo animation is not reaching the rasterizer"
    );
}

#[test]
fn svg_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir_a = temp_dir("svg-a");
    let dir_b = temp_dir("svg-b");

    let n_a = svg::export_svg(&project, &dir_a).expect("svg export A");
    let n_b = svg::export_svg(&project, &dir_b).expect("svg export B");
    assert_eq!(n_a, 3, "svg export should write one file per frame");
    assert_eq!(n_a, n_b);

    for i in 1..=3 {
        let name = format!("frame-{i:04}.svg");
        let text_a = std::fs::read_to_string(dir_a.join(&name)).expect("read svg A");
        let text_b = std::fs::read_to_string(dir_b.join(&name)).expect("read svg B");
        assert_eq!(text_a, text_b, "{name}: two svg exports differ");
        if i == 1 {
            // Non-vacuity: a real vector document with figure lines.
            assert!(text_a.starts_with("<svg"), "not an svg document");
            assert!(text_a.contains("<line"), "figure segments missing");
        }
    }

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[test]
fn gif_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir = temp_dir("gif");
    let path_a = dir.join("a.gif");
    let path_b = dir.join("b.gif");

    let n_a = export::export_gif(&project, &path_a).expect("gif export A");
    let n_b = export::export_gif(&project, &path_b).expect("gif export B");
    assert_eq!(n_a, 3, "gif should hold the 3 authored frames");
    assert_eq!(n_a, n_b);

    let bytes_a = std::fs::read(&path_a).expect("read gif A");
    let bytes_b = std::fs::read(&path_b).expect("read gif B");
    assert!(bytes_a.len() > 6, "gif suspiciously small");
    assert!(bytes_a.starts_with(b"GIF"), "missing GIF magic");
    assert_eq!(bytes_a, bytes_b, "two gif exports differ");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spritesheet_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir = temp_dir("sheet");
    let path_a = dir.join("a.png");
    let path_b = dir.join("b.png");

    let n_a = export::export_spritesheet(&project, &path_a).expect("spritesheet export A");
    let n_b = export::export_spritesheet(&project, &path_b).expect("spritesheet export B");
    assert_eq!(n_a, 3, "spritesheet should hold the 3 authored frames");
    assert_eq!(n_a, n_b);

    let bytes_a = std::fs::read(&path_a).expect("read spritesheet A");
    let bytes_b = std::fs::read(&path_b).expect("read spritesheet B");
    assert!(bytes_a.len() > 8, "spritesheet suspiciously small");
    assert!(bytes_a.starts_with(b"\x89PNG"), "missing PNG magic");
    assert_eq!(bytes_a, bytes_b, "two spritesheet exports differ");

    let _ = std::fs::remove_dir_all(&dir);
}
