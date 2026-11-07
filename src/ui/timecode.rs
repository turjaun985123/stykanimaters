//! Timecode readout for the timeline bar.
//!
//! The timeline's frame chips say *which* frame the playhead is
//! on, but not *when* — how far into the animation, in playback
//! time, that frame falls. This chip answers that in the format
//! every animator reads at a glance: `current / total` as
//! `M:SS.t` timecodes derived from the project's frame rate.
//!
//! Presentation only, like the rest of the kit: plain numbers
//! in, a `Div` out, no listeners and no element id inside — the
//! caller drops [`timecode_chip`] into the bottom bar as-is.

use crate::ui_kit::C_MUTED;
use gpui::*;

/// Format a duration in seconds as `M:SS.t` — minutes (not
/// capped at 59), two-digit seconds, one decimal tenth.
///
/// The value is rounded once, to total tenths (half away from
/// zero), before being split into minutes/seconds/tenths, so a
/// duration just under a minute boundary carries cleanly:
/// 59.97 s formats as "1:00.0", never "0:60.0". One consequence
/// of total-tenths rounding: 3.25 s is exactly 32.5 tenths and
/// rounds up, giving "0:03.3". Negative and NaN inputs clamp to
/// "0:00.0".
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

/// The chip's text: `"<current> / <total>"`, where current is
/// `current_frame / fps` seconds and total is `frame_count /
/// fps`. A non-positive `fps` falls back to 1.0 — the codebase's
/// convention wherever a frame rate divides time — so the label
/// still resolves instead of dividing by zero. A zero
/// `frame_count` (and frame) yields "0:00.0 / 0:00.0".
pub fn timecode_label(current_frame: usize, frame_count: usize, fps: f32) -> String {
    let fps = if fps > 0.0 { fps } else { 1.0 };
    format!(
        "{} / {}",
        format_timecode(current_frame as f32 / fps),
        format_timecode(frame_count as f32 / fps)
    )
}

/// A small read-only pill showing [`timecode_label`]: 11px muted
/// text on the same neutral background as `info_chip`'s inactive
/// state, so it sits quietly beside the louder frame/FPS chips.
pub fn timecode_chip(current_frame: usize, frame_count: usize, fps: f32) -> Div {
    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(rgb(0x2C313A))
        .text_size(px(11.0))
        .text_color(rgb(C_MUTED))
        .child(timecode_label(current_frame, frame_count, fps))
}

#[cfg(test)]
mod tests {
    use super::{format_timecode, timecode_label};

    #[test]
    fn zero_formats_as_zero() {
        assert_eq!(format_timecode(0.0), "0:00.0");
    }

    #[test]
    fn seconds_pad_to_two_digits() {
        assert_eq!(format_timecode(3.24), "0:03.2");
        assert_eq!(format_timecode(9.0), "0:09.0");
    }

    #[test]
    fn tenths_round_half_away_from_zero() {
        // 3.25 s is exactly 32.5 tenths; total-tenths rounding
        // (half away from zero) pins this to "0:03.3".
        assert_eq!(format_timecode(3.25), "0:03.3");
    }

    #[test]
    fn minutes_roll_over_and_are_not_capped() {
        assert_eq!(format_timecode(65.0), "1:05.0");
        assert_eq!(format_timecode(600.0), "10:00.0");
        assert_eq!(format_timecode(3600.0), "60:00.0");
    }

    #[test]
    fn tenths_carry_across_the_minute_boundary() {
        assert_eq!(format_timecode(59.97), "1:00.0");
    }

    #[test]
    fn negative_and_nan_clamp_to_zero() {
        assert_eq!(format_timecode(-5.0), "0:00.0");
        assert_eq!(format_timecode(f32::NAN), "0:00.0");
    }

    #[test]
    fn label_shows_current_over_total() {
        assert_eq!(timecode_label(45, 90, 30.0), "0:01.5 / 0:03.0");
        assert_eq!(timecode_label(0, 150, 30.0), "0:00.0 / 0:05.0");
    }

    #[test]
    fn label_with_no_frames_is_zero_over_zero() {
        assert_eq!(timecode_label(0, 0, 30.0), "0:00.0 / 0:00.0");
    }

    #[test]
    fn label_falls_back_to_one_fps() {
        assert_eq!(timecode_label(10, 20, 0.0), "0:10.0 / 0:20.0");
        assert_eq!(timecode_label(10, 20, -30.0), "0:10.0 / 0:20.0");
    }
}
