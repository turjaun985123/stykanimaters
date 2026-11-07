//! Saved-file row for file lists (Home panel projects, recents).
//!
//! The Home panel lists one row per saved project: the project name
//! with a small meta line under it ("3 figure(s), 12 frame(s),
//! 24 fps") and an Open button on the right. `app.rs` used to
//! hand-build that row inline inside `render()` for every entry;
//! this component owns the row's layout — the rounded container,
//! the name column, the trailing cluster — while the caller keeps
//! building the contents that carry behaviour. That split is
//! deliberate, and the same one [`crate::ui_layer_row`] uses: every
//! button in the trailing cluster carries a click listener bound to
//! editor state, and listeners stay with the caller. The caller
//! hands its buttons over as pre-built `trailing` elements (wrapping
//! each `btn(...)` in a `div().child(...)`, since `btn` returns
//! `impl IntoElement`, not a `Div`).
//!
//! Presentation only, like the rest of the kit: no state, no
//! handlers, no editor knowledge. The name is clipped with the
//! shared [`crate::ui_rows::clamp_label`] so a long project name
//! ellipsises instead of squeezing the buttons off the row.

use gpui::*;

use crate::ui_kit::{C_INK, C_MUTED};
use crate::ui_rows::clamp_label;

/// Maximum number of characters shown for a file name before it is
/// truncated with an ellipsis. Wider than the row kit's
/// `MAX_LABEL_CHARS` (24): file names are the row's main content,
/// not a caption, so they get more room before clipping.
pub const MAX_NAME_CHARS: usize = 40;

/// Everything [`file_row`] needs to know about one file: its display
/// name and the small meta line shown under it. Plain data, built by
/// the caller from whatever model it scans (the Home panel builds
/// `meta` with [`meta_line`] from a `projects::ProjectInfo`).
#[derive(Clone, Debug, PartialEq)]
pub struct FileRowData {
    pub name: String,
    pub meta: String,
}

/// The name as the row shows it: `name` clipped to
/// [`MAX_NAME_CHARS`] characters with the shared row clipping, so
/// the rendered text and the tests agree on exactly what is cut.
pub fn display_name(name: &str) -> String {
    clamp_label(name, MAX_NAME_CHARS)
}

/// The meta line for one saved project, in the exact format the
/// Home panel has always printed:
/// `"{figures} figure(s), {frames} frame(s), {fps} fps"`. Pure
/// formatting — the caller supplies the counts from its scan.
pub fn meta_line(figures: usize, frames: usize, fps: i32) -> String {
    format!("{figures} figure(s), {frames} frame(s), {fps} fps")
}

/// The row container: a rounded horizontal strip on the Home
/// panel's metrics (padding 8, radius 8, 8px gaps, children centred
/// vertically). On the left, a flexible name column — the name at
/// 13px in ink, clipped by [`display_name`], with the meta line
/// under it at 11px muted. On the right, `trailing` — the caller's
/// action elements (usually one Open button), laid out left to
/// right in order in a 4px-gap cluster. No listeners or ids are
/// attached inside; the caller wires those onto its own elements.
pub fn file_row(data: &FileRowData, trailing: Vec<Div>) -> Div {
    let mut cluster = div().flex().flex_row().items_center().gap(px(4.0));
    for el in trailing {
        cluster = cluster.child(el);
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(C_INK))
                        .child(display_name(&data.name)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(C_MUTED))
                        .child(data.meta.clone()),
                ),
        )
        .child(cluster)
}

#[cfg(test)]
mod tests {
    use super::{display_name, meta_line, MAX_NAME_CHARS};

    #[test]
    fn meta_line_matches_the_home_panel_format() {
        assert_eq!(meta_line(3, 12, 24), "3 figure(s), 12 frame(s), 24 fps");
        assert_eq!(meta_line(2, 5, 30), "2 figure(s), 5 frame(s), 30 fps");
    }

    #[test]
    fn meta_line_keeps_the_plural_wording_at_zero_and_one() {
        // The panel's format never singularises — pin that, so a
        // "helpful" grammar fix can't silently change the UI text.
        assert_eq!(meta_line(0, 0, 0), "0 figure(s), 0 frame(s), 0 fps");
        assert_eq!(meta_line(1, 1, 1), "1 figure(s), 1 frame(s), 1 fps");
    }

    #[test]
    fn display_name_keeps_short_names_unchanged() {
        assert_eq!(display_name("My Project"), "My Project");
        assert_eq!(display_name(""), "");
    }

    #[test]
    fn display_name_keeps_a_name_exactly_at_the_limit() {
        let name = "a".repeat(MAX_NAME_CHARS);
        assert_eq!(display_name(&name), name);
    }

    #[test]
    fn display_name_truncates_long_names_with_an_ellipsis() {
        let name = "a".repeat(MAX_NAME_CHARS + 10);
        let shown = display_name(&name);
        assert_eq!(shown.chars().count(), MAX_NAME_CHARS);
        assert!(shown.ends_with('…'));
        assert_eq!(shown, format!("{}…", "a".repeat(MAX_NAME_CHARS - 1)));
    }
}
