//! Labelled stepper rows: a caption, a − button, the current
//! value, a + button.
//!
//! The weather panel in `app.rs` grew the first of these inline —
//! a closure that built the same row twice (Amount, Wind), each
//! time wiring its own buttons to a backing step helper. This
//! component owns the row's *shape* so other panels can stop
//! re-deriving it: the fixed-width muted label, the centred value,
//! the 6px gaps. Following the layer-row split, the buttons are
//! built by the caller and handed in as [`Div`]s — every button
//! carries a click listener bound to editor state, and listeners
//! stay with the caller. No ids, no handlers, no editor knowledge
//! live here.
//!
//! [`clamp_step`] is the pure half of the pattern: the arithmetic
//! a caller's step handler applies before writing a value back
//! (the weather helpers clamp amount to 0..=200, say). Keeping it
//! here, next to the row it serves, means every stepper clamps
//! the same way.

use gpui::*;

use crate::ui_kit::{C_INK, C_MUTED};

/// One clamped step: `value + delta`, held inside `min..=max`.
///
/// NaN never reaches the result. A NaN `delta` is ignored, so the
/// step reads as "value unchanged, then clamped". A NaN `value`
/// has no meaningful "unchanged" reading — the control would show
/// nothing — so it is treated as `min` before the delta applies,
/// and the result is always a real number inside the range. When
/// the range itself is empty (`min > max`) the result is `min`:
/// the floor is the only defensible answer, and callers passing a
/// reversed range get that instead of a panic.
pub fn clamp_step(value: f32, delta: f32, min: f32, max: f32) -> f32 {
    if min > max {
        return min;
    }
    let base = if value.is_nan() { min } else { value };
    let step = if delta.is_nan() { 0.0 } else { delta };
    let stepped = base + step;
    if stepped < min {
        min
    } else if stepped > max {
        max
    } else {
        stepped
    }
}

/// The stepper row: `label`, then the caller's `minus` button,
/// then `value`, then the caller's `plus` button.
///
/// Metrics generalise the weather panel's inline rows: a flex row,
/// children centred vertically, 6px gaps. The label is muted 11px
/// on a fixed 46px width so the buttons of stacked rows line up
/// into a column; the value is 12px ink, centred in a flexible
/// slot at least 36px wide so short readouts ("90", "-0.5") don't
/// make the + button jump as the value changes length. The label
/// text is copied into an owned `String` — a `Div` keeps its text
/// for as long as it lives, so borrowed text cannot be stored.
pub fn stepper_row(label: &str, value: String, minus: Div, plus: Div) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .w(px(46.0))
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(label.to_string()),
        )
        .child(minus)
        .child(
            div()
                .flex()
                .justify_center()
                .min_w(px(36.0))
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(value),
        )
        .child(plus)
}

#[cfg(test)]
mod tests {
    // Exact names only: this file glob-imports gpui::*, whose own
    // `test` proc-macro would shadow #[test] if the test module
    // re-imported the parent with a glob (that bug cost 4 CI runs).
    use super::clamp_step;

    #[test]
    fn mid_range_steps_freely() {
        assert_eq!(clamp_step(5.0, 2.0, 0.0, 10.0), 7.0);
        assert_eq!(clamp_step(5.0, -2.0, 0.0, 10.0), 3.0);
        assert_eq!(clamp_step(5.0, 0.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp_step(-4.0, 1.5, -10.0, 10.0), -2.5);
    }

    #[test]
    fn steps_clamp_at_both_ends() {
        assert_eq!(clamp_step(1.0, -5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp_step(9.0, 5.0, 0.0, 10.0), 10.0);
        // Landing exactly on a bound is not clamping.
        assert_eq!(clamp_step(8.0, 2.0, 0.0, 10.0), 10.0);
        assert_eq!(clamp_step(2.0, -2.0, 0.0, 10.0), 0.0);
        // Already outside the range, a zero step pulls it back in.
        assert_eq!(clamp_step(-3.0, 0.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp_step(13.0, 0.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn nan_never_reaches_the_result() {
        // NaN delta: value unchanged, then clamped.
        assert_eq!(clamp_step(5.0, f32::NAN, 0.0, 10.0), 5.0);
        assert_eq!(clamp_step(50.0, f32::NAN, 0.0, 10.0), 10.0);
        // NaN value: treated as the range floor before stepping.
        assert_eq!(clamp_step(f32::NAN, 0.0, 2.0, 9.0), 2.0);
        assert_eq!(clamp_step(f32::NAN, 1.0, 0.0, 10.0), 1.0);
        assert_eq!(clamp_step(f32::NAN, f32::NAN, 0.0, 10.0), 0.0);
    }

    #[test]
    fn reversed_range_returns_min() {
        assert_eq!(clamp_step(5.0, 1.0, 10.0, 0.0), 10.0);
        assert_eq!(clamp_step(5.0, -1.0, 10.0, 0.0), 10.0);
        assert_eq!(clamp_step(f32::NAN, f32::NAN, 10.0, 0.0), 10.0);
    }
}
