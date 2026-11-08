//! Playback extras for the Rune editor shell (Phase D,
//! section 29 — GPUI→Rune migration). Declared from `main.rs`
//! only (as `rune_sec::playback_controls`) — bin-crate code,
//! not part of the engine library.
//!
//! The basic transport (Prev / Play-Pause / Next, frame ops)
//! already lives in `chrome_view::transport_row`; this section
//! ADDS the playback controls the GPUI Playback tool group
//! carried, each wired to the same real `EditorCore` op the
//! GPUI button called (ids and semantics verified against
//! `editor.rs` and the GPUI app at commit `1ea3633`):
//!
//! * **Frame rate** — preset chips for the GPUI cycler's
//!   preset list `[6, 12, 24, 30, 60]` (old `app.rs` `fps-
//!   preset` handler), each applied through
//!   `EditorCore::adjust_fps` (`editor.rs:1681`, adds a delta,
//!   clamps 1..=60) as `adjust_fps(preset - current)` — the
//!   same delta route the cycler used, so the status line
//!   stays consistent. Plus the `fps-dn` / `fps-up` steppers
//!   (`adjust_fps(∓1.0)`, old `app.rs:4177` / `:4181`).
//! * **Tween play** — `EditorCore::toggle_tween_playback`
//!   (`editor.rs:1691`), flipping `Project.tween_playback`
//!   (`model.rs:694`); shown active from that field.
//! * **Loop range** — `loop-start` / `loop-end` move one end
//!   of `EditorCore::loop_range` (`editor.rs:61`) to the
//!   current frame via `EditorCore::set_loop_range`
//!   (`editor.rs:738`, orders + clamps; the other end is kept,
//!   or the current frame when no range exists — the GPUI
//!   handlers at old `app.rs:3933` / `:3939` verbatim);
//!   `loop-clear` calls `EditorCore::clear_loop_range`
//!   (`editor.rs:752`). Playback wraps inside the range in
//!   `advance_play` (`editor.rs:687`).
//! * **Frame duration** — the `dur-dn` / `dur-up` steppers
//!   call `EditorCore::adjust_frame_duration`
//!   (`editor.rs:1675`) with the GPUI factors 0.5 / 2.0
//!   (multiplies the current frame's `duration`, clamps
//!   0.25..=8.0).
//! * **Timecode** — a read-only `current / total` readout in
//!   `M:SS.t`, ported verbatim from the GPUI `ui/timecode.rs`
//!   chip: frame index ÷ fps (NOT duration-weighted; the
//!   duration-weighted total is `Project::total_time`,
//!   `model.rs:855`, and is deliberately not what this chip
//!   showed).
//!
//! No new `State` fields: everything reads `s.core` live
//! (`project.fps` `model.rs:683`, `loop_range`, `current`,
//! `project.frames[..].duration`).

use rune::tok::type_;
use rune::{col, row, text, Theme, El};

use crate::chrome_view::btn;
use crate::State;

/// The GPUI fps cycler's preset frame rates, ascending
/// (old `app.rs`, `fps-preset` handler: `[6, 12, 24, 30,
/// 60]`). Every value sits inside `adjust_fps`'s 1..=60
/// clamp, so a chip press always lands exactly on its preset.
pub const FPS_PRESETS: [f32; 5] = [6.0, 12.0, 24.0, 30.0, 60.0];

/// Element key for a preset chip (stable, section-prefixed;
/// the presets are whole numbers, so the key carries the
/// integer rate).
fn preset_key(preset: f32) -> String {
    format!("fps-preset-{}", preset as i64)
}

/// Chip label for a preset (`"12"` — the row's heading and
/// the readout already say "fps").
fn preset_label(preset: f32) -> String {
    format!("{}", preset as i64)
}

/// Whether `fps` is (within float tolerance) exactly the
/// given preset — drives the chip's active fill. Tolerance
/// covers fps values reached by repeated ±1 steps, which are
/// exact in practice; anything off-preset (e.g. 13) lights no
/// chip.
pub fn is_active_preset(fps: f32, preset: f32) -> bool {
    (fps - preset).abs() < 0.01
}

/// The current rate as the chrome status line writes it:
/// whole rates as an integer, fractional rates with one
/// decimal (`"12 fps"`, `"29.9 fps"` — same convention as
/// `chrome_view::status_text`).
pub fn fps_text(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{} fps", fps as i64)
    } else {
        format!("{fps:.1} fps")
    }
}

/// Readout for the loop-range row: the inclusive range in
/// 1-based frame numbers, or the whole-timeline note when no
/// range is set.
pub fn loop_range_text(range: Option<(usize, usize)>) -> String {
    match range {
        Some((a, b)) => format!("Looping frames {}–{}", a + 1, b + 1),
        None => "No loop range — whole timeline loops".to_string(),
    }
}

/// Readout for the current frame's duration multiplier, in
/// the same `{:.2}x` shape the core's own status message
/// uses (`adjust_frame_duration`, `editor.rs:1675`).
pub fn frame_duration_text(duration: f32) -> String {
    format!("{duration:.2}x")
}

/// Format a duration in seconds as `M:SS.t` — minutes (not
/// capped at 59), two-digit seconds, one decimal tenth.
/// Ported verbatim from the GPUI `ui/timecode.rs`: the value
/// is rounded once, to total tenths (half away from zero),
/// before being split, so 59.97 s formats as "1:00.0", never
/// "0:60.0" — and 3.25 s (exactly 32.5 tenths) rounds up to
/// "0:03.3". Negative and NaN inputs clamp to "0:00.0".
pub fn format_timecode(total_secs: f32) -> String {
    let clamped = if total_secs.is_nan() || total_secs < 0.0 {
        0.0
    } else {
        total_secs
    };
    let tenths = (clamped * 10.0).round() as u64;
    let minutes = tenths / 600;
    let seconds = (tenths % 600) / 10;
    let tenth = tenths % 10;
    format!("{minutes}:{seconds:02}.{tenth}")
}

/// The readout's text: `"<current> / <total>"`, where current
/// is `current_frame / fps` seconds and total is
/// `frame_count / fps` (the GPUI chip's semantics, ported
/// verbatim). A non-positive `fps` falls back to 1.0 — the
/// codebase's convention wherever a frame rate divides time —
/// so the label still resolves instead of dividing by zero.
pub fn timecode_label(current_frame: usize, frame_count: usize, fps: f32) -> String {
    let fps = if fps > 0.0 { fps } else { 1.0 };
    format!(
        "{} / {}",
        format_timecode(current_frame as f32 / fps),
        format_timecode(frame_count as f32 / fps)
    )
}

/// A muted section heading inside the panel (same helper
/// shape as the other `rune_sec` panels).
fn section(label: String, t: &Theme) -> El {
    text(label).font(type_::LABEL).color(t.text_secondary)
}

/// The playback extras panel body. Mounting (and any
/// show/hide state) is the caller's business; this builds the
/// full panel for the current core state. Element keys carry
/// the old GPUI control ids where one existed (`fps-dn`,
/// `fps-up`, `tween`, `loop-start`, `loop-end`, `loop-clear`,
/// `dur-dn`, `dur-up`), so headless `--tree` checks can find
/// every control.
pub fn playback_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let fps = core.project.fps;
    let tween_on = core.project.tween_playback;
    let loop_range = core.loop_range;
    let cur_duration = core
        .project
        .frames
        .get(core.current)
        .map(|f| f.duration);

    let mut panel = col()
        .key("playback-controls")
        .gap(8.0)
        .padding(10.0)
        .fill(t.card)
        .radius(8.0)
        .child(text("Playback").font(type_::TITLE).color(t.text));

    // ---- Frame rate: stepper + current-rate readout.
    panel = panel.child(section(format!("Frame rate — now {}", fps_text(fps)), t));
    let mut rate_row = row().gap(6.0).center_y();
    rate_row = rate_row.child(btn("fps-dn", "FPS −", false, t, |s| {
        s.core.adjust_fps(-1.0);
    }));
    rate_row = rate_row.child(btn("fps-up", "FPS +", false, t, |s| {
        s.core.adjust_fps(1.0);
    }));
    panel = panel.child(rate_row);

    // ---- Frame rate: one chip per GPUI preset, applied as a
    // delta through adjust_fps (the cycler's route).
    let mut preset_row = row().gap(6.0).center_y();
    for p in FPS_PRESETS {
        preset_row = preset_row.child(btn(
            &preset_key(p),
            &preset_label(p),
            is_active_preset(fps, p),
            t,
            move |s: &mut State| {
                let cur = s.core.project.fps;
                s.core.adjust_fps(p - cur);
            },
        ));
    }
    panel = panel.child(preset_row);

    // ---- Tween playback toggle (project.tween_playback).
    panel = panel.child(section("Tween playback".to_string(), t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn("tween", "Tween play", tween_on, t, |s| {
                s.core.toggle_tween_playback();
            }))
            .child(section(
                if tween_on {
                    "On — in-between frames while playing".to_string()
                } else {
                    "Off — authored frames only".to_string()
                },
                t,
            )),
    );

    // ---- Loop range: move either end to the current frame
    // (GPUI handlers verbatim), or clear.
    panel = panel.child(section(loop_range_text(loop_range), t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn(
                "loop-start",
                "Loop start",
                loop_range.is_some(),
                t,
                |s| {
                    let cur = s.core.current;
                    let (_, b) = s.core.loop_range.unwrap_or((cur, cur));
                    s.core.set_loop_range(cur, b);
                },
            ))
            .child(btn(
                "loop-end",
                "Loop end",
                loop_range.is_some(),
                t,
                |s| {
                    let cur = s.core.current;
                    let (a, _) = s.core.loop_range.unwrap_or((cur, cur));
                    s.core.set_loop_range(a, cur);
                },
            ))
            .child(btn("loop-clear", "Loop clear", false, t, |s| {
                s.core.clear_loop_range();
            })),
    );

    // ---- Current-frame duration stepper (0.5x / 2x, the
    // GPUI factors; the op clamps 0.25..=8.0).
    panel = panel.child(section(
        match cur_duration {
            Some(d) => format!(
                "Frame {} duration — now {}",
                core.current + 1,
                frame_duration_text(d)
            ),
            None => "Frame duration".to_string(),
        },
        t,
    ));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn("dur-dn", "Dur −", false, t, |s| {
                s.core.adjust_frame_duration(0.5);
            }))
            .child(btn("dur-up", "Dur +", false, t, |s| {
                s.core.adjust_frame_duration(2.0);
            })),
    );

    // ---- Timecode readout (read-only).
    panel = panel.child(section("Time".to_string(), t));
    panel = panel.child(
        text(timecode_label(core.current, core.frame_count(), fps))
            .key("timecode")
            .font(type_::TITLE)
            .color(t.text),
    );

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_the_gpui_cycler_list_ascending() {
        assert_eq!(FPS_PRESETS, [6.0, 12.0, 24.0, 30.0, 60.0]);
        assert!(FPS_PRESETS.windows(2).all(|w| w[0] < w[1]));
        // Every preset is reachable: inside adjust_fps's clamp.
        assert!(FPS_PRESETS.iter().all(|p| (1.0..=60.0).contains(p)));
    }

    #[test]
    fn preset_keys_and_labels_carry_the_whole_rate() {
        assert_eq!(preset_key(6.0), "fps-preset-6");
        assert_eq!(preset_key(24.0), "fps-preset-24");
        assert_eq!(preset_key(60.0), "fps-preset-60");
        assert_eq!(preset_label(12.0), "12");
        assert_eq!(preset_label(30.0), "30");
    }

    #[test]
    fn active_preset_matches_exact_rates_only() {
        assert!(is_active_preset(24.0, 24.0));
        assert!(is_active_preset(12.0, 12.0));
        assert!(!is_active_preset(13.0, 12.0));
        assert!(!is_active_preset(13.0, 24.0));
        assert!(!is_active_preset(25.0, 24.0));
    }

    #[test]
    fn fps_text_writes_whole_rates_as_integers() {
        assert_eq!(fps_text(12.0), "12 fps");
        assert_eq!(fps_text(60.0), "60 fps");
        assert_eq!(fps_text(29.97), "30.0 fps");
    }

    #[test]
    fn loop_range_text_is_one_based_and_handles_none() {
        assert_eq!(loop_range_text(Some((1, 3))), "Looping frames 2–4");
        assert_eq!(loop_range_text(Some((0, 0))), "Looping frames 1–1");
        assert!(loop_range_text(None).starts_with("No loop range"));
    }

    #[test]
    fn frame_duration_text_uses_the_core_status_shape() {
        assert_eq!(frame_duration_text(1.0), "1.00x");
        assert_eq!(frame_duration_text(0.5), "0.50x");
        assert_eq!(frame_duration_text(2.25), "2.25x");
    }

    #[test]
    fn timecode_rounds_to_total_tenths_before_splitting() {
        assert_eq!(format_timecode(0.0), "0:00.0");
        // Just under a minute boundary carries cleanly.
        assert_eq!(format_timecode(59.97), "1:00.0");
        // Exactly 32.5 tenths rounds half away from zero.
        assert_eq!(format_timecode(3.25), "0:03.3");
        assert_eq!(format_timecode(61.0), "1:01.0");
        // Minutes are not capped at 59.
        assert_eq!(format_timecode(600.0), "10:00.0");
    }

    #[test]
    fn timecode_clamps_negative_and_nan_to_zero() {
        assert_eq!(format_timecode(-5.0), "0:00.0");
        assert_eq!(format_timecode(f32::NAN), "0:00.0");
    }

    #[test]
    fn timecode_label_is_current_over_total_at_fps() {
        // The GPUI chip's own example shape: 5 frames at 12 fps.
        assert_eq!(timecode_label(0, 5, 12.0), "0:00.0 / 0:00.4");
        assert_eq!(timecode_label(24, 48, 24.0), "0:01.0 / 0:02.0");
        assert_eq!(timecode_label(0, 0, 12.0), "0:00.0 / 0:00.0");
    }

    #[test]
    fn timecode_label_falls_back_to_1fps_for_bad_rates() {
        assert_eq!(timecode_label(30, 60, 0.0), "0:30.0 / 1:00.0");
        assert_eq!(timecode_label(30, 60, -12.0), "0:30.0 / 1:00.0");
    }
}
