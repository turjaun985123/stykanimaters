//! App bar, transport, timeline and status builders for the
//! Rune editor shell (Phase C of the GPUI→Rune migration).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Every control here calls exactly one real `EditorCore` op
//! (or, for Play/Pause, flips the same public core field the
//! GPUI app's button flipped — there is no op for it; see
//! `editor.rs` field docs). Element keys carry the old GPUI
//! control ids where one existed, so headless `--tree` checks
//! can find them. Phase F removed the app bar's + Figure and
//! Onion buttons: they duplicated the figures panel's own
//! add-figure button and the onion controls panel's
//! onion-toggle, so those panels are the single homes now.
//!
//! Labels: since Phase E the app-bar and transport labels
//! resolve through `rune_sec::i18n_wire::tr` against the
//! engine's current language (`core.lang`), so they follow the
//! language picker. One label takes the table's canonical text
//! rather than Phase C's shorter literal: the delete-frame
//! button shows `delete_frame` ("Delete"). The timeline chips
//! are bare frame numbers and stay untranslated. `status_text`
//! deliberately keeps its literal English format — see its own
//! doc comment.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::i18n_wire;
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

/// Top bar: project name, then Save / Load / Undo / Redo —
/// each wired to its core op. Labels come from the i18n table
/// (keys `save`, `load`, `undo`, `redo`) in the core's current
/// language. (Phase F: the + Figure and Onion buttons that
/// used to sit here were removed — they duplicated the
/// figures panel's add-figure button and the onion panel's
/// onion-toggle.)
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
        .child(btn("save", &i18n_wire::tr(core, "save"), false, t, |s| {
            s.core.save()
        }))
        .child(btn("load", &i18n_wire::tr(core, "load"), false, t, |s| {
            s.core.load()
        }))
        .child(btn("undo", &i18n_wire::tr(core, "undo"), false, t, |s| {
            s.core.undo()
        }))
        .child(btn("redo", &i18n_wire::tr(core, "redo"), false, t, |s| {
            s.core.redo()
        }))
}

/// Transport + frame ops row: Prev / Play-Pause / Next (frame
/// stepping wraps, as the GPUI key handler did), then Add /
/// Duplicate / Delete frame. Labels come from the i18n table
/// (keys `prev_frame`, `play`/`pause`, `next_frame`,
/// `add_frame`, `duplicate_frame`, `delete_frame`) in the
/// core's current language.
pub fn transport_row(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let play_label = i18n_wire::tr(core, if core.playing { "pause" } else { "play" });
    row()
        .key("transport")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(btn(
            "prev",
            &i18n_wire::tr(core, "prev_frame"),
            false,
            t,
            |s| {
                let n = s.core.frame_count();
                if n > 0 {
                    s.core.goto((s.core.current + n - 1) % n);
                }
            },
        ))
        .child(btn("play", &play_label, core.playing, t, |s| toggle_play(s)))
        .child(btn(
            "next",
            &i18n_wire::tr(core, "next_frame"),
            false,
            t,
            |s| {
                let n = s.core.frame_count();
                if n > 0 {
                    s.core.goto((s.core.current + 1) % n);
                }
            },
        ))
        .child(el().grow(1.0))
        .child(btn(
            "add-frame",
            &i18n_wire::tr(core, "add_frame"),
            false,
            t,
            |s| s.core.add_frame(),
        ))
        .child(btn(
            "dup-frame",
            &i18n_wire::tr(core, "duplicate_frame"),
            false,
            t,
            |s| s.core.duplicate_frame(),
        ))
        .child(btn(
            "del-frame",
            &i18n_wire::tr(core, "delete_frame"),
            false,
            t,
            |s| s.core.delete_frame(),
        ))
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
///
/// Phase E decision: this format stays literal English. The
/// `frame` and `zoom` words do exist in the i18n table, but
/// the `fps` entry is the label "FPS" (an abbreviation), while
/// this line uses lowercase "fps" as a unit suffix after the
/// number — dropping the table value in would change the
/// rendered text and break the format this function's tests
/// pin. The segmented `status_strip` section is the shell's
/// live status surface; this line remains as the compact
/// fallback it was in Phase C.
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

    /// Every i18n key the app bar / transport labels resolve
    /// through must exist in the table — otherwise `tr` would
    /// silently render the raw key as the button label.
    #[test]
    fn chrome_label_keys_exist_in_the_table() {
        for key in [
            "save",
            "load",
            "undo",
            "redo",
            "prev_frame",
            "play",
            "pause",
            "next_frame",
            "add_frame",
            "duplicate_frame",
            "delete_frame",
        ] {
            assert!(i18n_wire::key_in_table(key), "label key {key} not in table");
        }
    }

    /// With the default language (English) the wired labels are
    /// exactly the table's English text — the documented
    /// change from the Phase C literals included.
    #[test]
    fn english_labels_match_the_table() {
        let core = EditorCore::new();
        assert_eq!(i18n_wire::tr(&core, "save"), "Save");
        assert_eq!(i18n_wire::tr(&core, "prev_frame"), "Previous frame");
        assert_eq!(i18n_wire::tr(&core, "next_frame"), "Next frame");
        assert_eq!(i18n_wire::tr(&core, "delete_frame"), "Delete");
    }

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
