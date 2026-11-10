//! The subtitle lane for the Rune editor shell (Wave P,
//! P32) — the project's subtitle cues drawn as blocks
//! inside the timeline card, under the clips lane and the
//! dopesheet: one block per cue, spanning the frames the
//! cue covers, its text inside the block where the block
//! is wide enough to carry it.
//!
//! ## Geometry (reused, never re-derived)
//!
//! A cue is a span, and the timeline already has exactly
//! one span→pixels maths: [`span_rect`] in
//! `timeline_marks`, which the clips lane's audio/FX bars
//! are drawn with. [`cue_rect`] delegates to it verbatim —
//! the frame window is carried in a [`ClipSpan`], whose
//! `kind` field `span_rect` never reads (its body consumes
//! only `from_frame` / `to_frame` / the frame count; the
//! kind-driven consumers — `span_rgb`, `clip_lane_rows` —
//! are NOT fed cue spans: the colour here is the lane's
//! own and the row packing below works on the drawn rects
//! directly). The pitch ([`CHIP_PITCH`]) is imported from
//! `timeline_marks`, never re-typed, so a cue block sits
//! under its frames exactly as a clip bar does — the lane
//! has NO name gutter, for the clips lane's reason: a span
//! lane that shifts its spans off their frames is lying.
//!
//! One pitch is all there is: the timeline has no zoom
//! levels (its chip geometry is the fixed
//! `CHIP_W`/[`CHIP_GAP`] pair; zoom in this shell is a
//! stage-view concept). What varies is the frame count,
//! which moves the clamp at the timeline's right edge —
//! the pins below cover both.
//!
//! ## Rows
//!
//! Cues may overlap (the core's `subtitles_at` returns
//! every live cue, stacked), so one row cannot hold them
//! all: blocks are first-fit onto sub-rows in stored
//! order, judged on the DRAWN rects — a cue takes the
//! first row whose last block ends at or before it starts,
//! so blocks merely touching at an edge still share a
//! row. That is `clip_lane_rows`' rule, re-applied to cue
//! rects (the clips version splits rows by span kind;
//! cues have one kind, so no split exists to mirror). A
//! cue whose rect is zero-width (degenerate window, or
//! wholly past the last frame) draws nothing and takes
//! no row — the clips lane's zero-width rule.
//!
//! ## Interaction: click-to-select, drag-to-move
//!
//! A block press selects its cue and seeks the playhead
//! to the cue's first frame ([`press_cue`] — the
//! dopesheet cell's select-plus-goto, with the selection
//! half landing on the shell's subtitle selection:
//! `State::subtitle_selected`, an `Option<usize>` into
//! `project.subtitles`, the `State::fx_selected` /
//! `stroke_selected` / `particle_selected` idiom). That
//! field is the ONE channel the P31 Subtitles panel reads
//! — the panel join is a shared field, not a conversion.
//! Selection and playhead are runtime state, so a press
//! spends no undo step, exactly like every other timeline
//! press. A stale selection (the project shrank under it)
//! tints nothing — [`selection_is_live`] is the guard the
//! panel shares (the audio panel's helper of the same
//! name and shape).
//!
//! Drag-to-move (Wave R, R3 — P32's named remainder,
//! now built): a block carries the lane's one `on_drag`;
//! the framework fires its `on_press` on release, AFTER
//! the drag's Up phase, so a block cannot carry both —
//! the click lives in the gesture's Down phase instead
//! ([`drag_down`] runs [`press_cue`]), the timeline
//! chips' own settlement. The gesture record
//! ([`CueDrag`]: cue index, grab zone, the window as
//! grabbed, an identity copy of the cue's text, whether
//! the gesture's snapshot is taken) rides a
//! `thread_local`, the timeline's `PRESSED_CHIP`
//! precedent — gesture state with the gesture's own
//! lifetime, needing no new `State` field. The factored
//! handlers ([`drag_down`] / [`drag_apply`] / [`drag_up`])
//! take the record as a parameter, the camera dial's
//! testable-composition shape.
//!
//! The write discipline is the dial's, rebuilt for
//! blocks (the gap P32 named): the pointer's travel
//! (`Drag::dx`, window points since the press,
//! independent of the block moving under it) becomes a
//! whole-frame delta against the window AS GRABBED, so
//! no rounding accumulates across Move events. The first
//! Move that actually changes the window takes the
//! gesture's ONE `EditorCore::snapshot`; later Moves
//! write the cue's window in place (no re-sort mid-
//! gesture, so the grabbed index stays the cue's index
//! throughout). On a clean release the cue is re-settled
//! into the core's sorted order by the update op's own
//! remove-and-insert rule, the selection follows it to
//! its post-sort index (the panel's discipline), and the
//! status line takes the op's own "Subtitle N updated."
//! voice — a completed drag is exactly ONE undo step. A
//! cancelled release rolls the staged snapshot back with
//! one `undo()` (the dial's and the stage drag's rule),
//! netting zero steps and the pre-drag project. A grab
//! within [`EDGE_PX`] of either end resizes that edge
//! instead of moving (the timeline's own edge measure);
//! a body grab preserves the duration exactly. Frame
//! numbers clamp only where validity forces it (no
//! negative start, no edge crossing its sibling) — past
//! the timeline's end is legal, exactly as the core ops
//! leave it.
//!
//! While a gesture is in flight the lane pins its row
//! assignment to the windows as grabbed ([`lane_rows`]):
//! the runtime captures a gesture by element path, and a
//! block whose sub-row changed mid-drag (because its
//! live window crossed a neighbour) would change path
//! and strand the capture. Rows are the only path input
//! the drag can move — sibling order inside a row is
//! stored order, which the no-re-sort rule freezes — so
//! pinning the rows pins the path.
//!
//! ## Mounting and labels
//!
//! The lane exists only when the project HAS cues (the
//! clips lane's `Option` discipline): a project without
//! subtitles spends no timeline height on an empty strip.
//! The header label is the dopesheet's idiom —
//! `design::section_label` over the lane, the frame
//! readout (the dopesheet's own [`frame_readout`]) at the
//! right. Since Wave R the word itself goes through the
//! catalogue like the dopesheet's header:
//! `i18n_wire::tr(core, "subtitles")` (the `subtitles`
//! key is R4's, landed in the same wave).
//!
//! ## Integrator wiring (landed in Wave Q)
//!
//! P32's filed actions are all in the tree: `pub mod
//! subtitle_lane;` in `rune_sec/mod.rs`,
//! `State::subtitle_selected` plus its `init` seed and
//! project-swap clearing in `src/main.rs`, and the lane's
//! append in `timeline_marks` after the dopesheet, gated
//! on `!collapsed` only (the clips lane's gate — the lane
//! must be visible in the Audio workspace, where the P31
//! panel lives, not just Animate). The drag needs NO new
//! wiring: its record is the module's `thread_local`
//! above.
//!
//! ## Keys (all additive, Wave P)
//!
//! `subtitle-lane` on the lane root, [`block_key`]
//! (`subtitle-cue-{i}`, the cue's stored index) on each
//! block.

use std::cell::RefCell;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Drag, El, Phase, Tx};
use stykanimate_rs::subtitles::SubtitleCue;

use crate::rune_sec::design;
use crate::rune_sec::dopesheet::frame_readout;
use crate::rune_sec::timeline_marks::{
    span_rect, timeline_width, ClipSpan, SpanKind, CHIP_GAP, CHIP_PITCH, EDGE_PX,
};
use crate::State;

/// Height of the lane's header row ("Subtitles" + the
/// frame readout): the dopesheet header's measure
/// ([`crate::rune_sec::dopesheet::DOPE_HEADER_H`]) — one
/// LABEL line plus air.
pub const SUB_HEADER_H: f32 = 18.0;

/// Height of one block sub-row — and of the blocks in
/// it: a LABEL line of cue text plus breathing room,
/// where the clips lane's 8 pt bars carry no text at
/// all. One point slimmer than the dopesheet's rows
/// (20), which must also fit a press diamond.
pub const SUB_ROW_H: f32 = 18.0;

/// Vertical gap between two stacked block sub-rows: the
/// clips lane's [`crate::rune_sec::timeline_marks::CLIP_ROW_GAP`]
/// measure.
pub const SUB_ROW_GAP: f32 = 2.0;

/// The narrowest block, in points, that carries its
/// cue's text: two frames' extent — a one-frame block
/// (30 pt) is a tick, and text truncated into a tick
/// would show an ellipsis and nothing else, which says
/// less than the block alone. At two frames the block
/// fits a short word whole or a longer cue's readable
/// start.
pub const SUB_TEXT_MIN_W: f32 = 2.0 * CHIP_PITCH - CHIP_GAP;

/// A cue block's key: `subtitle-cue-{i}`, `i` the cue's
/// index in `project.subtitles` (stored order — the
/// index the selection and the panel both speak).
pub fn block_key(i: usize) -> String {
    format!("subtitle-cue-{i}")
}

/// A cue's rectangle in timeline points, as `[x, width]`:
/// the cue's half-open window `start_frame..end_frame`
/// through the timeline's own span maths ([`span_rect`],
/// delegated to verbatim — see the module docs for the
/// carrier note). Both edges clamp to the timeline's
/// extent; a degenerate window, a window wholly past the
/// last frame, or a zero-frame project gives width 0.
pub fn cue_rect(cue: &SubtitleCue, frame_count: usize) -> [f32; 2] {
    let carrier = ClipSpan {
        kind: SpanKind::Audio,
        from_frame: cue.start_frame as f32,
        to_frame: cue.end_frame as f32,
        muted_or_hidden: false,
    };
    span_rect(&carrier, frame_count)
}

/// Whether a block of `width` points carries its cue's
/// text (see [`SUB_TEXT_MIN_W`]).
pub fn block_shows_text(width: f32) -> bool {
    width >= SUB_TEXT_MIN_W
}

/// Which sub-row each cue paints on: first-fit in stored
/// order, judged on the drawn [`cue_rect`]s (a cue takes
/// the first row whose last block ends at or before the
/// cue starts, so edge-touching cues share a row).
/// `None` for a cue that draws nothing (zero width): it
/// takes no row.
pub fn cue_rows(cues: &[SubtitleCue], frame_count: usize) -> Vec<Option<usize>> {
    let mut rows: Vec<Option<usize>> = vec![None; cues.len()];
    let mut ends: Vec<f32> = Vec::new();
    for (i, cue) in cues.iter().enumerate() {
        let [x, w] = cue_rect(cue, frame_count);
        if w <= 0.0 {
            continue;
        }
        let end = x + w;
        let local = match ends.iter().position(|&e| e <= x) {
            Some(j) => {
                ends[j] = end;
                j
            }
            None => {
                ends.push(end);
                ends.len() - 1
            }
        };
        rows[i] = Some(local);
    }
    rows
}

/// How many sub-rows `rows` (from [`cue_rows`]) needs:
/// the highest assigned row plus one, or 0 when no cue
/// draws.
pub fn cue_row_count(rows: &[Option<usize>]) -> usize {
    rows.iter().flatten().max().map_or(0, |m| m + 1)
}

/// Whether `selected` still names a cue in a project
/// with `cue_count` cues — the audio panel's
/// `selection_is_live`, same name and shape, so the P31
/// panel guards its reads the way the audio panel does.
pub fn selection_is_live(selected: Option<usize>, cue_count: usize) -> bool {
    selected.is_some_and(|i| i < cue_count)
}

/// What a block press does (factored out of the closure
/// so it is unit-testable without a runtime gesture —
/// the timeline's press-handler discipline): select cue
/// `i` on the shell's subtitle selection, and seek the
/// playhead to the cue's first frame. `EditorCore::goto`
/// is bounds-safe, so a cue starting past the (shrunken)
/// end selects without moving the playhead. A stale `i`
/// (past the last cue) does nothing at all — no selection
/// is better than a selection naming no cue.
pub fn press_cue(s: &mut State, i: usize) {
    let Some(cue) = s.core.project.subtitles.get(i) else {
        return;
    };
    let start = cue.start_frame;
    s.subtitle_selected = Some(i);
    s.core.goto(start);
}

// ---------------------------------------------------------------------
// Drag-to-move (Wave R, R3): the gesture composition the
// module docs describe. The handlers are factored to take
// the gesture record as a parameter (the camera dial's
// `dial_gesture` shape), so every rule below is pinned by
// unit tests driving them directly; `block_drag` is the
// thin runtime adapter over the module's `thread_local`.
// ---------------------------------------------------------------------

/// Which part of a block a grab landed on: within
/// [`EDGE_PX`] of the left edge resizes the cue's start,
/// within [`EDGE_PX`] of the right edge its end, and
/// anywhere between moves the whole window. The measure
/// is the timeline's own loop-edge tolerance, imported
/// from `timeline_marks`, never re-typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMode {
    Body,
    Start,
    End,
}

/// The grab zone for a press at block-local `x` in a
/// block `w` points wide. The left edge is read first
/// (the loop edges' nearer-edge tie cannot arise: a
/// block is at least one chip wide, wider than both
/// tolerances together).
pub fn grab_mode(x: f32, w: f32) -> DragMode {
    if x <= EDGE_PX {
        DragMode::Start
    } else if x >= w - EDGE_PX {
        DragMode::End
    } else {
        DragMode::Body
    }
}

/// One in-flight cue drag: everything the gesture's
/// later phases need that the pointer alone cannot say.
/// `text` is the grabbed cue's identity — between two
/// phases of one gesture another surface (the panel, a
/// live-MCP call) can rewrite the cue list, and an index
/// alone would then name a different cue; a phase that
/// finds a different text at `index` abandons the
/// gesture without writing (see [`drag_apply`]).
#[derive(Clone, Debug, PartialEq)]
pub struct CueDrag {
    /// The grabbed cue's index in `project.subtitles`.
    /// It stays the cue's index for the gesture's whole
    /// life: nothing re-sorts the list mid-gesture.
    pub index: usize,
    /// The grab zone the press landed in.
    pub mode: DragMode,
    /// The cue's window as grabbed; every phase computes
    /// its target from THIS window plus the pointer's
    /// total travel, so rounding never accumulates.
    pub orig_start: usize,
    pub orig_end: usize,
    /// The grabbed cue's text (identity guard, above).
    pub text: String,
    /// Whether the gesture's one snapshot is taken —
    /// staged lazily by the first Move that actually
    /// changes the window, so a plain click (or a wiggle
    /// inside one frame) spends no undo step.
    pub snapshot_taken: bool,
}

thread_local! {
    /// The cue drag in flight, if any. A `thread_local`
    /// because it is gesture state with the gesture's own
    /// lifetime — the timeline's `PRESSED_CHIP` pattern
    /// in `timeline_marks`. Written and read only by
    /// [`block_drag`] (and read by [`lane_rows`] for the
    /// row pin); the factored handlers take the record as
    /// a parameter and never touch this cell.
    static CUE_DRAG: RefCell<Option<CueDrag>> = const { RefCell::new(None) };
}

/// Whole frames of travel for a pointer that has moved
/// `dx` window points since the press: the nearest frame
/// boundary, at the timeline's one pitch. Negative
/// travel gives negative frames.
pub fn frame_delta(dx: f32) -> i32 {
    (dx / CHIP_PITCH).round() as i32
}

/// The window a drag is asking for: the grabbed window
/// shifted (Body — duration preserved exactly) or
/// edge-moved (Start / End) by `delta` whole frames.
/// Clamps are validity's, not the timeline's: no
/// negative frames, and a moving edge never crosses its
/// sibling (a cue keeps at least one frame, the core
/// validation's range rule). Past the last frame is NOT
/// clamped — the core ops do not clamp frame numbers to
/// the timeline either, and a cue may legally live past
/// a shrunken end.
pub fn drag_target(mode: DragMode, orig_start: usize, orig_end: usize, delta: i32) -> (usize, usize) {
    let (start, end, delta) = (orig_start as i64, orig_end as i64, delta as i64);
    let duration = end - start;
    let clamped = match mode {
        DragMode::Body => {
            let start = (start + delta).max(0);
            (start, start + duration)
        }
        DragMode::Start => ((start + delta).clamp(0, (end - 1).max(0)), end),
        DragMode::End => (start, (end + delta).max(start + 1)),
    };
    (clamped.0 as usize, clamped.1 as usize)
}

/// Whether the record still names the cue it grabbed:
/// the cue at `index` exists and carries the grabbed
/// text. The one guard every writing phase runs (see
/// [`CueDrag::text`]).
fn drag_identity_holds(s: &State, g: &CueDrag) -> bool {
    s.core
        .project
        .subtitles
        .get(g.index)
        .is_some_and(|c| c.text == g.text)
}

/// The gesture's Down phase: the click, then the grab.
/// The click is [`press_cue`] verbatim (select + seek —
/// the framework delivers `on_press` only on release and
/// after the drag's Up, so a draggable block folds its
/// click into Down, the chips row's settlement); then
/// the record is planted from the cue as it stands. A
/// stale `i` does nothing at all, exactly like a stale
/// press. A record somehow left over from an earlier
/// gesture (its Up never arrived) is settled first —
/// rolled back if it had moved anything — so no staged
/// step can leak into this gesture.
pub fn drag_down(s: &mut State, drag: &mut Option<CueDrag>, i: usize, x: f32, w: f32) {
    if let Some(old) = drag.take() {
        if old.snapshot_taken && drag_identity_holds(s, &old) {
            s.core.undo();
        }
    }
    let Some(cue) = s.core.project.subtitles.get(i) else {
        return;
    };
    let record = CueDrag {
        index: i,
        mode: grab_mode(x, w),
        orig_start: cue.start_frame,
        orig_end: cue.end_frame,
        text: cue.text.clone(),
        snapshot_taken: false,
    };
    press_cue(s, i);
    *drag = Some(record);
}

/// The gesture's Move phase (and the release's final
/// application): point the grabbed cue at the window
/// [`drag_target`] computes from the pointer's total
/// travel `dx`. The first application that actually
/// changes the window stages the gesture's ONE snapshot
/// and writes in place; an application landing on the
/// window already stored writes nothing. If the record
/// no longer names its cue (the list was rewritten
/// between phases), the gesture is abandoned: the record
/// is dropped and NOTHING is written or undone — the
/// staged snapshot, if any, stays in the history as the
/// pre-drag state it honestly is, and the world that
/// replaced the list owns what happens next.
pub fn drag_apply(s: &mut State, drag: &mut Option<CueDrag>, dx: f32) {
    let Some(g) = drag.as_ref() else {
        return;
    };
    if !drag_identity_holds(s, g) {
        *drag = None;
        return;
    }
    let (index, mode, orig_start, orig_end, snapshot_taken) = (
        g.index,
        g.mode,
        g.orig_start,
        g.orig_end,
        g.snapshot_taken,
    );
    let target = drag_target(mode, orig_start, orig_end, frame_delta(dx));
    let cue = &s.core.project.subtitles[index];
    if (cue.start_frame, cue.end_frame) == target {
        return;
    }
    if !snapshot_taken {
        s.core.snapshot();
        if let Some(g) = drag.as_mut() {
            g.snapshot_taken = true;
        }
    }
    let cue = &mut s.core.project.subtitles[index];
    cue.start_frame = target.0;
    cue.end_frame = target.1;
}

/// The gesture's Up phase. Cancelled: the staged
/// snapshot, if one was taken, is rolled back with one
/// `undo()` — the pre-drag project returns and the net
/// undo cost of the whole gesture is zero (the dial's
/// rule); the selection keeps naming the cue, which is
/// back at the index the grab selected. Clean release:
/// the release position applies once more (the chips'
/// Up reads its release frame the same way), then a
/// gesture that moved settles — the cue is re-inserted
/// by the update op's own stable rule (remove, insert
/// after every cue whose `(start, end)` key is less or
/// equal), the selection follows it to its post-sort
/// index, and the status line takes the op's own
/// sentence. A gesture that never moved was a click:
/// [`drag_down`] already did its whole work, and no
/// undo step was ever staged.
pub fn drag_up(s: &mut State, drag: &mut Option<CueDrag>, dx: f32, cancelled: bool) {
    if cancelled {
        if let Some(g) = drag.take() {
            if g.snapshot_taken && drag_identity_holds(s, &g) {
                s.core.undo();
            }
        }
        return;
    }
    drag_apply(s, drag, dx);
    // If the final application abandoned the gesture
    // (identity lost between phases) the record is
    // already gone and there is nothing to settle.
    let Some(g) = drag.take() else {
        return;
    };
    if !g.snapshot_taken {
        return;
    }
    let cue = s.core.project.subtitles.remove(g.index);
    let key = (cue.start_frame, cue.end_frame);
    let new_index = s
        .core
        .project
        .subtitles
        .partition_point(|c| (c.start_frame, c.end_frame) <= key);
    s.core.project.subtitles.insert(new_index, cue);
    s.subtitle_selected = Some(new_index);
    s.core.status = format!("Subtitle {} updated.", new_index + 1);
}

/// A block's drag handler — the thin adapter between the
/// runtime's [`Drag`] and the factored handlers above:
/// Down plants the record from the block-local press
/// point, Move and Up read the pointer's total travel
/// (`Drag::dx` — window points since the press,
/// unaffected by the block itself moving as its cue is
/// written live) and the release's cancellation.
fn block_drag(s: &mut State, d: &mut Drag, i: usize) {
    CUE_DRAG.with(|cell| {
        let mut slot = cell.borrow_mut();
        match d.phase {
            Phase::Down => drag_down(s, &mut slot, i, d.x, d.w),
            Phase::Move => drag_apply(s, &mut slot, d.dx),
            Phase::Up => drag_up(s, &mut slot, d.dx, d.cancelled()),
        }
    });
}

/// The lane's row assignment for this build:
/// [`cue_rows`] over the project's cues — except while
/// a drag gesture is in flight, when the dragged cue's
/// window is read AS GRABBED, so every block keeps the
/// sub-row (and therefore the element path the runtime
/// captures the gesture by) it had at grab time for the
/// gesture's whole life; see the module docs. The pin
/// holds only while the record still names the cue at
/// its index (the gesture handlers' identity rule);
/// otherwise the live rows are the truthful ones.
fn lane_rows(s: &State) -> Vec<Option<usize>> {
    let cues = &s.core.project.subtitles;
    let frame_count = s.core.frame_count();
    let pinned = CUE_DRAG.with(|cell| {
        cell.borrow().as_ref().and_then(|g| {
            cues.get(g.index)
                .filter(|c| c.text == g.text)
                .map(|_| (g.index, g.orig_start, g.orig_end))
        })
    });
    match pinned {
        Some((index, start, end)) => {
            let mut windows: Vec<SubtitleCue> = cues.clone();
            windows[index].start_frame = start;
            windows[index].end_frame = end;
            cue_rows(&windows, frame_count)
        }
        None => cue_rows(cues, frame_count),
    }
}

/// The subtitle lane: header, then one sub-row of blocks
/// per [`cue_rows`] row. `None` when the project has no
/// cues at all (the lane exists only when it has
/// something to show — the clips lane's rule). Built to
/// be appended to the timeline's scrolled content column
/// — see the module docs for the wiring.
pub fn subtitle_lane(s: &State, t: &Theme) -> Option<El> {
    let core = &s.core;
    let cues = &core.project.subtitles;
    if cues.is_empty() {
        return None;
    }
    let frame_count = core.frame_count();
    let rows = lane_rows(s);

    // The header row: the lane's name at the left, the
    // current-frame readout at the right — the
    // dopesheet header's construction, sized to the
    // timeline's width so the readout lands over the
    // last frame column.
    let mut header = row().gap(8.0).center_y();
    if frame_count > 0 {
        header = header.size(timeline_width(frame_count), SUB_HEADER_H);
    } else {
        header = header.h(SUB_HEADER_H);
    }
    let header = header
        .child(design::section_label(
            &crate::rune_sec::i18n_wire::tr(core, "subtitles"),
            t,
        ))
        .child(el().grow(1.0))
        .child(
            text(frame_readout(core.current, frame_count))
                .font(type_::MONO_CAPTION.mono())
                .color(t.text_secondary),
        );

    let mut lane = col().key("subtitle-lane").gap(SUB_ROW_GAP).child(header);
    for r in 0..cue_row_count(&rows) {
        lane = lane.child(block_row(s, t, &rows, r));
    }
    Some(lane)
}

/// One sub-row of blocks: spacers carry each block to
/// its [`cue_rect`] x (the row's gap is 0 — the rects
/// ARE the layout), blocks in stored order (within one
/// row their x is non-decreasing, by [`cue_rows`]' fit
/// rule, so a single left-to-right cursor places them
/// all).
fn block_row(s: &State, t: &Theme, rows: &[Option<usize>], r: usize) -> El {
    let core = &s.core;
    let frame_count = core.frame_count();
    let mut row_el = row().gap(0.0).h(SUB_ROW_H).center_y();
    let mut cursor = 0.0f32;
    for (i, cue) in core.project.subtitles.iter().enumerate() {
        if rows[i] != Some(r) {
            continue;
        }
        let [x, w] = cue_rect(cue, frame_count);
        if x > cursor {
            row_el = row_el.child(el().size(x - cursor, SUB_ROW_H));
        }
        cursor = x + w;
        let selected = s.subtitle_selected == Some(i);
        let mut block = row()
            .key(block_key(i))
            .size(w, SUB_ROW_H)
            .px(4.0)
            .center_y()
            .radius(design::CONTROL_RADIUS)
            .fill(if selected {
                design::selected_fill()
            } else {
                t.control_off
            })
            .hover(t.hover)
            .press(t.press)
            .on_drag(move |s: &mut State, d: &mut Drag, _tx: &mut Tx| block_drag(s, d, i));
        if block_shows_text(w) && !cue.text.is_empty() {
            block = block.child(
                text(cue.text.clone())
                    .font(type_::LABEL)
                    .color(t.text)
                    .truncate(),
            );
        }
        row_el = row_el.child(block);
    }
    row_el
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rune_sec::timeline_marks::CHIP_W;
    use stykanimate_rs::model::{Frame, Pose};

    fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    fn frame() -> Frame {
        Frame {
            poses: Vec::<Pose>::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }
    }

    /// A state with `frames` frames and the given cues.
    fn cue_state(frames: usize, cues: Vec<SubtitleCue>) -> State {
        let mut s = crate::init();
        s.core.project.frames = (0..frames).map(|_| frame()).collect();
        s.core.project.subtitles = cues;
        s
    }

    #[test]
    fn block_key_follows_the_scheme() {
        assert_eq!(block_key(0), "subtitle-cue-0");
        assert_eq!(block_key(7), "subtitle-cue-7");
    }

    #[test]
    fn cue_rect_is_the_span_maths_at_the_timeline_pitch() {
        // Chip f starts at f * 34; a window's right edge
        // is the end of its last chip (end * 34 - 4).
        assert_eq!(cue_rect(&cue(0, 1, ""), 5), [0.0, CHIP_W]);
        assert_eq!(cue_rect(&cue(0, 3, ""), 5), [0.0, 3.0 * CHIP_PITCH - CHIP_GAP]);
        assert_eq!(cue_rect(&cue(2, 5, ""), 5), [68.0, 98.0]);
        assert_eq!(
            cue_rect(&cue(4, 5, ""), 5),
            [4.0 * CHIP_PITCH, CHIP_W],
            "the last frame's cue is one chip wide at the right edge"
        );
        // A long timeline: the same pitch, no drift.
        assert_eq!(cue_rect(&cue(50, 60, ""), 100), [1700.0, 336.0]);
    }

    #[test]
    fn cue_rect_agrees_with_span_rect_on_the_same_window() {
        // The delegation is the contract: for any window,
        // the cue rect IS the clips span rect.
        for (from, to, count) in [(0, 1, 5), (2, 5, 5), (3, 9, 5), (7, 9, 5), (0, 0, 3)] {
            let span = ClipSpan {
                kind: SpanKind::Fx,
                from_frame: from as f32,
                to_frame: to as f32,
                muted_or_hidden: false,
            };
            assert_eq!(
                cue_rect(&cue(from, to, ""), count),
                span_rect(&span, count),
                "window {from}..{to} over {count} frames"
            );
        }
    }

    #[test]
    fn cue_rect_clamps_at_the_timeline_end() {
        // Total width over 5 frames: 5 * 34 - 4 = 166.
        assert_eq!(timeline_width(5), 166.0);
        // Running past the end stops at the last chip.
        assert_eq!(cue_rect(&cue(3, 9, ""), 5), [102.0, 64.0]);
        // Wholly past the end: zero width, at the end.
        assert_eq!(cue_rect(&cue(7, 9, ""), 5), [166.0, 0.0]);
        // No frames, no rect.
        assert_eq!(cue_rect(&cue(0, 2, ""), 0), [0.0, 0.0]);
        // A degenerate window draws nothing.
        assert_eq!(cue_rect(&cue(3, 3, ""), 5), [102.0, 0.0]);
        assert_eq!(cue_rect(&cue(4, 2, ""), 5), [136.0, 0.0]);
    }

    #[test]
    fn text_shows_only_on_blocks_wide_enough_to_carry_it() {
        assert!(!block_shows_text(CHIP_W), "a one-frame tick carries no text");
        assert!(!block_shows_text(SUB_TEXT_MIN_W - 1.0));
        assert!(block_shows_text(SUB_TEXT_MIN_W));
        assert!(block_shows_text(200.0));
        // The threshold is exactly two frames' extent.
        assert_eq!(SUB_TEXT_MIN_W, cue_rect(&cue(0, 2, ""), 9)[1]);
    }

    #[test]
    fn cue_rows_share_when_separate_and_stack_when_overlapping() {
        let cues = vec![cue(0, 2, "a"), cue(2, 4, "b"), cue(5, 6, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(0), Some(0)]);
        assert_eq!(cue_row_count(&cue_rows(&cues, 10)), 1);

        // The middle cue overlaps the first; the third
        // fits back onto the first row once it ends.
        let cues = vec![cue(0, 4, "a"), cue(2, 6, "b"), cue(4, 8, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(1), Some(0)]);
        assert_eq!(cue_row_count(&cue_rows(&cues, 10)), 2);

        // Three mutually overlapping cues stack three deep.
        let cues = vec![cue(0, 6, "a"), cue(1, 5, "b"), cue(2, 3, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn cue_rows_give_no_row_to_cues_that_draw_nothing() {
        let cues = vec![cue(0, 2, "a"), cue(3, 3, "empty"), cue(9, 12, "past")];
        let rows = cue_rows(&cues, 5);
        assert_eq!(rows, vec![Some(0), None, None]);
        assert_eq!(cue_row_count(&rows), 1);
        assert_eq!(cue_row_count(&[]), 0);
    }

    #[test]
    fn selection_liveness_matches_the_audio_panel_rule() {
        assert!(selection_is_live(Some(0), 1));
        assert!(selection_is_live(Some(2), 3));
        assert!(!selection_is_live(Some(3), 3));
        assert!(!selection_is_live(Some(0), 0));
        assert!(!selection_is_live(None, 3));
    }

    #[test]
    fn press_cue_selects_and_seeks_to_the_cue_start() {
        let mut s = cue_state(10, vec![cue(0, 2, "a"), cue(4, 7, "b")]);
        press_cue(&mut s, 1);
        assert_eq!(s.subtitle_selected, Some(1));
        assert_eq!(s.core.current, 4, "the playhead seeks the cue's first frame");
        press_cue(&mut s, 0);
        assert_eq!(s.subtitle_selected, Some(0));
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn press_cue_with_a_start_past_the_end_selects_without_seeking() {
        // The project shrank under the cue: goto's own
        // bounds rule leaves the playhead where it was.
        let mut s = cue_state(3, vec![cue(8, 10, "stale")]);
        s.core.goto(2);
        press_cue(&mut s, 0);
        assert_eq!(s.subtitle_selected, Some(0));
        assert_eq!(s.core.current, 2);
    }

    #[test]
    fn press_cue_ignores_a_stale_index() {
        let mut s = cue_state(10, vec![cue(0, 2, "a")]);
        press_cue(&mut s, 5);
        assert_eq!(s.subtitle_selected, None);
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn lane_constructs_on_every_state_branch() {
        // Element trees are not readable from unit tests
        // — construction is the assertable level here
        // (the design kit's own discipline); the
        // presented-tree proof belongs to the headless
        // snaps (P33's seeded-cue proof covers this lane's
        // stage sibling; the lane itself rides the
        // timeline's existing snap coverage).
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // No cues: no lane.
            let s = cue_state(10, Vec::new());
            assert!(subtitle_lane(&s, &t).is_none());
            // One cue.
            let s = cue_state(10, vec![cue(1, 4, "Hello.")]);
            assert!(subtitle_lane(&s, &t).is_some());
            // Overlapping cues stack; a selected cue is
            // among them.
            let mut s = cue_state(10, vec![cue(0, 6, "a"), cue(2, 8, "b"), cue(3, 4, "c")]);
            s.subtitle_selected = Some(1);
            assert!(subtitle_lane(&s, &t).is_some());
            // Cues but no frames: the header's bare
            // branch, and every cue draws nothing.
            let s = cue_state(0, vec![cue(0, 2, "a")]);
            assert!(subtitle_lane(&s, &t).is_some());
            // A stale selection tints nothing and breaks
            // nothing.
            let mut s = cue_state(10, vec![cue(0, 2, "a")]);
            s.subtitle_selected = Some(99);
            assert!(subtitle_lane(&s, &t).is_some());
        }
    }

    // -------------------------------------------------
    // Drag-to-move (Wave R, R3)
    // -------------------------------------------------

    /// Two cues on a 10-frame project: A over 0..2 and
    /// B over 4..6 (B's block is 64 pt wide — two chips
    /// minus the gap — the width the grab tests pass).
    fn two_cue_state() -> State {
        cue_state(10, vec![cue(0, 2, "a"), cue(4, 6, "b")])
    }

    fn windows(s: &State) -> Vec<(usize, usize)> {
        s.core
            .project
            .subtitles
            .iter()
            .map(|c| (c.start_frame, c.end_frame))
            .collect()
    }

    #[test]
    fn frame_delta_rounds_travel_to_whole_frames() {
        assert_eq!(frame_delta(0.0), 0);
        assert_eq!(frame_delta(10.0), 0, "under half a pitch is no frame");
        assert_eq!(frame_delta(CHIP_PITCH), 1);
        assert_eq!(frame_delta(2.0 * CHIP_PITCH), 2);
        assert_eq!(frame_delta(0.5 * CHIP_PITCH), 1, "half a pitch rounds over");
        assert_eq!(frame_delta(-CHIP_PITCH), -1);
        assert_eq!(frame_delta(-0.5 * CHIP_PITCH), -1);
    }

    #[test]
    fn grab_mode_reads_the_edge_zones() {
        // A three-frame block is 98 pt wide; the edge
        // tolerance is the timeline's EDGE_PX (8).
        assert_eq!(grab_mode(0.0, 98.0), DragMode::Start);
        assert_eq!(grab_mode(EDGE_PX, 98.0), DragMode::Start);
        assert_eq!(grab_mode(EDGE_PX + 1.0, 98.0), DragMode::Body);
        assert_eq!(grab_mode(89.0, 98.0), DragMode::Body);
        assert_eq!(grab_mode(90.0, 98.0), DragMode::End);
        assert_eq!(grab_mode(97.0, 98.0), DragMode::End);
        // A one-frame tick (30 pt) still has a body.
        assert_eq!(grab_mode(8.0, CHIP_W), DragMode::Start);
        assert_eq!(grab_mode(15.0, CHIP_W), DragMode::Body);
        assert_eq!(grab_mode(22.0, CHIP_W), DragMode::End);
    }

    #[test]
    fn drag_target_body_preserves_duration_and_clamps_only_at_zero() {
        assert_eq!(drag_target(DragMode::Body, 4, 7, 3), (7, 10));
        assert_eq!(drag_target(DragMode::Body, 4, 7, 0), (4, 7));
        assert_eq!(drag_target(DragMode::Body, 4, 7, -4), (0, 3));
        assert_eq!(drag_target(DragMode::Body, 4, 7, -99), (0, 3));
        // Past the timeline's end is legal (the core ops
        // do not clamp frame numbers either).
        assert_eq!(drag_target(DragMode::Body, 4, 7, 100), (104, 107));
    }

    #[test]
    fn drag_target_edges_move_one_side_and_never_cross() {
        assert_eq!(drag_target(DragMode::Start, 4, 7, 2), (6, 7));
        assert_eq!(drag_target(DragMode::Start, 4, 7, 99), (6, 7));
        assert_eq!(drag_target(DragMode::Start, 4, 7, -99), (0, 7));
        assert_eq!(drag_target(DragMode::End, 4, 7, 2), (4, 9));
        assert_eq!(drag_target(DragMode::End, 4, 7, -2), (4, 5));
        assert_eq!(drag_target(DragMode::End, 4, 7, -99), (4, 5));
    }

    #[test]
    fn drag_down_is_the_click_and_plants_the_record() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        assert_eq!(s.subtitle_selected, Some(1), "the grab selects its cue");
        assert_eq!(s.core.current, 4, "and seeks to its first frame");
        assert_eq!(s.core.undo_depth(), 0, "a grab alone spends no undo step");
        let g = drag.expect("the grab plants a record");
        assert_eq!(g.index, 1);
        assert_eq!(g.mode, DragMode::Body);
        assert_eq!((g.orig_start, g.orig_end), (4, 6));
        assert!(!g.snapshot_taken);
    }

    #[test]
    fn drag_down_on_a_stale_index_plants_nothing() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 9, 32.0, 64.0);
        assert!(drag.is_none());
        assert_eq!(s.subtitle_selected, None);
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn a_body_drag_is_exactly_one_undo_step() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        drag_apply(&mut s, &mut drag, 3.0 * CHIP_PITCH);
        assert_eq!(windows(&s), vec![(0, 2), (7, 9)], "the cue moves live");
        assert_eq!(s.core.undo_depth(), 1, "the first real move stages the step");
        drag_apply(&mut s, &mut drag, 5.0 * CHIP_PITCH);
        assert_eq!(windows(&s), vec![(0, 2), (9, 11)]);
        assert_eq!(s.core.undo_depth(), 1, "later moves write under the same step");
        drag_up(&mut s, &mut drag, 5.0 * CHIP_PITCH, false);
        assert!(drag.is_none());
        assert_eq!(windows(&s), vec![(0, 2), (9, 11)]);
        assert_eq!(s.core.undo_depth(), 1, "the whole gesture is one undo step");
        assert_eq!(s.subtitle_selected, Some(1));
        assert_eq!(s.core.status, "Subtitle 2 updated.");
        s.core.undo();
        assert_eq!(windows(&s), vec![(0, 2), (4, 6)], "the one step undoes the whole drag");
        assert_eq!(s.core.undo_depth(), 0);
    }

    #[test]
    fn a_drag_past_a_neighbour_resorts_and_the_selection_follows() {
        let mut s = two_cue_state();
        let mut drag = None;
        // Drag A (0..2) right, past B (4..6).
        drag_down(&mut s, &mut drag, 0, 32.0, 64.0);
        drag_apply(&mut s, &mut drag, 5.0 * CHIP_PITCH);
        assert_eq!(
            windows(&s),
            vec![(5, 7), (4, 6)],
            "mid-gesture the list is NOT re-sorted — the grabbed index stays put"
        );
        drag_up(&mut s, &mut drag, 5.0 * CHIP_PITCH, false);
        assert_eq!(windows(&s), vec![(4, 6), (5, 7)], "the release re-settles into sorted order");
        assert_eq!(
            s.core.project.subtitles[1].text, "a",
            "the dragged cue is the one that moved in the list"
        );
        assert_eq!(s.subtitle_selected, Some(1), "the selection follows the cue");
        assert_eq!(s.core.status, "Subtitle 2 updated.");
        assert_eq!(s.core.undo_depth(), 1);
    }

    #[test]
    fn a_cancelled_drag_rolls_back_and_spends_nothing() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        drag_apply(&mut s, &mut drag, 2.0 * CHIP_PITCH);
        assert_eq!(windows(&s), vec![(0, 2), (6, 8)]);
        assert_eq!(s.core.undo_depth(), 1);
        drag_up(&mut s, &mut drag, 2.0 * CHIP_PITCH, true);
        assert!(drag.is_none());
        assert_eq!(
            windows(&s),
            vec![(0, 2), (4, 6)],
            "the pre-drag project is restored whole"
        );
        assert_eq!(s.core.undo_depth(), 0, "the staged step is consumed by the rollback");
        assert_eq!(
            s.subtitle_selected,
            Some(1),
            "the selection still names the cue at its original index"
        );
    }

    #[test]
    fn a_click_and_a_sub_frame_wiggle_spend_no_undo_step() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        drag_up(&mut s, &mut drag, 0.0, false);
        assert_eq!(s.core.undo_depth(), 0, "a plain click stages nothing");
        assert_eq!(s.subtitle_selected, Some(1));
        assert_eq!(s.core.current, 4);

        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        drag_apply(&mut s, &mut drag, 10.0);
        assert_eq!(windows(&s), vec![(0, 2), (4, 6)], "sub-frame travel moves nothing");
        drag_up(&mut s, &mut drag, 10.0, false);
        assert_eq!(s.core.undo_depth(), 0, "and stages nothing");
    }

    #[test]
    fn an_edge_drag_resizes_with_one_undo_step() {
        // Start edge: pull B's start two frames earlier.
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 2.0, 64.0);
        assert_eq!(drag.as_ref().map(|g| g.mode), Some(DragMode::Start));
        drag_apply(&mut s, &mut drag, -2.0 * CHIP_PITCH);
        assert_eq!(windows(&s), vec![(0, 2), (2, 6)], "only the start moved");
        drag_up(&mut s, &mut drag, -2.0 * CHIP_PITCH, false);
        assert_eq!(s.core.undo_depth(), 1);
        assert_eq!(s.subtitle_selected, Some(1));

        // End edge: push B's end one frame later.
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 62.0, 64.0);
        assert_eq!(drag.as_ref().map(|g| g.mode), Some(DragMode::End));
        drag_apply(&mut s, &mut drag, CHIP_PITCH);
        assert_eq!(windows(&s), vec![(0, 2), (4, 7)], "only the end moved");
        drag_up(&mut s, &mut drag, CHIP_PITCH, false);
        assert_eq!(s.core.undo_depth(), 1);
    }

    #[test]
    fn a_rewritten_list_abandons_the_gesture_without_writing() {
        let mut s = two_cue_state();
        let mut drag = None;
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        // Another surface rewrites the cue between
        // phases (the panel's edit, a live-MCP update):
        // the index now names a different cue.
        s.core.project.subtitles[1].text = "rewritten".to_string();
        drag_apply(&mut s, &mut drag, 3.0 * CHIP_PITCH);
        assert!(drag.is_none(), "the gesture is abandoned");
        assert_eq!(
            windows(&s),
            vec![(0, 2), (4, 6)],
            "and nothing was written under the wrong index"
        );
        assert_eq!(s.core.undo_depth(), 0);
        drag_up(&mut s, &mut drag, 3.0 * CHIP_PITCH, false);
        assert_eq!(s.core.undo_depth(), 0, "the abandoned release settles nothing");
    }

    #[test]
    fn drag_down_settles_a_leftover_record_first() {
        let mut s = two_cue_state();
        let mut drag = None;
        // A first gesture that moved B but whose Up
        // never arrived.
        drag_down(&mut s, &mut drag, 1, 32.0, 64.0);
        drag_apply(&mut s, &mut drag, 2.0 * CHIP_PITCH);
        assert_eq!(s.core.undo_depth(), 1);
        // A new grab settles it: rolled back, not leaked.
        drag_down(&mut s, &mut drag, 0, 32.0, 64.0);
        assert_eq!(windows(&s), vec![(0, 2), (4, 6)], "the leftover drag is rolled back");
        assert_eq!(s.core.undo_depth(), 0, "its staged step is consumed");
        assert_eq!(s.subtitle_selected, Some(0), "the new grab proceeds normally");
        assert_eq!(drag.as_ref().map(|g| g.index), Some(0));
    }

    #[test]
    fn rows_stay_pinned_to_the_grab_while_a_drag_is_in_flight() {
        // A (0..2) and B (3..5) share a row; dragging A
        // two frames right would cross B and, on live
        // windows, push B to a second row — changing its
        // block's element path mid-gesture.
        let mut s = cue_state(10, vec![cue(0, 2, "a"), cue(3, 5, "b")]);
        assert_eq!(lane_rows(&s), vec![Some(0), Some(0)]);
        CUE_DRAG.with(|cell| {
            let mut slot = cell.borrow_mut();
            drag_down(&mut s, &mut slot, 0, 32.0, 64.0);
            drag_apply(&mut s, &mut slot, 2.0 * CHIP_PITCH);
        });
        assert_eq!(windows(&s), vec![(2, 4), (3, 5)], "the cue really moved");
        assert_eq!(
            cue_rows(&s.core.project.subtitles, 10),
            vec![Some(0), Some(1)],
            "on live windows the rows WOULD change"
        );
        assert_eq!(
            lane_rows(&s),
            vec![Some(0), Some(0)],
            "but the lane pins the grab-time rows for the gesture's life"
        );
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            assert!(subtitle_lane(&s, &t).is_some(), "the lane builds mid-drag");
        }
        CUE_DRAG.with(|cell| {
            let mut slot = cell.borrow_mut();
            drag_up(&mut s, &mut slot, 2.0 * CHIP_PITCH, false);
        });
        assert_eq!(
            lane_rows(&s),
            vec![Some(0), Some(1)],
            "the release settles, and the rows follow the settled windows"
        );
    }
}
