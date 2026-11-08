//! StykAnimate RS — Rune editor shell (Phase C of the
//! GPUI→Rune migration).
//!
//! A working editor on the pinned Rune framework: the stage is
//! one canvas element whose `Shape` is built from the solved
//! rig each frame (`stage_view`), pointer drags pose through
//! the real `EditorCore` ops, the timeline/transport/app bar
//! live in `chrome_view`, playback runs on `AppSpec::poll` +
//! `Tx::wake_after`, and shortcuts on `AppSpec::on_key`.
//! Everything delegates to `stykanimate_rs::editor::EditorCore`
//! — the engine is untouched.

mod chrome_view;
mod stage_view;

use std::time::Duration;

use rune::tok::{self, type_};
use rune::{canvas, col, text, AppSpec, Cx, Drag, El, Key, KeyEvent, Phase, Tx, WheelEvent};
use stykanimate_rs::editor::EditorCore;

use stage_view::{stage_shape, to_stage, view_transform};

pub struct State {
    pub core: EditorCore,
    /// Last `poll` timestamp (seconds since start), for the
    /// playback delta. View never reads it.
    last_poll: Option<f64>,
    /// Sub-step wheel-zoom accumulation: `camera_zoom_by`
    /// snapshots once per call, so precise (trackpad) deltas are
    /// gathered here until they make a full 1.1× step instead of
    /// flooding the undo stack with micro-zooms.
    wheel_zoom: f32,
}

fn init() -> State {
    State {
        core: EditorCore::new(),
        last_poll: None,
        wheel_zoom: 0.0,
    }
}

/// Canvas drag → posing/camera/draw ops. The element-local
/// point converts to stage coordinates through the same
/// `ViewTransform` the drawing uses (built here from the drag's
/// own `w`/`h` and the camera at the current frame — the GPUI
/// stage's `view_transform`/`to_stage` pairing). The mode
/// branches mirror the core's own mode fields, as the GPUI
/// canvas did: draw mode paints, camera mode pans, otherwise
/// the press hit-tests and drags a joint.
fn stage_drag(s: &mut State, d: &mut Drag) {
    let core = &mut s.core;
    let vt = view_transform([d.w, d.h], core.project.stage, core.camera_here());
    let p = to_stage(&vt, [d.x, d.y]);
    match d.phase {
        Phase::Down => {
            if core.draw_mode {
                core.begin_draw(p);
            } else if core.camera_mode {
                core.begin_camera_drag(p);
            } else {
                core.begin_drag(p);
            }
        }
        Phase::Move => {
            if core.draw_mode {
                core.draw_to(p);
            } else {
                core.drag_to(p);
            }
        }
        Phase::Up => {
            if core.draw_mode {
                core.end_draw();
            } else {
                core.end_drag();
            }
        }
    }
}

/// Canvas wheel → camera zoom (`EditorCore::camera_zoom_by`).
/// Notched wheels step once per event; precise deltas
/// accumulate in `State::wheel_zoom` (see its docs). Returning
/// true captures the rest of the gesture.
fn stage_wheel(s: &mut State, e: WheelEvent) -> bool {
    if e.dy == 0.0 {
        return false;
    }
    s.wheel_zoom += if e.precise { -e.dy * 0.02 } else { -e.dy.signum() };
    let steps = s.wheel_zoom.trunc();
    if steps != 0.0 {
        s.core.camera_zoom_by(1.1f32.powf(steps).clamp(0.4, 2.5));
        s.wheel_zoom -= steps;
    }
    true
}

/// Playback clock: while `core.playing`, advance the playhead
/// by the real elapsed time (clamped so a long stall doesn't
/// jump) and schedule the next frame at the project fps
/// interval — the Rune form of the GPUI app's 33 ms tick
/// calling `advance_play`.
fn poll(s: &mut State, t: f64, tx: &mut Tx) -> bool {
    let dt = s
        .last_poll
        .map(|last| ((t - last) as f32).clamp(0.0, 0.1))
        .unwrap_or(0.0);
    s.last_poll = Some(t);
    if !s.core.playing {
        return false;
    }
    if dt > 0.0 {
        s.core.advance_play(dt);
    }
    let fps = s.core.project.fps.max(1.0);
    tx.wake_after(Duration::from_secs_f32(1.0 / fps));
    true
}

/// Wrap-around frame step (the GPUI Left/Right binding).
fn step_frame(s: &mut State, dir: i32) {
    let n = s.core.frame_count();
    if n == 0 {
        return;
    }
    let next = (s.core.current as i32 + dir).rem_euclid(n as i32);
    s.core.goto(next as usize);
}

/// App shortcuts (the GPUI root key handler's bindings):
/// Space play/pause, Left/Right frame step, Ctrl+Z undo,
/// Ctrl+Shift+Z / Ctrl+Y redo, Delete deletes the selected
/// segment (`delete_segment` no-ops safely — status only —
/// when nothing deletable is selected).
fn on_key(s: &mut State, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    if ev.is_command('z') {
        if ev.mods.shift {
            s.core.redo();
        } else {
            s.core.undo();
        }
        return true;
    }
    if ev.is_command('y') {
        s.core.redo();
        return true;
    }
    match &ev.key {
        Key::Text(text) if text == " " && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            chrome_view::toggle_play(s);
            true
        }
        Key::Left => {
            step_frame(s, -1);
            true
        }
        Key::Right => {
            step_frame(s, 1);
            true
        }
        Key::Delete => {
            s.core.delete_segment();
            true
        }
        _ => false,
    }
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    let core = &s.core;
    // The canvas size from the last committed layout (the read
    // settles on the first layout); before any layout exists,
    // draw at stage size (fit 1:1).
    let stage_size = cx.layout_size("stage").unwrap_or(core.project.stage);
    let stage_fill = stage_view::hex_color(core.project.background).unwrap_or(t.card);
    col()
        .w_full()
        .h_full()
        .fill(t.bg)
        .padding(16.0)
        .gap(12.0)
        .child(chrome_view::app_bar(s, &t))
        .child(
            canvas(stage_shape(core, stage_size))
                .key("stage")
                .grow(1.0)
                .w_full()
                .fill(stage_fill)
                .radius(8.0)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| stage_drag(s, d))
                .on_wheel(|s: &mut State, e: WheelEvent, _tx: &mut Tx| stage_wheel(s, e)),
        )
        .child(chrome_view::transport_row(s, &t))
        .child(chrome_view::timeline_row(s, &t))
        .child(
            text(chrome_view::status_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
}

fn main() {
    rune::run(AppSpec::<State> {
        min_size: Some((1280.0, 720.0)),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("StykAnimate RS", (1440.0, 900.0), init, view)
    });
}
