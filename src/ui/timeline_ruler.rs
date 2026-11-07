//! Frame ruler for the timeline bar.
//!
//! A single slim strip meant to sit directly above the frame
//! chips: one cell per frame on exactly the chips' pitch (min_w
//! 26, px 6, gap 4), so every column lines up with the chip below
//! it. Labelled cells — every [`tick_step`] frames — show their
//! 1-based frame number; unlabelled cells show a small tick mark
//! instead. Cells inside the loop range are shaded, keyframe
//! cells carry a small dot, and the current frame's cell is
//! accented, mirroring the chips row's own colour language.
//!
//! Presentation only, like the rest of the kit: plain data in, a
//! `Div` out, no listeners and no editor knowledge. The caller
//! supplies the keyframe flags (any figure / project-wide — the
//! [`crate::ui_dope::keyframe_map`] rows are one source) and the
//! project loop range as stored on the editor core.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK, C_MUTED};

/// Loop-range cell shading: the dimmed accent the frame chips
/// themselves use for in-range cells, so the ruler and the chips
/// read as one block.
const C_LOOP: u32 = 0x2E4470;
/// Neutral cell background, shared with the chips and dopesheet.
const C_CELL: u32 = 0x2C313A;

/// The labelling interval for a timeline of `frame_count` frames:
/// every frame is numbered on short timelines, and the step grows
/// as the timeline does so the numbers never crowd. Never 0 — a
/// zero step would label nothing (and divide by zero).
pub fn tick_step(frame_count: usize) -> usize {
    match frame_count {
        0..=20 => 1,
        21..=100 => 5,
        101..=250 => 10,
        _ => 25,
    }
}

/// True when `frame` lies inside the loop range, inclusive at both
/// ends. The stored range may run in either direction (start set
/// after end), so it is normalised before comparing — the same
/// rule the frame chips apply to their selection shading.
fn loop_contains(loop_range: Option<(usize, usize)>, frame: usize) -> bool {
    match loop_range {
        Some((a, b)) => {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            frame >= lo && frame <= hi
        }
        None => false,
    }
}

/// The ruler strip: one cell per frame in `0..frame_count`.
///
/// Each cell stacks a label row over a dot row. The label row shows
/// the 1-based frame number (10px) on labelled cells — frame 0 and
/// every [`tick_step`]-th frame after it — and a centred tick mark
/// on the rest; the dot row shows a 4px dot on keyframe cells
/// (`keyframes` may be shorter than the timeline or empty; missing
/// entries read as "not a keyframe").
///
/// Backgrounds follow the chips: accent for the current frame,
/// [`C_LOOP`] inside the loop range, neutral otherwise. Text and
/// dots use the ink colour on the current (accent) cell so they
/// stay visible, exactly as the dopesheet treats its current
/// column; elsewhere labels are muted and dots are accent.
pub fn ruler(
    frame_count: usize,
    current: usize,
    loop_range: Option<(usize, usize)>,
    keyframes: &[bool],
) -> Div {
    let step = tick_step(frame_count);
    let mut row = div().flex().flex_row().gap(px(4.0));
    for f in 0..frame_count {
        let is_current = f == current;
        let bg = if is_current {
            C_ACCENT
        } else if loop_contains(loop_range, f) {
            C_LOOP
        } else {
            C_CELL
        };
        let fg = if is_current { C_INK } else { C_MUTED };
        // Label row: fixed height so labelled and tick-only cells
        // keep the same rhythm down the strip.
        let label = if f % step == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(14.0))
                .text_size(px(10.0))
                .text_color(rgb(fg))
                .child(format!("{}", f + 1))
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(14.0))
                .child(div().w(px(1.0)).h(px(5.0)).bg(rgb(C_BORDER)))
        };
        // Dot row: the slot is always present (fixed height) so a
        // keyframe dot never changes the cell's height.
        let mut dot_row = div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(6.0));
        if keyframes.get(f).copied().unwrap_or(false) {
            let dot = if is_current { C_INK } else { C_ACCENT };
            dot_row = dot_row.child(
                div()
                    .w(px(4.0))
                    .h(px(4.0))
                    .rounded(px(999.0))
                    .bg(rgb(dot)),
            );
        }
        row = row.child(
            div()
                .flex_col()
                .items_center()
                .justify_center()
                .min_w(px(26.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(6.0))
                .bg(rgb(bg))
                .child(label)
                .child(dot_row),
        );
    }
    row
}

#[cfg(test)]
mod tests {
    use super::{loop_contains, tick_step};

    #[test]
    fn tick_step_numbers_every_frame_on_short_timelines() {
        assert_eq!(tick_step(0), 1);
        assert_eq!(tick_step(1), 1);
        assert_eq!(tick_step(20), 1);
    }

    #[test]
    fn tick_step_grows_at_each_boundary() {
        assert_eq!(tick_step(21), 5);
        assert_eq!(tick_step(100), 5);
        assert_eq!(tick_step(101), 10);
        assert_eq!(tick_step(250), 10);
        assert_eq!(tick_step(251), 25);
        assert_eq!(tick_step(10_000), 25);
    }

    #[test]
    fn tick_step_is_never_zero() {
        for n in [0, 1, 19, 20, 21, 99, 100, 101, 249, 250, 251, usize::MAX] {
            assert!(tick_step(n) >= 1, "tick_step({n}) must be >= 1");
        }
    }

    #[test]
    fn loop_contains_is_inclusive_and_direction_agnostic() {
        assert!(loop_contains(Some((2, 5)), 2));
        assert!(loop_contains(Some((2, 5)), 5));
        assert!(loop_contains(Some((2, 5)), 3));
        assert!(!loop_contains(Some((2, 5)), 1));
        assert!(!loop_contains(Some((2, 5)), 6));
        // Stored backwards, the same span matches.
        assert!(loop_contains(Some((5, 2)), 3));
        assert!(!loop_contains(Some((5, 2)), 6));
        // A single-frame range contains exactly that frame.
        assert!(loop_contains(Some((4, 4)), 4));
        assert!(!loop_contains(Some((4, 4)), 3));
    }

    #[test]
    fn loop_contains_without_a_range_is_always_false() {
        assert!(!loop_contains(None, 0));
        assert!(!loop_contains(None, 100));
    }
}
