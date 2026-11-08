//! Figure inspector panel (Rune migration Phase D, section
//! 4/40; upgraded in Phase E, section 17/40, to dispatch
//! colour and duplicate through the real engine ops in
//! src/figure_ops.rs instead of the local dispatch helpers
//! this file carried while the engine lacked them). Declared from `main.rs` only (as
//! `mod rune_sec::inspector_figure` or the section
//! aggregator's re-export) — bin-crate code, not part of the
//! engine library.
//!
//! Shows the selected figure (`EditorCore::selected_figure`):
//! its name as a heading, a rename field, a 16-colour swatch
//! grid, an opacity stepper, and the Flip / Duplicate /
//! Delete actions. Every control dispatches to the engine
//! exactly the way the engine's own surfaces do:
//!
//! * Colour swatches → `EditorCore::set_figure_color(fi,
//!   rgb)` (src/figure_ops.rs) — the Phase E engine op for
//!   exactly this write. The swatch handler reads the
//!   current selection at press time and delegates; the
//!   write itself and the status line are the op's
//!   business, as with every other core op. Keys
//!   `fig-color-{rrggbb}`.
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
//! * Duplicate → `EditorCore::duplicate_figure(fi)`
//!   (src/figure_ops.rs) — the Phase E engine op that
//!   performs the structural clone (the instance plus its
//!   per-frame pose/bend rows, keeping the parallel
//!   per-frame tables parallel) and returns the copy's
//!   index as `Option<usize>`. The panel ignores the
//!   return value: selecting the copy and wording the
//!   status line are the op's business. Key
//!   `fig-duplicate`.
//! * Delete figure → `delete_figure()` (editor.rs:1603) —
//!   key `fig-del` (the GPUI sidebar's tuple-id root),
//!   danger-styled; the op itself refuses to delete the
//!   last figure with a status message.
//! * Rename → the Name field + Rename button (Enter in the
//!   field submits too): [`rename_selected_figure`] writes
//!   `project.figures[fi].name` directly. There is no
//!   rename op in the core, and figure names are plain
//!   field writes everywhere (the project rename,
//!   `project_tools::apply_project_name`, is the same
//!   shape), so — like `set_figure_color` — it takes **no
//!   undo snapshot**. Empty (all-whitespace) names are
//!   refused with a status line. The field's buffer lives
//!   in [`InspectorFigureEditState`] (shell `State` field
//!   `inspector_figure`) and is kept seeded with the
//!   selected figure's name by [`sync_figure_name_edit`],
//!   the focus-guarded sync the integrator calls from
//!   `poll()` — `project_tools::sync_name_edit`'s pattern,
//!   plus a selection-change trigger. Keys `fig-name` /
//!   `fig-rename`. The Rename button carries the Font
//!   Awesome `pencil` glyph via `icon_btn` (Phase H); its
//!   key, label and handler are unchanged.
//! * Scale 90% / 110% → [`scale_selected_figure`] — the
//!   whole-figure resize this panel adds in Phase G. There
//!   is no scale op in the core, so it composes the
//!   single-node editor's own `adjust_length`
//!   (editor.rs:489) over every node: one `snapshot()` for
//!   the whole operation (the helper's doc has the full
//!   reasoning), the op's own clamp [4.0, 220.0] per length.
//!   Keys `fig-scale-dn` / `fig-scale-up`. What scaling
//!   does NOT touch, by design or by model: the root
//!   position and all pose angles (lengths only — see the
//!   helper); segment thickness (stroke widths are
//!   `NodeDef.thickness`, a separate field with its own
//!   stepper elsewhere); node style (cap / colour2 /
//!   opacity); and the head ring — figure scaling leaves
//!   it alone, so a scaled figure keeps the same head
//!   size; the ring's size is a project setting of its
//!   own since Phase L (see the Head ring bullet below).
//!   Both
//!   buttons carry Font Awesome glyphs via `icon_btn`:
//!   the 110% button's `expand` since Phase H, and the
//!   90% button's `down-left-and-up-right-to-center`
//!   since Phase I — Phase H left it text-only because
//!   the curated set then had no shrink glyph to pair
//!   with `expand` (there is still no `compress`).
//! * Measurements → [`figure_measure`] — a muted readout
//!   line under the scale row (key `fig-measure`): joint
//!   count, summed segment length and the joints'
//!   bounding box at the display frame, all from the
//!   evaluated positions the stage draws
//!   (`EditorCore::positions_f`), so the numbers describe
//!   the posed figure, graph channels included.
//! * Centre figure / Ground figure →
//!   [`centre_selected_figure`] / [`ground_selected_figure`]
//!   — the placement row (key `fig-place-row`, buttons
//!   `fig-centre` / `fig-ground`) under the scale row.
//!   Each shifts the selected figure's root at the
//!   **current frame only** by the delta between the
//!   stage and the figure's drawn bounding box
//!   ([`figure_bbox`]: joint positions, grown by the head
//!   ring's radius around head-flagged joints, exactly as
//!   the stage draws it), written through the core's own
//!   root-move primitive `nudge_path_point`
//!   (editor.rs) — one snapshot, one undo step. Other
//!   frames keep their roots: centring is a pose edit at
//!   this frame, not a project-wide move.
//! * Head ring −/+ → [`step_head_ring_scale`] — a
//!   stepper row under the scale row (row key
//!   `fig-ring-row`, buttons `fig-ring-dn` /
//!   `fig-ring-up`): the project's head-ring scale
//!   (`Project::head_ring_scale`, Phase L) stepped by
//!   0.1 through the core's own
//!   `EditorCore::set_head_ring_scale` — clamped
//!   [0.5, 2.0], one snapshot per real change, the
//!   op's status line reports the press. The row's
//!   muted readout names the current scale
//!   ([`head_ring_label`]: "Head ring 1×",
//!   "Head ring 1.5×"). The scale is project-wide,
//!   not per-figure — it lives in this panel because
//!   the ring is part of how a figure reads — and
//!   [`figure_bbox`] grows head joints by the scaled
//!   radius (`Project::head_ring_radius()`), so
//!   Centre/Ground keep bounding the figure as drawn
//!   at any scale.
//!
//! When the project has no figures — or the selection is
//! stale — the panel is one muted line.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod inspector_figure;
//! }
//! // State field + init literal:
//! pub inspector_figure: rune_sec::inspector_figure::InspectorFigureEditState,
//! //     inspector_figure:
//! //         rune_sec::inspector_figure::InspectorFigureEditState::new(),
//! // in `view()`, with the other inspector sections:
//! .child(rune_sec::inspector_figure::inspector_figure(s, &t))
//! // in `poll()`, every tick (beside
//! // project_tools::sync_name_edit):
//! rune_sec::inspector_figure::sync_figure_name_edit(s);
//! // in `text_focused()`, so global shortcuts do not fire
//! // while the user types a name:
//! //     || s.inspector_figure.name.focused
//! ```

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, text_input, Color, El, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::icon_btn;
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

/// Measurements of the selected figure at the display
/// frame: how many joints it has, how long its segments
/// add up to, and how much stage space the posed figure
/// covers.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureMeasure {
    /// Joint count (the evaluated position count).
    pub nodes: usize,
    /// Sum of the parent→joint distances over every
    /// parented node, in stage units.
    pub total_length: f32,
    /// Bounding-box width of the joint positions.
    pub width: f32,
    /// Bounding-box height of the joint positions.
    pub height: f32,
}

/// Measure the selected figure at the display frame, or
/// `None` when there is no figure to measure (empty
/// project, stale selection, or a figure whose rig
/// evaluates to no joints).
///
/// Everything derives from `EditorCore::positions_f` —
/// the same evaluated positions the stage draws — so the
/// readout describes the figure as posed at this frame,
/// graph channels included. Segment lengths are the
/// parent→joint distances over the figure definition's
/// parent links, measured on those positions (forward
/// kinematics keeps each one equal to the node's authored
/// length; a graph channel that drives lengths is
/// reflected, as it is on stage). The selection is
/// resolved through `project.figures.get` first because
/// `positions_f` indexes the figure table directly and
/// would panic on a stale selection.
pub fn figure_measure(core: &EditorCore) -> Option<FigureMeasure> {
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let pos = core.positions_f(core.display_frame(), fi);
    if pos.is_empty() {
        return None;
    }
    let mut total_length = 0.0f32;
    for (i, node) in fig.def.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            if let (Some(&a), Some(&b)) = (pos.get(parent), pos.get(i)) {
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                total_length += (dx * dx + dy * dy).sqrt();
            }
        }
    }
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in &pos {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    Some(FigureMeasure {
        nodes: pos.len(),
        total_length,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

/// The readout line for a measure: joint count, total
/// length and bounding box, whole numbers throughout —
/// "{nodes} joints · {total} long · {width}×{height}".
pub fn measure_line(m: &FigureMeasure) -> String {
    format!(
        "{} joints · {:.0} long · {:.0}×{:.0}",
        m.nodes, m.total_length, m.width, m.height
    )
}

/// Base radius of the ring drawn around a head-flagged
/// joint: 17 stage units, before the project's
/// head-ring scale. Since Phase L the drawn radius is
/// `Project::head_ring_radius()` — this base times
/// `Project::head_ring_scale` — and [`figure_bbox`]
/// grows by that current radius, so the placement box
/// keeps bounding the figure as drawn at any scale.
/// The base value has a single source, the model's
/// `HEAD_RING_RADIUS`, re-exported here under this
/// panel's established name (Phase K) because the
/// panel's docs and tests still name the base value.
pub use stykanimate_rs::model::HEAD_RING_RADIUS as HEAD_RING_R;

/// The selected figure's bounding box at the display
/// frame as `[min_x, min_y, max_x, max_y]`, in stage
/// coordinates — or `None` under the same conditions as
/// [`figure_measure`] (no figure, stale selection, a rig
/// that evaluates to no joints).
///
/// From the same evaluated positions as the measure
/// (`EditorCore::positions_f`, display frame), with one
/// difference: a head-flagged joint contributes its
/// drawn head ring, not just its point — the box grows
/// by the project's current ring radius
/// (`Project::head_ring_radius()`, the base
/// [`HEAD_RING_R`] times the project's head-ring
/// scale) on every side of each head joint, so the box
/// bounds the figure as the stage actually draws it.
/// That is the box Centre/Ground place by: what you
/// see is what gets centred and grounded.
pub fn figure_bbox(core: &EditorCore) -> Option<[f32; 4]> {
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let pos = core.positions_f(core.display_frame(), fi);
    if pos.is_empty() {
        return None;
    }
    let ring_r = core.project.head_ring_radius();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (i, p) in pos.iter().enumerate() {
        let r = if fig.def.nodes.get(i).map(|n| n.head).unwrap_or(false) {
            ring_r
        } else {
            0.0
        };
        min_x = min_x.min(p[0] - r);
        min_y = min_y.min(p[1] - r);
        max_x = max_x.max(p[0] + r);
        max_y = max_y.max(p[1] + r);
    }
    Some([min_x, min_y, max_x, max_y])
}

/// Root shift that centres a `[min_x, min_y, max_x,
/// max_y]` bounding box on a `[width, height]` stage:
/// stage centre minus box centre, per axis.
pub fn centre_delta(bbox: [f32; 4], stage: [f32; 2]) -> [f32; 2] {
    [
        stage[0] / 2.0 - (bbox[0] + bbox[2]) / 2.0,
        stage[1] / 2.0 - (bbox[1] + bbox[3]) / 2.0,
    ]
}

/// Root shift that grounds a `[min_x, min_y, max_x,
/// max_y]` bounding box on a `[width, height]` stage:
/// no horizontal move; vertically, stage bottom minus
/// box bottom. Stage coordinates are screen-like — y
/// grows downward, as `model::evaluate` lays poses out
/// and the stage draws them — so the box's maximum y IS
/// its visual bottom, and adding this delta to the root
/// sets that bottom exactly on the stage's lower edge.
pub fn ground_delta(bbox: [f32; 4], stage: [f32; 2]) -> [f32; 2] {
    [0.0, stage[1] - bbox[3]]
}

/// Shared tail of the two placement actions: write
/// `delta` into the selected figure's root at the
/// current frame through the core's own root-move
/// primitive, `nudge_path_point` (editor.rs) — the op
/// takes the operation's one snapshot and performs the
/// write; only its status wording (motion-path nudging)
/// is replaced with the placement's own, the way this
/// panel's composed ops report themselves. The frame's
/// pose row is validated first, so a stale selection
/// refuses here with the panel's usual wording and no
/// snapshot, and the op itself is only ever called when
/// it will succeed.
fn shift_selected_figure_root(core: &mut EditorCore, delta: [f32; 2], status: String) -> bool {
    let fi = core.selected_figure;
    let frame = core.current;
    let valid = core
        .project
        .frames
        .get(frame)
        .map(|f| fi < f.poses.len())
        .unwrap_or(false);
    if !valid {
        core.status = "No such figure.".into();
        return false;
    }
    core.nudge_path_point(frame, fi, delta);
    core.status = status;
    true
}

/// Centre the selected figure on the stage at the
/// current frame: shift its root by [`centre_delta`] of
/// its drawn bounding box ([`figure_bbox`]) — one undo
/// step. **Current frame only**: the root lives in this
/// frame's pose, so other frames keep their own roots
/// and the figure's motion path gains a step here, the
/// same consequence as dragging the figure on the stage
/// at this frame. Refusals (no figure, stale selection,
/// a rig with no joints) are status-only and take no
/// snapshot, like every other refusal in this panel.
pub fn centre_selected_figure(core: &mut EditorCore) -> bool {
    let Some(bbox) = figure_bbox(core) else {
        core.status = "No such figure.".into();
        return false;
    };
    let delta = centre_delta(bbox, core.project.stage);
    let fi = core.selected_figure;
    shift_selected_figure_root(
        core,
        delta,
        format!("Figure {} centred on the stage.", fi + 1),
    )
}

/// Ground the selected figure at the current frame:
/// shift its root down by [`ground_delta`] of its drawn
/// bounding box ([`figure_bbox`]) so the box's bottom —
/// feet, or the head ring when the head hangs lowest —
/// rests exactly on the stage's bottom edge. One undo
/// step; **current frame only**, and the same
/// status-only, snapshot-free refusals as
/// [`centre_selected_figure`].
pub fn ground_selected_figure(core: &mut EditorCore) -> bool {
    let Some(bbox) = figure_bbox(core) else {
        core.status = "No such figure.".into();
        return false;
    };
    let delta = ground_delta(bbox, core.project.stage);
    let fi = core.selected_figure;
    shift_selected_figure_root(core, delta, format!("Figure {} grounded.", fi + 1))
}

/// Persistent edit state for the rename field — the one
/// piece of this panel that cannot be derived from the core
/// each frame (a half-typed name must survive re-renders).
/// Lives in the shell `State` as `State::inspector_figure`
/// (integrator wiring in the module docs). Named
/// `InspectorFigureEditState` because [`InspectorFigureState`]
/// above is this panel's derived, per-frame view-state.
#[derive(Debug, Default)]
pub struct InspectorFigureEditState {
    /// The rename field's text buffer.
    pub name: TextEdit,
    /// The figure index `name` was last synced from, so a
    /// selection change re-syncs the field even when two
    /// figures happen to share a name.
    pub for_figure: Option<usize>,
}

impl InspectorFigureEditState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Trim a raw rename-field string into the name to store,
/// or `None` when nothing but whitespace was entered —
/// `project_tools::clean_project_name` for figures.
pub fn clean_figure_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Rename the selected figure to `raw` (trimmed); returns
/// true when the name was written. There is no rename op in
/// the core: figure names are plain field writes everywhere
/// (`project_tools::apply_project_name` is the same shape),
/// so this writes `project.figures[fi].name` directly and —
/// like `set_figure_color` — takes **no undo snapshot**.
/// Bad input (empty name, stale selection) only sets the
/// status line, mirroring the figure ops' refusal wording.
pub fn rename_selected_figure(core: &mut EditorCore, raw: &str) -> bool {
    let Some(name) = clean_figure_name(raw) else {
        core.status = "Figure name cannot be empty.".into();
        return false;
    };
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get_mut(fi) else {
        core.status = "No such figure.".into();
        return false;
    };
    fig.name = name.clone();
    core.status = format!("Figure {} renamed to \"{name}\".", fi + 1);
    true
}

/// Scale every segment length of the selected figure by
/// `factor` (0.9 shrinks, 1.1 grows) as ONE undo step;
/// returns true when the lengths were written.
///
/// There is no whole-figure scale op in the core, so this
/// composes the single-node editor's own op,
/// `adjust_length` (editor.rs:489), once per node: select
/// the node, pass the delta `len * factor − len`, and the
/// op's own write — parented nodes only, clamped to
/// [4.0, 220.0] — does the rest. A length that would pass
/// a clamp simply lands on it; the scale still succeeds.
/// `adjust_length` never snapshots, so the one
/// `snapshot()` below (the keyframe_tools / pose_tools
/// multi-write pattern: guard, snapshot once, write,
/// status) is the scale's only undo entry. The node
/// selection is restored afterwards.
///
/// Only lengths change. The root position, every pose
/// angle and each node's `rest_angle` are deliberately
/// left alone: joint positions are derived from lengths
/// and angles by forward kinematics (`model::evaluate`),
/// so growing the lengths around an unmoved root grows
/// the figure in place — the least-surprise resize.
/// Touching the root or the angles would translate or
/// re-pose the figure as a side effect of a resize.
///
/// Refusals (no figure, a root-only figure, a
/// non-positive or non-finite factor) are status-only and
/// take no snapshot, like every other refusal in this
/// panel.
pub fn scale_selected_figure(core: &mut EditorCore, factor: f32) -> bool {
    if !factor.is_finite() || factor <= 0.0 {
        core.status = "Invalid scale factor.".into();
        return false;
    }
    let fi = core.selected_figure;
    let Some(fig) = core.project.figures.get(fi) else {
        core.status = "No such figure.".into();
        return false;
    };
    if !fig.def.nodes.iter().any(|n| n.parent.is_some()) {
        core.status = format!("Figure {} has no segments to scale.", fi + 1);
        return false;
    }
    core.snapshot();
    let saved_node = core.selected_node;
    let count = core.project.figures[fi].def.nodes.len();
    for ni in 0..count {
        let len = core.project.figures[fi].def.nodes[ni].length;
        core.selected_node = Some(ni);
        core.adjust_length(len * factor - len);
    }
    core.selected_node = saved_node;
    core.status = format!("Figure {} scaled to {:.0}%.", fi + 1, factor * 100.0);
    true
}

/// Keep the rename field showing the selected figure's
/// current name — `project_tools::sync_name_edit`'s
/// focus-guarded pattern, plus a selection-change trigger:
/// while the field is focused it is never touched (a sync
/// mid-typing would eat keystrokes); otherwise the field is
/// re-seeded whenever the selection moved or the figure's
/// name changed elsewhere (a duplicate selects its copy, a
/// load replaces the project). The cost, as with the
/// project name: a selection change that lands while the
/// user is typing in the field leaves it stale until they
/// stop editing — deliberate.
pub fn sync_name_field(core: &EditorCore, edit: &mut InspectorFigureEditState) {
    if edit.name.focused {
        return;
    }
    let Some(st) = figure_state(core) else {
        edit.for_figure = None;
        return;
    };
    if edit.for_figure != Some(st.figure) || edit.name.text != st.name {
        edit.name.replace_text(st.name);
        edit.for_figure = Some(st.figure);
    }
}

/// `State`-level wrapper the integrator calls from `poll()`
/// every tick (the same arrangement as
/// `project_tools::sync_name_edit`).
pub fn sync_figure_name_edit(s: &mut State) {
    sync_name_field(&s.core, &mut s.inspector_figure);
}

/// "Rename" (and Enter in the field): apply the field's
/// text to the selected figure.
fn apply_figure_name(s: &mut State) {
    let raw = s.inspector_figure.name.text.clone();
    rename_selected_figure(&mut s.core, &raw);
}

/// The rename row: Name label, the field, and the Rename
/// button. The field binds `State::inspector_figure`'s
/// buffer; [`sync_figure_name_edit`] (from `poll`) keeps it
/// seeded with the selected figure's name.
fn name_field_row(s: &State, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Name").font(type_::LABEL).color(t.text_secondary))
        .child(
            text_input(&s.inspector_figure.name, type_::BODY)
                .key("fig-name")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .on_edit(|s: &mut State| &mut s.inspector_figure.name)
                .on_submit(|s: &mut State, _tx: &mut Tx| apply_figure_name(s)),
        )
        .child(icon_btn(
            "fig-rename",
            "pencil",
            "Rename",
            false,
            t,
            apply_figure_name,
        ))
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

/// Head-ring scale step for the −/+ stepper: a tenth
/// per press, the granularity the ring row's readout
/// shows (one decimal at most).
pub const HEAD_RING_STEP: f32 = 0.1;

/// The head-ring row's readout for a scale: "Head ring
/// 1×" at the default, "Head ring 1.5×" at one and a
/// half — the number formatted trim-style (up to two
/// decimals, trailing zeros and a bare dot dropped),
/// the way the transport's speed label trims.
pub fn head_ring_label(scale: f32) -> String {
    let mut n = format!("{scale:.2}");
    while n.ends_with('0') {
        n.pop();
    }
    if n.ends_with('.') {
        n.pop();
    }
    format!("Head ring {n}×")
}

/// One press of the head-ring steppers: move the
/// project's head-ring scale by `delta` through the
/// core's own setter, `EditorCore::set_head_ring_scale`
/// — the op clamps to [0.5, 2.0] and takes the
/// snapshot, and its status line reports the press, so
/// this helper only reads the current scale
/// (`Project::head_ring_scale`) and hands the op the
/// stepped value. The scale is project-wide, not
/// per-figure: it resizes every head ring the
/// renderers draw, whichever figure is selected.
pub fn step_head_ring_scale(core: &mut EditorCore, delta: f32) {
    let next = core.project.head_ring_scale + delta;
    core.set_head_ring_scale(next);
}

/// The scale row: label and the 90% / 110% buttons that
/// resize the whole selected figure through
/// [`scale_selected_figure`] (which reads the selection at
/// press time, like the colour swatches do).
fn scale_stepper_row(t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text("Scale")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(icon_btn(
            "fig-scale-dn",
            "down-left-and-up-right-to-center",
            "90%",
            false,
            t,
            |s: &mut State| {
                scale_selected_figure(&mut s.core, 0.9);
            },
        ))
        .child(icon_btn(
            "fig-scale-up",
            "expand",
            "110%",
            false,
            t,
            |s: &mut State| {
                scale_selected_figure(&mut s.core, 1.1);
            },
        ))
}

/// The head-ring row, under the scale row: a muted
/// readout naming the project's current head-ring
/// scale ([`head_ring_label`]) and the −/+ steppers
/// that move it by [`HEAD_RING_STEP`] through
/// [`step_head_ring_scale`]. Plain buttons like the
/// opacity steppers — the row's state is the readout
/// itself, not a toggle.
fn ring_stepper_row(core: &EditorCore, t: &Theme) -> El {
    row()
        .key("fig-ring-row")
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(head_ring_label(core.project.head_ring_scale))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(btn("fig-ring-dn", "−", false, t, |s: &mut State| {
            step_head_ring_scale(&mut s.core, -HEAD_RING_STEP);
        }))
        .child(btn("fig-ring-up", "+", false, t, |s: &mut State| {
            step_head_ring_scale(&mut s.core, HEAD_RING_STEP);
        }))
}

/// The placement row: Centre figure / Ground figure,
/// shifting the selected figure's root at the current
/// frame through [`centre_selected_figure`] /
/// [`ground_selected_figure`] (which read the selection
/// at press time, like the scale row does).
fn place_row(t: &Theme) -> El {
    row()
        .key("fig-place-row")
        .w_full()
        .center_y()
        .gap(8.0)
        .child(btn("fig-centre", "Centre figure", false, t, |s: &mut State| {
            centre_selected_figure(&mut s.core);
        }))
        .child(btn("fig-ground", "Ground figure", false, t, |s: &mut State| {
            ground_selected_figure(&mut s.core);
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
/// border. Pressing applies it through the engine op
/// `EditorCore::set_figure_color`, targeted at whatever
/// figure is selected at press time.
fn swatch(rgb: u32, selected: bool) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    let mut e = el()
        .key(format!("fig-color-{rgb:06x}"))
        .size(22.0, 22.0)
        .radius(6.0)
        .fill(fill)
        .hover(fill)
        .press(fill)
        .on_press(move |s: &mut State| {
            let fi = s.core.selected_figure;
            s.core.set_figure_color(fi, rgb);
        });
    if selected {
        e = e.border(2.0, Color::WHITE);
    }
    e
}

/// The Figure inspector panel. Mount it wherever the shell
/// shows selection details; it reads `State.core` plus the
/// rename field's buffer in `State::inspector_figure`.
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
        .child(name_field_row(s, t))
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
    panel = panel.child(scale_stepper_row(t));
    panel = panel.child(ring_stepper_row(&s.core, t));
    if let Some(m) = figure_measure(&s.core) {
        panel = panel.child(
            text(measure_line(&m))
                .key("fig-measure")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    panel = panel.child(place_row(t));
    panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn("fig-flip", "Flip horizontal", false, t, |s| {
                s.core.mirror_selected_pose()
            }))
            .child(btn("fig-duplicate", "Duplicate", false, t, |s| {
                let fi = s.core.selected_figure;
                s.core.duplicate_figure(fi);
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
    fn clean_figure_name_trims_and_rejects_empty() {
        assert_eq!(clean_figure_name("  Hero  "), Some("Hero".to_string()));
        assert_eq!(clean_figure_name("Hero"), Some("Hero".to_string()));
        assert_eq!(clean_figure_name(""), None);
        assert_eq!(clean_figure_name("   "), None);
    }

    #[test]
    fn rename_writes_the_selected_figures_name() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        assert!(rename_selected_figure(&mut core, "  Captain  "));
        assert_eq!(core.project.figures[fi].name, "Captain");
        assert!(
            core.status.contains("renamed"),
            "status should report the rename: {}",
            core.status
        );
    }

    #[test]
    fn rename_rejects_empty_and_keeps_the_old_name() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let before = core.project.figures[fi].name.clone();
        assert!(!rename_selected_figure(&mut core, "   "));
        assert_eq!(core.project.figures[fi].name, before);
        assert_eq!(core.status, "Figure name cannot be empty.");
    }

    #[test]
    fn rename_guards_a_stale_selection() {
        let mut core = EditorCore::new();
        core.selected_figure = 999;
        assert!(!rename_selected_figure(&mut core, "New"));
        assert_eq!(core.status, "No such figure.");
    }

    #[test]
    fn sync_seeds_the_field_and_follows_selection() {
        let mut core = EditorCore::new();
        let mut edit = InspectorFigureEditState::new();
        sync_name_field(&core, &mut edit);
        assert_eq!(
            edit.name.text,
            core.project.figures[core.selected_figure].name
        );
        assert_eq!(edit.for_figure, Some(core.selected_figure));
        // A second figure with a distinct name: selecting it
        // re-seeds the field.
        core.add_figure();
        let last = core.project.figures.len() - 1;
        core.project.figures[last].name = "Second".into();
        core.select_figure(last);
        sync_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "Second");
        assert_eq!(edit.for_figure, Some(last));
    }

    #[test]
    fn sync_picks_up_an_external_rename() {
        let mut core = EditorCore::new();
        let mut edit = InspectorFigureEditState::new();
        sync_name_field(&core, &mut edit);
        let fi = core.selected_figure;
        core.project.figures[fi].name = "Renamed elsewhere".into();
        sync_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "Renamed elsewhere");
    }

    #[test]
    fn sync_never_touches_a_focused_field() {
        let mut core = EditorCore::new();
        let mut edit = InspectorFigureEditState::new();
        edit.name.replace_text("half-typed");
        edit.name.focused = true;
        sync_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "half-typed", "sync must not eat keystrokes");
        // Even a selection change leaves a focused field
        // alone...
        core.add_figure();
        let last = core.project.figures.len() - 1;
        core.select_figure(last);
        sync_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "half-typed");
        // ...and the next unfocused sync catches up to the
        // new selection.
        edit.name.focused = false;
        sync_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, core.project.figures[last].name);
        assert_eq!(edit.for_figure, Some(last));
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
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
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
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
        };
        let _ = inspector_figure(&bare, &t);
    }

    /// Every node's length in figure `fi`, in node order.
    fn lengths(core: &EditorCore, fi: usize) -> Vec<f32> {
        core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.length)
            .collect()
    }

    #[test]
    fn scale_multiplies_every_parented_length_and_nothing_else() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let parents: Vec<Option<usize>> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.parent)
            .collect();
        let before = lengths(&core, fi);
        // Roots and angles of every frame's pose for this
        // figure — the scale must not touch any of them.
        let roots_before: Vec<_> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].root)
            .collect();
        let angles_before: Vec<Vec<f32>> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].angles.clone())
            .collect();
        let rest_before: Vec<f32> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.rest_angle)
            .collect();

        assert!(scale_selected_figure(&mut core, 1.1));

        let after = lengths(&core, fi);
        for (i, (old, new)) in before.iter().zip(after.iter()).enumerate() {
            if parents[i].is_some() {
                if old * 1.1 > 220.0 {
                    assert_eq!(*new, 220.0, "node {i} should clamp at the max");
                } else {
                    assert!(
                        (new - old * 1.1).abs() < 0.01,
                        "node {i}: {old} scaled by 1.1 should be {}, got {new}",
                        old * 1.1
                    );
                }
            } else {
                assert_eq!(*new, *old, "root node {i} has no length to scale");
            }
        }
        for (f, frame) in core.project.frames.iter().enumerate() {
            assert_eq!(frame.poses[fi].root, roots_before[f], "frame {f} root moved");
            assert_eq!(
                frame.poses[fi].angles, angles_before[f],
                "frame {f} angles changed"
            );
        }
        let rest_after: Vec<f32> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.rest_angle)
            .collect();
        assert_eq!(rest_after, rest_before, "rest angles changed");
        assert!(
            core.status.contains("scaled to 110%"),
            "status should report the scale: {}",
            core.status
        );
    }

    #[test]
    fn scale_is_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let before = lengths(&core, fi);
        let depth = core.undo_depth();
        assert!(scale_selected_figure(&mut core, 1.1));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "the whole scale must be one snapshot, not one per node"
        );
        core.undo();
        assert_eq!(
            lengths(&core, fi),
            before,
            "a single undo must restore every length"
        );
    }

    #[test]
    fn scale_clamps_at_the_single_node_limits() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let parented: Vec<usize> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.is_some())
            .map(|(i, _)| i)
            .collect();
        assert!(!parented.is_empty(), "the demo figure has segments");
        let ni = parented[0];
        // 210 × 1.1 = 231 — past adjust_length's 220 max.
        core.project.figures[fi].def.nodes[ni].length = 210.0;
        assert!(scale_selected_figure(&mut core, 1.1));
        assert_eq!(
            core.project.figures[fi].def.nodes[ni].length, 220.0,
            "must land on the clamp, not beyond it"
        );
        // 4 × 0.9 = 3.6 — below adjust_length's 4 min.
        core.project.figures[fi].def.nodes[ni].length = 4.0;
        assert!(scale_selected_figure(&mut core, 0.9));
        assert_eq!(
            core.project.figures[fi].def.nodes[ni].length, 4.0,
            "must land on the clamp, not below it"
        );
    }

    #[test]
    fn scale_refuses_without_a_figure_and_takes_no_snapshot() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        let depth = core.undo_depth();
        assert!(!scale_selected_figure(&mut core, 1.1));
        assert_eq!(core.undo_depth(), depth, "refusal must be snapshot-free");
        assert_eq!(core.status, "No such figure.");

        let mut core = EditorCore::new();
        core.selected_figure = 999;
        assert!(!scale_selected_figure(&mut core, 0.9));
        assert_eq!(core.undo_depth(), 0, "refusal must be snapshot-free");
        assert_eq!(core.status, "No such figure.");
    }

    #[test]
    fn figure_measure_is_none_without_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert!(figure_measure(&core).is_none());
    }

    #[test]
    fn figure_measure_is_none_for_a_stale_selection() {
        let mut core = EditorCore::new();
        core.selected_figure = 999;
        // Must read as "nothing to measure", not panic:
        // positions_f indexes the figure table directly.
        assert!(figure_measure(&core).is_none());
    }

    #[test]
    fn figure_measure_reads_the_demo_figure() {
        let core = EditorCore::new();
        let fi = core.selected_figure;
        let m = figure_measure(&core).expect("the demo project seeds a figure");
        let pos = core.positions_f(core.display_frame(), fi);
        assert_eq!(m.nodes, core.project.figures[fi].def.nodes.len());
        assert_eq!(m.nodes, pos.len());
        assert!(m.nodes > 0);
        assert!(m.total_length > 0.0);
        assert!(m.width >= 0.0 && m.height >= 0.0);
        // The bounding box contains every joint: recompute
        // the extremes from the raw positions and compare.
        let min_x = pos.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = pos.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        let min_y = pos.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = pos.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        assert!((m.width - (max_x - min_x)).abs() < 0.01);
        assert!((m.height - (max_y - min_y)).abs() < 0.01);
        // With no graph channels, forward kinematics keeps
        // every segment at its authored length, so the
        // measured total is the sum of the parented nodes'
        // lengths.
        if core.project.channels.is_empty() {
            let authored: f32 = core.project.figures[fi]
                .def
                .nodes
                .iter()
                .filter(|n| n.parent.is_some())
                .map(|n| n.length)
                .sum();
            assert!(
                (m.total_length - authored).abs() < 0.1,
                "measured {} vs authored sum {authored}",
                m.total_length
            );
        }
    }

    #[test]
    fn figure_measure_matches_a_hand_built_chain() {
        use stykanimate_rs::model::{FigureDef, NodeDef, NodeStyle, Shape};
        let node = |name: &str, parent: Option<usize>, length: f32| NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].def = FigureDef {
            nodes: vec![
                node("root", None, 0.0),
                node("reach", Some(0), 30.0),
                node("drop", Some(1), 40.0),
            ],
        };
        // Pose (angles are degrees; dir(a) = [sin a,
        // cos a]): root at (100, 200); node 1 reaches +x
        // by 30 (angle 90); node 2 hangs +y by 40 from
        // node 1 (relative −90, absolute 0). The joints
        // land at (100, 200), (130, 200), (130, 240).
        for frame in core.project.frames.iter_mut() {
            frame.poses[fi].root = [100.0, 200.0];
            frame.poses[fi].angles[1] = 90.0;
            frame.poses[fi].angles[2] = -90.0;
        }
        let m = figure_measure(&core).expect("the chain measures");
        assert_eq!(m.nodes, 3);
        assert!(
            (m.total_length - 70.0).abs() < 0.01,
            "30 + 40 by hand, got {}",
            m.total_length
        );
        assert!(
            (m.width - 30.0).abs() < 0.01,
            "x spans 100..=130, got {}",
            m.width
        );
        assert!(
            (m.height - 40.0).abs() < 0.01,
            "y spans 200..=240, got {}",
            m.height
        );
    }

    #[test]
    fn measure_line_formats_whole_numbers() {
        let m = FigureMeasure {
            nodes: 14,
            total_length: 512.4,
            width: 120.6,
            height: 288.2,
        };
        assert_eq!(measure_line(&m), "14 joints · 512 long · 121×288");
    }

    #[test]
    fn centre_delta_is_zero_for_a_centred_box() {
        // Box centre (480, 360) is the 960×720 stage's
        // centre: nothing to shift.
        let d = centre_delta([380.0, 260.0, 580.0, 460.0], [960.0, 720.0]);
        assert_eq!(d, [0.0, 0.0]);
    }

    #[test]
    fn centre_delta_shifts_an_off_centre_box_exactly() {
        // Box centre (200, 300); stage centre (480, 360).
        let d = centre_delta([100.0, 100.0, 300.0, 500.0], [960.0, 720.0]);
        assert_eq!(d, [280.0, 60.0]);
        // A box past the centre shifts back by the mirror
        // amount.
        let d = centre_delta([660.0, 260.0, 860.0, 460.0], [960.0, 720.0]);
        assert_eq!(d, [-280.0, 0.0]);
    }

    #[test]
    fn ground_delta_drops_the_box_bottom_to_the_stage_bottom() {
        // Bottom at y 500 on a 720-tall stage: 220 down,
        // and never any horizontal shift.
        let d = ground_delta([100.0, 100.0, 300.0, 500.0], [960.0, 720.0]);
        assert_eq!(d, [0.0, 220.0]);
        // Already grounded: no shift.
        let d = ground_delta([100.0, 420.0, 300.0, 720.0], [960.0, 720.0]);
        assert_eq!(d, [0.0, 0.0]);
        // Bottom past the stage edge (a figure hanging
        // off-stage): the shift is negative, lifting it
        // back inside.
        let d = ground_delta([100.0, 100.0, 300.0, 800.0], [960.0, 720.0]);
        assert_eq!(d, [0.0, -80.0]);
    }

    /// The hand-built 3-node chain of
    /// `figure_measure_matches_a_hand_built_chain`, joints
    /// at (100, 200), (130, 200), (130, 240); `head`
    /// marks the root node as a head when true.
    fn chain_core(head: bool) -> EditorCore {
        use stykanimate_rs::model::{FigureDef, NodeDef, NodeStyle, Shape};
        let node = |name: &str, parent: Option<usize>, length: f32, head: bool| NodeDef {
            name: name.into(),
            parent,
            length,
            rest_angle: 0.0,
            head,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media: None,
        };
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].def = FigureDef {
            nodes: vec![
                node("root", None, 0.0, head),
                node("reach", Some(0), 30.0, false),
                node("drop", Some(1), 40.0, false),
            ],
        };
        for frame in core.project.frames.iter_mut() {
            frame.poses[fi].root = [100.0, 200.0];
            frame.poses[fi].angles[1] = 90.0;
            frame.poses[fi].angles[2] = -90.0;
        }
        core
    }

    #[test]
    fn figure_bbox_matches_the_hand_built_chain() {
        let core = chain_core(false);
        assert_eq!(figure_bbox(&core), Some([100.0, 200.0, 130.0, 240.0]));
    }

    #[test]
    fn figure_bbox_grows_around_head_joints_by_the_ring_radius() {
        // The root is the head: the box grows by
        // HEAD_RING_R around (100, 200) only — the other
        // joints contribute their bare points, so max_x /
        // max_y are unchanged.
        let core = chain_core(true);
        assert_eq!(
            figure_bbox(&core),
            Some([100.0 - HEAD_RING_R, 200.0 - HEAD_RING_R, 130.0, 240.0])
        );
    }

    #[test]
    fn figure_bbox_grows_by_the_scaled_ring_radius() {
        // At head-ring scale 2.0 the drawn radius is
        // 2 × 17 = 34, and the box grows by that — the
        // growth follows the project's scale, not the
        // base constant.
        let mut core = chain_core(true);
        core.set_head_ring_scale(2.0);
        assert_eq!(core.project.head_ring_radius(), 34.0);
        assert_eq!(
            figure_bbox(&core),
            Some([100.0 - 34.0, 200.0 - 34.0, 130.0, 240.0])
        );
    }

    #[test]
    fn figure_bbox_is_none_without_a_figure() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert!(figure_bbox(&core).is_none());
        let mut core = EditorCore::new();
        core.selected_figure = 999;
        assert!(figure_bbox(&core).is_none());
    }

    #[test]
    fn centre_puts_the_demo_figures_box_centre_on_the_stage_centre() {
        let mut core = EditorCore::new();
        let stage = core.project.stage;
        assert!(centre_selected_figure(&mut core));
        let b = figure_bbox(&core).expect("the demo figure still measures");
        let centre = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
        assert!(
            (centre[0] - stage[0] / 2.0).abs() < 0.5,
            "box centre x {} vs stage centre {}",
            centre[0],
            stage[0] / 2.0
        );
        assert!(
            (centre[1] - stage[1] / 2.0).abs() < 0.5,
            "box centre y {} vs stage centre {}",
            centre[1],
            stage[1] / 2.0
        );
        assert!(
            core.status.contains("centred on the stage"),
            "status should report the centring: {}",
            core.status
        );
    }

    #[test]
    fn ground_puts_the_demo_figures_box_bottom_on_the_stage_bottom() {
        let mut core = EditorCore::new();
        let stage = core.project.stage;
        assert!(ground_selected_figure(&mut core));
        let b = figure_bbox(&core).expect("the demo figure still measures");
        assert!(
            (b[3] - stage[1]).abs() < 0.5,
            "box bottom {} vs stage bottom {}",
            b[3],
            stage[1]
        );
        // Grounding never moves the figure sideways: the
        // box's horizontal span is exactly what it was
        // before the press (a fresh core holds the
        // un-grounded demo figure).
        let fresh = EditorCore::new();
        let before = figure_bbox(&fresh).expect("demo figure");
        assert_eq!(b[0], before[0]);
        assert_eq!(b[2], before[2]);
        assert!(
            core.status.contains("grounded"),
            "status should report the grounding: {}",
            core.status
        );
    }

    #[test]
    fn centre_is_one_undo_step_and_current_frame_only() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let current = core.current;
        let roots_before: Vec<_> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].root)
            .collect();
        let depth = core.undo_depth();
        assert!(centre_selected_figure(&mut core));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "the whole placement must be one snapshot"
        );
        for (f, frame) in core.project.frames.iter().enumerate() {
            if f != current {
                assert_eq!(
                    frame.poses[fi].root, roots_before[f],
                    "frame {f} is not the current frame: its root must not move"
                );
            }
        }
        core.undo();
        assert_eq!(
            core.project.frames[current].poses[fi].root, roots_before[current],
            "a single undo must restore the current frame's root"
        );
    }

    #[test]
    fn ground_is_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let root_before = core.project.frames[core.current].poses[fi].root;
        let depth = core.undo_depth();
        assert!(ground_selected_figure(&mut core));
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(
            core.project.frames[core.current].poses[fi].root, root_before,
            "a single undo must restore the root"
        );
    }

    #[test]
    fn placement_refuses_without_a_figure_and_takes_no_snapshot() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        let depth = core.undo_depth();
        assert!(!centre_selected_figure(&mut core));
        assert!(!ground_selected_figure(&mut core));
        assert_eq!(core.undo_depth(), depth, "refusals must be snapshot-free");
        assert_eq!(core.status, "No such figure.");

        let mut core = EditorCore::new();
        core.selected_figure = 999;
        let depth = core.undo_depth();
        assert!(!centre_selected_figure(&mut core));
        assert!(!ground_selected_figure(&mut core));
        assert_eq!(core.undo_depth(), depth, "refusals must be snapshot-free");
        assert_eq!(core.status, "No such figure.");
    }

    #[test]
    fn head_ring_label_trims_the_scale() {
        assert_eq!(head_ring_label(1.0), "Head ring 1×");
        assert_eq!(head_ring_label(1.5), "Head ring 1.5×");
        assert_eq!(head_ring_label(0.5), "Head ring 0.5×");
        assert_eq!(head_ring_label(2.0), "Head ring 2×");
        assert_eq!(head_ring_label(1.95), "Head ring 1.95×");
    }

    #[test]
    fn head_ring_stepper_steps_and_clamps_through_the_op() {
        // From the default 1.0, one press up is 1.1 and
        // one press back down returns to 1.0 — the value
        // lands on the project via the real setter.
        let mut core = EditorCore::new();
        assert_eq!(core.project.head_ring_scale, 1.0);
        step_head_ring_scale(&mut core, HEAD_RING_STEP);
        assert!((core.project.head_ring_scale - 1.1).abs() < 1e-6);
        step_head_ring_scale(&mut core, -HEAD_RING_STEP);
        assert!((core.project.head_ring_scale - 1.0).abs() < 1e-6);

        // 1.95 + 0.1 overshoots the top of the op's
        // [0.5, 2.0] range and clamps to exactly 2.0;
        // a further press up is clamped to the current
        // scale, so the op takes no further snapshot.
        core.set_head_ring_scale(1.95);
        step_head_ring_scale(&mut core, HEAD_RING_STEP);
        assert_eq!(core.project.head_ring_scale, 2.0);
        let depth = core.undo_depth();
        step_head_ring_scale(&mut core, HEAD_RING_STEP);
        assert_eq!(core.project.head_ring_scale, 2.0);
        assert_eq!(
            core.undo_depth(),
            depth,
            "a press clamped to the current scale must not snapshot"
        );

        // The bottom clamps the same way at 0.5.
        core.set_head_ring_scale(0.5);
        step_head_ring_scale(&mut core, -HEAD_RING_STEP);
        assert_eq!(core.project.head_ring_scale, 0.5);
    }
}
