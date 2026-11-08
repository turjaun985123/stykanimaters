//! Integration tests: export determinism at the crate boundary.
//!
//! The port's selling point is "same input, same bytes": exports
//! are closed-form and deterministic. These tests pin that claim
//! for every exporter whose bytes this crate fully controls:
//!
//! - raw frame pixels (`export::render_frame_at`),
//! - SVG documents (`svg::export_svg`),
//! - GIF (`export::export_gif`, encoded in-crate via `image`),
//! - spritesheet PNG (`export::export_spritesheet`, in-crate),
//! - PNG sequence (`export::export_png_sequence`, in-crate via
//!   `image`),
//! - contact sheet (`export::render_contact_sheet`, in-memory
//!   pixels),
//! - APNG (`export::export_apng`, in-crate via the `png` crate),
//! - JPEG sequence (`export::export_jpeg_sequence`, in-crate via
//!   `image`),
//! - rich-project frame renders: the same `render_frame_at`
//!   determinism, but on a project exercising the newer
//!   closed-form systems (Storm weather, an FX clip, a spray
//!   stroke), with non-vacuity checks that those systems
//!   actually reach the rasterizer.
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
use stykanimate_rs::model::{FxClipEntry, Project, SprayMark, Stroke};
use stykanimate_rs::svg;
use stykanimate_rs::vfx::{ClipKind, ParticleClip, Weather, WeatherKind};

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
    // render_frame_at supersamples 2x internally but returns the
    // downsampled stage-size frame, so the RGB buffer is
    // stage_w * stage_h * 3 bytes.
    let expected_len = project.stage[0] as usize * project.stage[1] as usize * 3;

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

#[test]
fn png_sequence_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir_a = temp_dir("png-a");
    let dir_b = temp_dir("png-b");

    let n_a = export::export_png_sequence(&project, &dir_a).expect("png sequence A");
    let n_b = export::export_png_sequence(&project, &dir_b).expect("png sequence B");
    assert_eq!(n_a, 3, "png sequence should write one file per frame");
    assert_eq!(n_a, n_b);

    // The file SETS must match: the same names in both directories.
    let mut names_a: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir_a).expect("list png dir A") {
        let entry = entry.expect("png dir A entry");
        names_a.push(entry.file_name().to_string_lossy().into_owned());
    }
    let mut names_b: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir_b).expect("list png dir B") {
        let entry = entry.expect("png dir B entry");
        names_b.push(entry.file_name().to_string_lossy().into_owned());
    }
    names_a.sort();
    names_b.sort();
    assert_eq!(names_a.len(), 3, "png sequence should hold 3 files");
    assert_eq!(names_a, names_b, "png sequence file sets differ");

    // And every file's bytes must match, with the PNG magic as a
    // non-vacuity check that these are real PNG documents.
    for name in &names_a {
        let bytes_a = std::fs::read(dir_a.join(name)).expect("read png A");
        let bytes_b = std::fs::read(dir_b.join(name)).expect("read png B");
        assert!(bytes_a.len() > 8, "{name}: png suspiciously small");
        assert!(bytes_a.starts_with(b"\x89PNG"), "{name}: missing PNG magic");
        assert_eq!(bytes_a, bytes_b, "{name}: two png sequence exports differ");
    }

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[test]
fn contact_sheet_is_byte_identical_across_renders() {
    let project = tiny_project();

    let a = export::render_contact_sheet(&project, &[0, 1, 2], 2, 120);
    let b = export::render_contact_sheet(&project, &[0, 1, 2], 2, 120);
    assert_eq!(a.dimensions(), b.dimensions());
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "two contact sheets of the same frames differ"
    );

    // Non-vacuity: frames 0 and 2 of the wave animation are
    // different poses, so single-frame sheets of each (same grid
    // shape) must differ in pixels, not just in size.
    let first = export::render_contact_sheet(&project, &[0], 1, 120);
    let third = export::render_contact_sheet(&project, &[2], 1, 120);
    assert_eq!(first.dimensions(), third.dimensions());
    assert_ne!(
        first.as_raw(),
        third.as_raw(),
        "contact sheets of frames 0 and 2 are identical — the demo animation is not reaching the contact sheet"
    );
}

#[test]
fn apng_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir = temp_dir("apng");
    let path_a = dir.join("a.png");
    let path_b = dir.join("b.png");

    let n_a = export::export_apng(&project, &path_a).expect("apng export A");
    let n_b = export::export_apng(&project, &path_b).expect("apng export B");
    assert_eq!(n_a, 3, "apng should hold the 3 authored frames");
    assert_eq!(n_a, n_b);

    let bytes_a = std::fs::read(&path_a).expect("read apng A");
    let bytes_b = std::fs::read(&path_b).expect("read apng B");
    assert!(bytes_a.len() > 1000, "apng suspiciously small");
    assert!(
        bytes_a.starts_with(b"\x89PNG\r\n\x1a\n"),
        "missing PNG signature"
    );
    assert_eq!(bytes_a, bytes_b, "two apng exports differ");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jpeg_sequence_export_is_byte_identical_across_runs() {
    let project = tiny_project();
    let dir_a = temp_dir("jpeg-a");
    let dir_b = temp_dir("jpeg-b");

    let n_a = export::export_jpeg_sequence(&project, &dir_a).expect("jpeg sequence A");
    let n_b = export::export_jpeg_sequence(&project, &dir_b).expect("jpeg sequence B");
    assert_eq!(n_a, 3, "jpeg sequence should write one file per frame");
    assert_eq!(n_a, n_b);

    // The file SETS must match: the same names in both directories.
    let mut names_a: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir_a).expect("list jpeg dir A") {
        let entry = entry.expect("jpeg dir A entry");
        names_a.push(entry.file_name().to_string_lossy().into_owned());
    }
    let mut names_b: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir_b).expect("list jpeg dir B") {
        let entry = entry.expect("jpeg dir B entry");
        names_b.push(entry.file_name().to_string_lossy().into_owned());
    }
    names_a.sort();
    names_b.sort();
    assert_eq!(names_a.len(), 3, "jpeg sequence should hold 3 files");
    assert_eq!(names_a, names_b, "jpeg sequence file sets differ");

    // And every file's bytes must match, with the JPEG SOI marker
    // as a non-vacuity check that these are real JPEG documents.
    for name in &names_a {
        let bytes_a = std::fs::read(dir_a.join(name)).expect("read jpeg A");
        let bytes_b = std::fs::read(dir_b.join(name)).expect("read jpeg B");
        assert!(bytes_a.len() > 1000, "{name}: jpeg suspiciously small");
        assert_eq!(&bytes_a[0..2], &[0xFF, 0xD8], "{name}: missing JPEG SOI marker");
        assert_eq!(bytes_a, bytes_b, "{name}: two jpeg sequence exports differ");
    }

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

/// The tiny project dressed with two of the newer closed-form
/// systems: Storm weather and a Shard FX clip anchored to the
/// demo figure's origin node. The clip window is frames
/// [0.0, 2.0), so frame 1.0 sits at its midpoint (t = 0.5).
fn rich_project() -> Project {
    let mut project = tiny_project();
    project.weather = Some(Weather {
        kind: WeatherKind::Storm,
        amount: 120.0,
        wind: 25.0,
    });
    project.fx_clips.push(FxClipEntry {
        clip: ParticleClip {
            start: 0.0,
            duration: 2.0,
            kind: ClipKind::Shard,
            color: 0xFFCC33,
            seed: 7,
            angle: 0.0,
            length: 120.0,
            color2: None,
        },
        figure: 0,
        node: 0,
        hidden: false,
    });
    project
}

#[test]
fn rich_project_frames_are_byte_identical_across_renders() {
    let project = rich_project();
    let expected_len = project.stage[0] as usize * project.stage[1] as usize * 3;

    // Frame 1.0 is inside the FX clip window; frame 2.0 is past
    // it but still under Storm weather. Both must render
    // byte-identically across calls.
    for frame_f in [1.0f32, 2.0] {
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
            "frame {frame_f}: two renders of the storm + FX project differ"
        );
    }

    // Non-vacuity: the same project with weather and FX removed
    // must NOT produce the frame-1.0 pixels — the storm drops and
    // the shard burst have to reach the rasterizer, not just the
    // determinism check.
    let mut stripped = project.clone();
    stripped.weather = None;
    stripped.fx_clips.clear();
    let rich = export::render_frame_at(&project, 1.0);
    let plain = export::render_frame_at(&stripped, 1.0);
    assert_ne!(
        rich.as_raw(),
        plain.as_raw(),
        "frame 1.0 is unchanged by removing Storm weather and the Shard clip — weather/FX are not reaching the rasterizer"
    );
}

#[test]
fn spray_stroke_renders_deterministically_and_differs_from_plain() {
    // One horizontal stroke across the middle of the stage on
    // authored frame 0; the spray twin carries a SprayMark, so
    // its marks spread around the recorded path (seeded, hence
    // deterministic) instead of only tracing it.
    let plain_stroke = Stroke {
        frame: 0,
        points: vec![[200.0, 360.0], [760.0, 360.0]],
        color: 0x3366CC,
        width: 6.0,
        widths: Vec::new(),
        fill: None,
        spray: None,
        opacity: 1.0,
    };
    let spray_stroke = Stroke {
        spray: Some(SprayMark {
            seed: 42,
            radius: 18.0,
            opacity: 0.6,
        }),
        ..plain_stroke.clone()
    };

    let mut plain_project = tiny_project();
    plain_project.strokes.push(plain_stroke);
    let mut spray_project = tiny_project();
    spray_project.strokes.push(spray_stroke);

    let a = export::render_frame_at(&spray_project, 0.0);
    let b = export::render_frame_at(&spray_project, 0.0);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "two renders of the spray stroke differ — spray placement is not deterministic"
    );

    // Non-vacuity: the spray render must differ from the plain
    // stroke's render — the SprayMark has to change pixels, not
    // just be stored.
    let plain = export::render_frame_at(&plain_project, 0.0);
    assert_ne!(
        a.as_raw(),
        plain.as_raw(),
        "spray stroke renders identically to its spray: None twin — the SprayMark is not reaching the rasterizer"
    );
}
