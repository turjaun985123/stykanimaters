//! One row of the node list: a single segment of the selected
//! figure, shown as a colour dot, a name, and its length/thickness
//! readouts.
//!
//! The inspector panel describes the *selected* node; this row is
//! for the list the selection comes from — one row per node in the
//! figure, so the user can see the whole skeleton at a glance and
//! pick the next node. Like the rest of the kit it is presentation
//! only: the caller assembles one [`NodeRowData`] per node and
//! attaches the row's selection id and click listener itself, the
//! same split [`crate::ui_layer_row`] uses for figure rows.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_INK, C_MUTED};

/// Plain display data for one node row, mirrored from the fields
/// of `model::NodeDef` the list shows: `label` is the node's
/// `name`, `length`/`thickness` its segment metrics, `colour` its
/// per-segment colour override (`None` inherits the figure colour).
#[derive(Clone, Debug)]
pub struct NodeRowData {
    /// Index of the node within its figure.
    pub index: usize,
    /// Display name of the node (`NodeDef::name`).
    pub label: String,
    /// Segment length in stage units (`NodeDef::length`).
    pub length: f32,
    /// Segment stroke thickness (`NodeDef::thickness`).
    pub thickness: f32,
    /// Per-segment colour override (`NodeDef::color`); `None`
    /// means the segment inherits the figure's colour.
    pub colour: Option<u32>,
    /// Whether this node is the figure origin (index 0, no parent):
    /// it has no segment of its own, so the row shows "origin"
    /// instead of a length.
    pub is_origin: bool,
}

/// A segment metric as the row shows it: whole stage units, no
/// decimals. Non-finite values (a corrupt or half-authored def)
/// read as "0" rather than painting "NaN"/"inf" into the list.
pub fn length_label(length: f32) -> String {
    if length.is_finite() {
        format!("{length:.0}")
    } else {
        "0".to_string()
    }
}

/// Build one node row on the sidebar row metrics (padding 6,
/// radius 6, 4px gaps, children centred vertically): the node's
/// colour dot, its label, then the length and thickness readouts.
///
/// The background follows [`crate::ui_layer_row`]'s RGBA
/// convention: kit colours are plain `0xRRGGBB` for `rgb()`, which
/// cannot express "no background", so the selected state packs the
/// accent as `0xRRGGBBAA` for `rgba()` — `(C_ACCENT << 8) | 0xFF` —
/// and the unselected state is `rgba(0)`, fully transparent, so the
/// panel behind the list shows through.
pub fn node_row(data: &NodeRowData, selected: bool) -> Div {
    let bg = if selected {
        rgba((C_ACCENT << 8) | 0xFF)
    } else {
        rgba(0)
    };
    // The dot shows the node's own colour; a node with no override
    // gets the neutral chip grey the kit uses for "inherit/default"
    // (the same 0x2C313A as an inactive info_chip).
    let dot = data.colour.unwrap_or(0x2C313A);
    let length_text = if data.is_origin {
        "origin".to_string()
    } else {
        format!("L {}", length_label(data.length))
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .p(px(6.0))
        .rounded(px(6.0))
        .bg(bg)
        .child(
            div()
                .w(px(10.0))
                .h(px(10.0))
                .rounded(px(999.0))
                .bg(rgb(dot)),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(data.label.clone()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(length_text),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(format!("T {}", length_label(data.thickness))),
        )
}

#[cfg(test)]
mod tests {
    use super::length_label;

    #[test]
    fn length_label_rounds_to_whole_units() {
        assert_eq!(length_label(42.4), "42");
        assert_eq!(length_label(42.6), "43");
        assert_eq!(length_label(0.0), "0");
    }

    #[test]
    fn length_label_non_finite_reads_as_zero() {
        assert_eq!(length_label(f32::NAN), "0");
        assert_eq!(length_label(f32::INFINITY), "0");
        assert_eq!(length_label(f32::NEG_INFINITY), "0");
    }

    #[test]
    fn length_label_keeps_the_sign_of_negatives() {
        assert_eq!(length_label(-7.6), "-8");
        assert_eq!(length_label(-3.2), "-3");
    }
}
