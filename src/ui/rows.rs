//! Reusable row layouts for panel and sidebar content.
//!
//! Panels across the editor repeat the same few row shapes: a muted
//! caption with a value beside it, and a section caption with small
//! action elements trailing on the right. Those shapes live here so
//! every panel lays its rows out identically instead of re-deriving
//! the metrics per file.
//!
//! Everything here is presentation only: plain `Div` builders with
//! no state and no handlers. Interactive elements (buttons,
//! steppers, swatches) are built by the caller and passed in —
//! [`labeled_row`] takes the value element, [`section_header`] takes
//! the trailing action elements — so listeners stay with the caller,
//! exactly like the inspector panel's appended steppers.

use crate::ui_kit::{section_label, C_INK, C_MUTED};
use gpui::*;

/// Maximum number of characters shown for a row label before it is
/// truncated with an ellipsis. Row labels are short captions
/// ("Length", "Weather"); anything longer would squeeze the value
/// out of a narrow panel, so it is clipped instead.
pub const MAX_LABEL_CHARS: usize = 24;

/// Truncate `label` to at most `max_chars` characters (counted as
/// Unicode scalar values, never splitting a character), appending
/// "…" when anything was cut. A `max_chars` of 0 yields an empty
/// string; 1 yields just the ellipsis.
pub fn clamp_label(label: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if label.chars().count() <= max_chars {
        return label.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let kept: String = label.chars().take(max_chars - 1).collect();
    format!("{kept}…")
}

/// The muted caption half of a row, shared by [`labeled_row`] and
/// [`value_row`] so both clip and style their labels identically.
fn row_label(label: &str) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(rgb(C_MUTED))
        .child(clamp_label(label, MAX_LABEL_CHARS))
}

/// A row with a muted label on the left and a caller-built value
/// element on the right, pushed apart by a flexible spacer. Use this
/// when the value is more than plain text — a chip, a colour dot, a
/// stepper pair. For a plain text value, [`value_row`] is shorter.
pub fn labeled_row(label: &str, value: Div) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(row_label(label))
        .child(div().flex_1())
        .child(value)
}

/// A row with a muted label on the left and a plain text value on
/// the right, in the standard 12px ink style. This is the common
/// read-only property row used across the panels.
pub fn value_row(label: &str, text: String) -> Div {
    labeled_row(
        label,
        div()
            .text_size(px(12.0))
            .text_color(rgb(C_INK))
            .child(text),
    )
}

/// A section caption with caller-built action elements trailing on
/// the right: the [`section_label`] on the left, a flexible spacer,
/// then each action in order. Actions are usually small buttons the
/// caller wired to its own listeners (a sidebar's All / None /
/// Invert trio, a panel's close or add button). An empty `actions`
/// list yields just the caption.
pub fn section_header(label: &str, actions: Vec<Div>) -> Div {
    let mut header = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(section_label(label))
        .child(div().flex_1());
    for action in actions {
        header = header.child(action);
    }
    header
}

#[cfg(test)]
mod tests {
    use super::{clamp_label, MAX_LABEL_CHARS};

    #[test]
    fn clamp_keeps_short_labels_unchanged() {
        assert_eq!(clamp_label("Length", MAX_LABEL_CHARS), "Length");
        assert_eq!(clamp_label("", MAX_LABEL_CHARS), "");
    }

    #[test]
    fn clamp_keeps_a_label_exactly_at_the_limit() {
        let label = "a".repeat(MAX_LABEL_CHARS);
        assert_eq!(clamp_label(&label, MAX_LABEL_CHARS), label);
    }

    #[test]
    fn clamp_truncates_long_labels_with_an_ellipsis() {
        let label = "a".repeat(MAX_LABEL_CHARS + 6);
        let clamped = clamp_label(&label, MAX_LABEL_CHARS);
        assert_eq!(clamped.chars().count(), MAX_LABEL_CHARS);
        assert!(clamped.ends_with('…'));
        assert_eq!(
            clamped,
            format!("{}…", "a".repeat(MAX_LABEL_CHARS - 1))
        );
    }

    #[test]
    fn clamp_counts_characters_not_bytes() {
        // 'é' is two bytes in UTF-8; the limit is on characters.
        let label = "é".repeat(MAX_LABEL_CHARS + 2);
        let clamped = clamp_label(&label, MAX_LABEL_CHARS);
        assert_eq!(clamped.chars().count(), MAX_LABEL_CHARS);
        assert!(clamped.ends_with('…'));
    }

    #[test]
    fn clamp_handles_degenerate_limits() {
        assert_eq!(clamp_label("abc", 0), "");
        assert_eq!(clamp_label("abc", 1), "…");
        assert_eq!(clamp_label("abc", 3), "abc");
    }
}
