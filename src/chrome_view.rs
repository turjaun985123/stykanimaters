//! App bar, transport, timeline and status builders for the
//! Rune editor shell (Phase C of the GPUI→Rune migration).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Every control here calls exactly one real `EditorCore` op
//! (or, for Play/Pause and Onion, flips the same public core
//! field the GPUI app's buttons flipped — there is no op for
//! those two; see `editor.rs` field docs). Element keys carry
//! the old GPUI control ids where one existed, so headless
//! `--tree` checks can find them.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::State;

/// One chrome button: a padded, rounded label with hover/press
/// fills; `active` renders it in the accent colour (toggle
/// state). The press handler is attached here, as in the GPUI
/// app's `btn()` helper.
pub fn btn(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    el()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if active { Color::WHITE } else { t.text }),
        )
}

/// Play/pause: flips `core.playing` (the GPUI app's
/// `toggle_play` did exactly this field flip; audio start is a
/// later phase). Starting playback syncs the playhead to the
/// shown frame, the same sync `EditorCore::goto` performs, so
/// `advance_play` starts from the frame on screen.
pub fn toggle_play(s: &mut State) {
    if !s.core.playing {
        s.core.playhead = s.core.current as f32;
    }
    s.core.playing = !s.core.playing;
}

/// Top bar: project name, then Save / Load / Undo / Redo /
/// + Figure / Onion — each wired to its core op.
pub fn app_bar(s: &State, t: &Theme) -> El {
    let core = &s.core;
    row()
        .key("app-bar")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(core.project.name.clone())
                .font(type_::TITLE)
                .color(t.text)
                .grow(1.0),
        )
        .child(btn("save", "Save", false, t, |s| s.core.save()))
        .child(btn("load", "Load", false, t, |s| s.core.load()))
        .child(btn("undo", "Undo", false, t, |s| s.core.undo()))
        .child(btn("redo", "Redo", false, t, |s| s.core.redo()))
        .child(btn("add-figure", "+ Figure", false, t, |s| s.core.add_figure()))
        .child(btn("onion", "Onion", core.onion, t, |s| {
            s.core.onion = !s.core.onion;
        }))
}

/// Transport + frame ops row: Prev / Play-Pause / Next (frame
/// stepping wraps, as the GPUI key handler did), then Add /
/// Duplicate / Delete frame.
pub fn transport_row(s: &State, t: &Theme) -> El {
    let core = &s.core;
    row()
        .key("transport")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(btn("prev", "Prev", false, t, |s| {
            let n = s.core.frame_count();
            if n > 0 {
                s.core.goto((s.core.current + n - 1) % n);
            }
        }))
        .child(btn(
            "play",
            if core.playing { "Pause" } else { "Play" },
            core.playing,
            t,
            |s| toggle_play(s),
        ))
        .child(btn("next", "Next", false, t, |s| {
            let n = s.core.frame_count();
            if n > 0 {
                s.core.goto((s.core.current + 1) % n);
            }
        }))
        .child(el().grow(1.0))
        .child(btn("add-frame", "+ Frame", false, t, |s| s.core.add_frame()))
        .child(btn("dup-frame", "Duplicate", false, t, |s| {
            s.core.duplicate_frame()
        }))
        .child(btn("del-frame", "Delete frame", false, t, |s| {
            s.core.delete_frame()
        }))
}

/// The timeline: one numbered chip per frame in a horizontally
/// scrolling row; the current frame is accent-filled. A chip
/// press goes to that frame (`EditorCore::goto`).
pub fn timeline_row(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let accent = tok::accent(0, true);
    let mut chips = row().key("timeline").gap(4.0);
    for i in 0..core.frame_count() {
        let active = i == core.current;
        chips = chips.child(
            el()
                .key(format!("frame-{}", i + 1))
                .size(30.0, 24.0)
                .radius(6.0)
                .fill(if active { accent } else { t.card })
                .hover(if active { accent } else { t.control_track })
                .center()
                .on_press(move |s: &mut State| s.core.goto(i))
                .child(
                    text(format!("{}", i + 1))
                        .font(type_::LABEL)
                        .color(if active { Color::WHITE } else { t.text }),
                ),
        );
    }
    chips.scroll_x().w_full()
}

/// The status line text: `Frame {cur+1}/{count} · {fps} fps ·
/// Zoom {pct}% · {core.status}` — the same fields the GPUI
/// status strip showed (zoom is the camera key zoom at the
/// current frame, via `EditorCore::camera_here`).
pub fn status_text(core: &EditorCore) -> String {
    let fps = core.project.fps;
    let fps_txt = if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    };
    let zoom_pct = (core.camera_here().2 * 100.0).round() as i64;
    format!(
        "Frame {}/{} · {} fps · Zoom {}% · {}",
        core.current + 1,
        core.frame_count(),
        fps_txt,
        zoom_pct,
        core.status
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_reports_frame_fps_zoom_and_status() {
        let core = EditorCore::new();
        let txt = status_text(&core);
        assert!(txt.starts_with("Frame 1/"), "unexpected status: {txt}");
        assert!(txt.contains(" fps · Zoom 100% · "), "unexpected status: {txt}");
        assert!(txt.ends_with(&core.status), "status op text missing: {txt}");
    }

    #[test]
    fn status_text_formats_fractional_fps() {
        let mut core = EditorCore::new();
        core.project.fps = 29.97;
        assert!(status_text(&core).contains("30.0 fps") || status_text(&core).contains("29.9 fps"));
    }
}
