//! Empty-state placeholders for panels and lists with no content.
//!
//! When a panel has nothing to show yet — no saved projects, no FX
//! clips, no search results — an empty area reads as a bug. These
//! builders give callers a consistent, quiet placeholder instead:
//! a large muted glyph, a short title saying what is missing, and an
//! optional smaller hint saying how to fix it. Presentation only:
//! no listeners, no editor types, the caller supplies all strings.

use crate::ui_kit::{C_BORDER, C_INK, C_MUTED};
use gpui::*;

/// Glyph used when the caller passes a blank one.
pub const DEFAULT_GLYPH: &str = "○";
/// Title used when the caller passes a blank one.
pub const DEFAULT_TITLE: &str = "Nothing here yet";

/// True when `s` is empty or only whitespace.
///
/// The builders use this to fall back to [`DEFAULT_GLYPH`] /
/// [`DEFAULT_TITLE`] for blank inputs, and to omit the hint line
/// entirely when the hint is blank.
pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// A centred empty-state block: large muted glyph, title, hint.
///
/// `glyph` is any single character or short string the caller picks
/// for the context ("○" for projects, "♪" for audio, "▢" for clips);
/// a blank glyph falls back to [`DEFAULT_GLYPH`], and a blank title
/// to [`DEFAULT_TITLE`]. A blank `hint` omits the hint line rather
/// than showing a placeholder for it.
///
/// The box uses a solid `border_1` in `C_BORDER`: GPUI has no dashed
/// border style, so the classic dashed "drop zone" outline is
/// approximated with the kit's plain border.
pub fn empty_state(glyph: &str, title: &str, hint: &str) -> Div {
    let glyph = if is_blank(glyph) { DEFAULT_GLYPH } else { glyph };
    let title = if is_blank(title) { DEFAULT_TITLE } else { title };

    let mut block = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .p(px(24.0))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .child(
            div()
                .text_size(px(32.0))
                .text_color(rgb(C_MUTED))
                .child(glyph.to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(C_INK))
                .child(title.to_string()),
        );

    if !is_blank(hint) {
        block = block.child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(hint.to_string()),
        );
    }

    block
}

/// Compact one-line empty state: a single muted, centred line.
///
/// For narrow slots (a sidebar list, one row of a panel) where the
/// full [`empty_state`] block would be too tall. A blank `text`
/// falls back to [`DEFAULT_TITLE`].
pub fn empty_line(text: &str) -> Div {
    let text = if is_blank(text) { DEFAULT_TITLE } else { text };
    div()
        .flex()
        .items_center()
        .justify_center()
        .p(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(text.to_string()),
        )
}

#[cfg(test)]
mod tests {
    use super::is_blank;

    #[test]
    fn blank_means_empty_or_whitespace_only() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\t\n "));
    }

    #[test]
    fn any_visible_character_is_not_blank() {
        assert!(!is_blank("x"));
        assert!(!is_blank(" x "));
        assert!(!is_blank("○"));
    }
}
