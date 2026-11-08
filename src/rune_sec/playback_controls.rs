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
//! * **Restart / Play range** (Phase F) — `pb-restart`
//!   goes to frame 0 and plays; `pb-play-range` goes to the
//!   loop range's start and plays (frame 0 when no range
//!   is set — the Restart behaviour). Both compose
//!   `EditorCore::goto` (`editor.rs:681`, sets `current` +
//!   `playhead`) with the `playing` field (`editor.rs:38`)
//!   set true — the same field the chrome transport flips.
//!   Audio start is NOT called here: the shell's poll hooks
//!   playback start (Phase F audio glue); these buttons
//!   only set the state it observes.
//! * **Loop toggle** (Phase G) — `pb-loop` flips the
//!   shell-level `State::loop_playback` flag (integrator-
//!   owned, initialised `true` = the behaviour every earlier
//!   phase shipped). The core still has no loop field:
//!   `advance_play` (`editor.rs:710`) wraps unconditionally
//!   — at the timeline end, or inside `loop_range` when one
//!   is set — so "off" is enforced by the shell's poll,
//!   which consults `should_stop_at_end` before each advance
//!   and pauses instead of letting the playhead wrap. The
//!   button itself touches no core state and takes no undo
//!   snapshot.
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
//! `State` fields: everything reads `s.core` live
//! (`project.fps` `model.rs:683`, `loop_range`, `current`,
//! `project.frames[..].duration`) except the loop toggle,
//! which reads/writes the shell-level `State::loop_playback`
//! (integrator-owned, initialised `true`).

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};

use crate::chrome_view::btn;
use crate::State;
use stykanimate_rs::editor::EditorCore;

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

/// The frame **Play range** starts from: the loop range's
/// start when a range is set, else frame 0 — which is
/// exactly Restart's behaviour, so Play range degrades to
/// Restart on a timeline with no range.
pub fn play_range_start(loop_range: Option<(usize, usize)>) -> usize {
    loop_range.map(|(a, _)| a).unwrap_or(0)
}

/// Button label for the loop toggle — the state is in the
/// label itself ("Loop: on" / "Loop: off"), so the control
/// reads correctly at a glance in either state.
pub fn loop_toggle_label(loop_playback: bool) -> &'static str {
    if loop_playback {
        "Loop: on"
    } else {
        "Loop: off"
    }
}

/// Should the shell's poll stop playback at the end instead
/// of letting `EditorCore::advance_play` wrap the playhead?
///
/// **Integrator contract (poll, `main.rs`).** Call this on
/// every poll tick at the point where poll currently calls
/// `advance_play` — after the `!core.playing` early return,
/// before the advance. When it returns `true`, set
/// `core.playing = false` and skip `advance_play` for that
/// tick entirely. The stop has to be decided from the
/// *pre-advance* state: `advance_play` wraps the fractional
/// playhead itself (`editor.rs:710`) and reports nothing,
/// so after an advance there is no way to tell a wrap
/// happened. The existing audio flip hook at the top of
/// poll observes the cleared `playing` flag on the next
/// tick and stops playback audio through the Phase F path,
/// like any other pause.
///
/// The check is frame-granular: it reads `current`, not the
/// fractional playhead, so the final frame is left on
/// screen and playback halts at the first tick that finds
/// it current — the last frame is not guaranteed its full
/// duration of display time.
///
/// The "end" is the end playback would actually wrap at.
/// `advance_play` loops inside `loop_range` when one is set
/// (the playhead wraps at the range's end and never reaches
/// the project end), so with a range the effective last
/// frame is the range's end — clamped to the timeline
/// exactly as `advance_play` clamps it (`editor.rs:719`) —
/// and without one it is `frame_count - 1`. `current` at
/// *or past* that frame stops: a playhead parked beyond a
/// range's end (e.g. after a `goto` outside the range)
/// stops too, rather than being yanked back into the range
/// by the wrap.
///
/// `false` whenever playback is paused, looping is on, or
/// the project has no frames.
pub fn should_stop_at_end(
    playing: bool,
    loop_playback: bool,
    loop_range: Option<(usize, usize)>,
    current: usize,
    frame_count: usize,
) -> bool {
    if !playing || loop_playback || frame_count == 0 {
        return false;
    }
    let last = match loop_range {
        Some((_, b)) => b.min(frame_count - 1),
        None => frame_count - 1,
    };
    current >= last
}

/// Go to `frame` and make sure playback is running — the
/// shared body of the Restart and Play range buttons.
/// `goto` also resets the fractional `playhead`, so a
/// restart mid-tween starts cleanly on the frame. Only the
/// `playing` field is set: audio start is the shell poll's
/// business (Phase F audio glue), not a button's.
fn start_playback_at(core: &mut EditorCore, frame: usize) {
    core.goto(frame);
    core.playing = true;
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
/// `dur-dn`, `dur-up`); the Phase F transport buttons are
/// `pb-restart` / `pb-play-range` and the Phase G loop
/// toggle is `pb-loop`, so headless `--tree` checks can
/// find every control.
pub fn playback_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let fps = core.project.fps;
    let tween_on = core.project.tween_playback;
    let loop_range = core.loop_range;
    let loop_playback = s.loop_playback;
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

    // ---- Transport basics: restart from the top, or play
    // from the loop range's start (frame 0 without a range).
    panel = panel.child(section("Transport".to_string(), t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn("pb-restart", "Restart", false, t, |s| {
                start_playback_at(&mut s.core, 0);
            }))
            .child(btn(
                "pb-play-range",
                "Play range",
                loop_range.is_some(),
                t,
                |s| {
                    let start = play_range_start(s.core.loop_range);
                    start_playback_at(&mut s.core, start);
                },
            )),
    );

    // ---- Loop playback: shell-level on/off for the wrap
    // advance_play always performs. Off = poll pauses at the
    // end (see should_stop_at_end); the button itself only
    // flips the State flag — no core call, no undo snapshot.
    panel = panel.child(section("Loop playback".to_string(), t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn(
                "pb-loop",
                loop_toggle_label(loop_playback),
                loop_playback,
                t,
                |s| {
                    s.loop_playback = !s.loop_playback;
                },
            ))
            .child(section(
                if loop_playback {
                    "On — wraps at the end".to_string()
                } else {
                    "Off — stops at the end".to_string()
                },
                t,
            )),
    );

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
    use stykanimate_rs::editor::EditorCore;

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

    #[test]
    fn play_range_start_is_the_range_start_or_zero() {
        assert_eq!(play_range_start(Some((2, 4))), 2);
        assert_eq!(play_range_start(Some((0, 3))), 0);
        // No range: Play range falls back to Restart's frame.
        assert_eq!(play_range_start(None), 0);
    }

    #[test]
    fn restart_goes_to_frame_zero_and_plays() {
        // The demo project has 5 frames; start mid-timeline,
        // paused, as a user pressing Restart would find it.
        let mut core = EditorCore::new();
        core.goto(3);
        core.playing = false;
        start_playback_at(&mut core, 0);
        assert_eq!(core.current, 0);
        // goto resets the fractional playhead too.
        assert_eq!(core.playhead, 0.0);
        assert!(core.playing, "Restart ensures playback runs");
    }

    #[test]
    fn play_range_starts_at_the_range_start_and_plays() {
        let mut core = EditorCore::new();
        core.set_loop_range(2, 4);
        core.goto(0);
        core.playing = false;
        // The pb-play-range handler's exact composition.
        let start = play_range_start(core.loop_range);
        start_playback_at(&mut core, start);
        assert_eq!(core.current, 2);
        assert!(core.playing);
    }

    #[test]
    fn start_playback_at_keeps_playing_when_already_playing() {
        // Pressing Restart mid-playback restarts, never pauses.
        let mut core = EditorCore::new();
        core.goto(2);
        core.playing = true;
        start_playback_at(&mut core, 0);
        assert!(core.playing);
        assert_eq!(core.current, 0);
    }

    #[test]
    fn loop_toggle_label_states_the_state() {
        assert_eq!(loop_toggle_label(true), "Loop: on");
        assert_eq!(loop_toggle_label(false), "Loop: off");
    }

    #[test]
    fn stop_at_end_never_fires_with_loop_on() {
        // At the project end, at a range end, single frame:
        // looping on always wraps, never stops.
        assert!(!should_stop_at_end(true, true, None, 4, 5));
        assert!(!should_stop_at_end(true, true, Some((1, 3)), 3, 5));
        assert!(!should_stop_at_end(true, true, None, 0, 1));
    }

    #[test]
    fn stop_at_end_never_fires_while_paused() {
        assert!(!should_stop_at_end(false, false, None, 4, 5));
        assert!(!should_stop_at_end(false, false, Some((1, 3)), 3, 5));
        assert!(!should_stop_at_end(false, false, None, 0, 1));
    }

    #[test]
    fn stop_at_end_only_on_the_last_frame() {
        // 5-frame project, no range: frames 0..3 keep going,
        // frame 4 (the last) stops.
        for current in 0..4 {
            assert!(
                !should_stop_at_end(true, false, None, current, 5),
                "frame {current} is mid-project"
            );
        }
        assert!(should_stop_at_end(true, false, None, 4, 5));
    }

    #[test]
    fn stop_at_end_single_frame_project_stops_at_once() {
        // Frame 0 is also the last frame.
        assert!(should_stop_at_end(true, false, None, 0, 1));
    }

    #[test]
    fn stop_at_end_empty_project_never_stops() {
        assert!(!should_stop_at_end(true, false, None, 0, 0));
    }

    #[test]
    fn stop_at_end_uses_the_range_end_when_a_range_is_set() {
        // advance_play wraps inside (1, 3): frame 3 is the
        // end even though the project has a frame 4.
        assert!(!should_stop_at_end(true, false, Some((1, 3)), 2, 5));
        assert!(should_stop_at_end(true, false, Some((1, 3)), 3, 5));
        // Before the range, playback still plays into it.
        assert!(!should_stop_at_end(true, false, Some((2, 4)), 0, 5));
        // Parked past the range end: stops instead of being
        // wrapped back into the range.
        assert!(should_stop_at_end(true, false, Some((1, 3)), 4, 5));
    }

    #[test]
    fn stop_at_end_clamps_a_stale_range_end() {
        // Range end past the timeline (frames deleted since
        // it was set): advance_play clamps to the last frame
        // (editor.rs:719), so the stop check does too.
        assert!(!should_stop_at_end(true, false, Some((1, 99)), 3, 5));
        assert!(should_stop_at_end(true, false, Some((1, 99)), 4, 5));
    }
}
