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
//!
//! Phase H switches the unambiguous micro-buttons to
//! Font Awesome glyphs through the shell's icon buttons:
//! the lane header gains a `wand-magic-sparkles` glyph,
//! the inspector's duplicate/delete become icon-only
//! (`copy` / `trash-can`), and the nudge row's
//! single-step pair become icon-only chevrons
//! (`chevron-left` / `chevron-right`). Keys and handlers
//! are unchanged. The ±10 nudge pair and the duration
//! steppers keep their text — `-10` / `+10` / `±0.1s`
//! are magnitudes a glyph cannot carry — and the strip
//! canvas is drawing code, not element code, so it is
//! untouched.
//!
//! Phase I adds two gestures. The strip press now
//! seeks: a press on empty strip moves the playhead to
//! the frame under the press ([`frame_at_x`] over the
//! nominal [`STRIP_W`] space; [`press_outcome`] splits
//! the select-or-seek decision out of the handler),
//! while a press on a bar still selects, exactly as
//! before — the fractional hit mapping the bars need
//! was renamed [`frame_frac_at_x`] to make room for
//! the seek's whole-frame [`frame_at_x`]. And the
//! inspector gains a kind stepper: prev/next buttons
//! cycling the selected clip's kind through the FX
//! browser's category order via `fx_ops::fx_kind_cycle`
//! and `EditorCore::fx_set_kind`, with the current
//! kind labelled the way the browser labels it
//! (`fx_browser::kind_label`).
//!
//! Phase J adds a Split button to the inspector: it
//! cuts the selected clip at the playhead through
//! `EditorCore::fx_split_clip`, at the playhead
//! frame's centre ([`split_frame`]) rather than on
//! the frame's integer boundary, so splitting while
//! parked on the clip's first frame still lands
//! strictly inside the clip. The op inserts the
//! second half immediately after the original, so
//! the selection stays on the first half with no
//! repair; on refusal the op sets the status line
//! and the panel adds nothing.
//!
//! Phase K adds a playhead row beside the split row:
//! To playhead moves the selected clip so its start
//! lands on the playhead frame ([`move_delta`] over
//! `EditorCore::fx_move_clip`), and End at playhead
//! resizes it so the playhead frame is the last one
//! it covers ([`end_delta`] over
//! `EditorCore::fx_resize_clip`) — the two landings
//! the ±1 steppers take many presses to reach. Both
//! ops edit the clip in place, so the selection
//! index never moves and needs no repair; both
//! handlers validate the selection the way
//! [`split_selected`] does and otherwise add
//! nothing of their own.
//!
//! Phase M adds the visibility surface for the
//! model's per-clip `hidden` flag (hidden clips are
//! skipped by the stage and by export renderers but
//! stay editable): a Hide/Show toggle in the
//! inspector ([`toggle_selected_hidden`] — no
//! EditorCore op exists for the flag, so the handler
//! follows the fx_ops no-backend pattern itself: one
//! `EditorCore::snapshot`, then the direct field
//! write, then the status line), a one-line readout
//! under the inspector's summary ([`clip_readout`],
//! key `fx-readout`), and strip bars for hidden
//! clips drawn dimmed ([`bar_alpha`] scales the
//! bar fill's alpha; geometry is untouched).
//!
//! Phase N restyles the section onto the design kit
//! without touching behaviour: the `fx-section` root
//! is a kit card ([`design::panel_col`]), the lane
//! header is a kit [`design::panel_title`] (the
//! `wand-magic-sparkles` glyph, the clip count as its
//! meta) over a kit [`design::divider`], the Colour /
//! End colour group labels are kit
//! [`design::section_label`]s, and the colour
//! swatches are the kit's 24×24 [`design::swatch`] —
//! the local 18×18 white-outline construction is
//! retired. The strip's geometry, keys and seek
//! behaviour are unchanged, as are every handler,
//! readout and status line.
//!
//! UI mission Wave 4 (audit S13-1 / S13-3):
//! [`ClipRowData`] carries the entry's `hidden` flag,
//! and a hidden clip's lane row is identifiable at
//! list level — colour dot dimmed by [`bar_alpha`],
//! name in the secondary colour, an `eye-slash`
//! marker before the selection dot. The lane's
//! no-clips line is the kit [`design::empty_state`]
//! under `wand-magic-sparkles`, text byte-identical.
//! Keys, handlers and strip geometry are unchanged.
//!
//! Wave 10 (round-2 audit of this panel) fixes four
//! proven defects. (1) The lane header's count named
//! only the total (`"3 clip(s)"`), so a project whose
//! clips were all hidden read as fully live; the
//! count now comes from [`lane_count_text`] and names
//! the hidden share while any clip is hidden. (2) The
//! start steppers could push a clip fully past the
//! project end: `fx_edit::move_clip` clamps only at
//! frame 0, and once a clip's start reaches
//! `frame_count` no frame samples it, its strip bar
//! collapses to zero width at the edge (unpressable)
//! and its timeline span draws nothing — the lane row
//! was the only surface still showing it.
//! [`nudge_clip_start`] now clamps positive deltas at
//! the project end, symmetric with its frame-0 guard;
//! the duration stepper's mirror no-op (shrinking a
//! clip already at the backend's 0.1-frame floor) is
//! refused the same way in [`nudge_duration`]. (3) A
//! lane row was one line — dot, name, window,
//! anchor, eye slot, selection dot — whose texts
//! overflow the 316 pt column at ordinary values
//! (the longest kind name with single-digit numbers
//! already exceeds the line's text budget under the
//! shell's conservative LABEL accounting; see
//! [`fx_lane`]). The row is now two lines inside the
//! same keyed, pressable shell, so nothing truncates
//! at realistic values and every marker stays inside
//! the card. Keys and handlers are unchanged
//! throughout.
//!
//! Wave T (T5): the panel's labels resolve through
//! `i18n_wire::tr` (the subtitle/camera/3D idiom) —
//! the lane title and empty state, the nudge row, and
//! the inspector's captions, steppers, icon buttons,
//! colour sections and anchor row — English
//! byte-identical, element keys untouched, pinned by
//! `fx_i18n_keys_exist_and_match_the_english_labels`.
//! The keys land via IA-T5-1. Locale-invariant
//! notation stays literal by design (the R6
//! precedent): the `-10` / `+10` / `-0.1s` / `+0.1s`
//! step magnitudes and the `-` / `+` steppers. The
//! composed readouts (`window_text`, `clip_readout`,
//! `lane_count_text`, the inspector's title/summary
//! lines) are dynamic text, not labels, and stay
//! English this wave; so do the kind names — the
//! snake-case name is the engine's serialised/MCP
//! vocabulary (`fx_edit::kind_name`), and its display
//! form is `fx_browser::kind_label`'s, another
//! module's surface.

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, Drag, El, LineCap, Phase, ScrollSpec, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fx_edit;
use stykanimate_rs::model::FxClipEntry;
use stykanimate_rs::fx_ops::fx_kind_cycle;
use stykanimate_rs::vfx::ClipKind;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::fx_browser::kind_label;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn, icon_only_btn_inert};
use crate::rune_sec::icons::{icon_el, icon_width};
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
    /// Whether the clip's hidden flag is set (UI mission
    /// Wave 4, audit S13-1): threaded from
    /// `FxClipEntry::hidden` so the lane row can dim and
    /// mark the clip at list level — before this field
    /// existed, a hidden clip's row rendered identically
    /// to a live one and the state was readable only in
    /// the strip's dimmed bar and the selected clip's
    /// inspector readout.
    pub hidden: bool,
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
                hidden: entry.hidden,
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

/// The inspector's one-line readout for one clip
/// (Phase M): `{Kind} · starts frame {first} ·
/// {duration} frames`, with ` · hidden` appended
/// while the clip's hidden flag is set. The kind is
/// named the way the FX browser names it
/// ([`kind_label`] over the engine's snake-case
/// name); the start is the 1-based first covered
/// frame and both frame numbers go through
/// [`frame_label`], so whole frames render as
/// integers and fractional ones keep their fraction
/// — the phrasing the start/duration steppers use.
pub fn clip_readout(entry: &FxClipEntry) -> String {
    let mut line = format!(
        "{} · starts frame {} · {} frames",
        kind_label(fx_edit::kind_name(entry.clip.kind)),
        frame_label(entry.clip.start + 1.0),
        frame_label(entry.clip.duration)
    );
    if entry.hidden {
        line.push_str(" · hidden");
    }
    line
}

/// The lane header's count text (Wave 10): `{total}
/// clip(s)` while every clip is live, with
/// ` · {hidden} hidden` appended while any clip is
/// hidden. The header is the panel's one summary of
/// the whole list, and a hidden clip renders nothing
/// on the stage or in exports — a bare total read as
/// "this much effect will play" even when every clip
/// was hidden. The plain form is byte-identical to
/// the pre-Wave-10 text, so nothing changes for a
/// project with nothing hidden.
pub fn lane_count_text(total: usize, hidden: usize) -> String {
    if hidden == 0 {
        format!("{total} clip(s)")
    } else {
        format!("{total} clip(s) · {hidden} hidden")
    }
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

/// The kind `dir` steps from `current` in the
/// canonical cycle order (Phase I): a named seam over
/// `stykanimate_rs::fx_ops::fx_kind_cycle`, whose
/// `FX_KIND_ORDER` walks the FX browser's five
/// categories (Explosion, Energy, Magic, Nature,
/// Particles) in the browser's row order, wrapping at
/// both ends — Wave steps forward to Burst, Burst
/// steps back to Wave. Kept here so this module's
/// handlers and tests speak one vocabulary.
pub fn cycle_kind(current: ClipKind, dir: i32) -> ClipKind {
    fx_kind_cycle(current, dir)
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

/// Alpha a hidden clip's strip bar is drawn at, as a
/// fraction of its normal opacity (Phase M): the bar
/// keeps its colour, position and size — hidden clips
/// are skipped by the stage and the exporters, and
/// the strip says so by dimming, not by removing the
/// bar (the clip is still there to select and edit).
pub const HIDDEN_BAR_ALPHA: f32 = 0.35;

/// The alpha multiplier for one clip's strip bar:
/// [`HIDDEN_BAR_ALPHA`] for a hidden clip, 1.0 (the
/// bar's colour untouched) for a visible one. Factored
/// out of [`strip_shape`] so the dimming decision is
/// testable without a `Shape`.
pub fn bar_alpha(hidden: bool) -> f32 {
    if hidden {
        HIDDEN_BAR_ALPHA
    } else {
        1.0
    }
}

/// The laid-out width of the lane row's eye-slash
/// marker slot, in points (Wave 9): `icon_el` sizes by
/// the glyph's aspect, so at 12 pt tall the 576×512
/// eye-slash lays out 13.5 pt wide — not the 12 pt the
/// live rows' spacer originally used, which shifted a
/// hidden row's trailing elements 1.5 pt left of its
/// live neighbours'. The spacer takes this width, so
/// the selection dot on the row's first line lines up
/// across hidden and live rows alike. Factored out so
/// the width the builder uses is testable without an
/// `El`.
pub fn eye_slash_slot_width() -> f32 {
    icon_width("eye-slash", 12.0).unwrap_or(12.0)
}

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

/// The fractional frame under strip x `x` (element
/// points) in a strip `w` wide spanning `frames_total`
/// frames: linear, clamped to `[0, frames_total]`. A
/// press inside the element has `x < w`, so the result
/// is effectively in `[0, frames_total)`. This is the
/// hit-testing mapping — clip windows are fractional,
/// so bar selection needs the fraction. The playhead
/// seek uses the whole-frame [`frame_at_x`] instead
/// (Phase I; this helper carried that name until the
/// seek landed).
pub fn frame_frac_at_x(x: f32, w: f32, frames_total: f32) -> f32 {
    if w <= 0.0 || frames_total <= 0.0 {
        return 0.0;
    }
    (x / w * frames_total).clamp(0.0, frames_total)
}

/// The whole frame a seek press at strip x `x` lands
/// on, where `x` is in NOMINAL strip points: the strip
/// draws in a [`STRIP_W`] space stretched over its
/// element by `view_box`, so the handler converts an
/// element x into nominal points first (see
/// [`press_outcome`]). The mapping is the proportion
/// `x / STRIP_W` of a `frame_count`-frame timeline,
/// floored and clamped to `0..frame_count - 1`: the
/// playhead can only rest on a real frame, so the
/// strip's right edge seeks the LAST frame, not one
/// past it. A project with no frames seeks frame 0
/// (there is nowhere else to be), and a non-finite `x`
/// likewise — the waveform mapping's convention
/// (`waveform_view::frame_at_x`).
pub fn frame_at_x(x: f32, frame_count: usize) -> usize {
    if frame_count == 0 || !x.is_finite() {
        return 0;
    }
    let frac = (x / STRIP_W).clamp(0.0, 1.0);
    ((frac * frame_count as f32).floor() as usize).min(frame_count - 1)
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
/// empty strip. The "select nothing" answer is this
/// function's whole contract — [`press_outcome`] layers
/// the seek on top of it for the element handler.
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
    let frame = frame_frac_at_x(x, w, frames_total);
    let lane = lane_at_y(y, h, lanes_n);
    clip_at(clips, frame, lane)
}

/// What a strip press resolves to: the clip it selects,
/// or the frame it seeks the playhead to (Phase I).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressOutcome {
    /// Select this clip (list index).
    Select(usize),
    /// Seek the playhead to this frame (0-based).
    Seek(usize),
}

/// Resolve a strip press at element point (`x`, `y`)
/// in an element `w` × `h` spanning the whole
/// timeline: a press on a bar selects it (via
/// [`press_select`], byte-for-byte the pre-seek
/// behaviour); a press on empty strip seeks. The seek
/// conversion first turns the element x into the
/// strip's nominal [`STRIP_W`] space — the drawing
/// stretches by `view_box`, so element and nominal
/// points differ whenever the laid-out element is not
/// exactly [`STRIP_W`] wide — then maps it with
/// [`frame_at_x`] over `frame_count` frames. Split
/// out of the handler so the select-or-seek decision
/// is testable without a `State`.
pub fn press_outcome(
    clips: &[(f32, f32)],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frames_total: f32,
    frame_count: usize,
) -> PressOutcome {
    if let Some(i) = press_select(clips, x, y, w, h, frames_total) {
        return PressOutcome::Select(i);
    }
    let nominal_x = if w > 0.0 { x / w * STRIP_W } else { 0.0 };
    PressOutcome::Seek(frame_at_x(nominal_x, frame_count))
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
///
/// Positive deltas are clamped at the project end
/// (Wave 10), the symmetric guard: the backend clamps
/// only at frame 0 (`fx_edit::move_clip` takes
/// `new_start.max(0.0)`), so an unclamped `+10` could
/// land a clip's start at or past `frame_count`, where
/// no frame ever samples it (`fx_edit::clips_at` needs
/// `frame_f >= start` for a `frame_f` below
/// `frame_count`), its strip bar collapses to zero
/// width at the right edge ([`bar_rect`]) and its
/// timeline span draws zero-width — the lane row would
/// be the only surface still showing the clip, and
/// every further `+` press buried it deeper. The clamp
/// keeps the clip's fraction (a split's second half at
/// x.5 still lands at y.5): the step shrinks to the
/// largest whole-frame delta whose landing stays
/// before `frame_count`, and a clip already starting
/// in the last frame — or already past the end, where
/// only an earlier move can rescue it — refuses the
/// press instead of wasting a snapshot.
fn nudge_clip_start(core: &mut EditorCore, i: usize, delta: i32) -> bool {
    let Some(entry) = core.project.fx_clips.get(i) else {
        return false;
    };
    if delta < 0 && entry.clip.start <= 0.0 {
        return false;
    }
    if delta > 0 {
        let frames = core.frame_count() as f32;
        // The largest whole-frame step whose landing
        // `start + step` stays < frames: step < frames
        // - start, so step_max = ceil(frames - start)
        // - 1 (for a whole-frame distance d, that is
        // d - 1: starting exactly d frames before the
        // end, the last in-project landing is one step
        // short of the end itself).
        let max_delta = ((frames - entry.clip.start).ceil() as i32 - 1).max(0);
        if max_delta == 0 {
            return false;
        }
        core.fx_move_clip(i, delta.min(max_delta));
        return true;
    }
    core.fx_move_clip(i, delta);
    true
}

/// Nudge the selected clip's start by `delta` frames.
/// Whether a start step in `delta`'s direction can
/// move a clip starting at `start` in a project of
/// `frames` frames — the exact mirror of
/// [`nudge_clip_start`]'s guards (Wave Q): negative
/// steps refuse at `start <= 0`; positive steps
/// refuse when the clip already starts in the last
/// frame (`start >= frames - 1`, where the clamped
/// step would be 0). The StepForm source for every
/// start-stepping control in this panel (the start
/// stepper, Earlier/Later, the nudge row), so the
/// buttons and the guard cannot disagree: at a bound
/// the step is a silent no-op today — no status, no
/// snapshot — and the button renders inert instead.
fn start_step_live(start: f32, frames: usize, delta: i32) -> bool {
    if delta < 0 {
        start > 0.0
    } else {
        start < frames as f32 - 1.0
    }
}

/// Whether shrinking a clip of `duration` frames can
/// change it — the mirror of [`nudge_duration`]'s
/// floor guard (`duration <= 0.1` refuses silently).
/// Growing is unbounded, always live.
fn duration_shrink_live(duration: f32) -> bool {
    duration > 0.1
}

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
/// untouched). One guard (Wave 10), mirroring
/// [`nudge_clip_start`]'s frame-0 guard: shrinking a
/// clip already AT the 0.1 floor is a no-op the op
/// would only snapshot and report as a resize, so the
/// press is refused before the call. A shrink that
/// crosses the floor still goes through — the backend
/// clamps its landing to 0.1, which is a real change.
fn nudge_duration(s: &mut State, delta: f32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    if delta < 0.0 && s.core.project.fx_clips[i].clip.duration <= 0.1 {
        return;
    }
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

/// The kind `dir` steps from the selected clip's kind,
/// per [`cycle_kind`]; `None` when nothing valid is
/// selected. Split from the handler so the choice is
/// testable without a `State`.
pub fn cycled_kind(core: &EditorCore, selected: Option<usize>, dir: i32) -> Option<ClipKind> {
    let index = valid_selection(selected, core.project.fx_clips.len())?;
    Some(cycle_kind(core.project.fx_clips[index].clip.kind, dir))
}

/// Cycle the selected clip's kind by `dir` steps
/// (Phase I) through `EditorCore::fx_set_kind` — the
/// op owns the snapshot and the status line ("FX clip
/// N kind set to …"); this handler owns only the
/// selection validation, like every other inspector
/// handler here.
fn cycle_selected_kind(s: &mut State, dir: i32) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let next = cycle_kind(s.core.project.fx_clips[i].clip.kind, dir);
    s.core.fx_set_kind(i, next);
}

/// The frame position the Split button cuts at for
/// playhead frame `current`: the frame's CENTRE,
/// `current + 0.5`. `EditorCore::fx_split_clip` only
/// cuts strictly inside the clip's span
/// (`start < at_frame < start + duration`, with at
/// least the backend's minimum clip duration left on
/// both sides), so cutting on the integer `current`
/// would refuse in exactly the case a user expects
/// to work: parked on the clip's first frame, where
/// `current == start` sits on the edge, not inside.
/// The centre is strictly inside whenever the
/// playhead rests on a frame the clip covers and the
/// clip runs at least half a frame past that frame's
/// start — the first-frame case included, where it
/// leaves a half-frame first half, comfortably above
/// the op's minimum. Split out of the handler so the
/// choice is testable without a `State`.
pub fn split_frame(current: usize) -> f32 {
    current as f32 + 0.5
}

/// Split the selected clip at the playhead via
/// `EditorCore::fx_split_clip` (Phase J), cutting at
/// the playhead frame's centre ([`split_frame`]). The
/// op inserts the second half immediately after the
/// original, so the selection index still names the
/// clip the user was editing — now its first half —
/// and, unlike [`delete_selected`], no selection
/// repair is needed. The selection therefore stays
/// on the original (first half) after a successful
/// split. On refusal (playhead outside the clip, or
/// too close to an edge) the op sets the status line
/// and changes nothing; the panel adds nothing of
/// its own, exactly like the move/resize handlers.
fn split_selected(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let at = split_frame(s.core.current);
    s.core.fx_split_clip(i, at);
}

/// The shift that lands a clip starting at `start`
/// on the playhead frame `current`: `current -
/// start`, signed (negative moves the clip earlier).
/// Pure helper for the To playhead button
/// ([`move_selected_to_playhead`]), split out so the
/// arithmetic is testable without a `State`. The
/// delta is exact as an `f32`; the move op beneath
/// it speaks whole frames (`fx_move_clip` takes an
/// `i32`), so the handler rounds to the nearest
/// whole frame — for the whole-frame starts clips
/// are created with, the landing is exact, and only
/// a fractional start (a split's second half, at
/// x.5) lands within half a frame of the playhead.
pub fn move_delta(start: f32, current: usize) -> f32 {
    current as f32 - start
}

/// The duration delta that makes a clip starting at
/// `start` with length `duration` END on the
/// playhead frame `current`: the playhead frame is
/// the last frame the clip covers, so the target
/// end is `current + 1` and the delta is
/// `(current + 1) - (start + duration)`, signed
/// (negative shortens the clip). Pure helper for
/// the End at playhead button
/// ([`end_selected_at_playhead`]). The resize
/// beneath it clamps the resulting duration to
/// >= 0.1, so a playhead parked before the clip's
/// start shrinks the clip to that floor rather
/// than collapsing it — the op's behaviour, not
/// the panel's.
pub fn end_delta(start: f32, duration: f32, current: usize) -> f32 {
    (current as f32 + 1.0) - (start + duration)
}

/// Move the selected clip so its start lands on
/// the playhead frame via `EditorCore::fx_move_clip`
/// (Phase K): the delta comes from [`move_delta`]
/// over the clip's CURRENT start, rounded to the
/// whole frames the op takes. The op edits the
/// start in place (the backend clamps it at frame
/// 0), so the clip keeps its index and the
/// selection needs no repair; the op's status line
/// reports the landing frame. With no valid
/// selection the handler returns before any op
/// call, exactly like [`split_selected`]. A clip
/// already starting on the playhead (rounded delta
/// zero) is likewise refused before the call
/// (Wave 11): the op would only snapshot and report
/// a move that did not happen — the no-op refusal
/// every neighbouring handler already makes
/// ([`nudge_clip_start`]'s frame-0 guard,
/// [`nudge_duration`]'s floor guard).
fn move_selected_to_playhead(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let start = s.core.project.fx_clips[i].clip.start;
    let delta = move_delta(start, s.core.current).round() as i32;
    if delta == 0 {
        return;
    }
    s.core.fx_move_clip(i, delta);
}

/// Resize the selected clip so it ends on the
/// playhead frame via `EditorCore::fx_resize_clip`
/// (Phase K): the delta comes from [`end_delta`]
/// over the clip's CURRENT start and duration and
/// is passed through untouched — the op applies it
/// and the backend clamps the duration to >= 0.1,
/// so a playhead before the clip's start leaves
/// the floor length, never nothing. The op edits
/// in place, so the selection needs no repair;
/// with no valid selection the handler returns
/// before any op call. A zero delta — the clip
/// already ends on the playhead frame — is refused
/// the same way (Wave 11; see
/// [`move_selected_to_playhead`]).
fn end_selected_at_playhead(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    let clip = s.core.project.fx_clips[i].clip;
    let delta = end_delta(clip.start, clip.duration, s.core.current);
    if delta == 0.0 {
        return;
    }
    s.core.fx_resize_clip(i, delta);
}

/// Toggle the selected clip's hidden flag (Phase M).
/// No EditorCore op exists for the flag, so the
/// handler follows the pattern the fx_ops no-backend
/// ops themselves use (`fx_set_color` / `fx_set_kind`
/// in fx_ops.rs): the selection is validated first,
/// then ONE `EditorCore::snapshot`, the direct field
/// write, and the status line. A hidden clip is
/// skipped by the stage and by export renderers but
/// remains in the list, editable — hiding is not
/// deleting. With no valid selection the handler
/// returns before the snapshot, exactly like
/// [`split_selected`] and the playhead handlers: no
/// snapshot, no edit, no status.
fn toggle_selected_hidden(s: &mut State) {
    let Some(i) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return;
    };
    s.core.snapshot();
    let entry = &mut s.core.project.fx_clips[i];
    entry.hidden = !entry.hidden;
    let hidden = entry.hidden;
    s.core.status = format!(
        "FX clip {} {}.",
        i + 1,
        if hidden { "hidden" } else { "shown" }
    );
}

/// The strip's press gesture: a press (drag Down) on a
/// clip's bar selects that clip in `State::fx_selected`
/// — the selection the lane rows and inspector already
/// share, so no new state exists. A press on empty
/// strip instead seeks the playhead to the frame under
/// the press (Phase I, via [`press_outcome`] and
/// `EditorCore::goto`, which sets the current frame
/// and the fractional playhead together) — the ruler
/// behaviour the strip's whole-timeline span invites.
/// Only the Down phase acts (both gestures are
/// presses, not scrubs), and the conversion runs over
/// the drag's own reported element size, matching the
/// drawn fractions at any laid-out width. Selection is
/// view state: no snapshot, no status line, exactly
/// like a lane-row press; `goto` is likewise
/// snapshot-free, so a seek leaves the undo stack and
/// the status line untouched too.
fn strip_press(s: &mut State, d: &Drag) {
    if d.phase != Phase::Down {
        return;
    }
    let windows = strip_windows(&s.core);
    let total = s.core.frame_count().max(1) as f32;
    let count = s.core.frame_count();
    match press_outcome(&windows, d.x, d.y, d.w, d.h, total, count) {
        PressOutcome::Select(i) => s.fx_selected = Some(i),
        PressOutcome::Seek(frame) => s.core.goto(frame),
    }
}

// ----------------------------------------------------------------
// Views
// ----------------------------------------------------------------

/// One colour swatch: the kit's 24×24 pressable
/// ([`design::swatch`]) filled with the colour; the
/// current value carries the kit's 2 pt accent ring,
/// the others a 1 pt card hairline so pale colours
/// stay visible on the panel card. Pressing it runs
/// `f` (a colour-set handler). Phase N retired the
/// local 18×18 white-outline construction onto the
/// kit — the outline dialect's last user (P09 forms
/// census, F-SWATCH). The key arrives by value from
/// [`swatch_rows`]' format and is passed on untouched.
fn swatch(key: String, hex: u32, current: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El {
    let fill = hex_color(hex).unwrap_or(Color::WHITE);
    design::swatch(&key, fill, current, t, f)
}

/// A labelled row of swatches (two rows of eight, so the
/// inspector stays narrow), built by `make` per colour so
/// the primary-colour and end-colour rows share the code
/// but not the element keys or handlers.
fn swatch_rows(
    clip_index: usize,
    key_stem: &str,
    current: Option<u32>,
    t: &Theme,
    mut make: impl FnMut(u32) -> Box<dyn Fn(&mut State)>,
) -> El {
    let mut rows = col().gap(4.0);
    for chunk in SWATCHES.chunks(8) {
        let mut row = row().gap(4.0);
        for &hex in chunk {
            let key = format!("{key_stem}-{}-{hex:06x}", clip_index);
            row = row.child(swatch(key, hex, current == Some(hex), t, make(hex)));
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
/// white) and outlined in the accent, and a hidden
/// clip's bar is dimmed through [`bar_alpha`]. The shape carries
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
        // A hidden clip's bar draws dimmed (Phase M):
        // its fill alpha scales by bar_alpha — ×1.0
        // for a visible clip, so visible bars render
        // exactly as before. The selected outline
        // below is deliberately NOT dimmed: a hidden
        // clip stays fully selectable and editable.
        let fill = fill.alpha(fill.a * bar_alpha(entry.hidden));
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
/// A hidden clip's row is readable at list level (UI
/// mission Wave 4, audit S13-1): its colour dot dims by
/// the strip's own [`bar_alpha`] rule, its name drops to
/// the secondary colour, and an `eye-slash` marker sits
/// before the selection dot (a spacer of the glyph's
/// own laid-out width — [`eye_slash_slot_width`],
/// Wave 9 — on live rows keeps the marker column and
/// the trailing edge aligned). The header count names
/// the hidden share while any clip is hidden
/// ([`lane_count_text`], Wave 10). The empty
/// lane is the kit [`design::empty_state`] under
/// `wand-magic-sparkles` (audit S13-3), text unchanged.
///
/// Wave 10: the row is TWO lines inside the same keyed
/// shell — name + eye marker + selection dot on line 1,
/// window + anchor on line 2. The pre-Wave-10 single
/// line could not fit its own texts: the card leaves
/// 276 pt inside the row's padding (316 column − 2×12
/// panel pad − 2×8 row px), the fixed children took
/// 71.5 pt of it (dot 10 + eye slot 13.5 + selection
/// dot 8 + five 8 pt gaps), and at the shell's
/// conservative LABEL accounting (the fx_browser
/// upper-bound model) even an ordinary row — `burst
/// #1`, `frames 5–14`, `fig 2 · node 5` — sums to
/// 288.5 pt, the longest kind (`shock_ring #1`) to
/// 325.5 pt: the trailing markers were pushed past
/// the card edge. Split across two lines, line 1's
/// name has 220.5 pt (a `shock_ring #100` measures
/// 101) and line 2's
/// pair fits whole at four-digit fractional frames;
/// both grown texts still carry `truncate()` as the
/// valve for values beyond that envelope, so no
/// marker can ever be pushed out of the card again.
pub fn fx_lane(s: &State, t: &Theme) -> El {
    let rows = clip_rows(
        &s.core,
        valid_selection(s.fx_selected, s.core.project.fx_clips.len()),
    );
    let accent = tok::accent(0, true);
    let mut list = col().key("fx-clips").gap(4.0);
    if rows.is_empty() {
        list = list.child(design::empty_state(
            "wand-magic-sparkles",
            &i18n_wire::tr(&s.core, "fx_no_clips"),
            t,
        ));
    }
    for data in rows {
        let i = data.index;
        // A hidden clip's dot dims by the strip's bar
        // rule (×1.0 for a live clip — the colour is
        // untouched).
        let dot_fill = hex_color(data.color).unwrap_or(Color::WHITE);
        let dot_fill = dot_fill.alpha(dot_fill.a * bar_alpha(data.hidden));
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
                // Wave P (P4): the kit row press overlay —
                // the same `t.press` token `design::row_base`
                // paints on `list_row` / `row_shell`. This
                // two-line row keeps its local fill/hover
                // (its anatomy predates the kit row's
                // metrics) but carried no press state at
                // all, so a press answered nothing.
                .press(t.press)
                .on_press(move |s: &mut State| {
                    s.fx_selected = Some(i);
                })
                .child(
                    el()
                        .size(10.0, 10.0)
                        .radius(5.0)
                        .fill(dot_fill),
                )
                // Two text lines beside the dot (Wave
                // 10 — see the fn docs for the width
                // proof): the name shares line 1 with
                // the eye marker and the selection dot,
                // the window and anchor share line 2.
                // Every text, key and handler is the
                // pre-restructure row's own.
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(
                            row()
                                .w_full()
                                .gap(8.0)
                                .center_y()
                                .child(
                                    text(data.name)
                                        .font(type_::LABEL)
                                        .color(if data.hidden {
                                            t.text_secondary
                                        } else {
                                            t.text
                                        })
                                        .grow(1.0)
                                        .truncate(),
                                )
                                .child(if data.hidden {
                                    icon_el("eye-slash", 12.0, t.text_secondary)
                                } else {
                                    el().size(eye_slash_slot_width(), 12.0)
                                })
                                .child(if data.selected {
                                    el().size(8.0, 8.0).radius(4.0).fill(accent)
                                } else {
                                    el().size(8.0, 8.0)
                                }),
                        )
                        .child(
                            row()
                                .w_full()
                                .gap(8.0)
                                .center_y()
                                .child(
                                    text(data.window)
                                        .font(type_::LABEL)
                                        .color(t.text_secondary)
                                        .grow(1.0)
                                        .truncate(),
                                )
                                .child(
                                    text(data.anchor)
                                        .font(type_::LABEL)
                                        .color(t.text_secondary),
                                ),
                        ),
                ),
        );
    }
    let total = s.core.project.fx_clips.len();
    let hidden = s.core.project.fx_clips.iter().filter(|e| e.hidden).count();
    let count = lane_count_text(total, hidden);
    col()
        .key("fx-lane")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("wand-magic-sparkles"),
                    &i18n_wire::tr(&s.core, "fx_title"),
                    Some(&count),
                    t,
                )),
        )
        .child(design::divider(t))
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
/// `fx_inspector`'s own empty state. The two clusters
/// sit on separate lines under the one `fx-nudge` key
/// (Phase O, p05 O-8): the single line measured 343.5 pt
/// against the 316 pt column budget, with `fx-dur-up`
/// at the window edge; the split keeps the Dur label
/// with its two buttons instead of letting a wrap
/// separate them.
pub fn nudge_row(s: &State, t: &Theme) -> El {
    let Some(sel) = valid_selection(s.fx_selected, s.core.project.fx_clips.len()) else {
        return row().key("fx-nudge").w_full().child(
            text(i18n_wire::tr(&s.core, "fx_select_clip_to_edit"))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    // Wave Q: the selected clip's bounds, for the
    // StepForm mirrors (`start_step_live` /
    // `duration_shrink_live`) — every button in this
    // row drives the same guarded nudge paths as the
    // inspector's steppers, so it takes the same
    // inert-at-the-bound forms.
    let start = s.core.project.fx_clips[sel].clip.start;
    let duration = s.core.project.fx_clips[sel].clip.duration;
    let frames = s.core.frame_count();
    col()
        .key("fx-nudge")
        .w_full()
        .gap(8.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(&s.core, "fx_nudge"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(design::step_btn(
                    "fx-nudge-l10",
                    "-10",
                    if start_step_live(start, frames, -10) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, -10),
                ))
                .child(if start_step_live(start, frames, -1) {
                    icon_only_btn("fx-nudge-l", "chevron-left", &i18n_wire::tr(&s.core, "fx_nudge_earlier"), 26.0, false, t, |s| {
                        nudge_start(s, -1)
                    })
                } else {
                    icon_only_btn_inert("fx-nudge-l", "chevron-left", &i18n_wire::tr(&s.core, "fx_nudge_earlier"), 26.0, t)
                })
                .child(if start_step_live(start, frames, 1) {
                    icon_only_btn("fx-nudge-r", "chevron-right", &i18n_wire::tr(&s.core, "fx_nudge_later"), 26.0, false, t, |s| {
                        nudge_start(s, 1)
                    })
                } else {
                    icon_only_btn_inert("fx-nudge-r", "chevron-right", &i18n_wire::tr(&s.core, "fx_nudge_later"), 26.0, t)
                })
                .child(design::step_btn(
                    "fx-nudge-r10",
                    "+10",
                    if start_step_live(start, frames, 10) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, 10),
                )),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(&s.core, "fx_duration_short"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(design::step_btn(
                    "fx-dur-dn",
                    "-0.1s",
                    if duration_shrink_live(duration) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_duration_seconds(s, -0.1),
                ))
                .child(design::step_btn(
                    "fx-dur-up",
                    "+0.1s",
                    design::StepForm::Live,
                    t,
                    |s| nudge_duration_seconds(s, 0.1),
                )),
        )
}

/// The clip inspector for the selected FX clip: kind and
/// anchor summary, a one-line readout (Phase M:
/// [`clip_readout`], key `fx-readout`), a kind stepper
/// (Phase I: prev/next
/// cycling through the browser's category order),
/// start/duration steppers (move/resize),
/// earlier/later nudge buttons (the GPUI panel's
/// `fx-earlier`/`fx-later`), a split row (Phase J:
/// cut the clip at the playhead), a playhead row
/// (Phase K), a visibility row (Phase M: the
/// Hide/Show toggle, key `fx-hide`), a nudge row (fine
/// start moves and ±0.1 s duration steps),
/// colour + end-colour
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
        // Readout (Phase M): the one-line summary of
        // the clip — display kind name, first covered
        // frame, length, and its hidden flag — built
        // by clip_readout from the entry itself.
        .child(
            text(clip_readout(&s.core.project.fx_clips[i]))
                .key("fx-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        // Kind stepper (Phase I): prev/next cycle the
        // clip's kind through the FX browser's category
        // order (fx_ops::fx_kind_cycle +
        // EditorCore::fx_set_kind); the label names the
        // current kind the way the browser names it
        // (fx_browser::kind_label).
        .child(
            row()
                .key("fx-kind-row")
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(&s.core, "fx_kind"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(icon_only_btn("fx-kind-prev", "chevron-left", &i18n_wire::tr(&s.core, "fx_previous_kind"), 26.0, false, t, |s| {
                    cycle_selected_kind(s, -1)
                }))
                .child(
                    text(kind_label(data.kind))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(icon_only_btn("fx-kind-next", "chevron-right", &i18n_wire::tr(&s.core, "fx_next_kind"), 26.0, false, t, |s| {
                    cycle_selected_kind(s, 1)
                })),
        )
        // Start stepper: -/+ move the clip by one frame
        // (EditorCore::fx_move_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(&s.core, "fx_start"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(design::step_btn(
                    &format!("fx-start-dec-{i}"),
                    "-",
                    // Wave Q: inert where
                    // `nudge_clip_start`'s guards refuse
                    // (see `start_step_live`).
                    if start_step_live(data.start, s.core.frame_count(), -1) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, -1),
                ))
                .child(
                    text(frame_label(data.start + 1.0))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(design::step_btn(
                    &format!("fx-start-inc-{i}"),
                    "+",
                    if start_step_live(data.start, s.core.frame_count(), 1) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, 1),
                )),
        )
        // Duration stepper: -/+ resize by one frame
        // (EditorCore::fx_resize_clip).
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(&s.core, "fx_duration"))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(design::step_btn(
                    &format!("fx-dur-dec-{i}"),
                    "-",
                    // Wave Q: inert at `nudge_duration`'s
                    // 0.1 floor (see
                    // `duration_shrink_live`); growing is
                    // unbounded, always Live.
                    if duration_shrink_live(data.duration) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_duration(s, -1.0),
                ))
                .child(
                    text(frame_label(data.duration))
                        .font(type_::BODY)
                        .color(t.text),
                )
                .child(design::step_btn(
                    &format!("fx-dur-inc-{i}"),
                    "+",
                    design::StepForm::Live,
                    t,
                    |s| nudge_duration(s, 1.0),
                )),
        )
        // Earlier / later: the GPUI panel's per-row nudge
        // buttons, same ops as the start stepper.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(design::step_btn(
                    &format!("fx-earlier-{i}"),
                    &i18n_wire::tr(&s.core, "fx_earlier"),
                    // Wave Q: the same start guards as
                    // the stepper above — these are the
                    // same op in a labelled form.
                    if start_step_live(data.start, s.core.frame_count(), -1) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, -1),
                ))
                .child(design::step_btn(
                    &format!("fx-later-{i}"),
                    &i18n_wire::tr(&s.core, "fx_later"),
                    if start_step_live(data.start, s.core.frame_count(), 1) {
                        design::StepForm::Live
                    } else {
                        design::StepForm::Noop
                    },
                    t,
                    |s| nudge_start(s, 1),
                ))
                .child(el().grow(1.0))
                .child(icon_only_btn(&format!("fx-dup-{i}"), "copy", &i18n_wire::tr(&s.core, "fx_duplicate_clip"), 26.0, false, t, |s| {
                    duplicate_selected(s)
                }))
                .child(icon_only_btn(&format!("fx-del-{i}"), "trash-can", &i18n_wire::tr(&s.core, "fx_delete_clip"), 26.0, false, t, |s| {
                    delete_selected(s)
                })),
        )
        // Split row (Phase J): cut the clip at the
        // playhead into two clips via
        // EditorCore::fx_split_clip (see
        // split_selected / split_frame for the
        // frame-centre cut and the selection rule).
        .child(
            row()
                .key("fx-split-row")
                .gap(8.0)
                .center_y()
                .child(icon_btn("fx-split", "scissors", &i18n_wire::tr(&s.core, "fx_split_at_playhead"), false, t, |s| {
                    split_selected(s)
                })),
        )
        // Playhead row (Phase K): land the clip's
        // start on the playhead frame, or its end
        // on the playhead frame, in one press each
        // — the two landings the Earlier/Later and
        // duration steppers take many presses to
        // reach (see move_selected_to_playhead /
        // end_selected_at_playhead).
        .child(
            row()
                .key("fx-playhead-row")
                .gap(8.0)
                .center_y()
                .child(icon_btn("fx-to-playhead", "crosshairs", &i18n_wire::tr(&s.core, "fx_to_playhead"), false, t, |s| {
                    move_selected_to_playhead(s)
                }))
                .child(icon_btn("fx-end-playhead", "crosshairs", &i18n_wire::tr(&s.core, "fx_end_at_playhead"), false, t, |s| {
                    end_selected_at_playhead(s)
                })),
        )
        // Visibility row (Phase M): hide the selected
        // clip (the stage and export renderers skip
        // it; it stays listed and editable) or show it
        // again. The button names the action it will
        // take: eye-slash "Hide" while the clip is
        // visible, eye "Show" while it is hidden —
        // one key (fx-hide) for both states.
        .child(
            row()
                .key("fx-visibility-row")
                .gap(8.0)
                .center_y()
                .child(if s.core.project.fx_clips[i].hidden {
                    icon_btn("fx-hide", "eye", &i18n_wire::tr(&s.core, "fx_show"), false, t, |s| {
                        toggle_selected_hidden(s)
                    })
                } else {
                    icon_btn("fx-hide", "eye-slash", &i18n_wire::tr(&s.core, "fx_hide"), false, t, |s| {
                        toggle_selected_hidden(s)
                    })
                }),
        )
        // Nudge row: fine start moves (±1/±10 frames)
        // and ±0.1 s duration steps.
        .child(nudge_row(s, t))
        // Colours: primary, then the gradient end
        // colour block (keyed fx-color2-row since
        // Phase L; a Solid chip clears color2 to None).
        // The block's children are spaced by the same
        // 8.0 gap the panel column used when they were
        // its direct children, so the layout is
        // unchanged — the key just gives the row a name
        // of its own.
        .child(design::section_label(&i18n_wire::tr(&s.core, "fx_colour"), t))
        .child(swatch_rows(i, "fx-color", Some(data.color), t, |hex| {
            Box::new(move |s: &mut State| set_color(s, hex))
        }))
        .child(
            col()
                .key("fx-color2-row")
                .gap(8.0)
                .child(design::section_label(&i18n_wire::tr(&s.core, "fx_end_colour"), t))
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(btn(
                            &format!("fx-color2-none-{i}"),
                            &i18n_wire::tr(&s.core, "fx_solid"),
                            data.color2.is_none(),
                            t,
                            |s| set_color2(s, None),
                        ))
                        .child(
                            text(i18n_wire::tr(
                                &s.core,
                                if data.color2.is_none() {
                                    "fx_no_gradient"
                                } else {
                                    "fx_gradient_end"
                                },
                            ))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                        ),
                )
                .child(swatch_rows(i, "fx-color2", data.color2, t, |hex| {
                    Box::new(move |s: &mut State| set_color2(s, Some(hex)))
                })),
        )
        // Anchor: re-anchor to the selected joint.
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn(
                    &format!("fx-anchor-{i}"),
                    &i18n_wire::tr(&s.core, "fx_anchor_to_joint"),
                    false,
                    t,
                    |s| reanchor_selected(s),
                ))
                .child(
                    text(i18n_wire::tr(&s.core, "fx_anchor_hint"))
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
/// Phase N: the root is the kit card
/// ([`design::panel_col`]) under the same `fx-section`
/// key — card fill, hairline border, radius 12,
/// padding 12, gap 8.
pub fn fx_section(s: &State, t: &Theme) -> El {
    design::panel_col("fx-section", t)
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
            hidden: false,
        }
    }

    fn core_with(clips: Vec<FxClipEntry>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fx_clips = clips;
        core
    }

    /// `core_with` over a project lengthened to
    /// `frames` frames. The demo project has five
    /// frames, so timing tests that move clips against
    /// a project end need the end somewhere specific;
    /// the extra frames are empty `Frame`s (nothing in
    /// this module reads their poses).
    fn core_with_frames(clips: Vec<FxClipEntry>, frames: usize) -> EditorCore {
        let mut core = core_with(clips);
        while core.project.frames.len() < frames {
            core.project.frames.push(stykanimate_rs::model::Frame {
                poses: Vec::new(),
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            });
        }
        core
    }

    /// Wave P (P4): the lane rows construct with their
    /// press overlay in place, in both selection branches
    /// (one clip selected, one not), under both themes.
    /// Element internals are opaque to a bin-crate unit
    /// test, so construction is the assertable level; the
    /// presented-tree proof is the headless `fx-selected`
    /// scene's job.
    #[test]
    fn fx_lane_rows_construct_in_both_selection_branches() {
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let mut s = crate::init();
            s.core = core_with(vec![
                entry(0.0, 10.0, ClipKind::Burst, 0xFFAA00),
                entry(12.0, 8.0, ClipKind::Burst, 0x00AAFF),
            ]);
            s.fx_selected = Some(0);
            let _ = fx_lane(&s, &t);
            s.fx_selected = None;
            let _ = fx_lane(&s, &t);
        }
    }

    #[test]
    fn start_step_live_mirrors_the_nudge_guards() {
        // Wave Q: the StepForm source for every
        // start-stepping control must agree with
        // `nudge_clip_start`'s guards exactly.
        // Frame 0: negative refused, positive live.
        assert!(!start_step_live(0.0, 30, -1));
        assert!(!start_step_live(0.0, 30, -10));
        assert!(start_step_live(0.0, 30, 1));
        // A fractional start above 0 still moves.
        assert!(start_step_live(0.5, 30, -1));
        // Last frame of 30: positive refused (the
        // clamped step would be 0), negative live.
        assert!(!start_step_live(29.0, 30, 1));
        assert!(!start_step_live(29.0, 30, 10));
        assert!(start_step_live(29.0, 30, -1));
        // One short of the last frame still moves.
        assert!(start_step_live(28.5, 30, 1));
        // Duration floor: at/under 0.1 the shrink is
        // refused; above it, live.
        assert!(!duration_shrink_live(0.1));
        assert!(!duration_shrink_live(0.05));
        assert!(duration_shrink_live(0.2));
    }

    #[test]
    fn fx_stepper_surfaces_construct_at_the_bounds() {
        // Wave Q: the inspector and the nudge row
        // build in their inert branches (clip at
        // frame 0 with duration at the floor; clip in
        // the last frame) under both themes.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let mut s = crate::init();
            s.core = core_with_frames(
                vec![entry(0.0, 0.1, ClipKind::Burst, 0xFFAA00)],
                30,
            );
            s.fx_selected = Some(0);
            let _ = fx_inspector(&s, &t);
            let _ = nudge_row(&s, &t);
            s.core = core_with_frames(
                vec![entry(29.0, 10.0, ClipKind::Burst, 0xFFAA00)],
                30,
            );
            s.fx_selected = Some(0);
            let _ = fx_inspector(&s, &t);
            let _ = nudge_row(&s, &t);
        }
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
    fn clip_rows_carry_the_hidden_flag() {
        // S13-1: the lane row data must say whether the
        // clip is hidden, per clip, so the row can dim
        // and mark it without a selection round-trip.
        let mut hidden = entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF);
        hidden.hidden = true;
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            hidden,
        ]);
        let rows = clip_rows(&core, None);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].hidden, "a live clip's row reads live");
        assert!(rows[1].hidden, "the hidden clip's row carries the flag");
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
        // A 30-frame project: in the five-frame demo
        // project the +1 step from 4.5 would land at
        // 5.5, already past the end, and the Wave 10
        // project-end clamp would (rightly) refuse it.
        let mut core = core_with_frames(vec![entry(4.5, 10.0, ClipKind::Burst, 0xFF5D5D)], 30);
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

    #[test]
    fn nudge_clip_start_clamps_at_the_project_end() {
        // A clip starting at frame 28 of a 30-frame
        // project: a +10 step would land it at 38,
        // fully past the end, where nothing renders or
        // selects it. The step clamps to the last
        // in-project landing — the last frame's start.
        let mut core = core_with_frames(vec![entry(28.0, 10.0, ClipKind::Burst, 0xFF5D5D)], 30);
        assert!(nudge_clip_start(&mut core, 0, 10));
        assert_eq!(core.project.fx_clips[0].clip.start, 29.0);
        // Already starting in the last frame, a further
        // step is refused outright — no snapshot spent
        // (undo depth unchanged), no movement.
        let depth = core.undo_depth();
        assert!(!nudge_clip_start(&mut core, 0, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 29.0);
        assert_eq!(core.undo_depth(), depth, "a refused step snapshots nothing");
        // A fractional start keeps its fraction under
        // the clamp: 28.5 steps to 29.5 — it begins
        // during the last frame — and no further.
        let mut core = core_with_frames(vec![entry(28.5, 10.0, ClipKind::Burst, 0xFF5D5D)], 30);
        assert!(nudge_clip_start(&mut core, 0, 10));
        assert_eq!(core.project.fx_clips[0].clip.start, 29.5);
        assert!(!nudge_clip_start(&mut core, 0, 1));
        assert_eq!(core.project.fx_clips[0].clip.start, 29.5);
        // A clip already past the end (an MCP move or
        // a duplicate can put one there) refuses
        // positive steps — they would only bury it
        // deeper — but still steps back into the
        // project.
        let mut core = core_with_frames(vec![entry(35.0, 10.0, ClipKind::Burst, 0xFF5D5D)], 30);
        assert!(!nudge_clip_start(&mut core, 0, 1));
        assert!(nudge_clip_start(&mut core, 0, -10));
        assert_eq!(core.project.fx_clips[0].clip.start, 25.0);
    }

    #[test]
    fn duration_nudge_refuses_to_shrink_a_clip_already_at_the_floor() {
        // At the backend's 0.1 floor, the shrink step
        // is a no-op: the handler refuses it before the
        // op, so no snapshot is spent and the duration
        // is untouched.
        let core = core_with(vec![entry(4.0, 0.1, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        let depth = state.core.undo_depth();
        nudge_duration(&mut state, -1.0);
        assert_eq!(state.core.undo_depth(), depth, "no snapshot for a no-op");
        assert_eq!(state.core.project.fx_clips[0].clip.duration, 0.1);
        // A shrink crossing the floor still goes
        // through and lands on it via the backend
        // clamp — a real change, exactly one snapshot.
        let core = core_with(vec![entry(4.0, 0.5, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        let depth = state.core.undo_depth();
        nudge_duration(&mut state, -1.0);
        assert_eq!(state.core.undo_depth(), depth + 1);
        assert_eq!(state.core.project.fx_clips[0].clip.duration, 0.1);
        // Growing from the floor is unguarded.
        nudge_duration(&mut state, 1.0);
        let duration = state.core.project.fx_clips[0].clip.duration;
        assert!((duration - 1.1).abs() < 1e-4, "duration {duration}");
    }

    #[test]
    fn lane_count_text_names_the_hidden_share() {
        // Nothing hidden: the pre-Wave-10 text,
        // byte-identical.
        assert_eq!(lane_count_text(0, 0), "0 clip(s)");
        assert_eq!(lane_count_text(3, 0), "3 clip(s)");
        // Any hidden clips: the share is named — the
        // header must not read as fully live.
        assert_eq!(lane_count_text(3, 1), "3 clip(s) · 1 hidden");
        assert_eq!(lane_count_text(5, 5), "5 clip(s) · 5 hidden");
    }

    /// Conservative advance estimate for one character
    /// at the LABEL font (12 pt, weight 500) — the
    /// fx_browser width model's class table, copied:
    /// every class is an UPPER bound against measured
    /// tree text, so a line the model passes truly
    /// fits. Non-ASCII is charged the full em.
    fn label_width_model(s: &str) -> f32 {
        s.chars()
            .map(|c| match c {
                ' ' => 4.0,
                'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' => {
                    4.0
                }
                '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' => 6.0,
                'm' | 'w' => 10.0,
                'M' | 'W' => 12.0,
                '0'..='9' => 7.0,
                'A'..='Z' => 9.0,
                'a'..='z' => 7.0,
                _ if c.is_ascii() => 7.0,
                _ => 12.0,
            })
            .sum()
    }

    #[test]
    fn lane_row_lines_fit_the_column_budget() {
        // Wave 10 width proof, pinned (see `fx_lane`'s
        // docs). The card leaves 316 − 2×12 (panel pad)
        // = 292 pt for a row; the row's own px leaves
        // 292 − 2×8 = 276; the dot and the gap beside
        // it leave a 258 pt text column.
        let text_col = 316.0 - 24.0 - 16.0 - 10.0 - 8.0;
        assert_eq!(text_col, 258.0);
        // Worst-case row strings, built by the module's
        // own formatters: the 100th clip, of the
        // longest kind, with a four-digit fractional
        // window and a three-digit figure anchor.
        let mut clips: Vec<FxClipEntry> = (0..99)
            .map(|_| entry(0.0, 1.0, ClipKind::Burst, 0xFF5D5D))
            .collect();
        let mut worst = entry(1000.5, 1000.0, ClipKind::ShockRing, 0x5DB2FF);
        worst.figure = 99;
        worst.node = 99;
        clips.push(worst);
        let core = core_with(clips);
        let rows = clip_rows(&core, None);
        let worst = &rows[99];
        assert_eq!(worst.name, "shock_ring #100");
        assert_eq!(worst.window, "frames 1001.5–2000.5");
        assert_eq!(worst.anchor, "fig 100 · node 99");
        // Line 1: name + gap + eye slot (13.5, the
        // `eye_slash_slot_width` value) + gap +
        // selection dot = the name against a 220.5 pt
        // budget.
        let line1 = label_width_model(&worst.name) + 8.0 + 13.5 + 8.0 + 8.0;
        assert!(
            line1 <= text_col,
            "line 1 sums {line1} pt against the {text_col} pt column"
        );
        // Line 2: window + gap + anchor, whole — no
        // truncation needed even at this envelope.
        let line2 =
            label_width_model(&worst.window) + 8.0 + label_width_model(&worst.anchor);
        assert!(
            line2 <= text_col,
            "line 2 sums {line2} pt against the {text_col} pt column"
        );
        // The pre-Wave-10 single line, same strings:
        // the three texts plus the fixed children
        // (dot 10 + eye 13.5 + selection dot 8 + five
        // gaps 40 = 71.5) overflowed the 276 pt row —
        // and already did at ordinary values (`burst
        // #1` / `frames 5–14` / `fig 2 · node 5`).
        let fixed = 10.0 + 13.5 + 8.0 + 40.0;
        let single = label_width_model(&worst.name)
            + label_width_model(&worst.window)
            + label_width_model(&worst.anchor)
            + fixed;
        assert!(single > 276.0, "the old line summed {single} pt");
        let ordinary = label_width_model("burst #1")
            + label_width_model("frames 5–14")
            + label_width_model("fig 2 · node 5")
            + fixed;
        assert!(
            ordinary > 276.0,
            "even an ordinary old row summed {ordinary} pt"
        );
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
    fn frame_frac_and_lane_at_convert_points_back() {
        assert_eq!(frame_frac_at_x(0.0, 300.0, 30.0), 0.0);
        assert_eq!(frame_frac_at_x(150.0, 300.0, 30.0), 15.0);
        assert_eq!(frame_frac_at_x(300.0, 300.0, 30.0), 30.0);
        // Outside the element clamps to the ends.
        assert_eq!(frame_frac_at_x(-10.0, 300.0, 30.0), 0.0);
        assert_eq!(frame_frac_at_x(450.0, 300.0, 30.0), 30.0);
        assert_eq!(frame_frac_at_x(10.0, 0.0, 30.0), 0.0);

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

    // --------------------------------------------------
    // Phase I: strip seek + kind cycling
    // --------------------------------------------------

    #[test]
    fn seek_frame_at_x_maps_the_nominal_width_proportionally() {
        // STRIP_W nominal points over 30 frames: the
        // left edge is frame 0, halfway is frame 15.
        assert_eq!(frame_at_x(0.0, 30), 0);
        assert_eq!(frame_at_x(STRIP_W / 2.0, 30), 15);
        // Just inside the right edge is the last frame,
        // and the edge itself clamps to it — never one
        // past the end.
        assert_eq!(frame_at_x(STRIP_W - 1.0, 30), 29);
        assert_eq!(frame_at_x(STRIP_W, 30), 29);
        // Beyond both edges clamps; a one-frame project
        // maps everywhere to frame 0.
        assert_eq!(frame_at_x(-20.0, 30), 0);
        assert_eq!(frame_at_x(STRIP_W * 2.0, 30), 29);
        assert_eq!(frame_at_x(STRIP_W / 2.0, 1), 0);
    }

    #[test]
    fn seek_frame_at_x_handles_degenerate_inputs() {
        // No frames: frame 0 is the only answer.
        assert_eq!(frame_at_x(0.0, 0), 0);
        assert_eq!(frame_at_x(STRIP_W / 2.0, 0), 0);
        // Non-finite points seek nowhere (frame 0), the
        // waveform mapping's convention.
        assert_eq!(frame_at_x(f32::NAN, 30), 0);
        assert_eq!(frame_at_x(f32::INFINITY, 30), 0);
    }

    #[test]
    fn press_outcome_selects_bars_and_seeks_empty_strip() {
        // One clip covering frames 0..10; strip element
        // 300 x 14 over a 30-frame project.
        let clips = [(0.0, 10.0)];
        // On the bar: select, exactly as press_select
        // resolved presses before the seek existed.
        assert_eq!(
            press_outcome(&clips, 50.0, 5.0, 300.0, 14.0, 30.0, 30),
            PressOutcome::Select(0)
        );
        // Empty strip past the clip: seek. Element x 150
        // of 300 is half the strip, so half the
        // timeline — frame 15.
        assert_eq!(
            press_outcome(&clips, 150.0, 5.0, 300.0, 14.0, 30.0, 30),
            PressOutcome::Seek(15)
        );
        // The far right of the strip seeks the last
        // frame, not one past it.
        assert_eq!(
            press_outcome(&clips, 299.0, 5.0, 300.0, 14.0, 30.0, 30),
            PressOutcome::Seek(29)
        );
        // No clips at all: every press is a seek.
        assert_eq!(
            press_outcome(&[], 150.0, 5.0, 300.0, 14.0, 30.0, 30),
            PressOutcome::Seek(15)
        );
        // A wider element: the press converts through
        // the element's own width, so half of a
        // 600-wide element is still frame 15.
        assert_eq!(
            press_outcome(&clips, 300.0, 5.0, 600.0, 14.0, 30.0, 30),
            PressOutcome::Seek(15)
        );
    }

    #[test]
    fn kind_cycle_wraps_at_both_ends_of_the_order() {
        // The order is the FX browser's categories
        // (fx_ops::FX_KIND_ORDER): Explosion opens with
        // Burst, Particles closes with Wave.
        assert_eq!(cycle_kind(ClipKind::Wave, 1), ClipKind::Burst);
        assert_eq!(cycle_kind(ClipKind::Burst, -1), ClipKind::Wave);
        // Neighbours step singly within a category...
        assert_eq!(cycle_kind(ClipKind::Burst, 1), ClipKind::ShockRing);
        assert_eq!(cycle_kind(ClipKind::ShockRing, -1), ClipKind::Burst);
        // ...and across a category boundary
        // (Explosion's last kind steps into Energy's
        // first).
        assert_eq!(cycle_kind(ClipKind::Splash, 1), ClipKind::Beam);
        assert_eq!(cycle_kind(ClipKind::Beam, -1), ClipKind::Splash);
        // A zero step is the kind itself.
        assert_eq!(cycle_kind(ClipKind::Sparkle, 0), ClipKind::Sparkle);
    }

    #[test]
    fn cycled_kind_reads_the_selected_clip() {
        let core = core_with(vec![
            entry(0.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(12.0, 6.0, ClipKind::Wave, 0x5DB2FF),
        ]);
        assert_eq!(cycled_kind(&core, Some(0), 1), Some(ClipKind::ShockRing));
        // The last kind in the order wraps to the first.
        assert_eq!(cycled_kind(&core, Some(1), 1), Some(ClipKind::Burst));
        assert_eq!(cycled_kind(&core, Some(1), -1), Some(ClipKind::Glitter));
        // No selection, or a stale one, cycles nothing.
        assert_eq!(cycled_kind(&core, None, 1), None);
        assert_eq!(cycled_kind(&core, Some(7), 1), None);
    }

    #[test]
    fn set_kind_through_the_op_changes_only_the_kind() {
        // The handler's path, exercised the way
        // cycle_selected_kind drives it: pick the
        // cycled kind, then apply it through the op.
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let next = cycled_kind(&core, Some(0), 1).expect("a cycled kind");
        core.fx_set_kind(0, next);
        let clip = &core.project.fx_clips[0].clip;
        assert_eq!(clip.kind, ClipKind::ShockRing);
        // Start, duration and colour are untouched.
        assert_eq!(clip.start, 4.0);
        assert_eq!(clip.duration, 10.0);
        assert_eq!(clip.color, 0xFF5D5D);
    }

    // --------------------------------------------------
    // Phase J: split at playhead
    // --------------------------------------------------

    /// A shell `State` over `core` with the given FX
    /// selection — the fixture the builder and handler
    /// tests below need (the same full-literal pattern
    /// the other sections' test modules use).
    fn state_with(core: EditorCore, fx_selected: Option<usize>) -> State {
        State {
            core,
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected,
            subtitle_selected: None,
            pose_clipboard: None,
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
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    #[test]
    fn split_frame_is_the_playhead_frames_centre() {
        assert_eq!(split_frame(0), 0.5);
        assert_eq!(split_frame(4), 4.5);
        assert_eq!(split_frame(29), 29.5);
        // The case the centre exists for: a clip
        // starting at frame 4, playhead parked on
        // frame 4. The cut at 4.5 is strictly inside
        // (4, 14); the integer 4.0 would sit on the
        // clip's start edge and the op would refuse.
        let at = split_frame(4);
        assert!(at > 4.0 && at < 14.0, "cut {at} must be inside the clip");
    }

    #[test]
    fn inspector_builds_with_and_without_a_selection() {
        let t = rune::tok::theme(true);
        // No clips at all: the guidance branch.
        let state = state_with(core_with(vec![]), None);
        let _ = fx_inspector(&state, &t);
        // Clips but no selection: still the guidance
        // branch — the split row lives in the detail
        // block, which only a valid selection opens
        // (the gate inspector_data_needs_a_valid_selection
        // pins), so it is present iff selected.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let state = state_with(core, None);
        let _ = fx_inspector(&state, &t);
        // A selection: the detail block, split row
        // included, builds.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let state = state_with(core, Some(0));
        let _ = fx_inspector(&state, &t);
        // A stale selection falls back to guidance
        // rather than a detail block on the wrong clip.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let state = state_with(core, Some(9));
        let _ = fx_inspector(&state, &t);
    }

    #[test]
    fn section_builds_with_and_without_clips() {
        let t = rune::tok::theme(true);
        // Phase N: the whole section — kit card root,
        // kit header over its divider, the strip, and
        // (with a valid selection) the inspector with
        // its kit swatch rows — builds in every state
        // the lane/inspector pair can be in.
        let state = state_with(core_with(vec![]), None);
        let _ = fx_section(&state, &t);
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let state = state_with(core, None);
        let _ = fx_section(&state, &t);
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let state = state_with(core, Some(0));
        let _ = fx_section(&state, &t);
    }

    #[test]
    fn split_selected_cuts_at_the_playhead_centre_and_keeps_the_selection() {
        // Clip spans frames [4, 14); the playhead is
        // parked on its FIRST frame — the case the
        // frame-centre cut exists for.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 4;
        split_selected(&mut state);
        let clips = &state.core.project.fx_clips;
        assert_eq!(clips.len(), 2, "the clip became two");
        // First half: [4, 4.5); second half inserted
        // right after it: [4.5, 14). Between them they
        // are exactly the original.
        assert_eq!(clips[0].clip.start, 4.0);
        assert_eq!(clips[0].clip.duration, 0.5);
        assert_eq!(clips[1].clip.start, 4.5);
        assert_eq!(clips[1].clip.duration, 9.5);
        // The selection stayed on the original — now
        // the first half — with no repair.
        assert_eq!(state.fx_selected, Some(0));
    }

    #[test]
    fn split_selected_without_a_selection_or_inside_playhead_changes_nothing() {
        // No selection: the handler returns before any
        // op call, even with the playhead inside a clip.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, None);
        state.core.current = 6;
        split_selected(&mut state);
        assert_eq!(state.core.project.fx_clips.len(), 1);
        // Playhead outside the selected clip: the op
        // refuses (status only) and nothing is split.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 20;
        split_selected(&mut state);
        assert_eq!(state.core.project.fx_clips.len(), 1);
        assert_eq!(state.core.project.fx_clips[0].clip.duration, 10.0);
    }

    // --------------------------------------------------
    // Phase K: playhead landing buttons
    // --------------------------------------------------

    #[test]
    fn move_delta_is_the_signed_distance_to_the_playhead() {
        assert_eq!(move_delta(4.0, 9), 5.0);
        assert_eq!(move_delta(9.0, 4), -5.0);
        // Zero delta: the clip already starts on the
        // playhead frame.
        assert_eq!(move_delta(9.0, 9), 0.0);
        // A fractional start (a split's second half)
        // keeps its fraction in the exact delta; the
        // whole-frame rounding happens in the
        // handler, where the op's contract lives.
        assert_eq!(move_delta(4.5, 9), 4.5);
    }

    #[test]
    fn end_delta_is_the_signed_distance_to_the_playheads_end() {
        // Clip [4, 14), playhead on frame 9: the end
        // must travel from 14 back to 10 (the
        // playhead frame is the last covered one) —
        // four frames shorter.
        assert_eq!(end_delta(4.0, 10.0, 9), -4.0);
        // Playhead past the end: the clip grows to
        // reach it.
        assert_eq!(end_delta(4.0, 10.0, 20), 7.0);
        // Zero delta: start 4 + duration 6 ends at
        // 10, so frame 9 is already the last
        // covered frame.
        assert_eq!(end_delta(4.0, 6.0, 9), 0.0);
    }

    #[test]
    fn move_selected_to_playhead_lands_the_start_and_keeps_the_selection() {
        // Clip spans [4, 14); the playhead is on
        // frame 9.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 9;
        move_selected_to_playhead(&mut state);
        let clip = state.core.project.fx_clips[0].clip;
        assert_eq!(clip.start, 9.0);
        assert_eq!(clip.duration, 10.0, "a move never resizes");
        assert_eq!(state.core.project.fx_clips.len(), 1, "a move never splits");
        // The op edits in place: same index, same
        // selection, no repair.
        assert_eq!(state.fx_selected, Some(0));
    }

    #[test]
    fn end_selected_at_playhead_lands_the_end_on_the_playhead_frame() {
        // Clip spans [4, 14); the playhead is on
        // frame 9, which must become the last
        // covered frame: the clip ends at 10, so
        // the duration is 6.0.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 9;
        end_selected_at_playhead(&mut state);
        let clip = state.core.project.fx_clips[0].clip;
        assert_eq!(clip.start, 4.0, "a resize never moves the start");
        assert_eq!(clip.duration, 6.0);
        assert_eq!(state.fx_selected, Some(0));
    }

    #[test]
    fn playhead_buttons_without_a_selection_change_nothing() {
        // No selection: both handlers return before
        // any op call — no snapshot (undo depth
        // unchanged), no edit.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, None);
        state.core.current = 9;
        let depth = state.core.undo_depth();
        move_selected_to_playhead(&mut state);
        end_selected_at_playhead(&mut state);
        assert_eq!(state.core.undo_depth(), depth);
        let clip = state.core.project.fx_clips[0].clip;
        assert_eq!(clip.start, 4.0);
        assert_eq!(clip.duration, 10.0);
    }

    #[test]
    fn playhead_buttons_at_zero_delta_spend_no_snapshot() {
        // Wave 11: the clip already starts on the
        // playhead frame (start 9, playhead 9), so To
        // playhead is a no-op — the handler refuses it
        // before the op, exactly like the nudge
        // handlers' no-op guards: no snapshot, no
        // status claiming a move.
        let core = core_with(vec![entry(9.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 9;
        let depth = state.core.undo_depth();
        let status_before = state.core.status.clone();
        move_selected_to_playhead(&mut state);
        assert_eq!(state.core.undo_depth(), depth, "no snapshot for a no-op");
        assert_eq!(state.core.status, status_before, "no move reported");
        assert_eq!(state.core.project.fx_clips[0].clip.start, 9.0);
        // Same for End at playhead when the playhead
        // frame is already the last covered one:
        // start 4 + duration 6 ends at 10, so frame 9
        // is the last covered frame and the delta
        // is zero.
        let core = core_with(vec![entry(4.0, 6.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(0));
        state.core.current = 9;
        let depth = state.core.undo_depth();
        let status_before = state.core.status.clone();
        end_selected_at_playhead(&mut state);
        assert_eq!(state.core.undo_depth(), depth, "no snapshot for a no-op");
        assert_eq!(state.core.status, status_before, "no resize reported");
        assert_eq!(state.core.project.fx_clips[0].clip.duration, 6.0);
    }

    // --------------------------------------------------
    // Phase L: gradient end colour row (fx-color2-row)
    // --------------------------------------------------
    // The row itself predates Phase L (Phase D built
    // the End colour label, the Solid chip and the
    // fx-color2 swatches over `set_color2`); what the
    // phase adds is the keyed fx-color2-row container
    // and these handler tests — the press path had no
    // coverage at all.

    #[test]
    fn set_color2_swatch_press_sets_the_end_on_the_selected_clip_only() {
        // Two clips; the SECOND is selected. The press
        // the fx-color2 swatch runs is
        // set_color2(s, Some(hex)) — it must land on
        // the selected clip and nowhere else.
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF),
        ]);
        let mut state = state_with(core, Some(1));
        set_color2(&mut state, Some(0xFF7BAC));
        let clips = &state.core.project.fx_clips;
        assert_eq!(clips[1].clip.color2, Some(0xFF7BAC));
        assert_eq!(clips[0].clip.color2, None, "unselected clip untouched");
        // The gradient end is its own field: neither
        // clip's start colour moved.
        assert_eq!(clips[1].clip.color, 0x5DB2FF);
        assert_eq!(clips[0].clip.color, 0xFF5D5D);
        // The op's own status names the clip and the
        // colour it was given.
        assert_eq!(
            state.core.status,
            "FX clip 2 end colour set to #FF7BAC."
        );
    }

    #[test]
    fn set_color2_solid_chip_clears_the_end_back_to_none() {
        // A clip that already has a gradient end; the
        // Solid chip's press is set_color2(s, None).
        let mut gradient = entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D);
        gradient.clip.color2 = Some(0x7BE495);
        let core = core_with(vec![gradient]);
        let mut state = state_with(core, Some(0));
        set_color2(&mut state, None);
        let clip = state.core.project.fx_clips[0].clip;
        assert_eq!(clip.color2, None, "cleared back to solid");
        assert_eq!(clip.color, 0xFF5D5D, "start colour untouched");
        assert_eq!(state.core.status, "FX clip 1 end colour cleared.");
    }

    #[test]
    fn set_color2_without_a_valid_selection_changes_nothing() {
        // No selection and a stale selection both make
        // the handler return before the op: no
        // snapshot (undo depth unchanged), no edit,
        // no status written by the handler itself.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, None);
        let depth = state.core.undo_depth();
        set_color2(&mut state, Some(0xFF7BAC));
        set_color2(&mut state, None);
        assert_eq!(state.core.undo_depth(), depth);
        assert_eq!(state.core.project.fx_clips[0].clip.color2, None);

        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(9));
        let depth = state.core.undo_depth();
        set_color2(&mut state, Some(0xFF7BAC));
        assert_eq!(state.core.undo_depth(), depth);
        assert_eq!(state.core.project.fx_clips[0].clip.color2, None);
    }

    #[test]
    fn inspector_builds_with_a_gradient_end_selected() {
        // The fx-color2-row block in its gradient
        // state: the selected clip carries a color2,
        // so the Solid chip renders inactive, the
        // caption reads "gradient end" and the current
        // end swatch takes the ring.
        let t = rune::tok::theme(true);
        let mut gradient = entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D);
        gradient.clip.color2 = Some(0x7BE495);
        let state = state_with(core_with(vec![gradient]), Some(0));
        let _ = fx_inspector(&state, &t);
    }

    // --------------------------------------------------
    // Phase M: hide/show toggle, readout, strip dimming
    // --------------------------------------------------

    #[test]
    fn clip_readout_names_kind_first_frame_duration_and_hidden_flag() {
        // Visible, whole frames: the kind's display
        // name (browser label over the snake name), the
        // 1-based first covered frame as an integer,
        // and the length in frames.
        let visible = entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D);
        assert_eq!(
            clip_readout(&visible),
            "Burst · starts frame 5 · 10 frames"
        );
        // Hidden appends the flag — and only that.
        let mut hidden = entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D);
        hidden.hidden = true;
        assert_eq!(
            clip_readout(&hidden),
            "Burst · starts frame 5 · 10 frames · hidden"
        );
        // A fractional start (a split's second half)
        // keeps its fraction through frame_label, and a
        // multi-word kind humanises like the browser's
        // buttons do.
        let fractional = entry(4.5, 9.5, ClipKind::ShockRing, 0x5DB2FF);
        assert_eq!(
            clip_readout(&fractional),
            "Shock ring · starts frame 5.5 · 9.5 frames"
        );
    }

    #[test]
    fn toggle_selected_hidden_flips_the_flag_with_exactly_one_snapshot() {
        // Two clips; the SECOND is selected — the
        // toggle must land on it and nowhere else.
        let core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D),
            entry(20.0, 6.0, ClipKind::ShockRing, 0x5DB2FF),
        ]);
        let mut state = state_with(core, Some(1));
        let depth = state.core.undo_depth();
        toggle_selected_hidden(&mut state);
        assert!(state.core.project.fx_clips[1].hidden, "selected clip hid");
        assert!(
            !state.core.project.fx_clips[0].hidden,
            "unselected clip untouched"
        );
        assert_eq!(
            state.core.undo_depth(),
            depth + 1,
            "one toggle is one undo step"
        );
        assert_eq!(state.core.status, "FX clip 2 hidden.");
        // Toggling back shows it again — another single
        // snapshot, the mirrored status.
        toggle_selected_hidden(&mut state);
        assert!(!state.core.project.fx_clips[1].hidden, "shown again");
        assert_eq!(state.core.undo_depth(), depth + 2);
        assert_eq!(state.core.status, "FX clip 2 shown.");
        // Hiding never edits the clip itself.
        let clip = state.core.project.fx_clips[1].clip;
        assert_eq!(clip.start, 20.0);
        assert_eq!(clip.duration, 6.0);
        assert_eq!(state.fx_selected, Some(1), "selection unmoved");
    }

    #[test]
    fn toggle_selected_hidden_without_a_selection_changes_nothing() {
        // No selection and a stale selection both make
        // the handler return before the snapshot: no
        // undo step, no flag change, no status.
        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, None);
        let depth = state.core.undo_depth();
        toggle_selected_hidden(&mut state);
        assert_eq!(state.core.undo_depth(), depth);
        assert!(!state.core.project.fx_clips[0].hidden);

        let core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let mut state = state_with(core, Some(9));
        let depth = state.core.undo_depth();
        toggle_selected_hidden(&mut state);
        assert_eq!(state.core.undo_depth(), depth);
        assert!(!state.core.project.fx_clips[0].hidden);
    }

    #[test]
    fn bar_alpha_dims_only_hidden_clips() {
        // Visible bars keep their full opacity (the
        // ×1.0 leaves strip rendering byte-identical);
        // hidden bars draw at HIDDEN_BAR_ALPHA.
        assert_eq!(bar_alpha(false), 1.0);
        assert_eq!(bar_alpha(true), HIDDEN_BAR_ALPHA);
        assert!((bar_alpha(true) - 0.35).abs() < 1e-6);
    }

    #[test]
    fn eye_slash_slot_width_is_the_glyphs_laid_out_width() {
        // Wave 9: the live rows' spacer must match the
        // marker it stands in for, or a hidden row's
        // anchor column shifts. The eye-slash view box
        // is 576×512, so at 12 pt tall the glyph lays
        // out 13.5 pt wide — the helper the builder
        // sizes the spacer with returns exactly the
        // icon system's own aspect maths.
        assert_eq!(eye_slash_slot_width(), 13.5);
        assert_eq!(
            eye_slash_slot_width(),
            icon_width("eye-slash", 12.0).expect("eye-slash is in the curated set")
        );
    }

    #[test]
    fn inspector_builds_with_a_hidden_clip_selected() {
        // The visibility row in its Show state and the
        // readout in its hidden form: a hidden clip is
        // still selected and editable, so the detail
        // block builds exactly as for a visible one.
        let t = rune::tok::theme(true);
        let mut hidden = entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D);
        hidden.hidden = true;
        let state = state_with(core_with(vec![hidden]), Some(0));
        let _ = fx_inspector(&state, &t);
    }

    #[test]
    fn fx_i18n_keys_exist_and_match_the_english_labels() {
        // Wave T (T5): every label the panel wires
        // through `i18n_wire::tr` must resolve to exactly
        // the English string the panel rendered before
        // the wiring. The keys land via IA-T5-1 (the
        // integrator appends them to all 28 tables after
        // the threed block), so this pin passes only
        // once that IA lands — until then `tr` falls back
        // to the key itself and `key_in_table` is false
        // (the Q7 pattern: wiring and keys land together
        // at integration).
        let core = EditorCore::new();
        for (key, english) in [
            ("fx_title", "FX clips"),
            ("fx_no_clips", "No FX clips in this project."),
            ("fx_select_clip_to_edit", "Select an FX clip to edit it."),
            ("fx_nudge", "Nudge"),
            ("fx_nudge_earlier", "Nudge earlier"),
            ("fx_nudge_later", "Nudge later"),
            ("fx_duration_short", "Dur"),
            ("fx_kind", "Kind"),
            ("fx_previous_kind", "Previous kind"),
            ("fx_next_kind", "Next kind"),
            ("fx_start", "Start"),
            ("fx_duration", "Duration"),
            ("fx_earlier", "Earlier"),
            ("fx_later", "Later"),
            ("fx_duplicate_clip", "Duplicate clip"),
            ("fx_delete_clip", "Delete clip"),
            ("fx_split_at_playhead", "Split at playhead"),
            ("fx_to_playhead", "To playhead"),
            ("fx_end_at_playhead", "End at playhead"),
            ("fx_show", "Show"),
            ("fx_hide", "Hide"),
            ("fx_colour", "Colour"),
            ("fx_end_colour", "End colour"),
            ("fx_solid", "Solid"),
            ("fx_no_gradient", "no gradient"),
            ("fx_gradient_end", "gradient end"),
            ("fx_anchor_to_joint", "Anchor to joint"),
            ("fx_anchor_hint", "uses the selected figure joint"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }
}
