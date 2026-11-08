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
//!
//! Phase F (section 18/40) adds a live thumbnail to each
//! row's first line: a [`THUMB_SIZE`]-point canvas drawing
//! the figure's rig at the current display frame — the same
//! `EditorCore::positions_f` evaluation the stage draws —
//! fitted into the box by [`thumb_fit`] and built by the
//! pure [`thumb_shape`] helper. The thumbnail is an identity
//! preview, so it draws every figure in its own colour
//! regardless of hidden/solo state (the row's name dimming
//! and eye toggle already carry visibility).

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, El, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::V2;

use crate::stage_view::{hex_color, ngon};
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

/// Edge length (points) of a figure row's thumbnail canvas.
pub const THUMB_SIZE: f32 = 44.0;

/// Inset (points) kept between the fitted silhouette and the
/// thumbnail's edge.
const THUMB_MARGIN: f32 = 4.0;

/// Stroke width (points) of thumbnail segments and head
/// rings — one hairline weight for the whole miniature.
const THUMB_WIDTH: f32 = 1.5;

/// Head-ring radius in stage units: the stage's own radius
/// (`stage_shape` draws head rings at 17 stage units), so
/// the miniature keeps the stage's proportions.
const HEAD_RING_R: f32 = 17.0;

/// Sides of the thumbnail head-ring polygon (the stage uses
/// 24 at full size; 16 is smooth at 44 points).
const HEAD_RING_SIDES: usize = 16;

/// One piece of thumbnail geometry in canvas points: a rig
/// segment between two mapped joints, or a head ring as a
/// polygon around a mapped head joint. `thumb_shape` turns
/// these into a `Shape`; keeping them separate makes the
/// geometry unit-testable (a built `Shape`'s ops are
/// crate-private to rune).
#[derive(Clone, Debug, PartialEq)]
pub enum ThumbPrim {
    Segment { a: [f32; 2], b: [f32; 2] },
    Ring { pts: Vec<[f32; 2]> },
}

/// The bounding-box fit of a thumbnail: stage points map to
/// canvas points centred in the `size` square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThumbFit {
    /// Canvas points per stage unit.
    pub scale: f32,
    /// Stage-space centre of the fitted box.
    pub centre: [f32; 2],
    /// Canvas edge length the fit targets.
    pub size: f32,
}

impl ThumbFit {
    /// Stage point → canvas point under this fit.
    pub fn map(&self, p: V2) -> [f32; 2] {
        [
            (p[0] - self.centre[0]) * self.scale + self.size / 2.0,
            (p[1] - self.centre[1]) * self.scale + self.size / 2.0,
        ]
    }
}

/// Fit the bounding box of `positions` into a `size` square,
/// keeping [`THUMB_MARGIN`] to spare on the limiting axis.
/// When the figure has a head node the box first grows by
/// the head-ring radius on every side, so the ring stays
/// inside the canvas instead of clipping at the box edge.
/// Degenerate boxes (one point, a straight vertical line)
/// clamp their extents to 1 stage unit so the scale stays
/// finite and the content lands centred.
pub fn thumb_fit(positions: &[V2], has_head: bool, size: f32) -> ThumbFit {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in positions {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    if positions.is_empty() {
        lo = [0.0, 0.0];
        hi = [0.0, 0.0];
    }
    if has_head {
        lo = [lo[0] - HEAD_RING_R, lo[1] - HEAD_RING_R];
        hi = [hi[0] + HEAD_RING_R, hi[1] + HEAD_RING_R];
    }
    let extents = [(hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0)];
    let avail = (size - 2.0 * THUMB_MARGIN).max(1.0);
    ThumbFit {
        scale: (avail / extents[0]).min(avail / extents[1]),
        centre: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
        size,
    }
}

/// The thumbnail geometry for figure `fi` at the current
/// display frame: one segment per parented node, then one
/// ring per head node, all fitted into the `size` box by
/// [`thumb_fit`]. Positions come from
/// `EditorCore::positions_f` at `EditorCore::display_frame`
/// — the stage's own evaluation, graph channels included.
/// Empty when `fi` is out of range or the figure has no
/// nodes.
pub fn thumb_prims(core: &EditorCore, fi: usize, size: f32) -> Vec<ThumbPrim> {
    let Some(fig) = core.project.figures.get(fi) else {
        return Vec::new();
    };
    if fig.def.nodes.is_empty() {
        return Vec::new();
    }
    let pos = core.positions_f(core.display_frame(), fi);
    let has_head = fig.def.nodes.iter().any(|n| n.head);
    let fit = thumb_fit(&pos, has_head, size);
    let mut prims = Vec::new();
    for (i, node) in fig.def.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            prims.push(ThumbPrim::Segment {
                a: fit.map(pos[p]),
                b: fit.map(pos[i]),
            });
        }
    }
    for (i, node) in fig.def.nodes.iter().enumerate() {
        if node.head {
            prims.push(ThumbPrim::Ring {
                pts: ngon(fit.map(pos[i]), HEAD_RING_R * fit.scale, HEAD_RING_SIDES),
            });
        }
    }
    prims
}

/// Figure `fi`'s rig at the current display frame as a
/// miniature `Shape` for a `size`-point square canvas:
/// segments as [`THUMB_WIDTH`]-point round-capped lines in
/// the figure's colour, plus a head ring per head node.
/// An out-of-range `fi` yields an empty shape.
pub fn thumb_shape(core: &EditorCore, fi: usize, size: f32) -> Shape {
    let Some(fig) = core.project.figures.get(fi) else {
        return Shape::new();
    };
    let color = hex_color(fig.color).unwrap_or(Color::WHITE);
    let mut shape = Shape::new();
    for prim in thumb_prims(core, fi, size) {
        shape = match prim {
            ThumbPrim::Segment { a, b } => shape.line(a, b, THUMB_WIDTH, LineCap::Round, color),
            ThumbPrim::Ring { pts } => {
                shape.stroke_with(pts, true, THUMB_WIDTH, LineCap::Round, color)
            }
        };
    }
    shape
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

/// One figure row, as a two-line card: line 1 is the live
/// thumbnail canvas (`fig-thumb-{fi}`, Phase F), then the
/// colour dot + name (press selects), the opacity readout,
/// then the lock / solo / opacity-stepper / eye toggles —
/// the GPUI row's trailing cluster; line 2 (Phase E) carries
/// the remaining GPUI row controls, right-aligned under the
/// cluster: reorder up/down (`fig-up-{fi}` / `fig-dn-{fi}`),
/// opacity cycle (`fig-opac-{fi}`) and delete (`fig-del-{fi}`).
/// Two lines keep every control at the same compact toggle
/// size instead of squeezing nine onto one line.
fn figure_row(r: &FigureRow, core: &EditorCore, t: &Theme) -> El {
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
                    // Live silhouette of this figure's rig at
                    // the display frame (Phase F); a pure view
                    // element — no press handler, no op.
                    canvas(thumb_shape(core, fi, THUMB_SIZE))
                        .key(format!("fig-thumb-{fi}"))
                        .size(THUMB_SIZE, THUMB_SIZE),
                )
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
        panel = panel.child(figure_row(&r, core, t));
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

    #[test]
    fn thumb_prims_match_the_demo_figures_rig() {
        let core = EditorCore::new();
        assert!(!core.project.figures.is_empty());
        for (fi, fig) in core.project.figures.iter().enumerate() {
            let prims = thumb_prims(&core, fi, THUMB_SIZE);
            let segments = prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Segment { .. }))
                .count();
            let rings = prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Ring { .. }))
                .count();
            // One segment per parented node, one ring per
            // head node — the stage's draw list, miniaturised.
            assert_eq!(
                segments,
                fig.def.nodes.iter().filter(|n| n.parent.is_some()).count(),
                "segment count for figure {fi}"
            );
            assert_eq!(
                rings,
                fig.def.nodes.iter().filter(|n| n.head).count(),
                "ring count for figure {fi}"
            );
            assert!(!prims.is_empty(), "demo figure {fi} draws something");
        }
    }

    #[test]
    fn thumb_points_stay_inside_the_box() {
        let core = EditorCore::new();
        for fi in 0..core.project.figures.len() {
            for prim in thumb_prims(&core, fi, THUMB_SIZE) {
                let pts: Vec<[f32; 2]> = match &prim {
                    ThumbPrim::Segment { a, b } => vec![*a, *b],
                    ThumbPrim::Ring { pts } => pts.clone(),
                };
                assert!(!pts.is_empty());
                for p in pts {
                    assert!(
                        p[0] >= -1e-3 && p[0] <= THUMB_SIZE + 1e-3,
                        "x of {p:?} outside the {THUMB_SIZE} box (figure {fi})"
                    );
                    assert!(
                        p[1] >= -1e-3 && p[1] <= THUMB_SIZE + 1e-3,
                        "y of {p:?} outside the {THUMB_SIZE} box (figure {fi})"
                    );
                }
            }
        }
    }

    #[test]
    fn thumb_shape_builds_scales_and_guards_range() {
        let core = EditorCore::new();
        let shape = thumb_shape(&core, 0, THUMB_SIZE);
        assert!(!shape.is_empty(), "the demo figure draws a thumbnail");
        // A bigger box produces a different drawing.
        assert_ne!(shape, thumb_shape(&core, 0, THUMB_SIZE * 2.0));
        // Out-of-range figure: empty results, no panic.
        assert!(thumb_shape(&core, 999, THUMB_SIZE).is_empty());
        assert!(thumb_prims(&core, 999, THUMB_SIZE).is_empty());
    }

    #[test]
    fn thumb_fit_centres_and_contains_the_bbox() {
        // A 100×50 rig in a 44 box: the width limits
        // (avail 36 / 100 = 0.36), the bbox centre lands on
        // the box centre, and the corners sit on or inside
        // the margin on the limiting axis.
        let pos = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
        let fit = thumb_fit(&pos, false, THUMB_SIZE);
        assert!((fit.scale - 0.36).abs() < 1e-6);
        assert_eq!(fit.map([50.0, 25.0]), [22.0, 22.0]);
        for p in pos {
            let m = fit.map(p);
            assert!(m[0] >= THUMB_MARGIN - 1e-4 && m[0] <= THUMB_SIZE - THUMB_MARGIN + 1e-4);
            assert!(m[1] >= THUMB_MARGIN - 1e-4 && m[1] <= THUMB_SIZE - THUMB_MARGIN + 1e-4);
        }
        // A single point maps to the centre at a finite scale.
        let dot = thumb_fit(&[[7.0, 9.0]], false, THUMB_SIZE);
        assert!(dot.scale.is_finite() && dot.scale > 0.0);
        assert_eq!(dot.map([7.0, 9.0]), [22.0, 22.0]);
        // No points at all: still a finite, positive scale.
        let empty = thumb_fit(&[], true, THUMB_SIZE);
        assert!(empty.scale.is_finite() && empty.scale > 0.0);
    }

    #[test]
    fn thumb_fit_grows_the_box_for_the_head_ring() {
        // Same rig with and without a head: the head fit must
        // zoom out (the ring needs 17 stage units of room on
        // every side) yet still centre the rig.
        let pos = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
        let plain = thumb_fit(&pos, false, THUMB_SIZE);
        let headed = thumb_fit(&pos, true, THUMB_SIZE);
        assert!(headed.scale < plain.scale);
        assert_eq!(headed.map([50.0, 25.0]), [22.0, 22.0]);
    }
}
