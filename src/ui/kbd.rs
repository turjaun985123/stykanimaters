//! Keyboard key-cap chips and shortcut rows.
//!
//! The help panel ([`crate::ui_help`]) lists every keyboard
//! shortcut, but renders the key label as a plain rounded pill.
//! This module gives those labels a proper key-cap look: a small
//! squared chip that reads as a physical key, plus a ready-made
//! [`shortcut_row`] that drops into the help panel in place of
//! its private pill row. Callers building labels from parts use
//! [`combo_text`] to join them the way the shortcut data already
//! spells them (`"Ctrl+Shift+Z"`).
//!
//! Everything here is presentation only: plain `Div` builders
//! with no state and no handlers, in the [`crate::ui_kit`] style.
//!
//! One honest limitation: GPUI 0.2.2 borders take a single colour
//! per element — there is no per-side border colour — so the
//! key-cap's heavier bottom edge (the side of a real key that
//! catches the shadow) cannot be drawn darker than the other
//! edges. [`kbd`] fakes it with width instead: the bottom border
//! is 2px where the rest are 1px, same [`C_BORDER`] colour. The
//! cap is otherwise flat: panel background, one border colour,
//! 4px corners.

use gpui::*;

use crate::ui_kit::{C_BORDER, C_INK, C_MUTED, C_PANEL};

/// Join the parts of a key combination into its display label,
/// separated with `+`: `["Ctrl", "Shift", "Z"]` becomes
/// `"Ctrl+Shift+Z"`. Parts are used exactly as given (no
/// trimming or case changes), a single part comes back as-is,
/// and an empty slice yields an empty string.
pub fn combo_text(parts: &[&str]) -> String {
    parts.join("+")
}

/// One keyboard key as a small key-cap chip: panel background,
/// a 1px [`C_BORDER`] frame with a 2px bottom edge (see the
/// module docs for why the bottom edge is thicker, not darker),
/// 4px corners, and the label in 11px monospace ink. Padding
/// matches the help panel's existing key pill (6px × 2px), so
/// swapping pills for caps does not change the row height.
pub fn kbd(key: &str) -> Div {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(rgb(C_PANEL))
        .border_1()
        // After border_1 on purpose: overrides just the bottom
        // width, giving the cap its heavier bottom edge.
        .border_b_2()
        .border_color(rgb(C_BORDER))
        .font_family("monospace")
        .text_size(px(11.0))
        .text_color(rgb(C_INK))
        .child(key.to_string())
}

/// One keyboard-shortcut row for the help panel: a [`kbd`] chip
/// holding the key label, then the action it performs. Metrics
/// match the help panel's existing rows exactly — a flex row,
/// centred items, 6px gap; the action takes the rest of the row
/// in 12px muted text — so the panel can adopt this builder
/// without its layout shifting.
pub fn shortcut_row(key: &str, action: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(kbd(key))
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child(action.to_string()),
        )
}

#[cfg(test)]
mod tests {
    use super::combo_text;

    #[test]
    fn combo_text_joins_parts_with_plus() {
        assert_eq!(combo_text(&["Ctrl", "Z"]), "Ctrl+Z");
        assert_eq!(combo_text(&["Ctrl", "Shift", "Z"]), "Ctrl+Shift+Z");
        assert_eq!(combo_text(&["Ctrl", "Alt", "Shift", "N"]), "Ctrl+Alt+Shift+N");
    }

    #[test]
    fn combo_text_single_part_is_the_part_itself() {
        assert_eq!(combo_text(&["Space"]), "Space");
        assert_eq!(combo_text(&["N"]), "N");
    }

    #[test]
    fn combo_text_empty_slice_is_empty() {
        assert_eq!(combo_text(&[]), "");
    }

    #[test]
    fn combo_text_uses_parts_verbatim() {
        // No trimming or re-casing: labels like "Any character
        // key" keep their spaces, and an empty part still earns
        // its separator.
        assert_eq!(combo_text(&["Any character key"]), "Any character key");
        assert_eq!(combo_text(&["ctrl", "z"]), "ctrl+z");
        assert_eq!(combo_text(&["Ctrl", ""]), "Ctrl+");
    }
}
