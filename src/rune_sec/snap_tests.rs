//! Runtime interaction tests for the Rune editor shell
//! (Phase D, section 36 — the preservation inventory made
//! executable).
//!
//! These tests drive the REAL shell — `crate::view`, the
//! AppSpec `poll`/`on_key` hooks and `crate::init` — through
//! rune's in-process `Runtime` (docs/RUNE_API_AUDIT.md Q8): no
//! window, no GPU, a deterministic virtual clock. Pointer input
//! is queued by `pointer_down`/`pointer_move`/`pointer_up` and
//! lands on the next `rt.frame(..)`, exactly as the worked test
//! in rune's `examples/hello` does it.
//!
//! This file is test-only. The integrator declares it from
//! `main.rs` (see the section report for the exact line); it is
//! never compiled into the shipped binary.
//!
//! API notes vs the audit (verified against the pinned rune
//! source, `crates/rune/src/runtime*`):
//! * `Runtime::new(state, view, viewport, env)` leaves the
//!   `poll` and `on_key` fields `None`; both are `pub`, and the
//!   tests set them to the app's real hooks so playback runs
//!   through the same code path as the windowed app.
//! * `Runtime::presented(key)` returns
//!   `Option<([f32; 4], f32)>` — drawn rect `[x, y, w, h]` in
//!   window points plus effective opacity.
//! * `Runtime::find(key)` returns the element's drawn centre,
//!   which is what the taps below aim at.

use std::collections::BTreeSet;

use rune::{Env, Runtime};

use crate::stage_view::{to_screen, to_stage, view_transform};
use crate::{init, on_key, poll, view, State};

/// The AppSpec's window size, so layout matches the real app.
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
/// One 60 fps step of the virtual clock.
const DT: f64 = 1.0 / 60.0;

/// A runtime holding a fresh editor (the demo project), with
/// the app's real `poll`/`on_key` hooks installed and two
/// frames already run so layout — and the stage's
/// `layout_size` read — has settled.
fn editor() -> Runtime<State> {
    let mut rt = Runtime::new(
        init(),
        view,
        VIEWPORT,
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    rt.frame(0.0);
    rt.frame(DT);
    rt
}

/// Press-and-release the centre of the element keyed `key`,
/// then run one frame so the queued press handler lands.
fn tap(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(DT);
}

/// The set of element keys the last frame presented.
fn presented_keys(rt: &Runtime<State>) -> BTreeSet<String> {
    rt.elements()
        .iter()
        .filter_map(|e| e.key.clone())
        .collect()
}

#[test]
fn shell_mounts_stage_chrome_and_timeline() {
    let rt = editor();
    let keys = presented_keys(&rt);
    for want in [
        // The stage canvas and the chrome rows.
        "stage",
        "app-bar",
        "transport",
        "timeline",
        // App bar controls.
        "save",
        "load",
        "undo",
        "redo",
        "add-figure",
        "onion",
        // Transport + frame ops.
        "prev",
        "play",
        "next",
        "add-frame",
        "dup-frame",
        "del-frame",
        // One chip per demo frame (the demo project has five).
        "frame-1",
        "frame-2",
        "frame-3",
        "frame-4",
        "frame-5",
    ] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }
    assert!(
        rt.find("stage").is_some(),
        "stage has a findable centre"
    );
    let (rect, _opacity) = rt.presented("stage").expect("stage is presented");
    assert!(
        rect[2] > 100.0 && rect[3] > 100.0,
        "stage has a real size: {rect:?}"
    );
    assert!(
        rect[0] >= 0.0 && rect[1] >= 0.0 && rect[0] + rect[2] <= VIEWPORT.0 && rect[1] + rect[3] <= VIEWPORT.1,
        "stage lies inside the viewport: {rect:?}"
    );
    // State: a fresh editor shows the demo project's first of
    // five frames, and the status line says so.
    assert_eq!(rt.state.core.frame_count(), 5);
    assert_eq!(rt.state.core.current, 0);
    assert!(
        rt.elements().iter().any(|e| e
            .text
            .as_deref()
            .is_some_and(|t| t.starts_with("Frame 1/5"))),
        "status line reads Frame 1/5"
    );
}

#[test]
fn add_frame_button_and_chip_navigation() {
    let mut rt = editor();
    assert_eq!(rt.state.core.frame_count(), 5);

    tap(&mut rt, "add-frame");
    // `add_frame` inserts after the current frame and steps
    // onto the new one (editor.rs op semantics).
    assert_eq!(rt.state.core.frame_count(), 6);
    assert_eq!(rt.state.core.current, 1);
    assert!(
        presented_keys(&rt).contains("frame-6"),
        "a sixth chip appears"
    );

    // A chip press goes to that frame (`goto`).
    tap(&mut rt, "frame-3");
    assert_eq!(rt.state.core.current, 2);

    // The Next transport button steps one frame forward.
    tap(&mut rt, "next");
    assert_eq!(rt.state.core.current, 3);
}

#[test]
fn play_advances_the_playhead_on_the_virtual_clock() {
    let mut rt = editor();
    assert!(!rt.state.core.playing);
    assert_eq!(rt.state.core.current, 0);

    tap(&mut rt, "play");
    assert!(rt.state.core.playing, "Play flips core.playing");

    // A quarter second of virtual time at the demo's 12 fps is
    // three frames of playhead. `poll` measures from the
    // runtime clock and `advance_play` honours fps, so the
    // playhead must land near 3.0 and the current frame must
    // have left frame 0 — deterministically, with no wall
    // clock involved.
    for _ in 0..15 {
        rt.frame(DT);
    }
    let playhead = rt.state.core.playhead;
    assert!(
        playhead > 2.0,
        "playhead advanced with the virtual clock: {playhead}"
    );
    assert!(
        rt.state.core.current >= 2,
        "current frame followed the playhead: {}",
        rt.state.core.current
    );
    assert!(rt.state.core.playing, "still playing (no end stop)");

    // Pausing freezes the playhead exactly: more frames, no
    // movement.
    tap(&mut rt, "play");
    assert!(!rt.state.core.playing);
    let frozen = rt.state.core.playhead;
    for _ in 0..10 {
        rt.frame(DT);
    }
    assert_eq!(rt.state.core.playhead, frozen, "paused playhead is frozen");
}

#[test]
fn stage_drag_grabs_the_root_joint_and_moves_the_pose() {
    let mut rt = editor();
    let (rect, _opacity) = rt.presented("stage").expect("stage is presented");
    let [sx, sy, sw, sh] = rect;
    assert!(sw > 100.0 && sh > 100.0, "stage has real size: {rect:?}");

    // The demo figure's root joint (node 0), in stage
    // coordinates, and where the stage transform draws it.
    // The root hit-tests to itself by construction (distance
    // zero), which pins the press point to a real joint
    // without depending on the demo pose's exact numbers.
    let core = &rt.state.core;
    let frame = core.current;
    let root = core.positions(frame, 0)[0];
    assert_eq!(
        core.hit_test(root),
        Some((0, 0)),
        "root joint hit-tests to itself"
    );
    let vt = view_transform([sw, sh], core.project.stage, core.camera_here());
    let local = to_screen(&vt, root);
    let down = (sx + local[0], sy + local[1]);

    // Press: `begin_drag` via the canvas `on_drag` Phase::Down
    // selects the figure and the joint it grabbed.
    rt.pointer_down(down.0, down.1);
    rt.frame(DT);
    assert_eq!(rt.state.core.selected_figure, 0);
    assert_eq!(
        rt.state.core.selected_node,
        Some(0),
        "the press grabbed the root joint"
    );

    // Move: a root drag sets the pose root to the pointer's
    // stage point, so the joint must track the pointer through
    // the same transform the drawing uses.
    let delta = [48.0f32, 30.0];
    rt.pointer_move(down.0 + delta[0], down.1 + delta[1]);
    rt.frame(DT);
    let moved = rt.state.core.positions(frame, 0)[0];
    let expected = to_stage(&vt, [local[0] + delta[0], local[1] + delta[1]]);
    assert!(
        (moved[0] - root[0]).abs() > 20.0 || (moved[1] - root[1]).abs() > 10.0,
        "the root actually moved: {root:?} -> {moved:?}"
    );
    assert!(
        (moved[0] - expected[0]).abs() < 2.0 && (moved[1] - expected[1]).abs() < 2.0,
        "the root tracks the pointer: expected {expected:?}, got {moved:?}"
    );

    // Release: the pose stays where the drag left it.
    rt.pointer_up(down.0 + delta[0], down.1 + delta[1]);
    rt.frame(DT);
    let settled = rt.state.core.positions(frame, 0)[0];
    assert!(
        (settled[0] - moved[0]).abs() < 0.01 && (settled[1] - moved[1]).abs() < 0.01,
        "the pose stays where the drag left it: {moved:?} -> {settled:?}"
    );
}
