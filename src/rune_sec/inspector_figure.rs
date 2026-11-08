//! Figure inspector panel (Rune migration Phase D, section
//! 4/40). Declared from `main.rs` only (as
//! `mod rune_sec::inspector_figure` or the section
//! aggregator's re-export) — bin-crate code, not part of the
//! engine library.
//!
//! Shows the selected figure (`EditorCore::selected_figure`):
//! its name as a heading, a 16-colour swatch grid, an opacity
//! stepper, and the Flip / Duplicate / Delete actions. Every
//! control dispatches to the engine exactly the way the
//! engine's own surfaces do:
//!
//! * Colour swatches → there is **no `EditorCore` op** that
//!   sets a figure's colour (verified against the full
//!   `pub fn` list of editor.rs). The established precedent
//!   is a direct field write plus a status line — the core's
//!   own colour ops do exactly that for nodes
//!   (`set_node_color`, editor.rs:521: writes the field, sets
//!   the status, and deliberately does not snapshot), and so
//!   does the MCP layer for figures (mcp_tools.rs:321,
//!   `core.project.figures[idx].color = color`). The
//!   [`set_figure_color`] dispatch below follows that
//!   precedent: bounds-checked field write + status, no
//!   snapshot. Keys `fig-color-{rrggbb}`.
//! * Opacity −/+ → `step_figure_opacity(fi, ∓0.1)`
//!   (editor.rs:1177) — the layers engine's clamped setter,
//!   the same op the figures sidebar's steppers call. Keys
//!   `insp-fop-dn` / `insp-fop-up`.
//! * Flip horizontal → `mirror_selected_pose()`
//!   (editor.rs:1405) — key `fig-flip`. Honest scope, from
//!   the op's own docs: it mirrors the selected figure's
//!   pose **at the current frame** by negating the angle
//!   data through the root axis; per-node bends, segment
//!   lengths and graph channels keep their values, so a
//!   graph-driven or bent pose can differ slightly from a
//!   true geometric mirror. It is the only horizontal-flip
//!   the core offers for a figure (`mirror_strokes_x`,
//!   editor.rs:2482, flips frame drawings, not the figure).
//! * Duplicate → there is **no duplicate-figure op**
//!   anywhere in the engine (verified: no
//!   `duplicate_figure`/`clone_figure` in editor.rs,
//!   layers.rs or the model). The [`duplicate_figure`]
//!   dispatch below performs the structural clone in this
//!   file, mirroring `delete_figure`'s bookkeeping
//!   (editor.rs:1603): it snapshots first (as every
//!   structural figure op does), inserts the cloned
//!   instance right after the original, inserts a clone of
//!   the figure's pose (and bend row, where one is stored)
//!   into every frame at the same index so the parallel
//!   per-frame tables stay parallel, shifts stored figure
//!   indices above the insertion point (particle anchors,
//!   the session's solo target), and selects the copy. A
//!   movieclip figure's copy shares the same clip, like a
//!   second instance of it. Key `fig-duplicate`.
//! * Delete figure → `delete_figure()` (editor.rs:1603) —
//!   key `fig-del` (the GPUI sidebar's tuple-id root),
//!   danger-styled; the op itself refuses to delete the
//!   last figure with a status message.
//!
//! When the project has no figures — or the selection is
//! stale — the panel is one muted line. This section needs
//! **no new `State` fields**: it reads only `State.core`,
//! like the segment inspector.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod inspector_figure;
//! }
//! // in `view()`, with the other inspector sections:
//! .child(rune_sec::inspector_figure::inspector_figure(s, &t))
//! ```

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Opacity step for the −/+ stepper: 10%, the step the
/// figures sidebar's steppers pass to
/// `step_figure_opacity` (its doc: "the sidebar's −/+
/// steppers").
pub const OPACITY_STEP: f32 = 0.1;

/// The 16-colour figure palette, in grid order (two rows of
/// eight). The first six are the core's own figure-colour
/// cycle (`FIGURE_COLORS`, editor.rs:102 — the colours
/// `add_figure` and friends assign), followed by the ink of
/// the core's node-colour cycle (editor.rs:484) and nine
/// further hues at the same saturation family, so a figure
/// can be recoloured to anything `add_figure` would have
/// given it and well beyond.
pub const PALETTE: [u32; 16] = [
    0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830, 0xF16F78, 0xFF9F43,
    0xB7E26B, 0x2ED573, 0x67C9B3, 0x2686E8, 0x8C7BFF, 0xFF6BCE, 0x8A5A2B,
];

/// Derived view-state for the panel: everything the builder
/// renders, computed once from the core so the builder stays
/// a pure function of it (and so the readout math is unit
/// testable without elements).
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorFigureState {
    /// Selected figure index (the `fi` the per-figure ops
    /// take).
    pub figure: usize,
    /// Figure name from the instance.
    pub name: String,
    /// Figure colour as `0xRRGGBB`.
    pub color: u32,
    /// Opacity as a whole percentage (0–100).
    pub opacity_pct: i32,
    /// Segment (node) count of the figure's definition.
    pub segments: usize,
    /// The figure is hidden (layers flag).
    pub hidden: bool,
    /// The figure is locked (layers flag).
    pub locked: bool,
}

/// Whole-percentage opacity for display (`1.0 → 100`),
/// clamped like the layers engine clamps the value itself.
pub fn opacity_pct(opacity: f32) -> i32 {
    (opacity.clamp(0.0, 1.0) * 100.0).round() as i32
}

/// Compute the panel's view-state from the core, or `None`
/// when the project has no figures or the selection is
/// stale. All indexing is defensive: a stale selection left
/// over from a figure edit reads as "nothing to inspect"
/// instead of panicking.
pub fn figure_state(core: &EditorCore) -> Option<InspectorFigureState> {
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    Some(InspectorFigureState {
        figure: fi,
        name: fig.name.clone(),
        color: fig.color,
        opacity_pct: opacity_pct(fig.opacity),
        segments: fig.def.nodes.len(),
        hidden: fig.hidden,
        locked: fig.locked,
    })
}

/// Set the selected figure's colour. No `EditorCore` op
/// exists for this (see the module docs); this follows the
/// engine's own colour-write precedent — bounds-checked
/// direct field write plus a status line, no snapshot, the
/// same shape as `set_node_color` (editor.rs:521) and the
/// MCP layer's figure-colour write (mcp_tools.rs:321).
pub fn set_figure_color(core: &mut EditorCore, rgb: u32) {
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get_mut(fi) else {
        core.status = "No such figure.".into();
        return;
    };
    fig.color = rgb;
    core.status = format!("Figure {} colour changed.", fi + 1);
}

/// Duplicate the selected figure: insert a clone of the
/// instance right after the original, with the original's
/// pose (and bend row, where stored) cloned into every
/// frame at the same index. No engine op exists for this
/// (see the module docs); the bookkeeping mirrors
/// `delete_figure` (editor.rs:1603), run in reverse, plus
/// the index shifts an insertion requires. The copy is
/// named `"<name> copy"` and becomes the selection.
pub fn duplicate_figure(core: &mut EditorCore) {
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get(fi).cloned() else {
        core.status = "No figure to duplicate.".into();
        return;
    };
    // Clone the per-frame rows before mutating anything.
    let frame_poses: Vec<Option<Pose>> = core
        .project
        .frames
        .iter()
        .map(|f| f.poses.get(fi).cloned())
        .collect();
    let frame_bends: Vec<Option<Vec<f32>>> = core
        .project
        .frames
        .iter()
        .map(|f| f.bends.get(fi).cloned())
        .collect();
    core.snapshot();
    let mut copy = fig;
    copy.name = format!("{} copy", copy.name);
    core.project.figures.insert(fi + 1, copy);
    for (frame, pose) in core.project.frames.iter_mut().zip(frame_poses) {
        if let Some(p) = pose {
            frame.poses.insert(fi + 1, p);
        }
    }
    for (frame, bends) in core.project.frames.iter_mut().zip(frame_bends) {
        if let Some(b) = bends {
            frame.bends.insert(fi + 1, b);
        }
    }
    // Stored figure indices above the insertion point shift
    // by one, so anchors keep pointing at the same figure:
    // particle anchors in the project, and the session's
    // solo target. (`delete_figure` performs the mirror
    // bookkeeping for its removal.)
    for e in &mut core.project.particles {
        if let Some(f) = e.figure {
            if f > fi {
                e.figure = Some(f + 1);
            }
        }
    }
    if let Some(solo) = core.solo {
        if solo > fi {
            core.solo = Some(solo + 1);
        }
    }
    core.selected_figure = fi + 1;
    core.selected_node = None;
    core.status = "Figure duplicated.".into();
}

/// One read-only label/value row.
fn readout_row(label: &str, value: String, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(text(value).font(type_::LABEL).color(t.text))
}

/// The opacity stepper row: label, current percentage, −
/// and + buttons wired to `step_figure_opacity` (chrome
/// `btn` styling, inspector key style).
fn opacity_stepper_row(st: &InspectorFigureState, t: &Theme) -> El {
    let fi = st.figure;
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text("Opacity")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(
            text(format!("{}%", st.opacity_pct))
                .font(type_::LABEL)
                .color(t.text),
        )
        .child(btn("insp-fop-dn", "−", false, t, move |s: &mut State| {
            s.core.step_figure_opacity(fi, -OPACITY_STEP)
        }))
        .child(btn("insp-fop-up", "+", false, t, move |s: &mut State| {
            s.core.step_figure_opacity(fi, OPACITY_STEP)
        }))
}

/// The Delete figure button: chrome-button shape in the
/// error colour (the 2.0 palette's `C_ERROR`).
fn danger_btn(key: &str, label: &str, f: impl Fn(&mut State) + 'static) -> El {
    let red = hex_color(0xF16F78).unwrap_or(Color::WHITE);
    let red_dark = hex_color(0xD85560).unwrap_or(red);
    el()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(red)
        .hover(red_dark)
        .press(red_dark)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(Color::WHITE))
}

/// One colour swatch: a rounded square in the palette
/// colour; the figure's current colour gets a white
/// border. Pressing writes it via [`set_figure_color`].
fn swatch(rgb: u32, selected: bool) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    let mut e = el()
        .key(format!("fig-color-{rgb:06x}"))
        .size(22.0, 22.0)
        .radius(6.0)
        .fill(fill)
        .hover(fill)
        .press(fill)
        .on_press(move |s: &mut State| set_figure_color(&mut s.core, rgb));
    if selected {
        e = e.border(2.0, Color::WHITE);
    }
    e
}

/// The Figure inspector panel. Mount it wherever the shell
/// shows selection details; it reads only `State.core`.
pub fn inspector_figure(s: &State, t: &Theme) -> El {
    let panel = col()
        .key("inspector-figure")
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .radius(8.0)
        .fill(t.card);
    let Some(st) = figure_state(&s.core) else {
        return panel.child(
            text("No figures in the project.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    let mut panel = panel
        .child(
            text(format!("Figure {} · {}", st.figure + 1, st.name))
                .font(type_::LABEL)
                .color(t.text),
        )
        .child(readout_row("Colour", format!("#{:06X}", st.color), t))
        .child(readout_row("Segments", format!("{}", st.segments), t));
    if st.hidden || st.locked {
        let mut flags: Vec<&str> = Vec::new();
        if st.hidden {
            flags.push("hidden");
        }
        if st.locked {
            flags.push("locked");
        }
        panel = panel.child(readout_row("State", flags.join(" · "), t));
    }
    panel = panel.child(
        text("Colour")
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    for chunk in PALETTE.chunks(8) {
        let mut swatches = row().gap(6.0).center_y();
        for &rgb in chunk {
            swatches = swatches.child(swatch(rgb, st.color == rgb));
        }
        panel = panel.child(swatches);
    }
    panel = panel.child(opacity_stepper_row(&st, t));
    panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn("fig-flip", "Flip horizontal", false, t, |s| {
                s.core.mirror_selected_pose()
            }))
            .child(btn("fig-duplicate", "Duplicate", false, t, |s| {
                duplicate_figure(&mut s.core)
            }))
            .child(danger_btn("fig-del", "Delete figure", |s| {
                s.core.delete_figure()
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_state_is_none_without_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert!(figure_state(&core).is_none());
    }

    #[test]
    fn figure_state_guards_a_stale_selection() {
        let mut core = EditorCore::new();
        core.selected_figure = 999;
        assert!(figure_state(&core).is_none());
    }

    #[test]
    fn figure_state_reads_the_selected_figure() {
        let core = EditorCore::new();
        let st = figure_state(&core).expect("the demo project seeds a figure");
        let fig = &core.project.figures[core.selected_figure];
        assert_eq!(st.figure, core.selected_figure);
        assert_eq!(st.name, fig.name);
        assert_eq!(st.color, fig.color);
        assert_eq!(st.segments, fig.def.nodes.len());
        assert_eq!(st.opacity_pct, opacity_pct(fig.opacity));
        assert_eq!(st.hidden, fig.hidden);
        assert_eq!(st.locked, fig.locked);
    }

    #[test]
    fn figure_state_follows_selection() {
        let mut core = EditorCore::new();
        if core.project.figures.len() > 1 {
            core.select_figure(1);
            let st = figure_state(&core).expect("figure 1 exists");
            assert_eq!(st.figure, 1);
            assert_eq!(st.name, core.project.figures[1].name);
        }
    }

    #[test]
    fn opacity_pct_rounds_and_clamps() {
        assert_eq!(opacity_pct(1.0), 100);
        assert_eq!(opacity_pct(0.5), 50);
        assert_eq!(opacity_pct(0.333), 33);
        assert_eq!(opacity_pct(0.0), 0);
        assert_eq!(opacity_pct(1.7), 100);
        assert_eq!(opacity_pct(-0.2), 0);
    }

    #[test]
    fn palette_has_sixteen_unique_colours_starting_with_the_core_cycle() {
        assert_eq!(PALETTE.len(), 16);
        let mut sorted = PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), PALETTE.len(), "palette colours repeat");
        // The core's FIGURE_COLORS cycle (editor.rs:102), in
        // order — recolours the add-figure ops hand out stay
        // reachable from the grid.
        assert!(PALETTE.starts_with(&[
            0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF
        ]));
    }

    #[test]
    fn set_figure_color_writes_the_selected_figure() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        set_figure_color(&mut core, 0x2686E8);
        assert_eq!(core.project.figures[fi].color, 0x2686E8);
        assert!(core.status.contains("colour changed"));
    }

    #[test]
    fn set_figure_color_guards_a_stale_selection() {
        let mut core = EditorCore::new();
        core.selected_figure = 999;
        set_figure_color(&mut core, 0x2686E8);
        assert_eq!(core.status, "No such figure.");
    }

    #[test]
    fn duplicate_figure_inserts_a_selected_copy_with_its_poses() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        let fi = core.selected_figure;
        let orig = core.project.figures[fi].clone();
        duplicate_figure(&mut core);
        assert_eq!(core.project.figures.len(), n + 1);
        let copy = &core.project.figures[fi + 1];
        assert_eq!(copy.name, format!("{} copy", orig.name));
        assert_eq!(copy.color, orig.color);
        assert_eq!(copy.def.nodes.len(), orig.def.nodes.len());
        assert_eq!(copy.opacity, orig.opacity);
        // The original is untouched and every frame's pose
        // table stays parallel to the figure list.
        assert_eq!(core.project.figures[fi].name, orig.name);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), core.project.figures.len());
        }
        if let Some(frame) = core.project.frames.first() {
            assert_eq!(
                frame.poses[fi + 1].angles,
                frame.poses[fi].angles,
                "the copy starts from the original's pose"
            );
        }
        assert_eq!(core.selected_figure, fi + 1);
        assert!(core.selected_node.is_none());
        assert_eq!(core.status, "Figure duplicated.");
    }

    #[test]
    fn duplicate_figure_without_figures_is_a_no_op() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        duplicate_figure(&mut core);
        assert!(core.project.figures.is_empty());
        assert_eq!(core.status, "No figure to duplicate.");
    }

    #[test]
    fn panel_builds_with_and_without_figures() {
        let t = rune::tok::theme(true);
        let with_figures = State {
            core: EditorCore::new(),
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
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
        };
        let _ = inspector_figure(&with_figures, &t);
        let mut bare_core = EditorCore::new();
        bare_core.project.figures.clear();
        let bare = State {
            core: bare_core,
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
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
        };
        let _ = inspector_figure(&bare, &t);
    }
}
