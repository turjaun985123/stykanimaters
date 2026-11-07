//! Slider track: a horizontal bar with an accent fill and a knob
//! dot marking a value's position inside a range.
//!
//! Presentation only — no state, no handlers, no editor knowledge
//! (the same contract as [`crate::ui_kit`]). The component renders
//! the *current* value; the caller owns the value and wires up
//! whatever pointer handling changes it (steppers, drags, keys),
//! then re-renders with the new fraction from [`clamp_fraction`].
//! Typical uses: weather amount, figure opacity, graph ranges —
//! anywhere a panel now shows a bare number.
//!
//! Two pieces:
//! - [`clamp_fraction`] is the pure maths: a value in an arbitrary
//!   `min..=max` range mapped to a `0.0..=1.0` fraction.
//! - [`slider_track`] is the picture: a [`crate::ui_kit::C_BORDER`]
//!   track, an accent fill whose width is that fraction of the
//!   track, and a knob dot centred on the end of the fill.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER};

/// Total height of the slider strip: the 12px knob plus a little
/// air above and below.
const TRACK_H: f32 = 14.0;
/// Thickness of the track bar itself.
const BAR_H: f32 = 4.0;
/// Knob diameter.
const KNOB_D: f32 = 12.0;

/// Map `value` within `min..=max` to a fraction in `0.0..=1.0`:
/// `min` is 0.0, `max` is 1.0, the midpoint is 0.5, and anything
/// outside the range clamps to the nearest end.
///
/// Degenerate inputs are safe rather than panicking: a NaN value
/// or NaN bound yields 0.0, and an empty or reversed range
/// (`max <= min`) yields 0.0 instead of dividing by zero.
pub fn clamp_fraction(value: f32, min: f32, max: f32) -> f32 {
    if value.is_nan() || min.is_nan() || max.is_nan() || max <= min {
        return 0.0;
    }
    let t = (value - min) / (max - min);
    if t.is_nan() {
        0.0
    } else {
        t.clamp(0.0, 1.0)
    }
}

/// Build the slider track for a fraction in `0.0..=1.0` (see
/// [`clamp_fraction`] for arbitrary ranges). Out-of-range and NaN
/// fractions are clamped for display, so callers may pass a raw
/// computed value without pre-checking it.
///
/// The track stretches to the width of its parent; the knob
/// overhangs the fill's end by half its diameter, the usual slider
/// look, so give the parent a few pixels of horizontal padding.
pub fn slider_track(fraction: f32) -> Div {
    let f = clamp_fraction(fraction, 0.0, 1.0);
    let bar_top = (TRACK_H - BAR_H) / 2.0;
    div()
        .relative()
        .w_full()
        .h(px(TRACK_H))
        // The empty track, end to end.
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .top(px(bar_top))
                .h(px(BAR_H))
                .rounded(px(999.0))
                .bg(rgb(C_BORDER)),
        )
        // The filled portion, proportional to the fraction.
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(bar_top))
                .h(px(BAR_H))
                .w(relative(f))
                .rounded(px(999.0))
                .bg(rgb(C_ACCENT)),
        )
        // The knob: a wrapper exactly as wide as the fill, with
        // the dot right-aligned inside it and nudged right by half
        // its diameter so it sits centred on the fill's end.
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .h(px(TRACK_H))
                .w(relative(f))
                .flex()
                .justify_end()
                .items_center()
                .child(
                    div()
                        .w(px(KNOB_D))
                        .h(px(KNOB_D))
                        .mr(px(-KNOB_D / 2.0))
                        .rounded(px(999.0))
                        .bg(rgb(C_ACCENT)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::clamp_fraction;

    #[test]
    fn below_min_clamps_to_zero() {
        assert_eq!(clamp_fraction(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp_fraction(2.0, 10.0, 20.0), 0.0);
        assert_eq!(clamp_fraction(0.0, 0.0, 10.0), 0.0);
    }

    #[test]
    fn above_max_clamps_to_one() {
        assert_eq!(clamp_fraction(15.0, 0.0, 10.0), 1.0);
        assert_eq!(clamp_fraction(25.0, 10.0, 20.0), 1.0);
        assert_eq!(clamp_fraction(10.0, 0.0, 10.0), 1.0);
    }

    #[test]
    fn mid_range_maps_linearly() {
        assert_eq!(clamp_fraction(5.0, 0.0, 10.0), 0.5);
        assert_eq!(clamp_fraction(15.0, 10.0, 20.0), 0.5);
        assert_eq!(clamp_fraction(2.5, 0.0, 10.0), 0.25);
        assert_eq!(clamp_fraction(-50.0, -100.0, 100.0), 0.25);
    }

    #[test]
    fn nan_is_safe() {
        assert_eq!(clamp_fraction(f32::NAN, 0.0, 10.0), 0.0);
        assert!(clamp_fraction(5.0, 0.0, 10.0).is_finite());
    }

    #[test]
    fn empty_or_reversed_range_yields_zero() {
        assert_eq!(clamp_fraction(5.0, 3.0, 3.0), 0.0);
        assert_eq!(clamp_fraction(3.0, 3.0, 3.0), 0.0);
        assert_eq!(clamp_fraction(5.0, 10.0, 0.0), 0.0);
    }
}
