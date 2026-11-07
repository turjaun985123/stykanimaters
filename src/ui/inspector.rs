//! Read-only inspector panel for the current figure/node selection.
//!
//! This component is deliberately data-in only: the caller assembles
//! an [`InspectorData`] snapshot from the editor and passes it in.
//! Keeping editor types out of this file makes the panel reusable and
//! leaves all selection/pose calculations with the caller.

use crate::ui_kit::*;
use gpui::*;

/// Plain display data for one inspector render.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorData {
    /// Name of the selected figure.
    pub figure_name: String,
    /// Selected figure colour as an RGB value (`0xRRGGBB`).
    pub figure_color: u32,
    /// Index of the selected node, when a node is selected.
    pub node_index: Option<usize>,
    /// Number of drawable segments in the selected figure (all nodes
    /// except the origin).
    pub seg_count: usize,
    /// Length of the selected segment, when the selection has one.
    pub length: Option<f32>,
    /// Thickness of the selected segment, when the selection has one.
    pub thickness: Option<f32>,
    /// Current absolute angle of the selected segment in degrees,
    /// calculated by the caller from the evaluated pose. This is
    /// `None` for the origin and when no node is selected.
    pub angle_deg: Option<f32>,
    /// Whether the selected node is the figure origin.
    pub is_origin: bool,
}

fn property_row(label: &str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .w(px(76.0))
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(value),
        )
}

fn optional_number(value: Option<f32>, digits: usize) -> String {
    match value {
        Some(value) => format!("{value:.digits$}"),
        None => "—".to_string(),
    }
}

/// Build the inspector panel from a plain data snapshot.
///
/// The returned panel is a container, so the caller may append its
/// own interactive controls (for example, stepper buttons) as final
/// children while keeping those controls visually inside the panel.
pub fn inspector_panel(data: InspectorData) -> Div {
    let node_value = match data.node_index {
        Some(index) => format!("#{index}"),
        None => "None selected".to_string(),
    };
    let angle_value = match data.angle_deg {
        Some(angle) => format!("{angle:.1}°"),
        None => "—".to_string(),
    };

    let mut panel = div()
        .flex()
        .flex_col()
        .w(px(224.0))
        .gap(px(6.0))
        .p(px(8.0))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .child(section_label("Inspector"))
        .child(section_label("Figure"))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(12.0))
                        .rounded(px(3.0))
                        .bg(rgb(data.figure_color)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.0))
                        .text_color(rgb(C_INK))
                        .child(data.figure_name.clone()),
                ),
        )
        .child(property_row(
            "Colour",
            format!("#{:06X}", data.figure_color & 0x00FF_FFFF),
        ))
        .child(property_row("Segments", format!("{}", data.seg_count)))
        .child(section_label("Node"))
        .child(property_row("Node", node_value))
        .child(property_row(
            "Type",
            if data.is_origin {
                "Origin".to_string()
            } else if data.node_index.is_some() {
                "Segment".to_string()
            } else {
                "—".to_string()
            },
        ))
        .child(property_row("Length", optional_number(data.length, 0)))
        .child(property_row(
            "Thickness",
            optional_number(data.thickness, 0),
        ))
        .child(property_row("Angle", angle_value));

    if data.node_index.is_none() {
        panel = panel.child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child("Select a node to inspect its segment."),
        );
    } else if data.is_origin {
        panel = panel.child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child("The origin has no segment length or angle."),
        );
    }

    panel
}
