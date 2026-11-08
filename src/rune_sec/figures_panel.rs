//! Figures panel (Phase D, section 1/40 of the Rune rebuild;
//! completed in Phase E, section 29/40): the figures sidebar —
//! one row per figure with a colour dot, name, eye/lock/solo
//! toggles and opacity steppers on the first line, and reorder
//! (up/down), opacity-cycle and delete buttons on a second
//! line, a header with the figure count and "+ Figure", and a
//! footer bulk visibility row (All / None / Invert).
//!
//! Direction convention for reorder (shared with the layers
//! panel, `rune_sec::layers_panel::move_layer`): "up" means
//! toward the front of the draw stack — a higher `figures`
//! index, drawn later/on top — and dispatches
//! `move_figure_full(+1)`; "down" is toward the back (index 0)
//! and dispatches `move_figure_full(-1)`. The figures list
//! itself is shown in project order, so an "up" press moves
//! the row one slot later in this list while moving the figure
//! one slot toward the front of the stack.
//!
//! Every control calls the same real op the GPUI sidebar's
//! listener called (ids and listener bodies were recovered from
//! `git show a5e0b18:src/app.rs`; the per-figure GPUI tuple ids
//! `("fig-sel", fi)` become Rune keys `fig-sel-{fi}`, matching
//! the `frame-{n}` key style `chrome_view` already uses).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Required shell state: `State` must gain
//! `pub figures_panel: FiguresPanelState` (see the struct
//! below); the panel builder reads it and the header toggle
//! writes it. Nothing else in `State` is touched — all figure
//! state lives in `EditorCore`, as before.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::stage_view::hex_color;
use crate::State;

/// View-local state for the figures panel. The engine tracks
/// everything about the figures themselves; the only thing the
/// shell owns is whether the panel body is collapsed.
#[derive(Clone, Debug, Default)]
pub struct FiguresPanelState {
    /// When true, only the header renders (rows and the bulk
    /// footer are omitted). Toggled by the header's Hide/Show
    /// control — a pure view-state change, no engine op.
    pub collapsed: bool,
}

/// Display-ready data for one figure row, derived from the
/// core in one pass so the builder never re-reads the project
/// per element.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureRow {
    /// Figure index (the `fi` every per-figure op takes).
    pub index: usize,
    pub name: String,
    /// Figure colour as `0xRRGGBB` (the row's dot).
    pub color: u32,
    pub hidden: bool,
    pub locked: bool,
    /// This figure is the solo target (`core.solo == Some(fi)`).
    pub solo: bool,
    /// This figure is `core.selected_figure`.
    pub selected: bool,
    /// Opacity as a whole percentage (0–100).
    pub opacity_pct: i32,
}

/// Whole-percentage opacity for display (`1.0 → 100`).
pub fn opacity_pct(opacity: f32) -> i32 {
    (opacity.clamp(0.0, 1.0) * 100.0).round() as i32
}

/// One [`FigureRow`] per figure in the project, in project
/// order (the sidebar's top-to-bottom order).
pub fn figure_rows(core: &EditorCore) -> Vec<FigureRow> {
    core.project
        .figures
        .iter()
        .enumerate()
        .map(|(fi, fig)| FigureRow {
            index: fi,
            name: fig.name.clone(),
            color: fig.color,
            hidden: fig.hidden,
            locked: fig.locked,
            solo: core.solo == Some(fi),
            selected: fi == core.selected_figure,
            opacity_pct: opacity_pct(fig.opacity),
        })
        .collect()
}

/// A compact row toggle in the chrome button style (same
/// fill/hover/press treatment as `chrome_view::btn`, tighter
/// padding so five of them fit a sidebar row). `active`
/// renders accent-filled.
fn row_toggle(
    key: String,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    el()
        .key(key)
        .px(6.0)
        .py(3.0)
        .radius(5.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if active { Color::WHITE } else { t.text }),
        )
}

/// Reorder dispatch shared by the up/down buttons: select
/// figure `fi`, then move it one slot with the full layers
/// engine (`EditorCore::move_figure_full` acts on the
/// selection and routes through `layers::move_figure`, so
/// poses and every figure reference travel with the figure —
/// the layers panel's `move_layer` precedent). `dir` is +1
/// toward the front (drawn on top), -1 toward the back; at
/// the ends of the list the op is a safe no-op.
fn move_row(core: &mut EditorCore, fi: usize, dir: i32) {
    core.select_figure(fi);
    core.move_figure_full(dir);
}

/// Opacity-cycle dispatch: select figure `fi`, then cycle its
/// opacity (1.0 → 0.6 → 0.3 → 1.0) via
/// `EditorCore::cycle_figure_opacity`, which takes the index
/// directly — the select keeps the panel's "row actions act
/// on the row's figure and select it" behaviour uniform.
fn cycle_row_opacity(core: &mut EditorCore, fi: usize) {
    core.select_figure(fi);
    core.cycle_figure_opacity(fi);
}

/// Delete dispatch: select figure `fi`, then
/// `EditorCore::delete_figure` (which acts on the selection,
/// snapshots for undo, and itself refuses to remove the last
/// figure — "Keep at least one figure.").
fn delete_row(core: &mut EditorCore, fi: usize) {
    core.select_figure(fi);
    core.delete_figure();
}

/// One figure row, as a two-line card: line 1 is the colour
/// dot + name (press selects), the opacity readout, then the
/// lock / solo / opacity-stepper / eye toggles — the GPUI
/// row's trailing cluster; line 2 (Phase E) carries the
/// remaining GPUI row controls, right-aligned under the
/// cluster: reorder up/down (`fig-up-{fi}` / `fig-dn-{fi}`),
/// opacity cycle (`fig-opac-{fi}`) and delete (`fig-del-{fi}`).
/// Two lines keep every control at the same compact toggle
/// size instead of squeezing nine onto one line.
fn figure_row(r: &FigureRow, t: &Theme) -> El {
    let fi = r.index;
    let accent = tok::accent(0, true);
    let dot = hex_color(r.color).unwrap_or(Color::WHITE);
    col()
        .key(format!("fig-row-{fi}"))
        .w_full()
        .gap(2.0)
        .px(6.0)
        .py(4.0)
        .radius(6.0)
        .fill(if r.selected {
            accent.alpha(0.22)
        } else {
            Color::TRANSPARENT
        })
        .hover(t.control_track)
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(
                    // Leading cluster: dot + name; pressing it selects
                    // the figure (old `fig-sel`).
                    row()
                        .key(format!("fig-sel-{fi}"))
                        .gap(6.0)
                        .center_y()
                        .grow(1.0)
                        .on_press(move |s: &mut State| s.core.select_figure(fi))
                        .child(el().size(10.0, 10.0).radius(5.0).fill(dot))
                        .child(
                            text(r.name.clone())
                                .font(type_::LABEL)
                                .color(if r.hidden { t.text_secondary } else { t.text }),
                        ),
                )
                .child(
                    text(format!("{}%", r.opacity_pct))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(row_toggle(
                    format!("fig-lock-{fi}"),
                    if r.locked { "lock" } else { "open" },
                    r.locked,
                    t,
                    move |s: &mut State| s.core.toggle_figure_lock(fi),
                ))
                .child(row_toggle(
                    format!("fig-solo-{fi}"),
                    "solo",
                    r.solo,
                    t,
                    move |s: &mut State| s.core.toggle_solo(fi),
                ))
                .child(row_toggle(
                    format!("fig-op-dn-{fi}"),
                    "−",
                    false,
                    t,
                    move |s: &mut State| s.core.step_figure_opacity(fi, -0.1),
                ))
                .child(row_toggle(
                    format!("fig-op-up-{fi}"),
                    "+",
                    false,
                    t,
                    move |s: &mut State| s.core.step_figure_opacity(fi, 0.1),
                ))
                .child(row_toggle(
                    format!("fig-eye-{fi}"),
                    if r.hidden { "off" } else { "eye" },
                    r.hidden,
                    t,
                    // The old listener selected the figure first:
                    // `toggle_figure_hidden` acts on the selection.
                    move |s: &mut State| {
                        s.core.select_figure(fi);
                        s.core.toggle_figure_hidden();
                    },
                )),
        )
        .child(
            // Line 2: reorder / opacity cycle / delete,
            // right-aligned under line 1's toggle cluster.
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(el().grow(1.0))
                .child(row_toggle(
                    format!("fig-up-{fi}"),
                    "up",
                    false,
                    t,
                    move |s: &mut State| move_row(&mut s.core, fi, 1),
                ))
                .child(row_toggle(
                    format!("fig-dn-{fi}"),
                    "down",
                    false,
                    t,
                    move |s: &mut State| move_row(&mut s.core, fi, -1),
                ))
                .child(row_toggle(
                    format!("fig-opac-{fi}"),
                    "opac",
                    false,
                    t,
                    move |s: &mut State| cycle_row_opacity(&mut s.core, fi),
                ))
                .child(row_toggle(
                    format!("fig-del-{fi}"),
                    "del",
                    false,
                    t,
                    move |s: &mut State| delete_row(&mut s.core, fi),
                )),
        )
}

/// The figures panel: header (title, count, collapse toggle,
/// "+ Figure"), one row per figure, and the bulk visibility
/// footer (All / None / Invert). When
/// `s.figures_panel.collapsed` is set, only the header renders.
pub fn figures_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let collapsed = s.figures_panel.collapsed;
    let mut panel = col()
        .key("figures-panel")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .key("figures-panel-header")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text("Figures").font(type_::LABEL).color(t.text))
                .child(
                    text(format!("{}", core.project.figures.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(el().grow(1.0))
                .child(crate::chrome_view::btn(
                    "fig-collapse",
                    if collapsed { "Show" } else { "Hide" },
                    false,
                    t,
                    |s| s.figures_panel.collapsed = !s.figures_panel.collapsed,
                ))
                .child(crate::chrome_view::btn("add-figure", "+ Figure", false, t, |s| {
                    s.core.add_figure()
                })),
        );
    if collapsed {
        return panel;
    }
    for r in figure_rows(core) {
        panel = panel.child(figure_row(&r, t));
    }
    // Bulk visibility footer: the layers ops own the `hidden`
    // flags; each handler also clears solo so the bulk result
    // is the effective visibility, and reports the change
    // count in the status line — the old listeners verbatim.
    panel.child(
        row()
            .key("figures-panel-footer")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(crate::chrome_view::btn("fig-vis-all", "All", false, t, |s| {
                let changed = stykanimate_rs::layers::set_all_visible(&mut s.core.project, true);
                s.core.solo = None;
                s.core.status = format!("All figures shown ({changed} changed).");
            }))
            .child(crate::chrome_view::btn("fig-vis-none", "None", false, t, |s| {
                let changed = stykanimate_rs::layers::set_all_visible(&mut s.core.project, false);
                s.core.solo = None;
                s.core.status = format!("All figures hidden ({changed} changed).");
            }))
            .child(crate::chrome_view::btn("fig-vis-inv", "Invert", false, t, |s| {
                let flipped = stykanimate_rs::layers::invert_visibility(&mut s.core.project);
                s.core.solo = None;
                s.core.status = format!("Figure visibility inverted ({flipped} flipped).");
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_state_defaults_to_expanded() {
        assert!(!FiguresPanelState::default().collapsed);
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
    fn figure_rows_mirror_the_project() {
        let core = EditorCore::new();
        let rows = figure_rows(&core);
        assert_eq!(rows.len(), core.project.figures.len());
        assert!(!rows.is_empty(), "the demo project seeds figures");
        for (r, fig) in rows.iter().zip(core.project.figures.iter()) {
            assert_eq!(r.name, fig.name);
            assert_eq!(r.color, fig.color);
            assert_eq!(r.hidden, fig.hidden);
            assert_eq!(r.locked, fig.locked);
            assert_eq!(r.selected, r.index == core.selected_figure);
            assert_eq!(r.solo, core.solo == Some(r.index));
        }
    }

    #[test]
    fn figure_rows_reflect_hidden_solo_and_opacity() {
        let mut core = EditorCore::new();
        core.project.figures[0].hidden = true;
        core.project.figures[0].opacity = 0.5;
        core.toggle_solo(0);
        let rows = figure_rows(&core);
        assert!(rows[0].hidden);
        assert!(rows[0].solo);
        assert_eq!(rows[0].opacity_pct, 50);
    }

    #[test]
    fn figure_rows_follow_selection() {
        let mut core = EditorCore::new();
        if core.project.figures.len() > 1 {
            core.select_figure(1);
            let rows = figure_rows(&core);
            assert!(!rows[0].selected);
            assert!(rows[1].selected);
        }
    }

    #[test]
    fn move_row_reorders_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // "Up" (+1, toward the front): the back figure swaps
        // one slot later and the selection follows it.
        move_row(&mut core, 0, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        assert_eq!(core.selected_figure, 1);
        // Moving up again at the front is a no-op.
        move_row(&mut core, 1, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        // "Down" (-1, toward the back) restores the order.
        move_row(&mut core, 1, -1);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
    }

    #[test]
    fn cycle_row_opacity_walks_the_cycle_and_selects() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        // add_figure seeds opacity 1.0; the cycle is
        // 1.0 → 0.6 → 0.3 → 1.0 (layers::cycle_opacity).
        // Cycling row 1 while row 0 is selected also selects
        // the cycled row.
        core.select_figure(0);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.selected_figure, 1);
        assert_eq!(core.project.figures[1].opacity, 0.6);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.project.figures[1].opacity, 0.3);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.project.figures[1].opacity, 1.0);
        // The untouched figure keeps its opacity.
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn delete_row_removes_and_the_last_figure_is_refused() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let n = core.project.figures.len();
        assert!(n >= 2);
        // Deleting row 1 removes exactly that figure.
        delete_row(&mut core, 1);
        assert_eq!(core.project.figures.len(), n - 1);
        assert!(core.project.figures.iter().all(|f| f.name != "B"));
        // Delete down to one figure, then the op refuses.
        while core.project.figures.len() > 1 {
            delete_row(&mut core, 0);
        }
        delete_row(&mut core, 0);
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(core.status, "Keep at least one figure.");
    }
}
