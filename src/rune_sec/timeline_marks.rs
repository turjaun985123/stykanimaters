//! Timeline marks for the Rune editor shell (Phase E,
//! section 6/40 — replacement timeline).
//!
//! A richer drop-in for `chrome_view::timeline_row`: the same
//! numbered frame chips with the same element keys, plus the
//! three marks the Phase D timeline could not show — frames
//! inside the core's loop range are tinted, frames inside the
//! timeline range selection are outlined, and a chip whose
//! frame holds longer or shorter than the base rate carries
//! its duration as a small `×N` suffix in its label. On top of
//! that, the row takes drag selection: press-and-drag across
//! chips writes [`FrameOpsState::range_sel`], the selection
//! the Frames section's Copy/Delete range ops consume (the
//! GPUI app set it by shift-click; the core tracks no range
//! state of its own besides `loop_range`).
//!
//! Integrator wiring (main.rs, not edited here): declare the
//! module and swap the view's `.child(chrome_view::
//! timeline_row(s, &t))` for `.child(timeline_marks::
//! timeline_marks(s, &t))`. No new `State` fields — the drag
//! writes the existing `s.frame_ops.range_sel`.
//!
//! ## Keys (preservation)
//!
//! `timeline` on the outer row and `frame-{n}` (1-based) on
//! every chip, exactly as `chrome_view::timeline_row` — the
//! headless tests find the chips by those keys. Two keys are
//! added: `timeline-chips` on the inner content row that
//! carries the drag handler (see below), and
//! `timeline-markers` on the marker row under the chips.
//! Phase I adds one key per marker glyph:
//! `marker-cam-{frame}` and `marker-graph-{frame}`, where
//! `{frame}` is the 0-based timeline index the key belongs
//! to — the engine's own frame numbering (`CameraKey.frame`,
//! [`graph_key_frame`]), not the chips' 1-based labels.
//! Phase J adds `timeline-ruler` on the time ruler above
//! the chips. Phase L adds `timeline-loop-row` on the loop
//! row under the timeline (see the Loop row section
//! below), with `loop-set` / `loop-clear` and the four
//! endpoint steppers `loop-a-dn` / `loop-a-up` /
//! `loop-b-dn` / `loop-b-up` inside it.
//!
//! ## Time ruler
//!
//! Above the chips sits a slim ruler in the same scrolled
//! column, using the same [`CHIP_W`] / [`CHIP_GAP`]
//! geometry and starting at the same x = 0 as the first
//! chip: one fixed-width slot per frame. A slot at a whole
//! second — where the frame index is a multiple of the
//! project frame rate rounded to an integer, with a
//! minimum rate of one frame per second — carries that
//! frame's `m:ss` time in muted small text; every other
//! slot carries a short, thin tick mark.
//!
//! Phase K gives the ruler a drag of its own: pressing or
//! dragging along it seeks — `core.goto` on the frame
//! under the pointer ([`ruler_frame_at_x`]), on both the
//! down and the move phase, so a press lands on its frame
//! at once and a drag scrubs. The handler sits on the
//! ruler row itself, a sibling of the chips row inside
//! the same scrolled column, so its drag points are the
//! same content coordinates the chips' drag reads — the
//! ruler can never be mistaken for the chips' gesture (a
//! separate element owns it), and it writes nothing but
//! the playhead: no range selection, no loop edges, and
//! `goto` never touches the play state (the marker-press
//! precedent). Like the chips row, a horizontal drag on
//! the ruler seeks instead of scrolling the viewport;
//! the wheel still scrolls.
//!
//! ## Marker row
//!
//! Under the chips sits a slim row of marker cells, one per
//! chip, aligned chip-for-chip (same [`CHIP_W`] /
//! [`CHIP_GAP`] geometry, inside the same scrolled column,
//! so the two rows scroll together). A cell shows `◆` when
//! its frame carries a camera key (`Project::camera`,
//! `CameraKey.frame` — an exact frame number) and `●` when
//! any graph channel has a key on it; both glyphs when both,
//! blank when neither. Each glyph is its own pressable
//! element (Phase I): pressing one seeks to its frame with
//! `core.goto`. That cannot fight the chips' drag gesture —
//! the drag handler sits on the chips row, a sibling, and a
//! press on a marker never reaches it. The glyphs, their
//! colours, the cell geometry and the row's height are
//! exactly what the display-only row had; the pressable
//! wrapper adds no fill, padding or radius.
//!
//! Graph keys live at fractional times (`graph::Key.frame:
//! f32`), so "on a frame" needs the engine's own mapping:
//! the graph panel's seek (`graph_panel::seek_frame`) rounds
//! with `f32::round` — nearest frame, halves away from
//! zero. [`graph_key_frame`] mirrors that rounding exactly.
//! One deliberate difference: seek *clamps* its result into
//! the timeline because a click must land somewhere, while
//! a marker asserts a key is **on** the frame — so a key
//! whose rounded frame falls outside the timeline marks
//! nothing rather than piling onto an end frame.
//!
//! Cost: the row is computed once per render by
//! [`markers_for_project`] — one pass over the camera keys
//! and one over every channel's keys — never per-cell
//! searches ([`frame_markers`] exists for single-frame
//! callers and tests).
//!
//! ## Press / drag semantics (exact)
//!
//! Chrome's chips had `on_press → goto(i)` (Rune presses fire
//! on release inside the element). A drag and a press cannot
//! both own the same pointer, so the chips here carry **no**
//! `on_press`; the gesture is one `on_drag` on the inner
//! content row, and click behaviour is reconstructed inside
//! it:
//!
//! - **Down** on chip *f*: `range_sel = (f, f)` (anchor and
//!   focus), and *f* is stashed in the drag's scratch `data`.
//! - **Move**: focus becomes the chip under the pointer
//!   (`frame_at_x` on the drag's local x); `range_sel =
//!   (anchor, focus)`. The current frame does **not** change
//!   while dragging — a drag is a selection, not a seek.
//! - **Up**: if the release chip is the anchor chip, the
//!   gesture was a plain click — the selection is cleared and
//!   `core.goto(anchor)` runs, exactly chrome's click. (The
//!   path doesn't matter: drag out and back onto the anchor
//!   chip and release, and it still counts as a click.) If
//!   the release chip differs, the selection stands and the
//!   current frame is untouched.
//! - **Cancelled Up** (Escape, focus loss, a scroll taking
//!   the pointer): nothing is undone and nothing goes
//!   anywhere — the selection as last drawn stands. There is
//!   no snapshot to restore; a selection is view state, and
//!   the next press starts a fresh one.
//!
//! ## Loop edge dragging
//!
//! When a loop range exists, a press within [`EDGE_PX`]
//! points of the range's start-chip left edge or end-chip
//! right edge ([`edge_at`]) starts a loop edge drag instead
//! of the selection gesture: Move rewrites the dragged
//! endpoint to the chip under the pointer
//! ([`loop_after_edge_drag`] / `apply_loop_edge`), Up posts
//! the core's own status line once. The range selection is
//! not touched by an edge press, and a cancelled edge drag
//! leaves the range as last drawn — the same discipline as
//! the selection gesture above.
//!
//! The core convention this mirrors (editor.rs): the range
//! is one `EditorCore::loop_range: Option<(usize, usize)>`
//! field, written through `set_loop_range`, which orders
//! the endpoints and clamps them to the timeline; endpoints
//! meeting is a valid single-frame loop, **not** a clear —
//! clearing is `clear_loop_range`'s job alone. The field is
//! documented in the core as runtime state that is never
//! snapshotted for undo, so the drag takes no snapshots.
//! Because `set_loop_range` also writes `core.status` on
//! every call (and the shell toasts status changes), Move
//! writes the field directly with exactly
//! `set_loop_range`'s normalisation, and Up calls
//! `set_loop_range` once with the final endpoints to post
//! its status line — the frame_ops Loop start/end buttons'
//! write, once per gesture instead of once per move.
//!
//! Which gesture a press started is carried in the drag's
//! scratch `data`, like the selection anchor: non-negative
//! is the anchor frame; `-1.0` / `-2.0` are the start /
//! end edge sentinels (`edge_scratch`).
//!
//! Coordinates: `Drag` points are in the dragged element's
//! own layout points "from its top-left as drawn (after
//! scrolling and live offsets)" (rune `el.rs`, `Drag` docs).
//! The handler therefore sits on the **inner** row — the
//! element actually translated by the outer row's `scroll_x`
//! — so drag x is a content coordinate and `frame_at_x` maps
//! it directly, scrolled or not. The trade-off, accepted for
//! a ruler: a horizontal drag on the chips selects instead
//! of scrolling the viewport (pointer capture); the wheel
//! still scrolls.
//!
//! ## Loop row (Phase L)
//!
//! Under the scrolled timeline — outside the scroll
//! viewport, so its buttons never scroll away on a long
//! timeline — sits a small loop row (`timeline-loop-row`),
//! rendered only when it has content:
//!
//! - **No loop range**: one `loop-set` button ("Set loop")
//!   that asks the core for a loop from the current frame
//!   to four frames on, clamped to the last frame
//!   ([`loop_set_range`]). On a zero-frame project the row
//!   renders nothing at all — there is nothing to loop.
//! - **A loop range**: the [`loop_label`] readout ("Loop
//!   {a+1}–{b+1}", the status strip's wording), four
//!   steppers moving one endpoint by one frame
//!   (`loop-a-dn` / `loop-a-up` for the start, `loop-b-dn`
//!   / `loop-b-up` for the end) and a `loop-clear` button
//!   ("Clear").
//!
//! Every write goes through the core's own ops, once per
//! press: the steppers hand [`loop_after_step`]'s adjusted
//! pair to `set_loop_range`, which orders and clamps it —
//! so stepping an endpoint *past* the other one swaps the
//! endpoints' roles (the stepped endpoint becomes the far
//! end), exactly the handover an edge drag across the
//! range performs. Clear calls `clear_loop_range`, the
//! core's own op (it also posts "Loop range cleared.") —
//! the same op, under the same `loop-clear` key, as the
//! transport's and the Frames section's Loop clear
//! buttons: one action, one key, three homes. Like the
//! edge drag, the row takes no snapshots: `loop_range` is
//! runtime state the core never undoes.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::frame_ops::{selected_range, FrameOpsState};
use crate::State;

/// Chip width in points — chrome_view's timeline chip size,
/// and the width [`frame_at_x`] is parameterised with.
pub const CHIP_W: f32 = 30.0;
/// Chip height in points (chrome_view parity).
pub const CHIP_H: f32 = 24.0;
/// Horizontal gap between chips (chrome_view parity).
pub const CHIP_GAP: f32 = 4.0;

/// Which frame a content x falls on: chips of `chip_w`
/// points separated by `gap`, laid out from x = 0. A point in
/// the gap after a chip still counts as that chip (the floor
/// does it); points before the first chip clamp to frame 0
/// and points past the last chip clamp to `count - 1` (a
/// captured drag keeps reporting outside the row). `None`
/// only when there are no frames, the stride is degenerate,
/// or `x` is NaN.
pub fn frame_at_x(x: f32, chip_w: f32, gap: f32, count: usize) -> Option<usize> {
    if count == 0 || x.is_nan() {
        return None;
    }
    let stride = chip_w + gap;
    if stride <= 0.0 {
        return None;
    }
    let idx = (x / stride).floor() as i64;
    Some(idx.clamp(0, count as i64 - 1) as usize)
}

/// The `×N` suffix for a frame whose hold-time multiplier
/// differs from the base rate: `×2`, `×0.5`, `×1.25` —
/// integers print bare, fractions print with up to two
/// decimals (durations are made by the core's ×0.5/×2 steps,
/// so two decimals are exact). `None` at exactly 1.0, the
/// base rate, which is most frames.
pub fn duration_suffix(duration: f32) -> Option<String> {
    if duration == 1.0 {
        return None;
    }
    if duration.fract() == 0.0 {
        return Some(format!("×{}", duration as i64));
    }
    let trimmed = format!("{duration:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    Some(format!("×{trimmed}"))
}

/// A chip's label: the 1-based frame number, plus the
/// duration suffix when the frame's hold differs from 1.0 —
/// `3`, or `3 ×2` for a double-length third frame.
pub fn chip_label(frame: usize, duration: f32) -> String {
    match duration_suffix(duration) {
        Some(suffix) => format!("{} {suffix}", frame + 1),
        None => format!("{}", frame + 1),
    }
}

/// Half-width, in points, of the grab zone around a loop
/// edge: a press this close to the start chip's left edge
/// or the end chip's right edge starts a loop edge drag
/// (see the module docs). Small next to the 30-point chip,
/// so the zones of a one-frame loop never overlap.
pub const EDGE_PX: f32 = 8.0;

/// A loop range endpoint a press can grab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The range's first frame: its chip's left edge.
    Start,
    /// The range's last frame: its chip's right edge.
    End,
}

/// Which loop edge, if any, a press at content x `x` grabs:
/// within [`EDGE_PX`] of the start chip's left edge or the
/// end chip's right edge, in the same chip geometry
/// [`frame_at_x`] uses (chip `i`'s left is `i * (chip_w +
/// gap)`, its right that plus `chip_w`). When both zones
/// match (impossible at the shell's geometry — a chip is
/// wider than two zones — but possible for a caller's), the
/// nearer edge wins and a tie goes to Start. `None` when
/// there is no loop range, no frames, a degenerate stride,
/// or `x` is NaN. Hand-built endpoints past the timeline
/// clamp to the last frame first, as [`in_loop`] does.
pub fn edge_at(
    x: f32,
    chip_w: f32,
    gap: f32,
    loop_range: Option<(usize, usize)>,
    count: usize,
) -> Option<Edge> {
    let (a, b) = loop_range?;
    if count == 0 || x.is_nan() {
        return None;
    }
    let stride = chip_w + gap;
    if stride <= 0.0 {
        return None;
    }
    let (a, b) = (a.min(b).min(count - 1), a.max(b).min(count - 1));
    let start_x = a as f32 * stride;
    let end_x = b as f32 * stride + chip_w;
    let d_start = (x - start_x).abs();
    let d_end = (x - end_x).abs();
    match (d_start <= EDGE_PX, d_end <= EDGE_PX) {
        (true, true) if d_end < d_start => Some(Edge::End),
        (true, _) => Some(Edge::Start),
        (_, true) => Some(Edge::End),
        _ => None,
    }
}

/// The loop range after dragging `edge` to `frame`: the
/// dragged endpoint replaced, then normalised with exactly
/// `EditorCore::set_loop_range`'s maths — endpoints
/// ordered, end clamped to the timeline, start clamped to
/// the end. Endpoints meeting yields a single-frame loop
/// (the core's convention), never a clear. `None` only
/// when there is no range to drag or no frames.
pub fn loop_after_edge_drag(
    range: Option<(usize, usize)>,
    edge: Edge,
    frame: usize,
    count: usize,
) -> Option<(usize, usize)> {
    let (a, b) = range?;
    if count == 0 {
        return None;
    }
    let (a, b) = match edge {
        Edge::Start => (frame, b),
        Edge::End => (a, frame),
    };
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let b = b.min(count - 1);
    let a = a.min(b);
    Some((a, b))
}

/// Write one edge-drag move into the core: the field only,
/// no status (see the module docs — `set_loop_range` runs
/// once on release and posts the status line then). The
/// core never snapshots `loop_range`, so there is no undo
/// bookkeeping to mirror.
fn apply_loop_edge(core: &mut EditorCore, edge: Edge, frame: usize) {
    if let Some(next) = loop_after_edge_drag(core.loop_range, edge, frame, core.frame_count()) {
        core.loop_range = Some(next);
    }
}

/// The loop range the loop row's Set loop button asks for:
/// from `current`, four frames on, the end clamped to the
/// last frame — so near the end of the timeline the loop
/// is simply shorter, and on the last frame it is a
/// single-frame loop. `None` only when there are no
/// frames. The pair is what the handler hands to
/// `EditorCore::set_loop_range`, which applies its own
/// ordering and clamping on top (a hand-built `current`
/// past the end therefore still lands on the last frame).
pub fn loop_set_range(current: usize, frame_count: usize) -> Option<(usize, usize)> {
    if frame_count == 0 {
        return None;
    }
    let last = frame_count - 1;
    Some((current, current.saturating_add(4).min(last)))
}

/// The loop range after one stepper press: the chosen
/// endpoint moved one frame in `dir`'s sign (negative
/// towards the start of the timeline, positive towards
/// the end), saturating at frame 0 — the one bound
/// `set_loop_range` cannot apply for the caller, since a
/// `usize` endpoint stepped below zero would wrap before
/// the op ever saw it. The input is normalised first, so
/// `Edge::Start` always names the earlier endpoint, the
/// one the row's label prints first. The result is the
/// raw pair for `set_loop_range`: deliberately *not*
/// ordered or end-clamped here, because the op does both
/// — an endpoint stepped past the other one therefore
/// comes back from the op with the roles swapped.
pub fn loop_after_step(range: (usize, usize), edge: Edge, dir: i32) -> (usize, usize) {
    let (a, b) = (range.0.min(range.1), range.0.max(range.1));
    let step = |v: usize| {
        if dir < 0 {
            v.saturating_sub(1)
        } else {
            v.saturating_add(1)
        }
    };
    match edge {
        Edge::Start => (step(a), b),
        Edge::End => (a, step(b)),
    }
}

/// The loop row's readout for a range: "Loop {a+1}–{b+1}"
/// — 1-based, en dash, the status strip's `loop_segment`
/// wording for the same state. A hand-built unordered
/// range reads in timeline order.
pub fn loop_label(range: (usize, usize)) -> String {
    let (a, b) = (range.0.min(range.1), range.0.max(range.1));
    format!("Loop {}–{}", a + 1, b + 1)
}

/// What the loop row's Set loop button does: ask the core
/// for [`loop_set_range`]'s pair at the playhead. On an
/// empty project there is no pair and nothing happens —
/// the button is not rendered then anyway.
fn set_loop_here(s: &mut State) {
    if let Some((a, b)) = loop_set_range(s.core.current, s.core.frame_count()) {
        s.core.set_loop_range(a, b);
    }
}

/// What one loop-row stepper press does: hand the adjusted
/// endpoint pair to `set_loop_range`, whose ordering and
/// clamping settle it (see [`loop_after_step`]). No range,
/// no press — the steppers render only beside a range.
fn step_loop(s: &mut State, edge: Edge, dir: i32) {
    if let Some(range) = s.core.loop_range {
        let (a, b) = loop_after_step(range, edge, dir);
        s.core.set_loop_range(a, b);
    }
}

/// What the loop row's Clear button does: the core's own
/// `clear_loop_range` — the shell's one way of clearing a
/// loop, shared with the transport and Frames buttons.
fn clear_loop_here(s: &mut State) {
    s.core.clear_loop_range();
}

/// The drag-scratch sentinel for an edge gesture (the
/// anchor frames the selection gesture stores are all
/// non-negative, so negatives are free).
fn edge_scratch(edge: Edge) -> f32 {
    match edge {
        Edge::Start => -1.0,
        Edge::End => -2.0,
    }
}

/// The edge a drag's scratch value encodes, if it encodes
/// one (see `edge_scratch`).
fn edge_from_scratch(data: f32) -> Option<Edge> {
    if data == -1.0 {
        Some(Edge::Start)
    } else if data == -2.0 {
        Some(Edge::End)
    } else {
        None
    }
}

/// Is frame `i` inside the loop range? Endpoints are
/// inclusive; the core keeps them ordered
/// (`EditorCore::set_loop_range`), and this normalises
/// anyway so a hand-built range still reads correctly.
pub fn in_loop(loop_range: Option<(usize, usize)>, i: usize) -> bool {
    match loop_range {
        Some((a, b)) => a.min(b) <= i && i <= a.max(b),
        None => false,
    }
}

/// Is frame `i` inside the raw `(anchor, focus)` range
/// selection? Either order counts (a selection dragged
/// leftwards has anchor > focus). For the clamped,
/// timeline-aware answer the Frames section's
/// `selected_range` is the authority; this pure form is what
/// the chip rendering uses, so a stale endpoint simply
/// matches no chip past the end.
pub fn in_sel(range_sel: Option<(usize, usize)>, i: usize) -> bool {
    match range_sel {
        Some((a, b)) => a.min(b) <= i && i <= a.max(b),
        None => false,
    }
}

/// Height of one marker cell in points: a slim strip under
/// its chip, just tall enough for one line of glyphs.
pub const MARKER_H: f32 = 14.0;

/// Height of the time ruler in points: a slim strip above
/// the chips, just tall enough for its 10-point labels.
pub const RULER_H: f32 = 12.0;

/// The ruler's label style: small enough that an `m:ss`
/// label (at most five characters until ten minutes, and
/// still short beyond it) sits inside one fixed-width chip
/// slot without widening it; the line height matches
/// [`RULER_H`] exactly.
const RULER_LABEL_STYLE: tok::TextStyle = tok::TextStyle::new(10.0, 500.0).lh(RULER_H);

/// The whole number of frames in one second for the ruler:
/// the project rate rounded to the nearest integer (halves
/// away from zero, `f32::round`), with a minimum of one so
/// a stored rate below half a frame per second — or a
/// non-finite one from a hand-edited file — still yields a
/// usable ruler instead of a division by zero.
fn ruler_fps(fps: f32) -> u64 {
    if fps.is_finite() {
        fps.round().max(1.0) as u64
    } else {
        1
    }
}

/// The ruler label for `frame` at project rate `fps`:
/// `Some("m:ss")` — minutes unpadded, seconds zero-padded
/// — when the frame starts a whole second (its index is a
/// multiple of [`ruler_fps`]), `None` for every other
/// frame. Frame 0 always starts the first second, so it
/// always reads `"0:00"`.
pub fn ruler_label(frame: usize, fps: f32) -> Option<String> {
    let frames_per_second = ruler_fps(fps);
    let frame = frame as u64;
    if frame % frames_per_second != 0 {
        return None;
    }
    let seconds = frame / frames_per_second;
    Some(format!("{}:{:02}", seconds / 60, seconds % 60))
}

/// One ruler slot per frame, in frame order: the
/// [`ruler_label`] for each of the first `frame_count`
/// frames at project rate `fps` — `Some` at whole seconds,
/// `None` (a plain tick) elsewhere. The vector's length is
/// exactly `frame_count`, so the ruler row the builder
/// renders from it can never drift out of step with the
/// chips.
pub fn ruler_ticks(frame_count: usize, fps: f32) -> Vec<Option<String>> {
    (0..frame_count).map(|frame| ruler_label(frame, fps)).collect()
}

/// The frame a ruler drag at content x `x` seeks to: the
/// ruler's slots use the chips' own pitch — one
/// [`CHIP_W`] slot plus one [`CHIP_GAP`] per frame, laid
/// out from x = 0 (see the builder) — so the frame is
/// `floor(x / (CHIP_W + CHIP_GAP))`, clamped to the last
/// frame (a captured drag keeps reporting past the row's
/// end). Unlike [`frame_at_x`] this never fails: an
/// empty timeline, a negative x and a non-finite x all
/// read as frame 0, which the core's `goto` then simply
/// ignores or lands on.
pub fn ruler_frame_at_x(x: f32, frame_count: usize) -> usize {
    if frame_count == 0 || !x.is_finite() || x <= 0.0 {
        return 0;
    }
    let idx = (x / (CHIP_W + CHIP_GAP)).floor() as usize;
    idx.min(frame_count - 1)
}

/// The timeline frame a graph key at fractional time `t`
/// belongs to: `t` rounded with `f32::round` (nearest frame,
/// halves away from zero) — the graph panel's own seek
/// convention (`graph_panel::seek_frame`). `None` when
/// there are no frames, `t` is not finite, or the rounded
/// frame falls outside the timeline (see the module docs
/// for why out-of-timeline keys are skipped, not clamped).
pub fn graph_key_frame(t: f32, count: usize) -> Option<usize> {
    if count == 0 || !t.is_finite() {
        return None;
    }
    let f = t.round();
    if f < 0.0 || f >= count as f32 {
        return None;
    }
    Some(f as usize)
}

/// Does `frame` carry a camera key, and does it carry a
/// graph key — `(camera, graph)`? A camera key matches by
/// its exact stored frame; a graph key matches by
/// [`graph_key_frame`]'s rounding, over every channel. A
/// frame index past the timeline end carries neither.
pub fn frame_markers(core: &EditorCore, frame: usize) -> (bool, bool) {
    let count = core.frame_count();
    if frame >= count {
        return (false, false);
    }
    let camera = core.project.camera.iter().any(|k| k.frame == frame);
    let graph = core
        .project
        .channels
        .iter()
        .flat_map(|ch| ch.keys.iter())
        .any(|k| graph_key_frame(k.frame, count) == Some(frame));
    (camera, graph)
}

/// [`frame_markers`] for the whole timeline in one pass:
/// one sweep over the camera keys and one over every
/// channel's keys, marking into a per-frame vector. This is
/// what the marker row renders from — the per-cell cost is
/// a vector read, however many keys the project carries.
pub fn markers_for_project(core: &EditorCore) -> Vec<(bool, bool)> {
    let count = core.frame_count();
    let mut out = vec![(false, false); count];
    for key in &core.project.camera {
        if key.frame < count {
            out[key.frame].0 = true;
        }
    }
    for channel in &core.project.channels {
        for key in &channel.keys {
            if let Some(f) = graph_key_frame(key.frame, count) {
                out[f].1 = true;
            }
        }
    }
    out
}

/// Element key of the camera marker on `frame`:
/// `marker-cam-{frame}`, with `frame` the 0-based timeline
/// index (see the module docs' key section).
pub fn camera_marker_key(frame: usize) -> String {
    format!("marker-cam-{frame}")
}

/// Element key of the graph marker on `frame`:
/// `marker-graph-{frame}`, with `frame` the 0-based
/// timeline index (see the module docs' key section).
pub fn graph_marker_key(frame: usize) -> String {
    format!("marker-graph-{frame}")
}

/// What pressing a marker does: seek the playhead to the
/// marker's frame — the core's plain `goto`, the same seek
/// a chip click performs. A marker only exists for a frame
/// inside the timeline, so no clamping is needed here.
fn goto_marker(s: &mut State, frame: usize) {
    s.core.goto(frame);
}

/// The ruler's drag gesture (Phase K, see the module
/// docs): a seek, nothing more. Down and Move both go to
/// the frame under the pointer — Down so a plain press
/// lands at once, Move so a drag scrubs — and Up does
/// nothing (the release frame is already current from
/// the last Move). The conversion is exactly the chips'
/// drag's: `d.x` read directly as a content coordinate,
/// because this handler sits on the ruler row inside the
/// same scrolled column. Play state, the range selection
/// and the loop range are never written here.
fn ruler_drag(s: &mut State, d: &mut Drag) {
    match d.phase {
        Phase::Down | Phase::Move => {
            let frame = ruler_frame_at_x(d.x, s.core.frame_count());
            s.core.goto(frame);
        }
        Phase::Up => {}
    }
}

/// The row's drag gesture (see the module docs for the exact
/// semantics). `d.data` records which gesture the press
/// started: the selection anchor across phases as a fallback
/// (the selection itself is the primary record), or a loop
/// edge sentinel for an edge drag.
fn timeline_drag(s: &mut State, d: &mut Drag) {
    let n = s.core.frame_count();
    let Some(f) = frame_at_x(d.x, CHIP_W, CHIP_GAP, n) else {
        return;
    };
    match d.phase {
        Phase::Down => {
            // A press on a loop edge starts an edge drag and
            // leaves the range selection untouched; anywhere
            // else is the selection gesture.
            if let Some(edge) = edge_at(d.x, CHIP_W, CHIP_GAP, s.core.loop_range, n) {
                d.data = edge_scratch(edge);
                return;
            }
            d.data = f as f32;
            s.frame_ops.range_sel = Some((f, f));
        }
        Phase::Move => {
            if let Some(edge) = edge_from_scratch(d.data) {
                apply_loop_edge(&mut s.core, edge, f);
                return;
            }
            let anchor = s
                .frame_ops
                .range_sel
                .map(|(a, _)| a)
                .unwrap_or(d.data as usize);
            if anchor < n {
                s.frame_ops.range_sel = Some((anchor, f));
            }
        }
        Phase::Up => {
            if edge_from_scratch(d.data).is_some() {
                // The range as last drawn stands either way
                // (a cancelled edge drag restores nothing,
                // like the selection gesture); a clean
                // release posts the core's status line once.
                if !d.cancelled() {
                    if let Some((a, b)) = s.core.loop_range {
                        s.core.set_loop_range(a, b);
                    }
                }
                return;
            }
            if d.cancelled() {
                return;
            }
            let anchor = s
                .frame_ops
                .range_sel
                .map(|(a, _)| a)
                .unwrap_or(d.data as usize);
            if f == anchor && anchor < n {
                // Plain click: chrome's press, reconstructed —
                // go to the frame, leave no selection behind.
                s.frame_ops.range_sel = None;
                s.core.goto(anchor);
            }
        }
    }
}

/// The replacement timeline: one numbered chip per frame in a
/// horizontally scrolling row, with the time ruler (see the
/// module docs) directly above the chips and the marker row
/// (camera `◆` / graph `●`, see the module docs) directly
/// under them. Current frame accent-filled; loop-range frames
/// tinted toward the accent; selected frames outlined (white
/// on the current frame, accent elsewhere, so the outline
/// survives the accent fill).
pub fn timeline_marks(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let accent = tok::accent(0, true);
    let loop_fill = t.card.mix(accent, 0.35);
    let markers = markers_for_project(core);
    // The time ruler: one fixed-width slot per frame in
    // the chips' own geometry — a muted time label at
    // whole seconds, a short tick elsewhere — carrying
    // its own drag-to-seek (Phase K): the handler sits on
    // this row, so its points are content coordinates
    // exactly as the chips' drag reads them, and the two
    // gestures can never be confused — different elements
    // own them.
    let mut ruler = row()
        .key("timeline-ruler")
        .gap(CHIP_GAP)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| ruler_drag(s, d));
    for label in ruler_ticks(core.frame_count(), core.project.fps) {
        let mut cell = row().size(CHIP_W, RULER_H).center();
        cell = match label {
            Some(label) => cell.child(
                text(label)
                    .font(RULER_LABEL_STYLE)
                    .color(t.text_secondary)
                    .truncate(),
            ),
            None => cell.child(el().size(1.0, 5.0).fill(t.text_secondary)),
        };
        ruler = ruler.child(cell);
    }
    let mut chips = row()
        .key("timeline-chips")
        .gap(CHIP_GAP)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| timeline_drag(s, d));
    for i in 0..core.frame_count() {
        let current = i == core.current;
        let duration = core.project.frames[i].duration;
        let fill = if current {
            accent
        } else if in_loop(core.loop_range, i) {
            loop_fill
        } else {
            t.card
        };
        let mut chip = el()
            .key(format!("frame-{}", i + 1))
            .size(CHIP_W, CHIP_H)
            .radius(6.0)
            .fill(fill)
            .hover(if current { accent } else { t.control_track })
            .center()
            .child(
                text(chip_label(i, duration))
                    .font(type_::LABEL)
                    .color(if current { Color::WHITE } else { t.text }),
            );
        if in_sel(s.frame_ops.range_sel, i) {
            let ink = if current { Color::WHITE } else { accent };
            chip = chip.outline(2.0, 1.0, ink);
        }
        chips = chips.child(chip);
    }
    // The marker row: one cell per chip, same geometry as
    // the display-only row it replaces — each glyph now
    // wrapped in a transparent pressable element keyed by
    // its frame, pressing it seeks there (see the module
    // docs). The camera diamond takes the accent so it
    // reads apart from the graph dot at a glance.
    let mut marker_row = row().key("timeline-markers").gap(CHIP_GAP);
    for (i, &(camera, graph)) in markers.iter().enumerate() {
        let mut cell = row().size(CHIP_W, MARKER_H).center().gap(2.0);
        if camera {
            cell = cell.child(
                el()
                    .key(camera_marker_key(i))
                    .on_press(move |s: &mut State| goto_marker(s, i))
                    .child(text("◆").font(type_::LABEL).color(accent)),
            );
        }
        if graph {
            cell = cell.child(
                el()
                    .key(graph_marker_key(i))
                    .on_press(move |s: &mut State| goto_marker(s, i))
                    .child(text("●").font(type_::LABEL).color(t.text_secondary)),
            );
        }
        marker_row = marker_row.child(cell);
    }
    // All three rows live in one column inside the scroll
    // viewport, so they translate together: the ruler stays
    // above its chips and the markers stay under theirs at
    // any scroll offset.
    let content = col().gap(2.0).child(ruler).child(chips).child(marker_row);
    let timeline = row().key("timeline").child(content).scroll_x().w_full();
    // The loop row hangs under the scrolled timeline,
    // outside the viewport, so its buttons stay put however
    // far the chips scroll. With nothing to show (an empty
    // project) the timeline element is returned exactly as
    // it was before the row existed.
    match loop_row(s, t) {
        Some(loop_row) => col().gap(2.0).child(timeline).child(loop_row),
        None => timeline,
    }
}

/// The loop row under the timeline (Phase L, see the
/// module docs): a Set loop button while no loop range
/// exists, or the range's label with its four endpoint
/// steppers and a Clear button while one does. `None`
/// only on a zero-frame project, where there is nothing
/// to loop and nothing to clear.
fn loop_row(s: &State, t: &Theme) -> Option<El> {
    let core = &s.core;
    if core.frame_count() == 0 {
        return None;
    }
    let mut r = row().key("timeline-loop-row").gap(CHIP_GAP).center_y();
    match core.loop_range {
        None => {
            r = r.child(btn("loop-set", "Set loop", false, t, set_loop_here));
        }
        Some(range) => {
            r = r
                .child(text(loop_label(range)).font(type_::LABEL).color(t.text))
                .child(btn("loop-a-dn", "Start -", false, t, |s: &mut State| {
                    step_loop(s, Edge::Start, -1);
                }))
                .child(btn("loop-a-up", "Start +", false, t, |s: &mut State| {
                    step_loop(s, Edge::Start, 1);
                }))
                .child(btn("loop-b-dn", "End -", false, t, |s: &mut State| {
                    step_loop(s, Edge::End, -1);
                }))
                .child(btn("loop-b-up", "End +", false, t, |s: &mut State| {
                    step_loop(s, Edge::End, 1);
                }))
                .child(btn("loop-clear", "Clear", false, t, clear_loop_here));
        }
    }
    Some(r)
}

/// Kept for the integrator: the selection this row produces,
/// normalised the way the Frames section reads it.
pub fn timeline_selection(core: &EditorCore, ops: &FrameOpsState) -> Option<(usize, usize)> {
    selected_range(core, ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
    use stykanimate_rs::model::CameraKey;

    // ---- frame_at_x ----

    #[test]
    fn frame_at_x_is_none_without_frames() {
        assert_eq!(frame_at_x(0.0, CHIP_W, CHIP_GAP, 0), None);
        assert_eq!(frame_at_x(100.0, CHIP_W, CHIP_GAP, 0), None);
    }

    #[test]
    fn frame_at_x_maps_chip_spans() {
        // Chip 0: [0,30), gap [30,34), chip 1: [34,64), chip 2: [68,98).
        assert_eq!(frame_at_x(0.0, CHIP_W, CHIP_GAP, 5), Some(0));
        assert_eq!(frame_at_x(29.9, CHIP_W, CHIP_GAP, 5), Some(0));
        assert_eq!(frame_at_x(34.0, CHIP_W, CHIP_GAP, 5), Some(1));
        assert_eq!(frame_at_x(63.9, CHIP_W, CHIP_GAP, 5), Some(1));
        assert_eq!(frame_at_x(68.0, CHIP_W, CHIP_GAP, 5), Some(2));
        assert_eq!(frame_at_x(83.0, CHIP_W, CHIP_GAP, 5), Some(2));
    }

    #[test]
    fn frame_at_x_counts_the_gap_as_the_chip_before_it() {
        assert_eq!(frame_at_x(31.0, CHIP_W, CHIP_GAP, 5), Some(0));
        assert_eq!(frame_at_x(33.9, CHIP_W, CHIP_GAP, 5), Some(0));
        assert_eq!(frame_at_x(66.0, CHIP_W, CHIP_GAP, 5), Some(1));
    }

    #[test]
    fn frame_at_x_clamps_outside_the_row() {
        assert_eq!(frame_at_x(-40.0, CHIP_W, CHIP_GAP, 5), Some(0));
        assert_eq!(frame_at_x(10_000.0, CHIP_W, CHIP_GAP, 5), Some(4));
        assert_eq!(frame_at_x(f32::NAN, CHIP_W, CHIP_GAP, 5), None);
    }

    #[test]
    fn frame_at_x_honours_its_geometry_parameters() {
        // 10pt chips, no gap: stride 10.
        assert_eq!(frame_at_x(25.0, 10.0, 0.0, 4), Some(2));
        // Degenerate stride maps nothing.
        assert_eq!(frame_at_x(0.0, 0.0, 0.0, 4), None);
    }

    // ---- chip_label / duration_suffix ----

    #[test]
    fn chip_label_is_the_plain_number_at_base_rate() {
        assert_eq!(chip_label(0, 1.0), "1");
        assert_eq!(chip_label(11, 1.0), "12");
        assert_eq!(duration_suffix(1.0), None);
    }

    #[test]
    fn chip_label_carries_the_duration_suffix() {
        assert_eq!(chip_label(2, 2.0), "3 ×2");
        assert_eq!(chip_label(0, 0.5), "1 ×0.5");
        assert_eq!(chip_label(4, 0.25), "5 ×0.25");
        assert_eq!(chip_label(1, 1.25), "2 ×1.25");
        assert_eq!(chip_label(11, 8.0), "12 ×8");
        assert_eq!(duration_suffix(2.5), Some("×2.5".to_string()));
        assert_eq!(duration_suffix(0.75), Some("×0.75".to_string()));
    }

    // ---- in_loop / in_sel ----

    #[test]
    fn in_loop_is_inclusive_and_needs_a_range() {
        assert!(!in_loop(None, 0));
        let range = Some((1, 3));
        assert!(!in_loop(range, 0));
        assert!(in_loop(range, 1));
        assert!(in_loop(range, 2));
        assert!(in_loop(range, 3));
        assert!(!in_loop(range, 4));
        // A single-frame loop contains its frame.
        assert!(in_loop(Some((2, 2)), 2));
    }

    #[test]
    fn in_sel_accepts_either_anchor_order() {
        assert!(!in_sel(None, 1));
        assert!(in_sel(Some((1, 3)), 2));
        assert!(in_sel(Some((3, 1)), 2));
        assert!(in_sel(Some((3, 1)), 3));
        assert!(!in_sel(Some((3, 1)), 0));
        assert!(!in_sel(Some((3, 1)), 4));
    }

    // ---- edge_at (chip geometry: stride 34; the range
    // (1, 3) over 5 frames has its start edge at x = 34 and
    // its end edge at x = 3 * 34 + 30 = 132) ----

    #[test]
    fn edge_at_needs_a_range_frames_and_a_real_x() {
        assert_eq!(edge_at(34.0, CHIP_W, CHIP_GAP, None, 5), None);
        assert_eq!(edge_at(34.0, CHIP_W, CHIP_GAP, Some((1, 3)), 0), None);
        assert_eq!(edge_at(f32::NAN, CHIP_W, CHIP_GAP, Some((1, 3)), 5), None);
        assert_eq!(edge_at(0.0, 0.0, 0.0, Some((0, 0)), 4), None);
    }

    #[test]
    fn edge_at_grabs_within_the_zone_of_each_edge() {
        let range = Some((1, 3));
        // Start edge at 34, zone [26, 42].
        assert_eq!(edge_at(34.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::Start));
        assert_eq!(edge_at(26.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::Start));
        assert_eq!(edge_at(42.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::Start));
        // End edge at 132, zone [124, 140].
        assert_eq!(edge_at(132.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::End));
        assert_eq!(edge_at(124.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::End));
        assert_eq!(edge_at(140.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::End));
    }

    #[test]
    fn edge_at_misses_away_from_the_edges() {
        let range = Some((1, 3));
        // Just outside both zones.
        assert_eq!(edge_at(43.0, CHIP_W, CHIP_GAP, range, 5), None);
        assert_eq!(edge_at(141.0, CHIP_W, CHIP_GAP, range, 5), None);
        // Deep inside the start chip, far from its left edge.
        assert_eq!(edge_at(54.0, CHIP_W, CHIP_GAP, range, 5), None);
        // The middle chip's centre.
        assert_eq!(edge_at(83.0, CHIP_W, CHIP_GAP, range, 5), None);
    }

    #[test]
    fn edge_at_single_frame_loop_has_two_distinct_edges() {
        // Range (2, 2): start edge at 68, end edge at 98,
        // centre 83 is 15pt from both — outside both zones.
        let range = Some((2, 2));
        assert_eq!(edge_at(68.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::Start));
        assert_eq!(edge_at(98.0, CHIP_W, CHIP_GAP, range, 5), Some(Edge::End));
        assert_eq!(edge_at(83.0, CHIP_W, CHIP_GAP, range, 5), None);
    }

    #[test]
    fn edge_at_normalises_hand_built_ranges() {
        // Unordered endpoints read as (1, 3).
        assert_eq!(edge_at(34.0, CHIP_W, CHIP_GAP, Some((3, 1)), 5), Some(Edge::Start));
        // An end past the timeline clamps to the last chip:
        // (0, 99) over 5 frames ends at chip 4, edge 166.
        assert_eq!(edge_at(0.0, CHIP_W, CHIP_GAP, Some((0, 99)), 5), Some(Edge::Start));
        assert_eq!(edge_at(166.0, CHIP_W, CHIP_GAP, Some((0, 99)), 5), Some(Edge::End));
    }

    // ---- loop_after_edge_drag ----

    #[test]
    fn loop_after_edge_drag_moves_one_endpoint() {
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::Start, 2, 5), Some((2, 3)));
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::End, 2, 5), Some((1, 2)));
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::Start, 0, 5), Some((0, 3)));
    }

    #[test]
    fn loop_after_edge_drag_crossing_swaps_the_endpoints() {
        // The core orders endpoints, so dragging an edge
        // past the other one hands the range over.
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::Start, 4, 5), Some((3, 4)));
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::End, 0, 5), Some((0, 1)));
    }

    #[test]
    fn loop_after_edge_drag_meeting_is_a_single_frame_loop_not_a_clear() {
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::Start, 3, 5), Some((3, 3)));
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::End, 1, 5), Some((1, 1)));
    }

    #[test]
    fn loop_after_edge_drag_clamps_and_needs_a_range() {
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::End, 99, 5), Some((1, 4)));
        assert_eq!(loop_after_edge_drag(None, Edge::Start, 2, 5), None);
        assert_eq!(loop_after_edge_drag(Some((1, 3)), Edge::Start, 2, 0), None);
    }

    // ---- edge scratch sentinels ----

    #[test]
    fn edge_scratch_roundtrips_and_frames_are_not_edges() {
        assert_eq!(edge_from_scratch(edge_scratch(Edge::Start)), Some(Edge::Start));
        assert_eq!(edge_from_scratch(edge_scratch(Edge::End)), Some(Edge::End));
        assert_eq!(edge_from_scratch(0.0), None);
        assert_eq!(edge_from_scratch(3.0), None);
    }

    // ---- the gesture's core writes, end to end on a real
    // core (the handler's exact call sequence, minus the
    // runtime-only `Drag`) ----

    #[test]
    fn edge_drag_writes_the_field_per_move_and_posts_status_on_release() {
        let mut core = EditorCore::new();
        core.set_loop_range(1, 3);
        // Down: a press on the start edge grabs it.
        assert_eq!(
            edge_at(34.0, CHIP_W, CHIP_GAP, core.loop_range, core.frame_count()),
            Some(Edge::Start)
        );
        // Move: the field moves, the status line does not
        // (it is rewritten once, on release).
        let status_before = core.status.clone();
        apply_loop_edge(&mut core, Edge::Start, 2);
        assert_eq!(core.loop_range, Some((2, 3)));
        assert_eq!(core.status, status_before);
        // Up: the handler's single set_loop_range posts the
        // core's own status for the final range.
        let (a, b) = core.loop_range.unwrap();
        core.set_loop_range(a, b);
        assert_eq!(core.status, "Looping frames 3–4.");
        // Dragging the end onto the start leaves a
        // single-frame loop, not a cleared range.
        apply_loop_edge(&mut core, Edge::End, 2);
        assert_eq!(core.loop_range, Some((2, 2)));
    }

    // ---- frame markers (the demo project has 5 frames;
    // expectations below are computed by hand from the
    // fixture keys, with `f32::round` — halves away from
    // zero — as the graph convention) ----

    /// The marker fixture: a camera key on frame 2 only
    /// (plus one hand-pushed at frame 99, past the end),
    /// and two graph channels whose keys sit at fractional
    /// times:
    ///
    /// - channel A (RootX): 0.4 → frame 0, 2.5 → frame 3,
    ///   3.6 → frame 4
    /// - channel B (Angle 1): 2.49 → frame 2, 9.2 → rounds
    ///   to 9 (past the end, marks nothing), −1.2 → rounds
    ///   to −1 (before the start, marks nothing)
    fn marked_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.set_camera_key(2, 480.0, 360.0, 1.5);
        core.project.camera.push(CameraKey {
            frame: 99,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            angle: 0.0,
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.4, 10.0), Key::flat(2.5, 20.0), Key::flat(3.6, 30.0)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(2.49, 5.0), Key::flat(9.2, 6.0), Key::flat(-1.2, 7.0)],
        });
        core
    }

    #[test]
    fn markers_for_project_empty_project_is_all_false() {
        // The demo project carries no camera keys and no
        // channels, so all five frames are unmarked.
        let core = EditorCore::new();
        assert_eq!(core.frame_count(), 5);
        assert_eq!(markers_for_project(&core), vec![(false, false); 5]);
        assert_eq!(frame_markers(&core, 0), (false, false));
        assert_eq!(frame_markers(&core, 4), (false, false));
    }

    #[test]
    fn markers_for_project_maps_camera_and_graph_keys() {
        let core = marked_core();
        assert_eq!(
            markers_for_project(&core),
            vec![
                (false, true),  // 0: graph 0.4
                (false, false), // 1: nothing
                (true, true),   // 2: camera + graph 2.49
                (false, true),  // 3: graph 2.5 rounds up
                (false, true),  // 4: graph 3.6
            ]
        );
        // The single-frame helper agrees, and a frame past
        // the end reports neither marker even though a
        // camera key is stored there.
        assert_eq!(frame_markers(&core, 2), (true, true));
        assert_eq!(frame_markers(&core, 1), (false, false));
        assert_eq!(frame_markers(&core, 3), (false, true));
        assert_eq!(frame_markers(&core, 99), (false, false));
    }

    #[test]
    fn graph_key_frame_rounds_like_the_graph_seek() {
        assert_eq!(graph_key_frame(0.4, 5), Some(0));
        assert_eq!(graph_key_frame(0.5, 5), Some(1)); // halves away from zero
        assert_eq!(graph_key_frame(2.49, 5), Some(2));
        assert_eq!(graph_key_frame(2.5, 5), Some(3));
        assert_eq!(graph_key_frame(4.4, 5), Some(4));
        // Rounds onto or past the end: skipped, not clamped
        // onto the last frame.
        assert_eq!(graph_key_frame(4.5, 5), None);
        assert_eq!(graph_key_frame(9.2, 5), None);
        // −0.4 rounds to −0.0, which is frame 0; −0.6 rounds
        // to −1, before the start.
        assert_eq!(graph_key_frame(-0.4, 5), Some(0));
        assert_eq!(graph_key_frame(-0.6, 5), None);
        // No frames, or a non-finite time, maps nowhere.
        assert_eq!(graph_key_frame(1.0, 0), None);
        assert_eq!(graph_key_frame(f32::NAN, 5), None);
        assert_eq!(graph_key_frame(f32::INFINITY, 5), None);
    }

    // ---- time ruler (Phase J): labels only at whole
    // seconds, counted with the project rate rounded to an
    // integer and floored at one frame per second ----

    #[test]
    fn ruler_label_marks_whole_seconds_at_12_fps() {
        assert_eq!(ruler_label(0, 12.0), Some("0:00".to_string()));
        assert_eq!(ruler_label(12, 12.0), Some("0:01".to_string()));
        assert_eq!(ruler_label(24, 12.0), Some("0:02".to_string()));
        assert_eq!(ruler_label(1, 12.0), None);
        assert_eq!(ruler_label(13, 12.0), None);
    }

    #[test]
    fn ruler_label_rounds_the_rate_and_floors_it_at_one() {
        // 12.4 rounds to 12: the second boundaries stay on
        // frames 12 and 24.
        assert_eq!(ruler_label(12, 12.4), Some("0:01".to_string()));
        assert_eq!(ruler_label(24, 12.4), Some("0:02".to_string()));
        // 12.5 rounds to 13 (halves away from zero): frame
        // 12 is no longer a boundary, frame 13 is.
        assert_eq!(ruler_label(12, 12.5), None);
        assert_eq!(ruler_label(13, 12.5), Some("0:01".to_string()));
        assert_eq!(ruler_label(26, 12.5), Some("0:02".to_string()));
        // A rate under half a frame per second floors to
        // one, so every frame starts a second; a non-finite
        // rate degrades to the same floor rather than
        // panicking.
        assert_eq!(ruler_label(1, 0.4), Some("0:01".to_string()));
        assert_eq!(ruler_label(2, f32::NAN), Some("0:02".to_string()));
    }

    #[test]
    fn ruler_label_formats_minutes_unpadded_and_seconds_padded() {
        // One frame per second: the frame index is the time
        // in seconds.
        assert_eq!(ruler_label(65, 1.0), Some("1:05".to_string()));
        assert_eq!(ruler_label(754, 1.0), Some("12:34".to_string()));
    }

    #[test]
    fn ruler_ticks_covers_every_frame_and_labels_only_boundaries() {
        let ticks = ruler_ticks(25, 12.0);
        assert_eq!(ticks.len(), 25);
        assert_eq!(ticks[0], Some("0:00".to_string()));
        assert_eq!(ticks[12], Some("0:01".to_string()));
        assert_eq!(ticks[24], Some("0:02".to_string()));
        assert!(ticks
            .iter()
            .enumerate()
            .all(|(frame, tick)| frame % 12 == 0 || tick.is_none()));
        assert_eq!(ruler_ticks(0, 12.0), Vec::<Option<String>>::new());
    }

    #[test]
    fn ruler_ticks_on_the_demo_label_only_frame_zero() {
        // The demo project: five frames at 12 fps — not one
        // whole second long, so frame 0 is the only label.
        let core = EditorCore::new();
        assert_eq!(core.frame_count(), 5);
        assert_eq!(core.project.fps, 12.0);
        assert_eq!(
            ruler_ticks(core.frame_count(), core.project.fps),
            vec![Some("0:00".to_string()), None, None, None, None]
        );
    }

    // ---- ruler drag-to-seek (Phase K): the slot pitch is
    // CHIP_W + CHIP_GAP = 34, the chips' own geometry ----

    #[test]
    fn ruler_frame_at_x_maps_slots_at_the_chip_pitch() {
        // Slot 0 spans [0, 34): its chip and the gap after
        // it both seek to frame 0.
        assert_eq!(ruler_frame_at_x(0.0, 5), 0);
        assert_eq!(ruler_frame_at_x(29.9, 5), 0);
        assert_eq!(ruler_frame_at_x(33.9, 5), 0);
        // Exactly one pitch is the first point of slot 1.
        assert_eq!(ruler_frame_at_x(34.0, 5), 1);
        assert_eq!(ruler_frame_at_x(49.0, 5), 1);
        assert_eq!(ruler_frame_at_x(68.0, 5), 2);
        // The last frame's own slot still reads as itself.
        assert_eq!(ruler_frame_at_x(4.0 * 34.0, 5), 4);
    }

    #[test]
    fn ruler_frame_at_x_clamps_and_guards_bad_input() {
        // A captured drag far past the row's end clamps to
        // the last frame instead of running off the
        // timeline.
        assert_eq!(ruler_frame_at_x(10_000.0, 5), 4);
        assert_eq!(ruler_frame_at_x(135.9, 5), 3);
        // Negative and non-finite x read as frame 0.
        assert_eq!(ruler_frame_at_x(-1.0, 5), 0);
        assert_eq!(ruler_frame_at_x(-40.0, 5), 0);
        assert_eq!(ruler_frame_at_x(f32::NAN, 5), 0);
        assert_eq!(ruler_frame_at_x(f32::INFINITY, 5), 0);
        assert_eq!(ruler_frame_at_x(f32::NEG_INFINITY, 5), 0);
        // No frames: frame 0 is the only answer (and the
        // core's `goto` ignores it there).
        assert_eq!(ruler_frame_at_x(0.0, 0), 0);
        assert_eq!(ruler_frame_at_x(100.0, 0), 0);
    }

    #[test]
    fn ruler_seek_moves_the_playhead_and_touches_nothing_else() {
        // `Drag` cannot be constructed outside the runtime
        // (see the builder smoke test below), so this runs
        // the handler's exact call sequence — convert with
        // `ruler_frame_at_x`, then `core.goto` — on a real
        // shell state, the same idiom the edge-drag test
        // uses for the chips' gesture.
        let mut state = demo_state();
        state.core.set_loop_range(1, 3);
        state.frame_ops.range_sel = Some((3, 1));
        state.core.playing = true;
        // A press/drag landing in slot 3 (x in [102, 136))
        // seeks there — and writes nothing else: the play
        // state, the range selection and the loop range
        // all stand exactly as they were.
        let frame = ruler_frame_at_x(110.0, state.core.frame_count());
        state.core.goto(frame);
        assert_eq!(state.core.current, 3);
        assert!(state.core.playing);
        assert_eq!(state.frame_ops.range_sel, Some((3, 1)));
        assert_eq!(state.core.loop_range, Some((1, 3)));
        // Paused, the same seek moves the playhead and
        // stays paused — a ruler press never starts
        // playback.
        state.core.playing = false;
        let frame = ruler_frame_at_x(40.0, state.core.frame_count());
        state.core.goto(frame);
        assert_eq!(state.core.current, 1);
        assert!(!state.core.playing);
    }

    #[test]
    fn timeline_marks_builds_with_the_ruler_drag_attached() {
        // The builder smoke idiom: with the drag handler on
        // the ruler row, the whole timeline column still
        // builds — for the plain demo, and for a timeline
        // long enough that the ruler carries several
        // second labels (25 frames at 12 fps: labels on
        // frames 0, 12 and 24).
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        let _ = timeline_marks(&state, &t);
        for _ in 0..20 {
            state.core.add_frame();
        }
        assert_eq!(state.core.frame_count(), 25);
        let _ = timeline_marks(&state, &t);
    }

    // ---- clickable markers (Phase I): the keys the row
    // renders and the seek a press performs ----

    #[test]
    fn marker_keys_use_the_engine_frame_index() {
        assert_eq!(camera_marker_key(0), "marker-cam-0");
        assert_eq!(camera_marker_key(2), "marker-cam-2");
        assert_eq!(graph_marker_key(2), "marker-graph-2");
        assert_eq!(graph_marker_key(4), "marker-graph-4");
    }

    #[test]
    fn markers_row_keys_the_camera_marker_for_frame_2() {
        // `El` has no public tree-walk, so the built row's
        // keys are asserted through the helpers the builder
        // itself calls, over the same markers vector it
        // renders: build the row for the fixture (camera key
        // on frame 2), then check its keyed frames.
        let mut state = demo_state();
        state.core = marked_core();
        let t = rune::tok::theme(true);
        let _ = timeline_marks(&state, &t);
        let markers = markers_for_project(&state.core);
        let mut camera_frames = Vec::new();
        for (i, &(camera, _)) in markers.iter().enumerate() {
            if camera {
                camera_frames.push(i);
            }
        }
        assert_eq!(camera_frames, vec![2]);
        assert_eq!(camera_marker_key(camera_frames[0]), "marker-cam-2");
        // Frame 2 carries a graph key too, keyed alongside
        // the camera marker in the same cell.
        assert!(markers[2].1);
        assert_eq!(graph_marker_key(2), "marker-graph-2");
        // A frame with neither key gets no marker element.
        assert_eq!(markers[1], (false, false));
    }

    #[test]
    fn pressing_a_marker_seeks_to_its_frame() {
        let mut state = demo_state();
        state.core = marked_core();
        assert_eq!(state.core.current, 0);
        goto_marker(&mut state, 2);
        assert_eq!(state.core.current, 2);
        goto_marker(&mut state, 4);
        assert_eq!(state.core.current, 4);
    }

    // ---- builder smoke test (the drag handler itself is
    // untestable from here: `Drag` has crate-private fields,
    // so only the real runtime can construct one — the
    // helpers above carry its mapping logic) ----

    fn demo_state() -> State {
        State {
            core: EditorCore::new(),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: FrameOpsState::default(),
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
        }
    }

    #[test]
    fn timeline_marks_builds_for_plain_looped_and_selected_states() {
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        let _ = timeline_marks(&state, &t);
        // Loop tint + a held frame + a selection to outline.
        state.core.set_loop_range(1, 3);
        state.core.project.frames[2].duration = 2.0;
        state.frame_ops.range_sel = Some((3, 1));
        let _ = timeline_marks(&state, &t);
        // The selection the Frames section will read back.
        assert_eq!(timeline_selection(&state.core, &state.frame_ops), Some((1, 3)));
        // The marker row builds with both glyph kinds live.
        state.core.project.set_camera_key(1, 480.0, 360.0, 1.0);
        state.core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(2.5, 1.0)],
        });
        let _ = timeline_marks(&state, &t);
    }

    // ---- loop row (Phase L): Set loop from the playhead,
    // per-endpoint steppers through the core's own op, and
    // Clear through `clear_loop_range` ----

    #[test]
    fn loop_set_range_spans_four_frames_from_current() {
        assert_eq!(loop_set_range(0, 20), Some((0, 4)));
        assert_eq!(loop_set_range(3, 20), Some((3, 7)));
        assert_eq!(loop_set_range(15, 20), Some((15, 19)));
    }

    #[test]
    fn loop_set_range_clamps_near_the_end() {
        // The end clamps to the last frame: the loop is
        // shorter near the end, and a single frame on the
        // last frame itself.
        assert_eq!(loop_set_range(3, 5), Some((3, 4)));
        assert_eq!(loop_set_range(4, 5), Some((4, 4)));
        assert_eq!(loop_set_range(0, 1), Some((0, 0)));
        // No frames, no range to ask for.
        assert_eq!(loop_set_range(0, 0), None);
    }

    #[test]
    fn set_loop_button_loops_from_the_playhead() {
        let mut state = demo_state();
        state.core.goto(1);
        set_loop_here(&mut state);
        assert_eq!(state.core.loop_range, Some((1, 4)));
        assert_eq!(state.core.status, "Looping frames 2–5.");
        // On the last frame the set loop is that frame
        // alone.
        state.core.clear_loop_range();
        state.core.goto(4);
        set_loop_here(&mut state);
        assert_eq!(state.core.loop_range, Some((4, 4)));
    }

    #[test]
    fn loop_after_step_moves_the_named_endpoint() {
        let range = (1, 3);
        assert_eq!(loop_after_step(range, Edge::Start, -1), (0, 3));
        assert_eq!(loop_after_step(range, Edge::Start, 1), (2, 3));
        assert_eq!(loop_after_step(range, Edge::End, -1), (1, 2));
        assert_eq!(loop_after_step(range, Edge::End, 1), (1, 4));
        // Only the sign of `dir` matters.
        assert_eq!(loop_after_step(range, Edge::Start, -5), (0, 3));
        // A hand-built unordered range steps its earlier
        // endpoint when Start is named.
        assert_eq!(loop_after_step((3, 1), Edge::Start, 1), (2, 3));
    }

    #[test]
    fn loop_after_step_saturates_at_frame_zero() {
        // The start cannot step below frame 0 (the op
        // never sees a wrapped usize); the end of a
        // single-frame loop at 0 saturates the same way.
        assert_eq!(loop_after_step((0, 3), Edge::Start, -1), (0, 3));
        assert_eq!(loop_after_step((0, 0), Edge::End, -1), (0, 0));
    }

    #[test]
    fn loop_steppers_move_the_right_endpoint_through_the_op() {
        let mut state = demo_state();
        state.core.set_loop_range(1, 3);
        step_loop(&mut state, Edge::Start, 1);
        assert_eq!(state.core.loop_range, Some((2, 3)));
        assert_eq!(state.core.status, "Looping frames 3–4.");
        step_loop(&mut state, Edge::End, -1);
        assert_eq!(state.core.loop_range, Some((2, 2)));
        // Stepping the end past the start hands the pair
        // to `set_loop_range` unordered; the op orders
        // them, so the roles swap — the end the user is
        // stepping is now the range's start side.
        step_loop(&mut state, Edge::End, -1);
        assert_eq!(state.core.loop_range, Some((1, 2)));
        // Stepping the end beyond the last frame clamps
        // there, in the op.
        step_loop(&mut state, Edge::End, 1);
        step_loop(&mut state, Edge::End, 1);
        step_loop(&mut state, Edge::End, 1);
        assert_eq!(state.core.loop_range, Some((1, 4)));
    }

    #[test]
    fn loop_steppers_without_a_range_do_nothing() {
        let mut state = demo_state();
        assert_eq!(state.core.loop_range, None);
        let status_before = state.core.status.clone();
        step_loop(&mut state, Edge::Start, 1);
        step_loop(&mut state, Edge::End, -1);
        assert_eq!(state.core.loop_range, None);
        assert_eq!(state.core.status, status_before);
    }

    #[test]
    fn loop_clear_button_empties_the_range() {
        let mut state = demo_state();
        state.core.set_loop_range(1, 3);
        clear_loop_here(&mut state);
        assert_eq!(state.core.loop_range, None);
        // The op's own status line, word for word.
        assert_eq!(state.core.status, "Loop range cleared.");
    }

    #[test]
    fn loop_label_reads_one_based_in_timeline_order() {
        assert_eq!(loop_label((1, 3)), "Loop 2–4");
        assert_eq!(loop_label((0, 4)), "Loop 1–5");
        assert_eq!(loop_label((2, 2)), "Loop 3–3");
        // A hand-built unordered range reads ordered.
        assert_eq!(loop_label((3, 1)), "Loop 2–4");
    }

    #[test]
    fn loop_row_is_absent_on_an_empty_project() {
        // No frames: `loop_row` has nothing to offer, the
        // Set button's handler is a no-op there, and the
        // whole timeline still builds with the row absent.
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        state.core.project.frames.clear();
        assert_eq!(state.core.frame_count(), 0);
        assert!(loop_row(&state, &t).is_none());
        set_loop_here(&mut state);
        assert_eq!(state.core.loop_range, None);
        let _ = timeline_marks(&state, &t);
        // With frames, the row exists in both its forms.
        let mut state = demo_state();
        assert!(loop_row(&state, &t).is_some());
        state.core.set_loop_range(1, 3);
        assert!(loop_row(&state, &t).is_some());
        let _ = timeline_marks(&state, &t);
    }
}
