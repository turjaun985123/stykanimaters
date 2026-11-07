//! Level gauge: a thin meter bar whose fill colour shifts with
//! the level it shows — accent while things are normal, amber as
//! the level climbs, red when it is nearly full.
//!
//! Presentation only — no state, no handlers, no editor knowledge
//! (the same contract as [`crate::ui_kit`]). The caller computes a
//! fraction (memory in use, export progress, audio level, …) and
//! this component only draws it. Range handling is not reinvented
//! here: fractions are sanitised with
//! [`crate::ui_slider::clamp_fraction`], the same helper the
//! slider uses, so out-of-range and NaN inputs are safe to pass.
//!
//! Two pieces:
//! - [`level_colors`] is the pure part: the fill colour for a
//!   fraction, following the warn/error tones of
//!   `crate::ui_badge`.
//! - [`gauge`] / [`gauge_labeled`] are the pictures: an 8px rounded
//!   bar, optionally with a caption and a percentage readout.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_INK, C_MUTED};
use crate::ui_slider::clamp_fraction;

/// Thickness of the meter bar.
const BAR_H: f32 = 8.0;
/// The kit's neutral well colour (the inactive chip background in
/// `ui_kit::info_chip`), used for the unfilled track.
const TRACK_BG: u32 = 0x2C313A;
/// Amber fill once the level reaches 70% — the warn tone from
/// `crate::ui_badge`.
const LEVEL_WARN: u32 = 0x8A6D1F;
/// Red fill once the level passes 90% — the error tone from
/// `crate::ui_badge`.
const LEVEL_ERROR: u32 = 0xB23A3A;
/// First fraction drawn in the warn colour.
const WARN_AT: f32 = 0.7;
/// First fraction drawn in the error colour (exclusive: exactly
/// 0.9 is still warn).
const ERROR_AT: f32 = 0.9;

/// The fill colour for a level fraction: [`C_ACCENT`] below 0.7,
/// the warn tone for `0.7..=0.9`, and the error tone above 0.9.
/// A NaN fraction reads as 0, so a broken input shows an empty,
/// calm meter rather than a red one.
pub fn level_colors(frac: f32) -> u32 {
    let f = if frac.is_nan() { 0.0 } else { frac };
    if f > ERROR_AT {
        LEVEL_ERROR
    } else if f >= WARN_AT {
        LEVEL_WARN
    } else {
        C_ACCENT
    }
}

/// The meter bar for a fraction in `0.0..=1.0`: a full-width, 8px
/// rounded track with a fill whose width is the fraction of the
/// track and whose colour comes from [`level_colors`]. The input
/// is sanitised through [`clamp_fraction`], so callers may pass a
/// raw computed value without pre-checking it.
pub fn gauge(frac: f32) -> Div {
    let f = clamp_fraction(frac, 0.0, 1.0);
    div()
        .w_full()
        .h(px(BAR_H))
        .rounded(px(999.0))
        .bg(rgb(TRACK_BG))
        .child(
            div()
                .h(px(BAR_H))
                .w(relative(f))
                .rounded(px(999.0))
                .bg(rgb(level_colors(f))),
        )
}

/// The meter with a caption and a percentage: a fixed-width muted
/// label on the left, the [`gauge`] bar stretching across the
/// middle, and the clamped percentage in ink on the right — the
/// row shape panels use for "label, level, number" readouts.
pub fn gauge_labeled(label: &str, frac: f32) -> Div {
    let f = clamp_fraction(frac, 0.0, 1.0);
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
        .child(div().flex_1().child(gauge(f)))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_INK))
                .child(format!("{:.0}%", f * 100.0)),
        )
}

#[cfg(test)]
mod tests {
    use super::level_colors;
    use crate::ui_kit::C_ACCENT;

    #[test]
    fn normal_levels_use_the_accent() {
        assert_eq!(level_colors(0.0), C_ACCENT);
        assert_eq!(level_colors(0.5), C_ACCENT);
        assert_eq!(level_colors(0.69), C_ACCENT);
    }

    #[test]
    fn warn_band_starts_at_point_seven() {
        assert_eq!(level_colors(0.7), 0x8A6D1F);
        assert_eq!(level_colors(0.8), 0x8A6D1F);
        assert_eq!(level_colors(0.9), 0x8A6D1F);
    }

    #[test]
    fn error_band_is_above_point_nine() {
        assert_eq!(level_colors(0.91), 0xB23A3A);
        assert_eq!(level_colors(1.0), 0xB23A3A);
    }

    #[test]
    fn nan_reads_as_zero() {
        assert_eq!(level_colors(f32::NAN), C_ACCENT);
    }
}
