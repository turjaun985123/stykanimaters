//! Figures panel (Phase D, section 1/40 of the Rune rebuild):
//! the figures sidebar — one row per figure with a colour dot,
//! name, eye/lock/solo toggles and opacity steppers, a header
//! with the figure count and "+ Figure", and a footer bulk
//! visibility row (All / None / Invert).
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

/// One figure row: colour dot + name (press selects), the
/// opacity readout, then the lock / solo / opacity-stepper /
/// eye toggles — the GPUI row's trailing cluster, minus the
/// reorder and delete buttons (not in this section's scope).
fn figure_row(r: &FigureRow, t: &Theme) -> El {
    let fi = r.index;
    let accent = tok::accent(0, true);
    let dot = hex_color(r.color).unwrap_or(Color::WHITE);
    row()
        .key(format!("fig-row-{fi}"))
        .w_full()
        .gap(6.0)
        .center_y()
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
        ))
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
}
