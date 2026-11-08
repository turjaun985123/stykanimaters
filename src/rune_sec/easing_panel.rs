//! Easing panel for the Rune editor shell (Phase D,
//! section 10 — GPUI→Rune migration). Declared from `main.rs`
//! only (as `rune_sec::easing_panel`) — bin-crate code, not
//! part of the engine library.
//!
//! Three groups, labelled by what the underlying `EditorCore`
//! op actually does (verified in `editor.rs` / `graph.rs` /
//! `model.rs` — the two easing families are different systems
//! and are never conflated here):
//!
//! * **Frame tween** — the current frame's `model::Easing` for
//!   the tween segment FROM this frame to the next
//!   (`Frame.easing`, `model.rs:565`; `None` inherits the
//!   project-wide easing). One button per value, calling the
//!   direct setter `EditorCore::set_frame_easing`
//!   (`editor.rs:2368`), plus a "Project default" button that
//!   sets `None`.
//! * **Project tween** — the project-wide default
//!   (`Project.easing`, `model.rs:740`). One button per
//!   value, calling the direct setter
//!   `EditorCore::set_project_easing` (added in Phase E),
//!   with the current easing shown active. Above the
//!   buttons, the Phase F curve picker draws the four
//!   project curves in pressable cells (see `picker_cell`
//!   for the click design). (Phase D shipped
//!   a single Cycle button here because `cycle_easing` was
//!   the only op then; the direct setter superseded it and
//!   the cycle button was removed rather than kept as a
//!   second, drifting path to the same setting.)
//! * **Graph keys** — `graph::TangentPreset`s applied to every
//!   graph channel of the SELECTED figure via
//!   `EditorCore::apply_graph_ease_preset` (`editor.rs:1422`).
//!   This is a one-shot apply (it rewrites key tangents), not
//!   a persistent setting, so its buttons are never shown as
//!   "active".
//!
//! Every curve preview is drawn by sampling the engine's own
//! easing math, never an approximation: frame/project previews
//! sample `Easing::apply` (`model.rs:605`) and graph previews
//! delegate to `graph_panel::preset_preview`
//! (`graph_panel.rs:225`), which applies the preset to the
//! canonical ramp channel and evaluates it through
//! `Channel::value_at` — the same curve the engine will play.
//!
//! Phase N adoption: the panel root is the kit card
//! (`design::panel_col`), the header is `design::panel_title`
//! + `design::divider`, and the group headings are
//! `design::section_label` — the local `section` helper is
//! retired. `picker_cell` keeps its curve content (the kit
//! `chip_btn` has no content slot — plan P03: "keep the
//! content; rebuild the shell on btn-family metrics") with
//! its shell aligned to the button family's state table and
//! press feedback. Keys, handlers and status behaviour are
//! unchanged.
//!
//! UI mission wave 4 (audit S12-09): the Graph keys group
//! renders through the kit's [`design::tangent_preset_chip`]
//! — the graph panel's drawn-preview chip, lifted into the
//! kit — instead of this panel's own preview-plus-button
//! rows, so the three co-mounted tangent-preset surfaces
//! share one construction. The `ease-graph-*` keys, labels,
//! one-shot handlers and never-active rule are unchanged;
//! [`tangent_curve`] remains for the group's data tests
//! (and states the curve the chip draws, via the same
//! `graph_panel::preset_preview`).

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, El, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::TangentPreset;
use stykanimate_rs::model::Easing;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::PRESS_SCALE;
use crate::State;

/// Width / height (points) of each preset's curve preview box.
pub const CURVE_W: f32 = 44.0;
pub const CURVE_H: f32 = 28.0;
/// Samples taken along each preview curve.
pub const CURVE_SAMPLES: usize = 24;

/// Width / height (points) of each project-picker cell's
/// curve box. Four padded cells plus gaps span ≈306pt
/// wide and ≈63pt tall — the picker's ~300×64 strip.
pub const PICK_W: f32 = 64.0;
pub const PICK_H: f32 = 38.0;

/// The four tween easings, in the engine's own cycle order
/// (`Easing::next`, `model.rs:613`).
pub const TWEEN_EASINGS: [Easing; 4] = [
    Easing::Linear,
    Easing::Smooth,
    Easing::EaseIn,
    Easing::EaseOut,
];

/// The five graph tangent presets, in `graph_panel`'s picker
/// order (`graph_panel.rs` tests' `ALL` list).
pub const TANGENT_PRESETS: [TangentPreset; 5] = [
    TangentPreset::Smooth,
    TangentPreset::EaseIn,
    TangentPreset::EaseOut,
    TangentPreset::EaseInOut,
    TangentPreset::Linear,
];

/// Button label for a tween easing (the engine's `label()` is
/// lowercase status text; buttons read better capitalised).
fn tween_label(e: Easing) -> &'static str {
    match e {
        Easing::Linear => "Linear",
        Easing::Smooth => "Smooth",
        Easing::EaseIn => "Ease in",
        Easing::EaseOut => "Ease out",
    }
}

/// Button label for a graph tangent preset (the enum has no
/// `label()` of its own).
fn tangent_label(p: TangentPreset) -> &'static str {
    match p {
        TangentPreset::Smooth => "Smooth",
        TangentPreset::EaseIn => "Ease in",
        TangentPreset::EaseOut => "Ease out",
        TangentPreset::EaseInOut => "Ease in-out",
        TangentPreset::Linear => "Linear",
    }
}

/// Sample a tween easing's curve: `samples` evenly spaced
/// values of the engine's own `Easing::apply` over t in
/// 0.0..=1.0, so the first sample is exactly `apply(0.0)` and
/// the last exactly `apply(1.0)`. `samples < 2` is treated
/// as 2.
pub fn easing_curve(easing: Easing, samples: usize) -> Vec<f32> {
    let samples = samples.max(2);
    (0..samples)
        .map(|i| easing.apply(i as f32 / (samples - 1) as f32))
        .collect()
}

/// The project picker's data: the four tween easings in
/// display order, each paired with its sampled curve — the
/// same `easing_curve` samples (from the engine's own
/// `Easing::apply`, `model.rs:605`) that the per-row preview
/// chips draw. Factored out of the builder so tests can pin
/// the strip's contents without a `State`.
pub fn picker_curves() -> [(Easing, Vec<f32>); 4] {
    TWEEN_EASINGS.map(|e| (e, easing_curve(e, CURVE_SAMPLES)))
}

/// Sample a graph tangent preset's curve via the engine's
/// canonical preview (`graph_panel::preset_preview`): the
/// preset applied to the 0→100 ramp channel, evaluated through
/// `Channel::value_at` and normalised into 0.0..=1.0.
pub fn tangent_curve(preset: TangentPreset, samples: usize) -> Vec<f32> {
    stykanimate_rs::graph_panel::preset_preview(preset, samples)
}

/// Apply a project-tween preset — the project group button
/// handler's whole body, factored out so tests can drive the
/// dispatch against a bare `EditorCore`. Goes through the
/// core's direct setter (Phase E). Unlike the frame group's
/// `set_frame_easing`, the press is NOT an undoable op:
/// `set_project_easing` (`editor.rs:1974`) takes no snapshot,
/// by the core's own ruling that project-settings tweaks are
/// not undoable edits (the same rule `cycle_easing` follows).
/// The pinning test below keeps this doc honest.
pub fn set_project_preset(core: &mut EditorCore, e: Easing) {
    core.set_project_easing(e);
}

/// Whether `preset` is the core's current project tween —
/// the project group's active-state rule. The project easing
/// is a single value, so exactly one preset is active at
/// any time.
pub fn project_preset_active(core: &EditorCore, preset: Easing) -> bool {
    core.project.easing == preset
}

/// Map normalised curve samples into preview-box points:
/// t runs left→right, value runs bottom→top (canvas points
/// are y-down), inset by `pad` on every side so the stroke
/// is never clipped by the box edge.
pub fn curve_points(samples: &[f32], w: f32, h: f32, pad: f32) -> Vec<[f32; 2]> {
    if samples.len() < 2 {
        return Vec::new();
    }
    let inner_w = (w - 2.0 * pad).max(0.0);
    let inner_h = (h - 2.0 * pad).max(0.0);
    samples
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let t = i as f32 / (samples.len() - 1) as f32;
            [
                pad + t * inner_w,
                pad + (1.0 - v.clamp(0.0, 1.0)) * inner_h,
            ]
        })
        .collect()
}

/// Build the preview `Shape` for one curve: a single polyline
/// in a `w`×`h` view box.
pub fn curve_shape(samples: &[f32], w: f32, h: f32, color: Color) -> Shape {
    Shape::new()
        .view_box(w, h)
        .stroke(curve_points(samples, w, h, 2.0), 1.5, color)
}

/// One preset row: the curve preview chip beside its button.
fn preset_row(preview: El, button: El) -> El {
    row().gap(6.0).center_y().child(preview).child(button)
}

/// One project-picker cell: a pressable card carrying its
/// easing's curve above its label.
///
/// Click design (Phase F): a Rune canvas has no per-region
/// clicks, but none are needed — `on_press` is a generic
/// `El` method (rune `el/handlers.rs:6`) and the shell's
/// own `chrome_view::btn` is exactly
/// `el().on_press(..).child(text)`. A cell built the same
/// way, with a canvas and a label as its children, is a
/// real button: its whole area applies the preset through
/// the same `set_project_preset` dispatch as the text
/// buttons below it. (The fallback — one wide display-only
/// canvas with the text buttons doing the applying — was
/// not needed; per-cell buttons are genuinely supported
/// and give the strip real per-curve clicks.) Active
/// styling mirrors `btn`: accent fill with white content;
/// inactive cells keep the card fill with a muted curve.
///
/// Phase N: the kit's `chip_btn` absorbs the shell's
/// text-only chips, but it has no slot for this cell's
/// curve canvas — P03's ruling for this cell is to keep
/// the content and align the shell, so the shell now
/// spells its radius via the kit (`CONTROL_RADIUS`),
/// takes the accent from `design::accent()` and carries
/// the button family's press feedback (`PRESS_SCALE`).
/// Padding stays px 4 / py 5 on purpose: at the chip's
/// px 6 the four cells plus gaps would span ≈322pt and
/// overflow the panel card's 316pt inner width.
fn picker_cell(e: Easing, samples: &[f32], active: bool, t: &Theme) -> El {
    let accent = design::accent();
    let curve_color = if active { Color::WHITE } else { t.text_secondary };
    el()
        .key(match e {
            Easing::Linear => "ease-pick-linear",
            Easing::Smooth => "ease-pick-smooth",
            Easing::EaseIn => "ease-pick-ease-in",
            Easing::EaseOut => "ease-pick-ease-out",
        })
        .px(4.0)
        .py(5.0)
        .radius(design::CONTROL_RADIUS)
        .press_scale(PRESS_SCALE)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(move |s: &mut State| set_project_preset(&mut s.core, e))
        .child(
            col()
                .gap(3.0)
                .center_y()
                .child(
                    canvas(curve_shape(samples, PICK_W, PICK_H, curve_color))
                        .size(PICK_W, PICK_H),
                )
                .child(
                    text(tween_label(e))
                        .font(type_::LABEL)
                        .color(if active { Color::WHITE } else { t.text }),
                ),
        )
}

/// The project curve picker strip: the four picker cells
/// in a row, the active cell following the project easing.
fn picker_strip(core: &EditorCore, t: &Theme) -> El {
    let mut strip = row().key("ease-picker").gap(6.0);
    for (e, samples) in picker_curves() {
        strip = strip.child(picker_cell(
            e,
            &samples,
            project_preset_active(core, e),
            t,
        ));
    }
    strip
}

/// A curve preview chip: the sampled curve on a card fill.
/// (Phase N: radius aligned to the kit's control radius,
/// per P03 — chips elsewhere are r6; content unchanged.)
fn preview(samples: &[f32], color: Color, t: &Theme) -> El {
    canvas(curve_shape(samples, CURVE_W, CURVE_H, color))
        .size(CURVE_W, CURVE_H)
        .fill(t.card)
        .radius(design::CONTROL_RADIUS)
}

/// The easing panel body. Mounting (and any show/hide state)
/// is the caller's business; this builds the full panel for
/// the current core state. Element keys carry stable,
/// section-prefixed names so headless `--tree` checks can
/// find every control.
pub fn easing_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let accent = design::accent();
    let frame_easing = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.easing);
    let project_easing = core.project.easing;

    // Panel header (Phase H, kit form since Phase N): the
    // chart-line glyph — axes with a rising curve, the
    // shape an easing curve is — before the title, via
    // `design::panel_title` (13pt glyph in
    // `text_secondary`, title in SECTION), with the kit
    // hairline under it. The preset buttons below
    // deliberately stay text: each already sits beside
    // its sampled curve preview, and no glyph honestly
    // distinguishes ease-in from ease-out — the drawn
    // curve does that.
    let mut panel = design::panel_col("easing-panel", t)
        .child(
            row()
                .w_full()
                .gap(design::PANEL_GAP)
                .center_y()
                .child(design::panel_title(Some("chart-line"), "Easing", None, t)),
        )
        .child(design::divider(t));

    // ---- Frame tween: set_frame_easing on the current frame.
    panel = panel.child(design::section_label(
        &format!("Frame {} tween → next frame", core.current + 1),
        t,
    ));
    // "Project default" is `None`: its effective curve is the
    // project easing's, drawn muted to say so.
    panel = panel.child(preset_row(
        preview(
            &easing_curve(project_easing, CURVE_SAMPLES),
            t.text_secondary,
            t,
        ),
        btn("ease-frame-default", "Project default", frame_easing.is_none(), t, |s| {
            s.core.set_frame_easing(None);
        }),
    ));
    for e in TWEEN_EASINGS {
        panel = panel.child(preset_row(
            preview(&easing_curve(e, CURVE_SAMPLES), accent, t),
            btn(
                match e {
                    Easing::Linear => "ease-frame-linear",
                    Easing::Smooth => "ease-frame-smooth",
                    Easing::EaseIn => "ease-frame-ease-in",
                    Easing::EaseOut => "ease-frame-ease-out",
                },
                tween_label(e),
                frame_easing == Some(e),
                t,
                move |s| s.core.set_frame_easing(Some(e)),
            ),
        ));
    }

    // ---- Project tween: set_project_easing on the project.
    panel = panel.child(design::section_label(
        "Project tween (default for all frames)",
        t,
    ));
    panel = panel.child(picker_strip(core, t));
    for e in TWEEN_EASINGS {
        panel = panel.child(preset_row(
            preview(&easing_curve(e, CURVE_SAMPLES), accent, t),
            btn(
                match e {
                    Easing::Linear => "ease-project-linear",
                    Easing::Smooth => "ease-project-smooth",
                    Easing::EaseIn => "ease-project-ease-in",
                    Easing::EaseOut => "ease-project-ease-out",
                },
                tween_label(e),
                project_preset_active(core, e),
                t,
                move |s| set_project_preset(&mut s.core, e),
            ),
        ));
    }

    // ---- Graph keys: apply_graph_ease_preset, one-shot.
    // UI mission wave 4 (audit S12-09): the group's rows
    // converge on the kit's drawn-preview chip — the
    // graph panel's chip construction, lifted into
    // `design::tangent_preset_chip` — keeping the
    // `ease-graph-*` keys, the labels and the handlers;
    // one-shot actions, so never active. The frame and
    // project groups above keep their preview-plus-
    // button rows: those presets ARE state (the current
    // easing highlights), a different control.
    panel = panel.child(design::section_label("Graph keys — selected figure", t));
    // UI mission Wave 6 (audit S12-16): with no graph
    // channels on the selected figure the five chips
    // could only write a refusal ("No graph channels
    // for figure N."), so the group shows the kit
    // empty state instead — the same predicate
    // `apply_graph_ease_preset` itself no-ops on.
    // Wave 9 (agent D9): the empty state's "Key angle"
    // below is the graph panel's button name verbatim
    // — the graph panel owns the channels this group
    // is about and sits directly above this panel in
    // the Graph workspace column. It is deliberately
    // NOT the keyframe tools' "Key angle here" (that
    // section's playhead dialect); do not "correct"
    // the sentence to match it.
    let fig_has_channels = core
        .project
        .channels
        .iter()
        .any(|ch| ch.figure == core.selected_figure);
    if fig_has_channels {
        for p in TANGENT_PRESETS {
            panel = panel.child(design::tangent_preset_chip(
                match p {
                    TangentPreset::Smooth => "ease-graph-smooth",
                    TangentPreset::EaseIn => "ease-graph-ease-in",
                    TangentPreset::EaseOut => "ease-graph-ease-out",
                    TangentPreset::EaseInOut => "ease-graph-ease-in-out",
                    TangentPreset::Linear => "ease-graph-linear",
                },
                p,
                tangent_label(p),
                false,
                t,
                move |s| s.core.apply_graph_ease_preset(p),
            ));
        }
    } else {
        panel = panel.child(design::empty_state(
            "chart-line",
            "No graph channels on this figure yet — Key angle starts one.",
            t,
        ));
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_endpoints(curve: &[f32], what: &str) {
        assert!(curve.len() >= 2, "{what}: too few samples");
        assert!(
            curve.first().copied().unwrap_or(f32::NAN).abs() < 1e-3,
            "{what}: starts at {:?}, not 0",
            curve.first()
        );
        assert!(
            (curve.last().copied().unwrap_or(f32::NAN) - 1.0).abs() < 1e-3,
            "{what}: ends at {:?}, not 1",
            curve.last()
        );
    }

    #[test]
    fn tween_curves_run_zero_to_one_for_every_preset() {
        for e in TWEEN_EASINGS {
            assert_endpoints(&easing_curve(e, CURVE_SAMPLES), e.label());
        }
    }

    #[test]
    fn tangent_curves_run_zero_to_one_for_every_preset() {
        for p in TANGENT_PRESETS {
            assert_endpoints(&tangent_curve(p, CURVE_SAMPLES), tangent_label(p));
        }
    }

    #[test]
    fn linear_tween_curve_is_the_identity() {
        for (i, v) in easing_curve(Easing::Linear, 11).iter().enumerate() {
            assert!((v - i as f32 / 10.0).abs() < 1e-6, "sample {i}: {v}");
        }
    }

    #[test]
    fn ease_in_lags_and_ease_out_leads_at_the_midpoint() {
        // The engine's own semantics (t² vs 1−(1−t)² at t=0.5).
        let mid = CURVE_SAMPLES / 2;
        assert!(easing_curve(Easing::EaseIn, CURVE_SAMPLES)[mid] < 0.5);
        assert!(easing_curve(Easing::EaseOut, CURVE_SAMPLES)[mid] > 0.5);
    }

    #[test]
    fn tangent_ease_in_lags_linear_on_the_engine_ramp() {
        // On graph_panel's canonical ramp, EaseIn samples
        // ≈12.3% at the quarter point (graph.rs docs) — far
        // below the linear diagonal's 25%.
        let curve = tangent_curve(TangentPreset::EaseIn, 41);
        assert!(
            curve[10] < 0.2,
            "ease-in quarter point should lag: {}",
            curve[10]
        );
        let linear = tangent_curve(TangentPreset::Linear, 41);
        assert!((linear[10] - 0.25).abs() < 1e-3);
    }

    #[test]
    fn curve_points_map_start_bottom_left_and_end_top_right() {
        let pts = curve_points(&[0.0, 0.5, 1.0], 44.0, 28.0, 2.0);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], [2.0, 26.0]);
        assert_eq!(pts[2], [42.0, 2.0]);
        // Midpoint sits at the box centre.
        assert_eq!(pts[1], [22.0, 14.0]);
    }

    #[test]
    fn curve_points_clamp_out_of_band_values() {
        let pts = curve_points(&[-1.0, 2.0], 44.0, 28.0, 2.0);
        assert_eq!(pts[0][1], 26.0);
        assert_eq!(pts[1][1], 2.0);
        assert!(curve_points(&[0.5], 44.0, 28.0, 2.0).is_empty());
    }

    #[test]
    fn project_preset_dispatch_sets_exactly_that_easing() {
        // The project group's buttons dispatch through
        // `set_project_preset` → the core's direct setter:
        // after each dispatch the project easing is exactly
        // the pressed preset (no cycling, no drift).
        let mut core = EditorCore::new();
        for e in TWEEN_EASINGS {
            set_project_preset(&mut core, e);
            assert_eq!(core.project.easing, e, "dispatching {}", e.label());
        }
    }

    #[test]
    fn project_preset_dispatch_takes_no_undo_snapshot() {
        // The truth `set_project_preset`'s doc states:
        // the press is NOT an undoable op. The core's
        // project setter takes no snapshot (project-
        // settings tweaks are not undoable edits there),
        // unlike the frame group's snapshotting
        // `set_frame_easing`. Pin the undo depth across a
        // dispatch, mirroring the engine's own test for
        // the setter — if the core ever starts
        // snapshotting here, this panel's doc must change
        // with it, and this test is what says so.
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        set_project_preset(&mut core, Easing::EaseIn);
        assert_eq!(core.project.easing, Easing::EaseIn);
        assert_eq!(core.undo_depth(), depth, "no snapshot is taken");
    }

    #[test]
    fn project_preset_active_tracks_the_current_easing() {
        // Exactly one project preset is active at any time,
        // and it follows each dispatch.
        let mut core = EditorCore::new();
        for e in TWEEN_EASINGS {
            set_project_preset(&mut core, e);
            let active: Vec<Easing> = TWEEN_EASINGS
                .into_iter()
                .filter(|&p| project_preset_active(&core, p))
                .collect();
            assert_eq!(active, vec![e], "after dispatching {}", e.label());
        }
    }

    #[test]
    fn picker_curves_come_in_tween_display_order() {
        for (i, (e, _)) in picker_curves().into_iter().enumerate() {
            assert_eq!(e, TWEEN_EASINGS[i]);
        }
    }

    #[test]
    fn picker_curves_hit_the_endpoints_exactly() {
        // `Easing::apply` (model.rs:605) is exact at the ends
        // for all four formulas, so the strip's curves start
        // at exactly 0.0 and end at exactly 1.0 — assert_eq,
        // not a tolerance.
        for (e, curve) in picker_curves() {
            assert_eq!(curve.first().copied(), Some(0.0), "{} start", e.label());
            assert_eq!(curve.last().copied(), Some(1.0), "{} end", e.label());
        }
    }

    #[test]
    fn picker_ease_in_sits_below_and_ease_out_above_the_diagonal() {
        // The diagonal is the Linear curve itself (index 0
        // in display order): at every interior sample the
        // engine's t² sits strictly below it and 1−(1−t)²
        // strictly above it.
        let curves = picker_curves();
        let linear = &curves[0].1;
        let ease_in = &curves[2].1;
        let ease_out = &curves[3].1;
        assert_eq!(curves[0].0, Easing::Linear);
        assert_eq!(curves[2].0, Easing::EaseIn);
        assert_eq!(curves[3].0, Easing::EaseOut);
        for i in 1..linear.len() - 1 {
            assert!(ease_in[i] < linear[i], "ease-in sample {i} not below diagonal");
            assert!(ease_out[i] > linear[i], "ease-out sample {i} not above diagonal");
        }
    }

    #[test]
    fn picker_cells_and_strip_build_for_every_preset() {
        // Builder smoke: every cell constructs in both the
        // active and inactive styling, and the strip builds
        // against a bare core (no State needed — the cells'
        // press closures are only exercised by the runtime).
        let t = tok::theme(true);
        for (e, samples) in picker_curves() {
            let _ = picker_cell(e, &samples, true, &t);
            let _ = picker_cell(e, &samples, false, &t);
        }
        let core = EditorCore::new();
        let _ = picker_strip(&core, &t);
    }
}
