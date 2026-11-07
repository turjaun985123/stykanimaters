//! Project statistics grid: the headline numbers of a project
//! ([`crate::stats::ProjectStats`]) laid out as a two-column grid.
//!
//! Like the inspector, this component is data-in only: the caller
//! computes the stats (see [`crate::stats::project_stats`]) and
//! passes them in. The label/value pairs come from the pure
//! [`stat_cells`], so the ordering and formatting are testable
//! without any UI; [`stat_grid`] only arranges them. No listeners,
//! no editor knowledge.

use gpui::*;

use crate::stats::ProjectStats;
use crate::ui_kit::{section_label, C_BORDER, C_INK, C_MUTED, C_PANEL};

/// Format a duration in seconds for display.
///
/// Short durations keep one decimal place (`"12.5 s"`): at the
/// scale of a single scene, fractions of a second are the
/// interesting part. Once the rounded duration reaches a minute,
/// a clock readout is easier to scan than a long decimal, so it
/// renders as `M:SS` (`"1:05"`, `"10:00"`; minutes are not capped
/// at 59). Non-finite or non-positive input — which
/// [`crate::stats`] never produces, but a hand-built
/// [`ProjectStats`] could — reports `"0.0 s"` rather than a
/// nonsense string.
pub fn format_duration(secs: f32) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return "0.0 s".to_string();
    }
    if secs.round() < 60.0 {
        format!("{secs:.1} s")
    } else {
        let total = secs.round() as u64;
        format!("{}:{:02}", total / 60, total % 60)
    }
}

/// The statistics as ordered label/value pairs, in display order.
///
/// Pure: this is the single source of truth for what the grid
/// shows and in which order, kept separate from the `Div`
/// building so it can be unit-tested directly.
pub fn stat_cells(stats: &ProjectStats) -> Vec<(String, String)> {
    vec![
        ("Figures".to_string(), stats.figures.to_string()),
        ("Frames".to_string(), stats.frames.to_string()),
        ("Duration".to_string(), format_duration(stats.duration_secs)),
        (
            "Angle keys".to_string(),
            stats.total_angle_keys.to_string(),
        ),
        (
            "Figures with clips".to_string(),
            stats.figures_with_clips.to_string(),
        ),
        ("FX clips".to_string(), stats.fx_clips.to_string()),
        ("Audio clips".to_string(), stats.audio_clips.to_string()),
        ("Strokes".to_string(), stats.strokes.to_string()),
        ("Max nodes".to_string(), stats.used_nodes_max.to_string()),
    ]
}

/// One grid cell: a muted caption over an ink value.
fn stat_cell(label: &str, value: &str) -> Div {
    div()
        .flex_1()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(C_INK))
                .child(value.to_string()),
        )
}

/// Build the statistics grid panel from computed stats.
///
/// Cells are laid out two per row in [`stat_cells`] order; with
/// an odd cell count the last row holds one cell and an empty
/// spacer, so the columns stay aligned. The panel sets no width
/// of its own — the caller sizes it.
pub fn stat_grid(stats: &ProjectStats) -> Div {
    let mut grid = div()
        .flex_col()
        .gap(px(6.0))
        .p(px(8.0))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .child(section_label("Statistics"));
    for pair in stat_cells(stats).chunks(2) {
        let mut row = div().flex().gap(px(12.0));
        for (label, value) in pair {
            row = row.child(stat_cell(label, value));
        }
        if pair.len() == 1 {
            row = row.child(div().flex_1());
        }
        grid = grid.child(row);
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::{format_duration, stat_cells};
    use crate::model::Project;
    use crate::stats::{project_stats, ProjectStats};

    #[test]
    fn format_duration_under_a_minute_keeps_one_decimal() {
        assert_eq!(format_duration(0.0), "0.0 s");
        assert_eq!(format_duration(12.5), "12.5 s");
        assert_eq!(format_duration(30.0), "30.0 s");
        assert_eq!(format_duration(59.4), "59.4 s");
    }

    #[test]
    fn format_duration_at_a_minute_switches_to_clock() {
        // 59.6 rounds to 60 s, so it already reads as a clock time.
        assert_eq!(format_duration(59.6), "1:00");
        assert_eq!(format_duration(60.0), "1:00");
        assert_eq!(format_duration(75.0), "1:15");
        assert_eq!(format_duration(125.4), "2:05");
        assert_eq!(format_duration(600.0), "10:00");
    }

    #[test]
    fn format_duration_degenerate_input_is_zero() {
        assert_eq!(format_duration(-3.0), "0.0 s");
        assert_eq!(format_duration(f32::NAN), "0.0 s");
        assert_eq!(format_duration(f32::INFINITY), "0.0 s");
    }

    #[test]
    fn stat_cells_are_ordered_label_value_pairs() {
        let stats = ProjectStats {
            figures: 2,
            frames: 4,
            duration_secs: 12.5,
            total_angle_keys: 21,
            figures_with_clips: 1,
            fx_clips: 3,
            audio_clips: 2,
            strokes: 5,
            used_nodes_max: 15,
        };
        let pairs = stat_cells(&stats);
        let cells: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(label, value)| (label.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            cells,
            vec![
                ("Figures", "2"),
                ("Frames", "4"),
                ("Duration", "12.5 s"),
                ("Angle keys", "21"),
                ("Figures with clips", "1"),
                ("FX clips", "3"),
                ("Audio clips", "2"),
                ("Strokes", "5"),
                ("Max nodes", "15"),
            ]
        );
    }

    #[test]
    fn stat_cells_on_empty_stats_are_all_zero() {
        let cells = stat_cells(&ProjectStats::default());
        assert_eq!(cells.len(), 9);
        assert_eq!(cells[0], ("Figures".to_string(), "0".to_string()));
        assert_eq!(cells[2], ("Duration".to_string(), "0.0 s".to_string()));
        assert!(cells
            .iter()
            .all(|(_, value)| value == "0" || value == "0.0 s"));
    }

    #[test]
    fn stat_cells_from_demo_project_stats() {
        // End-to-end: real stats computed by stats.rs, formatted
        // here. The demo is 1 figure, 5 frames at 12 fps.
        let cells = stat_cells(&project_stats(&Project::new_demo()));
        assert_eq!(cells[0], ("Figures".to_string(), "1".to_string()));
        assert_eq!(cells[1], ("Frames".to_string(), "5".to_string()));
        assert_eq!(cells[2], ("Duration".to_string(), "0.4 s".to_string()));
    }
}
