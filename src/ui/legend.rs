//! Legend: a colour key for a set of named categories.
//!
//! A vertical stack of rows, one per [`LegendItem`]: a 10px colour
//! dot, the category's label, and — when the item carries a count —
//! a muted "×N" suffix saying how many things fall in that category.
//! Like every component in this folder it is presentation only:
//! plain data in, `Div` out, no state, no handlers, no ids. Callers
//! attach behaviour to the returned panel if they need it.

use gpui::*;

use crate::ui_kit::{section_label, C_INK, C_MUTED};

/// One legend entry: a category's colour, its display label, and an
/// optional count of how many things belong to it. `colour` is a
/// plain `0xRRGGBB` value, the same convention as the rest of the
/// crate.
#[derive(Clone, Debug)]
pub struct LegendItem {
    /// Category colour (`0xRRGGBB`), shown as the row's dot.
    pub colour: u32,
    /// Category name, shown in ink next to the dot.
    pub label: String,
    /// Optional tally; rendered as a muted "×N" suffix when `Some`.
    pub count: Option<usize>,
}

/// Total of every item's count. Items without a count contribute 0,
/// so the total is the number of counted things across all
/// categories. Pure; an empty slice totals 0.
pub fn total_count(items: &[LegendItem]) -> usize {
    items.iter().map(|item| item.count.unwrap_or(0)).sum()
}

/// The legend rows on their own: a column (gap 4) with one row per
/// item — colour dot, label (flex_1, so suffixes line up on the
/// right), and the count suffix when the item has one.
pub fn legend_rows(items: &[LegendItem]) -> Div {
    let mut rows = div().flex_col().gap(px(4.0));
    for item in items {
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded(px(999.0))
                    .bg(rgb(item.colour)),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(rgb(C_INK))
                    .child(item.label.clone()),
            );
        if let Some(n) = item.count {
            row = row.child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(C_MUTED))
                    .child(format!("×{n}")),
            );
        }
        rows = rows.child(row);
    }
    rows
}

/// A titled legend: the kit's [`section_label`] over the rows, in a
/// column with gap 6. The title is taken as an owned `String`
/// internally, so the returned panel owns its text outright.
pub fn legend_panel(title: &str, items: &[LegendItem]) -> Div {
    let title: String = title.to_string();
    div()
        .flex_col()
        .gap(px(6.0))
        .child(section_label(&title))
        .child(legend_rows(items))
}

#[cfg(test)]
mod tests {
    use super::{legend_rows, total_count, LegendItem};

    fn item(label: &str, count: Option<usize>) -> LegendItem {
        LegendItem {
            colour: 0x3D5A99,
            label: label.to_string(),
            count,
        }
    }

    #[test]
    fn total_count_empty_slice_is_zero() {
        assert_eq!(total_count(&[]), 0);
    }

    #[test]
    fn total_count_sums_some_and_skips_none() {
        let items = vec![
            item("Poses", Some(3)),
            item("Uncounted", None),
            item("Clips", Some(4)),
            item("Also uncounted", None),
        ];
        assert_eq!(total_count(&items), 7);
    }

    #[test]
    fn total_count_all_none_is_zero() {
        let items = vec![item("A", None), item("B", None)];
        assert_eq!(total_count(&items), 0);
    }

    #[test]
    fn legend_rows_empty_slice_does_not_panic() {
        // Building the component over no items must still produce a
        // (childless) column rather than panicking.
        let _rows = legend_rows(&[]);
    }
}
