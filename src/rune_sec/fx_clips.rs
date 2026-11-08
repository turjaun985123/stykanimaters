//! FX clips lane + clip inspector for the Rune editor shell
//! (Phase D, section 14; Phase E, section 16 — handlers
//! moved onto the real EditorCore FX ops). Declared from
//! `main.rs` only — this is bin-crate code, not part of the
//! engine library.
//!
//! ## Integration contract
//!
//! This section needs ONE field on `State` in `main.rs`:
//!
//! ```ignore
//! pub fx_selected: Option<usize>,
//! ```
//!
//! Selection is pure view state: there is no EditorCore FX
//! selection op or field (the `ClipEditSession` in
//! `editor.rs` is the *movieclip* session, unrelated), and
//! the GPUI FX panel likewise kept no core-side FX selection
//! — it rendered one row per clip straight from
//! `fx_edit::list`. Here a row press records the clip index
//! in `State::fx_selected` and the inspector edits that clip.
//!
//! ## Ops (all real; no invented engine API)
//!
//! Every mutation goes through the `EditorCore` FX ops in
//! `src/fx_ops.rs` (Phase E, section 9), which own the undo
//! snapshot and route to the `fx_edit` backend the MCP
//! tools also use:
//!
//! * earlier / later + start stepper —
//!   `EditorCore::fx_move_clip(index, delta)` (whole-frame
//!   delta, `i32` — the codebase's move-op convention, cf.
//!   `shift_frames` / `move_figure`; the backend clamps the
//!   resulting start to >= 0 and fractional starts keep
//!   their fraction)
//! * duration stepper — `EditorCore::fx_resize_clip(index,
//!   delta)` (frame delta, `f32`; the backend clamps the
//!   resulting duration to >= 0.1)
//! * nudge row (Phase F) — start ±1/±10 frames through
//!   the same guarded move path as the steppers, and
//!   duration ±0.1 s through `fx_resize_clip`. The op's
//!   delta is FRAMES (fx_ops.rs), so the seconds step is
//!   converted at the project fps by
//!   [`seconds_to_frames`] before the call
//! * duplicate — `EditorCore::fx_duplicate_clip(index)`,
//!   returning the copy's index (the copy is inserted right
//!   after the original, in list and in time)
//! * delete — `EditorCore::fx_delete_clip(index)`
//! * colour / end colour — `EditorCore::fx_set_color` /
//!   `fx_set_color2`
//! * re-anchor — `EditorCore::fx_set_anchor(index, figure,
//!   node)`, the anchoring `add_fx_clip` performs at
//!   creation (editor.rs:2005)
//!
//! Phase D note: this section originally snapshotted
//! locally and called `fx_edit` (or wrote colour/anchor
//! fields) directly, because no EditorCore FX ops existed;
//! Phase E replaced those handlers with the ops above.
//! Selection bookkeeping stays in this module (the ops
//! know nothing of `State::fx_selected`): duplicate selects
//! the returned index, delete repairs the selection with
//! [`selection_after_delete`]. The one status line the
//! handlers still own is the no-joint re-anchor message —
//! `fx_set_anchor` assumes a valid joint, so the handler
//! checks `selected_node` first, exactly as `add_fx_clip`
//! does.
//!
//! Phase G adds a clip timeline strip (`fx-strip`): a
//! canvas above the lane rows drawing one bar per clip
//! over the project's frame range, lane-packed where
//! clips overlap in time ([`pack_lanes`]) and coloured
//! by each clip's own colour, the selected clip's bar
//! brightened and outlined. Pressing the strip selects
//! the clip under the point — the same
//! `State::fx_selected` the rows write, so strip, rows
//! and inspector always agree. The strip takes no `Cx`
//! (this section's builders never receive one), so it
//! draws in a nominal width ([`STRIP_W`], the right
//! column's canvas width) stretched to the laid-out
//! element by `Shape::view_box`, and converts presses
//! from the drag's own reported size — fractions on both
//! sides, so the two always agree.

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, Drag, El, LineCap, Phase, ScrollSpec, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fx_edit;

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Swatch colours offered for clip colour / end colour: the
/// 16-colour picker palette the GPUI colour panel used
/// (pre-removal `ui/color.rs` `PICKER_PALETTE`), so the Rune
/// shell offers the same choices the old UI did.
pub const SWATCHES: [u32; 16] = [
    0xFFFFFF, 0x9AA3B2, 0x232830, 0x000000, 0xFF5D5D, 0xFF9F43, 0xFFC65D, 0xFFE066, 0x7BE495,
    0x2EC4B6, 0x4DD0E1, 0x5DB2FF, 0x3D5A99, 0xC792FF, 0xFF7BAC, 0x8D6E63,
];

// ----------------------------------------------------------------
// Pure data helpers
// ----------------------------------------------------------------

/// Display text for a frame position: whole frames render
/// without a fraction (`4`), fractional ones with one
/// decimal (`4.5`). Clip starts/durations are `f32` (the
/// `fx_edit` move/resize ops accept fractional frames), so
/// the label must not silently truncate them.
pub fn frame_label(frame: f32) -> String {
    if frame.fract() == 0.0 {
        format!("{}", frame as i64)
    } else {
        format!("{frame:.1}")
    }
}

/// Frames spanned by `seconds` at `fps` — the conversion
/// the nudge row's duration buttons need: they are
/// labelled in seconds (±0.1 s) but
/// `EditorCore::fx_resize_clip` takes a FRAME delta
/// (fx_ops.rs: the new duration is `duration + delta`
/// frames, clamped to >= 0.1). A degenerate fps falls
/// back the way the shell's playback clock does
/// (`project.fps.max(1.0)` in `main.rs`), so the step
/// never collapses to zero.
pub fn seconds_to_frames(seconds: f32, fps: f32) -> f32 {
    seconds * fps.max(1.0)
}

/// The 1-based frame range a clip covers, e.g. `frames
/// 5–14` for the half-open window `[4, 14)`: the first
/// covered frame is `start + 1` and the last is `end`
/// (the same convention the GPUI panel's `f{start+1}`
/// label used).
pub fn window_text(start: f32, end: f32) -> String {
    format!(
        "frames {}–{}",
        frame_label(start + 1.0),
        frame_label(end)
    )
}

/// One lane row's data, built from `fx_edit::list` plus the
/// entry's colours (which `FxInfo` does not carry).
#[derive(Clone, Debug, PartialEq)]
pub struct ClipRowData {
    /// Position in `Project::fx_clips`.
    pub index: usize,
    /// Snake-case kind name (MCP vocabulary, e.g. `burst`).
    pub kind: &'static str,
    /// Row title: `{kind} #{index+1}` (clips have no name
    /// field of their own).
    pub name: String,
    /// `frames {first}–{last}` coverage text.
    pub window: String,
    /// `fig {figure+1} · node {node}` anchor text.
    pub anchor: String,
    /// Clip start frame (fractional-capable).
    pub start: f32,
    /// Clip length in frames.
    pub duration: f32,
    /// Primary colour, `0xRRGGBB`.
    pub color: u32,
    /// Gradient end colour, if any.
    pub color2: Option<u32>,
    /// Whether this row is the inspector's clip.
    pub selected: bool,
}

/// The selection if it still points at a live clip, else
/// `None` (a stale index — e.g. after an undo shrank the
/// list — must never highlight a row or open an inspector
/// on the wrong clip).
pub fn valid_selection(selected: Option<usize>, clip_count: usize) -> Option<usize> {
    selected.filter(|&i| i < clip_count)
}

/// Lane rows for every FX clip in the project, in list
/// order. `selected` is the raw view-state selection; it
/// is validated here so a stale index selects nothing.
pub fn clip_rows(core: &EditorCore, selected: Option<usize>) -> Vec<ClipRowData> {
    let selected = valid_selection(selected, core.project.fx_clips.len());
    fx_edit::list(&core.project)
        .into_iter()
        .map(|info| {
            let entry = &core.project.fx_clips[info.index];
            ClipRowData {
                index: info.index,
                kind: info.kind,
                name: format!("{} #{}", info.kind, info.index + 1),
                window: window_text(info.start, info.end),
                anchor: format!("fig {} · node {}", info.figure + 1, info.node),
                start: info.start,
                duration: info.duration,
                color: entry.clip.color,
                color2: entry.clip.color2,
                selected: selected == Some(info.index),
            }
        })
        .collect()
}

/// Everything the inspector shows/edits for one clip.
#[derive(Clone, Debug, PartialEq)]
pub struct FxInspectorData {
    /// Position in `Project::fx_clips`.
    pub index: usize,
    /// Snake-case kind name.
    pub kind: &'static str,
    /// First frame the clip covers.
    pub start: f32,
    /// Length of the clip in frames.
    pub duration: f32,
    /// `frames {first}–{last}` coverage text.
    pub window: String,
    /// Primary colour, `0xRRGGBB`.
    pub color: u32,
    /// Gradient end colour (`None` = solid colour).
    pub color2: Option<u32>,
    /// Anchored figure (0-based; display adds 1).
    pub figure: usize,
    /// Anchored node of that figure.
    pub node: usize,
}

/// Inspector data for the selected clip, or `None` when
/// nothing (valid) is selected.
pub fn inspector_data(core: &EditorCore, selected: Option<usize>) -> Option<FxInspectorData> {
    let index = valid_selection(selected, core.project.fx_clips.len())?;
    let info = &fx_edit::list(&core.project)[index];
    let entry = &core.project.fx_clips[index];
    Some(FxInspectorData {
        index,
        kind: info.kind,
        start: info.start,
        duration: info.duration,
        window: window_text(info.start, info.end),
        color: entry.clip.color,
        color2: entry.clip.color2,
        figure: entry.figure,
        node: entry.node,
    })
}

/// Selection after clip `deleted` was removed, with
/// `len_after` clips remaining: a selection before the gap
/// is untouched, one after it shifts down by one, and the
/// deleted clip's own selection lands on the clip that
/// slid into its slot (or the new last clip, or `None`
/// when the list is now empty).
pub fn selection_after_delete(
    selected: Option<usize>,
    deleted: usize,
    len_after: usize,
) -> Option<usize> {
    let sel = selected?;
    if len_after == 0 {
        return None;
    }
    if sel < deleted {
        Some(sel)
    } else if sel > deleted {
        Some(sel - 1)
    } else {
        Some(deleted.min(len_after - 1))
    }
}

// ----------------------------------------------------------------
// Clip timeline strip — pure geometry
//
// The strip works on `(start, duration)` windows in
// FRAMES (the units of `ParticleClip::start` /
// `duration`, fx_ops.rs), in `Project::fx_clips` list
// order, so an index into these windows is the clip
// index everywhere else in this module.
// ----------------------------------------------------------------

/// Nominal drawing width of the strip, in points: the
/// width the shell gives right-column canvases (the
/// waveform is mounted at 308 in `main.rs`). The shape
/// carries a `view_box` of this width, so the drawing
/// stretches to whatever width the element actually
/// lays out at; presses convert through the drag's own
/// reported width, so drawing and hit-testing agree at
/// any real width.
pub const STRIP_W: f32 = 308.0;

/// Height of one strip lane, in points. Bars are inset
/// [`BAR_INSET_Y`] top and bottom inside their lane.
pub const LANE_H: f32 = 14.0;

/// Vertical inset of a bar inside its lane, in points.
pub const BAR_INSET_Y: f32 = 2.0;

/// Minimum drawn bar width, in points: at the backend's
/// 0.1-frame duration floor a long project would
/// otherwise draw a sub-point sliver no press could
/// find. Drawing only — the clip's data is untouched.
pub const MIN_BAR_W: f32 = 2.0;

/// Outline colour of the selected clip's bar (the
/// shell's primary accent).
pub const STRIP_ACCENT: u32 = 0x6D8CFF;

/// The `(start, duration)` window of every FX clip in
/// the project, in list order.
pub fn strip_windows(core: &EditorCore) -> Vec<(f32, f32)> {
    core.project
        .fx_clips
        .iter()
        .map(|entry| (entry.clip.start, entry.clip.duration))
        .collect()
}

/// Greedy lane packing: the lane each clip's strip bar
/// occupies, in list order. Clips are considered in
/// start order (list order breaks ties) and each takes
/// the lowest lane whose last clip has ended by its
/// start — coverage is half-open, so a clip starting
/// exactly where a lane-mate ends shares that lane.
/// Clips on one lane therefore never overlap, which is
/// what makes [`clip_at`] unambiguous.
pub fn pack_lanes(clips: &[(f32, f32)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..clips.len()).collect();
    order.sort_by(|&a, &b| {
        clips[a]
            .0
            .partial_cmp(&clips[b].0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let mut lanes = vec![0usize; clips.len()];
    let mut lane_ends: Vec<f32> = Vec::new();
    for i in order {
        let (start, duration) = clips[i];
        let end = start + duration;
        let lane = match lane_ends.iter().position(|&e| e <= start) {
            Some(lane) => {
                lane_ends[lane] = end;
                lane
            }
            None => {
                lane_ends.push(end);
                lane_ends.len() - 1
            }
        };
        lanes[i] = lane;
    }
    lanes
}

/// How many lanes a packing uses (0 for no clips).
pub fn lane_count(lanes: &[usize]) -> usize {
    lanes.iter().max().map_or(0, |m| m + 1)
}

/// Strip height in points for `lanes` lanes: at least
/// one lane tall, so an empty project still draws the
/// track line in a full-height strip.
pub fn strip_height(lanes: usize) -> f32 {
    lanes.max(1) as f32 * LANE_H
}

/// The drawn rect `[x, y, w, h]` of one clip's bar in a
/// strip `w` × `h` points spanning `frames_total`
/// frames, on lane `lane` of `lanes`. The x span is the
/// clip's `[start, start + duration)` window scaled to
/// the strip, CLAMPED to the strip: a clip running past
/// the project end is cut at the right edge and one
/// starting past it collapses to zero width there (the
/// data is never clamped — only the drawing; such a bar
/// also cannot be pressed, see [`clip_at`]). A visible
/// sliver is guaranteed the [`MIN_BAR_W`] minimum unless
/// it already ends at the strip's edge. The y span is
/// the lane's band inset by [`BAR_INSET_Y`].
pub fn bar_rect(
    clip: (f32, f32),
    frames_total: f32,
    w: f32,
    h: f32,
    lane: usize,
    lanes: usize,
) -> [f32; 4] {
    let w = w.max(0.0);
    let total = if frames_total > 0.0 { frames_total } else { 1.0 };
    let (start, duration) = clip;
    let sx = (start / total * w).clamp(0.0, w);
    let mut ex = ((start + duration) / total * w).clamp(sx, w);
    if ex > sx && ex - sx < MIN_BAR_W {
        ex = (sx + MIN_BAR_W).min(w);
    }
    let lane_h = if lanes > 0 { h / lanes as f32 } else { h };
    let y = lane as f32 * lane_h + BAR_INSET_Y;
    let bh = (lane_h - 2.0 * BAR_INSET_Y).max(1.0);
    [sx, y, ex - sx, bh]
}

/// The frame under strip x `x` (element points) in a
/// strip `w` wide spanning `frames_total` frames:
/// linear, clamped to `[0, frames_total]`. A press
/// inside the element has `x < w`, so the result is
/// effectively in `[0, frames_total)`.
pub fn frame_at_x(x: f32, w: f32, frames_total: f32) -> f32 {
    if w <= 0.0 || frames_total <= 0.0 {
        return 0.0;
    }
    (x / w * frames_total).clamp(0.0, frames_total)
}

/// The lane under strip y `y` (element points) in a
/// strip `h` tall with `lanes` lanes: equal bands,
/// clamped to the last lane (a press at the very bottom
/// edge still names a lane).
pub fn lane_at_y(y: f32, h: f32, lanes: usize) -> usize {
    if h <= 0.0 || lanes == 0 {
        return 0;
    }
    let lane = (y / h * lanes as f32).floor();
    if lane < 0.0 {
        0
    } else {
        (lane as usize).min(lanes - 1)
    }
}

/// The clip (list index) whose bar covers `frame` on
/// `lane`, or `None`. Coverage is the half-open window
/// `[start, start + duration)` — at a shared boundary
/// the later clip owns the frame, matching the lane
/// rows' 1-based window text. Lanes are re-packed here
/// so the answer always matches the drawn strip.
pub fn clip_at(clips: &[(f32, f32)], frame: f32, lane: usize) -> Option<usize> {
    let lanes = pack_lanes(clips);
    clips.iter().enumerate().find_map(|(i, &(start, duration))| {
        (lanes[i] == lane && frame >= start && frame < start + duration).then_some(i)
    })
}

/// Resolve a strip press at element point (`x`, `y`) in
/// an element `w` × `h` spanning `frames_total` frames
/// to the clip it selects, or `None` for a press on
/// empty strip (which leaves the selection unchanged —
/// there is no "select nothing" gesture). This is the
/// whole press logic; the element handler is a thin
/// wrapper over it.
pub fn press_select(
    clips: &[(f32, f32)],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frames_total: f32,
) -> Option<usize> {
    if clips.is_empty() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let lanes_n = lane_count(&pack_lanes(clips)).max(1);
    let frame = frame_at_x(x, w, frames_total);
    let lane = lane_at_y(y, h, lanes_n);
    clip_at(clips, frame, lane)
}

// ----------------------------------------------------------------
// Handlers (thin wrappers over the EditorCore FX ops in
// fx_ops.rs; the ops own the snapshot, these own the
// selection)
// ----------------------------------------------------------------

/// Nudge clip `i` by `delta` whole frames via
/// `EditorCore::fx_move_clip`. Returns false (and calls
/// nothing) when `i` is out of range, or when moving
/// earlier from frame 0 — a no-op the op would only
/// snapshot (the GPUI panel's `fx-earlier` guard, kept).
fn nudge_clip_start(core: &mut EditorCore, i: usize, delta: i32) -> bool {
    let Some(entry) = core.project.fx_clips.get(i) else {
        return false;
    };
    if delta < 0 && entry.clip.start <= 0.0 {
        return false;
    }
    core.fx_move_clip(i, delta);
    true
}

/// Nudge the selected clip's start by `delta` frames.
fn nudge_start(s: &mut State, delta: i32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    nudge_clip_start(&mut s.core, i, delta);
}

/// Nudge the selected clip's duration by `delta` frames
/// via `EditorCore::fx_resize_clip` (the op applies the
/// delta itself and the backend clamps the result to
/// >= 0.1, so the handler passes the delta through
/// untouched).
fn nudge_duration(s: &mut State, delta: f32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_resize_clip(i, delta);
}

/// Nudge the selected clip's duration by `seconds`
/// seconds (the nudge row's ±0.1 s buttons), converted
/// to the frame delta `fx_resize_clip` takes at the
/// project's fps ([`seconds_to_frames`]).
fn nudge_duration_seconds(s: &mut State, seconds: f32) {
    let delta = seconds_to_frames(seconds, s.core.project.fps);
    nudge_duration(s, delta);
}

/// Duplicate the selected clip via
/// `EditorCore::fx_duplicate_clip`; the copy becomes the
/// selection, as it is the clip the user most likely
/// wants to adjust next.
fn duplicate_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    if let Some(new_index) = s.core.fx_duplicate_clip(i) {
        s.fx_selected = Some(new_index);
    }
}

/// Delete the selected clip via
/// `EditorCore::fx_delete_clip` and repair the selection
/// with [`selection_after_delete`]. The op returns
/// nothing, so whether a clip was actually removed is
/// judged by the list length.
fn delete_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let len_before = s.core.project.fx_clips.len();
    s.core.fx_delete_clip(i);
    let len_after = s.core.project.fx_clips.len();
    if len_after < len_before {
        s.fx_selected = selection_after_delete(s.fx_selected, i, len_after);
    }
}

/// Set the selected clip's primary colour via
/// `EditorCore::fx_set_color`.
fn set_color(s: &mut State, color: u32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_set_color(i, color);
}

/// Set (or clear, with `None`) the selected clip's
/// gradient end colour via `EditorCore::fx_set_color2`.
fn set_color2(s: &mut State, color2: Option<u32>) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.fx_set_color2(i, color2);
}

/// Re-anchor the selected clip to the currently selected
/// joint (`EditorCore::selected_figure` /
/// `selected_node`) via `EditorCore::fx_set_anchor` — the
/// anchoring `add_fx_clip` performs at creation. The op
/// bounds-checks only the clip index and assumes a valid
/// joint, so the no-joint case is handled here with the
/// same status message `add_fx_clip` uses; on success the
/// op's own status line stands (it kept this panel's
/// exact "FX clip N re-anchored to fig F node M."
/// wording).
fn reanchor_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let Some(node) = s.core.selected_node else {
        s.core.status = "Select a joint first, then re-anchor the FX clip.".into();
        return;
    };
    let figure = s.core.selected_figure;
    s.core.fx_set_anchor(i, figure, node);
}

/// The strip's press gesture: a press (drag Down) on a
/// clip's bar selects that clip in `State::fx_selected`
/// — the selection the lane rows and inspector already
/// share, so no new state exists. Only the Down phase
/// acts (selection is a press, not a scrub), a press on
/// empty strip changes nothing, and the conversion runs
/// over the drag's own reported element size, matching
/// the drawn fractions at any laid-out width. Selection
/// is view state: no snapshot, no status line, exactly
/// like a lane-row press.
fn strip_press(s: &mut State, d: &Drag) {
    if d.phase != Phase::Down {
        return;
    }
    let windows = strip_windows(&s.core);
    let total = s.core.frame_count().max(1) as f32;
    if let Some(i) = press_select(&windows, d.x, d.y, d.w, d.h, total) {
        s.fx_selected = Some(i);
    }
}

// ----------------------------------------------------------------
// Views
// ----------------------------------------------------------------

/// One colour swatch: a rounded square filled with the
/// colour; the current value carries a white outline
/// ring. Pressing it runs `f` (a colour-set handler).
fn swatch(key: String, hex: u32, current: bool, f: impl Fn(&mut State) + 'static) -> El {
    let fill = hex_color(hex).unwrap_or(Color::WHITE);
    let swatch = el()
        .key(key)
        .size(18.0, 18.0)
        .radius(5.0)
        .fill(fill)
        .hover(fill)
        .on_press(f);
    if current {
        swatch.outline(2.0, 1.0, Color::WHITE)
    } else {
        swatch
    }
}

/// A labelled row of swatches (two rows of eight, so the
/// inspector stays narrow), built by `make` per colour so
/// the primary-colour and end-colour rows share the code
/// but not the element keys or handlers.
fn swatch_rows(
    clip_index: usize,
    key_stem: &str,
    current: Option<u32>,
    mut make: impl FnMut(u32) -> Box<dyn Fn(&mut State)>,
) -> El {
    let mut rows = col().gap(4.0);
    for chunk in SWATCHES.chunks(8) {
        let mut row = row().gap(4.0);
        for &hex in chunk {
            let key = format!("{key_stem}-{}-{hex:06x}", clip_index);
            row = row.child(swatch(key, hex, current == Some(hex), make(hex)));
        }
        rows = rows.child(row);
    }
    rows
}

/// The strip drawing as one `Shape` in a nominal
/// `size` space ([`STRIP_W`] × [`strip_height`]): a dim
/// track line, then one filled bar per clip from
/// [`bar_rect`] in the clip's own colour; the selected
/// clip's bar is brightened (its colour mixed toward
/// white) and outlined in the accent. The shape carries
/// a `view_box` of the same size so it stretches to the
/// element's laid-out size (this section receives no
/// `Cx` to read the real one with). Bars a clip's window
/// pushes wholly past the project end clamp to zero
/// width in [`bar_rect`] and are skipped here.
pub fn strip_shape(core: &EditorCore, selected: Option<usize>, size: [f32; 2], t: &Theme) -> Shape {
    let (w, h) = (size[0], size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    shape = shape.line([0.0, h / 2.0], [w, h / 2.0], 1.0, LineCap::Butt, t.control_track);
    let windows = strip_windows(core);
    if windows.is_empty() {
        return shape.view_box(w, h);
    }
    let lanes = pack_lanes(&windows);
    let lanes_n = lane_count(&lanes).max(1);
    let total = core.frame_count().max(1) as f32;
    let selected = valid_selection(selected, windows.len());
    let accent = hex_color(STRIP_ACCENT).unwrap_or(Color::WHITE);
    for (i, entry) in core.project.fx_clips.iter().enumerate() {
        let [x, y, bw, bh] = bar_rect(windows[i], total, w, h, lanes[i], lanes_n);
        if bw <= 0.0 {
            continue;
        }
        let base = hex_color(entry.clip.color).unwrap_or(Color::WHITE);
        let fill = if selected == Some(i) {
            base.mix(Color::WHITE, 0.25)
        } else {
            base
        };
        shape = shape.fill(
            vec![[x, y], [x + bw, y], [x + bw, y + bh], [x, y + bh]],
            fill,
        );
        if selected == Some(i) {
            shape = shape.stroke_with(
                vec![[x, y], [x + bw, y], [x + bw, y + bh], [x, y + bh]],
                true,
                2.0,
                LineCap::Round,
                accent,
            );
        }
    }
    shape.view_box(w, h)
}

/// The clip timeline strip element (key `fx-strip`):
/// the [`strip_shape`] canvas at full panel width and
/// one [`LANE_H`] band per packed lane, over a card
/// fill like the graph canvas. Pressing it runs
/// [`strip_press`]. Mounted at the head of the lane, so
/// the bars sit directly over the rows they summarise.
pub fn strip_view(s: &State, t: &Theme) -> El {
    let lanes_n = lane_count(&pack_lanes(&strip_windows(&s.core))).max(1);
    let h = strip_height(lanes_n);
    canvas(strip_shape(&s.core, s.fx_selected, [STRIP_W, h], t))
        .key("fx-strip")
        .w_full()
        .h(h)
        .fill(t.card)
        .radius(8.0)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| strip_press(s, d))
}

/// The clips lane: one row per FX clip in the project —
/// colour dot, kind name, covered frame range, anchor —
/// with the selected row highlighted. A row press selects
/// the clip for the inspector (`State::fx_selected`).
pub fn fx_lane(s: &State, t: &Theme) -> El {
    let rows = clip_rows(
        &s.core,
        valid_selection(s.fx_selected, s.core.project.fx_clips.len()),
    );
    let accent = tok::accent(0, true);
    let mut list = col().key("fx-clips").gap(4.0);
    if rows.is_empty() {
        list = list.child(
            text("No FX clips in this project.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for data in rows {
        let i = data.index;
        let dot_fill = hex_color(data.color).unwrap_or(Color::WHITE);
        list = list.child(
            row()
                .key(format!("fx-clip-{i}"))
                .w_full()
                .gap(8.0)
                .center_y()
                .px(8.0)
                .py(5.0)
                .radius(6.0)
                .fill(if data.selected {
                    t.control_track
                } else {
                    t.card
                })
                .hover(if data.selected {
                    t.control_track
                } else {
                    t.control_off
                })
                .on_press(move |s: &mut State| {
                    s.fx_selected = Some(i);
                })
                .child(
                    el()
                        .size(10.0, 10.0)
                        .radius(5.0)
                        .fill(dot_fill),
                )
                .child(text(data.name).font(type_::LABEL).color(t.text))
                .child(
                    text(data.window)
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(
                    text(data.anchor)
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(if data.selected {
                    el().size(8.0, 8.0).radius(4.0).fill(accent)
                } else {
                    el().size(8.0, 8.0)
                }),
        );
    }
    col()
        .key("fx-lane")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("FX clips").font(type_::TITLE).color(t.text))
                .child(
                    text(format!("{} clip(s)", s.core.project.fx_clips.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(strip_view(s, t))
        .child(list.scroll(ScrollSpec { stick_bottom: false }).min_h(0.0))
}

/// The nudge row (Phase F): precise small moves for the
/// selected clip alongside the steppers — start
/// −10/−1/+1/+10 frames through the same guarded
/// [`nudge_start`] path as the steppers and
/// earlier/later buttons (so the frame-0 no-snapshot
/// guard still applies), and duration −0.1 s/+0.1 s via
/// [`nudge_duration_seconds`]. With no valid selection
/// it renders the inspector's no-selection guidance
/// line instead of dead buttons, mirroring
/// `fx_inspector`'s own empty state.
pub fn nudge_row(s: &State, t: &Theme) -> El {
    if valid_selection(s.fx_selected, s.core.project.fx_clips.len()).is_none() {
        return row().key("fx-nudge").w_full().child(
            text("Select an FX clip to edit it.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    row()
        .key("fx-nudge")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("Nudge")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(btn("fx-nudge-l10", "-10", false, t, |s| {
            nudge_start(s, -10)
        }))
        .child(btn("fx-nudge-l", "-1", false, t, |s| {
            nudge_start(s, -1)
        }))
        .child(btn("fx-nudge-r", "+1", false, t, |s| {
            nudge_start(s, 1)
        }))
        .child(btn("fx-nudge-r10", "+10", false, t, |s| {
            nudge_start(s, 10)
        }))
        .child(
            text("Dur")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(btn("fx-dur-dn", "-0.1s", false, t, |s| {
            nudge_duration_seconds(s, -0.1)
        }))
        .child(btn("fx-dur-up", "+0.1s", false, t, |s| {
            nudge_duration_seconds(s, 0.1)
        }))
}

/// The clip inspector for the selected FX clip: kind and
/// anchor summary, start/duration steppers (move/resize),
/// earlier/later nudge buttons (the GPUI panel's
/// `fx-earlier`/`fx-later`), a nudge row (fine start
/// moves and ±0.1 s duration steps), colour + end-colour
/// swatch rows, a re-anchor button, and
/// duplicate/delete. With no valid selection it shows
/// the nudge row's guidance line instead.
pub fn fx_inspector(s: &State, t: &Theme) -> El {
    let Some(data) = inspector_data(&s.core, s.fx_selected) else {
        return col()
            .key("fx-inspector")
            .w_full()
            .child(nudge_row(s, t));
    };
    let i = data.index;
    let mut panel = col()
        .key("fx-inspector")
        .w_full()
        .gap(8.0)
        .child(
            text(format!("{} · clip {}", data.kind, i + 1))
                .font(type_::TITLE)
                .color(t.text),
        )
        .child(
            text(format!(
                "{} · anchor fig {} · node {}",
                data.window,
                data.figure + 1,
                data.node
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        // Start stepper: -/+ move the clip by one frame
        // (EditorCore::fx_move_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Start")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn(&format!("fx-start-dec-{i}"), "-", false, t, |s| {
                    nudge_start(s, -1)
                }))
                .child(
                    text(frame_label(data.start + 1.0))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(btn(&format!("fx-start-inc-{i}"), "+", false, t, |s| {
                    nudge_start(s, 1)
                })),
        )
        // Duration stepper: -/+ resize by one frame
        // (EditorCore::fx_resize_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Duration")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn(&format!("fx-dur-dec-{i}"), "-", false, t, |s| {
                    nudge_duration(s, -1.0)
                }))
                .child(
                    text(frame_label(data.duration))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(btn(&format!("fx-dur-inc-{i}"), "+", false, t, |s| {
                    nudge_duration(s, 1.0)
                })),
        )
        // Earlier / later: the GPUI panel's per-row nudge
        // buttons, same ops as the start stepper.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(&format!("fx-earlier-{i}"), "Earlier", false, t, |s| {
                    nudge_start(s, -1)
                }))
                .child(btn(&format!("fx-later-{i}"), "Later", false, t, |s| {
                    nudge_start(s, 1)
                }))
                .child(el().grow(1.0))
                .child(btn(&format!("fx-dup-{i}"), "Duplicate", false, t, |s| {
                    duplicate_selected(s)
                }))
                .child(btn(&format!("fx-del-{i}"), "Delete", false, t, |s| {
                    delete_selected(s)
                })),
        )
        // Nudge row: fine start moves (±1/±10 frames)
        // and ±0.1 s duration steps.
        .child(nudge_row(s, t))
        // Colours: primary, then gradient end colour
        // (with a Solid chip clearing color2 to None).
        .child(
            text("Colour")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(swatch_rows(i, "fx-color", Some(data.color), |hex| {
            Box::new(move |s: &mut State| set_color(s, hex))
        }))
        .child(
            text("End colour")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(
                    &format!("fx-color2-none-{i}"),
                    "Solid",
                    data.color2.is_none(),
                    t,
                    |s| set_color2(s, None),
                ))
                .child(
                    text(if data.color2.is_none() {
                        "no gradient"
                    } else {
                        "gradient end"
                    })
                    .font(type_::LABEL)
                    .color(t.text_secondary),
                ),
        )
        .child(swatch_rows(i, "fx-color2", data.color2, |hex| {
            Box::new(move |s: &mut State| set_color2(s, Some(hex)))
        }))
        // Anchor: re-anchor to the selected joint.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(
                    &format!("fx-anchor-{i}"),
                    "Anchor to joint",
                    false,
                    t,
                    |s| reanchor_selected(s),
                ))
                .child(
                    text("uses the selected figure joint")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        );
    panel = panel.child(
        text(format!(
            "seed {} · angle {:.0}° · length {:.0}",
            s.core.project.fx_clips[i].clip.seed,
            s.core.project.fx_clips[i].clip.angle,
            s.core.project.fx_clips[i].clip.length
        ))
        .font(type_::LABEL)
        .color(t.text_secondary),
    );
    panel
}

/// Lane + inspector stacked — the whole section 14
/// surface, for the shell layout to mount as one block.
pub fn fx_section(s: &State, t: &Theme) -> El {
    col()
        .key("fx-section")
        .w_full()
        .gap(10.0)
        .child(fx_lane(s, t))
        .child(fx_inspector(s, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::FxClipEntry;
    use stykanimate_rs::vfx::{ClipKind, ParticleClip};

    fn entry(start: f32, duration: f32, kind: ClipKind, color: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind,
                color,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 1,
            node: 5,
        }
    }

    fn core_with(clips: Vec<FxClipEntry>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fx_clips = clips;
        core
    }

    #[test]
    fn frame_label_drops_integral_fractions_only() {
        assert_eq!(frame_label(4.0), "4");
        assert_eq!(frame_label(0.0), "0");
        assert_eq!(frame_label(4.5), "4.5");
        assert_eq!(frame_label(12.26), "12.3");
    }

    #[test]
    fn window_text_is_one_based_first_to_last_covered() {
        // [4, 14) covers frames 5..=14 in 1-based terms.
        assert_eq!(window_text(4.0, 14.0), "frames 5–14");
        assert_eq!(window_text(0.0, 16.0), "frames 1–16");
    }

    #[test]
    fn clip_rows_carry_kind_window_anchor_colour_and_selection() {
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF),
        ]);
        let rows = clip_rows(&core, Some(1));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "burst #1");
        assert_eq!(rows[0].window, "frames 5–14");
        assert_eq!(rows[0].anchor, "fig 2 · node 5");
        assert_eq!(rows[0].color, 0xFF5D5D);
        assert!(!rows[0].selected);
        assert_eq!(rows[1].kind, "shock_ring");
        assert!(rows[1].selected);
        // Empty project rows nothing; stale selection selects nothing.
        assert!(clip_rows(&core_with(vec![]), Some(0)).is_empty());
        assert!(clip_rows(&core, Some(9)).iter().all(|r| !r.selected));
    }

    #[test]
    fn inspector_data_needs_a_valid_selection() {
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFFC65D)]);
        assert!(inspector_data(&core, None).is_none());
        assert!(inspector_data(&core, Some(1)).is_none(), "out of range");
        let data = inspector_data(&core, Some(0)).expect("valid selection");
        assert_eq!(data.kind, "burst");
        assert_eq!(data.start, 4.0);
        assert_eq!(data.duration, 10.0);
        assert_eq!(data.window, "frames 5–14");
        assert_eq!(data.color, 0xFFC65D);
        assert_eq!(data.color2, None);
        assert_eq!(data.figure, 1);
        assert_eq!(data.node, 5);
    }

    #[test]
    fn selection_after_delete_repairs_each_position() {
        // Nothing selected stays nothing.
        assert_eq!(selection_after_delete(None, 0, 3), None);
        // Empty list clears the selection.
        assert_eq!(selection_after_delete(Some(0), 0, 0), None);
        // Before the gap: untouched.
        assert_eq!(selection_after_delete(Some(0), 2, 4), Some(0));
        // After the gap: shifts down one.
        assert_eq!(selection_after_delete(Some(3), 1, 4), Some(2));
        // The deleted clip itself: the clip that slid in,
        // or the new last clip when the tail was deleted.
        assert_eq!(selection_after_delete(Some(1), 1, 4), Some(1));
        assert_eq!(selection_after_delete(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn nudge_clip_start_moves_by_whole_frame_deltas() {
        let mut core = core_with(vec![entry(4.5, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        // The op takes a whole-frame delta; a fractional
        // start keeps its fraction.
        assert!(nudge_clip_start(&mut core, 0, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 5.5);
        assert!(nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 4.5);
        // A delta that would cross frame 0 is clamped by
        // the backend, and the call still counts as issued.
        let mut core = core_with(vec![entry(0.5, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        assert!(nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
    }

    #[test]
    fn seconds_to_frames_scales_with_the_project_fps() {
        // The ±0.1 s duration nudge at the frame rates
        // projects actually use: 12 fps (the demo) ->
        // 1.2 frames, 24 -> 2.4, 30 -> 3.0.
        let eps = 1e-4;
        assert!((seconds_to_frames(0.1, 12.0) - 1.2).abs() < eps);
        assert!((seconds_to_frames(0.1, 24.0) - 2.4).abs() < eps);
        assert!((seconds_to_frames(0.1, 30.0) - 3.0).abs() < eps);
        // The negative step mirrors the positive one.
        assert!((seconds_to_frames(-0.1, 24.0) + 2.4).abs() < eps);
        // A degenerate fps never collapses the step to 0
        // (the `fps.max(1.0)` fallback).
        assert!((seconds_to_frames(0.1, 0.0) - 0.1).abs() < eps);
    }

    #[test]
    fn duration_nudge_step_resizes_by_the_converted_delta() {
        // The op-level effect of the +0.1 s button at
        // 30 fps: the frame delta is 3, so a 10-frame
        // clip grows to 13 frames.
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        core.fx_resize_clip(0, seconds_to_frames(0.1, 30.0));
        let duration = core.project.fx_clips[0].clip.duration;
        assert!((duration - 13.0).abs() < 1e-4, "duration {duration}");
        // And the -0.1 s button at 12 fps removes
        // 1.2 frames.
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        core.fx_resize_clip(0, seconds_to_frames(-0.1, 12.0));
        let duration = core.project.fx_clips[0].clip.duration;
        assert!((duration - 8.8).abs() < 1e-4, "duration {duration}");
    }

    #[test]
    fn nudge_clip_start_guards_frame_zero_and_bad_index() {
        let mut core = core_with(vec![entry(0.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        // Earlier at frame 0: no call, no change (the op
        // would only take a snapshot).
        assert!(!nudge_clip_start(&mut core, 0, -1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
        // Out of range: no call, no panic.
        assert!(!nudge_clip_start(&mut core, 5, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0, "no change");
    }

    // --------------------------------------------------
    // Clip timeline strip
    // --------------------------------------------------

    fn assert_rect(actual: [f32; 4], expected: [f32; 4]) {
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert!((a - e).abs() < 1e-3, "rect {actual:?} != {expected:?}");
        }
    }

    #[test]
    fn strip_windows_reads_start_and_duration_in_list_order() {
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF),
        ]);
        assert_eq!(strip_windows(&core), vec![(4.0, 10.0), (20.0, 6.0)]);
        assert!(strip_windows(&core_with(vec![])).is_empty());
    }

    #[test]
    fn pack_lanes_puts_disjoint_clips_all_on_lane_zero() {
        assert!(pack_lanes(&[]).is_empty());
        assert_eq!(
            pack_lanes(&[(0.0, 5.0), (10.0, 5.0), (20.0, 5.0)]),
            vec![0, 0, 0]
        );
        // Touching at a boundary is not overlapping
        // (half-open coverage): the second clip reuses
        // the lane.
        assert_eq!(pack_lanes(&[(0.0, 10.0), (10.0, 5.0)]), vec![0, 0]);
    }

    #[test]
    fn pack_lanes_stacks_an_overlapping_chain() {
        // 0–10 and 5–15 overlap; 12–22 overlaps the
        // second but not the first, so it returns to
        // lane 0.
        assert_eq!(
            pack_lanes(&[(0.0, 10.0), (5.0, 10.0), (12.0, 10.0)]),
            vec![0, 1, 0]
        );
    }

    #[test]
    fn pack_lanes_stacks_nested_clips_by_depth() {
        // Two short clips inside one long one, disjoint
        // from each other: they share lane 1.
        assert_eq!(
            pack_lanes(&[(0.0, 30.0), (5.0, 5.0), (12.0, 5.0)]),
            vec![0, 1, 1]
        );
        // Fully nested three deep: one lane each.
        assert_eq!(
            pack_lanes(&[(0.0, 30.0), (2.0, 20.0), (4.0, 10.0)]),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn pack_lanes_reports_in_list_order_for_unsorted_input() {
        // Packing considers start order (clip 1 starts
        // first), but the answer is indexed by list
        // position.
        assert_eq!(pack_lanes(&[(20.0, 5.0), (0.0, 10.0)]), vec![0, 0]);
        assert_eq!(
            pack_lanes(&[(0.0, 10.0), (20.0, 5.0), (5.0, 10.0)]),
            vec![0, 0, 1]
        );
    }

    #[test]
    fn lane_count_and_strip_height_follow_the_packing() {
        assert_eq!(lane_count(&[]), 0);
        assert_eq!(lane_count(&[0, 0]), 1);
        assert_eq!(lane_count(&[0, 2, 1]), 3);
        // An empty strip is still one lane tall.
        assert_eq!(strip_height(0), LANE_H);
        assert_eq!(strip_height(1), LANE_H);
        assert_eq!(strip_height(3), 3.0 * LANE_H);
    }

    #[test]
    fn bar_rect_scales_the_window_into_the_strip() {
        // 300 pt over 30 frames = 10 pt per frame; one
        // lane of 14 pt, bar inset 2 -> y 2, height 10.
        assert_rect(
            bar_rect((0.0, 10.0), 30.0, 300.0, 14.0, 0, 1),
            [0.0, 2.0, 100.0, 10.0],
        );
        // Fractional start/duration land fractionally.
        assert_rect(
            bar_rect((4.5, 3.0), 30.0, 300.0, 14.0, 0, 1),
            [45.0, 2.0, 30.0, 10.0],
        );
        // Second lane of two: the band below the first.
        assert_rect(
            bar_rect((0.0, 10.0), 30.0, 300.0, 28.0, 1, 2),
            [0.0, 16.0, 100.0, 10.0],
        );
    }

    #[test]
    fn bar_rect_clamps_drawing_at_the_strip_end() {
        // Ending exactly at the project end: full width
        // to the edge.
        assert_rect(
            bar_rect((20.0, 10.0), 30.0, 300.0, 14.0, 0, 1),
            [200.0, 2.0, 100.0, 10.0],
        );
        // Running past the end: cut at the edge (the
        // data keeps its real duration; only the bar is
        // clamped).
        assert_rect(
            bar_rect((25.0, 10.0), 30.0, 300.0, 14.0, 0, 1),
            [250.0, 2.0, 50.0, 10.0],
        );
        // Starting past the end: zero width at the edge.
        assert_rect(
            bar_rect((40.0, 5.0), 30.0, 300.0, 14.0, 0, 1),
            [300.0, 2.0, 0.0, 10.0],
        );
        // Starting before frame 0 (an imported window):
        // cut at the left edge.
        assert_rect(
            bar_rect((-5.0, 10.0), 30.0, 300.0, 14.0, 0, 1),
            [0.0, 2.0, 50.0, 10.0],
        );
    }

    #[test]
    fn bar_rect_gives_tiny_clips_a_minimum_visible_width() {
        // 0.1 frames of 600 over 300 pt is 0.05 pt raw;
        // the bar draws at MIN_BAR_W instead.
        assert_rect(
            bar_rect((0.0, 0.1), 600.0, 300.0, 14.0, 0, 1),
            [0.0, 2.0, MIN_BAR_W, 10.0],
        );
    }

    #[test]
    fn frame_and_lane_at_convert_points_back() {
        assert_eq!(frame_at_x(0.0, 300.0, 30.0), 0.0);
        assert_eq!(frame_at_x(150.0, 300.0, 30.0), 15.0);
        assert_eq!(frame_at_x(300.0, 300.0, 30.0), 30.0);
        // Outside the element clamps to the ends.
        assert_eq!(frame_at_x(-10.0, 300.0, 30.0), 0.0);
        assert_eq!(frame_at_x(450.0, 300.0, 30.0), 30.0);
        assert_eq!(frame_at_x(10.0, 0.0, 30.0), 0.0);

        assert_eq!(lane_at_y(0.0, 28.0, 2), 0);
        assert_eq!(lane_at_y(13.0, 28.0, 2), 0);
        assert_eq!(lane_at_y(14.0, 28.0, 2), 1);
        assert_eq!(lane_at_y(27.9, 28.0, 2), 1);
        // The bottom edge itself still names the last
        // lane; a single lane answers 0 everywhere.
        assert_eq!(lane_at_y(28.0, 28.0, 2), 1);
        assert_eq!(lane_at_y(5.0, 14.0, 1), 0);
        assert_eq!(lane_at_y(5.0, 14.0, 0), 0);
    }

    #[test]
    fn clip_at_hits_the_bar_on_its_lane_only() {
        // Lanes pack to [0, 1, 0].
        let clips = [(0.0, 10.0), (5.0, 10.0), (12.0, 10.0)];
        assert_eq!(clip_at(&clips, 7.0, 0), Some(0));
        assert_eq!(clip_at(&clips, 7.0, 1), Some(1));
        assert_eq!(clip_at(&clips, 13.0, 0), Some(2));
        assert_eq!(clip_at(&clips, 13.0, 1), Some(1));
        // The gap on lane 0 between clips 0 and 2.
        assert_eq!(clip_at(&clips, 11.0, 0), None);
        // Past every clip, before frame 0, an unoccupied
        // lane, and no clips at all.
        assert_eq!(clip_at(&clips, 25.0, 0), None);
        assert_eq!(clip_at(&clips, -1.0, 0), None);
        assert_eq!(clip_at(&clips, 7.0, 5), None);
        assert_eq!(clip_at(&[], 0.0, 0), None);
    }

    #[test]
    fn clip_at_gives_a_shared_boundary_to_the_later_clip() {
        let clips = [(0.0, 10.0), (10.0, 5.0)];
        assert_eq!(clip_at(&clips, 9.99, 0), Some(0));
        assert_eq!(clip_at(&clips, 10.0, 0), Some(1));
    }

    #[test]
    fn press_select_resolves_points_to_clips() {
        // Two overlapping clips -> two lanes, strip
        // 300 x 28 over 30 frames.
        let clips = [(0.0, 10.0), (5.0, 10.0)];
        // Frame 7 on the top lane is clip 0, on the
        // bottom lane clip 1.
        assert_eq!(press_select(&clips, 70.0, 5.0, 300.0, 28.0, 30.0), Some(0));
        assert_eq!(press_select(&clips, 70.0, 20.0, 300.0, 28.0, 30.0), Some(1));
        // Frame 25 is past both clips.
        assert_eq!(press_select(&clips, 250.0, 5.0, 300.0, 28.0, 30.0), None);
        // Frame 10 on lane 0: clip 0 has ended and clip 1
        // lives on lane 1, so the press selects nothing.
        assert_eq!(press_select(&clips, 100.0, 5.0, 300.0, 28.0, 30.0), None);
        // Degenerate geometry and empty projects select
        // nothing rather than panicking.
        assert_eq!(press_select(&clips, 70.0, 5.0, 0.0, 28.0, 30.0), None);
        assert_eq!(press_select(&[], 70.0, 5.0, 300.0, 28.0, 30.0), None);
    }

    #[test]
    fn strip_shape_builds_for_empty_and_populated_projects() {
        let t = rune::tok::theme(true);
        // No clips: just the track line.
        let empty = core_with(vec![]);
        assert!(!strip_shape(&empty, None, [STRIP_W, LANE_H], &t).is_empty());
        // Clips, one selected: bars plus the outline.
        let core = core_with(vec![
            entry(0.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(5.0, 10.0, ClipKind::Smoke, 0x9AA3B2),
        ]);
        let shape = strip_shape(&core, Some(1), [STRIP_W, 2.0 * LANE_H], &t);
        assert!(!shape.is_empty());
        // A stale selection outlines nothing but still
        // builds; a zero size draws nothing.
        let shape = strip_shape(&core, Some(9), [STRIP_W, 2.0 * LANE_H], &t);
        assert!(!shape.is_empty());
        assert!(strip_shape(&core, None, [0.0, 0.0], &t).is_empty());
    }
}
