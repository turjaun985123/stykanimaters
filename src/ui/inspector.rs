//! Read-only inspector panel for the current figure/node selection.
//!
//! This component is deliberately data-in only: the caller assembles
//! an [`InspectorData`] snapshot from the editor and passes it in.
//! Keeping editor types out of this file makes the panel reusable and
//! leaves all selection/pose calculations with the caller.
//!
//! When no figure is selected (an empty project, or a selection
//! index with no figure behind it), the caller fills the same slot
//! with [`project_summary_panel`] instead, so the inspector always
//! shows real information about the project rather than a fabricated
//! empty figure.

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

/// Plain display data for the project summary shown when no figure
/// is selected. Mirrors the field types of the document model
/// (`Project::stage` is a pair of `f32` stage units, not pixels).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSummary {
    /// Number of figures in the project.
    pub figures: usize,
    /// Number of frames on the timeline.
    pub frames: usize,
    /// Project playback rate in frames per second.
    pub fps: f32,
    /// Logical stage size `[width, height]` in stage units.
    pub stage: [f32; 2],
}

/// Format a stage size as `"W × H"`. Integral sizes print without a
/// decimal part (the common case: stage sizes are whole units).
pub fn stage_label(stage: [f32; 2]) -> String {
    format!("{} × {}", stage[0], stage[1])
}

/// Build the project summary panel: the inspector slot's content
/// when no figure is selected. Same container metrics and row style
/// as [`inspector_panel`], so the column looks identical whichever
/// selection state fills it.
pub fn project_summary_panel(s: &ProjectSummary) -> Div {
    div()
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
        .child(section_label("Project"))
        .child(property_row("Figures", format!("{}", s.figures)))
        .child(property_row("Frames", format!("{}", s.frames)))
        .child(property_row("FPS", format!("{}", s.fps)))
        .child(property_row("Stage", stage_label(s.stage)))
}

#[cfg(test)]
mod tests {
    use super::{project_summary_panel, stage_label, ProjectSummary};

    #[test]
    fn stage_label_formats_integral_sizes_without_decimals() {
        assert_eq!(stage_label([960.0, 720.0]), "960 × 720");
        assert_eq!(stage_label([1920.0, 1080.0]), "1920 × 1080");
    }

    #[test]
    fn stage_label_keeps_fractional_sizes() {
        assert_eq!(stage_label([960.5, 720.25]), "960.5 × 720.25");
    }

    #[test]
    fn project_summary_panel_builds_for_a_populated_project() {
        let summary = ProjectSummary {
            figures: 3,
            frames: 48,
            fps: 30.0,
            stage: [960.0, 720.0],
        };
        let _panel = project_summary_panel(&summary);
    }

    #[test]
    fn project_summary_panel_builds_for_an_empty_project() {
        let summary = ProjectSummary {
            figures: 0,
            frames: 1,
            fps: 30.0,
            stage: [960.0, 720.0],
        };
        let _panel = project_summary_panel(&summary);
    }
}
