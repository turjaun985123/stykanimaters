//! Sparkline: a tiny trend strip for a series of numbers.
//!
//! Panels and the HUD often want the *shape* of a series —
//! per-frame durations, audio levels, keyframe density —
//! without a full graph panel. A sparkline is that shape in
//! miniature: one polyline on a 64×24 canvas, no axes, no
//! labels, no numbers.
//!
//! Presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`]). The
//! caller hands over plain `f32` values (frame durations,
//! peak levels, counts cast to `f32`, …); scaling lives in
//! the pure [`normalise`] helper, and the canvas follows the
//! `curve_canvas` structure in [`crate::ui_easing`].

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_MUTED};

/// Map `values` onto `0.0..=1.0` by their own min/max: the
/// minimum maps to 0.0, the maximum to 1.0, everything else
/// linearly in between. Output length always equals input
/// length.
///
/// Degenerate cases, by design:
///
/// - Empty input returns an empty vector.
/// - Constant input (min == max, including a single value)
///   has no range to stretch, so every output is 0.5 — the
///   strip draws as a flat mid-height line.
/// - `NaN` is treated as the minimum: it is ignored while
///   finding the range, then scaled as if it were the
///   minimum value itself (so it lands on 0.0, or on 0.5
///   when the rest of the input is constant). If *every*
///   value is `NaN` there is no range at all, and — like
///   constant input — every output is 0.5.
pub fn normalise(values: &[f32]) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values {
        if v.is_nan() {
            continue;
        }
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    // No non-NaN value was found, or the range is a single
    // point: nothing to stretch, everything sits mid-height.
    if min > max || min == max {
        return vec![0.5; values.len()];
    }
    let span = max - min;
    values
        .iter()
        .map(|&v| {
            let v = if v.is_nan() { min } else { v };
            (v - min) / span
        })
        .collect()
}

/// A 64×24 canvas drawing `values` as a sparkline: a faint
/// baseline (the series minimum) in `C_BORDER`, and the
/// polyline itself in `C_ACCENT` when `accent` is set, or
/// `C_MUTED` for a quieter, secondary strip.
pub fn sparkline_canvas(values: Vec<f32>, accent: bool) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, _state, window, _cx| {
            paint_sparkline(window, bounds, &values, accent);
        },
    )
    .w(px(64.0))
    .h(px(24.0))
}

/// Paint one sparkline into `bounds`, following the easing
/// curve canvas's idiom (`PathBuilder` + `window.paint_path`).
fn paint_sparkline(window: &mut Window, bounds: Bounds<Pixels>, values: &[f32], accent: bool) {
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    // Inset so the strokes never clip the canvas edge.
    let pad = 2.0;
    let (x0, x1) = (pad, w - pad);
    let (y_top, y_bot) = (pad, h - pad);
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    // Baseline: the bottom edge of the plot area, i.e. the
    // series minimum after normalisation.
    let mut base = PathBuilder::stroke(px(1.0));
    base.move_to(at(x0, y_bot));
    base.line_to(at(x1, y_bot));
    if let Ok(path) = base.build() {
        window.paint_path(path, rgb(C_BORDER));
    }
    let pts = normalise(values);
    if pts.is_empty() {
        return;
    }
    // The polyline: values spread evenly left to right,
    // heights bottom (0.0) to top (1.0) — y flipped, screen
    // y grows downward. A lone value has no span to spread
    // over, so it draws as a flat line across the strip.
    let mut pb = PathBuilder::stroke(px(1.5));
    if pts.len() == 1 {
        let y = y_bot - pts[0] * (y_bot - y_top);
        pb.move_to(at(x0, y));
        pb.line_to(at(x1, y));
    } else {
        let last = (pts.len() - 1) as f32;
        for (i, &v) in pts.iter().enumerate() {
            let p = at(
                x0 + (i as f32 / last) * (x1 - x0),
                y_bot - v * (y_bot - y_top),
            );
            if i == 0 {
                pb.move_to(p);
            } else {
                pb.line_to(p);
            }
        }
    }
    if let Ok(path) = pb.build() {
        window.paint_path(path, rgb(if accent { C_ACCENT } else { C_MUTED }));
    }
}

#[cfg(test)]
mod tests {
    use super::normalise;

    #[test]
    fn normalise_empty_is_empty() {
        assert_eq!(normalise(&[]), Vec::<f32>::new());
    }

    #[test]
    fn normalise_maps_min_to_zero_and_max_to_one() {
        assert_eq!(normalise(&[0.0, 5.0, 10.0]), vec![0.0, 0.5, 1.0]);
        assert_eq!(normalise(&[-10.0, 0.0, 10.0]), vec![0.0, 0.5, 1.0]);
        assert_eq!(normalise(&[10.0, 0.0]), vec![1.0, 0.0]);
    }

    #[test]
    fn normalise_preserves_length() {
        let values = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0];
        assert_eq!(normalise(&values).len(), values.len());
    }

    #[test]
    fn normalise_constant_input_is_all_half() {
        assert_eq!(normalise(&[2.0, 2.0, 2.0]), vec![0.5, 0.5, 0.5]);
        assert_eq!(normalise(&[7.0]), vec![0.5]);
    }

    #[test]
    fn normalise_nan_is_treated_as_the_minimum() {
        // The NaN is ignored for the range (1..=3) and then
        // scaled as if it were the minimum itself.
        assert_eq!(normalise(&[1.0, f32::NAN, 3.0]), vec![0.0, 0.0, 1.0]);
        // With an otherwise constant series the NaN follows
        // the constant rule and also lands mid-height.
        assert_eq!(normalise(&[2.0, f32::NAN, 2.0]), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn normalise_all_nan_is_all_half() {
        assert_eq!(normalise(&[f32::NAN, f32::NAN]), vec![0.5, 0.5]);
    }
}
