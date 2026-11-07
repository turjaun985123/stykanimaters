//! Breadcrumb trail: where the current view sits in a hierarchy,
//! as a row of path parts separated by "/" marks.
//!
//! The trail reads left to right from the broadest level to the
//! current one: every part but the last is muted, the last (current)
//! part is drawn in ink so the eye lands on "you are here". Part
//! text is clipped to [`CRUMB_MAX_CHARS`] characters through the
//! shared [`crate::ui_rows::clamp_label`], the same clipping the
//! panel rows use, so a deep path cannot push the rest of the bar
//! off screen.
//!
//! This version is display-only, following the kit contract: plain
//! `Div` builders over plain data, with no listeners and no element
//! ids inside. A future caller that wants clickable crumbs can
//! rebuild the same row shape and attach its own `.id()` /
//! `.on_click()` to each part, the way the colour panel attaches
//! handlers to the kit's swatches.

use crate::ui_kit::{C_INK, C_MUTED};
use crate::ui_rows::clamp_label;
use gpui::*;

/// Maximum number of characters shown for one path part before it
/// is truncated with an ellipsis. Matches the row-label limit in
/// [`crate::ui_rows`]: a part is a short caption ("Projects",
/// "Wave 9"), and anything longer would crowd out the rest of the
/// trail.
pub const CRUMB_MAX_CHARS: usize = 24;

/// The trail as one line of text: the parts joined with " / ",
/// each part first clipped with [`clamp_label`] at
/// [`CRUMB_MAX_CHARS`]. An empty slice yields an empty string; a
/// single part yields just that part, with no separators.
pub fn crumb_text(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| clamp_label(part, CRUMB_MAX_CHARS))
        .collect::<Vec<String>>()
        .join(" / ")
}

/// The breadcrumb row: a flex row of the parts as 12px text — muted
/// for every part but the last, ink for the last (current) part —
/// with a muted 12px "/" separator between parts, never before the
/// first or after the last. An empty slice yields an empty row.
/// All text is owned `String`s built here; nothing borrows from
/// `parts`.
pub fn breadcrumb(parts: &[String]) -> Div {
    let mut row = div().flex().items_center().gap(px(6.0));
    let last = parts.len().saturating_sub(1);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            row = row.child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(C_MUTED))
                    .child("/".to_string()),
            );
        }
        row = row.child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(if i == last { C_INK } else { C_MUTED }))
                .child(clamp_label(part, CRUMB_MAX_CHARS)),
        );
    }
    row
}

/// The same breadcrumb row as [`breadcrumb`], taking ownership of
/// the parts — for callers that built the trail as a fresh `Vec`
/// and have no use for it afterwards.
pub fn breadcrumb_owned(parts: Vec<String>) -> Div {
    breadcrumb(&parts)
}

#[cfg(test)]
mod tests {
    use super::crumb_text;

    #[test]
    fn crumb_text_of_no_parts_is_empty() {
        let parts: Vec<String> = Vec::new();
        assert_eq!(crumb_text(&parts), "");
    }

    #[test]
    fn crumb_text_of_one_part_has_no_separator() {
        let parts = vec!["Projects".to_string()];
        assert_eq!(crumb_text(&parts), "Projects");
    }

    #[test]
    fn crumb_text_joins_parts_with_slashes() {
        let parts = vec![
            "Projects".to_string(),
            "StykAnimate".to_string(),
            "Wave 9".to_string(),
        ];
        assert_eq!(crumb_text(&parts), "Projects / StykAnimate / Wave 9");
    }

    #[test]
    fn crumb_text_clamps_long_parts_with_an_ellipsis() {
        let parts = vec!["Home".to_string(), "a".repeat(30)];
        let text = crumb_text(&parts);
        assert_eq!(text, format!("Home / {}…", "a".repeat(23)));
        // The clamped part is exactly the 24-character limit.
        let tail = text.rsplit(" / ").next().unwrap();
        assert_eq!(tail.chars().count(), 24);
        assert!(tail.ends_with('…'));
    }
}
