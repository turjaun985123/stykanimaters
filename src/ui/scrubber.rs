//! Playback scrubber for the bottom timeline bar.
//!
//! A slim progress track: an accent fill up to the playhead, a round
//! knob on the playhead, and a row of small tick marks showing where
//! the keyframes sit. The data half ([`progress_frac`]) is a pure
//! function; the builder half ([`scrubber`]) is presentation only —
//! no listeners, so the caller decides how (or whether) interacting
//! with the strip seeks; this component only draws state.
//!
//! Tick placement uses the flex-spacer approach: this codebase lays
//! its UI out in flex rows, so each tick in the ruler row is
//! preceded by an empty spacer whose width is the fraction of the
//! track between the previous tick and this one
//! (`w(relative(delta))`). The ticks themselves are a fixed 2px
//! wide, which the spacers do not compensate for, so a tick can sit
//! up to (its index × 2px) further right than its exact fraction —
//! invisible at timeline scale, but worth knowing before anyone
//! measures pixels off this ruler.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_CELL, C_INK, C_MUTED};

/// Height of the progress track itself.
const TRACK_H: f32 = 6.0;
/// Diameter of the playhead knob (wider than the track and centred
/// on the playhead, so it reads as a handle).
const KNOB_D: f32 = 10.0;
/// Tick mark size in the ruler row.
const TICK_W: f32 = 2.0;
const TICK_H: f32 = 5.0;
/// The kit's neutral well colour (the inactive chip background in
/// `ui_kit::info_chip`), used for the unfilled track.
const TRACK_BG: u32 = C_CELL;

/// Playback progress as a fraction of the timeline: `frame_f` over
/// the last frame's index, clamped to `0.0..=1.0`. A timeline of
/// zero or one frame has no span to progress across, so it reads
/// `0.0`; a NaN playhead also reads `0.0` rather than poisoning the
/// layout.
pub fn progress_frac(frame_f: f32, frame_count: usize) -> f32 {
    if frame_count <= 1 || frame_f.is_nan() {
        return 0.0;
    }
    let last = (frame_count - 1) as f32;
    (frame_f / last).clamp(0.0, 1.0)
}

/// The keyframe fractions prepared for the ruler row: NaNs dropped,
/// the rest clamped to `0.0..=1.0` and sorted ascending, so the
/// spacer widths come out non-negative.
fn sorted_fracs(fracs: &[f32]) -> Vec<f32> {
    let mut out: Vec<f32> = fracs
        .iter()
        .copied()
        .filter(|f| !f.is_nan())
        .map(|f| f.clamp(0.0, 1.0))
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

/// The scrubber strip: (optionally) a ruler row of keyframe ticks
/// over the progress track. `frame_f` is the fractional playhead
/// (fractional so tweened playback moves the knob smoothly), and
/// `keyframe_fracs` are the keyframe positions as fractions of the
/// timeline — the same scale [`progress_frac`] returns, e.g. one
/// fraction per key in a `ui_dope::keyframe_map` row. The ruler row
/// is omitted when there are no keyframes to show.
pub fn scrubber(frame_f: f32, frame_count: usize, keyframe_fracs: &[f32]) -> Div {
    let frac = progress_frac(frame_f, frame_count);
    let mut root = div().flex_col().w_full().gap(px(2.0));

    let ticks = sorted_fracs(keyframe_fracs);
    if !ticks.is_empty() {
        let mut ruler = div().flex().flex_row().w_full().h(px(TICK_H));
        let mut prev = 0.0;
        for f in ticks {
            let gap = (f - prev).max(0.0);
            ruler = ruler
                .child(div().w(relative(gap)).h(px(TICK_H)))
                .child(
                    div()
                        .w(px(TICK_W))
                        .h(px(TICK_H))
                        .rounded(px(1.0))
                        .bg(rgb(C_MUTED)),
                );
            prev = f;
        }
        root = root.child(ruler);
    }

    // The knob is a flex child right after the fill, pulled back by
    // half its diameter so it centres on the playhead; at the very
    // start and end it overhangs the track by that half, like the
    // handle on a media player's seek bar.
    let knob = div()
        .flex_none()
        .w(px(KNOB_D))
        .h(px(KNOB_D))
        .ml(px(-(KNOB_D / 2.0)))
        .rounded(px(999.0))
        .bg(rgb(C_INK))
        .border_1()
        .border_color(rgb(C_BORDER));
    let track = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(TRACK_H))
        .rounded(px(999.0))
        .bg(rgb(TRACK_BG))
        .child(
            div()
                .h(px(TRACK_H))
                .w(relative(frac))
                .rounded(px(999.0))
                .bg(rgb(C_ACCENT)),
        )
        .child(knob);
    root.child(track)
}

#[cfg(test)]
mod tests {
    use super::{progress_frac, sorted_fracs};

    #[test]
    fn empty_and_single_frame_timelines_have_no_progress() {
        assert_eq!(progress_frac(0.0, 0), 0.0);
        assert_eq!(progress_frac(0.0, 1), 0.0);
        assert_eq!(progress_frac(1.0, 1), 0.0);
    }

    #[test]
    fn progress_is_frame_over_last_frame_index() {
        // 11 frames: indices 0..=10.
        assert_eq!(progress_frac(0.0, 11), 0.0);
        assert_eq!(progress_frac(2.5, 11), 0.25);
        assert_eq!(progress_frac(5.0, 11), 0.5);
        assert_eq!(progress_frac(10.0, 11), 1.0);
    }

    #[test]
    fn progress_clamps_out_of_range_playheads() {
        assert_eq!(progress_frac(-4.0, 11), 0.0);
        assert_eq!(progress_frac(42.0, 11), 1.0);
        assert_eq!(progress_frac(f32::INFINITY, 11), 1.0);
        assert_eq!(progress_frac(f32::NAN, 11), 0.0);
    }

    #[test]
    fn sorted_fracs_clamps_sorts_and_drops_nan() {
        assert_eq!(
            sorted_fracs(&[0.75, -1.0, 0.25, 2.0, f32::NAN]),
            vec![0.0, 0.25, 0.75, 1.0]
        );
        assert!(sorted_fracs(&[]).is_empty());
    }
}
