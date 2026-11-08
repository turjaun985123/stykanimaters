//! Timeline clip row: one entry in a clip lane (FX clips, audio
//! clips) showing the clip's name and a bar positioned over the
//! project timeline.
//!
//! Presentation only, like the other components in this folder:
//! the caller converts each clip into a plain [`ClipRowData`]
//! (name, kind label, colour, and the clip's start/end as
//! fractions of the whole timeline) and [`clip_row`] lays it out.
//! No listeners, no editor knowledge.
//!
//! Span rendering: GPUI has no percentage-margin idiom in this
//! codebase, so the track is a full-width flex row of three
//! segments whose widths are definite fractions of the track
//! (`relative(..)`): an invisible left spacer, the coloured span,
//! and an invisible right spacer. The three fractions always sum
//! to 1.0, so the span lands exactly over the frames the clip
//! covers at any lane width.

use gpui::*;

use crate::ui_kit::{C_CELL, C_INK, C_MUTED, TEXT_XS};

/// Smallest span width, as a fraction of the timeline, that a
/// clip row will draw — a zero-length (or nearly zero-length)
/// clip still shows a visible sliver instead of vanishing.
pub const MIN_SPAN_FRAC: f32 = 0.01;

/// Display-ready data for one clip row. `start_frac` / `end_frac`
/// are the clip's start and end frame divided by the timeline's
/// frame count; out-of-range values are tolerated — [`span`]
/// clamps them when the row is built.
pub struct ClipRowData {
    /// Clip name as shown at the left of the header line.
    pub name: String,
    /// Short kind label ("burst", "audio", …) shown muted after
    /// the name.
    pub kind_label: String,
    /// Bar colour, `0xRRGGBB`.
    pub colour: u32,
    /// Clip start as a fraction of the timeline (0.0–1.0).
    pub start_frac: f32,
    /// Clip end as a fraction of the timeline (0.0–1.0).
    pub end_frac: f32,
}

/// Normalise a clip's start/end fractions into a `(left, width)`
/// pair for the track: both ends are clamped into `0.0..=1.0`
/// (non-finite values read as 0.0), a reversed pair is ordered,
/// and the width is raised to at least [`MIN_SPAN_FRAC`] —
/// shifting the span left if that minimum would push it past the
/// end of the track. The result always satisfies
/// `0.0 <= left`, `MIN_SPAN_FRAC <= width` and
/// `left + width <= 1.0`.
pub fn span(start: f32, end: f32) -> (f32, f32) {
    let clamp = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let (a, b) = (clamp(start), clamp(end));
    let mut left = a.min(b);
    let mut width = (a - b).abs();
    if width < MIN_SPAN_FRAC {
        width = MIN_SPAN_FRAC;
        if left + width > 1.0 {
            left = 1.0 - width;
        }
    }
    (left, width)
}

/// One clip row: a header line (name in ink, kind label muted)
/// over an 8px track in the neutral lane colour, with the clip's
/// coloured span positioned by its timeline fractions.
pub fn clip_row(data: &ClipRowData) -> Div {
    let (left, width) = span(data.start_frac, data.end_frac);
    let right = (1.0 - left - width).max(0.0);
    div()
        .flex_col()
        .gap(px(2.0))
        .w_full()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(C_INK))
                        .child(data.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(TEXT_XS))
                        .text_color(rgb(C_MUTED))
                        .child(data.kind_label.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .h(px(8.0))
                .rounded(px(4.0))
                .bg(rgb(C_CELL))
                .child(div().w(relative(left)).flex_shrink_0())
                .child(
                    div()
                        .w(relative(width))
                        .h_full()
                        .rounded(px(4.0))
                        .flex_shrink_0()
                        .bg(rgb(data.colour)),
                )
                .child(div().w(relative(right)).flex_shrink_0()),
        )
}

#[cfg(test)]
mod tests {
    use super::span;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn span_passes_through_a_normal_range() {
        let (left, width) = span(0.25, 0.75);
        assert!(close(left, 0.25) && close(width, 0.5), "{left} {width}");
    }

    #[test]
    fn span_clamps_to_the_track() {
        let (left, width) = span(-0.5, 1.5);
        assert!(close(left, 0.0) && close(width, 1.0), "{left} {width}");
    }

    #[test]
    fn span_orders_a_reversed_pair() {
        let (left, width) = span(0.75, 0.25);
        assert!(close(left, 0.25) && close(width, 0.5), "{left} {width}");
    }

    #[test]
    fn span_gives_a_zero_length_clip_the_minimum_width() {
        let (left, width) = span(0.5, 0.5);
        assert!(close(left, 0.5) && close(width, 0.01), "{left} {width}");
    }

    #[test]
    fn span_minimum_width_shifts_left_at_the_right_edge() {
        let (left, width) = span(0.999, 1.0);
        assert!(close(width, 0.01), "{width}");
        assert!(close(left + width, 1.0), "{left} {width}");
    }

    #[test]
    fn span_treats_non_finite_values_as_zero() {
        let (left, width) = span(f32::NAN, 0.5);
        assert!(close(left, 0.0) && close(width, 0.5), "{left} {width}");
        let (left, width) = span(f32::INFINITY, f32::NEG_INFINITY);
        assert!(close(left, 0.0) && close(width, 0.01), "{left} {width}");
    }
}
