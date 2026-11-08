//! Stroke / brush inspector (Rune Phase D, section 12/40).
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
//! swatch and a data summary (points, width, fill, spray
//! mark). **There is no per-stroke delete op** — a grep for
//! delete/remove stroke across the engine finds nothing;
//! the removal ops that exist are frame-scoped
//! (`clear_frame_drawings`, editor.rs:2391 — the Clear
//! button below) and pointer-scoped (the eraser drag and
//! `lasso_erase`, editor.rs:2449). So the list is
//! display-only and Clear is the one destructive control.
//!
//! No new `State` fields: everything reads `s.core` (active
//! brush, `brush_index`, `project.strokes`, `current`) and
//! writes go through the `brush_index` field write or the
//! one frame-clear op.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Stroke;
use stykanimate_rs::paint::{Brush, Nib};

use crate::chrome_view::btn;
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

/// The stroke inspector panel: the active brush's parameter
/// set (steppers + nib segmented control, all stepping the
/// preset selection — see the module docs), then the
/// current frame's stroke list with the frame Clear button.
pub fn stroke_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let brush = core.active_brush();
    let mut panel = col().key("stroke-inspector").w_full().gap(8.0);

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
    let indices = frame_stroke_indices(core);
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
    if !indices.is_empty() {
        header = header.child(btn("strokes-clear", "Clear frame", false, t, |s| {
            s.core.clear_frame_drawings()
        }));
    }
    panel = panel.child(header);

    if indices.is_empty() {
        panel = panel.child(
            text("No strokes on this frame.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        for (k, &idx) in indices.iter().enumerate() {
            let stroke = &core.project.strokes[idx];
            panel = panel.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(
                        el()
                            .size(14.0, 14.0)
                            .radius(4.0)
                            .fill(hex_color(stroke.color).unwrap_or(t.text)),
                    )
                    .child(
                        text(format!("#{}", k + 1))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                    )
                    .child(
                        text(stroke_summary(stroke))
                            .font(type_::LABEL)
                            .color(t.text)
                            .grow(1.0),
                    ),
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
}
