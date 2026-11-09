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
//! Focused Wave (D1): the card has a collapsed form —
//! `s.frame_ops.timeline_collapsed` (a `FrameOpsState`
//! field, seeded from and written back to
//! `LayoutPrefs::timeline_collapsed` by main.rs's
//! init/poll) reduces the card to its frame-chips row:
//! ruler, marker row, clips lane, dopesheet lane and
//! loop row all stay unmounted. There is no GUI toggle
//! for it in this shell; the writers are the persisted
//! prefs at startup and the shell layout tool.
//!
//! ## Keys (preservation)
//!
//! `timeline` on the outer row and `frame-{n}` (1-based) on
//! every chip, exactly as `chrome_view::timeline_row` — the
//! headless tests find the chips by those keys. Two keys are
//! added: `timeline-chips` on the inner content row that
//! carries the drag handler (see below), and
//! `timeline-markers` on the marker row under the chips
//! (mounted only when a frame carries a marker — Wave 10,
//! see the Marker row section).
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
//! `loop-b-dn` / `loop-b-up` inside it. Phase M adds
//! `timeline-clips` on the clips lane in the same
//! under-timeline stack (see the Clips lane section
//! below). Phase N adds NO key: the timeline card (see
//! the Timeline card section below) is deliberately
//! unkeyed.
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
//! wrapper adds no fill, padding or radius. Phase N grows
//! the wrapper to fill its share of the cell — the full
//! `CHIP_W × MARKER_H` for a lone marker, an equal split
//! when both glyphs share one cell — so the press target
//! is the cell, not the ~11-point glyph (P18's smallest
//! target in the app); the glyph itself is drawn exactly
//! as before, centred in the wrapper.
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
//! Wave 10 (audit F1/F2) changes two things about the
//! row. **Current column (F1):** the current frame's
//! cell takes the current chip's own treatment — the
//! accent fill at the chips' radius, both glyphs white
//! ([`marker_glyph_color`]) — so the row reads against
//! the playhead even where the current cell itself
//! carries no glyph; every other cell is drawn exactly
//! as before, and the marker press targets and handlers
//! are unchanged. **Collapse (F2):** the row is mounted
//! only when at least one frame carries a marker
//! ([`any_markers`]) — a marker-free project's timeline
//! is ruler + chips (+ clips) and the row's 14 points
//! are not spent on a strip of blanks, the clips lane's
//! rule. The predicate is the row's own content,
//! deliberately not `stats::figure_keyframes`: that
//! helper measures figure *pose* changes, which this
//! row never renders, and it disagrees with the row's
//! content in both directions — non-empty on the
//! marker-free demo project (a frame-0 pose is a pose
//! key), empty on a camera-keyed project whose figures
//! are never posed (see [`any_markers`]).
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
//! ## Press feedback (UI mission Wave 6-D, audit S17 D1/I-2)
//!
//! The frame chip is the most-pressed control in the shell
//! and, alone among the kit's pressables, had no press
//! state: it defined fill + hover and stopped. The kit's
//! own treatment — a `.press(t.press)` overlay and
//! `.press_scale(PRESS_SCALE)` — cannot fire on a chip,
//! and this is proven from the pinned Rune source, not
//! assumed: the press and press-scale tracks engage only
//! on the element that **captured** the pointer
//! (`runtime/input/hover.rs`: `captured == Some(n.path)`),
//! capture goes to the press's primary target
//! (`runtime/input/pointer.rs`), and a node is a primary
//! target only if it takes primary input
//! (`runtime/input/hit.rs`) — a handler, a textbox, a
//! focusable. The chips carry no handlers by design (the
//! gesture above owns the pointer), so the row captures
//! and a `.press()` chained on a chip would be dead
//! paint. Giving a chip its own `on_press`/`on_drag` to
//! make it capture would take the gesture off the row
//! and break the coordinate basis the drag reads.
//!
//! What the gesture code itself can honestly support is
//! what happens here: [`timeline_drag`] records the chip
//! under the held pointer in a `thread_local`
//! ([`pressed_chip`] — the shell's pattern for view state
//! that is not project state, cf. the stage's view nav),
//! and the chip builder bakes the kit's press overlay
//! into that chip's fill with [`pressed_fill`] — the same
//! `t.press` token, composited source-over exactly as
//! Rune paints `.press()` at full strength. While the
//! chip is held its hover overlay is not chained: Rune
//! paints press **over** hover, and the chip's hover is a
//! baked colour (for the current chip, the opaque
//! accent), which would cover the tint entirely; dropping
//! it for the duration of the hold is the static
//! equivalent of that paint order. The record is written
//! before the handler can bail out and cleared on **Up**
//! wherever the release lands, so no stale press paint
//! can outlive the gesture. `press_scale` stays unadopted
//! for the reason above — it is the same capture-bound
//! track — and is recorded here as a known limit, not
//! silently skipped.
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
//! core's own op (it also posts "Loop range cleared.").
//! UI mission Wave 5 (audit D3/T4, integrator-approved
//! recorded key change): this row is now the single
//! timeline home of loop-range editing — the Playback
//! panel's duplicate `loop-start` / `loop-end` /
//! `loop-clear` mounts were removed (its jump buttons
//! remain, relabelled "Go to start" / "Go to end"), so
//! within this wave's files `loop-clear` is mounted
//! here alone; the Frames section's own mounts of the
//! three keys remain (that file is outside this wave).
//! Like the edge drag, the row takes no snapshots:
//! `loop_range` is runtime state the core never
//! undoes.
//!
//! ## Clips lane (Phase M, moved in Wave 5)
//!
//! The scrolled column's last row — under the marker
//! row, inside the scroll viewport — is a clips lane
//! (`timeline-clips`): one slim bar per audio or FX
//! clip, so the timeline itself shows where the
//! project's clips sit without opening the Audio or
//! Effects panels. The lane renders **only when at
//! least one clip exists** — a clip-free project gets
//! no lane element at all, so its timeline is exactly
//! what it was before the lane existed.
//!
//! Bars use the chips' own geometry: [`span_rect`] maps
//! a span's fractional frame range through the same
//! [`CHIP_W`] / [`CHIP_GAP`] pitch from the same x = 0
//! the ruler and chips start at, and the canvas is
//! content-width, like the chips row it mirrors. The
//! drawing is one `Shape` of round-cap bars (the fx
//! strip's canvas idiom) in the 2.0 palette: audio in
//! the primary accent `0x6D8CFF`, FX in the secondary
//! accent `0xA184FF`. A muted audio clip or a hidden FX
//! clip still draws, dimmed to [`CLIP_DIM_ALPHA`] — the
//! lane shows where a clip *is*, its state included.
//!
//! An FX clip's span is its own stored window (`start`
//! to `start + duration`, both fractional). An audio
//! clip stores no length, so its span's length is
//! probed from its file
//! (`stykanimate_rs::audio_ops::clip_duration_frames`);
//! when the probe cannot read the file, the span falls
//! back to a one-frame tick at the start frame — a
//! marker that a clip starts there, never an invented
//! duration. A probed length is floored at one frame
//! for the same reason: a span must show.
//!
//! Wave 5 (audit D1/T1) moved the lane inside the
//! scrolled column: it used to hang under the scroll
//! row in the card, drawn from the timeline's first
//! frame while the chips above it scrolled, so on a
//! scrolled timeline every bar sat under the wrong
//! frames. As the column's last row it shares the one
//! scroll offset, so a bar always sits under the
//! frames it summarises. The lane is a passive canvas
//! — it has no handlers, and sitting below the marker
//! row it shifts no existing row's content
//! coordinates, so both drag gestures read exactly
//! what they read before the move.
//!
//! Wave 5 (audit D2/T2) also stacks the bars: the
//! lane is a stack of sub-lane rows, audio above FX
//! (the kind split), and within one kind a span that
//! overlaps a span already on a sub-lane drops to the
//! next sub-lane instead of painting over it — no bar
//! can hide another. [`CLIPS_H`] is one sub-lane
//! row's height; the lane grows by one row plus
//! [`CLIP_ROW_GAP`] per stacked row
//! ([`clips_lane_height`]). [`clip_lane_rows`] is the
//! assignment; muted/hidden dimming is unchanged.
//!
//! ## Timeline card (Phase N)
//!
//! The whole stack — the scrolled ruler / chips /
//! markers / clips column, then the loop row, in that
//! order — sits in one card (charter §13):
//! fill `t.card`, border 1.0 `t.card_border`, radius
//! [`design::PANEL_RADIUS`], padding 8. The card is
//! **unkeyed** (a `timeline-card` key was considered
//! and rejected — the gate diffs key sets and every key
//! a test uses lives on the card's children) and it
//! wraps **outside** the scroll row: no padded element
//! sits between the scroll row and its content column,
//! so the drag handlers keep reading element-local
//! content coordinates and every drag maps exactly as
//! it did before the card existed (see the press / drag
//! semantics above — an 8-point shift would mis-map
//! clicks by a full loop-edge zone). The stack's
//! internal gap stays 2.0 — the charter's panel gap
//! (8) is for control panels; the marker row must sit
//! directly under its chips. Two consequences inside
//! the card: an idle chip's resting fill is
//! `t.control_off`, not `t.card` (a `t.card` chip on a
//! `t.card` ground would be invisible), and the loop
//! row's button gap is 8, the card's spacing, rather
//! than the chips' [`CHIP_GAP`]. Every geometry
//! constant, handler, label and key is unchanged; the
//! card is always returned. Wave 9 (audit D8/T9): on a
//! zero-frame project the scroll row's rows would all
//! be empty, so the card holds the kit
//! [`design::empty_state`] in the scroll row's place
//! instead — the one state in which the timeline has
//! something to say before it has frames to show.
//!
//! ## Label widths (measured, Wave 9)
//!
//! Both number-bearing rows were re-audited against
//! Rune's *measured* advances, read off a production
//! headless tree dump rather than estimated: at
//! `type_::LABEL` (12 pt) the digit `1` paints 5.0 pt
//! wide and the wide digits (`0`, `2`) 7.5 pt; the
//! ruler's `0:00` at its 10 pt style paints 22.5 pt in
//! its 30 pt slot, which prices the wide digits at
//! 6.25 pt and the colon at 3.75 pt at that size.
//!
//! - **Ruler labels fit.** `59:59` — the widest label
//!   under 100 minutes — prices at 4 × 6.25 + 3.75 =
//!   28.75 pt, inside the [`CHIP_W`] slot with 1.25 pt
//!   to spare. `100:00` prices at 35 pt and does not
//!   fit, but the label cell truncates by construction,
//!   so it ellipsises rather than spilling — and it is
//!   reached only past 100 minutes of timeline (72,000
//!   frames at 12 fps). The current one-label-per-
//!   second density stands; no change was made.
//! - **Plain chip numbers fit.** Four wide digits
//!   price at exactly 30.0 pt (`9999`), the chip's own
//!   width; five price over it, past frame 9999.
//! - **Suffixed chip labels do not fit, and no font
//!   split can fix them.** `12 ×0.25`'s digits and dot
//!   alone price at 5.0 + 7.5 + 7.5 + ≈4 + 7.5 + 7.5
//!   ≈ 39 pt — over the chip before the space and the
//!   `×` are counted at all. The audit's T6 split
//!   (number in LABEL, suffix in MONO_CAPTION) prices
//!   *worse*: MONO_CAPTION is the same 12 pt
//!   (`crates/rune/src/tok.rs:142`) and a mono advance
//!   (≈ 0.6 em, 7.2 pt) is wider than the proportional
//!   digits it would replace — the suffix alone would
//!   price at 6 × 7.2 = 43.2 pt. `type_` offers no
//!   smaller numeric style, and shrinking the suffix
//!   to a local style still leaves the two-part label
//!   over 30 pt at any size ≥ 6 pt (at 9 pt: number
//!   12.5 + suffix ≈ 27 ≈ 39.5 pt total). The remedies
//!   that remain — a wider chip pitch or a different
//!   label format — are contract changes (the pitch is
//!   shared by the ruler, markers and clips lane; the
//!   format is pinned by [`chip_label`]'s tests), so
//!   they are recorded here as integrator decisions,
//!   not landed as an unproven restyle.

use std::cell::Cell;

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::frame_ops::{selected_range, FrameOpsState};
use crate::stage_view::hex_color;
use crate::State;

/// Chip width in points — chrome_view's timeline chip size,
/// and the width [`frame_at_x`] is parameterised with.
pub const CHIP_W: f32 = 30.0;
/// Chip height in points (chrome_view parity).
pub const CHIP_H: f32 = 24.0;
/// Horizontal gap between chips (chrome_view parity).
pub const CHIP_GAP: f32 = 4.0;

/// The frame-chip pitch in points: one chip plus the gap
/// after it — the x distance between successive frames'
/// left edges in every row laid out on the chips'
/// geometry (the ruler, the marker row, [`span_rect`]'s
/// clip spans). Spelled as one constant (Wave 10) so the
/// dopesheet lane lays its rows out on the same pitch by
/// importing this instead of duplicating the 34-point
/// stride.
pub const CHIP_PITCH: f32 = CHIP_W + CHIP_GAP;

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
/// `clear_loop_range`. Wave 5 made this row the single
/// timeline home of loop-range editing (the Playback
/// panel's duplicate mounts were removed; the Frames
/// section keeps its own mounts).
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
    let idx = (x / CHIP_PITCH).floor() as usize;
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

/// Whether the marker row has anything to show: at
/// least one frame carrying a camera or a graph marker.
/// This is the row's mount predicate (Wave 10, audit
/// F2): the row renders only camera and graph markers,
/// so "no markers anywhere" is the one state in which
/// it would paint a full row of blank cells — the state
/// it now collapses out of, the clips lane's rule
/// ([`clips_lane`] is `None` on a clip-free project for
/// the same reason).
///
/// The predicate is the row's own content, deliberately
/// not `stats::figure_keyframes`: that helper measures
/// figure *pose* changes, which this row never renders,
/// and it disagrees with the row's content in both
/// directions — it is non-empty on the marker-free demo
/// project (a frame-0 pose is a pose key), where the
/// row is blank, and empty on a camera-keyed project
/// whose figures are never posed, whose markers must
/// still show.
pub fn any_markers(markers: &[(bool, bool)]) -> bool {
    markers.iter().any(|&(camera, graph)| camera || graph)
}

/// The colour one marker glyph paints. Off the current
/// frame the camera diamond takes the accent and the
/// graph dot the secondary text tone, as the row has
/// always drawn them; on the current frame both go
/// white — the current chip's own treatment (accent
/// fill, white content) carried into its marker cell
/// (Wave 10, audit F1), so the glyph stays legible on
/// the cell's accent ground.
pub fn marker_glyph_color(
    current: bool,
    camera: bool,
    accent: Color,
    secondary: Color,
) -> Color {
    if current {
        Color::WHITE
    } else if camera {
        accent
    } else {
        secondary
    }
}

/// The marker row under the chips (see the module
/// docs): one cell per chip, same geometry as the
/// display-only row it replaces — each glyph wrapped
/// in a transparent pressable element keyed by its
/// frame, pressing it seeks there. Phase N: the wrapper
/// fills its share of the cell (the whole `CHIP_W ×
/// MARKER_H` cell for a lone marker, an equal split
/// when both glyphs share one) so the press target is
/// the cell, not the glyph — the glyph is drawn
/// unchanged, centred in the wrapper. Wave 10 (audit
/// F1): the current frame's cell takes the current
/// chip's treatment — accent fill at the chips' own
/// radius, both glyphs white ([`marker_glyph_color`])
/// — so the row reads against the playhead even where
/// the current cell itself carries no glyph; every
/// other cell is drawn exactly as before. `None` when
/// no frame carries a marker ([`any_markers`], audit
/// F2): the row is not mounted at all then, and a
/// marker-free timeline is ruler + chips (+ clips),
/// exactly as it was before the row existed.
fn marker_row(core: &EditorCore, t: &Theme) -> Option<El> {
    let markers = markers_for_project(core);
    if !any_markers(&markers) {
        return None;
    }
    let accent = tok::accent(0, true);
    let mut marker_row = row().key("timeline-markers").gap(CHIP_GAP);
    for (i, &(camera, graph)) in markers.iter().enumerate() {
        let current = i == core.current;
        let mut cell = row().size(CHIP_W, MARKER_H).center().gap(2.0);
        if current {
            cell = cell.fill(accent).radius(6.0);
        }
        if camera {
            cell = cell.child(
                el()
                    .key(camera_marker_key(i))
                    .grow(1.0)
                    .h(MARKER_H)
                    .center()
                    .on_press(move |s: &mut State| goto_marker(s, i))
                    .child(
                        text("◆")
                            .font(type_::LABEL)
                            .color(marker_glyph_color(
                                current,
                                true,
                                accent,
                                t.text_secondary,
                            )),
                    ),
            );
        }
        if graph {
            cell = cell.child(
                el()
                    .key(graph_marker_key(i))
                    .grow(1.0)
                    .h(MARKER_H)
                    .center()
                    .on_press(move |s: &mut State| goto_marker(s, i))
                    .child(
                        text("●")
                            .font(type_::LABEL)
                            .color(marker_glyph_color(
                                current,
                                false,
                                accent,
                                t.text_secondary,
                            )),
                    ),
            );
        }
        marker_row = marker_row.child(cell);
    }
    Some(marker_row)
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

thread_local! {
    /// The chip under the held pointer while a chips-row
    /// drag is in flight (Wave 6-D press feedback — see
    /// the module docs' "Press feedback" section for why
    /// this record exists and why the kit's `.press()` /
    /// `press_scale` cannot do the job). Written by
    /// [`timeline_drag`] on every phase, read by the chip
    /// builder in [`timeline_marks`]; `None` whenever no
    /// chips-row gesture is held. A `thread_local` because
    /// it is pure view state with the gesture's own
    /// lifetime — the shell's pattern for such state (cf.
    /// the stage's view nav in `stage_view`).
    static PRESSED_CHIP: Cell<Option<usize>> = const { Cell::new(None) };
}

/// The chip currently showing press feedback, if a
/// chips-row drag is being held over one.
pub fn pressed_chip() -> Option<usize> {
    PRESSED_CHIP.with(|c| c.get())
}

fn set_pressed_chip(frame: Option<usize>) {
    PRESSED_CHIP.with(|c| c.set(frame));
}

/// The press record one drag phase leaves behind: a
/// release clears it wherever it lands (even where
/// `frame` names no chip); a press or a move points it
/// at the chip under the pointer (or clears it when the
/// position names none). Pure, so the bookkeeping the
/// handler performs is testable without a `Drag` (which
/// only the runtime can construct).
pub fn press_target(phase: Phase, frame: Option<usize>) -> Option<usize> {
    match phase {
        Phase::Up => None,
        Phase::Down | Phase::Move => frame,
    }
}

/// A fill with the kit's press overlay baked in: Rune
/// paints `.press(c)` as `c` composited source-over the
/// element's fill at full strength while the element is
/// captured, which for an opaque fill is exactly the
/// fill mixed toward `press` by `press`'s own alpha.
/// The fill's alpha is restored after the mix (`mix`
/// lerps alpha too; source-over onto an opaque fill
/// keeps it opaque). This is the composite
/// [`timeline_marks`] paints on the held chip, since
/// the chip itself can never be the captured element
/// (see the module docs).
pub fn pressed_fill(fill: Color, press: Color) -> Color {
    fill.mix(press, press.a).alpha(fill.a)
}

/// The row's drag gesture (see the module docs for the exact
/// semantics). `d.data` records which gesture the press
/// started: the selection anchor across phases as a fallback
/// (the selection itself is the primary record), or a loop
/// edge sentinel for an edge drag.
fn timeline_drag(s: &mut State, d: &mut Drag) {
    let n = s.core.frame_count();
    // Wave 6-D press feedback: record the chip under the
    // held pointer before anything can bail out, so a
    // release clears the record even when it lands where
    // `frame_at_x` names no chip (the early return below
    // would otherwise strand the press paint).
    let frame = frame_at_x(d.x, CHIP_W, CHIP_GAP, n);
    set_pressed_chip(press_target(d.phase, frame));
    let Some(f) = frame else {
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
/// under them whenever a frame carries a marker. Current frame accent-filled; loop-range frames
/// tinted toward the accent; selected frames outlined (white
/// on the current frame, accent elsewhere, so the outline
/// survives the accent fill).
pub fn timeline_marks(s: &State, t: &Theme) -> El {
    let core = &s.core;
    // Focused Wave (D1): the collapsed form — the card
    // keeps only the frame-chips row (the old GPUI
    // shell's collapsed timeline; the status strip is a
    // separate centre child and always renders). The
    // flag is the timeline section's own state
    // (`FrameOpsState::timeline_collapsed`), seeded from
    // and written back to
    // `LayoutPrefs::timeline_collapsed` by main.rs's
    // init/poll; nothing else about the card changes,
    // and every key below still exists in the expanded
    // form exactly as before.
    let collapsed = s.frame_ops.timeline_collapsed;
    let accent = tok::accent(0, true);
    let loop_fill = t.card.mix(accent, 0.35);
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
    // Collapsed: the ruler is not mounted (see the
    // assembly below), so its tick cells are not built.
    if !collapsed {
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
    }
    let mut chips = row()
        .key("timeline-chips")
        .gap(CHIP_GAP)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| timeline_drag(s, d));
    for i in 0..core.frame_count() {
        let current = i == core.current;
        let duration = core.project.frames[i].duration;
        // Phase N: the chips sit on the timeline card
        // (fill `t.card`), so an idle chip filled `t.card`
        // would vanish into its ground — its resting fill
        // is `t.control_off`, the theme's resting-control
        // tone. The current frame's accent and the loop
        // tint (`t.card` mixed toward the accent, still
        // visibly bluer than the ground) are unchanged.
        let fill = if current {
            accent
        } else if in_loop(core.loop_range, i) {
            loop_fill
        } else {
            t.control_off
        };
        // Wave 6-D (audit S17 D1/I-2): the held chip paints
        // the kit's press treatment — the `t.press` overlay
        // baked into its fill by `pressed_fill`, the closest
        // honest equivalent of `.press(t.press)` for an
        // element that can never capture the pointer (see
        // the module docs' "Press feedback" section). While
        // held, the hover overlay is not chained: Rune
        // paints press over hover, and this chip's hover is
        // a baked colour (the opaque accent on the current
        // chip) that would cover the tint outright.
        let held = pressed_chip() == Some(i);
        let fill = if held { pressed_fill(fill, t.press) } else { fill };
        let mut chip = el()
            .key(format!("frame-{}", i + 1))
            .size(CHIP_W, CHIP_H)
            .radius(6.0)
            .fill(fill);
        if !held {
            chip = chip.hover(if current { accent } else { t.control_track });
        }
        chip = chip.center().child(
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
    // The marker row (see [`marker_row`]): Wave 10
    // (audit F2) mounts it only when a frame carries a
    // marker — `marker_row` is None on a marker-free
    // project, whose column is then ruler + chips (+
    // clips), so the row's height is never spent on a
    // strip of blank cells.
    // All rows live in one column inside the scroll
    // viewport, so they translate together: the ruler stays
    // above its chips and the markers stay under theirs at
    // any scroll offset.
    // Focused Wave (D1): collapsed, the column is the
    // chips row alone — ruler, markers, clips lane and
    // dopesheet all stay unmounted, so the chips keep
    // their exact geometry (and drag coordinates) in
    // both forms.
    let mut content = col().gap(2.0);
    if !collapsed {
        content = content.child(ruler);
    }
    content = content.child(chips);
    if !collapsed {
        if let Some(markers_row) = marker_row(core, t) {
            content = content.child(markers_row);
        }
    }
    // UI mission Wave 5 (audit D1/T1): the clips lane is
    // the scrolled column's last row, so it shares the
    // chips' one scroll offset — a clip bar always sits
    // under the frames it summarises, however far the
    // timeline is scrolled. It used to hang under the
    // scroll row in the card, drawn from the timeline's
    // first frame while the chips above it scrolled, so
    // on a scrolled timeline every bar sat under the
    // wrong frames. The lane is a passive canvas (no
    // handlers), and adding it below the marker row puts
    // no padded element into the scroll chain and shifts
    // no existing row's content coordinates — the ruler,
    // chips and marker rows keep the exact geometry their
    // drag handlers read.
    if !collapsed {
        if let Some(clips) = clips_lane(core) {
            content = content.child(clips);
        }
    }
    // Wave 10 (dopesheet rebuild): the dopesheet lane is
    // the scrolled column's last row in the Animate
    // workspace — one row per figure, a diamond on each
    // of the figure's keyframes (stats::figure_keyframes),
    // cells pressing to that frame with that figure
    // selected. It lives in the same column for the same
    // reason the clips lane does: one shared scroll
    // offset, so a cell always sits under its frame chip.
    // The timeline card itself mounts in every workspace
    // (it is centre-column chrome), so the Animate gate
    // sits here at the call site; the lane builder is
    // workspace-agnostic. See docs/DOPESHEET.md.
    if !collapsed && s.workspace == crate::rune_sec::workspaces::Workspace::Animate {
        content = content.child(crate::rune_sec::dopesheet::dopesheet_lane(s, t));
    }
    let timeline = row().key("timeline").child(content).scroll_x().w_full();
    // Phase N (charter §13, see the module docs' Timeline
    // card section): the whole stack sits in one unkeyed
    // card — fill `t.card`, hairline border, radius 12,
    // padding 8, internal gap still 2.0. The card wraps
    // OUTSIDE the scroll row, so no padded element enters
    // the chain between the scroll row and its content
    // column and every drag keeps its content coordinates.
    // The loop row hangs under the scrolled timeline,
    // outside the viewport, so its buttons stay put
    // however far the chips scroll. The card is returned
    // in every state: the centre column's slot no longer
    // alternates between a bare row and a stack column.
    let mut card = col()
        .gap(2.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .radius(design::PANEL_RADIUS)
        .padding(8.0);
    if core.frame_count() == 0 {
        // Wave 9 (audit D8/T9): on an empty project the
        // scroll row's rows are all empty loops over zero
        // frames — the card would say nothing at all — so
        // the card holds the kit empty state in the scroll
        // row's place. The message names the one real next
        // action in words: the transport's `+ Frame` adds
        // the first frame. This panel has no
        // frame-creating handler of its own to offer as a
        // button, and the kit empty state carries no
        // action slot, so a button here would be fake.
        // No key is added or removed: the empty state is
        // unkeyed, and every timeline key lives on the
        // scroll row's children, which render exactly as
        // before whenever a frame exists.
        card = card.child(design::empty_state(
            "film",
            "No frames yet — add one to start animating.",
            t,
        ));
    } else {
        card = card.child(timeline);
        // The loop row is part of the expanded form
        // (Focused Wave, D1): collapsed, the card is
        // the chips strip alone. The empty-project
        // branch above still wins in both forms — with
        // no frames there are no chips to keep, and the
        // empty state's next-action message matters
        // more than the saved collapse preference.
        if !collapsed {
            if let Some(loop_row) = loop_row(s, t) {
                card = card.child(loop_row);
            }
        }
    }
    card
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
    // Phase N: the row's button gap is 8.0 — the timeline
    // card's spacing — not the chips' `CHIP_GAP` pitch it
    // borrowed before. Keys, label and handlers unchanged.
    let mut r = row().key("timeline-loop-row").gap(8.0).center_y();
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

/// Height of ONE sub-lane row of the clips lane in
/// points: a slim strip in the marker row's spirit
/// ([`MARKER_H`] is 14), just tall enough for one bar.
/// The lane stacks these rows (audio rows above FX
/// rows, overlapping same-kind spans on separate rows)
/// and is [`clips_lane_height`] tall overall.
pub const CLIPS_H: f32 = 10.0;

/// Height of one clip bar inside its sub-lane row: the
/// row minus a point of breathing room above and
/// below.
pub const CLIP_BAR_H: f32 = 8.0;

/// Vertical gap between two stacked sub-lane rows of
/// the clips lane.
pub const CLIP_ROW_GAP: f32 = 2.0;

/// Total height of the clips lane for `row_count`
/// stacked sub-lane rows: one row always reserves the
/// classic single-lane height ([`CLIPS_H`]), and each
/// further row adds its height plus [`CLIP_ROW_GAP`].
pub fn clips_lane_height(row_count: usize) -> f32 {
    if row_count == 0 {
        CLIPS_H
    } else {
        row_count as f32 * CLIPS_H + (row_count - 1) as f32 * CLIP_ROW_GAP
    }
}

/// The clips lane's audio colour: the 2.0 palette's
/// primary accent.
pub const AUDIO_SPAN_RGB: u32 = 0x6D8CFF;

/// The clips lane's FX colour: the 2.0 palette's
/// secondary accent.
pub const FX_SPAN_RGB: u32 = 0xA184FF;

/// The alpha a muted or hidden clip's bar is drawn at:
/// the clip is still on the lane, just visibly off.
pub const CLIP_DIM_ALPHA: f32 = 0.35;

/// Which kind of clip a [`ClipSpan`] summarises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// An audio clip (`Project::audio`).
    Audio,
    /// An FX clip (`Project::fx_clips`).
    Fx,
}

/// One clip's span on the timeline, in fractional
/// frames: the window `from_frame..to_frame` the clip
/// occupies, plus whether it is muted (audio) or hidden
/// (FX) — flagged, not filtered out, so the lane can
/// draw it dimmed (see [`clip_spans`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipSpan {
    /// Which kind of clip this is (it picks the bar's
    /// colour).
    pub kind: SpanKind,
    /// The frame the clip starts on (fractional for FX
    /// clips, whose stored start is an `f32`).
    pub from_frame: f32,
    /// The frame the clip ends on: start plus length.
    pub to_frame: f32,
    /// Audio: the clip is muted. FX: the clip is hidden.
    /// Either way the bar draws at [`CLIP_DIM_ALPHA`].
    pub muted_or_hidden: bool,
}

/// The timeline's total content width in points for
/// `frame_count` frames: every chip plus the gaps
/// between them — the width the ruler, chips and marker
/// rows lay out to, and the width [`span_rect`] clamps
/// against. Zero frames have no width.
pub fn timeline_width(frame_count: usize) -> f32 {
    if frame_count == 0 {
        return 0.0;
    }
    frame_count as f32 * CHIP_PITCH - CHIP_GAP
}

/// Every clip in the project as a [`ClipSpan`]: audio
/// clips first (in project order), then FX clips (in
/// project order). See the module docs for the audio
/// length probe and its one-frame fallback.
pub fn clip_spans(core: &EditorCore) -> Vec<ClipSpan> {
    let mut spans = Vec::new();
    for (i, clip) in core.project.audio.iter().enumerate() {
        let from = clip.start_frame as f32;
        // The clip's length lives only in its file, so
        // it is probed; an unreadable file (or a probe
        // that rounds to nothing) falls back to a
        // one-frame tick at the start frame — the lane
        // marks that a clip starts there and invents no
        // duration.
        let len = stykanimate_rs::audio_ops::clip_duration_frames(core, i)
            .map(|frames| (frames as f32).max(1.0))
            .unwrap_or(1.0);
        spans.push(ClipSpan {
            kind: SpanKind::Audio,
            from_frame: from,
            to_frame: from + len,
            muted_or_hidden: clip.muted,
        });
    }
    for entry in &core.project.fx_clips {
        spans.push(ClipSpan {
            kind: SpanKind::Fx,
            from_frame: entry.clip.start,
            to_frame: entry.clip.start + entry.clip.duration,
            muted_or_hidden: entry.hidden,
        });
    }
    spans
}

/// A span's rectangle in timeline points, as
/// `[x, width]` in the chips' geometry: frame `f`'s
/// chip starts at `f * (CHIP_W + CHIP_GAP)`, and a span
/// ending at frame `t` ends at the end of chip `t - 1`,
/// so fractional endpoints interpolate linearly through
/// the pitch. Both edges clamp to the timeline's extent
/// ([`timeline_width`]): a span running past the last
/// frame draws up to the last chip and no further, and
/// a span wholly past the end comes back zero-width at
/// the end. A non-finite endpoint, or no frames, gives
/// `[0.0, 0.0]`.
pub fn span_rect(span: &ClipSpan, frame_count: usize) -> [f32; 2] {
    if frame_count == 0 || !span.from_frame.is_finite() || !span.to_frame.is_finite() {
        return [0.0, 0.0];
    }
    let total = timeline_width(frame_count);
    let stride = CHIP_PITCH;
    let x = (span.from_frame * stride).clamp(0.0, total);
    let right = (span.to_frame * stride - CHIP_GAP).clamp(x, total);
    [x, right - x]
}

/// The lane colour for a span kind, as a palette token
/// ([`AUDIO_SPAN_RGB`] / [`FX_SPAN_RGB`]).
pub fn span_rgb(kind: SpanKind) -> u32 {
    match kind {
        SpanKind::Audio => AUDIO_SPAN_RGB,
        SpanKind::Fx => FX_SPAN_RGB,
    }
}

/// Which sub-lane row each span paints on (Wave 5, audit
/// D2/T2): audio rows sit above FX rows, and within one
/// kind, spans are first-fit onto rows in span order — a
/// span takes the first row of its kind whose last span
/// ends at or before it starts (judged on the drawn
/// [`span_rect`]s, so spans merely touching at an edge
/// still share a row). `None` for a span that draws
/// nothing (zero width): it takes no row. Row indices
/// are global: the first FX row is numbered after ALL
/// audio rows, never interleaved, so the kind split
/// holds structurally even when no spans overlap.
pub fn clip_lane_rows(spans: &[ClipSpan], frame_count: usize) -> Vec<Option<usize>> {
    let mut rows: Vec<Option<usize>> = vec![None; spans.len()];
    // Per kind, the end x of the last span placed on each
    // of that kind's rows. Audio is placed first so the
    // FX rows can be numbered after ALL audio rows.
    let mut audio_ends: Vec<f32> = Vec::new();
    let mut fx_ends: Vec<f32> = Vec::new();
    for want_fx in [false, true] {
        let base = audio_ends.len();
        let ends = if want_fx { &mut fx_ends } else { &mut audio_ends };
        for (i, span) in spans.iter().enumerate() {
            if matches!(span.kind, SpanKind::Fx) != want_fx {
                continue;
            }
            let [x, w] = span_rect(span, frame_count);
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
            rows[i] = Some(if want_fx { base + local } else { local });
        }
    }
    rows
}

/// How many sub-lane rows the lane needs for `spans`:
/// the highest assigned row plus one, or 0 when no span
/// draws (an empty lane still reserves one row's
/// height — see [`clips_lane_height`]).
pub fn clip_lane_row_count(spans: &[ClipSpan], frame_count: usize) -> usize {
    clip_lane_rows(spans, frame_count)
        .into_iter()
        .flatten()
        .max()
        .map_or(0, |m| m + 1)
}

/// The clips lane's drawing as one `Shape` in a nominal
/// `timeline_width × clips_lane_height` space (the fx
/// strip's canvas idiom): one bar per span at
/// [`span_rect`]'s rect, on its [`clip_lane_rows`]
/// sub-lane row, skipped when zero-width. A bar is a
/// round-cap line of [`CLIP_BAR_H`] — a capsule exactly
/// the span's width — except a span narrower than the bar
/// is tall, which draws as an octagon dot at its centre
/// (the stage's polygon-circle idiom), so a very short
/// clip still marks its place. Muted/hidden spans draw
/// at [`CLIP_DIM_ALPHA`].
pub fn clips_shape(spans: &[ClipSpan], frame_count: usize) -> Shape {
    let total = timeline_width(frame_count);
    let mut shape = Shape::new();
    if total <= 0.0 {
        return shape;
    }
    let rows = clip_lane_rows(spans, frame_count);
    let row_count = rows.iter().flatten().max().map_or(0, |m| m + 1);
    let r = CLIP_BAR_H / 2.0;
    for (span, row) in spans.iter().zip(rows.iter()) {
        let [x, w] = span_rect(span, frame_count);
        let Some(row) = row else {
            continue;
        };
        if w <= 0.0 {
            continue;
        }
        let cy = *row as f32 * (CLIPS_H + CLIP_ROW_GAP) + CLIPS_H / 2.0;
        let base = hex_color(span_rgb(span.kind)).unwrap_or(Color::WHITE);
        let color = if span.muted_or_hidden {
            base.alpha(CLIP_DIM_ALPHA)
        } else {
            base
        };
        if w >= CLIP_BAR_H {
            shape = shape.line([x + r, cy], [x + w - r, cy], CLIP_BAR_H, LineCap::Round, color);
        } else {
            let cx = x + w / 2.0;
            let dot: Vec<[f32; 2]> = (0..8)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / 8.0;
                    [cx + r * a.cos(), cy + r * a.sin()]
                })
                .collect();
            shape = shape.fill(dot, color);
        }
    }
    shape.view_box(total, clips_lane_height(row_count))
}

/// The clips lane, the scrolled column's last row in the
/// timeline (Phase M; moved inside the scroll viewport
/// in Wave 5, see the module docs): a content-width
/// canvas keyed `timeline-clips` carrying
/// [`clips_shape`], as tall as its stacked sub-lane rows
/// need. `None` when the project has no audio and no FX
/// clips — the lane exists only when it has something
/// to show.
fn clips_lane(core: &EditorCore) -> Option<El> {
    let spans = clip_spans(core);
    if spans.is_empty() {
        return None;
    }
    let h = clips_lane_height(clip_lane_row_count(&spans, core.frame_count()));
    let shape = clips_shape(&spans, core.frame_count());
    Some(
        canvas(shape)
            .key("timeline-clips")
            .w(timeline_width(core.frame_count()))
            .h(h),
    )
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
    use stykanimate_rs::model::{AudioClip, CameraKey, FxClipEntry};
    use stykanimate_rs::vfx::{ClipKind, ParticleClip};

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

    // ---- press feedback (Wave 6-D) ----

    #[test]
    fn press_target_tracks_the_pointer_and_clears_on_release() {
        assert_eq!(press_target(Phase::Down, Some(2)), Some(2));
        assert_eq!(press_target(Phase::Move, Some(0)), Some(0));
        assert_eq!(press_target(Phase::Move, Some(4)), Some(4));
        // A release clears wherever it lands — over a
        // chip or over nothing at all.
        assert_eq!(press_target(Phase::Up, Some(2)), None);
        assert_eq!(press_target(Phase::Up, None), None);
        // A press position that names no chip holds
        // nothing.
        assert_eq!(press_target(Phase::Down, None), None);
        assert_eq!(press_target(Phase::Move, None), None);
    }

    #[test]
    fn pressed_fill_is_the_press_overlay_composite() {
        let fill = Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 };
        // The dark theme's press token: white at 0.03.
        let press = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.03 };
        let out = pressed_fill(fill, press);
        // Source-over at full strength: each channel
        // moves 3% of the way to white; the fill's alpha
        // is kept (source-over onto an opaque fill stays
        // opaque — `mix` alone would lerp it to 0.97).
        assert!((out.r - 0.224).abs() < 1e-6, "r: {}", out.r);
        assert!((out.g - 0.418).abs() < 1e-6, "g: {}", out.g);
        assert!((out.b - 0.612).abs() < 1e-6, "b: {}", out.b);
        assert_eq!(out.a, 1.0);
        // A transparent press colour changes nothing.
        assert_eq!(pressed_fill(fill, Color::TRANSPARENT), fill);
    }

    #[test]
    fn timeline_builds_with_a_chip_held() {
        // Builder smoke over the held branch (the gesture
        // itself needs a runtime-made `Drag`; its
        // bookkeeping is the pure `press_target` above):
        // with the press record set the timeline still
        // builds, and the record is cleared afterwards so
        // no other test on this thread inherits it.
        let state = demo_state();
        let t = rune::tok::theme(true);
        set_pressed_chip(Some(0));
        assert_eq!(pressed_chip(), Some(0));
        let _held = timeline_marks(&state, &t);
        set_pressed_chip(None);
        assert_eq!(pressed_chip(), None);
        let _resting = timeline_marks(&state, &t);
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
    fn timeline_marks_builds_in_both_collapse_forms() {
        // Focused Wave (D1) builder smoke: the card
        // builds expanded (the default every existing
        // literal carries) and collapsed (chips row
        // only), on the demo and on an empty project —
        // the empty state's precedence over the
        // collapsed form must not panic either. Key-level
        // proof of which rows mount is the Runtime/snap
        // layer's job (D2's round-trip tests); this pins
        // that both assembly paths construct.
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        let _expanded = timeline_marks(&state, &t);
        state.frame_ops.timeline_collapsed = true;
        let _collapsed = timeline_marks(&state, &t);
        state.core.project.frames.clear();
        let _collapsed_empty = timeline_marks(&state, &t);
        state.frame_ops.timeline_collapsed = false;
        let _expanded_empty = timeline_marks(&state, &t);
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

    // ---- Wave 10 (audit F1/F2): the marker row's
    // current-column state, its collapse when it has
    // nothing to show, and the shared chip pitch ----

    #[test]
    fn any_markers_needs_one_marked_frame() {
        assert!(!any_markers(&[]));
        assert!(!any_markers(&[(false, false); 5]));
        assert!(any_markers(&[(false, false), (true, false)]));
        assert!(any_markers(&[(false, true), (false, false)]));
        // The computed vectors agree: the marked fixture
        // qualifies, the demo's all-blank one does not.
        assert!(any_markers(&markers_for_project(&marked_core())));
        assert!(!any_markers(&markers_for_project(&EditorCore::new())));
    }

    #[test]
    fn marker_row_collapses_on_the_marker_free_demo() {
        // The demo project HAS pose keyframes (a frame-0
        // pose is one, by `figure_keyframes`' own rule)
        // but no camera or graph markers — the row is
        // blank there, so it is not mounted. The mount
        // predicate is the row's own content, not pose
        // keyframes.
        let t = rune::tok::theme(true);
        let core = EditorCore::new();
        assert!(
            !stykanimate_rs::stats::figure_keyframes(&core.project, 0).is_empty(),
            "the demo figure is posed, so it has pose keyframes"
        );
        assert!(marker_row(&core, &t).is_none());
    }

    #[test]
    fn marker_row_survives_a_camera_key_with_no_pose_keyframes() {
        // The other direction, and the counterexample
        // that rules `figure_keyframes` out as the
        // predicate: strip every pose (no figure has a
        // pose keyframe anywhere), plant one camera key,
        // and the row must still mount — its content is
        // the camera marker, not poses.
        let t = rune::tok::theme(true);
        let mut core = EditorCore::new();
        for frame in &mut core.project.frames {
            frame.poses.clear();
        }
        assert!(stykanimate_rs::stats::figure_keyframes(&core.project, 0).is_empty());
        core.project.set_camera_key(2, 480.0, 360.0, 1.0);
        assert!(marker_row(&core, &t).is_some());
    }

    #[test]
    fn marker_glyph_color_matches_the_chips_current_treatment() {
        let accent = Color {
            r: 0.43,
            g: 0.55,
            b: 1.0,
            a: 1.0,
        };
        let secondary = Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        };
        // Off the current frame: the standing colours —
        // the camera diamond in the accent, the graph
        // dot in the secondary tone.
        assert_eq!(marker_glyph_color(false, true, accent, secondary), accent);
        assert_eq!(
            marker_glyph_color(false, false, accent, secondary),
            secondary
        );
        // On the current frame both go white — the
        // current chip's content colour on its accent
        // fill, so the glyph survives the cell's new
        // accent ground.
        assert_eq!(
            marker_glyph_color(true, true, accent, secondary),
            Color::WHITE
        );
        assert_eq!(
            marker_glyph_color(true, false, accent, secondary),
            Color::WHITE
        );
    }

    #[test]
    fn timeline_builds_with_the_current_cell_highlighted() {
        // Builder smoke over the F1 branch (the fill
        // itself is proven by `marker_glyph_color`'s
        // test and the mount by `marker_row`'s — `El`
        // has no public tree-walk): the row is mounted
        // for the marked fixture, and the timeline
        // builds with the playhead on a marked frame
        // (frame 2, both glyphs in the current cell)
        // and on an unmarked one (frame 1 — its current
        // cell is blank but still accent-filled, the
        // column state F1 asks for).
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        state.core = marked_core();
        assert!(marker_row(&state.core, &t).is_some());
        state.core.goto(2);
        assert_eq!(state.core.current, 2);
        let _ = timeline_marks(&state, &t);
        state.core.goto(1);
        let _ = timeline_marks(&state, &t);
    }

    #[test]
    fn chip_pitch_is_one_chip_plus_one_gap() {
        // The stride the dopesheet lane imports: one
        // chip plus the gap after it, 34 points at the
        // shell's geometry — and the helpers that read
        // on the pitch agree with it.
        assert_eq!(CHIP_PITCH, CHIP_W + CHIP_GAP);
        assert_eq!(CHIP_PITCH, 34.0);
        assert_eq!(timeline_width(5), 5.0 * CHIP_PITCH - CHIP_GAP);
        assert_eq!(ruler_frame_at_x(CHIP_PITCH, 5), 1);
        assert_eq!(ruler_frame_at_x(CHIP_PITCH - 0.1, 5), 0);
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
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
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
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
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

    #[test]
    fn timeline_card_builds_empty_and_populated() {
        // Wave 9 (audit D8/T9): on an empty project the
        // card takes the empty-state branch in place of
        // the scroll row; with frames it takes the scroll
        // row branch. `El` has no public tree-walk (see
        // the marker tests), so this is the builder-smoke
        // idiom over both branches — the branch condition
        // itself is the `frame_count() == 0` the loop-row
        // test above pins, and the non-empty branch is
        // the one every other smoke test in this module
        // already exercises.
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        state.core.project.frames.clear();
        assert_eq!(state.core.frame_count(), 0);
        let _ = timeline_marks(&state, &t);
        let state = demo_state();
        assert!(state.core.frame_count() > 0);
        let _ = timeline_marks(&state, &t);
    }

    // ---- clips lane (Phase M): spans factored from the
    // project's audio and FX clips, in chip geometry ----

    /// The clips fixture: direct field writes on the demo
    /// core, the graph-duplicates precedent. One audio
    /// clip at frame 1 whose path names no real file (so
    /// the duration probe fails and the one-frame
    /// fallback applies), and one FX clip stored over
    /// frames 2..6.
    fn clipped_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio.push(AudioClip {
            path: "clips-lane-test/no-such-file.wav".to_string(),
            start_frame: 1,
            gain: 1.0,
            muted: false,
        });
        core.project.fx_clips.push(FxClipEntry {
            clip: ParticleClip {
                start: 2.0,
                duration: 4.0,
                kind: ClipKind::Burst,
                color: 0xFFFFFF,
                color2: None,
                seed: 11,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        });
        core
    }

    #[test]
    fn clip_spans_empty_project_has_no_spans() {
        // The demo project carries no audio and no FX
        // clips, so the lane would have nothing to show.
        let core = EditorCore::new();
        assert!(clip_spans(&core).is_empty());
    }

    #[test]
    fn clip_spans_audio_falls_back_to_a_one_frame_tick() {
        // The planted path names no real file, so
        // `clip_duration_frames` returns None and the
        // span is the one-frame fallback at the start
        // frame — no duration is invented.
        let core = clipped_core();
        let spans = clip_spans(&core);
        assert_eq!(spans.len(), 2);
        assert_eq!(
            spans[0],
            ClipSpan {
                kind: SpanKind::Audio,
                from_frame: 1.0,
                to_frame: 2.0,
                muted_or_hidden: false,
            }
        );
    }

    #[test]
    fn clip_spans_fx_uses_its_stored_window() {
        // Start 2.0, duration 4.0 → the span is [2, 6].
        let core = clipped_core();
        let spans = clip_spans(&core);
        assert_eq!(
            spans[1],
            ClipSpan {
                kind: SpanKind::Fx,
                from_frame: 2.0,
                to_frame: 6.0,
                muted_or_hidden: false,
            }
        );
    }

    #[test]
    fn clip_spans_flags_muted_and_hidden_clips() {
        let mut core = clipped_core();
        core.project.audio[0].muted = true;
        core.project.fx_clips[0].hidden = true;
        let spans = clip_spans(&core);
        assert!(spans[0].muted_or_hidden, "muted audio is flagged");
        assert!(spans[1].muted_or_hidden, "hidden fx is flagged");
        // The unflagged fixture reads unflagged.
        let spans = clip_spans(&clipped_core());
        assert!(!spans[0].muted_or_hidden);
        assert!(!spans[1].muted_or_hidden);
    }

    #[test]
    fn span_rect_uses_the_chip_geometry() {
        // Chip pitch 34: frame 2 starts at x = 68, and a
        // span to frame 6 ends at the end of chip 5,
        // x = 200 — width 132.
        let span = ClipSpan {
            kind: SpanKind::Fx,
            from_frame: 2.0,
            to_frame: 6.0,
            muted_or_hidden: false,
        };
        assert_eq!(span_rect(&span, 8), [68.0, 132.0]);
        // A one-frame tick is exactly one chip wide.
        let tick = ClipSpan {
            kind: SpanKind::Audio,
            from_frame: 0.0,
            to_frame: 1.0,
            muted_or_hidden: false,
        };
        assert_eq!(span_rect(&tick, 8), [0.0, CHIP_W]);
        // The total width the rect clamps against.
        assert_eq!(timeline_width(8), 8.0 * (CHIP_W + CHIP_GAP) - CHIP_GAP);
        assert_eq!(timeline_width(1), CHIP_W);
        assert_eq!(timeline_width(0), 0.0);
    }

    #[test]
    fn span_rect_clamps_to_the_timeline() {
        // Running past the last frame: frame 4 of 5 starts
        // at 136 and the span draws to the timeline's end
        // (166) — one chip's width, no further.
        let span = ClipSpan {
            kind: SpanKind::Fx,
            from_frame: 4.0,
            to_frame: 9.0,
            muted_or_hidden: false,
        };
        assert_eq!(span_rect(&span, 5), [136.0, CHIP_W]);
        // Wholly past the end: zero width, at the end.
        let past = ClipSpan {
            kind: SpanKind::Fx,
            from_frame: 9.0,
            to_frame: 12.0,
            muted_or_hidden: false,
        };
        assert_eq!(span_rect(&past, 5), [timeline_width(5), 0.0]);
        // No frames, or a non-finite endpoint: nothing.
        assert_eq!(span_rect(&span, 0), [0.0, 0.0]);
        let nan = ClipSpan {
            from_frame: f32::NAN,
            ..span
        };
        assert_eq!(span_rect(&nan, 5), [0.0, 0.0]);
    }

    #[test]
    fn clips_lane_is_absent_without_clips_and_builds_with_them() {
        let t = rune::tok::theme(true);
        let mut state = demo_state();
        // Clip-free: no lane element, and the timeline
        // builds exactly as it did before the lane.
        assert!(clips_lane(&state.core).is_none());
        let _ = timeline_marks(&state, &t);
        // With clips planted: the lane exists, the whole
        // timeline still builds, and the shape carries
        // bars (on the 5-frame demo the fx span clamps to
        // frames 2..5 and still draws).
        state.core = clipped_core();
        assert!(clips_lane(&state.core).is_some());
        let _ = timeline_marks(&state, &t);
        let shape = clips_shape(&clip_spans(&state.core), state.core.frame_count());
        assert!(!shape.is_empty());
        // No frames at all: an empty shape, no panic.
        assert!(clips_shape(&clip_spans(&state.core), 0).is_empty());
    }

    fn span(kind: SpanKind, from: f32, to: f32) -> ClipSpan {
        ClipSpan {
            kind,
            from_frame: from,
            to_frame: to,
            muted_or_hidden: false,
        }
    }

    #[test]
    fn clip_lane_rows_split_kinds_and_stack_overlaps() {
        // Non-overlapping mixed kinds: the kind split
        // alone puts FX below audio (audit D2/T2) — the
        // audio span takes row 0, the FX span row 1.
        let spans = vec![span(SpanKind::Audio, 0.0, 2.0), span(SpanKind::Fx, 3.0, 5.0)];
        assert_eq!(clip_lane_rows(&spans, 8), vec![Some(0), Some(1)]);
        // Two overlapping audio spans stack onto two
        // audio rows; a third starting after the first
        // ends reuses row 0 (touching at an edge counts
        // as free — judged on the drawn rects).
        let spans = vec![
            span(SpanKind::Audio, 0.0, 4.0),
            span(SpanKind::Audio, 2.0, 6.0),
            span(SpanKind::Audio, 4.0, 6.0),
        ];
        assert_eq!(clip_lane_rows(&spans, 8), vec![Some(0), Some(1), Some(0)]);
        assert_eq!(clip_lane_row_count(&spans, 8), 2);
        // FX rows start after ALL audio rows: one audio
        // row plus two stacked FX rows is rows 0, 1, 2 —
        // never interleaved with the audio band.
        let spans = vec![
            span(SpanKind::Fx, 0.0, 4.0),
            span(SpanKind::Audio, 0.0, 2.0),
            span(SpanKind::Fx, 2.0, 6.0),
        ];
        assert_eq!(clip_lane_rows(&spans, 8), vec![Some(1), Some(0), Some(2)]);
        assert_eq!(clip_lane_row_count(&spans, 8), 3);
        // A zero-width span (wholly past the end) takes
        // no row and forces no height.
        let spans = vec![span(SpanKind::Audio, 9.0, 12.0)];
        assert_eq!(clip_lane_rows(&spans, 5), vec![None]);
        assert_eq!(clip_lane_row_count(&spans, 5), 0);
    }

    #[test]
    fn clips_lane_height_grows_by_row_plus_gap() {
        // Zero or one row: the classic single-lane
        // height, so a non-overlapping project keeps the
        // exact lane geometry it had before stacking.
        assert_eq!(clips_lane_height(0), CLIPS_H);
        assert_eq!(clips_lane_height(1), CLIPS_H);
        assert_eq!(clips_lane_height(2), 2.0 * CLIPS_H + CLIP_ROW_GAP);
        assert_eq!(clips_lane_height(3), 3.0 * CLIPS_H + 2.0 * CLIP_ROW_GAP);
    }
}
