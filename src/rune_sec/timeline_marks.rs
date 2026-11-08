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
//! headless tests find the chips by those keys. One key is
//! added: `timeline-chips` on the inner content row that
//! carries the drag handler (see below).
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

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;

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
/// horizontally scrolling row. Current frame accent-filled;
/// loop-range frames tinted toward the accent; selected
/// frames outlined (white on the current frame, accent
/// elsewhere, so the outline survives the accent fill).
pub fn timeline_marks(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let accent = tok::accent(0, true);
    let loop_fill = t.card.mix(accent, 0.35);
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
    row()
        .key("timeline")
        .child(chips)
        .scroll_x()
        .w_full()
}

/// Kept for the integrator: the selection this row produces,
/// normalised the way the Frames section reads it.
pub fn timeline_selection(core: &EditorCore, ops: &FrameOpsState) -> Option<(usize, usize)> {
    selected_range(core, ops)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
