//! Wave W (W8) — the two export progress contracts,
//! composed end to end: Wave U's encode contract (U11)
//! and Wave V's render contract (V12), exercised here
//! together for the first time, against each other.
//!
//! What was already proven, and where:
//!
//! * U11's pins in `src/export.rs` prove the encode
//!   contract per funnel: a hooked MP4 range encode and
//!   a hooked WebM scaled encode each deliver the
//!   terminal event under `assert_encode_progress_contract`,
//!   and a failed encode reports no terminal.
//! * V12's pins in the same module prove the render
//!   contract per sequence entry: the exact event stream
//!   for ranged, full, scaled and scale-1.0-routed
//!   PNG/JPEG exports.
//!
//! What no leg had done is hold the two contracts side
//! by side on one project and assert their counts can
//! never be the same number by accident: the render
//! phase counts AUTHORED frames (a sequence writes one
//! file per authored frame), the encode phase counts
//! the SAMPLED frame times handed to ffmpeg
//! (duration-expanded — `export::expanded_frames`), so
//! on a project with a held frame the two totals must
//! differ by exactly the hold. That divergence — and
//! each contract keeping its own shape while it
//! happens — is this file.
//!
//! Registry reachability, stated plainly: the registry
//! `render` tool (src/mcp_io.rs) routes format / range
//! / scale to the PLAIN entries only; its argument
//! surface carries no callback, by the boarded design
//! of IA-U11-1 / IA-V12-1 (MCP is deliberately not
//! wired — its handler type has no notification sink,
//! and a post-hoc reply field would be fake progress).
//! No registry call can observe either contract, so
//! the legs run against the core entries the registry
//! itself dispatches to. If a future transport slice
//! wires MCP, these legs are the shape it must keep.
//!
//! The encode legs assume `ffmpeg` on PATH with
//! libx264 — the same assumption the in-crate hooked
//! pins make (they unwrap the same entries; CI
//! installs ffmpeg). The fixture is the in-crate pins'
//! tiny shape: the demo project at 24 fps on a 96x72
//! stage, so encodes stay small. fps is set explicitly
//! on every fixture — the demo ships at 12 fps and the
//! rate is part of the fixture, never inherited.
//! Intermediate encode event COUNTS are not pinned
//! anywhere in this file: how many advances ffmpeg
//! reports is wall-clock business (U11's discipline).
//! The render stream, delivered synchronously, is
//! pinned exactly. Files live in per-test temp dirs;
//! nothing touches a CWD-relative store, so no serial
//! guard is needed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use stykanimate_rs::export::{
    export_mp4_range_with_encode_progress, export_mp4_with_encode_progress,
    export_jpeg_sequence_range_scaled_with_render_progress,
    export_png_sequence_range_scaled_with_render_progress,
    export_png_sequence_with_render_progress, EncodeProgressEvent, RenderProgressEvent,
};
use stykanimate_rs::model::Project;

// ---------------------------------------------------------------------
// Fixtures and sinks
// ---------------------------------------------------------------------

/// The progress fixture: the demo project (5 authored
/// frames, duration 1.0 each) at 24 fps on the in-crate
/// pins' 96x72 tiny stage.
fn progress_project() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = [96.0, 72.0];
    project
}

/// The held-frame variant: frame 2 is held twice as
/// long (`duration` 2.0). Authored frames stay 5, but
/// the encoder's sampled times expand to 6
/// (`expanded_frames` repeats each frame
/// `duration.round()` times) — the one-frame wedge
/// leg 3 drives between the two contracts' totals.
fn held_project() -> Project {
    let mut project = progress_project();
    project.frames[2].duration = 2.0;
    project
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-w-progress-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn render_sink() -> (
    Arc<Mutex<Vec<RenderProgressEvent>>>,
    impl Fn(RenderProgressEvent),
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let callback = move |event: RenderProgressEvent| {
        sink.lock().unwrap().push(event);
    };
    (events, callback)
}

fn encode_sink() -> (
    Arc<Mutex<Vec<EncodeProgressEvent>>>,
    impl Fn(EncodeProgressEvent),
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let callback = move |event: EncodeProgressEvent| {
        sink.lock().unwrap().push(event);
    };
    (events, callback)
}

// ---------------------------------------------------------------------
// The two contracts, restated as assertions
// ---------------------------------------------------------------------

/// The exact event stream a successful hooked sequence
/// export of `total` frames must deliver (V12): one
/// event per written frame, unfinished until the final
/// frame's event, which is the single terminal at
/// (total, total). Synchronous delivery pins the FULL
/// stream — the deliberate contrast with the encode
/// contract below.
fn expected_render_events(total: usize) -> Vec<RenderProgressEvent> {
    (1..=total)
        .map(|done| RenderProgressEvent {
            rendered_frames: done,
            total_frames: total,
            finished: done == total,
        })
        .collect()
}

/// The delivery shape every hooked encode must keep
/// (U11): exactly one terminal event, last, at
/// (total, total); intermediates unfinished, strictly
/// increasing, within 1..=total, all carrying the same
/// total. How many intermediates arrive is ffmpeg's
/// wall-clock business and is NOT pinned.
fn assert_encode_contract(events: &[EncodeProgressEvent], total: usize) {
    let (terminal, intermediate) = events
        .split_last()
        .expect("a successful hooked encode reports at least its terminal event");
    assert_eq!(
        *terminal,
        EncodeProgressEvent {
            encoded_frames: total,
            total_frames: total,
            finished: true,
        },
        "the terminal event closes the encode at (total, total)"
    );
    assert_eq!(
        events.iter().filter(|e| e.finished).count(),
        1,
        "exactly one terminal event"
    );
    let mut last = 0usize;
    for event in intermediate {
        assert!(!event.finished, "no early terminal event: {event:?}");
        assert_eq!(event.total_frames, total);
        assert!(
            event.encoded_frames > last && event.encoded_frames <= total,
            "intermediate events strictly increase within the total: {event:?}"
        );
        last = event.encoded_frames;
    }
}

// ---------------------------------------------------------------------
// Leg 1 — the render contract's exact stream through a
// scaled + ranged sequence export
// ---------------------------------------------------------------------

#[test]
fn render_stream_is_exact_through_scaled_ranged_sequence_exports() {
    let project = progress_project();

    // PNG, frames 1..=3 at 2.0: scaled AND ranged, the
    // combination leg 1 names. Files keep their
    // authored indices.
    let (events, callback) = render_sink();
    let dir = temp_dir("png-scaled-range");
    let n = export_png_sequence_range_scaled_with_render_progress(
        &project,
        1,
        3,
        &dir,
        2.0,
        Some(&callback),
    )
    .unwrap();
    assert_eq!(n, 3);
    assert_eq!(*events.lock().unwrap(), expected_render_events(3));
    for name in ["frame-0002.png", "frame-0003.png", "frame-0004.png"] {
        assert!(dir.join(name).is_file(), "missing {name}");
    }
    let _ = std::fs::remove_dir_all(&dir);

    // JPEG, frames 2..=4 at 0.5: the other sequence
    // format through the same combined shape, scaling
    // down instead of up — the stream is authored
    // counts in both directions.
    let (events, callback) = render_sink();
    let dir = temp_dir("jpeg-scaled-range");
    let n = export_jpeg_sequence_range_scaled_with_render_progress(
        &project,
        2,
        4,
        &dir,
        0.5,
        Some(&callback),
    )
    .unwrap();
    assert_eq!(n, 3);
    assert_eq!(*events.lock().unwrap(), expected_render_events(3));
    for name in ["frame-0003.jpg", "frame-0004.jpg", "frame-0005.jpg"] {
        assert!(dir.join(name).is_file(), "missing {name}");
    }
    let _ = std::fs::remove_dir_all(&dir);

    // The same combined entry at exactly 1.0: the
    // stream survives the scale boundary unchanged
    // (the in-crate pins cover the JPEG route-through;
    // this is the ranged PNG shape at the boundary).
    let (events, callback) = render_sink();
    let dir = temp_dir("png-one-range");
    let n = export_png_sequence_range_scaled_with_render_progress(
        &project,
        0,
        1,
        &dir,
        1.0,
        Some(&callback),
    )
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(*events.lock().unwrap(), expected_render_events(2));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — the encode contract's delivery shape through
// a hooked core entry
// ---------------------------------------------------------------------

#[test]
fn encode_contract_holds_through_a_hooked_mp4_range_export() {
    // Frames 1..=3 of the 24 fps fixture sample to
    // exactly 3 encoder frames (durations 1.0, tween
    // playback off) — the same derivation the in-crate
    // U11 pin uses, asserted here at integration level
    // with the contract restated in full.
    let project = progress_project();
    let (events, callback) = encode_sink();
    let dir = temp_dir("mp4-range");
    let path = dir.join("range.mp4");
    let outcome =
        export_mp4_range_with_encode_progress(&project, 1, 3, &path, Some(&callback)).unwrap();
    assert_eq!(outcome.frames, 3);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");
    assert_encode_contract(&events.lock().unwrap(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 3 — phase separation: the two contracts' counts
// are different numbers on the same project
// ---------------------------------------------------------------------

#[test]
fn phases_never_share_counts_on_a_held_frame_project() {
    // The held project: 5 authored frames, 6 sampled
    // encoder times (frame 2 held x2). Every assertion
    // below follows from that one-frame wedge.
    let project = held_project();
    assert_eq!(project.frames.len(), 5, "fixture: 5 authored frames");

    // The sequence side reports the render phase only:
    // the full-timeline stream totals 5 — the authored
    // count — and every event carries it.
    let (events, callback) = render_sink();
    let dir = temp_dir("phase-render");
    let n = export_png_sequence_with_render_progress(&project, &dir, Some(&callback)).unwrap();
    assert_eq!(n, 5, "a sequence writes one file per authored frame");
    assert_eq!(*events.lock().unwrap(), expected_render_events(5));
    let _ = std::fs::remove_dir_all(&dir);

    // The video side reports the encode phase only:
    // the terminal closes at 6 — the sampled count —
    // and no event in the stream carries the render
    // phase's 5.
    let (events, callback) = encode_sink();
    let dir = temp_dir("phase-encode");
    let path = dir.join("full.mp4");
    let outcome = export_mp4_with_encode_progress(&project, &path, Some(&callback)).unwrap();
    assert_eq!(outcome.frames, 6, "the encoder was handed 6 sampled times");
    let events = events.lock().unwrap();
    assert_encode_contract(&events, 6);
    assert!(
        events.iter().all(|e| e.total_frames == 6),
        "no encode event may carry the render count: {events:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    // The same wedge inside one range, 1..=3: render
    // totals 3 authored frames; the encode of the same
    // frames samples the held frame twice and totals
    // 4. Same frames, two phases, two numbers — by
    // construction, not by coincidence of totals.
    let (events, callback) = render_sink();
    let dir = temp_dir("phase-render-range");
    let n = export_png_sequence_range_scaled_with_render_progress(
        &project,
        1,
        3,
        &dir,
        2.0,
        Some(&callback),
    )
    .unwrap();
    assert_eq!(n, 3);
    assert_eq!(*events.lock().unwrap(), expected_render_events(3));
    let _ = std::fs::remove_dir_all(&dir);

    let (events, callback) = encode_sink();
    let dir = temp_dir("phase-encode-range");
    let path = dir.join("range.mp4");
    let outcome =
        export_mp4_range_with_encode_progress(&project, 1, 3, &path, Some(&callback)).unwrap();
    assert_eq!(outcome.frames, 4, "frames 1, 2, 2, 3 reach the encoder");
    assert_encode_contract(&events.lock().unwrap(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}
