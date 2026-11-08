//! Status strip for the Rune editor shell (Phase D, section
//! 27): a segmented, separator-divided replacement for the
//! one-line `chrome_view::status_text`. Pure builder — it
//! reads `EditorCore` and returns an element tree; it
//! attaches no handlers and mutates nothing.
//!
//! Every value shown is read live from the core:
//!
//! - project name — `core.project.name`
//!   (`model.rs:682`)
//! - frame position — `core.current` + 1 over
//!   `core.frame_count()` (`editor.rs:33`, `editor.rs:192`)
//! - timecode — current frame at `core.project.fps`
//!   (`model.rs:683`), via the pure [`timecode`] fn
//! - fps — `core.project.fps`, formatted as in chrome
//! - zoom — `core.camera_here().2` (`editor.rs:530`), the
//!   camera key zoom at the current frame
//! - selection — `core.selected_figure` /
//!   `core.selected_node` (`editor.rs:34-35`) with the
//!   figure's `FigureInstance::name` (`model.rs:404`)
//! - play state — `core.playing` (`editor.rs:38`)
//! - last status message — `core.status` (`editor.rs:42`),
//!   the trailing segment chrome's line ended with, kept so
//!   Save/Load feedback stays visible
//!
//! Deliberate omissions (no fake data):
//!
//! - **No saved/unsaved marker.** The core does track edit
//!   state, but only in private fields — `edit_count` and
//!   `last_autosave_count` (`editor.rs:50`, `editor.rs:52`)
//!   — and the due-check `autosave_due` is a private fn
//!   (`editor.rs:1570`). `save()` itself
//!   (`editor.rs:1534`) exposes no public dirty flag, and
//!   this section may not edit the engine, so there is no
//!   real signal to read. A marker derived from anything
//!   else (e.g. sniffing `core.status`) would be invented
//!   data, so none is shown.
//! - **No undo depth.** The undo stack is a private field
//!   (`editor.rs:43`); the core offers the `undo()` /
//!   `redo()` ops (`editor.rs:230`, `editor.rs:215`) but
//!   no public accessor for the stack length, so a depth
//!   readout cannot be sourced honestly.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, El};
use stykanimate_rs::editor::EditorCore;

/// Timecode of a frame at a project frame rate, formatted
/// `m:ss.f` (minutes unpadded, seconds zero-padded, one
/// tenths digit). Frame 0 is time zero. A non-positive or
/// non-finite fps cannot time anything and yields
/// `"0:00.0"`.
///
/// The math is done in total tenths
/// (`frame * 10 / fps`, floored) so the seconds and tenths
/// digits can never disagree, with a hair of epsilon
/// against division landing just under a whole tenth
/// (e.g. frame 29 at 10 fps is exactly 2.9 s).
pub fn timecode(frame: usize, fps: f32) -> String {
    if !(fps > 0.0) || !fps.is_finite() {
        return "0:00.0".to_string();
    }
    let total_tenths = (frame as f64 * 10.0 / fps as f64 + 1e-6).floor() as u64;
    format!(
        "{}:{:02}.{}",
        total_tenths / 600,
        (total_tenths / 10) % 60,
        total_tenths % 10
    )
}

/// Fps readout, formatted exactly as chrome's status line
/// does it: whole rates without a fraction (`"12"`),
/// fractional rates with one decimal (`"30.0"` for 29.97).
pub fn fps_text(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    }
}

/// Frame position segment: `Frame {current + 1}/{total}`
/// (1-based, the convention chrome's line and the timeline
/// chips both use).
pub fn frame_text(core: &EditorCore) -> String {
    format!("Frame {}/{}", core.current + 1, core.frame_count())
}

/// Zoom segment from the camera key at the current frame:
/// `Zoom {pct}%`, rounded the way chrome's line rounds it.
pub fn zoom_text(core: &EditorCore) -> String {
    let pct = (core.camera_here().2 * 100.0).round() as i64;
    format!("Zoom {pct}%")
}

/// Selection summary. With a node selected:
/// `"{figure} › node {n}"` — the node index as stored,
/// matching the core's own status phrasing
/// (`"figure {} node {}"`, `editor.rs:1149`). With only a
/// figure selected, the figure name alone. With no figure
/// at all (empty project or a stale index), `"No
/// selection"`.
pub fn selection_text(core: &EditorCore) -> String {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return "No selection".to_string();
    };
    match core.selected_node {
        Some(n) => format!("{} › node {}", fig.name, n),
        None => fig.name.clone(),
    }
}

/// Play-state segment: a state glyph plus the word, so the
/// meaning survives even if a glyph falls back in the
/// embedded font. `▶` while playing (media rolling),
/// `❚❚` while paused (bars held).
pub fn play_text(playing: bool) -> &'static str {
    if playing {
        "▶ Playing"
    } else {
        "❚❚ Paused"
    }
}

/// One strip segment: a single label-styled text.
fn segment(label: String, t: &Theme) -> El {
    text(label).font(type_::LABEL).color(t.text)
}

/// Hairline divider between segments.
fn divider(t: &Theme) -> El {
    el().size(1.0, 12.0).fill(t.control_track)
}

/// The status strip: project name, frame position,
/// timecode, fps, zoom, selection, play state and the
/// core's current status message, divided by hairlines.
/// Keyed `status-strip` so headless `--tree` checks can
/// find it. Theme follows the shell's current convention
/// (`main.rs` builds its view with `tok::theme(true)`).
pub fn status_strip(core: &EditorCore) -> El {
    let t = tok::theme(true);
    let segments: Vec<El> = vec![
        segment(core.project.name.clone(), &t),
        segment(frame_text(core), &t),
        segment(timecode(core.current, core.project.fps), &t),
        segment(format!("{} fps", fps_text(core.project.fps)), &t),
        segment(zoom_text(core), &t),
        segment(selection_text(core), &t),
        // The play segment lights up in the accent colour
        // while playback is rolling.
        text(play_text(core.playing))
            .font(type_::LABEL)
            .color(if core.playing {
                tok::accent(0, true)
            } else {
                t.text
            }),
        // Chrome's line ended with the core's status
        // message; keep it, dimmed, as the tail segment.
        text(core.status.clone())
            .font(type_::LABEL)
            .color(t.text_secondary),
    ];
    let mut bar = row().key("status-strip").w_full().gap(8.0).center_y();
    for (i, seg) in segments.into_iter().enumerate() {
        if i > 0 {
            bar = bar.child(divider(&t));
        }
        bar = bar.child(seg);
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_frame_zero_is_zero() {
        assert_eq!(timecode(0, 12.0), "0:00.0");
        assert_eq!(timecode(0, 60.0), "0:00.0");
    }

    #[test]
    fn timecode_whole_seconds() {
        assert_eq!(timecode(12, 12.0), "0:01.0");
        assert_eq!(timecode(60, 30.0), "0:02.0");
    }

    #[test]
    fn timecode_tenths_digit() {
        // 150 frames at 12 fps = 12.5 s exactly.
        assert_eq!(timecode(150, 12.0), "0:12.5");
        // 29 frames at 10 fps = 2.9 s; the division must
        // not land a hair under the whole tenth.
        assert_eq!(timecode(29, 10.0), "0:02.9");
        // Sub-tenth remainders floor, never round up.
        assert_eq!(timecode(7, 30.0), "0:00.2");
    }

    #[test]
    fn timecode_rolls_into_minutes() {
        // 3723 frames at 60 fps = 62.05 s -> 1:02.0.
        assert_eq!(timecode(3723, 60.0), "1:02.0");
        // A full hour of 10 fps frames keeps counting
        // minutes rather than rolling to hours.
        assert_eq!(timecode(36_000, 10.0), "60:00.0");
    }

    #[test]
    fn timecode_unusable_fps_is_zero() {
        assert_eq!(timecode(100, 0.0), "0:00.0");
        assert_eq!(timecode(100, -12.0), "0:00.0");
        assert_eq!(timecode(100, f32::NAN), "0:00.0");
    }

    #[test]
    fn fps_text_whole_and_fractional() {
        assert_eq!(fps_text(12.0), "12");
        assert_eq!(fps_text(29.97), "30.0");
        assert_eq!(fps_text(23.976), "24.0");
    }

    #[test]
    fn frame_text_is_one_based() {
        let core = EditorCore::new();
        let txt = frame_text(&core);
        assert!(txt.starts_with("Frame 1/"), "unexpected: {txt}");
        assert!(txt.ends_with(&core.frame_count().to_string()));
    }

    #[test]
    fn zoom_text_defaults_to_100_percent() {
        let core = EditorCore::new();
        assert_eq!(zoom_text(&core), "Zoom 100%");
    }

    #[test]
    fn selection_text_figure_only() {
        let core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        assert_eq!(selection_text(&core), core.project.figures[0].name);
    }

    #[test]
    fn selection_text_figure_and_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(3);
        let name = core.project.figures[0].name.clone();
        assert_eq!(selection_text(&core), format!("{name} › node 3"));
    }

    #[test]
    fn selection_text_no_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(selection_text(&core), "No selection");
    }

    #[test]
    fn play_text_reflects_state() {
        assert_eq!(play_text(true), "▶ Playing");
        assert_eq!(play_text(false), "❚❚ Paused");
    }
}
