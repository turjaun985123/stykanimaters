//! Stroke / brush inspector (Rune Phase D, section 12/40;
//! per-stroke delete added in Phase E, section 14/40).
//! Declared under `crate::rune_sec` — bin-crate code, not part
//! of the engine library.
//!
//! Detail panel for the active brush preset and the strokes
//! recorded on the current frame. Sibling section 11
//! (`draw_tools`) owns brush *selection* (one button per
//! preset, the cycle button, the eraser toggle, Clear
//! drawing); this section is the inspector view of the same
//! state: the full parameter set of the active preset, and
//! the per-frame stroke list.
//!
//! ## The honest parameter mapping
//!
//! The brief asks for size / opacity / scatter / smoothing
//! steppers and a nib segmented control. The engine has NO
//! per-parameter brush ops: a brush is a preset bundle
//! (`paint::Brush`, paint.rs:304) from the fixed 10-entry
//! `paint::brush_library()` (paint.rs:333), and the only
//! brush ops are `active_brush` (editor.rs:2562),
//! `cycle_brush` (editor.rs:2576) and writing the public
//! `brush_index` field (editor.rs:73) — the write the
//! engine's own tests use to select a brush. There is no
//! `set_brush_width` / `adjust_brush_opacity` / … anywhere
//! (grep-verified), so a stepper that edited one parameter
//! of a custom brush would be a fake control.
//!
//! What the steppers here do instead is real: stepping a
//! parameter selects the nearest preset whose value for
//! THAT parameter is strictly higher / lower (smallest
//! difference, ties to the lowest library index —
//! [`step_index_by`]). The other parameters follow the
//! chosen preset, because presets are bundles; the status
//! line names the resulting brush, exactly as `cycle_brush`
//! reports it. The nib segmented control selects the first
//! library preset carrying the chosen nib
//! ([`first_index_with_nib`]). Every press performs a real
//! selection change (or reports, via the status line, that
//! no preset steps that way).
//!
//! ## Stroke list
//!
//! One row per stroke on the current frame
//! (`Project.strokes`, model.rs:703, filtered by
//! `Stroke.frame == EditorCore.current`), with a colour
//! swatch, a data summary (points, width, fill, spray
//! mark) and a **Delete** button. The button calls the
//! per-stroke delete op `EditorCore::delete_stroke_at`
//! (added in Phase E, section 7) with the stroke's index
//! in `Project.strokes` — carried on the row data
//! ([`StrokeRow::project_index`]), never the row position,
//! because the frame filter makes the two differ whenever
//! other frames carry strokes. The frame-scoped Clear
//! (`clear_frame_drawings`, editor.rs:2391) remains as
//! the bulk removal above the list.
//!
//! ## Bulk ops (Phase F, section 26)
//!
//! A row at the panel top acts on **every** stroke in
//! `Project.strokes`, across all frames — deliberately a
//! wider scope than the frame-filtered list below it,
//! because the frame-scoped removal already exists (the
//! Clear button above). **Recolour all** rewrites every
//! stroke's base `color` to the bulk target colour
//! ([`bulk_target_color`]: the selected node's colour,
//! falling back to the selected figure's — the stage's
//! own node-over-figure resolution); a stroke's `fill`
//! and spray mark keep their own values. **Delete all**
//! empties `Project.strokes`.
//!
//! Undo composition: there is no bulk op in the core,
//! and `delete_stroke_at` snapshots **per call**, so
//! looping it would flood the undo stack with one step
//! per stroke. Both bulk ops here therefore compose the
//! core's public parts exactly as `delete_stroke_at`
//! itself is built: one `EditorCore::snapshot()`
//! (editor.rs:202 — public, serialises the whole
//! project, strokes included) followed by direct
//! `project.strokes` writes — one press, one undo step.
//! An empty stroke list takes no snapshot at all: the
//! press only reports that there are no strokes.
//!
//! ## Stroke selection (Phase G, section 7)
//!
//! A selection row under the bulk row steps a
//! **project-wide** stroke selection: `State.stroke_selected`
//! (declared in main.rs) holds an index into
//! `Project.strokes` — the same index space
//! `delete_stroke_at` and the stage's
//! `stage_layers::highlight_geometry` use, and the space
//! the stage section reads when it draws the highlight.
//! The per-frame list further down is only a view onto
//! that space (its rows carry
//! [`StrokeRow::project_index`]); the selection ignores
//! the frame filter, so Prev / Next walk every stroke in
//! the project, wrapping at both ends, and the readout
//! names the selected stroke's own frame
//! ([`stroke_frame_of`]) so the user can jump there with
//! **Go to frame** (`EditorCore::goto` — the core's
//! navigation op: no snapshot, no status of its own).
//!
//! Selection is shell view state: stepping and clearing
//! write nothing but `s.stroke_selected` — no snapshot,
//! no core mutation — so they add no undo steps and no
//! status toasts. A stale selection (index ≥ the stroke
//! count, e.g. after a bulk delete elsewhere) is treated
//! as unset everywhere: the readout falls back to "No
//! stroke selected" and stepping re-enters at the first
//! (+1) or last (−1) stroke ([`step_selection`]). On an
//! empty project the row reads "No strokes" and every
//! button is inert apart from the status line.
//!
//! ## Selected stroke measure (Phase I, section 33)
//!
//! While a valid selection exists, a muted line under
//! the selection row (key `stroke-measure`) reports the
//! selected stroke's facts ([`stroke_measure`]):
//! `Stroke {n}: {points} pts · {length} long · frame
//! {f}`. The length sums the base polyline's segment
//! lengths — a spray stroke measures by its base
//! polyline, the picking convention of Phase G section
//! 5 — and a filled stroke's closing edge back to its
//! first point is not counted. No selection (or a stale
//! one) renders no line at all, so an unselected panel
//! gains nothing.
//!
//! `State` access: the brush and stroke data read
//! `s.core` (active brush, `brush_index`,
//! `project.strokes`, `current`) and writes go through
//! the `brush_index` field write, the frame-clear op,
//! `delete_stroke_at` or the bulk composition above;
//! the selection row additionally reads and writes
//! `s.stroke_selected`, declared in main.rs — no field
//! is declared in this file.
//!
//! ## Icons (Phase H, section 13)
//!
//! The bulk row and the selection row render through
//! the shell's icon buttons (`rune_sec::icon_btn`):
//! Recolour all carries `palette`, Delete all
//! `trash-can`, Go to frame `crosshairs` (each with its
//! text label, as before), while Prev / Next / Clear
//! are icon-only (`chevron-left` / `chevron-right` /
//! `xmark`) — their labels were pure navigation glyphs
//! in text form. Element keys are unchanged
//! (`stroke-recolor-all`, `stroke-delete-all`,
//! `stroke-sel-prev` / `-next` / `-goto` / `-clear`),
//! and the handlers are the same presses: the selection
//! buttons still write nothing but `s.stroke_selected`
//! (plus `goto`'s navigation), exactly as documented
//! above.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Stroke;
use stykanimate_rs::paint::{Brush, Nib};

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::stage_view::hex_color;
use crate::State;

/// The three nib kinds, in segmented-control display order.
pub const NIB_ORDER: [Nib; 3] = [Nib::Round, Nib::Square, Nib::Calligraphy];

/// A brush parameter a stepper can step through the preset
/// library by. The parameters are the preset-bundle fields
/// of `paint::Brush` (paint.rs:304).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Size,
    Opacity,
    Scatter,
    Smoothing,
}

impl Param {
    /// This parameter's value on a preset.
    pub fn value(self, b: &Brush) -> f32 {
        match self {
            Param::Size => b.width,
            Param::Opacity => b.opacity,
            Param::Scatter => b.scatter,
            Param::Smoothing => b.smoothing as f32,
        }
    }

    /// Status text when no preset steps in direction `dir`
    /// (the stepper press is then a no-op apart from this
    /// honest report).
    pub fn no_step_message(self, dir: i32) -> &'static str {
        match (self, dir > 0) {
            (Param::Size, true) => "No wider brush in the library.",
            (Param::Size, false) => "No narrower brush in the library.",
            (Param::Opacity, true) => "No more opaque brush in the library.",
            (Param::Opacity, false) => "No fainter brush in the library.",
            (Param::Scatter, true) => "No broader-scatter brush in the library.",
            (Param::Scatter, false) => "No tighter-scatter brush in the library.",
            (Param::Smoothing, true) => "No smoother brush in the library.",
            (Param::Smoothing, false) => "No rougher brush in the library.",
        }
    }
}

/// The active brush's library index, clamped exactly as
/// `EditorCore::active_brush` clamps it, so a stale
/// `brush_index` can never point outside the library.
pub fn active_index(core: &EditorCore) -> usize {
    let len = stykanimate_rs::paint::brush_library().len();
    if len == 0 {
        return 0;
    }
    core.brush_index.min(len - 1)
}

/// Step through a preset library by one parameter: from
/// `current`, the index whose `key` value is nearest in
/// direction `dir` (+1 = strictly greater, −1 = strictly
/// smaller; smallest absolute difference wins, ties go to
/// the lowest index). `None` when no preset steps that way,
/// the library is empty, `current` is out of range, or
/// `dir` is 0.
pub fn step_index_by(
    lib: &[Brush],
    current: usize,
    dir: i32,
    key: impl Fn(&Brush) -> f32,
) -> Option<usize> {
    if lib.is_empty() || current >= lib.len() || dir == 0 {
        return None;
    }
    let cur = key(&lib[current]);
    let mut best: Option<(f32, usize)> = None;
    for (i, b) in lib.iter().enumerate() {
        if i == current {
            continue;
        }
        let v = key(b);
        if (dir > 0 && v <= cur) || (dir < 0 && v >= cur) {
            continue;
        }
        let d = (v - cur).abs();
        match best {
            Some((bd, _)) if d >= bd => {}
            _ => best = Some((d, i)),
        }
    }
    best.map(|(_, i)| i)
}

/// The first library preset carrying `nib` — the nib
/// segmented control's selection target (Round → Pen,
/// Square → Brush, Calligraphy → Calligraphy).
pub fn first_index_with_nib(lib: &[Brush], nib: Nib) -> Option<usize> {
    lib.iter().position(|b| b.nib == nib)
}

/// Select preset `i`: the public `brush_index` field write
/// (clamped) plus the exact status string `cycle_brush`
/// sets — the same selection the engine's own tests perform
/// when they write the field directly.
fn apply_preset(core: &mut EditorCore, i: usize) {
    let lib = stykanimate_rs::paint::brush_library();
    if lib.is_empty() {
        return;
    }
    core.brush_index = i.min(lib.len() - 1);
    let b = lib[core.brush_index];
    core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
}

/// Stepper press: select the nearest preset stepping
/// parameter `param` in direction `dir`. Returns whether
/// the selection moved; when it cannot, the status line
/// says so (see [`Param::no_step_message`]).
pub fn step_param(core: &mut EditorCore, param: Param, dir: i32) -> bool {
    let lib = stykanimate_rs::paint::brush_library();
    let cur = active_index(core);
    match step_index_by(&lib, cur, dir, |b| param.value(b)) {
        Some(i) => {
            apply_preset(core, i);
            true
        }
        None => {
            core.status = param.no_step_message(dir).into();
            false
        }
    }
}

/// Nib segmented press: select the first preset carrying
/// `nib`. Returns whether a preset was selected (every nib
/// in [`NIB_ORDER`] exists in the shipped library, so this
/// is always true in practice).
pub fn select_nib(core: &mut EditorCore, nib: Nib) -> bool {
    let lib = stykanimate_rs::paint::brush_library();
    match first_index_with_nib(&lib, nib) {
        Some(i) => {
            apply_preset(core, i);
            true
        }
        None => false,
    }
}

/// A number the way the panel shows sizes and radii: whole
/// values without a fraction (`4`), others with one decimal
/// (`2.5`) — the library's widths are all whole, but loaded
/// data need not be.
pub fn width_text(w: f32) -> String {
    if w.fract() == 0.0 {
        format!("{}", w as i64)
    } else {
        format!("{w:.1}")
    }
}

/// Opacity as a percent (`100%`, `50%`).
pub fn opacity_text(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i64)
}

/// Scatter radius: `Off` at 0 (marks land exactly on the
/// recorded path), otherwise the radius in stage units.
pub fn scatter_text(scatter: f32) -> String {
    if scatter <= 0.0 {
        "Off".to_string()
    } else {
        width_text(scatter)
    }
}

/// Short display label for a nib kind.
pub fn nib_label(nib: Nib) -> &'static str {
    match nib {
        Nib::Round => "Round",
        Nib::Square => "Square",
        Nib::Calligraphy => "Calligraphy",
    }
}

/// The segmented control's element key for a nib.
pub fn nib_key(nib: Nib) -> &'static str {
    match nib {
        Nib::Round => "brush-nib-round",
        Nib::Square => "brush-nib-square",
        Nib::Calligraphy => "brush-nib-calligraphy",
    }
}

/// A preset's nib display: the kind, plus the edge angle
/// when it is non-zero (a round nib's angle is meaningless
/// and several presets leave it 0 — `Square`, not
/// `Square · 0°`).
pub fn nib_text(brush: &Brush) -> String {
    if brush.nib == Nib::Round || brush.nib_angle == 0.0 {
        nib_label(brush.nib).to_string()
    } else {
        format!("{} · {:.0}°", nib_label(brush.nib), brush.nib_angle)
    }
}

/// Smoothing display: `Off` at 0 Chaikin iterations, else
/// the pass count.
pub fn smoothing_text(smoothing: usize) -> String {
    match smoothing {
        0 => "Off".to_string(),
        1 => "1 pass".to_string(),
        n => format!("{n} passes"),
    }
}

/// One-line explainer shown when the active brush scatters
/// (today only Spray): the mark a stroke records, in the
/// model's own terms (`model::SprayMark`, model.rs:473 —
/// the seed is per-stroke, the radius and opacity come from
/// the brush, editor.rs:2303).
pub fn spray_hint(brush: &Brush) -> Option<String> {
    if brush.scatter <= 0.0 {
        return None;
    }
    Some(format!(
        "Spray brush: strokes record a spray mark (radius {}, opacity {}).",
        width_text(brush.scatter),
        opacity_text(brush.opacity)
    ))
}

/// The draw-tool readout: which tool is active and which
/// preset sizes it (the preset "applies to whichever tool
/// is active" — it also sizes the eraser, editor.rs:2573).
pub fn tool_text(core: &EditorCore) -> String {
    let brush = core.active_brush();
    if core.draw_tool_is_eraser() {
        format!("Tool: Eraser (sized by {})", brush.name)
    } else {
        format!("Tool: Brush ({})", brush.name)
    }
}

/// Indices into `Project.strokes` of the strokes recorded
/// on the current frame, in project order.
pub fn frame_stroke_indices(core: &EditorCore) -> Vec<usize> {
    core.project
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.frame == core.current)
        .map(|(i, _)| i)
        .collect()
}

/// How many strokes the current frame carries.
pub fn frame_stroke_count(core: &EditorCore) -> usize {
    core.project
        .strokes
        .iter()
        .filter(|s| s.frame == core.current)
        .count()
}

/// The stroke-list header: `Strokes · Frame {n} · {count}`.
pub fn strokes_header(core: &EditorCore) -> String {
    format!(
        "Strokes · Frame {} · {}",
        core.current + 1,
        frame_stroke_count(core)
    )
}

/// One stroke row's summary: point count and base width,
/// plus `filled` when the stroke carries a fill colour and
/// the spray mark's radius/opacity when it carries one
/// (`Stroke`, model.rs:481).
pub fn stroke_summary(stroke: &Stroke) -> String {
    let mut out = format!(
        "{} pts · w {}",
        stroke.points.len(),
        width_text(stroke.width)
    );
    if stroke.fill.is_some() {
        out.push_str(" · filled");
    }
    if let Some(mark) = &stroke.spray {
        out.push_str(&format!(
            " · spray r{} · {}",
            width_text(mark.radius),
            opacity_text(mark.opacity)
        ));
    }
    out
}

/// One stroke-list row's data. The list is filtered to the
/// current frame, so a row's display position and its
/// stroke's index in `Project.strokes` differ whenever
/// other frames carry strokes — the Delete button must
/// target [`StrokeRow::project_index`], never the position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrokeRow {
    /// Index into `Project.strokes` — the delete target
    /// and the row button's key suffix.
    pub project_index: usize,
    /// 1-based position within this frame's list (the
    /// `#n` label).
    pub position: usize,
    /// The stroke's colour (swatch fill).
    pub color: u32,
    /// The one-line data summary (see [`stroke_summary`]).
    pub summary: String,
}

/// The current frame's stroke rows, in project order,
/// built over [`frame_stroke_indices`] so every row
/// carries its stroke's real `Project.strokes` index.
pub fn stroke_rows(core: &EditorCore) -> Vec<StrokeRow> {
    frame_stroke_indices(core)
        .into_iter()
        .enumerate()
        .map(|(k, idx)| {
            let stroke = &core.project.strokes[idx];
            StrokeRow {
                project_index: idx,
                position: k + 1,
                color: stroke.color,
                summary: stroke_summary(stroke),
            }
        })
        .collect()
}

/// Row Delete press: remove the stroke at
/// `project_index` in `Project.strokes` via the core's
/// per-stroke delete op (`EditorCore::delete_stroke_at`,
/// Phase E section 7 — snapshot, removal and status are
/// the op's business). Later strokes shift down one
/// index; the panel rebuilds its rows from the core on
/// the next view pass, so no row data is ever reused
/// across a deletion.
pub fn delete_stroke(core: &mut EditorCore, project_index: usize) {
    core.delete_stroke_at(project_index);
}

/// Total strokes in the project, across all frames — the
/// size of the selection's index space. (The frame list
/// above counts only the current frame; the selection
/// deliberately does not.)
pub fn stroke_total(core: &EditorCore) -> usize {
    core.project.strokes.len()
}

/// The frame the stroke at project-wide `index` was
/// recorded on (`Stroke.frame`, model.rs:482), or `None`
/// when `index` is outside `Project.strokes`. This is
/// the mapping the selection readout and **Go to frame**
/// need: the selection lives in the project-wide space,
/// but a stroke is viewed on its own frame.
pub fn stroke_frame_of(core: &EditorCore, index: usize) -> Option<usize> {
    core.project.strokes.get(index).map(|s| s.frame)
}

/// Step a project-wide stroke selection by `dir`
/// (+1 = next, −1 = previous) over `total` strokes,
/// wrapping at both ends. A missing **or stale**
/// selection (`cur >= total`, e.g. after strokes were
/// deleted elsewhere) is treated as unset: +1 enters at
/// the first stroke, −1 at the last. `total == 0` never
/// selects anything; `dir == 0` holds a valid selection
/// and drops an unset or stale one.
pub fn step_selection(cur: Option<usize>, total: usize, dir: i32) -> Option<usize> {
    if total == 0 {
        return None;
    }
    let valid = cur.filter(|&i| i < total);
    match dir.signum() {
        1 => Some(match valid {
            Some(i) => (i + 1) % total,
            None => 0,
        }),
        -1 => Some(match valid {
            Some(i) => (i + total - 1) % total,
            None => total - 1,
        }),
        _ => valid,
    }
}

/// The selection row's readout. With a valid selection:
/// `Stroke {i+1} of {n} · frame {f+1}` — the stroke's
/// 1-based position in the project-wide space, the
/// project total, and the 1-based frame it lives on.
/// A missing or stale selection reads as no selection;
/// an empty project reads "No strokes".
pub fn selection_readout(core: &EditorCore, selected: Option<usize>) -> String {
    let total = core.project.strokes.len();
    if total == 0 {
        return "No strokes".to_string();
    }
    match selected.filter(|&i| i < total) {
        Some(i) => {
            // `i < total`, so the stroke — and its frame —
            // exist; the fallback is unreachable.
            let frame = stroke_frame_of(core, i).unwrap_or(0);
            format!("Stroke {} of {} · frame {}", i + 1, total, frame + 1)
        }
        None => format!(
            "No stroke selected · {} stroke{}",
            total,
            if total == 1 { "" } else { "s" }
        ),
    }
}

/// Measured facts about one stroke: its polyline point
/// count, its total length in stage units, and the frame
/// it was recorded on. Produced by [`stroke_measure`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeMeasure {
    /// Polyline point count (`Stroke.points.len()`).
    pub points: usize,
    /// Summed segment lengths along the base polyline, in
    /// stage units. A stroke with fewer than two points
    /// measures 0.
    pub length: f32,
    /// The frame the stroke was recorded on — the raw
    /// `Stroke.frame` value, as [`stroke_frame_of`]
    /// returns it (displays add 1).
    pub frame: usize,
}

/// Measure the stroke at project-wide `index`: point
/// count, total length and owning frame, or `None` when
/// `index` is outside `Project.strokes`. The length sums
/// the Euclidean lengths of the consecutive segments of
/// the stroke's **base polyline** — a spray stroke is
/// measured by that polyline too, never by its scattered
/// dots, matching the stage picking convention
/// (`stage_layers::stroke_at_point`, Phase G section 5).
/// A filled stroke is measured along its open polyline:
/// the closing edge back to the first point is not
/// counted. Pure: same project state, same measure.
pub fn stroke_measure(core: &EditorCore, index: usize) -> Option<StrokeMeasure> {
    let stroke = core.project.strokes.get(index)?;
    let length = stroke
        .points
        .windows(2)
        .map(|seg| {
            let d = [seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]];
            (d[0] * d[0] + d[1] * d[1]).sqrt()
        })
        .sum();
    Some(StrokeMeasure {
        points: stroke.points.len(),
        length,
        frame: stroke_frame_of(core, index)?,
    })
}

/// The measure line shown under the selection row while
/// a valid selection exists: `Stroke {n}: {points} pts ·
/// {length} long · frame {f}` — the stroke's 1-based
/// project position, its point count, its total length
/// rounded to whole stage units, and its 1-based frame.
/// `None` — and no line is rendered — with no selection,
/// a stale one, or an empty project.
pub fn measure_readout(core: &EditorCore, selected: Option<usize>) -> Option<String> {
    let i = selected.filter(|&i| i < core.project.strokes.len())?;
    let m = stroke_measure(core, i)?;
    Some(format!(
        "Stroke {}: {} pts · {:.0} long · frame {}",
        i + 1,
        m.points,
        m.length,
        m.frame + 1
    ))
}

/// Prev / Next press: step `s.stroke_selected` through
/// the project-wide stroke space ([`step_selection`]).
/// View state only — no snapshot, no status, no core
/// write. On an empty project the press only reports
/// that there are no strokes (and drops any stale
/// selection).
pub fn step_stroke_selection(s: &mut State, dir: i32) {
    let total = s.core.project.strokes.len();
    if total == 0 {
        s.stroke_selected = None;
        s.core.status = "No strokes to select.".into();
        return;
    }
    s.stroke_selected = step_selection(s.stroke_selected, total, dir);
}

/// Clear press: drop the selection. View state only,
/// like stepping; on an empty project the press only
/// reports that there are no strokes.
pub fn clear_stroke_selection(s: &mut State) {
    if s.core.project.strokes.is_empty() {
        s.core.status = "No strokes to select.".into();
        return;
    }
    s.stroke_selected = None;
}

/// Go to frame press: navigate to the selected stroke's
/// frame via `EditorCore::goto` — navigation, the one
/// core write the selection row performs (no snapshot:
/// `goto` is the same op the timeline chips call).
/// Returns whether it navigated; with no (or a stale)
/// selection, or on an empty project, the press only
/// reports why via the status line.
pub fn goto_selected_stroke_frame(s: &mut State) -> bool {
    let total = s.core.project.strokes.len();
    if total == 0 {
        s.core.status = "No strokes to select.".into();
        return false;
    }
    match s.stroke_selected.filter(|&i| i < total) {
        Some(i) => {
            let frame = stroke_frame_of(&s.core, i).unwrap_or(0);
            s.core.goto(frame);
            true
        }
        None => {
            s.core.status = "No stroke selected.".into();
            false
        }
    }
}

/// The bulk Recolour's target colour: the selected
/// node's own colour when a node is selected and
/// carries one, else the selected figure's colour —
/// the same node-over-figure resolution the stage
/// draws with (`stage_view`'s node-colour closure
/// reads `node.color` over the figure's base colour).
pub fn bulk_target_color(core: &EditorCore) -> u32 {
    let fig = &core.project.figures[core.selected_figure];
    match core.selected_node {
        Some(ni) => fig
            .def
            .nodes
            .get(ni)
            .and_then(|node| node.color)
            .unwrap_or(fig.color),
        None => fig.color,
    }
}

/// Bulk Recolour press: rewrite every stroke's base
/// `color` (all frames — see the module docs for the
/// scope choice) to [`bulk_target_color`]. Exactly one
/// snapshot, taken only when there is something to
/// recolour; on an empty stroke list the press only
/// reports that there are no strokes. Returns the
/// number of strokes recoloured.
pub fn recolor_all_strokes(core: &mut EditorCore) -> usize {
    let n = core.project.strokes.len();
    if n == 0 {
        core.status = "No strokes to recolour.".into();
        return 0;
    }
    let color = bulk_target_color(core);
    // One undo step for the whole press: `snapshot()`
    // is the same public call `delete_stroke_at`
    // makes per stroke; composing it once here and
    // writing the field directly is what keeps the
    // bulk op to a single step.
    core.snapshot();
    for stroke in &mut core.project.strokes {
        stroke.color = color;
    }
    core.status = format!(
        "Recoloured {n} stroke{}.",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Bulk Delete press: remove every stroke in
/// `Project.strokes` (all frames). Same one-snapshot
/// composition as [`recolor_all_strokes`]; looping
/// `delete_stroke_at` instead would cost one undo
/// step per stroke. On an empty stroke list the
/// press only reports that there are no strokes.
/// Returns the number of strokes removed.
pub fn delete_all_strokes(core: &mut EditorCore) -> usize {
    let n = core.project.strokes.len();
    if n == 0 {
        core.status = "No strokes to delete.".into();
        return 0;
    }
    core.snapshot();
    core.project.strokes.clear();
    core.status = format!(
        "Deleted {n} stroke{}.",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// One parameter row: label, current value, and the −/+
/// stepper pair stepping that parameter through the preset
/// library ([`step_param`]).
fn param_row(
    label: &str,
    value: String,
    dec_key: &'static str,
    inc_key: &'static str,
    param: Param,
    t: &Theme,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(text(label).font(type_::LABEL).color(t.text))
        .child(
            text(value)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(btn(dec_key, "-", false, t, move |s: &mut State| {
            step_param(&mut s.core, param, -1);
        }))
        .child(btn(inc_key, "+", false, t, move |s: &mut State| {
            step_param(&mut s.core, param, 1);
        }))
}

/// The stroke inspector panel: the bulk stroke row and
/// the project-wide selection row, then the active
/// brush's parameter set (steppers + nib segmented
/// control, all stepping the preset selection — see the
/// module docs), then the current frame's stroke list
/// with per-row Delete buttons and the frame Clear
/// button.
pub fn stroke_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let brush = core.active_brush();
    let mut panel = col().key("stroke-inspector").w_full().gap(8.0);

    // Bulk stroke ops, at the panel top per the brief:
    // both act on every stroke in the project (all
    // frames) as a single undo step — see the module
    // docs. Always rendered: on an empty stroke list a
    // press reports "no strokes" and changes nothing.
    panel = panel.child(
        row()
            .key("stroke-bulk-row")
            .gap(8.0)
            .center_y()
            .child(
                text("All strokes")
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(icon_btn(
                "stroke-recolor-all",
                "palette",
                "Recolour all",
                false,
                t,
                |s| {
                    recolor_all_strokes(&mut s.core);
                },
            ))
            .child(icon_btn(
                "stroke-delete-all",
                "trash-can",
                "Delete all",
                false,
                t,
                |s| {
                    delete_all_strokes(&mut s.core);
                },
            )),
    );

    // Stroke selection (Phase G): project-wide stepping
    // through `Project.strokes` — the stage draws the
    // highlight for `s.stroke_selected`. View state
    // only (see the module docs); always rendered, and
    // on an empty project the readout says "No strokes"
    // and the buttons only report via the status line.
    panel = panel.child(
        row()
            .key("stroke-sel-row")
            .gap(8.0)
            .center_y()
            .child(
                text(selection_readout(core, s.stroke_selected))
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(icon_only_btn(
                "stroke-sel-prev",
                "chevron-left",
                26.0, false,
                t,
                |s| {
                    step_stroke_selection(s, -1);
                },
            ))
            .child(icon_only_btn(
                "stroke-sel-next",
                "chevron-right",
                26.0, false,
                t,
                |s| {
                    step_stroke_selection(s, 1);
                },
            ))
            .child(icon_btn(
                "stroke-sel-goto",
                "crosshairs",
                "Go to frame",
                false,
                t,
                |s| {
                    goto_selected_stroke_frame(s);
                },
            ))
            .child(icon_only_btn("stroke-sel-clear", "xmark", 26.0, false, t, |s| {
                clear_stroke_selection(s);
            })),
    );

    // Selected stroke measure (Phase I): a muted facts
    // line under the selection row, rendered only while
    // a valid selection exists — no selection, no line.
    if let Some(line) = measure_readout(core, s.stroke_selected) {
        panel = panel.child(
            text(line)
                .key("stroke-measure")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    panel = panel.child(
        text(format!("Brush · {}", brush.name))
            .font(type_::LABEL)
            .color(t.text),
    );
    panel = panel.child(
        text(tool_text(core))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    panel = panel.child(param_row(
        "Size",
        width_text(brush.width),
        "brush-size-dec",
        "brush-size-inc",
        Param::Size,
        t,
    ));
    panel = panel.child(param_row(
        "Opacity",
        opacity_text(brush.opacity),
        "brush-opacity-dec",
        "brush-opacity-inc",
        Param::Opacity,
        t,
    ));
    panel = panel.child(param_row(
        "Scatter",
        scatter_text(brush.scatter),
        "brush-scatter-dec",
        "brush-scatter-inc",
        Param::Scatter,
        t,
    ));

    // Nib: segmented control of the three kinds; a chip
    // press selects the first preset carrying that nib.
    let mut nib_row = row()
        .key("brush-nib-row")
        .gap(8.0)
        .center_y()
        .child(text("Nib").font(type_::LABEL).color(t.text));
    for nib in NIB_ORDER {
        nib_row = nib_row.child(btn(
            nib_key(nib),
            nib_label(nib),
            brush.nib == nib,
            t,
            move |s: &mut State| {
                select_nib(&mut s.core, nib);
            },
        ));
    }
    panel = panel.child(nib_row);
    panel = panel.child(
        text(format!("Nib angle · {:.0}°", brush.nib_angle))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    panel = panel.child(param_row(
        "Smoothing",
        smoothing_text(brush.smoothing),
        "brush-smoothing-dec",
        "brush-smoothing-inc",
        Param::Smoothing,
        t,
    ));

    if let Some(hint) = spray_hint(&brush) {
        panel = panel.child(text(hint).font(type_::LABEL).color(t.text_secondary));
    }

    // ---- Current frame's strokes ----
    let rows = stroke_rows(core);
    let mut header = row()
        .key("strokes-header")
        .gap(8.0)
        .center_y()
        .child(
            text(strokes_header(core))
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0),
        );
    if !rows.is_empty() {
        header = header.child(btn("strokes-clear", "Clear frame", false, t, |s| {
            s.core.clear_frame_drawings()
        }));
    }
    panel = panel.child(header);

    if rows.is_empty() {
        panel = panel.child(
            text("No strokes on this frame.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        for srow in rows {
            let idx = srow.project_index;
            panel = panel.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(
                        el()
                            .size(14.0, 14.0)
                            .radius(4.0)
                            .fill(hex_color(srow.color).unwrap_or(t.text)),
                    )
                    .child(
                        text(format!("#{}", srow.position))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                    )
                    .child(
                        text(srow.summary)
                            .font(type_::LABEL)
                            .color(t.text)
                            .grow(1.0),
                    )
                    .child(btn(
                        &format!("stroke-del-{idx}"),
                        "Delete",
                        false,
                        t,
                        move |s: &mut State| {
                            delete_stroke(&mut s.core, idx);
                        },
                    )),
            );
        }
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::SprayMark;

    fn lib() -> Vec<Brush> {
        stykanimate_rs::paint::brush_library()
    }

    fn plain_stroke(frame: usize, points: usize) -> Stroke {
        Stroke {
            frame,
            points: vec![[0.0, 0.0]; points],
            color: 0xFF_0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
        }
    }

    #[test]
    fn number_and_percent_formats() {
        assert_eq!(width_text(4.0), "4");
        assert_eq!(width_text(20.0), "20");
        assert_eq!(width_text(2.5), "2.5");
        assert_eq!(opacity_text(1.0), "100%");
        assert_eq!(opacity_text(0.5), "50%");
        assert_eq!(scatter_text(0.0), "Off");
        assert_eq!(scatter_text(12.0), "12");
        assert_eq!(smoothing_text(0), "Off");
        assert_eq!(smoothing_text(1), "1 pass");
        assert_eq!(smoothing_text(3), "3 passes");
    }

    #[test]
    fn nib_text_shows_angle_only_when_it_matters() {
        let lib = lib();
        let by_name = |name: &str| lib.iter().find(|b| b.name == name).copied().unwrap();
        assert_eq!(nib_text(&by_name("Pen")), "Round");
        // Square at 0°: no angle shown.
        assert_eq!(nib_text(&by_name("Brush")), "Square");
        assert_eq!(nib_text(&by_name("Paint Brush")), "Square · 45°");
        assert_eq!(nib_text(&by_name("Calligraphy")), "Calligraphy · 30°");
        assert_eq!(nib_text(&by_name("Calligraphy L")), "Calligraphy · 315°");
    }

    #[test]
    fn step_by_size_walks_to_the_nearest_width() {
        let lib = lib();
        // Pen (idx 0, width 4): next wider is Chisel Marker
        // (idx 7, width 5); next narrower is Pencil (idx 1).
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.width), Some(7));
        assert_eq!(step_index_by(&lib, 0, -1, |b| b.width), Some(1));
        // Broad Brush (idx 8, width 20) is the widest; Pencil
        // (idx 1, width 2) the narrowest.
        assert_eq!(step_index_by(&lib, 8, 1, |b| b.width), None);
        assert_eq!(step_index_by(&lib, 1, -1, |b| b.width), None);
        // Degenerate inputs never step.
        assert_eq!(step_index_by(&lib, 0, 0, |b| b.width), None);
        assert_eq!(step_index_by(&lib, 99, 1, |b| b.width), None);
    }

    #[test]
    fn step_by_opacity_and_scatter_reach_spray() {
        let lib = lib();
        // Only Spray (idx 9) is below full opacity / above
        // zero scatter; from Spray, +opacity ties across all
        // full-opacity presets and resolves to the lowest
        // index (Pen).
        assert_eq!(step_index_by(&lib, 0, -1, |b| b.opacity), Some(9));
        assert_eq!(step_index_by(&lib, 9, 1, |b| b.opacity), Some(0));
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.scatter), Some(9));
        assert_eq!(step_index_by(&lib, 9, -1, |b| b.scatter), Some(0));
    }

    #[test]
    fn step_by_smoothing_ties_to_the_lowest_index() {
        let lib = lib();
        // Four presets smooth once; from Pen (0) the nearest
        // step up is the lowest-indexed of them: Marker (2).
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.smoothing as f32), Some(2));
        // Paint Brush (idx 5, 2 passes) steps up to Broad
        // Brush (idx 8, 3 passes — the library maximum),
        // which itself cannot step up.
        assert_eq!(step_index_by(&lib, 5, 1, |b| b.smoothing as f32), Some(8));
        assert_eq!(step_index_by(&lib, 8, 1, |b| b.smoothing as f32), None);
    }

    #[test]
    fn first_preset_per_nib() {
        let lib = lib();
        assert_eq!(first_index_with_nib(&lib, Nib::Round), Some(0));
        assert_eq!(first_index_with_nib(&lib, Nib::Calligraphy), Some(3));
        assert_eq!(first_index_with_nib(&lib, Nib::Square), Some(4));
        for nib in NIB_ORDER {
            assert!(!nib_label(nib).is_empty());
            assert!(nib_key(nib).starts_with("brush-nib-"));
        }
    }

    #[test]
    fn step_param_moves_the_selection_and_reports_like_cycle_brush() {
        let mut core = EditorCore::new();
        assert_eq!(core.active_brush().name, "Pen");
        assert!(step_param(&mut core, Param::Size, 1));
        assert_eq!(core.brush_index, 7);
        assert_eq!(core.active_brush().name, "Chisel Marker");
        assert_eq!(core.status, "Brush: Chisel Marker (width 5).");
        // The widest brush cannot step up; the status says so
        // and the selection stays.
        core.brush_index = 8;
        assert!(!step_param(&mut core, Param::Size, 1));
        assert_eq!(core.brush_index, 8);
        assert_eq!(core.status, "No wider brush in the library.");
    }

    #[test]
    fn select_nib_picks_the_first_preset_with_that_nib() {
        let mut core = EditorCore::new();
        assert!(select_nib(&mut core, Nib::Square));
        assert_eq!(core.active_brush().name, "Brush");
        assert!(select_nib(&mut core, Nib::Calligraphy));
        assert_eq!(core.active_brush().name, "Calligraphy");
        assert!(select_nib(&mut core, Nib::Round));
        assert_eq!(core.active_brush().name, "Pen");
    }

    #[test]
    fn spray_hint_only_for_scatter_brushes() {
        let lib = lib();
        let spray = lib.iter().find(|b| b.name == "Spray").unwrap();
        assert_eq!(
            spray_hint(spray),
            Some("Spray brush: strokes record a spray mark (radius 12, opacity 50%).".to_string())
        );
        assert_eq!(spray_hint(&lib[0]), None);
    }

    #[test]
    fn tool_text_names_the_tool_and_its_sizing_brush() {
        let mut core = EditorCore::new();
        assert_eq!(tool_text(&core), "Tool: Brush (Pen)");
        core.toggle_draw_tool();
        assert_eq!(tool_text(&core), "Tool: Eraser (sized by Pen)");
    }

    #[test]
    fn frame_stroke_list_filters_by_current_frame() {
        let mut core = EditorCore::new();
        assert_eq!(frame_stroke_count(&core), 0);
        assert_eq!(strokes_header(&core), "Strokes · Frame 1 · 0");
        core.project.strokes.push(plain_stroke(0, 3));
        core.project.strokes.push(plain_stroke(1, 5));
        core.project.strokes.push(plain_stroke(0, 7));
        assert_eq!(frame_stroke_indices(&core), vec![0, 2]);
        assert_eq!(frame_stroke_count(&core), 2);
        assert_eq!(strokes_header(&core), "Strokes · Frame 1 · 2");
        core.goto(1);
        assert_eq!(frame_stroke_indices(&core), vec![1]);
        assert_eq!(strokes_header(&core), "Strokes · Frame 2 · 1");
    }

    #[test]
    fn stroke_summary_reports_points_width_fill_and_spray() {
        let plain = plain_stroke(0, 3);
        assert_eq!(stroke_summary(&plain), "3 pts · w 4");
        let mut filled = plain_stroke(0, 12);
        filled.fill = Some(0xFF_0000);
        assert_eq!(stroke_summary(&filled), "12 pts · w 4 · filled");
        let mut sprayed = plain_stroke(0, 20);
        sprayed.spray = Some(SprayMark {
            seed: 42,
            radius: 12.0,
            opacity: 0.5,
        });
        assert_eq!(stroke_summary(&sprayed), "20 pts · w 4 · spray r12 · 50%");
    }

    #[test]
    fn stroke_rows_carry_project_indices_across_interleaved_frames() {
        let mut core = EditorCore::new();
        // Interleave frames 0, 1, 0, 1, 0; distinct point
        // counts make each stroke identifiable by summary.
        for (frame, pts) in [(0, 1), (1, 2), (0, 3), (1, 4), (0, 5)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        // Frame 0's rows: project indices 0, 2, 4 shown at
        // display positions 1, 2, 3 — the delete target is
        // the project index, never the row position.
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![0, 2, 4]
        );
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(rows[1].summary, "3 pts · w 4");
        assert_eq!(rows[1].color, 0xFF_0000);
        core.goto(1);
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(rows[0].summary, "2 pts · w 4");
    }

    #[test]
    fn delete_stroke_removes_the_row_s_project_stroke() {
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2), (0, 3), (1, 4), (0, 5)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        // Delete frame 0's middle row: its project index is
        // 2 (the 3-point stroke). Deleting by row position
        // instead would kill project index 1 — a frame-1
        // stroke — which must survive.
        let rows = stroke_rows(&core);
        delete_stroke(&mut core, rows[1].project_index);
        let remaining: Vec<usize> = core
            .project
            .strokes
            .iter()
            .map(|s| s.points.len())
            .collect();
        assert_eq!(remaining, vec![1, 2, 4, 5]);
        // Frame 0's rebuilt rows renumber over the
        // survivors and still point at the right strokes:
        // the 5-point stroke moved from project index 4
        // to index 3.
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![0, 3]
        );
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(rows[1].summary, "5 pts · w 4");
    }

    fn colored_stroke(frame: usize, color: u32) -> Stroke {
        let mut s = plain_stroke(frame, 2);
        s.color = color;
        s
    }

    #[test]
    fn bulk_target_color_prefers_node_colour_then_figure() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].color = 0x00_00FF;
        // No node selected: the figure's colour.
        core.selected_node = None;
        assert_eq!(bulk_target_color(&core), 0x00_00FF);
        // Node selected but carrying no colour of its
        // own: still the figure's colour.
        core.selected_node = Some(0);
        core.project.figures[fi].def.nodes[0].color = None;
        assert_eq!(bulk_target_color(&core), 0x00_00FF);
        // Node selected with its own colour: the node's.
        core.project.figures[fi].def.nodes[0].color = Some(0x12_3456);
        assert_eq!(bulk_target_color(&core), 0x12_3456);
    }

    #[test]
    fn recolor_all_strokes_recolors_all_frames_as_one_undo_step() {
        let mut core = EditorCore::new();
        core.project
            .strokes
            .push(colored_stroke(0, 0xAA_0000));
        core.project
            .strokes
            .push(colored_stroke(1, 0x00_BB00));
        core.project
            .strokes
            .push(colored_stroke(0, 0x00_00CC));
        let fi = core.selected_figure;
        core.project.figures[fi].color = 0x77_7777;
        core.selected_node = None;
        let depth = core.undo_depth();
        assert_eq!(recolor_all_strokes(&mut core), 3);
        assert!(
            core.project.strokes.iter().all(|s| s.color == 0x77_7777),
            "every stroke, on every frame, takes the target colour"
        );
        assert_eq!(core.status, "Recoloured 3 strokes.");
        // One press = one undo step, and that one undo
        // restores every original colour.
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        let colours: Vec<u32> =
            core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colours, vec![0xAA_0000, 0x00_BB00, 0x00_00CC]);
    }

    #[test]
    fn delete_all_strokes_empties_all_frames_as_one_undo_step() {
        let mut core = EditorCore::new();
        for frame in [0, 1, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let depth = core.undo_depth();
        assert_eq!(delete_all_strokes(&mut core), 3);
        assert!(core.project.strokes.is_empty());
        assert_eq!(core.status, "Deleted 3 strokes.");
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        let frames: Vec<usize> =
            core.project.strokes.iter().map(|s| s.frame).collect();
        assert_eq!(frames, vec![0, 1, 0], "one undo restores all frames' strokes");
    }

    #[test]
    fn bulk_ops_report_singular_counts() {
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 2));
        assert_eq!(recolor_all_strokes(&mut core), 1);
        assert_eq!(core.status, "Recoloured 1 stroke.");
        assert_eq!(delete_all_strokes(&mut core), 1);
        assert_eq!(core.status, "Deleted 1 stroke.");
    }

    #[test]
    fn bulk_ops_on_an_empty_list_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let depth = core.undo_depth();
        assert_eq!(recolor_all_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes to recolour.");
        assert_eq!(delete_all_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes to delete.");
        assert_eq!(
            core.undo_depth(),
            depth,
            "an empty press must not add an undo step"
        );
    }

    #[test]
    fn step_selection_wraps_and_enters_from_unset() {
        // Empty project: nothing to select, either way —
        // not even from a (necessarily stale) Some.
        assert_eq!(step_selection(None, 0, 1), None);
        assert_eq!(step_selection(None, 0, -1), None);
        assert_eq!(step_selection(Some(0), 0, 1), None);
        // Single stroke: entering lands on it, and
        // stepping wraps back onto it from both sides.
        assert_eq!(step_selection(None, 1, 1), Some(0));
        assert_eq!(step_selection(None, 1, -1), Some(0));
        assert_eq!(step_selection(Some(0), 1, 1), Some(0));
        assert_eq!(step_selection(Some(0), 1, -1), Some(0));
        // Walking five strokes, entering from unset at
        // both ends and wrapping past both ends.
        assert_eq!(step_selection(None, 5, 1), Some(0));
        assert_eq!(step_selection(None, 5, -1), Some(4));
        assert_eq!(step_selection(Some(2), 5, 1), Some(3));
        assert_eq!(step_selection(Some(2), 5, -1), Some(1));
        assert_eq!(step_selection(Some(4), 5, 1), Some(0), "next wraps");
        assert_eq!(step_selection(Some(0), 5, -1), Some(4), "prev wraps");
        // A stale selection is treated as unset: +1
        // re-enters at the first stroke, −1 at the last —
        // it never steps relative to the stale index.
        assert_eq!(step_selection(Some(5), 5, 1), Some(0));
        assert_eq!(step_selection(Some(5), 5, -1), Some(4));
        assert_eq!(step_selection(Some(99), 5, 1), Some(0));
        assert_eq!(step_selection(Some(99), 5, -1), Some(4));
        // No direction: a valid selection holds; an
        // unset or stale one stays unset.
        assert_eq!(step_selection(Some(2), 5, 0), Some(2));
        assert_eq!(step_selection(None, 5, 0), None);
        assert_eq!(step_selection(Some(9), 5, 0), None);
    }

    #[test]
    fn stroke_frame_of_maps_project_indices_to_frames() {
        let mut core = EditorCore::new();
        for frame in [0, 2, 0, 1, 2] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        // The project-wide index space interleaves frames;
        // each index maps to its own stroke's frame.
        assert_eq!(stroke_frame_of(&core, 0), Some(0));
        assert_eq!(stroke_frame_of(&core, 1), Some(2));
        assert_eq!(stroke_frame_of(&core, 2), Some(0));
        assert_eq!(stroke_frame_of(&core, 3), Some(1));
        assert_eq!(stroke_frame_of(&core, 4), Some(2));
        assert_eq!(stroke_frame_of(&core, 5), None);
        assert_eq!(stroke_total(&core), 5);
    }

    #[test]
    fn selection_readout_names_position_total_and_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        assert_eq!(selection_readout(&core, None), "No strokes");
        assert_eq!(selection_readout(&core, Some(0)), "No strokes");
        for frame in [0, 2, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 3 strokes"
        );
        // Index 1 is the stroke recorded on frame 2,
        // shown 1-based as frame 3.
        assert_eq!(
            selection_readout(&core, Some(1)),
            "Stroke 2 of 3 · frame 3"
        );
        assert_eq!(
            selection_readout(&core, Some(0)),
            "Stroke 1 of 3 · frame 1"
        );
        // A stale selection reads as no selection.
        assert_eq!(
            selection_readout(&core, Some(3)),
            "No stroke selected · 3 strokes"
        );
    }

    #[test]
    fn selection_readout_uses_the_singular_for_one_stroke() {
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 2));
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 1 stroke"
        );
        assert_eq!(
            selection_readout(&core, Some(0)),
            "Stroke 1 of 1 · frame 1"
        );
    }

    /// A core whose project carries strokes on frames
    /// 0, 2, 0 (project indices 0, 1, 2). The demo
    /// project has 5 frames, so frame 2 is a real
    /// `goto` target.
    fn core_with_strokes() -> EditorCore {
        let mut core = EditorCore::new();
        for frame in [0, 2, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        core
    }

    /// A shell `State` over the given core with the
    /// given stroke selection — field-for-field the
    /// literal the other sections' State tests use,
    /// with the Phase G fields included.
    fn state_with(core: EditorCore, sel: Option<usize>) -> State {
        State {
            core,
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: sel,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            was_playing: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
        }
    }

    #[test]
    fn selection_presses_step_wrap_and_clear_as_view_state_only() {
        let mut s = state_with(core_with_strokes(), None);
        let depth = s.core.undo_depth();
        let status = s.core.status.clone();
        // Stepping enters at the first stroke, walks the
        // project-wide space (frames are irrelevant to
        // the order) and wraps at both ends.
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(1));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(2));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0), "next wraps to the first");
        step_stroke_selection(&mut s, -1);
        assert_eq!(s.stroke_selected, Some(2), "prev wraps to the last");
        // View state only: no undo step was added and the
        // status line was never touched (no toast spam).
        assert_eq!(s.core.undo_depth(), depth);
        assert_eq!(s.core.status, status);
        // A stale selection re-enters instead of stepping
        // relative to the dead index.
        s.stroke_selected = Some(9);
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0));
        // Clear drops the selection, still silently.
        clear_stroke_selection(&mut s);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, status);
        assert_eq!(s.core.undo_depth(), depth);
    }

    #[test]
    fn goto_press_navigates_to_the_selected_strokes_frame() {
        // Index 1 lives on frame 2, index 2 on frame 0.
        let mut s = state_with(core_with_strokes(), Some(1));
        assert!(goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.current, 2);
        s.stroke_selected = Some(2);
        assert!(goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.current, 0);
        // No selection: the press refuses with a status
        // and does not navigate.
        let mut s = state_with(core_with_strokes(), None);
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No stroke selected.");
        assert_eq!(s.core.current, 0);
        // A stale selection refuses the same way.
        let mut s = state_with(core_with_strokes(), Some(7));
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No stroke selected.");
    }

    #[test]
    fn selection_presses_on_an_empty_project_only_report() {
        let core = EditorCore::new();
        assert!(core.project.strokes.is_empty());
        // Even a stale selection is dropped, never
        // stepped, and nothing panics.
        let mut s = state_with(core, Some(4));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
        step_stroke_selection(&mut s, -1);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
        clear_stroke_selection(&mut s);
        assert_eq!(s.core.status, "No strokes to select.");
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No strokes to select.");
        assert_eq!(s.stroke_selected, None);
    }

    /// A stroke on `frame` whose polyline is exactly
    /// `points` — the measure tests need known geometry,
    /// which the repeated-point `plain_stroke` cannot
    /// give.
    fn polyline_stroke(frame: usize, points: &[[f32; 2]]) -> Stroke {
        let mut s = plain_stroke(frame, 0);
        s.points = points.to_vec();
        s
    }

    #[test]
    fn stroke_measure_sums_the_base_polyline_segments() {
        let mut core = EditorCore::new();
        // Right angle: (0,0) → (3,0) → (3,4) is 3 + 4 = 7
        // stage units over 3 points, recorded on frame 2.
        core.project
            .strokes
            .push(polyline_stroke(2, &[[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]]));
        let m = stroke_measure(&core, 0).expect("index 0 exists");
        assert_eq!(m.points, 3);
        assert_eq!(m.length, 7.0);
        assert_eq!(m.frame, 2, "the frame is stroke_frame_of's raw value");
        // Out of range: no measure.
        assert_eq!(stroke_measure(&core, 1), None);
    }

    #[test]
    fn stroke_measure_handles_degenerate_polylines() {
        let mut core = EditorCore::new();
        core.project.strokes.push(polyline_stroke(0, &[]));
        core.project.strokes.push(polyline_stroke(1, &[[5.0, 5.0]]));
        assert_eq!(
            stroke_measure(&core, 0),
            Some(StrokeMeasure {
                points: 0,
                length: 0.0,
                frame: 0
            })
        );
        assert_eq!(
            stroke_measure(&core, 1),
            Some(StrokeMeasure {
                points: 1,
                length: 0.0,
                frame: 1
            })
        );
    }

    #[test]
    fn stroke_measure_uses_the_base_polyline_for_spray_strokes() {
        let mut core = EditorCore::new();
        let mut s = polyline_stroke(0, &[[0.0, 0.0], [6.0, 8.0]]);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes.push(s);
        let m = stroke_measure(&core, 0).expect("index 0 exists");
        assert_eq!(m.points, 2);
        // The 6-8-10 hypotenuse of the base polyline — the
        // scattered spray dots play no part in the measure.
        assert_eq!(m.length, 10.0);
        assert_eq!(m.frame, 0);
    }

    #[test]
    fn measure_readout_formats_the_selected_strokes_facts() {
        let mut core = EditorCore::new();
        assert_eq!(measure_readout(&core, None), None);
        assert_eq!(measure_readout(&core, Some(0)), None, "empty project");
        core.project
            .strokes
            .push(polyline_stroke(2, &[[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]]));
        // 1-based position and frame, whole-unit length.
        assert_eq!(
            measure_readout(&core, Some(0)).as_deref(),
            Some("Stroke 1: 3 pts · 7 long · frame 3")
        );
        // No selection or a stale one: no line at all.
        assert_eq!(measure_readout(&core, None), None);
        assert_eq!(measure_readout(&core, Some(1)), None);
    }

    #[test]
    fn stroke_panel_builds_with_and_without_strokes() {
        let t = rune::tok::theme(true);
        // Empty project: the selection row renders its
        // "No strokes" readout alongside everything else.
        let s = state_with(EditorCore::new(), None);
        let _ = stroke_panel(&s, &t);
        // Strokes present, one selected: the row renders
        // the position/frame readout.
        let s = state_with(core_with_strokes(), Some(1));
        let _ = stroke_panel(&s, &t);
    }
}
