//! Tab strip building blocks.
//!
//! Data and presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`] and the
//! colour picker's swatches). [`tab_strip`] returns one styled
//! `Div` per label; the caller arranges them in a row and wires
//! each one with its own id and click listener — e.g.
//! `.id(("tab", i))` plus `.on_click(...)` — exactly how `app.rs`
//! turns [`crate::ui_color::swatch`] cells into buttons. Which
//! tab is "current" is likewise the caller's state: it passes
//! the active index in on every render.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_INK, C_MUTED};

/// Background of one tab: the accent colour for the active tab,
/// the neutral button grey (`btn()`'s resting background in
/// `app.rs`) for the rest.
pub fn tab_bg(active: bool) -> u32 {
    if active { C_ACCENT } else { 0x2C313A }
}

/// Text colour of one tab: ink for the active tab, muted for
/// the rest.
pub fn tab_fg(active: bool) -> u32 {
    if active { C_INK } else { C_MUTED }
}

/// One styled tab per label, in label order: rounded top
/// corners (the strip is meant to sit on a panel's top edge),
/// button-like padding, 12px text in [`tab_fg`] on [`tab_bg`].
///
/// The tab at index `active` is highlighted. An out-of-range
/// `active` (including on an empty `labels`) is not an error:
/// it simply highlights nothing, so a caller whose selection
/// was just deleted can keep passing its stale index until it
/// picks a new one. No ids or listeners are attached here —
/// the caller adds both.
pub fn tab_strip(labels: &[&str], active: usize) -> Vec<Div> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active;
            div()
                .px(px(10.0))
                .py(px(5.0))
                .rounded_tl(px(6.0))
                .rounded_tr(px(6.0))
                .bg(rgb(tab_bg(is_active)))
                .text_size(px(12.0))
                .text_color(rgb(tab_fg(is_active)))
                .child(label.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{tab_bg, tab_fg, tab_strip};
    use crate::ui_kit::{C_ACCENT, C_INK, C_MUTED};

    #[test]
    fn active_tab_uses_accent_background_and_ink_text() {
        assert_eq!(tab_bg(true), C_ACCENT);
        assert_eq!(tab_fg(true), C_INK);
    }

    #[test]
    fn inactive_tab_uses_neutral_background_and_muted_text() {
        assert_eq!(tab_bg(false), 0x2C313A);
        assert_eq!(tab_fg(false), C_MUTED);
    }

    #[test]
    fn strip_returns_one_tab_per_label() {
        // Construction only — the caller owns layout and wiring.
        let tabs = tab_strip(&["Pose", "Draw", "3D"], 1);
        assert_eq!(tabs.len(), 3);
    }

    #[test]
    fn strip_with_no_labels_is_empty() {
        assert!(tab_strip(&[], 0).is_empty());
    }

    #[test]
    fn out_of_range_active_highlights_nothing_and_does_not_panic() {
        // Documented clamp: a stale index is legal and simply
        // yields an all-inactive strip.
        let tabs = tab_strip(&["A", "B"], 9);
        assert_eq!(tabs.len(), 2);
    }
}
