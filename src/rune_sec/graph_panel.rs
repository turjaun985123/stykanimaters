//! Graph panel for the Rune editor shell (Phase D, section 9
//! of the GPUI→Rune migration): the channel selector, the
//! curve canvas, the interpolation segmented control, the
//! tangent-preset picker and the channel transform actions.
//!
//! Declared from `main.rs` as `crate::rune_sec::graph_panel`;
//! bin-crate code, not part of the engine library. The public
//! entry point is [`graph_panel`].
//!
//! ## Where the data and the math live
//!
//! Channels are project data (`core.project.channels`); the
//! curve drawn here is sampled through the REAL evaluation —
//! `graph::Channel::value_at`, the cubic-Hermite / Linear /
//! Step evaluator the renderer itself uses — mapped to panel
//! pixels by the engine's own view-model
//! (`graph_panel::GraphView::auto_fit` + `curve_points`, and
//! `graph_panel::preset_preview` for the preset chips). This
//! module adds no evaluation math of its own.
//!
//! ## Where the edits go
//!
//! Every control calls a real `EditorCore` graph op
//! (`key_graph_angle`, `set_graph_key_interp`,
//! `apply_graph_ease_preset`, `smooth_graph_tangents`,
//! `bake_graph_channels`, `clear_graph`). Three actions have
//! NO dedicated core op — mirror-in-time, scale-values and
//! reduce-keys exist only as `graph::` free functions — so
//! their handlers follow the exact precedent the MCP tools
//! set (`mcp_tools::tool_mirror_channel_time` /
//! `tool_scale_channel_values`): `core.snapshot()` for undo,
//! apply the `graph::` function to the selected channel,
//! restore key order, and write the outcome to `core.status`.
//! Those handlers are factored as `apply_*(&mut EditorCore,
//! ..)` so they are testable without a `State`.
//!
//! ## Canvas pointer interaction
//!
//! The curve canvas also takes drags (`graph_drag`, factored
//! into the testable `drag_down` / `drag_move` / `drag_up`
//! over a bare `EditorCore` + [`GraphPanelState`]):
//!
//! - **Down on a key handle** (within [`GRAB_RADIUS`] px,
//!   hit-tested by `GraphView::hit_key`): one
//!   `EditorCore::snapshot()` — the only snapshot of the
//!   gesture — and the key is grabbed.
//! - **Move**: the pointer converts back to data through the
//!   SAME fit the drawing used (see [`drag_view`]), the key's
//!   frame is clamped to `>= 0.0`, its value is unclamped,
//!   and the channel is re-sorted after every move
//!   (`drag_key_to`), so `value_at` — and the graph-aware
//!   posing that reads these channels — stays valid on every
//!   redraw, even mid-drag. Angle channels need no extra
//!   work: the stage's `positions_f` evaluates channels
//!   itself.
//! - **Up**: the final key position is written to
//!   `core.status`. A gesture the runtime reports as
//!   cancelled (Escape / focus loss) instead restores the
//!   pre-drag project with one `core.undo()` — the Down
//!   snapshot is exactly one undo step.
//! - **A click that hits no key** (Down+Up within
//!   [`CLICK_TRAVEL`] px) seeks: `core.goto` of the frame
//!   under the pointer, rounded (`seek_frame`). A longer
//!   drag across empty canvas does nothing.
//!
//! The handlers recompute the fit per phase from the drag's
//! own `w`/`h` — the size the element was actually drawn at
//! for this gesture — via the same [`drag_view`] the builder
//! draws with, so hit math can never disagree with the
//! drawing. One honest consequence: `auto_fit` derives its
//! value range from the channel's keys, so dragging the
//! extreme key of a non-angle channel re-pads the fit
//! slightly as it moves (the key trails the pointer by the
//! padding factor); angle channels are immune (their fit is
//! always the full -180..=180 circle), and the frame range
//! only stretches when a key is dragged past the current
//! end. The drawing and the drag always share the fit, so
//! they never disagree with each other.
//!
//! ## Scoping, honestly
//!
//! The core's graph ops scope by SELECTION, not by this
//! panel's drawn channel: presets apply to every channel of
//! the selected figure, and `set_graph_key_interp` always
//! targets the selected figure's `Angle(selected_node)`
//! channel's key nearest the current frame (see that op's own
//! docs). The panel's channel selector therefore drives the
//! DRAWING and the mirror/scale/reduce actions, while the
//! interp segmented control is labelled by the channel it
//! really edits. Nothing here pretends otherwise.
//!
//! ## State the shell must provide
//!
//! `State` needs one field (integrator wires it):
//! `pub graph_panel: crate::rune_sec::graph_panel::GraphPanelState`
//! — see [`GraphPanelState`].

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, row, text, Color, Cx, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Interp, TangentPreset};
use stykanimate_rs::graph_panel::{preset_preview, GraphView};
use stykanimate_rs::model::V2;

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Height of the curve canvas, in points.
pub const GRAPH_H: f32 = 170.0;
/// Canvas size assumed before the first committed layout
/// exists (the `Cx::layout_size` read settles after one frame,
/// exactly as the stage's does).
const FALLBACK_SIZE: [f32; 2] = [720.0, GRAPH_H];
/// Key-reduction tolerance, in channel value units (degrees
/// for angle channels, stage units for root/length): interior
/// keys whose removal moves the curve by at most this much are
/// dropped. Deliberately conservative — it removes
/// near-duplicates, not shape.
pub const REDUCE_TOLERANCE: f32 = 0.5;
/// Tangent-preset preview chip size, in points.
const PREVIEW_W: f32 = 52.0;
const PREVIEW_H: f32 = 30.0;
/// Grab radius for key handles on the curve canvas, in
/// points. The handles DRAW at radius 4.5; the pick radius is
/// deliberately more forgiving.
pub const GRAB_RADIUS: f32 = 10.0;
/// Pointer travel, in points, under which a Down+Up on empty
/// canvas counts as a click (and seeks) rather than a drag.
/// Matches the 4 pt click-run threshold Rune itself uses for
/// click counting (see the `Drag::clicks` docs).
pub const CLICK_TRAVEL: f32 = 4.0;

/// Panel-local view state: which channel the user picked in
/// the selector, plus the in-progress canvas key drag.
/// Everything else the panel shows is derived from
/// `EditorCore` per frame.
#[derive(Clone, Debug, Default)]
pub struct GraphPanelState {
    /// The last selector pick. `None` — and any pick that no
    /// longer names a selector entry (figure switched, node
    /// deselected) — falls back to [`effective_target`]'s
    /// automatic choice, so the panel never draws a channel
    /// the selector doesn't offer.
    pub target: Option<ChannelTarget>,
    /// Index into the drawn channel's `keys` of the key an
    /// in-progress canvas drag grabbed at Down. `None`
    /// outside a key drag.
    ///
    /// STALE-INDEX RISK, and the mitigation: `drag_key_to`
    /// re-sorts the channel after every Move, so the grabbed
    /// key's index can change mid-drag (dragging a key past
    /// its neighbour permutes both). The index is therefore
    /// never trusted across a Move on its own: it is only a
    /// hint, re-resolved on every Move by
    /// [`resolve_drag_index`] against [`Self::drag_anchor`]
    /// (the grabbed key's frame), and refreshed to the key's
    /// post-sort index after each move. Two keys sitting at
    /// the EXACT same frame and value are interchangeable to
    /// the resolver — degenerate duplicate data; the curve
    /// they produce is identical either way, though a later
    /// move could in principle continue with the twin (they
    /// may carry different tangents).
    pub drag_key: Option<usize>,
    /// The grabbed key's frame as of Down / the last Move —
    /// the drag's identity anchor for [`resolve_drag_index`].
    /// Meaningless while `drag_key` is `None`.
    pub drag_anchor: f32,
}

/// Human label for a channel target, in UI casing: "Root X",
/// "Root Y", "Angle 3", "Length 3". (The engine view-model's
/// own legend labels are lower-case and private; the panel
/// owns its casing.)
pub fn channel_label(target: ChannelTarget) -> String {
    match target {
        ChannelTarget::RootX => "Root X".to_string(),
        ChannelTarget::RootY => "Root Y".to_string(),
        ChannelTarget::Angle(n) => format!("Angle {n}"),
        ChannelTarget::Length(n) => format!("Length {n}"),
    }
}

/// The selector's entries for the selected figure, in display
/// order: Root X and Root Y always; the selected node's Angle
/// when a node is selected; then every other target the
/// figure actually has a channel for, in project order,
/// without duplicates. Root/length channels appear even
/// before they have keys only via the pinned entries — keying
/// a root channel has no core op (see the module docs), so an
/// unpinned target is offered only once a channel exists.
pub fn selector_entries(core: &EditorCore) -> Vec<ChannelTarget> {
    let mut out = vec![ChannelTarget::RootX, ChannelTarget::RootY];
    if let Some(ni) = core.selected_node {
        out.push(ChannelTarget::Angle(ni));
    }
    for ch in &core.project.channels {
        if ch.figure == core.selected_figure && !out.contains(&ch.target) {
            out.push(ch.target);
        }
    }
    out
}

/// The channel the panel draws and the transform actions
/// edit: the user's pick when it is still a selector entry;
/// otherwise the figure's first real channel; otherwise the
/// selected node's Angle (the channel `key_graph_angle`
/// authors, so the empty state points at a channel the user
/// can create from this panel); otherwise Root X.
pub fn effective_target(core: &EditorCore, sel: Option<ChannelTarget>) -> ChannelTarget {
    if let Some(t) = sel {
        if selector_entries(core).contains(&t) {
            return t;
        }
    }
    if let Some(ch) = core
        .project
        .channels
        .iter()
        .find(|ch| ch.figure == core.selected_figure)
    {
        return ch.target;
    }
    if let Some(ni) = core.selected_node {
        return ChannelTarget::Angle(ni);
    }
    ChannelTarget::RootX
}

/// Index into `core.project.channels` of the SELECTED
/// FIGURE's channel for `target`, if one exists.
pub fn channel_index(core: &EditorCore, target: ChannelTarget) -> Option<usize> {
    core.project
        .channels
        .iter()
        .position(|ch| ch.figure == core.selected_figure && ch.target == target)
}

/// The selected figure's channel for `target`, if one exists.
pub fn channel_for<'a>(core: &'a EditorCore, target: ChannelTarget) -> Option<&'a Channel> {
    channel_index(core, target).map(|i| &core.project.channels[i])
}

/// Index of the key nearest `frame` — the same pick
/// `EditorCore::set_graph_key_interp` makes (an exact hit
/// wins; ties go to the EARLIER key, because only a strictly
/// smaller distance replaces the incumbent).
pub fn nearest_key(ch: &Channel, frame: f32) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_dist = f32::INFINITY;
    for (i, k) in ch.keys.iter().enumerate() {
        let d = (k.frame - frame).abs();
        if d < best_dist {
            best_dist = d;
            best = Some(i);
        }
    }
    best
}

/// The interp mode the segmented control should show as
/// active: the mode of the key nearest the current frame on
/// the selected figure's `Angle(selected_node)` channel —
/// exactly the key `set_graph_key_interp` would edit. `None`
/// (nothing highlighted) when there is no node, no channel or
/// no key.
pub fn current_interp(core: &EditorCore) -> Option<Interp> {
    let ni = core.selected_node?;
    let ch = channel_for(core, ChannelTarget::Angle(ni))?;
    let ki = nearest_key(ch, core.current as f32)?;
    Some(ch.keys[ki].interp)
}

/// The drawn curve as pixel points: `steps + 1` samples of
/// the REAL evaluation (`Channel::value_at` via
/// `GraphView::curve_points`) across the view's frame range.
/// Kept separate from the `Shape` assembly so tests can
/// assert on plain data.
pub fn sample_curve(view: &GraphView, ch: &Channel, steps: usize) -> Vec<V2> {
    view.curve_points(ch, steps)
}

/// A key diamond: four points (top, right, bottom, left) of
/// radius `r` around `centre`, ready for `Shape::fill`.
pub fn diamond(centre: V2, r: f32) -> [V2; 4] {
    [
        [centre[0], centre[1] - r],
        [centre[0] + r, centre[1]],
        [centre[0], centre[1] + r],
        [centre[0] - r, centre[1]],
    ]
}

/// A tangent preset's preview curve as pixel points in a
/// `w` x `h` box: `preset_preview`'s normalised 0.0..=1.0
/// samples with y flipped (value 1.0 at the TOP, screen
/// convention), x spread evenly edge to edge.
pub fn preview_points(preset: TangentPreset, w: f32, h: f32, samples: usize) -> Vec<V2> {
    let values = preset_preview(preset, samples);
    let n = values.len();
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let fx = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            [fx * w, h * (1.0 - v)]
        })
        .collect()
}

/// Mirror the `(selected figure, target)` channel in time
/// around the timeline's middle frame — the MCP
/// `graph_mirror_channel_time` behaviour, default centre
/// included. No such channel: status only, no snapshot.
pub fn apply_mirror(core: &mut EditorCore, target: ChannelTarget) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to mirror.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    let centre = (core.project.frames.len().saturating_sub(1)) as f32 / 2.0;
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    stykanimate_rs::graph::mirror_keys_time(&mut ch.keys, centre);
    // Mirroring reverses the key order; restore ascending
    // order (the MCP tool does the same after the call).
    ch.sort_keys();
    core.status = format!("Mirrored graph channel time around frame {centre}.");
}

/// Scale the `(selected figure, target)` channel's values by
/// `factor` around 0 — the MCP `graph_scale_channel_values`
/// behaviour with its default pivot. No such channel: status
/// only, no snapshot.
pub fn apply_scale(core: &mut EditorCore, target: ChannelTarget, factor: f32) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to scale.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    stykanimate_rs::graph::scale_key_values(&mut ch.keys, factor, 0.0);
    core.status = format!("Scaled graph channel values by {factor} around 0.");
}

/// Reduce the `(selected figure, target)` channel with
/// [`REDUCE_TOLERANCE`] (`graph::reduce_keys` drops interior
/// keys that contribute at most the tolerance and always
/// keeps the endpoints). No such channel: status only, no
/// snapshot.
pub fn apply_reduce(core: &mut EditorCore, target: ChannelTarget) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to reduce.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    core.snapshot();
    let removed = stykanimate_rs::graph::reduce_keys(
        &mut core.project.channels[idx],
        REDUCE_TOLERANCE,
    );
    core.status = format!("Reduced graph channel: removed {removed} key(s).");
}

/// The fit the panel draws AND drags with, for a canvas of
/// `size`: `GraphView::auto_fit` over the drawn channel alone
/// (an angle channel gets the full -180..=180 circle, a value
/// channel its keys with 10% padding; no channel fits the
/// bare timeline). [`graph_shape`] builds its view through
/// here and so does every drag phase, recomputed per phase
/// from the then-current project — pointer math and pixels
/// can never be based on different transforms.
pub fn drag_view(core: &EditorCore, target: ChannelTarget, size: [f32; 2]) -> GraphView {
    let fit: &[Channel] = match channel_for(core, target) {
        Some(c) => std::slice::from_ref(c),
        None => &[],
    };
    GraphView::auto_fit(fit, core.frame_count(), size[0], size[1])
}

/// Resolve a drag's grabbed key under the channel's CURRENT
/// key order. `hint` is the index recorded at Down / the last
/// Move and `anchor` the grabbed key's frame then. The hint
/// wins when the key sitting there still has the anchor
/// frame (the common no-crossing case); otherwise the key
/// nearest the anchor is the grabbed one (a re-sort moved
/// it). `None` when the channel has no keys at all.
pub fn resolve_drag_index(ch: &Channel, hint: usize, anchor: f32) -> Option<usize> {
    if let Some(k) = ch.keys.get(hint) {
        if (k.frame - anchor).abs() <= 1e-4 {
            return Some(hint);
        }
    }
    nearest_key(ch, anchor)
}

/// Move key `idx` of `ch` to (`frame`, `value`) — frame
/// clamped to `>= 0.0`, value unclamped — and re-sort the
/// channel. Returns the key's NEW index after the sort (the
/// sort can permute it past a crossed neighbour), re-found
/// by its exact frame+value with a nearest-frame fallback,
/// or `None` when `idx` is out of range (channel untouched).
pub fn drag_key_to(ch: &mut Channel, idx: usize, frame: f32, value: f32) -> Option<usize> {
    let k = ch.keys.get_mut(idx)?;
    k.frame = frame.max(0.0);
    k.value = value;
    let (f, v) = (k.frame, k.value);
    ch.sort_keys();
    ch.keys
        .iter()
        .position(|k| k.frame == f && k.value == v)
        .or_else(|| nearest_key(ch, f))
}

/// The frame a click at pixel `x` seeks to: the view's data
/// frame at `x`, rounded to the nearest whole frame (halves
/// round away from zero, `f32::round` semantics) and clamped
/// into the timeline. `None` when there are no frames.
pub fn seek_frame(view: &GraphView, x: f32, frame_count: usize) -> Option<usize> {
    if frame_count == 0 {
        return None;
    }
    let (frame, _) = view.to_data([x, 0.0]);
    let f = frame.round().clamp(0.0, (frame_count - 1) as f32);
    Some(f as usize)
}

/// Canvas drag, Down phase: hit-test the drawn channel's key
/// handles at `px` (a canvas of `size`) through the panel's
/// own fit. On a hit: snapshot ONCE for the whole gesture,
/// grab the key into `st`, return true. On a miss nothing is
/// recorded — whether the gesture becomes a seek is decided
/// at Up from its travel.
pub fn drag_down(core: &mut EditorCore, st: &mut GraphPanelState, px: V2, size: [f32; 2]) -> bool {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return false;
    }
    let target = effective_target(core, st.target);
    let Some(ci) = channel_index(core, target) else {
        return false;
    };
    let view = drag_view(core, target, size);
    let hit = view.hit_key(&core.project.channels[ci], px, GRAB_RADIUS);
    match hit {
        Some(i) => {
            core.snapshot();
            st.drag_anchor = core.project.channels[ci].keys[i].frame;
            st.drag_key = Some(i);
            true
        }
        None => false,
    }
}

/// Canvas drag, Move phase: while a key is grabbed, move it
/// to the pointer's data position (converted through the fit
/// as of this Move's start) and re-anchor the drag state to
/// its post-sort index. No snapshot — the Down snapshot
/// covers the whole gesture. Returns true when a key moved;
/// a stale grab (channel gone, key gone) ends the drag.
pub fn drag_move(core: &mut EditorCore, st: &mut GraphPanelState, px: V2, size: [f32; 2]) -> bool {
    let Some(hint) = st.drag_key else {
        return false;
    };
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return false;
    }
    let target = effective_target(core, st.target);
    let Some(ci) = channel_index(core, target) else {
        st.drag_key = None;
        return false;
    };
    let idx = resolve_drag_index(&core.project.channels[ci], hint, st.drag_anchor);
    let Some(idx) = idx else {
        st.drag_key = None;
        return false;
    };
    let view = drag_view(core, target, size);
    let (frame, value) = view.to_data(px);
    let ch = &mut core.project.channels[ci];
    match drag_key_to(ch, idx, frame, value) {
        Some(new_idx) => {
            st.drag_key = Some(new_idx);
            st.drag_anchor = ch.keys[new_idx].frame;
            true
        }
        None => {
            st.drag_key = None;
            false
        }
    }
}

/// Canvas drag, Up phase. With a key grabbed: end the drag —
/// the final position goes to `core.status`, or, when the
/// runtime cancelled the gesture (`cancelled`), the Down
/// snapshot is rolled back with one `core.undo()`. With no
/// key grabbed: a click (travel within [`CLICK_TRAVEL`])
/// seeks the playhead to the frame under the pointer via
/// `core.goto`; a longer drag over empty canvas does
/// nothing. Returns true when the gesture changed anything.
pub fn drag_up(
    core: &mut EditorCore,
    st: &mut GraphPanelState,
    px: V2,
    size: [f32; 2],
    travel: f32,
    cancelled: bool,
) -> bool {
    if let Some(hint) = st.drag_key.take() {
        if cancelled {
            core.undo();
            return true;
        }
        let target = effective_target(core, st.target);
        if let Some(ci) = channel_index(core, target) {
            let ch = &core.project.channels[ci];
            if let Some(idx) = resolve_drag_index(ch, hint, st.drag_anchor) {
                let k = ch.keys[idx];
                core.status = format!(
                    "Graph key moved: frame {:.1}, value {:.1}.",
                    k.frame, k.value
                );
            }
        }
        return true;
    }
    if cancelled || travel > CLICK_TRAVEL || size[0] <= 0.0 {
        return false;
    }
    let target = effective_target(core, st.target);
    let view = drag_view(core, target, size);
    match seek_frame(&view, px[0], core.frame_count()) {
        Some(f) => {
            core.goto(f);
            true
        }
        None => false,
    }
}

/// The canvas element's drag handler: element-local points
/// and the element's own drawn size go straight to the
/// phase helpers (pointer capture is Rune's — a drag that
/// leaves the canvas still reports its Moves and Up here).
fn graph_drag(s: &mut State, d: &mut Drag) {
    let px = [d.x, d.y];
    let size = [d.w, d.h];
    match d.phase {
        Phase::Down => {
            drag_down(&mut s.core, &mut s.graph_panel, px, size);
        }
        Phase::Move => {
            drag_move(&mut s.core, &mut s.graph_panel, px, size);
        }
        Phase::Up => {
            let travel =
                ((d.x - d.start_x).powi(2) + (d.y - d.start_y).powi(2)).sqrt();
            drag_up(
                &mut s.core,
                &mut s.graph_panel,
                px,
                size,
                travel,
                d.cancelled(),
            );
        }
    }
}

/// The curve canvas as one `Shape`: the zero line (when the
/// fitted range crosses zero), the current-frame line, the
/// sampled curve, a diamond per key, and an accent diamond on
/// the curve at the current frame. The view is
/// `GraphView::auto_fit` over the drawn channel alone, so an
/// angle channel gets the full -180..=180 circle and a value
/// channel gets its keys with 10% padding. `drag` is the
/// grabbed key's index (resolved for the CURRENT key order),
/// drawn larger in the accent colour while it is dragged.
fn graph_shape(
    core: &EditorCore,
    target: ChannelTarget,
    size: [f32; 2],
    t: &Theme,
    drag: Option<usize>,
) -> Shape {
    let (w, h) = (size[0], size[1]);
    let accent = tok::accent(0, true);
    let ch = channel_for(core, target);
    let view = drag_view(core, target, size);
    let mut shape = Shape::new();
    if view.val_min < 0.0 && view.val_max > 0.0 {
        let y = view.to_px(0.0, 0.0)[1];
        shape = shape.line([0.0, y], [w, y], 1.0, LineCap::Round, t.text.alpha(0.18));
    }
    let now_x = view.to_px(core.current as f32, 0.0)[0];
    shape = shape.line(
        [now_x, 0.0],
        [now_x, h],
        1.0,
        LineCap::Round,
        accent.alpha(0.55),
    );
    if let Some(ch) = ch {
        let steps = (w as usize).clamp(48, 256);
        let pts = sample_curve(&view, ch, steps);
        if pts.len() > 1 {
            shape = shape.stroke(pts, 2.0, accent);
        }
        let amber = hex_color(0xFFC65D).unwrap_or(Color::WHITE);
        for (i, handle) in view.key_handles(ch).into_iter().enumerate() {
            shape = if drag == Some(i) {
                shape.fill(diamond(handle, 6.5).to_vec(), accent)
            } else {
                shape.fill(diamond(handle, 4.5).to_vec(), amber)
            };
        }
        let now_pt = view.to_px(core.current as f32, ch.value_at(core.current as f32));
        shape = shape.fill(diamond(now_pt, 3.5).to_vec(), accent);
    }
    shape
}

/// One tangent-preset chip: the preset's real preview curve
/// (a tiny canvas) over its name; pressing applies the preset
/// to the selected figure's channels
/// (`EditorCore::apply_graph_ease_preset`). Presets are
/// actions, not state, so no chip highlights as "active".
fn preset_chip(preset: TangentPreset, label: &str, key: &str, t: &Theme) -> El {
    let shape = Shape::new().stroke(
        preview_points(preset, PREVIEW_W, PREVIEW_H, 24),
        1.5,
        t.text,
    );
    col()
        .key(key)
        .padding(4.0)
        .gap(2.0)
        .radius(6.0)
        .fill(t.card)
        .hover(t.control_track)
        .press(t.control_off)
        .center()
        .on_press(move |s: &mut State| s.core.apply_graph_ease_preset(preset))
        .child(canvas(shape).size(PREVIEW_W, PREVIEW_H))
        .child(text(label).font(type_::LABEL).color(t.text))
}

/// The Graph panel: header + readout, channel selector,
/// curve canvas, interp segmented control, tangent presets
/// and the transform actions. Mounted by the shell wherever
/// the Graph workspace lives.
pub fn graph_panel(s: &State, cx: &mut Cx<State>, t: &Theme) -> El {
    let core = &s.core;
    let target = effective_target(core, s.graph_panel.target);
    let ch = channel_for(core, target);
    let size = cx.layout_size("graph-canvas").unwrap_or(FALLBACK_SIZE);

    let readout = match ch {
        Some(ch) => format!(
            "{} · {} keys · frame {} = {:.1}",
            channel_label(target),
            ch.keys.len(),
            core.current + 1,
            ch.value_at(core.current as f32)
        ),
        None => format!("{} · no keys yet", channel_label(target)),
    };

    // Channel selector: one chip per entry; the drawn channel
    // is the active one.
    let mut selector = row().key("graph-channels").gap(4.0);
    for entry in selector_entries(core) {
        let label = channel_label(entry);
        let key = format!("graph-ch-{}", label.replace(' ', "-").to_lowercase());
        selector = selector.child(btn(
            &key,
            &label,
            entry == target,
            t,
            move |s: &mut State| {
                s.graph_panel.target = Some(entry);
            },
        ));
    }

    // Interp segmented control. The op behind it edits the
    // Angle(selected node) channel's key nearest the current
    // frame, so the row is labelled with exactly that and its
    // active state is read from exactly that key.
    let interp_now = current_interp(core);
    let interp_label = match core.selected_node {
        Some(ni) => format!("Interp · angle {ni} key"),
        None => "Interp · no joint selected".to_string(),
    };
    let interp_row = row()
        .key("graph-interp")
        .gap(4.0)
        .center_y()
        .child(text(interp_label).font(type_::LABEL).color(t.text_secondary))
        .child(btn("graph-interp-hermite", "Hermite", interp_now == Some(Interp::Hermite), t, |s| {
            s.core.set_graph_key_interp(Interp::Hermite)
        }))
        .child(btn("graph-interp-linear", "Linear", interp_now == Some(Interp::Linear), t, |s| {
            s.core.set_graph_key_interp(Interp::Linear)
        }))
        .child(btn("graph-interp-step", "Step", interp_now == Some(Interp::Step), t, |s| {
            s.core.set_graph_key_interp(Interp::Step)
        }));

    let presets = row()
        .key("graph-presets")
        .gap(4.0)
        .child(preset_chip(TangentPreset::Smooth, "Smooth", "graph-preset-smooth", t))
        .child(preset_chip(TangentPreset::EaseIn, "Ease in", "graph-preset-ease-in", t))
        .child(preset_chip(TangentPreset::EaseOut, "Ease out", "graph-preset-ease-out", t))
        .child(preset_chip(TangentPreset::EaseInOut, "Ease in-out", "graph-preset-ease-in-out", t))
        .child(preset_chip(TangentPreset::Linear, "Linear", "graph-preset-linear", t));

    let actions = row()
        .key("graph-actions")
        .gap(4.0)
        .child(btn("graph-key", "Key angle", false, t, |s| s.core.key_graph_angle()))
        .child(btn("graph-smooth", "Auto tangents", false, t, |s| {
            s.core.smooth_graph_tangents()
        }))
        .child(btn("graph-mirror", "Mirror time", false, t, |s| {
            let target = effective_target(&s.core, s.graph_panel.target);
            apply_mirror(&mut s.core, target);
        }))
        .child(btn("graph-scale-up", "Scale ×2", false, t, |s| {
            let target = effective_target(&s.core, s.graph_panel.target);
            apply_scale(&mut s.core, target, 2.0);
        }))
        .child(btn("graph-scale-down", "Scale ÷2", false, t, |s| {
            let target = effective_target(&s.core, s.graph_panel.target);
            apply_scale(&mut s.core, target, 0.5);
        }))
        .child(btn("graph-reduce", "Reduce keys", false, t, |s| {
            let target = effective_target(&s.core, s.graph_panel.target);
            apply_reduce(&mut s.core, target);
        }))
        .child(btn("graph-bake", "Bake", false, t, |s| {
            let last = s.core.frame_count().saturating_sub(1);
            s.core.bake_graph_channels(0, last);
        }))
        .child(btn("graph-clear", "Clear", false, t, |s| s.core.clear_graph()));

    // The grabbed key, resolved against the CURRENT key
    // order for the highlight (the stored index is only a
    // hint once re-sorts start permuting it — see
    // GraphPanelState::drag_key).
    let drag_idx = s.graph_panel.drag_key.and_then(|hint| {
        let ch = channel_for(core, target)?;
        resolve_drag_index(ch, hint, s.graph_panel.drag_anchor)
    });

    let mut panel = col()
        .key("graph-panel")
        .w_full()
        .gap(8.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(format!("Graph — figure {}", core.selected_figure + 1))
                        .font(type_::TITLE)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(text(readout).font(type_::LABEL).color(t.text_secondary)),
        )
        .child(selector.scroll_x().w_full())
        .child(
            canvas(graph_shape(core, target, size, t, drag_idx))
                .key("graph-canvas")
                .w_full()
                .h(GRAPH_H)
                .fill(t.card)
                .radius(8.0)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| graph_drag(s, d)),
        );
    if ch.is_none() {
        panel = panel.child(
            text("No graph keys on this channel yet — select a joint on the stage, then Key angle.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    panel
        .child(interp_row)
        .child(presets)
        .child(actions.scroll_x().w_full())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::Key;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn channel(target: ChannelTarget, keys: Vec<Key>) -> Channel {
        Channel {
            figure: 0,
            target,
            keys,
        }
    }

    #[test]
    fn labels_name_every_target() {
        assert_eq!(channel_label(ChannelTarget::RootX), "Root X");
        assert_eq!(channel_label(ChannelTarget::RootY), "Root Y");
        assert_eq!(channel_label(ChannelTarget::Angle(3)), "Angle 3");
        assert_eq!(channel_label(ChannelTarget::Length(12)), "Length 12");
    }

    #[test]
    fn selector_pins_roots_then_node_angle_then_figure_channels() {
        let mut core = EditorCore::new();
        // Demo core: no node selected, no channels.
        assert_eq!(
            selector_entries(&core),
            vec![ChannelTarget::RootX, ChannelTarget::RootY]
        );
        core.selected_node = Some(2);
        assert_eq!(
            selector_entries(&core),
            vec![
                ChannelTarget::RootX,
                ChannelTarget::RootY,
                ChannelTarget::Angle(2)
            ]
        );
        // The figure's own channels join in project order,
        // without duplicating the pinned entries; another
        // figure's channels do not appear.
        core.project.channels = vec![
            channel(ChannelTarget::Angle(5), vec![Key::flat(0.0, 0.0)]),
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 0.0)]),
            Channel {
                figure: 1,
                target: ChannelTarget::Length(1),
                keys: vec![Key::flat(0.0, 0.0)],
            },
        ];
        assert_eq!(
            selector_entries(&core),
            vec![
                ChannelTarget::RootX,
                ChannelTarget::RootY,
                ChannelTarget::Angle(2),
                ChannelTarget::Angle(5)
            ]
        );
    }

    #[test]
    fn effective_target_fallback_order() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        // No channels, no pick: the node angle (the channel
        // Key angle would author).
        assert_eq!(effective_target(&core, None), ChannelTarget::Angle(1));
        // A valid pick wins.
        assert_eq!(
            effective_target(&core, Some(ChannelTarget::RootY)),
            ChannelTarget::RootY
        );
        // A stale pick (not a selector entry) falls through to
        // the figure's first real channel once one exists.
        core.project.channels = vec![channel(
            ChannelTarget::RootY,
            vec![Key::flat(0.0, 5.0)],
        )];
        assert_eq!(
            effective_target(&core, Some(ChannelTarget::Length(9))),
            ChannelTarget::RootY
        );
        assert_eq!(effective_target(&core, None), ChannelTarget::RootY);
        // Nothing at all: Root X.
        let bare = EditorCore::new();
        assert_eq!(effective_target(&bare, None), ChannelTarget::RootX);
    }

    #[test]
    fn channel_index_scopes_to_the_selected_figure() {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            Channel {
                figure: 1,
                target: ChannelTarget::RootX,
                keys: vec![],
            },
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 0.0)]),
        ];
        assert_eq!(channel_index(&core, ChannelTarget::RootX), Some(1));
        assert_eq!(channel_index(&core, ChannelTarget::RootY), None);
        assert!(channel_for(&core, ChannelTarget::RootX).is_some());
    }

    #[test]
    fn nearest_key_picks_closest_and_breaks_ties_early() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        );
        assert_eq!(nearest_key(&ch, 0.0), Some(0));
        assert_eq!(nearest_key(&ch, 4.9), Some(0));
        // Exactly between the two keys: the earlier key wins,
        // matching set_graph_key_interp's documented pick.
        assert_eq!(nearest_key(&ch, 5.0), Some(0));
        assert_eq!(nearest_key(&ch, 5.1), Some(1));
        let empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(nearest_key(&empty, 3.0), None);
    }

    #[test]
    fn current_interp_reads_the_key_the_op_would_edit() {
        let mut core = EditorCore::new();
        // No node selected: nothing to read.
        assert_eq!(current_interp(&core), None);
        core.selected_node = Some(0);
        // Keys at frames 0 and 4 — inside the demo's 5-frame
        // timeline, so `goto` can park next to each of them.
        core.project.channels = vec![channel(
            ChannelTarget::Angle(0),
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Step),
                Key::flat(4.0, 90.0),
            ],
        )];
        core.goto(1);
        assert_eq!(current_interp(&core), Some(Interp::Step));
        // Near the second key (default Hermite) the readout
        // follows the op's target key.
        core.goto(3);
        assert_eq!(current_interp(&core), Some(Interp::Hermite));
    }

    #[test]
    fn sampled_curve_passes_through_its_keys() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(5.0, 80.0),
                Key::flat(10.0, 20.0),
            ],
        );
        let view = GraphView::auto_fit(std::slice::from_ref(&ch), 11, 500.0, 250.0);
        // 100 steps over 10 frames lands a sample exactly on
        // every key frame.
        let pts = sample_curve(&view, &ch, 100);
        assert_eq!(pts.len(), 101);
        for handle in view.key_handles(&ch) {
            let nearest = pts
                .iter()
                .map(|p| (p[0] - handle[0]).abs() + (p[1] - handle[1]).abs())
                .fold(f32::INFINITY, f32::min);
            assert!(nearest < 1e-2, "curve misses key {handle:?} by {nearest}");
        }
        assert!(sample_curve(&view, &ch, 0).is_empty());
    }

    #[test]
    fn diamond_is_four_points_on_the_radius() {
        let d = diamond([10.0, 20.0], 4.0);
        assert_eq!(d[0], [10.0, 16.0], "top first: {d:?}");
        assert_eq!(d[1], [14.0, 20.0]);
        assert_eq!(d[2], [10.0, 24.0]);
        assert_eq!(d[3], [6.0, 20.0]);
    }

    #[test]
    fn preview_points_put_value_zero_at_the_bottom() {
        // The Linear preview is the exact diagonal of the
        // canonical ramp, so its pixels are the box diagonal
        // from bottom-left to top-right.
        let pts = preview_points(TangentPreset::Linear, 48.0, 28.0, 5);
        assert_eq!(pts.len(), 5);
        for (got, want) in pts.iter().zip([[0.0, 28.0], [12.0, 21.0], [24.0, 14.0], [36.0, 7.0], [48.0, 0.0]]) {
            assert!(
                approx(got[0], want[0]) && approx(got[1], want[1]),
                "linear preview point {got:?} != {want:?}"
            );
        }
        // Ease-in lags the diagonal early, so at 25% its pixel
        // sits BELOW the diagonal's (larger y = lower value).
        let ease = preview_points(TangentPreset::EaseIn, 48.0, 28.0, 5);
        assert!(ease[1][1] > 21.0, "ease-in starts slow: {ease:?}");
    }

    #[test]
    fn apply_scale_doubles_values_and_reports() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 10.0), Key::flat(4.0, 30.0)],
        )];
        apply_scale(&mut core, ChannelTarget::RootX, 2.0);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[0].value, 20.0);
        assert_eq!(keys[1].value, 60.0);
        assert!(core.status.contains("Scaled graph channel values by 2"));
        // No channel: status only, nothing created.
        apply_scale(&mut core, ChannelTarget::RootY, 2.0);
        assert_eq!(core.project.channels.len(), 1);
        assert!(core.status.contains("to scale"));
    }

    #[test]
    fn apply_mirror_reflects_frames_around_the_timeline_middle() {
        let mut core = EditorCore::new();
        // The demo timeline has 5 frames, so the mirror centre
        // is frame 2: keys at 0 and 4 swap places and values.
        assert_eq!(core.frame_count(), 5);
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 10.0), Key::flat(4.0, 30.0)],
        )];
        apply_mirror(&mut core, ChannelTarget::RootX);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].frame, 0.0);
        assert_eq!(keys[0].value, 30.0, "ascending order restored");
        assert_eq!(keys[1].frame, 4.0);
        assert_eq!(keys[1].value, 10.0);
    }

    #[test]
    fn apply_reduce_drops_only_redundant_keys() {
        let mut core = EditorCore::new();
        // A constant channel's middle key contributes nothing.
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 42.0),
                Key::flat(2.0, 42.0),
                Key::flat(4.0, 42.0),
            ],
        )];
        apply_reduce(&mut core, ChannelTarget::RootX);
        assert_eq!(core.project.channels[0].keys.len(), 2);
        assert!(core.status.contains("removed 1 key"));
        // A zigzag's middle key carries the shape: kept.
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(2.0, 100.0),
                Key::flat(4.0, 0.0),
            ],
        )];
        apply_reduce(&mut core, ChannelTarget::RootX);
        assert_eq!(core.project.channels[0].keys.len(), 3);
    }

    #[test]
    fn pointer_roundtrip_through_the_panels_own_fit() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        let size = [480.0, 170.0];
        let view = drag_view(&core, ChannelTarget::RootX, size);
        let ch = &core.project.channels[0];
        // Every key's drawn pixel converts back to its data
        // and hit-tests to its own index — the drag uses the
        // exact transform the drawing used.
        for (i, k) in ch.keys.iter().enumerate() {
            let px = view.to_px(k.frame, k.value);
            let (f, v) = view.to_data(px);
            assert!(
                approx(f, k.frame) && approx(v, k.value),
                "key {i} roundtrip ({}, {}) -> ({f}, {v})",
                k.frame,
                k.value
            );
            assert_eq!(view.hit_key(ch, px, GRAB_RADIUS), Some(i));
        }
        // The middle of the canvas is far from every handle.
        assert_eq!(view.hit_key(ch, [240.0, 160.0], GRAB_RADIUS), None);
    }

    #[test]
    fn drag_key_to_crosses_neighbours_clamps_and_reindexes() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 40.0),
                Key::flat(8.0, 80.0),
            ],
        );
        // Drag key 0 past its neighbour: it lands at index 1
        // and the order is ascending again.
        assert_eq!(drag_key_to(&mut ch, 0, 6.5, 55.0), Some(1));
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![4.0, 6.5, 8.0]);
        assert_eq!(ch.keys[1].value, 55.0);
        // Frames clamp at 0; values do not clamp.
        assert_eq!(drag_key_to(&mut ch, 2, -2.0, -10.0), Some(0));
        assert_eq!(ch.keys[0].frame, 0.0);
        assert_eq!(ch.keys[0].value, -10.0);
        // Out-of-range index: no change, no new index.
        assert_eq!(drag_key_to(&mut ch, 99, 1.0, 1.0), None);
        assert_eq!(ch.keys.len(), 3);
        assert_eq!(ch.keys[1].frame, 4.0);
    }

    #[test]
    fn resolve_drag_index_uses_hint_then_anchor() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 10.0),
                Key::flat(4.0, 20.0),
                Key::flat(8.0, 30.0),
            ],
        );
        // Fast path: the hinted key still sits at the anchor.
        assert_eq!(resolve_drag_index(&ch, 1, 4.0), Some(1));
        // A re-sort permuted the vec (here by hand): the hint
        // now points at the frame-8 key, so the anchor decides.
        let permuted = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(4.0, 20.0),
                Key::flat(8.0, 30.0),
                Key::flat(0.0, 10.0),
            ],
        );
        assert_eq!(resolve_drag_index(&permuted, 1, 4.0), Some(0));
        // A stale out-of-range hint still resolves by anchor.
        assert_eq!(resolve_drag_index(&ch, 99, 4.0), Some(1));
        // No keys: nothing to resolve.
        let empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(resolve_drag_index(&empty, 0, 4.0), None);
    }

    #[test]
    fn seek_frame_rounds_and_clamps() {
        // 60 px per frame over frames 0..=8.
        let view = GraphView {
            frame_min: 0.0,
            frame_max: 8.0,
            val_min: -8.0,
            val_max: 88.0,
            width: 480.0,
            height: 170.0,
        };
        assert_eq!(seek_frame(&view, 144.0, 10), Some(2), "frame 2.4 rounds down");
        assert_eq!(seek_frame(&view, 150.0, 10), Some(3), "frame 2.5 rounds up");
        assert_eq!(seek_frame(&view, -30.0, 10), Some(0), "before the start clamps");
        assert_eq!(seek_frame(&view, 10_000.0, 5), Some(4), "past the end clamps");
        assert_eq!(seek_frame(&view, 144.0, 0), None, "no frames: no seek");
    }

    #[test]
    fn drag_gesture_end_to_end_and_click_seek() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        let mut st = GraphPanelState::default();
        let size = [480.0, 170.0];

        // Down on key 1's handle grabs it.
        let view = drag_view(&core, ChannelTarget::RootX, size);
        let grab = view.to_px(4.0, 80.0);
        assert!(drag_down(&mut core, &mut st, grab, size));
        assert_eq!(st.drag_key, Some(1));
        assert!(approx(st.drag_anchor, 4.0));

        // Move to data (6, 10): the key follows, still sorted.
        let dest = view.to_px(6.0, 10.0);
        assert!(drag_move(&mut core, &mut st, dest, size));
        let keys = &core.project.channels[0].keys;
        assert!(approx(keys[1].frame, 6.0) && approx(keys[1].value, 10.0));
        assert_eq!(st.drag_key, Some(1));

        // Up ends the drag and reports the final position.
        assert!(drag_up(&mut core, &mut st, dest, size, 40.0, false));
        assert_eq!(st.drag_key, None);
        assert!(
            core.status.contains("Graph key moved"),
            "status: {}",
            core.status
        );

        // A click on empty canvas seeks: x=470 is past the
        // fitted end (frame 8) but the demo has only 5 frames,
        // so the seek clamps to frame index 4.
        let far = [470.0, 165.0];
        assert!(!drag_down(&mut core, &mut st, far, size));
        core.goto(0);
        assert!(drag_up(&mut core, &mut st, far, size, 1.0, false));
        assert_eq!(core.current, 4);
        // A long drag over empty canvas does NOT seek, and
        // neither does a cancelled click.
        core.goto(0);
        assert!(!drag_up(&mut core, &mut st, far, size, 30.0, false));
        assert_eq!(core.current, 0);
        assert!(!drag_up(&mut core, &mut st, far, size, 1.0, true));
        assert_eq!(core.current, 0);

        // A cancelled key drag rolls back through the Down
        // snapshot: the key returns to (6, 10).
        let grab2 = drag_view(&core, ChannelTarget::RootX, size).to_px(6.0, 10.0);
        assert!(drag_down(&mut core, &mut st, grab2, size));
        let dest2 = drag_view(&core, ChannelTarget::RootX, size).to_px(2.0, 70.0);
        assert!(drag_move(&mut core, &mut st, dest2, size));
        assert!(drag_up(&mut core, &mut st, dest2, size, 200.0, true));
        assert_eq!(st.drag_key, None);
        let keys = &core.project.channels[0].keys;
        assert!(
            keys.iter()
                .any(|k| approx(k.frame, 6.0) && approx(k.value, 10.0)),
            "cancel restored the pre-drag key: {keys:?}"
        );
    }
}
