//! Axis ticks and label strips for the graph editor panel.
//!
//! `app.rs`'s `paint_graph` draws the channel curve, a zero line
//! and the playhead over a `GraphView` value/frame range, but the
//! axes carry no readable scale: nothing on screen says which
//! value a gridline is, or which frame a column belongs to. This
//! component supplies the missing scale as pure data plus one
//! small builder:
//!
//! - [`axis_ticks`] turns a value range into a short list of
//!   "nice" tick values (steps of 1, 2 or 5 times a power of ten)
//!   with human-readable labels.
//! - [`axis_label_row`] renders those labels as a flex row the
//!   panel can place under (frames) or beside (values) the graph
//!   canvas.
//!
//! Presentation only, like the rest of the kit: no listeners, no
//! canvas, no editor knowledge. The caller passes the same
//! `min`/`max` it gave the graph view.

use crate::ui_kit::*;
use gpui::*;

/// Upper bound on the requested tick count. A label strip with
/// more than a thousand entries would be unreadable (and the
/// caller is a panel a few hundred pixels wide), so larger
/// requests are clamped rather than honoured literally.
const MAX_TARGET: usize = 1000;

/// Format one tick value for display, given the tick `step`.
///
/// Labels never carry trailing zeros: when `step >= 1.0` the
/// ticks are whole numbers and print as integers (`-150`, `0`,
/// `200`); below that they print with one decimal (`0.2`, `0.6`),
/// with a trailing `.0` dropped (`1` rather than `1.0`). One
/// decimal is a deliberate panel-scale choice — steps below 0.1
/// (e.g. 0.05) can make neighbouring labels collide, which the
/// caller avoids by asking for fewer ticks on tiny ranges.
fn format_label(value: f32, step: f32) -> String {
    if value == 0.0 {
        // Also normalises -0.0.
        return "0".to_string();
    }
    if step >= 1.0 {
        format!("{}", value.round() as i64)
    } else {
        let text = format!("{value:.1}");
        match text.strip_suffix(".0") {
            Some(stripped) => stripped.to_string(),
            None => text,
        }
    }
}

/// Pick a "nice" step — 1, 2 or 5 times a power of ten — close to
/// `raw_step` on a log scale, so tick labels stay human-readable.
fn nice_step(raw_step: f32) -> f32 {
    let magnitude = 10f32.powf(raw_step.log10().floor());
    let normalised = raw_step / magnitude;
    let multiplier = if normalised < 1.5 {
        1.0
    } else if normalised < 3.5 {
        2.0
    } else if normalised < 7.5 {
        5.0
    } else {
        10.0
    };
    multiplier * magnitude
}

/// Compute axis ticks for the range `min..=max`.
///
/// Returns `(value, label)` pairs in ascending order, spaced by a
/// nice step (see [`nice_step`]) chosen so the count lands near
/// `target_count`. Ticks are the multiples of the step that fall
/// inside the range — the endpoints themselves appear only when
/// they happen to be multiples (on an angle axis of -180..=180
/// with a step of 50, the ticks are -150..=150).
///
/// Edge cases, none of which panic:
///
/// - Reversed ranges (`min > max`) are accepted and produce the
///   same ascending ticks as the swapped range.
/// - A degenerate range (`min == max`) yields a single tick at
///   that value.
/// - `target_count` is clamped to `1..=1000`; zero behaves as 1.
/// - Non-finite endpoints yield no ticks at all.
pub fn axis_ticks(min: f32, max: f32, target_count: usize) -> Vec<(f32, String)> {
    if !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }
    let lo = min.min(max);
    let hi = min.max(max);
    let target = target_count.clamp(1, MAX_TARGET);
    if lo == hi {
        return vec![(lo, format_label(lo, 1.0))];
    }
    let step = nice_step((hi - lo) / target as f32);
    // Index-based iteration (multiples of `step`) avoids the
    // drift a running `value += step` accumulation would add.
    // The small epsilon keeps a tick that lands exactly on an
    // endpoint from being lost to float division error
    // (1.0 / 0.2 is 4.9999998 in f32, which would floor to 4).
    let first = ((lo / step) - 1e-4).ceil() as i64;
    let last = ((hi / step) + 1e-4).floor() as i64;
    (first..=last)
        .map(|i| {
            let value = i as f32 * step;
            (value, format_label(value, step))
        })
        .collect()
}

/// Build a horizontal strip of the tick labels for `min..=max`,
/// spaced across the row with flexible spacers.
///
/// Approximation, stated plainly: the labels are spaced *evenly*
/// by the flex spacers, not placed at their true data positions.
/// That matches reality when the ticks divide the range evenly —
/// which ticks do by construction whenever the first and last
/// tick sit at the range ends — but when the outermost ticks fall
/// inside the range, the end labels still sit at the strip's ends,
/// slightly outside their true positions. For a small caption
/// strip under a graph this is the right trade; anything needing
/// exact placement should position labels on the canvas instead.
pub fn axis_label_row(min: f32, max: f32, target_count: usize) -> Div {
    let ticks = axis_ticks(min, max, target_count);
    let mut row = div().flex().flex_row().items_center().w_full();
    for (index, (_, label)) in ticks.iter().enumerate() {
        if index > 0 {
            row = row.child(div().flex_1());
        }
        row = row.child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(C_MUTED))
                .child(label.clone()),
        );
    }
    row
}

#[cfg(test)]
mod tests {
    use super::axis_ticks;

    fn values(ticks: &[(f32, String)]) -> Vec<f32> {
        ticks.iter().map(|(value, _)| *value).collect()
    }

    fn labels(ticks: &[(f32, String)]) -> Vec<&str> {
        ticks.iter().map(|(_, label)| label.as_str()).collect()
    }

    #[test]
    fn angle_range_gets_even_integer_ticks() {
        // The graph view fits angle channels to exactly -180..=180.
        let ticks = axis_ticks(-180.0, 180.0, 6);
        assert_eq!(
            values(&ticks),
            vec![-150.0, -100.0, -50.0, 0.0, 50.0, 100.0, 150.0]
        );
        assert_eq!(
            labels(&ticks),
            vec!["-150", "-100", "-50", "0", "50", "100", "150"]
        );
    }

    #[test]
    fn frame_range_ticks_every_frame() {
        let ticks = axis_ticks(0.0, 5.0, 5);
        assert_eq!(values(&ticks), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(labels(&ticks), vec!["0", "1", "2", "3", "4", "5"]);
    }

    #[test]
    fn decimal_range_uses_one_decimal_without_trailing_zeros() {
        let ticks = axis_ticks(0.0, 1.0, 5);
        assert_eq!(labels(&ticks), vec!["0", "0.2", "0.4", "0.6", "0.8", "1"]);
        for (value, expected) in values(&ticks)
            .iter()
            .zip([0.0, 0.2, 0.4, 0.6, 0.8, 1.0])
        {
            assert!((value - expected).abs() < 1e-6, "{value} != {expected}");
        }
    }

    #[test]
    fn large_range_labels_stay_integers() {
        let ticks = axis_ticks(0.0, 1000.0, 5);
        assert_eq!(
            values(&ticks),
            vec![0.0, 200.0, 400.0, 600.0, 800.0, 1000.0]
        );
        assert_eq!(
            labels(&ticks),
            vec!["0", "200", "400", "600", "800", "1000"]
        );
    }

    #[test]
    fn degenerate_range_is_a_single_tick() {
        assert_eq!(axis_ticks(3.0, 3.0, 5), vec![(3.0, "3".to_string())]);
        assert_eq!(axis_ticks(0.0, 0.0, 4), vec![(0.0, "0".to_string())]);
    }

    #[test]
    fn reversed_range_matches_forward_range() {
        assert_eq!(axis_ticks(180.0, -180.0, 6), axis_ticks(-180.0, 180.0, 6));
        assert_eq!(axis_ticks(5.0, 0.0, 5), axis_ticks(0.0, 5.0, 5));
    }

    #[test]
    fn zero_target_count_does_not_panic() {
        let ticks = axis_ticks(0.0, 10.0, 0);
        assert_eq!(values(&ticks), vec![0.0, 10.0]);
    }

    #[test]
    fn ticks_are_ascending_and_inside_the_range() {
        let ticks = axis_ticks(0.03, 0.97, 4);
        assert_eq!(labels(&ticks), vec!["0.2", "0.4", "0.6", "0.8"]);
        let vals = values(&ticks);
        assert!(vals.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(vals.iter().all(|v| (0.03..=0.97).contains(v)));
    }
}
