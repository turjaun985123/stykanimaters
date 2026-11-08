//! Segment inspector panel (Rune migration Phase D, section
//! 3/40). Declared from `main.rs` only (as
//! `mod rune_sec::inspector_segment` or the section aggregator's
//! re-export) — bin-crate code, not part of the engine library.
//!
//! Shown when a node is selected: real readouts from
//! `EditorCore` state (index, name, length, thickness, world
//! angle at the current frame, current-frame bend, shape,
//! colour) plus the editing controls, each wired to exactly
//! one real core op — the same ops the GPUI inspector's
//! buttons called, under the same element keys where one
//! existed:
//!
//! * Length −/+ → `adjust_length(∓4.0)` (editor.rs:466) —
//!   keys `insp-len-dn` / `insp-len-up`; the origin has no
//!   length (the op no-ops on it), so the row is omitted for
//!   the root, exactly as the GPUI inspector omitted it.
//! * Thickness −/+ → `adjust_thickness(∓1.0)` (editor.rs:475)
//!   — keys `insp-th-dn` / `insp-th-up`.
//! * Rotate −5°/−1°/+1°/+5° → `rotate_selected` (this
//!   module) — keys `seg-rot-m5` / `seg-rot-m1` /
//!   `seg-rot-p1` / `seg-rot-p5`. The core has no
//!   node-rotate op: a node's angle at a frame IS its pose
//!   angle (`frames[current].poses[fi].angles[ni]`, degrees
//!   — the value `key_graph_angle` keys and drag posing
//!   solves into), so the steppers write the current
//!   frame's pose directly, in the core ops' own pattern
//!   (validate → `snapshot()` → mutate → status, as
//!   `mirror_pose` does): one press = exactly one undo
//!   step. Pose semantics, not a graph key — an `Angle(ni)`
//!   graph channel still overrides the sampled pose at
//!   render (`positions_f`, editor.rs:1060). The stored
//!   angle is not normalised (the engine keeps unbounded
//!   degrees); the Angle readout normalises for display
//!   and shows one decimal via `format_deg` — the same
//!   formatter the Rotate row's value uses.
//! * Bend −5°/−1°/+1°/+5° → `bend_selected` (this
//!   module) — keys `seg-bend-m5` / `seg-bend-m1` /
//!   `seg-bend-p1` / `seg-bend-p5`, the fine-grained
//!   sibling of Cycle bend below. Bends live in
//!   `Frame.bends[fi][ni]` (model.rs:531), stored in
//!   **radians** in a SPARSE table parallel to `poses`:
//!   a missing figure row or a short node row reads as
//!   0.0 (straight), so the write grows the table first,
//!   exactly as `cycle_bend` grows it (editor.rs:1164).
//!   The model defines no hard clamp; the only range the
//!   engine itself ever writes is `cycle_bend`'s cycle,
//!   whose extremes are ±0.7 rad (≈40.1°), so the
//!   steppers clamp to ±`BEND_LIMIT_RAD` — an imported
//!   off-range bend is pulled to the limit on the first
//!   press, the same snap-toward-the-model spirit as the
//!   cycle snapping off-cycle values to a step. Bends
//!   are RENDER-ONLY: they never move a joint
//!   (`model::evaluate` and `positions_f` never see
//!   them); renderers fetch the interpolated
//!   `Project::bends_at` and arc the drawn segment
//!   through `model::bend_mid`. And unlike the pose
//!   angle, NO graph channel can override a bend —
//!   `ChannelTarget` (graph.rs:33) has only RootX /
//!   RootY / Angle / Length, so the stored bend is
//!   always the rendered bend. The origin has no parent
//!   segment and cannot bend (`cycle_bend` refuses it),
//!   so the row is omitted for the root, like the Length
//!   row. Steps are 1°/5° for the same reason the Rotate
//!   steps are: the panel speaks degrees for bends
//!   everywhere (the Bend readout, `cycle_bend`'s own
//!   status), while `bend_selected` takes the delta in
//!   the storage unit, radians — mirroring
//!   `rotate_selected`, whose degrees ARE the storage
//!   unit for angles.
//! * Bend readout + Straighten → `bend_degrees` /
//!   `straighten_selected` (this module) — the readout
//!   is one muted line under the header, beside the
//!   Index line (key `seg-bend-readout`): the selected
//!   segment's bend in whole degrees via
//!   `format_bend_readout` (negatives show their sign;
//!   a straight segment reads "Bend 0°").
//!   `bend_degrees` performs the same stored-value
//!   read as `segment_state`'s Bend readout — the
//!   current frame's `Frame.bends` entry,
//!   sparse-missing as 0 — NOT the renderers'
//!   interpolated `bends_at`, so the line can never
//!   disagree with the Bend readout row or the stepper
//!   row below it. Straighten (key `seg-bend-zero`,
//!   in the Bend stepper row) calls `bend_selected`
//!   with the exact negation of the stored radians, so
//!   the sum lands on bit-exact 0.0 — no degrees round
//!   trip, which can be an ulp off. At bend 0 the
//!   press is a silent no-op rather than an undo step:
//!   `bend_selected` takes its snapshot unconditionally
//!   once its guards pass, so even a zero delta would
//!   snapshot — the guard lives in
//!   `straighten_selected`, not in the shared op.
//! * Cycle bend → `cycle_bend()` (editor.rs:1106) — key
//!   `cycle-bend`. There is no ± bend op in the core; bend
//!   steps 0 → +0.35 → +0.7 → −0.35 → −0.7 rad per frame.
//! * Cycle shape → `cycle_shape()` (editor.rs:494) — key
//!   `node-shape` (line → circle → triangle → square →
//!   pentagon → hexagon → line).
//! * Colour swatches → `set_node_color(rgb)` (editor.rs:521)
//!   — keys `seg-color-{rrggbb}`; the palette mirrors the
//!   core's private `NODE_COLORS` cycle (editor.rs:484).
//! * Cycle colour → `cycle_node_color()` (editor.rs:506) —
//!   key `node-color`; the only op that can return a segment
//!   to the figure colour (`set_node_color` always writes
//!   `Some(rgb)`), so it stays as the "default" path.
//! * + Segment → `add_segment()` (editor.rs:402) — key
//!   `add-seg`.
//! * Delete segment → `delete_segment()` (editor.rs:434) —
//!   key `del-seg`, danger-styled; the op itself refuses the
//!   root and non-leaves with a status message.
//! * Index line → `segment_topology` (this module) — one
//!   muted line under the header giving the node's place
//!   in the figure tree: its own index, its parent's
//!   index (or "root") and how many children hang off
//!   it. Read-only. Length and world angle already have
//!   their own readout rows (see
//!   `InspectorSegmentState`), so this adds the topology
//!   the header's bare "Segment N" does not show.
//! * Joint navigation → `select_parent` /
//!   `select_first_child` (this module; Phase K) —
//!   keys `seg-nav-parent` / `seg-nav-child` in a row
//!   under the readout lines: the Index line's topology
//!   as buttons. Parent moves the selection to the
//!   node's parent; Child to its FIRST child (the
//!   lowest-index node naming it as parent — see
//!   `parent_of` / `first_child_of`). Selection is not
//!   an undoable edit — shell selection changes are
//!   bare `selected_node` field writes (home_panel
//!   clears the field the same way) — so the presses
//!   take no snapshot and set no status, and at the
//!   root (no parent) or a leaf (no child) a press is
//!   a silent no-op. (The audit behind this addition:
//!   length steppers already exist above —
//!   `insp-len-dn` / `insp-len-up` over
//!   `adjust_length`, editor.rs:489 — so the panel's
//!   missing segment control was navigation, not
//!   length.)
//!
//! When nothing is selected the panel is one muted line.
//!
//! Phase H icon pass: the header title leads with the
//! `shapes` glyph (`icons::icon_el`, rendered only when
//! the icon set has it), Cycle bend renders through
//! `icon_btn` with the `repeat` glyph, and Delete
//! segment keeps its danger styling with the `trash-can`
//! glyph inside it — element keys and labels unchanged
//! everywhere. Every stepper (Length, Thickness,
//! Rotate, Bend) deliberately stays text: compactness
//! is their point.
//!
//! Phase N adoption: the root is the kit card
//! (`design::panel_col`), the header is
//! `design::panel_title` (the `shapes` glyph rides
//! inside it, still guarded by the kit's own `has_icon`
//! check) over a `design::divider`, the steppers are
//! F-STEP (charter order — label, −, `design::
//! stepper_value`, +; the Rotate/Bend multi-step
//! clusters keep their buttons and only their readouts
//! become `stepper_value`), Delete segment is the kit
//! `danger_icon_btn` (this file's local hex-literal
//! copy is deleted), the colour swatches are
//! `design::swatch`, and the "Colour" heading is a kit
//! `section_label`. Keys, handlers and status strings
//! are unchanged.

use rune::tok::{type_, Theme};
use rune::{row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{NodeDef, Shape};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn};
use crate::stage_view::hex_color;
use crate::State;

/// Step sizes — the GPUI inspector's steps (its stepper
/// handlers called `adjust_length(±4.0)` /
/// `adjust_thickness(±1.0)`).
pub const LENGTH_STEP: f32 = 4.0;
pub const THICKNESS_STEP: f32 = 1.0;

/// The segment colour palette: the colour entries of the
/// core's `NODE_COLORS` cycle (editor.rs:484), in cycle
/// order. (`NODE_COLORS` also has a leading `None` — the
/// figure colour — which `set_node_color` cannot write; the
/// panel's Cycle colour button reaches it.)
pub const PALETTE: [u32; 6] = [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830];

/// Derived view-state for the panel: everything the builder
/// renders, computed once from the core so the builder stays
/// a pure function of it (and so the readout math is unit
/// testable without elements).
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorSegmentState {
    /// Selected figure index.
    pub figure: usize,
    /// Selected node index within the figure's definition.
    pub node: usize,
    /// Node name from the definition.
    pub name: String,
    /// True for the figure's origin node (no parent segment).
    pub is_root: bool,
    /// Segment length (definition units).
    pub length: f32,
    /// Segment thickness (definition units).
    pub thickness: f32,
    /// World direction of the segment at the current frame,
    /// degrees in `[0, 360)` — from the solved positions, so
    /// it is the angle actually on screen. For the root (no
    /// segment) it is the node's pose angle at the frame.
    pub angle_deg: f32,
    /// Current frame's bend for this node, degrees (0 when
    /// the frame stores no bend for it — the same read
    /// `cycle_bend` performs).
    pub bend_deg: f32,
    /// The node's own colour, or `None` for the figure colour.
    pub color: Option<u32>,
    /// Human label of the node's shape ("Line", "Circle",
    /// "Triangle", …).
    pub shape_label: String,
}

/// Wrap an angle in degrees into `[0, 360)`.
pub fn norm_deg_360(a: f32) -> f32 {
    let r = a % 360.0;
    if r < 0.0 {
        r + 360.0
    } else {
        r
    }
}

/// Format an angle in degrees with one decimal place. The
/// single formatter behind the Angle readout and the Rotate
/// row's value, so the two can never disagree.
pub fn format_deg(deg: f32) -> String {
    format!("{deg:.1}°")
}

/// Rotate the selected node's angle at the current frame by
/// `delta_deg` degrees (see the module docs for why this is
/// a pose write and what its undo semantics are). Refusals
/// are status-only — no snapshot, no edit — exactly like
/// the core ops' guards.
pub fn rotate_selected(core: &mut EditorCore, delta_deg: f32) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then rotate it.".into();
        return;
    };
    let fi = core.selected_figure;
    let has_angle = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .and_then(|p| p.angles.get(ni))
        .is_some();
    if !has_angle {
        core.status = format!(
            "Figure {} has no pose angle for segment {} on frame {}.",
            fi + 1,
            ni,
            core.current + 1
        );
        return;
    }
    core.snapshot();
    let pose = &mut core.project.frames[core.current].poses[fi];
    pose.angles[ni] += delta_deg;
    let new = pose.angles[ni];
    core.status = format!(
        "Segment {} angle {} on frame {}.",
        ni,
        format_deg(new),
        core.current + 1
    );
}

/// The bend limit, in radians: the extreme of
/// `cycle_bend`'s cycle (editor.rs:1143, steps
/// 0 / ±0.35 / ±0.7). The model stores bends unclamped,
/// but ±0.7 rad (≈40.1°) is the largest magnitude the
/// engine itself ever writes, so the steppers clamp
/// there — see the module docs.
pub const BEND_LIMIT_RAD: f32 = 0.7;

/// Bend the selected node's segment at the current frame
/// by `delta_rad` radians — the storage unit of
/// `Frame.bends` (see the module docs for the storage,
/// the clamp and why bends, unlike angles, have no graph
/// override). Same discipline as `rotate_selected`:
/// validate → ONE `snapshot()` → write → status, so one
/// press is exactly one undo step; refusals are
/// status-only. The bend table is sparse, so the write
/// grows it first, in `cycle_bend`'s own pattern
/// (editor.rs:1164): outer vec to the figure, inner vec
/// to the figure's node count, zero-filled.
pub fn bend_selected(core: &mut EditorCore, delta_rad: f32) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then bend it.".into();
        return;
    };
    if ni == 0 {
        // Mirrors cycle_bend's refusal (editor.rs:1135):
        // the origin has no parent segment to arc.
        core.status = "The origin has no segment to bend.".into();
        return;
    }
    let fi = core.selected_figure;
    let nodes = match core.project.figures.get(fi) {
        Some(fig) if ni < fig.def.nodes.len() => fig.def.nodes.len(),
        _ => {
            core.status = format!(
                "Figure {} has no segment {} to bend.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if core.current >= core.project.frames.len() {
        core.status = format!("No frame {} to bend on.", core.current + 1);
        return;
    }
    let cur = core.project.frames[core.current]
        .bends
        .get(fi)
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0);
    let new = (cur + delta_rad).clamp(-BEND_LIMIT_RAD, BEND_LIMIT_RAD);
    core.snapshot();
    let frame = &mut core.project.frames[core.current];
    if frame.bends.len() <= fi {
        frame.bends.resize(fi + 1, Vec::new());
    }
    if frame.bends[fi].len() < nodes {
        frame.bends[fi].resize(nodes, 0.0);
    }
    frame.bends[fi][ni] = new;
    core.status = format!(
        "Segment {} bend {} on frame {}.",
        ni,
        format_deg(new.to_degrees()),
        core.current + 1
    );
}

/// The selected node's bend at the current frame, in
/// degrees, or `None` when no (valid) node is selected —
/// the same defensive indexing as `segment_state`, so a
/// stale selection reads as "nothing selected" instead
/// of panicking. The read is the stored-value read the
/// panel already uses for its Bend readout (the current
/// frame's `Frame.bends` entry, sparse-missing as 0.0),
/// NOT the renderers' interpolated
/// `Project::bends_at`: the steppers and `cycle_bend`
/// also work on the stored value, so every bend surface
/// in the panel agrees. The origin qualifies (its
/// stored bend is definitionally 0 — nothing can write
/// it). This is the value behind the panel's
/// `seg-bend-readout` line.
pub fn bend_degrees(core: &EditorCore) -> Option<f32> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    if fig.def.nodes.get(ni).is_none() {
        return None;
    }
    Some(
        core.project
            .frames
            .get(core.current)
            .and_then(|f| f.bends.get(fi))
            .and_then(|b| b.get(ni))
            .copied()
            .unwrap_or(0.0)
            .to_degrees(),
    )
}

/// Format a bend in degrees as the readout line's
/// text: whole degrees, the sign shown on negatives
/// ("Bend -20°"), a straight segment reading exactly
/// "Bend 0°" (a negative zero is normalised away —
/// `-0.0 == 0.0`, so the remap costs nothing). The
/// single formatter behind the `seg-bend-readout`
/// line, so the wording lives in exactly one place
/// (and is unit testable without elements).
pub fn format_bend_readout(deg: f32) -> String {
    let deg = if deg == 0.0 { 0.0 } else { deg };
    format!("Bend {deg:.0}°")
}

/// Straighten the selected node's segment: set its
/// bend at the current frame to exactly 0, through the
/// steppers' own write path (`bend_selected` with the
/// negated current bend), so a press is exactly one
/// undo step with `bend_selected`'s status. Two
/// details keep it honest:
///
/// * The delta is the exact negation of the STORED
///   RADIANS, read here — not `-bend_degrees().to_radians()`:
///   the radians → degrees → radians round trip can be
///   an ulp off, while `cur + (-cur)` is bit-exact 0.0.
/// * At bend 0 the press is a silent no-op — no write,
///   no status, NO undo step. `bend_selected` takes
///   its snapshot unconditionally once its guards pass,
///   so delegating a zero delta would still snapshot;
///   the guard has to live here. With no (valid)
///   selection there is no bend to read, so the call
///   delegates to `bend_selected` with a zero delta
///   purely for its refusals, which all return
///   status-only BEFORE the snapshot — nothing is
///   written on that path either.
pub fn straighten_selected(core: &mut EditorCore) {
    let Some(deg) = bend_degrees(core) else {
        bend_selected(core, 0.0);
        return;
    };
    if deg == 0.0 {
        return;
    }
    // `bend_degrees` succeeded, so the selection is
    // valid; the `unwrap_or` is unreachable armour, and
    // node 0 would simply take `bend_selected`'s root
    // refusal.
    let fi = core.selected_figure;
    let ni = core.selected_node.unwrap_or(0);
    let cur = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.bends.get(fi))
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0);
    bend_selected(core, -cur);
}

/// Label for a node shape, naming the polygons `cycle_shape`
/// steps through (editor.rs:494).
pub fn shape_label(shape: &Shape) -> String {
    match shape {
        Shape::Line => "Line".to_string(),
        Shape::Circle => "Circle".to_string(),
        Shape::Polygon(3) => "Triangle".to_string(),
        Shape::Polygon(4) => "Square".to_string(),
        Shape::Polygon(5) => "Pentagon".to_string(),
        Shape::Polygon(6) => "Hexagon".to_string(),
        Shape::Polygon(n) => format!("Polygon {n}"),
    }
}

/// Compute the panel's view-state from the core, or `None`
/// when no (valid) node is selected. All indexing is
/// defensive: a stale selection left over from a figure edit
/// reads as "nothing selected" instead of panicking.
pub fn segment_state(core: &EditorCore) -> Option<InspectorSegmentState> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let node = fig.def.nodes.get(ni)?;
    // World angle from the solved positions at the current
    // frame (graph-aware, like the stage's `positions_f`).
    let angle_deg = match node.parent {
        Some(p) => {
            let pos = core.positions_f(core.current as f32, fi);
            match (pos.get(p), pos.get(ni)) {
                (Some(a), Some(b)) => {
                    norm_deg_360((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees())
                }
                _ => 0.0,
            }
        }
        None => core
            .project
            .frames
            .get(core.current)
            .and_then(|f| f.poses.get(fi))
            .and_then(|pose| pose.angles.get(ni))
            .map(|a| norm_deg_360(*a))
            .unwrap_or(0.0),
    };
    // The bend read mirrors `cycle_bend` (editor.rs:1106):
    // missing table entries read as 0.
    let bend_deg = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.bends.get(fi))
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0)
        .to_degrees();
    Some(InspectorSegmentState {
        figure: fi,
        node: ni,
        name: node.name.clone(),
        is_root: node.parent.is_none(),
        length: node.length,
        thickness: node.thickness,
        angle_deg,
        bend_deg,
        color: node.color,
        shape_label: shape_label(&node.shape),
    })
}

/// The selected node's place in its figure's tree: its
/// own index, its parent's index (`None` for the origin)
/// and how many nodes hang directly off it. Returns
/// `None` when no (valid) node is selected — the same
/// defensive indexing as `segment_state`, so a stale
/// selection reads as "nothing selected" instead of
/// panicking.
pub fn segment_topology(core: &EditorCore) -> Option<(usize, Option<usize>, usize)> {
    let ni = core.selected_node?;
    let fig = core.project.figures.get(core.selected_figure)?;
    let node = fig.def.nodes.get(ni)?;
    let children = fig
        .def
        .nodes
        .iter()
        .filter(|n| n.parent == Some(ni))
        .count();
    Some((ni, node.parent, children))
}

/// Format a `segment_topology` triple as the panel's
/// Index line: "Node 3 · parent 1 · 1 child" for a
/// mid-tree node, "Node 0 · root · 3 children" for the
/// origin. The single formatter behind the line, so the
/// wording lives in exactly one place (and is unit
/// testable without elements).
pub fn format_topology(node: usize, parent: Option<usize>, children: usize) -> String {
    let parent_part = match parent {
        Some(p) => format!("parent {p}"),
        None => "root".to_string(),
    };
    let child_part = if children == 1 {
        "1 child".to_string()
    } else {
        format!("{children} children")
    };
    format!("Node {node} · {parent_part} · {child_part}")
}

/// The parent of node `ni` in a figure definition, or
/// `None` for the origin — and for an out-of-range
/// index, so a stale selection navigates nowhere
/// instead of panicking. The pure walk behind the
/// panel's Parent joint button: the same `parent`
/// field the Index line (`segment_topology`) reads.
pub fn parent_of(nodes: &[NodeDef], ni: usize) -> Option<usize> {
    nodes.get(ni)?.parent
}

/// The first child of node `ni`: the lowest-index node
/// whose parent is `ni`, or `None` for a leaf (and for
/// an out-of-range index). "First" is definition
/// order — the order the Index line's child count
/// enumerates, and the order a reader meets the
/// children in the definition.
pub fn first_child_of(nodes: &[NodeDef], ni: usize) -> Option<usize> {
    nodes.get(ni)?;
    nodes.iter().position(|n| n.parent == Some(ni))
}

/// Move the selection to the selected node's parent
/// (the Parent joint button). Selection is not an
/// undoable edit — shell selection changes are bare
/// `selected_node` field writes — so this takes no
/// snapshot and sets no status; at the origin, or with
/// no (valid) selection, the press is a silent no-op.
pub fn select_parent(core: &mut EditorCore) {
    let target = core.selected_node.and_then(|ni| {
        let fig = core.project.figures.get(core.selected_figure)?;
        parent_of(&fig.def.nodes, ni)
    });
    if let Some(p) = target {
        core.selected_node = Some(p);
    }
}

/// Move the selection to the selected node's first
/// child (the Child joint button) — the downward twin
/// of [`select_parent`], with the same discipline: no
/// snapshot, no status, and a leaf press is a silent
/// no-op.
pub fn select_first_child(core: &mut EditorCore) {
    let target = core.selected_node.and_then(|ni| {
        let fig = core.project.figures.get(core.selected_figure)?;
        first_child_of(&fig.def.nodes, ni)
    });
    if let Some(c) = target {
        core.selected_node = Some(c);
    }
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

/// One stepper row in the charter's F-STEP order
/// (charter §6): the label grows, then the − button,
/// the value as the kit's centred mono readout
/// (`design::stepper_value`), and the + button (chrome
/// `btn` styling, GPUI inspector keys).
fn stepper_row(
    label: &str,
    value: String,
    dn_key: &str,
    up_key: &str,
    t: &Theme,
    dn: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
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
        .child(btn(dn_key, "−", false, t, dn))
        .child(design::stepper_value(&value, t))
        .child(btn(up_key, "+", false, t, up))
}

/// One colour swatch: the kit's single swatch
/// construction ([`design::swatch`] — the Phase N
/// adoption retired this file's hand-rolled 22×22
/// square and its white selected border for the kit's
/// 24×24 square with the accent selected ring),
/// carrying this panel's `seg-color-{rgb:06x}` key.
/// Pressing writes the colour via `set_node_color`.
fn swatch(rgb: u32, selected: bool, t: &Theme) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    design::swatch(
        &format!("seg-color-{rgb:06x}"),
        fill,
        selected,
        t,
        move |s: &mut State| s.core.set_node_color(rgb),
    )
}

/// The Segment inspector panel. Mount it wherever the shell
/// shows selection details; it reads only `State.core`.
pub fn inspector_segment(s: &State, t: &Theme) -> El {
    let panel = design::panel_col("inspector-segment", t);
    let Some(st) = segment_state(&s.core) else {
        return panel.child(
            text("Select a segment on the stage to edit it.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    // Panel header (Phase N): the kit title cluster —
    // the `shapes` glyph leads when the icon set has it
    // (the kit checks `has_icon` itself, the Phase H
    // guard's exact behaviour), the title text is
    // byte-identical to before, and a hairline divider
    // sits under the header (charter §2).
    let mut panel = panel
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("shapes"),
                    &format!("Segment {} · {}", st.node, st.name),
                    None,
                    t,
                )),
        )
        .child(design::divider(t));
    // The Index line: one muted line under the header
    // naming the node's place in the figure tree (see
    // `segment_topology`). `segment_state` succeeded
    // above, so this read succeeds too; the `if let` is
    // belt-and-braces, not an expected path.
    if let Some((ni, parent, children)) = segment_topology(&s.core) {
        panel = panel.child(
            text(format_topology(ni, parent, children))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    // The Bend readout line: the selected segment's
    // stored bend in whole degrees (see `bend_degrees`),
    // keyed so shell tests can find it. `segment_state`
    // succeeded above, so this read succeeds too; the
    // `if let` mirrors the Index line's idiom.
    if let Some(deg) = bend_degrees(&s.core) {
        panel = panel.child(
            text(format_bend_readout(deg))
                .key("seg-bend-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    // Joint navigation (Phase K): walk the selection up
    // to the parent joint or down to the first child —
    // the Index line's topology as buttons. Always live
    // while a node is selected; at the root or a leaf
    // the press is a silent no-op (see `select_parent`).
    panel = panel.child(
        row()
            .key("seg-nav-row")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn("seg-nav-parent", "Parent joint", false, t, |s| {
                select_parent(&mut s.core)
            }))
            .child(btn("seg-nav-child", "Child joint", false, t, |s| {
                select_first_child(&mut s.core)
            })),
    );
    panel = panel
        .child(readout_row("Length", format!("{:.0}", st.length), t))
        .child(readout_row("Thickness", format!("{:.0}", st.thickness), t))
        .child(readout_row("Angle", format_deg(st.angle_deg), t))
        .child(readout_row("Bend", format_deg(st.bend_deg), t))
        .child(readout_row("Shape", st.shape_label.clone(), t));
    // The origin has no segment to lengthen (the op no-ops);
    // the GPUI inspector omitted the row there too.
    if !st.is_root {
        panel = panel.child(stepper_row(
            "Length",
            format!("{:.0}", st.length),
            "insp-len-dn",
            "insp-len-up",
            t,
            |s| s.core.adjust_length(-LENGTH_STEP),
            |s| s.core.adjust_length(LENGTH_STEP),
        ));
    }
    panel = panel
        .child(stepper_row(
            "Thickness",
            format!("{:.0}", st.thickness),
            "insp-th-dn",
            "insp-th-up",
            t,
            |s| s.core.adjust_thickness(-THICKNESS_STEP),
            |s| s.core.adjust_thickness(THICKNESS_STEP),
        ))
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Rotate")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(design::stepper_value(&format_deg(st.angle_deg), t))
                .child(btn("seg-rot-m5", "−5°", false, t, |s| {
                    rotate_selected(&mut s.core, -5.0)
                }))
                .child(btn("seg-rot-m1", "−1°", false, t, |s| {
                    rotate_selected(&mut s.core, -1.0)
                }))
                .child(btn("seg-rot-p1", "+1°", false, t, |s| {
                    rotate_selected(&mut s.core, 1.0)
                }))
                .child(btn("seg-rot-p5", "+5°", false, t, |s| {
                    rotate_selected(&mut s.core, 5.0)
                })),
        );
    // The Bend stepper row, mirroring Rotate — omitted
    // for the origin, which has no segment to bend (the
    // Length row precedent; `bend_selected` refuses the
    // root exactly as `cycle_bend` does). Buttons speak
    // degrees like every other bend surface in the panel;
    // `bend_selected` takes radians, the storage unit.
    if !st.is_root {
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Bend")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(design::stepper_value(&format_deg(st.bend_deg), t))
                .child(btn("seg-bend-m5", "−5°", false, t, |s| {
                    bend_selected(&mut s.core, (-5.0f32).to_radians())
                }))
                .child(btn("seg-bend-m1", "−1°", false, t, |s| {
                    bend_selected(&mut s.core, (-1.0f32).to_radians())
                }))
                .child(btn("seg-bend-p1", "+1°", false, t, |s| {
                    bend_selected(&mut s.core, (1.0f32).to_radians())
                }))
                .child(btn("seg-bend-p5", "+5°", false, t, |s| {
                    bend_selected(&mut s.core, (5.0f32).to_radians())
                }))
                .child(btn("seg-bend-zero", "Straighten", false, t, |s| {
                    straighten_selected(&mut s.core)
                })),
        );
    }
    panel = panel
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(icon_btn("cycle-bend", "repeat", "Cycle bend", false, t, |s| {
                    s.core.cycle_bend()
                }))
                .child(btn("node-shape", "Cycle shape", false, t, |s| {
                    s.core.cycle_shape()
                })),
        )
        .child(design::section_label("Colour", t));
    let mut swatches = row().gap(6.0).center_y();
    for rgb in PALETTE {
        swatches = swatches.child(swatch(rgb, st.color == Some(rgb), t));
    }
    swatches = swatches.child(btn("node-color", "Cycle", false, t, |s| {
        s.core.cycle_node_color()
    }));
    panel = panel.child(swatches);
    panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn("add-seg", "+ Segment", false, t, |s| s.core.add_segment()))
            .child(danger_icon_btn(
                "del-seg",
                "trash-can",
                "Delete segment",
                false,
                t,
                |s| s.core.delete_segment(),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with_selection(ni: Option<usize>) -> EditorCore {
        let mut core = EditorCore::new();
        core.selected_node = ni;
        core
    }

    #[test]
    fn segment_state_is_none_without_selection() {
        // EditorCore::new selects no node.
        assert!(segment_state(&EditorCore::new()).is_none());
        assert!(segment_state(&core_with_selection(None)).is_none());
    }

    #[test]
    fn segment_state_guards_a_stale_selection() {
        assert!(segment_state(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn segment_state_reads_the_selected_node() {
        let core = core_with_selection(Some(1));
        let st = segment_state(&core).expect("node 1 exists in the demo figure");
        let node = &core.project.figures[core.selected_figure].def.nodes[1];
        assert_eq!(st.node, 1);
        assert_eq!(st.figure, core.selected_figure);
        assert_eq!(st.name, node.name);
        assert_eq!(st.length, node.length);
        assert_eq!(st.thickness, node.thickness);
        assert_eq!(st.color, node.color);
        assert!(!st.is_root, "node 1 hangs off the origin");
        assert!(
            (0.0..360.0).contains(&st.angle_deg),
            "angle not normalised: {}",
            st.angle_deg
        );
        assert_eq!(st.bend_deg, 0.0, "fresh frames store no bend");
    }

    #[test]
    fn segment_state_marks_the_root() {
        let core = core_with_selection(Some(0));
        let st = segment_state(&core).expect("the origin exists");
        assert!(st.is_root);
    }

    #[test]
    fn segment_topology_reads_the_demo_tree() {
        // The demo figure is `FigureDef::humanoid`
        // (model.rs): 0 hips is the root; 1 chest, 7 thigh
        // L and 9 thigh R hang off it; 2 head, 3 upper-arm
        // L and 5 upper-arm R hang off 1.
        let core = core_with_selection(Some(1));
        assert_eq!(segment_topology(&core), Some((1, Some(0), 3)));
        let core = core_with_selection(Some(3));
        assert_eq!(segment_topology(&core), Some((3, Some(1), 1)));
    }

    #[test]
    fn segment_topology_marks_the_root_and_leaves() {
        let core = core_with_selection(Some(0));
        assert_eq!(segment_topology(&core), Some((0, None, 3)));
        // Node 2 (head) is a leaf: a parent, no children.
        let core = core_with_selection(Some(2));
        assert_eq!(segment_topology(&core), Some((2, Some(1), 0)));
    }

    #[test]
    fn segment_topology_is_none_without_a_valid_selection() {
        assert!(segment_topology(&EditorCore::new()).is_none());
        assert!(segment_topology(&core_with_selection(None)).is_none());
        assert!(segment_topology(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn format_topology_names_root_parent_and_children() {
        assert_eq!(format_topology(0, None, 3), "Node 0 · root · 3 children");
        assert_eq!(
            format_topology(3, Some(1), 1),
            "Node 3 · parent 1 · 1 child"
        );
        assert_eq!(
            format_topology(2, Some(1), 0),
            "Node 2 · parent 1 · 0 children"
        );
    }

    #[test]
    fn shape_label_names_every_cycle_step() {
        assert_eq!(shape_label(&Shape::Line), "Line");
        assert_eq!(shape_label(&Shape::Circle), "Circle");
        assert_eq!(shape_label(&Shape::Polygon(3)), "Triangle");
        assert_eq!(shape_label(&Shape::Polygon(4)), "Square");
        assert_eq!(shape_label(&Shape::Polygon(5)), "Pentagon");
        assert_eq!(shape_label(&Shape::Polygon(6)), "Hexagon");
        assert_eq!(shape_label(&Shape::Polygon(9)), "Polygon 9");
    }

    #[test]
    fn palette_mirrors_the_core_cycle_colours() {
        // editor.rs's private NODE_COLORS, minus the leading
        // None (figure colour) that set_node_color cannot
        // write. If the core palette changes, this fails and
        // the swatches get updated with it.
        assert_eq!(
            PALETTE,
            [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830]
        );
        let mut sorted = PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), PALETTE.len(), "palette colours repeat");
    }

    #[test]
    fn norm_deg_360_wraps() {
        assert_eq!(norm_deg_360(0.0), 0.0);
        assert_eq!(norm_deg_360(360.0), 0.0);
        assert_eq!(norm_deg_360(-90.0), 270.0);
        assert_eq!(norm_deg_360(725.0), 5.0);
        assert!((norm_deg_360(-0.5) - 359.5).abs() < 1e-6);
    }

    #[test]
    fn format_deg_shows_one_decimal() {
        assert_eq!(format_deg(0.0), "0.0°");
        assert_eq!(format_deg(45.5), "45.5°");
        assert_eq!(format_deg(270.0), "270.0°");
        assert_eq!(format_deg(12.34), "12.3°");
        assert_eq!(format_deg(12.36), "12.4°");
    }

    #[test]
    fn rotate_selected_steps_the_pose_angle_exactly() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let before = core.project.frames[frame].poses[fi].angles[1];
        rotate_selected(&mut core, 5.0);
        assert_eq!(core.project.frames[frame].poses[fi].angles[1], before + 5.0);
        rotate_selected(&mut core, -1.0);
        rotate_selected(&mut core, -1.0);
        assert_eq!(core.project.frames[frame].poses[fi].angles[1], before + 3.0);
    }

    #[test]
    fn rotate_selected_is_one_undo_step_per_press() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let before = core.project.frames[frame].poses[fi].angles[1];
        let depth = core.undo_depth();
        rotate_selected(&mut core, 1.0);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        core.undo();
        assert_eq!(
            core.project.frames[frame].poses[fi].angles[1], before,
            "undo restores the pre-press angle"
        );
    }

    #[test]
    fn rotate_selected_without_selection_is_status_only() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        rotate_selected(&mut core, 5.0);
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn rotate_selected_moves_the_readout_by_the_same_step() {
        // The readout is the solved world direction in
        // screen space (atan2 over y-down positions, so it
        // grows clockwise on screen); pose angles turn the
        // other way (the engine's counter-clockwise
        // convention, the same one drag posing and graph
        // keys use). A local +5° step therefore moves the
        // readout by exactly 5° in the readout's own sense:
        // −5° (mod 360). The magnitude is what must match.
        let mut core = core_with_selection(Some(1));
        let before = segment_state(&core).unwrap().angle_deg;
        rotate_selected(&mut core, 5.0);
        let after = segment_state(&core).unwrap().angle_deg;
        let moved = norm_deg_360(after - before);
        assert!(
            (moved - 355.0).abs() < 0.01,
            "readout moved {moved}, want 355 (−5 in readout sense)"
        );
    }

    #[test]
    fn bend_selected_steps_the_stored_bend_exactly() {
        // The demo project's frames carry no bend table at
        // all; the first press must grow it (sparse storage)
        // and land exactly the delta, in radians.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        assert!(core.project.frames[frame].bends.is_empty());
        let step = (5.0f32).to_radians();
        bend_selected(&mut core, step);
        assert_eq!(core.project.frames[frame].bends[fi][1], step);
        // Neighbours grew to zero, not to the step.
        assert_eq!(core.project.frames[frame].bends[fi][0], 0.0);
        assert_eq!(core.project.frames[frame].bends[fi][2], 0.0);
        // Steps accumulate on the stored value.
        bend_selected(&mut core, (-1.0f32).to_radians());
        bend_selected(&mut core, (-1.0f32).to_radians());
        let got = core.project.frames[frame].bends[fi][1];
        assert!(
            (got - (3.0f32).to_radians()).abs() < 1e-6,
            "bend {got}, want 3° in radians"
        );
        // And the panel's readout sees the same value.
        let st = segment_state(&core).unwrap();
        assert!((st.bend_deg - 3.0).abs() < 0.01, "readout {}", st.bend_deg);
    }

    #[test]
    fn bend_selected_is_one_undo_step_per_press() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let depth = core.undo_depth();
        bend_selected(&mut core, (1.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        core.undo();
        assert!(
            core.project.bends_at(frame as f32, fi).is_empty(),
            "undo restores the pre-press (bend-free) frame"
        );
    }

    #[test]
    fn bend_selected_clamps_at_the_range_ends() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let nodes = core.project.figures[fi].def.nodes.len();
        core.project.frames[frame].bends = vec![vec![0.0; nodes]];
        // At the limit, a further step stays at the limit.
        core.project.frames[frame].bends[fi][1] = BEND_LIMIT_RAD;
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], BEND_LIMIT_RAD);
        core.project.frames[frame].bends[fi][1] = -BEND_LIMIT_RAD;
        bend_selected(&mut core, (-5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], -BEND_LIMIT_RAD);
        // Crossing the limit lands exactly on it.
        core.project.frames[frame].bends[fi][1] = BEND_LIMIT_RAD - 0.01;
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], BEND_LIMIT_RAD);
    }

    #[test]
    fn bend_selected_without_selection_is_status_only() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn bend_selected_refuses_the_root_snapshot_free() {
        // The origin has no parent segment to arc — the
        // same refusal cycle_bend makes (editor.rs:1135).
        let mut core = core_with_selection(Some(0));
        let depth = core.undo_depth();
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth, "no snapshot for the root");
        assert!(
            core.status.contains("origin has no segment"),
            "refusal explains itself: {}",
            core.status
        );
        assert!(core.project.frames[core.current].bends.is_empty());
    }

    #[test]
    fn bend_degrees_is_none_without_a_valid_selection() {
        assert!(bend_degrees(&EditorCore::new()).is_none());
        assert!(bend_degrees(&core_with_selection(None)).is_none());
        assert!(bend_degrees(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn bend_degrees_reads_zero_for_a_fresh_selection() {
        // Sparse storage: no bend table at all reads as 0.
        let core = core_with_selection(Some(1));
        assert_eq!(bend_degrees(&core), Some(0.0));
        // The origin too — it simply can never be bent.
        let core = core_with_selection(Some(0));
        assert_eq!(bend_degrees(&core), Some(0.0));
    }

    #[test]
    fn bend_degrees_converts_the_stored_radians() {
        let mut core = core_with_selection(Some(1));
        bend_selected(&mut core, 0.35);
        assert_eq!(bend_degrees(&core), Some(0.35f32.to_degrees()));
        bend_selected(&mut core, -0.7);
        assert_eq!(bend_degrees(&core), Some((-0.35f32).to_degrees()));
        // And it agrees with the panel's existing Bend
        // readout, which reads the same stored value.
        let st = segment_state(&core).unwrap();
        assert_eq!(bend_degrees(&core), Some(st.bend_deg));
    }

    #[test]
    fn format_bend_readout_shows_whole_degrees_with_sign() {
        assert_eq!(format_bend_readout(0.0), "Bend 0°");
        assert_eq!(format_bend_readout(-0.0), "Bend 0°");
        assert_eq!(format_bend_readout(20.053_523), "Bend 20°");
        assert_eq!(format_bend_readout(-20.053_523), "Bend -20°");
        assert_eq!(format_bend_readout(40.107_046), "Bend 40°");
    }

    #[test]
    fn straighten_zeroes_a_bent_segment_in_one_undo_step() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        bend_selected(&mut core, 0.35);
        let depth = core.undo_depth();
        straighten_selected(&mut core);
        assert_eq!(
            core.project.frames[frame].bends[fi][1], 0.0,
            "the stored bend is bit-exact zero"
        );
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the press");
        assert_eq!(bend_degrees(&core), Some(0.0));
        assert_eq!(core.status, "Segment 1 bend 0.0° on frame 1.");
        // Undo restores the bend.
        core.undo();
        assert_eq!(core.project.frames[frame].bends[fi][1], 0.35);
    }

    #[test]
    fn straighten_at_zero_is_a_silent_no_op() {
        // `bend_selected` snapshots unconditionally once
        // its guards pass, so the zero guard in
        // `straighten_selected` is what stands between a
        // press at bend 0 and a phantom undo step.
        let mut core = core_with_selection(Some(1));
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        straighten_selected(&mut core);
        assert_eq!(core.undo_depth(), depth, "no snapshot at bend 0");
        assert_eq!(core.status, "Sentinel.", "no status churn at bend 0");
        assert!(
            core.project.frames[core.current].bends.is_empty(),
            "the bend table is not even grown"
        );
    }

    #[test]
    fn straighten_without_selection_refuses_via_bend_selected() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        straighten_selected(&mut core);
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn phase_h_glyphs_exist_in_the_icon_set() {
        // The header, Cycle bend and Delete segment glyphs
        // must name real icons — `has_icon` false would
        // silently drop the header/Delete glyphs (and the
        // menus' convention is to pin this in a test).
        for name in ["shapes", "repeat", "trash-can"] {
            assert!(
                crate::rune_sec::icons::has_icon(name),
                "glyph {name} not in the icon set"
            );
        }
    }

    #[test]
    fn panel_builds_with_and_without_selection() {
        let t = rune::tok::theme(true);
        let bare = State {
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
        let _ = inspector_segment(&bare, &t);
        let selected = State {
            core: core_with_selection(Some(1)),
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
        let _ = inspector_segment(&selected, &t);
        let root = State {
            core: core_with_selection(Some(0)),
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
        let _ = inspector_segment(&root, &t);
    }

    /// A minimal node definition for the navigation
    /// fixtures: only `parent` varies (the field set
    /// mirrors the model constructors, model.rs:181).
    fn nav_node(parent: Option<usize>) -> NodeDef {
        NodeDef {
            name: "n".into(),
            parent,
            length: 10.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: stykanimate_rs::model::NodeStyle::default(),
            media: None,
        }
    }

    #[test]
    fn parent_of_walks_a_chain_and_stops_at_the_root() {
        // A straight chain 0 <- 1 <- 2 <- 3.
        let nodes = vec![
            nav_node(None),
            nav_node(Some(0)),
            nav_node(Some(1)),
            nav_node(Some(2)),
        ];
        assert_eq!(parent_of(&nodes, 3), Some(2));
        assert_eq!(parent_of(&nodes, 2), Some(1));
        assert_eq!(parent_of(&nodes, 1), Some(0));
        assert_eq!(parent_of(&nodes, 0), None, "the root has no parent");
        assert_eq!(parent_of(&nodes, 99), None, "out of range reads as none");
        assert_eq!(first_child_of(&nodes, 0), Some(1));
        assert_eq!(first_child_of(&nodes, 2), Some(3));
        assert_eq!(first_child_of(&nodes, 3), None, "the chain's end is a leaf");
        assert_eq!(first_child_of(&nodes, 99), None);
    }

    #[test]
    fn first_child_of_picks_the_lowest_index_child() {
        // A branching fixture: the root has three
        // children (1, 2, 3); node 2 has one child (4).
        let nodes = vec![
            nav_node(None),
            nav_node(Some(0)),
            nav_node(Some(0)),
            nav_node(Some(0)),
            nav_node(Some(2)),
        ];
        assert_eq!(
            first_child_of(&nodes, 0),
            Some(1),
            "first means lowest index, not any child"
        );
        assert_eq!(first_child_of(&nodes, 2), Some(4));
        assert_eq!(first_child_of(&nodes, 1), None, "a leaf has no child");
        assert_eq!(first_child_of(&nodes, 3), None);
        assert_eq!(parent_of(&nodes, 4), Some(2));
    }

    #[test]
    fn select_parent_moves_up_and_stops_at_the_root() {
        // The demo humanoid (model.rs humanoid): node 4
        // (forearm L) hangs off 3 (upper-arm L), off 1
        // (chest), off 0 (hips, the root).
        let mut core = core_with_selection(Some(4));
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(3));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(1));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(0));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(0), "the root press is a no-op");
        // Selection is not an undoable edit: no snapshot
        // was taken and no status was set, across all
        // four presses.
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);
    }

    #[test]
    fn select_first_child_moves_down_and_stops_at_a_leaf() {
        // Node 0's children are 1, 7 and 9 — the first
        // is 1 (chest); node 1's children are 2, 3 and
        // 5 — the first is 2 (head), a leaf.
        let mut core = core_with_selection(Some(0));
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(1));
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(2));
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(2), "the leaf press is a no-op");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);
    }

    #[test]
    fn navigation_without_a_valid_selection_changes_nothing() {
        let mut core = core_with_selection(None);
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_parent(&mut core);
        select_first_child(&mut core);
        assert_eq!(core.selected_node, None);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);

        // A stale index navigates nowhere either (the
        // panel is not even rendered for it — this pins
        // the handler's own discipline).
        let mut stale = core_with_selection(Some(999));
        select_parent(&mut stale);
        select_first_child(&mut stale);
        assert_eq!(stale.selected_node, Some(999));
    }
}
