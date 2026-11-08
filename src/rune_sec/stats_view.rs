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
//! Below the grid sit four counts-only blocks the stats
//! module does not cover: **Project totals**
//! ([`project_totals`] — every content vec on the project
//! model, counted), **Media** ([`media_totals`] — the
//! audio start-frame span, the summed FX-clip duration,
//! texts and strokes), **Flags** ([`flag_totals`] — the
//! muted-audio, hidden-FX and translucent-stroke counts)
//! and **Figures** ([`figure_rows`] —
//! per figure: node count plus how many graph channels,
//! FX clips and particle events are anchored to it). They
//! are labelled as counts and deliberately recompute none
//! of the motion statistics above.
//!
//! Phase N adoption: the panel root is the design kit's
//! card ([`design::panel_col`], key `stats-panel`
//! unchanged), the header is the kit's
//! [`design::panel_title`] (chart-simple glyph, P13's
//! assignment for this panel) over a kit
//! [`design::divider`], and the four block captions are
//! kit [`design::section_label`]s. The row text takes
//! P11 §4D's font assignment: labels LABEL /
//! `text_secondary`, counts and values MONO_CAPTION /
//! `t.text`, the per-figure prose line LABEL / `t.text`;
//! the grid cells keep their TITLE values (P11: the one
//! display-numeral role TITLE retains). Every display
//! string is byte-identical to before — only the chrome
//! moved.
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
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::stats::{busiest_figure, project_stats, root_travel};

use crate::rune_sec::design;
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

/// Format a length measured in (possibly fractional)
/// frames: whole values grouped like a count ("1,250"),
/// fractional values to one decimal ("3.5"). Non-finite
/// or negative input reads "0".
pub fn format_frames(frames: f32) -> String {
    if !frames.is_finite() || frames <= 0.0 {
        return "0".to_string();
    }
    if frames.fract() == 0.0 {
        format_count(frames as usize)
    } else {
        format!("{frames:.1}")
    }
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

/// Counts of every content vec on the project model, in a
/// stable display order: `(label, count)` pairs. Only
/// fields that exist on
/// [`stykanimate_rs::model::Project`] are counted —
/// figures, frames, strokes, particle events
/// (`Project::particles`), FX clips, audio clips,
/// movieclips (`Project::clips`, the reusable clip
/// definitions), graph channels (`Project::channels`) and
/// free-standing texts. These are plain counts, labelled
/// as counts in the panel; anything the stats module
/// already computes (duration, travel, busiest figure)
/// stays in the grid above and is not recomputed here.
pub fn project_totals(core: &EditorCore) -> Vec<(String, usize)> {
    let p = &core.project;
    vec![
        ("Figures".to_string(), p.figures.len()),
        ("Frames".to_string(), p.frames.len()),
        ("Strokes".to_string(), p.strokes.len()),
        ("Particle events".to_string(), p.particles.len()),
        ("FX clips".to_string(), p.fx_clips.len()),
        ("Audio clips".to_string(), p.audio.len()),
        ("Movieclips".to_string(), p.clips.len()),
        ("Graph channels".to_string(), p.channels.len()),
        ("Texts".to_string(), p.texts.len()),
    ]
}

/// Media depth as `(label, value)` pairs, the value
/// already formatted for display. What each row can say
/// is bounded by the model. An
/// [`stykanimate_rs::model::AudioClip`] stores only a
/// path and a start frame — a clip's length lives in its
/// sound file, not in the project — so the audio row is
/// the count plus the span of start frames (earliest to
/// latest), never an invented total duration. FX clips
/// do carry a length
/// (`stykanimate_rs::vfx::ParticleClip::duration`, in
/// frames), so their row adds the durations summed over
/// every clip. Texts and strokes are plain counts,
/// repeated here so the media block reads on its own.
pub fn media_totals(core: &EditorCore) -> Vec<(String, String)> {
    let p = &core.project;
    let audio = if p.audio.is_empty() {
        "0".to_string()
    } else {
        let first = p.audio.iter().map(|c| c.start_frame).min().unwrap_or(0);
        let last = p.audio.iter().map(|c| c.start_frame).max().unwrap_or(0);
        if first == last {
            format!("{} (start frame {})", format_count(p.audio.len()), first)
        } else {
            format!(
                "{} (start frames {}–{})",
                format_count(p.audio.len()),
                first,
                last
            )
        }
    };
    let fx = if p.fx_clips.is_empty() {
        "0".to_string()
    } else {
        let total: f32 = p.fx_clips.iter().map(|e| e.clip.duration).sum();
        format!(
            "{} · {} frames of effects",
            format_count(p.fx_clips.len()),
            format_frames(total)
        )
    };
    vec![
        ("Audio clips".to_string(), audio),
        ("FX clips".to_string(), fx),
        ("Texts".to_string(), format_count(p.texts.len())),
        ("Strokes".to_string(), format_count(p.strokes.len())),
    ]
}

/// Content-flag counts as `(label, value)` pairs, the
/// value a plain count: how much of the project's media
/// is switched off or softened —
/// [`stykanimate_rs::model::AudioClip::muted`] clips (on
/// the timeline but silent in playback plans and export
/// mixes), [`stykanimate_rs::model::FxClipEntry::hidden`]
/// clips (skipped by the stage and the export renderers)
/// and translucent
/// [`stykanimate_rs::model::Stroke`]s. The stroke test
/// is a plain `opacity < 1.0`, deliberately without an
/// epsilon: the recorded values are exact brush opacities
/// and the default is exactly 1.0, so a stored 0.999 from
/// a stepper is genuinely translucent and counts.
pub fn flag_totals(core: &EditorCore) -> Vec<(String, String)> {
    let p = &core.project;
    let muted = p.audio.iter().filter(|c| c.muted).count();
    let hidden = p.fx_clips.iter().filter(|e| e.hidden).count();
    let translucent = p.strokes.iter().filter(|s| s.opacity < 1.0).count();
    vec![
        ("Muted audio clips".to_string(), format_count(muted)),
        ("Hidden FX clips".to_string(), format_count(hidden)),
        (
            "Translucent strokes".to_string(),
            format_count(translucent),
        ),
    ]
}

/// One figure's counts for the per-figure block: its name,
/// the node count of its definition, and how many of the
/// project's figure-indexed entries belong to it — graph
/// channels (`Channel::figure`), FX clips
/// (`FxClipEntry::figure`) and anchored particle events
/// (`ParticleEvent::figure == Some(fi)`; stage-fixed
/// events with `figure: None` belong to no figure).
#[derive(Clone, Debug, PartialEq)]
pub struct FigureRow {
    pub name: String,
    pub nodes: usize,
    pub channels: usize,
    pub fx_clips: usize,
    pub particles: usize,
}

/// Per-figure count rows, in project figure order. One
/// pass over each figure-indexed vec per figure; an empty
/// project yields an empty vec.
pub fn figure_rows(core: &EditorCore) -> Vec<FigureRow> {
    let p = &core.project;
    p.figures
        .iter()
        .enumerate()
        .map(|(fi, f)| FigureRow {
            name: f.name.clone(),
            nodes: f.def.nodes.len(),
            channels: p.channels.iter().filter(|c| c.figure == fi).count(),
            fx_clips: p.fx_clips.iter().filter(|e| e.figure == fi).count(),
            particles: p
                .particles
                .iter()
                .filter(|e| e.figure == Some(fi))
                .count(),
        })
        .collect()
}

/// Element-key slug for a totals label ("Particle events"
/// → "particle-events"): lowercase, each run of
/// non-alphanumeric characters collapsed to one '-'.
fn label_slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut last_dash = true; // suppresses a leading dash
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// One cell element: the value over its caption, on the
/// window-bg fill so the tile still reads as a tile now
/// that the panel root itself carries the card fill
/// (Phase N; `t.bg` inner wells are the shell's existing
/// idiom — home_panel's rows do the same). Keyed
/// `stat-{key}` so headless `--tree` checks can find
/// individual cells. The cell grows to its half of the
/// grid row and may shrink below its content size
/// (`min_w(0)`): that bound is what keeps the grid
/// inside its card — see [`stats_panel`].
fn stat_cell_el(cell: &StatCell, t: &Theme) -> El {
    col()
        .key(format!("stat-{}", cell.key))
        .grow(1.0)
        .min_w(0.0)
        .fill(t.bg)
        .radius(8.0)
        .padding(10.0)
        .gap(2.0)
        .child(text(cell.value.clone()).font(type_::TITLE).color(t.text))
        .child(design::section_label(&cell.label, t))
}

/// One totals row: the label in secondary text, the count
/// after it. Fonts are P11 §4D's assignment (Phase N):
/// label LABEL / secondary, count MONO_CAPTION / `t.text`
/// (before Phase N both rendered at implicit BODY). Keyed
/// `stats-total-{slug}`.
fn total_row_el(label: &str, count: usize, t: &Theme) -> El {
    row()
        .key(format!("stats-total-{}", label_slug(label)))
        .gap(8.0)
        .child(
            text(label.to_string())
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(format_count(count))
                .font(type_::MONO_CAPTION.mono())
                .color(t.text),
        )
}

/// The Project totals block: a caption over the compact
/// count rows from [`project_totals`]. Keyed
/// `stats-totals`.
fn totals_el(s: &State, t: &Theme) -> El {
    let mut block = col()
        .key("stats-totals")
        .w_full()
        .gap(2.0)
        .child(design::section_label("Project totals", t));
    for (label, count) in project_totals(&s.core) {
        block = block.child(total_row_el(&label, count, t));
    }
    block
}

/// One media row: the label and its formatted value.
/// Fonts are P11 §4D's assignment (Phase N): label LABEL
/// / secondary, value MONO_CAPTION / `t.text` (before
/// Phase N both rendered muted at implicit BODY). Keyed
/// `stats-media-{slug}`.
fn media_row_el(label: &str, value: &str, t: &Theme) -> El {
    row()
        .key(format!("stats-media-{}", label_slug(label)))
        .gap(8.0)
        .child(
            text(label.to_string())
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(value.to_string())
                .font(type_::MONO_CAPTION.mono())
                .color(t.text),
        )
}

/// The Media block: a caption over the rows from
/// [`media_totals`] — audio clip count and start-frame
/// span, FX clip count and summed duration, text and
/// stroke counts. Keyed `stats-media`.
fn media_el(s: &State, t: &Theme) -> El {
    let mut block = col()
        .key("stats-media")
        .w_full()
        .gap(2.0)
        .child(design::section_label("Media", t));
    for (label, value) in media_totals(&s.core) {
        block = block.child(media_row_el(&label, &value, t));
    }
    block
}

/// One flags row: the label and its count, with
/// [`media_row_el`]'s P11 §4D fonts (Phase N): label
/// LABEL / secondary, value MONO_CAPTION / `t.text`.
/// Keyed `stats-flag-{i}` by row index.
fn flag_row_el(index: usize, label: &str, value: &str, t: &Theme) -> El {
    row()
        .key(format!("stats-flag-{index}"))
        .gap(8.0)
        .child(
            text(label.to_string())
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(value.to_string())
                .font(type_::MONO_CAPTION.mono())
                .color(t.text),
        )
}

/// The Flags block: a caption over the count rows from
/// [`flag_totals`] — muted audio clips, hidden FX clips
/// and translucent strokes. Rendered always, like the
/// Media block above it, so a project with no flagged
/// content reads three plain "0" rows instead of the
/// block vanishing. Keyed `stats-flags`.
fn flags_el(s: &State, t: &Theme) -> El {
    let mut block = col()
        .key("stats-flags")
        .w_full()
        .gap(2.0)
        .child(design::section_label("Flags", t));
    for (i, (label, value)) in flag_totals(&s.core).iter().enumerate() {
        block = block.child(flag_row_el(i, label, value, t));
    }
    block
}

/// The per-figure block: a "Figures" caption over one
/// compact line per figure from [`figure_rows`] — name,
/// then its node / channel / FX-clip / particle counts.
/// Keyed `stats-figures`.
fn figures_el(s: &State, t: &Theme) -> El {
    let mut block = col()
        .key("stats-figures")
        .w_full()
        .gap(2.0)
        .child(design::section_label("Figures", t));
    for r in figure_rows(&s.core) {
        block = block.child(
            text(format!(
                "{} — {} nodes · {} channels · {} FX clips · {} particles",
                r.name, r.nodes, r.channels, r.fx_clips, r.particles
            ))
            .font(type_::LABEL)
            .color(t.text),
        );
    }
    block
}

/// The stats panel: the kit card (Phase N) — a header
/// row holding the kit's [`design::panel_title`]
/// (chart-simple glyph + "Project stats") over a
/// hairline divider, then the stat-cell grid (two
/// columns, five rows at the current ten cells), the
/// Project totals count rows, the Media rows, the Flags
/// rows and the per-figure count lines.
///
/// The grid is rows of two growing cells, NOT
/// `equal_columns`: Rune's grid tracks size to the
/// widest child's content, so the old five-column grid
/// laid its tracks out past the card — the S19 visual
/// QA audit measured the last cells ending +153.5 pt
/// past the card's content edge (x 1565, off-window).
/// Two `grow` cells with `min_w(0)` per row split the
/// card's content width exactly, in whichever column
/// the panel is mounted.
pub fn stats_panel(s: &State, t: &Theme) -> El {
    let cells = stat_cells(&s.core);
    let mut grid = col().key("stats-grid").w_full().gap(8.0);
    for pair in cells.chunks(2) {
        let mut grid_row = row().w_full().gap(8.0);
        for cell in pair {
            grid_row = grid_row.child(stat_cell_el(cell, t));
        }
        grid = grid.child(grid_row);
    }
    design::panel_col("stats-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("chart-simple"),
                    "Project stats",
                    None,
                    t,
                )),
        )
        .child(design::divider(t))
        .child(grid)
        .child(totals_el(s, t))
        .child(media_el(s, t))
        .child(flags_el(s, t))
        .child(figures_el(s, t))
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

    /// The demo project stocked with known content in every
    /// counted vec: a second figure ("Pip"), two strokes,
    /// three particle events (two anchored to figure 0, one
    /// stage-fixed), two FX clips (one per figure), one
    /// audio clip, one movieclip, three graph channels
    /// (two on figure 0, one on figure 1) and one text.
    /// Built the way the tests above build: `EditorCore::new`
    /// plus direct project writes.
    fn stocked_core() -> EditorCore {
        use stykanimate_rs::graph::{Channel, ChannelTarget};
        use stykanimate_rs::model::{
            AudioClip, ClipDef, ClipLoop, FigureDef, FigureInstance, FxClipEntry,
            ParticleEvent, ParticleKind, Stroke, TextInstance,
        };
        use stykanimate_rs::vfx::{ClipKind, ParticleClip};

        let mut core = EditorCore::new();
        let p = &mut core.project;
        p.figures.push(FigureInstance {
            name: "Pip".into(),
            color: 0xFFFFFF,
            def: FigureDef::humanoid(),
            hidden: false,
            clip: None,
            locked: false,
            opacity: 1.0,
        });
        for _ in 0..2 {
            p.strokes.push(Stroke {
                frame: 0,
                points: vec![[0.0, 0.0], [10.0, 10.0]],
                color: 0xFFFFFF,
                width: 2.0,
                widths: Vec::new(),
                fill: None,
                spray: None,
                opacity: 1.0,
            });
        }
        let event = |figure: Option<usize>| ParticleEvent {
            frame: 0,
            figure,
            node: None,
            x: 0.0,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 1,
        };
        p.particles.push(event(Some(0)));
        p.particles.push(event(Some(0)));
        p.particles.push(event(None));
        let fx = |figure: usize| FxClipEntry {
            clip: ParticleClip {
                start: 0.0,
                duration: 1.0,
                kind: ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 1,
                angle: 0.0,
                length: 10.0,
                color2: None,
            },
            figure,
            node: 0,
            hidden: false,
        };
        p.fx_clips.push(fx(0));
        p.fx_clips.push(fx(1));
        p.audio.push(AudioClip {
            path: "clip.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        p.clips.push(ClipDef {
            name: "wave".into(),
            def: FigureDef::humanoid(),
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        let channel = |figure: usize| Channel {
            figure,
            target: ChannelTarget::RootX,
            keys: Vec::new(),
        };
        p.channels.push(channel(0));
        p.channels.push(channel(0));
        p.channels.push(channel(1));
        p.texts.push(TextInstance {
            content: "Hi".into(),
            size: 24.0,
            color: 0xFFFFFF,
            keys: Vec::new(),
        });
        core
    }

    #[test]
    fn project_totals_count_every_content_vec() {
        let core = stocked_core();
        // Demo base (1 figure, 5 frames) plus the stocked
        // content, in the helper's stable order.
        assert_eq!(
            project_totals(&core),
            vec![
                ("Figures".to_string(), 2),
                ("Frames".to_string(), 5),
                ("Strokes".to_string(), 2),
                ("Particle events".to_string(), 3),
                ("FX clips".to_string(), 2),
                ("Audio clips".to_string(), 1),
                ("Movieclips".to_string(), 1),
                ("Graph channels".to_string(), 3),
                ("Texts".to_string(), 1),
            ]
        );
    }

    #[test]
    fn figure_rows_split_the_indexed_counts_per_figure() {
        let core = stocked_core();
        // Both figures use the humanoid def, so both node
        // counts are its node count.
        let nodes = core.project.figures[0].def.nodes.len();
        assert!(nodes > 0);
        assert_eq!(
            figure_rows(&core),
            vec![
                FigureRow {
                    name: core.project.figures[0].name.clone(),
                    nodes,
                    channels: 2,
                    fx_clips: 1,
                    // Two events anchor to figure 0; the
                    // stage-fixed one (figure: None) counts
                    // for nobody.
                    particles: 2,
                },
                FigureRow {
                    name: "Pip".to_string(),
                    nodes,
                    channels: 1,
                    fx_clips: 1,
                    particles: 0,
                },
            ]
        );
    }

    #[test]
    fn totals_and_rows_for_an_empty_project_are_zeroed() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        core.project.frames.clear();
        let totals = project_totals(&core);
        assert_eq!(totals.len(), 9);
        assert!(
            totals.iter().all(|(_, n)| *n == 0),
            "totals not all zero: {totals:?}"
        );
        assert!(figure_rows(&core).is_empty());
    }

    #[test]
    fn frames_format_whole_fractional_and_degenerate() {
        assert_eq!(format_frames(0.0), "0");
        assert_eq!(format_frames(2.0), "2");
        assert_eq!(format_frames(3.5), "3.5");
        assert_eq!(format_frames(1250.0), "1,250");
        assert_eq!(format_frames(-1.0), "0");
        assert_eq!(format_frames(f32::NAN), "0");
    }

    #[test]
    fn media_totals_on_the_demo_project_are_zeroed() {
        // The demo carries no audio, FX clips, texts or
        // strokes, so every media row reads a plain "0".
        let core = EditorCore::new();
        assert_eq!(
            media_totals(&core),
            vec![
                ("Audio clips".to_string(), "0".to_string()),
                ("FX clips".to_string(), "0".to_string()),
                ("Texts".to_string(), "0".to_string()),
                ("Strokes".to_string(), "0".to_string()),
            ]
        );
    }

    #[test]
    fn media_totals_on_the_stocked_project() {
        let core = stocked_core();
        // Stocked content: one audio clip starting at
        // frame 0, two FX clips of 1.0 frames each, one
        // text and two strokes.
        assert_eq!(
            media_totals(&core),
            vec![
                (
                    "Audio clips".to_string(),
                    "1 (start frame 0)".to_string()
                ),
                (
                    "FX clips".to_string(),
                    "2 · 2 frames of effects".to_string()
                ),
                ("Texts".to_string(), "1".to_string()),
                ("Strokes".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn media_totals_span_audio_starts_and_sum_fx_durations() {
        use stykanimate_rs::model::{AudioClip, FxClipEntry};
        use stykanimate_rs::vfx::{ClipKind, ParticleClip};

        let mut core = EditorCore::new();
        // AudioClip carries no length, so only the start
        // frames can be spanned: starts at 40, 4 and 12
        // read as the range 4–40.
        for start in [40usize, 4, 12] {
            core.project.audio.push(AudioClip {
                path: "clip.wav".into(),
                start_frame: start,
                gain: 1.0,
                muted: false,
            });
        }
        // FX durations do live in the model and sum:
        // 1.5 + 2.0 = 3.5 frames.
        for duration in [1.5f32, 2.0] {
            core.project.fx_clips.push(FxClipEntry {
                clip: ParticleClip {
                    start: 0.0,
                    duration,
                    kind: ClipKind::Burst,
                    color: 0xFFFFFF,
                    seed: 1,
                    angle: 0.0,
                    length: 10.0,
                    color2: None,
                },
                figure: 0,
                node: 0,
                hidden: false,
            });
        }
        let media = media_totals(&core);
        assert_eq!(
            media[0],
            (
                "Audio clips".to_string(),
                "3 (start frames 4–40)".to_string()
            )
        );
        assert_eq!(
            media[1],
            (
                "FX clips".to_string(),
                "2 · 3.5 frames of effects".to_string()
            )
        );
    }

    #[test]
    fn flag_totals_on_the_demo_project_are_zeroed() {
        // The demo has no muted, hidden or translucent
        // content, so every flag row reads a plain "0" —
        // and all three rows are present in order.
        let core = EditorCore::new();
        assert_eq!(
            flag_totals(&core),
            vec![
                ("Muted audio clips".to_string(), "0".to_string()),
                ("Hidden FX clips".to_string(), "0".to_string()),
                ("Translucent strokes".to_string(), "0".to_string()),
            ]
        );
    }

    #[test]
    fn flag_totals_count_muted_hidden_and_translucent() {
        use stykanimate_rs::model::{AudioClip, FxClipEntry, Stroke};
        use stykanimate_rs::vfx::{ClipKind, ParticleClip};

        let mut core = EditorCore::new();
        let p = &mut core.project;
        // Three audio clips, two of them muted.
        for muted in [true, false, true] {
            p.audio.push(AudioClip {
                path: "clip.wav".into(),
                start_frame: 0,
                gain: 1.0,
                muted,
            });
        }
        // Two FX clips, one of them hidden.
        for hidden in [false, true] {
            p.fx_clips.push(FxClipEntry {
                clip: ParticleClip {
                    start: 0.0,
                    duration: 1.0,
                    kind: ClipKind::Burst,
                    color: 0xFFFFFF,
                    seed: 1,
                    angle: 0.0,
                    length: 10.0,
                    color2: None,
                },
                figure: 0,
                node: 0,
                hidden,
            });
        }
        // Three strokes: opaque, half and nearly clear —
        // the two below exactly 1.0 count as translucent.
        for opacity in [1.0f32, 0.5, 0.05] {
            p.strokes.push(Stroke {
                frame: 0,
                points: vec![[0.0, 0.0], [10.0, 10.0]],
                color: 0xFFFFFF,
                width: 2.0,
                widths: Vec::new(),
                fill: None,
                spray: None,
                opacity,
            });
        }
        assert_eq!(
            flag_totals(&core),
            vec![
                ("Muted audio clips".to_string(), "2".to_string()),
                ("Hidden FX clips".to_string(), "1".to_string()),
                ("Translucent strokes".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn label_slugs_match_the_row_key_idiom() {
        assert_eq!(label_slug("Figures"), "figures");
        assert_eq!(label_slug("Particle events"), "particle-events");
        assert_eq!(label_slug("FX clips"), "fx-clips");
        assert_eq!(label_slug("Graph channels"), "graph-channels");
    }
}
