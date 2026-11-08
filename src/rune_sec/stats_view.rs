//! Stats panel for the Rune editor shell (Phase D,
//! section 35 — stats view). Read-only: every value comes
//! from the engine's real statistics over the current
//! project — [`stykanimate_rs::stats::project_stats`] for
//! the headline counts and duration,
//! [`stykanimate_rs::stats::busiest_figure`] /
//! [`stykanimate_rs::stats::root_travel`] for the Wave 12
//! motion stats. Nothing here mutates the core, and the
//! builder takes `&State` like the other chrome builders.
//!
//! Cost note for the integrator: `project_stats` is linear
//! in frames × poses and the segment count is linear in
//! total nodes — both trivial. `busiest_figure` is the one
//! expensive entry: it runs `root_travel` for every figure,
//! O(figures × frames) distance sums per call, and this
//! builder then calls `root_travel` once more for the
//! winner (the stats module exposes no combined accessor).
//! Fine at demo scale; on a large project, cache the cells
//! keyed on an edit counter instead of rebuilding per
//! render.

use rune::tok::{type_, Theme};
use rune::{col, el, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::stats::{busiest_figure, project_stats, root_travel};

use crate::State;

/// One stat cell's data: stable element key suffix, the
/// caption under the value, and the already-formatted
/// value. Kept as plain data so the extraction is testable
/// without building elements.
struct StatCell {
    key: &'static str,
    label: String,
    value: String,
}

/// Format a duration in seconds as `m:ss` (the seconds
/// rounded to the nearest whole second, so 0.45 s reads
/// "0:00" and 59.6 s carries to "1:00"). Non-finite or
/// negative input reads "0:00" — the stats module reports
/// 0 for a frameless project or a non-positive fps, and
/// the panel never shows a negative or NaN duration.
pub fn format_duration(secs: f32) -> String {
    let total = if secs.is_finite() && secs > 0.0 {
        secs.round() as u64
    } else {
        0
    };
    format!("{}:{:02}", total / 60, total % 60)
}

/// Format a count with thousands separators
/// (1234567 → "1,234,567").
pub fn format_count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Format a stage-unit distance rounded to whole units
/// (1234.6 → "1,235 units"). Non-finite or negative input
/// reads "0 units".
pub fn format_distance(units: f32) -> String {
    let v = if units.is_finite() && units > 0.0 {
        units.round() as usize
    } else {
        0
    };
    format!("{} units", format_count(v))
}

/// Format an fps value the way the status line does
/// (`chrome_view::status_text`): whole numbers bare,
/// fractional to one decimal.
pub fn format_fps(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    }
}

/// Extract the panel's cells from the core's project, in
/// display order. All counts come from
/// `stats::project_stats`; the segment total is the one
/// figure the stats struct doesn't carry, counted here
/// directly from the project (every node with a parent is
/// one segment — the model's definition, see
/// `EditorCore::delete_segment`).
fn stat_cells(core: &EditorCore) -> Vec<StatCell> {
    let p = &core.project;
    let s = project_stats(p);
    let segments: usize = p
        .figures
        .iter()
        .flat_map(|f| f.def.nodes.iter())
        .filter(|n| n.parent.is_some())
        .count();
    let (busiest_name, busiest_travel) = match busiest_figure(p) {
        Some(fi) => (p.figures[fi].name.clone(), root_travel(p, fi)),
        None => ("—".to_string(), 0.0),
    };
    vec![
        StatCell {
            key: "figures",
            label: "Figures".into(),
            value: format_count(s.figures),
        },
        StatCell {
            key: "frames",
            label: "Frames".into(),
            value: format_count(s.frames),
        },
        StatCell {
            key: "segments",
            label: "Segments".into(),
            value: format_count(segments),
        },
        StatCell {
            key: "strokes",
            label: "Strokes".into(),
            value: format_count(s.strokes),
        },
        StatCell {
            key: "duration",
            label: format!("Duration · {} fps", format_fps(p.fps)),
            value: format_duration(s.duration_secs),
        },
        StatCell {
            key: "fx-clips",
            label: "FX clips".into(),
            value: format_count(s.fx_clips),
        },
        StatCell {
            key: "audio-clips",
            label: "Audio clips".into(),
            value: format_count(s.audio_clips),
        },
        StatCell {
            key: "clip-figures",
            label: "Clip figures".into(),
            value: format_count(s.figures_with_clips),
        },
        StatCell {
            key: "busiest",
            label: "Busiest figure".into(),
            value: busiest_name,
        },
        StatCell {
            key: "root-travel",
            label: "Root travel".into(),
            value: format_distance(busiest_travel),
        },
    ]
}

/// One cell element: the value over its caption, on a card
/// fill. Keyed `stat-{key}` so headless `--tree` checks can
/// find individual cells.
fn stat_cell_el(cell: &StatCell, t: &Theme) -> El {
    col()
        .key(format!("stat-{}", cell.key))
        .fill(t.card)
        .radius(8.0)
        .padding(10.0)
        .gap(2.0)
        .child(text(cell.value.clone()).font(type_::TITLE).color(t.text))
        .child(
            text(cell.label.clone())
                .font(type_::SECTION)
                .color(t.text_secondary),
        )
}

/// The stats panel: a "Project stats" caption over a
/// five-column grid of stat cells (two rows at the current
/// ten cells). Mount it wherever a full-width row fits;
/// the column count is the one-line change if a narrow
/// sidebar mount is preferred.
pub fn stats_panel(s: &State, t: &Theme) -> El {
    let mut grid = el().key("stats-grid").w_full().equal_columns(5).gap(8.0);
    for cell in stat_cells(&s.core) {
        grid = grid.child(stat_cell_el(&cell, t));
    }
    col()
        .key("stats-panel")
        .w_full()
        .gap(8.0)
        .child(
            text("Project stats")
                .font(type_::SECTION)
                .color(t.text_secondary),
        )
        .child(grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formats_minutes_and_padded_seconds() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(5.0), "0:05");
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(600.0), "10:00");
        // Seconds round to nearest and carry into minutes.
        assert_eq!(format_duration(59.6), "1:00");
        assert_eq!(format_duration(0.45), "0:00");
    }

    #[test]
    fn duration_degenerate_input_is_zero() {
        assert_eq!(format_duration(-3.0), "0:00");
        assert_eq!(format_duration(f32::NAN), "0:00");
        assert_eq!(format_duration(f32::INFINITY), "0:00");
    }

    #[test]
    fn count_groups_thousands() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(42), "42");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
    }

    #[test]
    fn distance_rounds_and_groups() {
        assert_eq!(format_distance(0.0), "0 units");
        assert_eq!(format_distance(10.4), "10 units");
        assert_eq!(format_distance(10.5), "11 units");
        assert_eq!(format_distance(1234.6), "1,235 units");
        assert_eq!(format_distance(-1.0), "0 units");
        assert_eq!(format_distance(f32::NAN), "0 units");
    }

    #[test]
    fn fps_matches_the_status_line_style() {
        assert_eq!(format_fps(12.0), "12");
        assert_eq!(format_fps(29.97), "30.0");
        assert_eq!(format_fps(23.5), "23.5");
    }

    #[test]
    fn cells_come_from_the_real_demo_project_stats() {
        // EditorCore::new seeds Project::new_demo: 1 figure,
        // 5 frames at 12 fps (see stats.rs tests).
        let core = EditorCore::new();
        let cells = stat_cells(&core);
        assert_eq!(cells.len(), 10);
        let by_key = |key: &str| {
            cells
                .iter()
                .find(|c| c.key == key)
                .map(|c| c.value.as_str())
                .unwrap_or("<missing>")
        };
        assert_eq!(by_key("figures"), "1");
        assert_eq!(by_key("frames"), "5");
        // Segments are the figure def's nodes with a parent,
        // counted straight off the project here.
        let demo_segments = core.project.figures[0]
            .def
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .count();
        assert!(demo_segments > 0);
        assert_eq!(by_key("segments"), format_count(demo_segments));
        assert_eq!(by_key("strokes"), "0");
        // 5 frames held 1.0 each at 12 fps ≈ 0.42 s.
        assert_eq!(by_key("duration"), "0:00");
        assert_eq!(by_key("fx-clips"), "0");
        assert_eq!(by_key("audio-clips"), "0");
        assert_eq!(by_key("clip-figures"), "0");
        // The demo's only figure is trivially the busiest.
        assert_eq!(by_key("busiest"), core.project.figures[0].name);
        // The root-travel cell agrees with the stats fn it
        // came from, applied to the busiest figure (index 0,
        // the demo's only figure).
        assert_eq!(
            by_key("root-travel"),
            format_distance(root_travel(&core.project, 0))
        );
    }

    #[test]
    fn cells_for_an_empty_project_are_zeroed_not_panicked() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        core.project.frames.clear();
        let cells = stat_cells(&core);
        let by_key = |key: &str| {
            cells
                .iter()
                .find(|c| c.key == key)
                .map(|c| c.value.as_str())
                .unwrap_or("<missing>")
        };
        assert_eq!(by_key("figures"), "0");
        assert_eq!(by_key("frames"), "0");
        assert_eq!(by_key("segments"), "0");
        assert_eq!(by_key("busiest"), "—");
        assert_eq!(by_key("root-travel"), "0 units");
    }
}
