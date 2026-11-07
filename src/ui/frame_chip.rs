//! A single frame chip for the timeline bar.
//!
//! The numbered cell an animator clicks to jump to a frame. This
//! component carries the frame chips' exact pitch metrics (min_w
//! 26, px 6, rounded 6 — the caller lays chips out with gap 4, so
//! columns line up with the ruler and dopesheet strips above them)
//! and extends the chips' colour language with a keyframe state:
//! a chip whose frame holds a keyframe shows its number in the
//! accent colour and a small dot under it.
//!
//! Presentation only, like the rest of the kit: plain data in, a
//! `Div` out, no listeners and no element id inside. The caller
//! attaches `.id(("frame", i))` and the click handler, exactly as
//! it does for the colour picker's swatches.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_INK, C_MUTED};

/// Loop/selection-range chip background: the dimmed accent the
/// frame chips use for in-range cells.
const C_RANGE: u32 = 0x2E4470;
/// Neutral chip background, shared with the chips and dopesheet.
const C_CELL: u32 = 0x2C313A;

/// How one frame chip should read, decided by the caller from the
/// frame's position: the playhead, the selection/loop range, and
/// the project's keyframe flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipState {
    /// An ordinary frame.
    Normal,
    /// The current frame (playhead).
    Current,
    /// Inside the selection or loop range, but not current.
    InRange,
    /// Holds a keyframe (any figure changes pose here).
    Keyframe,
    /// The current frame, and it also holds a keyframe.
    CurrentKeyframe,
}

/// The (background, foreground) pair for a chip state. Pure, so
/// the whole colour language is testable without a window.
///
/// Current frames take the accent background with ink text — the
/// brightest cell on the bar. In-range frames keep ink text on the
/// dimmed accent. A keyframe keeps the neutral background but its
/// number turns accent, so keys stand out without shouting; a
/// plain frame is neutral with muted text.
pub fn chip_colors(state: ChipState) -> (u32, u32) {
    match state {
        ChipState::Current | ChipState::CurrentKeyframe => (C_ACCENT, C_INK),
        ChipState::InRange => (C_RANGE, C_INK),
        ChipState::Keyframe => (C_CELL, C_ACCENT),
        ChipState::Normal => (C_CELL, C_MUTED),
    }
}

/// One frame chip showing `number` — the 1-based frame number, as
/// the chips display it (the caller adds 1 to the frame index).
///
/// The chip stacks the number over a dot slot of fixed height, the
/// same trick as the timeline ruler: keyframe states
/// ([`ChipState::Keyframe`], [`ChipState::CurrentKeyframe`]) fill
/// the slot with a 4px dot in the foreground colour, other states
/// leave it empty, and no state's chip is taller than another's.
pub fn frame_chip(number: usize, state: ChipState) -> Div {
    let (bg, fg) = chip_colors(state);
    let mut dot_row = div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(6.0));
    if matches!(state, ChipState::Keyframe | ChipState::CurrentKeyframe) {
        dot_row = dot_row.child(
            div()
                .w(px(4.0))
                .h(px(4.0))
                .rounded(px(999.0))
                .bg(rgb(fg)),
        );
    }
    div()
        .flex_col()
        .items_center()
        .justify_center()
        .min_w(px(26.0))
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(6.0))
        .bg(rgb(bg))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(16.0))
                .text_size(px(12.0))
                .text_color(rgb(fg))
                .child(format!("{number}")),
        )
        .child(dot_row)
}

#[cfg(test)]
mod tests {
    use super::{chip_colors, ChipState};

    #[test]
    fn normal_chip_is_neutral_with_muted_text() {
        assert_eq!(chip_colors(ChipState::Normal), (0x2C313A, 0x9AA3B2));
    }

    #[test]
    fn current_chip_is_accent_with_ink_text() {
        assert_eq!(chip_colors(ChipState::Current), (0x3D5A99, 0xE8E8EC));
    }

    #[test]
    fn in_range_chip_is_dimmed_accent_with_ink_text() {
        assert_eq!(chip_colors(ChipState::InRange), (0x2E4470, 0xE8E8EC));
    }

    #[test]
    fn keyframe_chip_is_neutral_with_accent_text() {
        assert_eq!(chip_colors(ChipState::Keyframe), (0x2C313A, 0x3D5A99));
    }

    #[test]
    fn current_keyframe_chip_matches_current() {
        assert_eq!(
            chip_colors(ChipState::CurrentKeyframe),
            (0x3D5A99, 0xE8E8EC)
        );
        assert_eq!(
            chip_colors(ChipState::CurrentKeyframe),
            chip_colors(ChipState::Current)
        );
    }
}
