//! Text tool section (Rune Phase D, section 33) — author
//! text segments on the selected figure's segments.
//!
//! Declared from `main.rs` only (bin-crate code, not part of
//! the engine library). Integration expectation: `State`
//! gains a `pub text_tool: TextToolState` field; the section
//! builder is [`text_tool`].
//!
//! ## The real engine path (verified by reading)
//!
//! There is **no** `EditorCore` op that adds a text segment
//! or sets a text size — `grep "text"` over `editor.rs`
//! finds only file I/O. The engine's one text-authoring API
//! is the UI-agnostic session in `stykanimate_rs::text_edit`
//! (the same one the GPUI app's "Text seg" button drove):
//! [`SegTextEdit::begin`] loads a node's existing text,
//! `push_char`/`backspace` edit it, and
//! [`SegTextEdit::commit`] writes `SegmentMedia::Text`
//! (`model.rs:103`) back onto the node. Rendering is the
//! 3×5 bitmap-font system in `media.rs` (`size` = glyph
//! height in stage units, `media.rs:51`); `.piv v5` text
//! side tables ride on the same `SegmentMedia::Text` data.
//!
//! Two honest consequences:
//!
//! - `commit` has **no size parameter**: it preserves an
//!   existing text size or falls back to
//!   `DEFAULT_TEXT_SIZE` (28, `text_edit.rs:21`). So after
//!   a successful commit this section writes the chosen
//!   size onto the fresh media directly — the same
//!   snapshot-then-assign-media idiom the engine itself
//!   uses in `EditorCore::clear_node_media`
//!   (`editor.rs:1861`). One `snapshot()` (`editor.rs:202`)
//!   covers the whole add, so it is a single undo step.
//! - `commit` with an **empty buffer deletes** the node's
//!   media. "Add text" therefore treats empty field content
//!   as a status-only no-op and never commits an empty
//!   buffer.
//!
//! The section also lists the project's **free-standing
//! text instances** (`Project.texts`, `model.rs:636`) —
//! the stage labels `stage_text` draws. No `EditorCore`
//! op reads or writes that vec (`grep "texts"` over
//! `editor.rs` finds no instance authoring), so the list's
//! Delete removes the instance by writing the project
//! field directly under exactly one `snapshot()` — the
//! same mutation idiom as the segment paths above — and a
//! row click loads the instance's content back into the
//! field above for re-stamping (a pure `TextToolState`
//! copy; the project is untouched).
//!
//! ## Phase G (§38): instance editing round 2
//!
//! `TextInstance` (model.rs:636) has exactly four fields
//! — `content`, `size`, `color`, `keys` — and **no
//! position field**: an instance's position lives in its
//! transform keys, `InstanceKey { frame, x, y, scale,
//! angle }` (model.rs:617), sampled by
//! `InstanceKey::sample` and drawn at `(key.x, key.y)`
//! in stage units (`media.rs:180`; the shell's
//! `stage_text` does the same). Moving an instance
//! therefore means shifting **every** key by the same
//! delta, which preserves the track's shape, timing,
//! scales and angles.
//!
//! On top of §27's list this section adds:
//!
//! - **Duplicate** per row (`text-dup-{i}`): a verbatim
//!   clone shifted by [`DUP_OFFSET`] on both axes,
//!   inserted directly after the original.
//! - A **nudge pad** (`text-nudge-l`/`r`/`u`/`d`) for
//!   the *loaded* instance — the one the last row click
//!   loaded, remembered in [`TextToolState::loaded`] —
//!   shifting every key by [`NUDGE_STEP`] stage units
//!   per press. Rune buttons carry no shift modifier,
//!   so one step size is all a press can mean.
//! - **Size steppers** for the loaded instance
//!   (`text-isize-dn`/`text-isize-up` — not the brief's
//!   `text-size-dn`/`up`: `text-size-up` already keys
//!   the segment stepper below, and duplicate keys
//!   shadow in tree order): the same [`SIZE_STEP`]
//!   step and `SIZE_MIN..=SIZE_MAX` clamp the segment
//!   stepper and `load_instance` use.
//!
//! Discipline is §27's throughout: bounds check first,
//! exactly one `snapshot()` per applied press (this
//! file has no drag to coalesce; its other stepper,
//! `step_from_button`, likewise snapshots once per
//! press), and a stale index is a status-only refusal
//! that snapshots nothing.
//!
//! ## Phase I (§36): row content preview
//!
//! The instance rows have carried a content preview
//! since §27 — the load button's label, capped by
//! [`preview`] at 18 characters. §36 caps that label
//! with the stricter [`text_preview`] instead: 14
//! characters, and line breaks flattened to spaces so
//! multi-line content stays on the row's one line.
//! [`preview`] remains for its existing tests; the
//! rows read [`text_preview`].
//!
//! ## Phase J (§24): write-back editing
//!
//! Until this phase the load path was one-way: a row
//! click copied an instance's content into the field
//! (§27, above) and editing it there could only
//! re-stamp a *segment* via Add text — the instance
//! itself never changed. [`update_text_instance`]
//! closes the loop: the **Update text** button
//! (`text-update`), shown in the loaded instance's
//! edit area only while a live instance is loaded,
//! writes the field's current content back into that
//! instance's `content` field — and nothing else:
//! its transform keys, size and colour are untouched,
//! so position and styling survive an edit. The
//! snapshot discipline is §27's: exactly one
//! `snapshot()` when the content actually changes,
//! and none (with a "Text unchanged." status) when
//! the field still holds the instance's own content.
//! Row previews need no fix-up after an update —
//! [`text_instances`] rebuilds them from the live
//! content on every render.
//!
//! ## Phase K (§30): rotating the loaded instance
//!
//! The loaded instance gains a rotate row
//! (`text-rotate-row`): **Rotate −15°** / **Rotate
//! +15°** (`text-rot-l` / `text-rot-r`), one press
//! turning every transform key's `angle` by
//! [`ROTATE_STEP_RAD`] (π/12). Rotation is a key
//! transform like §38's nudge — an instance's angle
//! lives in its keys, not in a field of its own — so
//! [`rotate_text_instance`] adds the same delta to
//! **every** key: keys that shared an angle still
//! share it afterwards, and the track's timing,
//! positions and scales are untouched. The snapshot
//! discipline is §27's throughout: bounds check first,
//! exactly one `snapshot()` per press, a stale index
//! a status-only refusal.
//!
//! One deliberate divergence from §38's nudge: an
//! instance with **no keys still rotates**. The
//! nudge refuses a keyless instance ("Text has no
//! position to move.") because a move with no keys
//! has nothing to act on; rotation instead mirrors
//! §38's **duplicate**, which clones a keyless
//! instance under one snapshot without refusing —
//! the per-key loop is simply a no-op over an empty
//! track. So a keyless rotate snapshots once, reports
//! "Text {n} rotated." and returns true, keeping the
//! press a real (if empty) undo step rather than a
//! silent dead button.
//!
//! ## Phase L (§23): colouring the loaded instance
//!
//! The loaded instance gains a colour row
//! (`text-color-row`): eight swatch chips
//! (`text-color-{i}`) over [`TEXT_PALETTE`] — the
//! particles panel's palette, reused value for value
//! so the shell's swatch language stays one language —
//! plus a current-colour readout chip
//! (`text-color-custom`) showing the loaded instance's
//! own colour, including colours no swatch carries.
//! A swatch press recolours through
//! [`set_text_color`]: only the instance's `color`
//! field changes, under exactly one `snapshot()`,
//! with the same stale-index refusal as
//! [`update_text_instance`]. The swatch whose colour
//! the instance already wears sits in an accent-filled
//! slot (the particles panel's construction); colours
//! are compared masked to `0xRRGGBB`, the engine's
//! colour form.
//!
//! ## Phase M (§29): z-order for the loaded instance
//!
//! Instances draw in vec order — later entries paint
//! over earlier ones — and until this phase a text
//! sent behind another could only be deleted and
//! recreated. The loaded instance's edit area gains
//! a z-order row (`text-zorder-row`), rendered only
//! while a live instance is loaded (the Update row's
//! conditional discipline): **Bring to front**
//! (`text-front`) moves the instance to the END of
//! `Project.texts` (drawn last = on top) and **Send
//! to back** (`text-back`) moves it to index 0. The
//! move is a remove + insert, so every other
//! instance keeps its relative order.
//! [`move_text_instance`] follows §27's discipline:
//! bounds check first, exactly one `snapshot()` per
//! applied move, and a status-only refusal that
//! snapshots nothing — a stale index ("No such
//! text.") or an instance already at the target end
//! ("Already at the front." / "Already at the
//! back."). The success status numbers the instance
//! by its position AFTER the move — the position
//! the list now shows it at. The target-index math
//! is the pure [`moved_index`], shared by the op
//! (its move decision) and by the handler (which
//! also uses it to follow the moved instance:
//! `TextToolState::loaded` is rewritten to the new
//! index, so the edit area keeps editing the same
//! text across the reorder).

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, text_input, El, TextEdit as FieldEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{InstanceKey, SegmentMedia, TextInstance};
use stykanimate_rs::text_edit::{TextEdit as SegTextEdit, DEFAULT_TEXT_SIZE};

use crate::chrome_view::btn;
use crate::State;

/// Smallest glyph height the stepper allows (stage units).
pub const SIZE_MIN: f32 = 4.0;
/// Largest glyph height the stepper allows (stage units).
pub const SIZE_MAX: f32 = 160.0;
/// Glyph-height change per stepper press (stage units).
pub const SIZE_STEP: f32 = 4.0;
/// Stage-unit offset a duplicated text instance is
/// shifted by on both axes: large enough that the copy
/// is plainly a second instance next to the original,
/// small enough to stay beside it.
pub const DUP_OFFSET: f32 = 12.0;
/// Stage units one nudge-pad press moves the loaded
/// instance (on one axis).
pub const NUDGE_STEP: f32 = 4.0;
/// Radians one rotate-button press turns the loaded
/// instance: π/12 — 15°, coarse enough to reach a
/// slant in a few presses, fine enough to correct
/// one. Rune buttons carry no shift modifier, so one
/// step size is all a press can mean (the nudge pad's
/// same limit).
pub const ROTATE_STEP_RAD: f32 = std::f32::consts::PI / 12.0;

/// The colour row's swatches (`0xRRGGBB`): the
/// particles panel's palette
/// (`particles_panel::PARTICLE_PALETTE`), reused value
/// for value so a colour means the same swatch in both
/// panels — six hues around the wheel, then white and
/// black.
pub const TEXT_PALETTE: [u32; 8] = [
    0xFF5D5D, // red
    0xFF9F43, // orange
    0xFFC65D, // amber
    0x7BE495, // green
    0x57C7E3, // sky
    0xA184FF, // violet
    0xFFFFFF, // white
    0x000000, // black
];

/// View state for the text tool: the Rune field's edit
/// model plus the pending glyph height used by the next
/// add (and shown by the stepper).
pub struct TextToolState {
    /// The text field's editing model (Rune core layer,
    /// audit Q5). Read `.text` for the current content.
    pub field: FieldEdit,
    /// Pending glyph height in stage units; starts at the
    /// engine's own default size.
    pub size: f32,
    /// Index into `Project.texts` of the instance the
    /// last list-row click loaded, if any. The nudge pad
    /// and the instance size steppers act on it; row
    /// inserts and removals adjust it so it keeps naming
    /// the same instance (`loaded_after_removal`,
    /// `loaded_after_insertion`).
    pub loaded: Option<usize>,
}

impl Default for TextToolState {
    fn default() -> Self {
        TextToolState {
            field: FieldEdit::default(),
            size: DEFAULT_TEXT_SIZE,
            loaded: None,
        }
    }
}

impl TextToolState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// One existing text segment on a figure, for the list:
/// which node carries it, that node's name, the text and
/// its glyph height.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSegmentRow {
    pub node: usize,
    pub name: String,
    pub content: String,
    pub size: f32,
}

/// All text segments on the currently selected figure, in
/// node order. The core exposes them directly: figure
/// definitions and node media are public data
/// (`EditorCore::selected_figure`, `model::SegmentMedia`).
pub fn text_segments(core: &EditorCore) -> Vec<TextSegmentRow> {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return Vec::new();
    };
    fig.def
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(node, def)| match &def.media {
            Some(SegmentMedia::Text { content, size }) => Some(TextSegmentRow {
                node,
                name: def.name.clone(),
                content: content.clone(),
                size: *size,
            }),
            _ => None,
        })
        .collect()
}

/// How many characters of a text instance's content the
/// "Existing texts" list shows before truncating.
pub const PREVIEW_CHARS: usize = 18;

/// The list preview of a text instance's content: the
/// content itself when it fits in [`PREVIEW_CHARS`]
/// characters, otherwise its first `PREVIEW_CHARS`
/// characters plus an ellipsis. Counts characters, never
/// bytes, so multi-byte text truncates cleanly. Pure.
pub fn preview(content: &str) -> String {
    if content.chars().count() <= PREVIEW_CHARS {
        return content.to_string();
    }
    let head: String = content.chars().take(PREVIEW_CHARS).collect();
    format!("{head}…")
}

/// How many characters of content [`text_preview`]
/// shows before truncating.
pub const TEXT_PREVIEW_CHARS: usize = 14;

/// The single-line preview of a text instance's content
/// that the "Existing texts" rows show (Phase I §36):
/// line breaks become spaces, then the flattened text
/// is kept whole when it fits in [`TEXT_PREVIEW_CHARS`]
/// characters and otherwise cut to its first
/// `TEXT_PREVIEW_CHARS` characters plus an ellipsis.
/// Counts characters, never bytes, so multi-byte text
/// truncates cleanly. Pure.
pub fn text_preview(content: &str) -> String {
    let flat: String = content
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    if flat.chars().count() <= TEXT_PREVIEW_CHARS {
        return flat;
    }
    let head: String = flat.chars().take(TEXT_PREVIEW_CHARS).collect();
    format!("{head}…")
}

/// One free-standing text instance (`Project.texts`), for
/// the "Existing texts" list: its real index in the vec
/// (the row's buttons act on the instance it names, even
/// as deletions shift the indices of the rest), a
/// single-line content preview ([`text_preview`]), how
/// many transform keys it carries, and its glyph height.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInstanceRow {
    pub index: usize,
    pub preview: String,
    pub keys: usize,
    pub size: f32,
}

/// All free-standing text instances in the project, in
/// vec order.
pub fn text_instances(core: &EditorCore) -> Vec<TextInstanceRow> {
    core.project
        .texts
        .iter()
        .enumerate()
        .map(|(index, inst)| TextInstanceRow {
            index,
            preview: text_preview(&inst.content),
            keys: inst.keys.len(),
            size: inst.size,
        })
        .collect()
}

/// Delete a free-standing text instance: remove index
/// `index` from `Project.texts`. No editor op exists for
/// text instances (see the module docs), so the panel
/// writes the project field directly under exactly one
/// `snapshot()` — this section's established mutation
/// idiom (see [`add_text_to_core`]) — making a delete a
/// single undo step. An out-of-range index is a
/// status-only no-op: nothing is snapshotted or removed.
pub fn delete_text_instance(core: &mut EditorCore, index: usize) {
    if index >= core.project.texts.len() {
        core.status = "No such text.".into();
        return;
    }
    core.snapshot();
    core.project.texts.remove(index);
    core.status = "Text deleted.".into();
}

/// Shift transform `keys` by `(dx, dy)` stage units.
/// Frame, scale and angle of every key are untouched, so
/// a track's shape and timing survive the move. Pure.
pub fn offset_keys(keys: &mut [InstanceKey], dx: f32, dy: f32) {
    for key in keys {
        key.x += dx;
        key.y += dy;
    }
}

/// Shift every transform key of `inst` by `(dx, dy)`
/// stage units. An instance's position lives entirely in
/// its keys (see the module docs), so this is what
/// "move the instance" means. Pure.
pub fn offset_instance_position(inst: &mut TextInstance, dx: f32, dy: f32) {
    offset_keys(&mut inst.keys, dx, dy);
}

/// Add `delta_rad` to the angle of every transform
/// key. Frame, position and scale of every key are
/// untouched, and because the delta is uniform, keys
/// that shared an angle still share it — the track
/// turns as one rigid rotation. Pure.
pub fn rotate_keys(keys: &mut [InstanceKey], delta_rad: f32) {
    for key in keys {
        key.angle += delta_rad;
    }
}

/// The clone a Duplicate press inserts: `inst` copied
/// verbatim (content, size, colour, key frames, scales
/// and angles), then shifted by [`DUP_OFFSET`] on both
/// axes so the copy lands visible-but-close — offset
/// enough to see it is a second instance, small enough
/// to stay next to the original it was cloned from.
/// Pure.
pub fn duplicated_instance(inst: &TextInstance) -> TextInstance {
    let mut copy = inst.clone();
    offset_instance_position(&mut copy, DUP_OFFSET, DUP_OFFSET);
    copy
}

/// Duplicate the text instance at `index`: insert
/// [`duplicated_instance`]'s clone directly after the
/// original, under exactly one `snapshot()` (§27's
/// mutation idiom, like [`delete_text_instance`]).
/// Returns whether a copy was inserted; an out-of-range
/// index is a status-only refusal that snapshots
/// nothing.
pub fn duplicate_text_instance(core: &mut EditorCore, index: usize) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    let copy = duplicated_instance(inst);
    core.snapshot();
    core.project.texts.insert(index + 1, copy);
    core.status = "Text duplicated.".into();
    true
}

/// Nudge the text instance at `index` by `(dx, dy)`
/// stage units: every transform key shifts by the same
/// delta ([`offset_instance_position`]). One press is
/// one `snapshot()`, so repeated presses accumulate in
/// position and in undo steps alike — the same
/// once-per-press discipline as this file's steppers.
/// Returns whether the instance moved. Status-only
/// refusals (nothing snapshotted): a stale index, or an
/// instance with no keys — it has no position to move.
pub fn nudge_text_instance(core: &mut EditorCore, index: usize, dx: f32, dy: f32) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    if inst.keys.is_empty() {
        core.status = "Text has no position to move.".into();
        return false;
    }
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        offset_instance_position(inst, dx, dy);
    }
    core.status = "Text nudged.".into();
    true
}

/// Resize the text instance at `index` by one stepper
/// step in `dir` (−1/+1): the shared [`step_size`]
/// math, so the step is [`SIZE_STEP`] and the clamp is
/// the `SIZE_MIN..=SIZE_MAX` range `load_instance`
/// clamps loaded sizes to. One `snapshot()` when the
/// size actually changes; at a clamp bound the press
/// reports the size but snapshots nothing (mirroring
/// `step_from_button`, which also declines to snapshot
/// an unchanged size), and a stale index refuses the
/// same way. Returns whether the size changed.
pub fn resize_text_instance(core: &mut EditorCore, index: usize, dir: i32) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    let next = step_size(inst.size, dir);
    if next == inst.size {
        core.status = format!("Text size {next:.0}.");
        return false;
    }
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        inst.size = next;
    }
    core.status = format!("Text size {next:.0}.");
    true
}

/// Write `content` back into the text instance at
/// `index` (Phase J §24): the write-back half of the
/// row-click load ([`load_instance`]). No editor op
/// exists for text instances, so — like this file's
/// other instance mutations — the panel writes the
/// project field directly. Only the `content` field
/// changes: the instance's transform keys (its
/// position track), size and colour are untouched.
/// Exactly one `snapshot()` when the content actually
/// changes, so an edit is a single undo step; when
/// `content` already equals the instance's content the
/// press is a status-only no-op ("Text unchanged.")
/// that snapshots nothing. A stale index refuses the
/// same way ("No such text."). Returns whether the
/// content changed.
pub fn update_text_instance(core: &mut EditorCore, index: usize, content: &str) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    if inst.content == content {
        core.status = "Text unchanged.".into();
        return false;
    }
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        inst.content = content.to_string();
    }
    core.status = format!("Text {} updated.", index + 1);
    true
}

/// Rotate the text instance at `index` by `delta_rad`
/// radians (Phase K §30): every transform key's angle
/// gains the same delta ([`rotate_keys`]), so the
/// instance turns uniformly and its timing, positions
/// and scales survive. No editor op exists for text
/// instances, so — like this file's other instance
/// mutations — the panel writes the project field
/// directly under exactly one `snapshot()`, making a
/// press a single undo step. A stale index refuses
/// exactly as [`update_text_instance`] does
/// ("No such text.", nothing snapshotted). An
/// instance with no keys still counts as rotated —
/// one snapshot, the success status, `true` —
/// mirroring [`duplicate_text_instance`], which
/// clones a keyless instance the same way rather
/// than refusing (see the module docs for why the
/// nudge's keyless refusal is not followed here).
/// Returns whether the instance was rotated.
pub fn rotate_text_instance(core: &mut EditorCore, index: usize, delta_rad: f32) -> bool {
    let Some(_inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        rotate_keys(&mut inst.keys, delta_rad);
    }
    core.status = format!("Text {} rotated.", index + 1);
    true
}

/// Set the text instance at `index`'s colour to
/// `color` (Phase L §23): the colouring half of the
/// loaded instance's edit area. No editor op exists
/// for text instances, so — like this file's other
/// instance mutations — the panel writes the project
/// field directly. Only the `color` field changes:
/// content, size and the transform keys are untouched.
/// Exactly one `snapshot()`, so a swatch press is a
/// single undo step; a stale index refuses exactly as
/// [`update_text_instance`] does ("No such text.",
/// nothing snapshotted). Returns whether the colour
/// was set.
pub fn set_text_color(core: &mut EditorCore, index: usize, color: u32) -> bool {
    let Some(_inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        inst.color = color;
    }
    core.status = format!("Text {} colour set to #{color:06X}.", index + 1);
    true
}

/// Where the instance at `index` in a vec of `len`
/// instances lands when sent to an end of the draw
/// order (Phase M §29): texts paint in vec order, so
/// the front of the stack is the vec's END — drawn
/// last, on top of everything else — and `to_front`
/// targets `len - 1`; the back of the stack is index
/// 0. `None` when there is nowhere to move to:
/// `index` out of range, or the instance already
/// sits at the target end (a one-instance vec is
/// both ends at once, so it never moves). Pure.
pub fn moved_index(len: usize, index: usize, to_front: bool) -> Option<usize> {
    if index >= len {
        return None;
    }
    let target = if to_front { len - 1 } else { 0 };
    (index != target).then_some(target)
}

/// Move the text instance at `index` to an end of
/// the draw order (Phase M §29): `to_front` sends it
/// to the end of `Project.texts` (drawn last = on
/// top), otherwise to index 0. The move is a remove
/// + insert at the [`moved_index`] target, so every
/// other instance keeps its relative order; the
/// instance itself is carried verbatim — content,
/// size, colour and transform keys all ride along.
/// No editor op exists for text instances, so — like
/// this file's other instance mutations — the panel
/// writes the project field directly under exactly
/// one `snapshot()`, making a press a single undo
/// step. Status-only refusals (nothing snapshotted):
/// a stale index ("No such text."), or the instance
/// already at the target end ("Already at the
/// front." / "Already at the back."). The success
/// status numbers the instance by its position
/// AFTER the move (`target + 1`) — the position the
/// list now shows it at. Returns whether the
/// instance moved.
pub fn move_text_instance(core: &mut EditorCore, index: usize, to_front: bool) -> bool {
    let len = core.project.texts.len();
    let Some(target) = moved_index(len, index, to_front) else {
        if index >= len {
            core.status = "No such text.".into();
        } else if to_front {
            core.status = "Already at the front.".into();
        } else {
            core.status = "Already at the back.".into();
        }
        return false;
    };
    core.snapshot();
    let inst = core.project.texts.remove(index);
    core.project.texts.insert(target, inst);
    core.status = format!(
        "Text {} moved to the {}.",
        target + 1,
        if to_front { "front" } else { "back" }
    );
    true
}

/// The full (untruncated) content of the text instance at
/// `index` — what a list-row click loads into the tool's
/// field.
pub fn instance_content(core: &EditorCore, index: usize) -> Option<String> {
    core.project
        .texts
        .get(index)
        .map(|inst| inst.content.clone())
}

/// Row-click handler: load the instance's content (and
/// its glyph height) into the tool, ready to be
/// re-stamped with Add text. A pure copy into
/// `TextToolState` — the project is not touched, nothing
/// is snapshotted, and the size is clamped to the
/// stepper's range like every other size the tool holds.
/// It also records the index in `TextToolState::loaded`,
/// which is what the nudge pad and the instance size
/// steppers act on.
fn load_instance(s: &mut State, index: usize) {
    let Some(inst) = s.core.project.texts.get(index) else {
        return;
    };
    let content = inst.content.clone();
    let size = inst.size;
    s.text_tool.field.replace_text(content);
    s.text_tool.size = size.clamp(SIZE_MIN, SIZE_MAX);
    s.text_tool.loaded = Some(index);
}

/// Where a remembered `loaded` index points after the
/// instance at `removed` is deleted: the loaded instance
/// itself is gone (`None`); an instance after the gap
/// shifts down one; one before it is unmoved. Pure.
pub fn loaded_after_removal(loaded: Option<usize>, removed: usize) -> Option<usize> {
    match loaded {
        Some(l) if l == removed => None,
        Some(l) if l > removed => Some(l - 1),
        other => other,
    }
}

/// Where a remembered `loaded` index points after a new
/// instance is inserted at `inserted_at`: an instance at
/// or after the gap shifts up one. Pure.
pub fn loaded_after_insertion(loaded: Option<usize>, inserted_at: usize) -> Option<usize> {
    match loaded {
        Some(l) if l >= inserted_at => Some(l + 1),
        other => other,
    }
}

/// Delete-button handler: [`delete_text_instance`] plus
/// keeping `TextToolState::loaded` naming the same
/// instance afterwards.
fn delete_instance_row(s: &mut State, index: usize) {
    if index < s.core.project.texts.len() {
        s.text_tool.loaded = loaded_after_removal(s.text_tool.loaded, index);
    }
    delete_text_instance(&mut s.core, index);
}

/// Duplicate-button handler: [`duplicate_text_instance`]
/// plus the `loaded` fix-up for the insertion it makes
/// at `index + 1`.
fn duplicate_instance_row(s: &mut State, index: usize) {
    if duplicate_text_instance(&mut s.core, index) {
        s.text_tool.loaded = loaded_after_insertion(s.text_tool.loaded, index + 1);
    }
}

/// The loaded instance's index, or a status-only refusal
/// when no row click has loaded one. The core ops
/// bounds-check the index itself, so a stale `loaded`
/// refuses there instead.
fn loaded_index(s: &mut State) -> Option<usize> {
    match s.text_tool.loaded {
        Some(i) => Some(i),
        None => {
            s.core.status = "Load a text first.".into();
            None
        }
    }
}

/// Update-button handler (Phase J §24): write the
/// field's current content back into the loaded
/// instance via [`update_text_instance`]. The button
/// renders only while a live instance is loaded, but
/// the op bounds-checks the index itself, so a stale
/// `loaded` refuses there instead of writing.
fn update_loaded(s: &mut State) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    let content = s.text_tool.field.text.clone();
    update_text_instance(&mut s.core, i, &content);
}

/// Rotate-button handler (Phase K §30): turn the
/// loaded instance by `delta_rad` via
/// [`rotate_text_instance`]. The two buttons pass
/// ∓/± [`ROTATE_STEP_RAD`]; resolution and refusal
/// are [`loaded_index`]'s and the op's, exactly like
/// the nudge pad's presses.
fn rotate_loaded(s: &mut State, delta_rad: f32) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    rotate_text_instance(&mut s.core, i, delta_rad);
}

/// Swatch-press handler (Phase L §23): recolour the
/// loaded instance via [`set_text_color`]. Resolution
/// and refusal are [`loaded_index`]'s and the op's,
/// exactly like the rotate row's presses.
fn set_loaded_color(s: &mut State, color: u32) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    set_text_color(&mut s.core, i, color);
}

/// Z-order handler (Phase M §29): move the loaded
/// instance to an end of the draw order via
/// [`move_text_instance`], then follow it —
/// `TextToolState::loaded` is rewritten to the
/// instance's new index (the same [`moved_index`]
/// computation the op moved it to), so the edit
/// area keeps editing the same text. When the op
/// refuses (a stale index, or the instance already
/// at the target end) nothing moved, so `loaded`
/// is left exactly as it was.
fn move_loaded(s: &mut State, to_front: bool) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    if move_text_instance(&mut s.core, i, to_front) {
        s.text_tool.loaded = moved_index(s.core.project.texts.len(), i, to_front);
    }
}

/// Nudge-pad handler: move the loaded instance by
/// `(dx, dy)`. The buttons pass one [`NUDGE_STEP`] on a
/// single axis (stage coordinates are y-down, so Up is
/// a negative `dy`); no shift modifier exists on Rune
/// buttons, so one step size is all a press can mean.
fn nudge_loaded(s: &mut State, dx: f32, dy: f32) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    nudge_text_instance(&mut s.core, i, dx, dy);
}

/// Instance size-stepper handler: resize the loaded
/// instance, and keep the tool's pending size in step
/// with it, the way [`load_instance`] set it.
fn resize_loaded(s: &mut State, dir: i32) {
    let Some(i) = loaded_index(s) else {
        return;
    };
    resize_text_instance(&mut s.core, i, dir);
    if let Some(inst) = s.core.project.texts.get(i) {
        s.text_tool.size = inst.size;
    }
}

/// Glyph height of the selected node's text segment, if
/// the selected node carries one.
pub fn selected_text_size(core: &EditorCore) -> Option<f32> {
    let node = core.selected_node?;
    match core
        .project
        .figures
        .get(core.selected_figure)?
        .def
        .nodes
        .get(node)?
        .media
    {
        Some(SegmentMedia::Text { size, .. }) => Some(size),
        _ => None,
    }
}

/// The size the stepper shows and steps from: the selected
/// text segment's real size when there is one, otherwise
/// the pending size for the next add.
pub fn display_size(core: &EditorCore, pending: f32) -> f32 {
    selected_text_size(core).unwrap_or(pending)
}

/// One stepper step from `size` in `dir` (−1/+1), clamped
/// to `SIZE_MIN..=SIZE_MAX`. Pure; the stepper's only math.
pub fn step_size(size: f32, dir: i32) -> f32 {
    (size + dir as f32 * SIZE_STEP).clamp(SIZE_MIN, SIZE_MAX)
}

/// "Add text": put `content` on the selected node's
/// segment at glyph height `size`, through the engine's
/// real session ops ([`SegTextEdit::begin`] → `push_char`
/// → [`SegTextEdit::commit`]).
///
/// Status-only no-ops (nothing is snapshotted or written):
/// empty `content`, no selected node, or a stale node
/// index. An existing text on the node is *replaced* by
/// `content` (the session buffer is cleared with the
/// session's own `backspace` before typing), never
/// appended to.
pub fn add_text_to_core(core: &mut EditorCore, content: &str, size: f32) {
    if content.is_empty() {
        core.status = "Type some text first.".into();
        return;
    }
    let Some(node) = core.selected_node else {
        core.status = "Click a joint first, then Add text.".into();
        return;
    };
    let fig = core.selected_figure;
    let Some(mut session) = SegTextEdit::begin(&core.project, fig, node) else {
        core.status = "No such segment.".into();
        return;
    };
    while session.char_len() > 0 {
        session.backspace();
    }
    for c in content.chars() {
        session.push_char(c);
    }
    if session.char_len() == 0 {
        // Only control characters were typed; committing
        // now would DELETE the node's existing media
        // (empty-buffer commit semantics) — refuse.
        core.status = "Type some text first.".into();
        return;
    }
    core.snapshot();
    if !session.commit(&mut core.project) {
        core.status = "Text target no longer exists.".into();
        return;
    }
    // No size parameter exists on commit (see module
    // docs): write the chosen glyph height onto the fresh
    // media, under the snapshot already taken.
    if let Some(def) = core
        .project
        .figures
        .get_mut(fig)
        .and_then(|f| f.def.nodes.get_mut(node))
    {
        if let Some(SegmentMedia::Text { size: sz, .. }) = &mut def.media {
            *sz = size.clamp(SIZE_MIN, SIZE_MAX);
        }
    }
    core.status = "Text set.".into();
}

/// Add button / field-submit handler: the field's current
/// content at the pending size.
fn add_from_field(s: &mut State) {
    let content = s.text_tool.field.text.clone();
    let size = s.text_tool.size;
    add_text_to_core(&mut s.core, &content, size);
}

/// Size stepper handler: step from the displayed size; if
/// the selected node already carries a text segment, the
/// new size is applied to it live (one snapshot, same
/// media-write idiom as the add path — no dedicated size
/// op exists). Otherwise only the pending size changes.
fn step_from_button(s: &mut State, dir: i32) {
    let next = step_size(display_size(&s.core, s.text_tool.size), dir);
    s.text_tool.size = next;
    let Some(node) = s.core.selected_node else {
        return;
    };
    if selected_text_size(&s.core) == Some(next) {
        return;
    }
    let fig = s.core.selected_figure;
    s.core.snapshot();
    if let Some(def) = s
        .core
        .project
        .figures
        .get_mut(fig)
        .and_then(|f| f.def.nodes.get_mut(node))
    {
        if let Some(SegmentMedia::Text { size: sz, .. }) = &mut def.media {
            *sz = next;
            s.core.status = format!("Text size {next:.0}.");
        }
    }
}

/// The Text section: field + Add text, the size stepper,
/// the selected figure's existing text segments, the
/// project's free-standing text instances, and the
/// loaded instance's nudge pad, rotate row and size
/// steppers.
/// The loaded instance's colour row (Phase L §23):
/// the [`TEXT_PALETTE`] swatches in the particles
/// panel's construction — a 26 pt slot per colour,
/// accent-filled behind the 22 pt colour chip when it
/// is the loaded instance's colour (compared masked
/// to `0xRRGGBB`) — plus a readout chip
/// (`text-color-custom`) carrying the instance's own
/// colour even when no swatch matches it. Built
/// either way, like the nudge pad: with nothing
/// loaded no slot is lit and no readout shows, and a
/// press refuses through [`loaded_index`].
fn text_color_row(s: &State, t: &Theme) -> El {
    let loaded_color = s
        .text_tool
        .loaded
        .and_then(|i| s.core.project.texts.get(i))
        .map(|inst| inst.color & 0xFF_FFFF);
    let accent = tok::accent(0, true);
    let mut row_el = row().key("text-color-row").w_full().gap(4.0).center_y();
    for (ci, &rgb) in TEXT_PALETTE.iter().enumerate() {
        row_el = row_el.child(
            el()
                .key(format!("text-color-{ci}"))
                .size(26.0, 26.0)
                .radius(8.0)
                .fill(if loaded_color == Some(rgb) { accent } else { t.bg })
                .center()
                .on_press(move |s: &mut State| set_loaded_color(s, rgb))
                .child(
                    el()
                        .size(22.0, 22.0)
                        .radius(6.0)
                        .fill(crate::stage_view::hex_color(rgb).unwrap_or(t.bg)),
                ),
        );
    }
    if let Some(current) = loaded_color {
        row_el = row_el.child(
            el()
                .key("text-color-custom")
                .size(26.0, 26.0)
                .radius(8.0)
                .fill(t.bg)
                .center()
                .child(
                    el()
                        .size(22.0, 22.0)
                        .radius(6.0)
                        .fill(crate::stage_view::hex_color(current).unwrap_or(t.bg)),
                ),
        );
    }
    row_el
}

pub fn text_tool(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let rows = text_segments(core);
    let shown_size = display_size(core, s.text_tool.size);

    // Free-standing text instances: one row per instance —
    // a load button carrying the content preview (clicking
    // loads it into the field above for re-stamping), its
    // key count and glyph height, a Delete button and a
    // Duplicate button. All buttons carry the instance's
    // real vec index.
    let instances = text_instances(core);
    let mut texts_list = col().key("text-instances").gap(2.0);
    texts_list = texts_list.child(
        text("Existing texts")
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    if instances.is_empty() {
        texts_list = texts_list.child(
            text("No free-standing texts.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        for r in &instances {
            let i = r.index;
            texts_list = texts_list.child(
                row()
                    .key(format!("text-row-{i}"))
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(btn(&format!("text-load-{i}"), &r.preview, false, t, move |s: &mut State| {
                        load_instance(s, i)
                    }))
                    .child(
                        text(format!(
                            "{} key{} · {:.0}",
                            r.keys,
                            if r.keys == 1 { "" } else { "s" },
                            r.size
                        ))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                    )
                    .child(btn(&format!("text-del-{i}"), "Delete", false, t, move |s: &mut State| {
                        delete_instance_row(s, i)
                    }))
                    .child(btn(&format!("text-dup-{i}"), "Duplicate", false, t, move |s: &mut State| {
                        duplicate_instance_row(s, i)
                    })),
            );
        }
    }

    // The loaded instance's edit area: the Update text
    // write-back button (Phase J §24), a nudge pad, a
    // rotate row (Phase K §30), a colour row (Phase
    // L §23) and size steppers acting
    // on `TextToolState::loaded` —
    // the instance the last row click loaded. Rendered
    // either way; with nothing loaded the handlers refuse
    // with a status line, and with a stale index the core
    // ops refuse the same way. The Update button and the
    // z-order row (Phase M §29) are the conditional
    // pieces: they appear only while `loaded` names an
    // instance that still exists.
    let loaded_row = instances
        .iter()
        .find(|r| Some(r.index) == s.text_tool.loaded);
    let loaded_label = match loaded_row {
        Some(r) => format!("Loaded: {}", r.preview),
        None => "No text loaded — click a text above to load it.".to_string(),
    };
    let loaded_size = match loaded_row {
        Some(r) => format!("{:.0}", r.size),
        None => "—".to_string(),
    };
    let mut instance_edit = col()
        .key("text-instance-edit")
        .w_full()
        .gap(2.0)
        .child(
            text(loaded_label)
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    if loaded_row.is_some() {
        instance_edit = instance_edit
            .child(
                row()
                    .key("text-update-row")
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(btn("text-update", "Update text", false, t, |s| {
                        update_loaded(s)
                    })),
            )
            .child(
                row()
                    .key("text-zorder-row")
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(btn("text-front", "Bring to front", false, t, |s| {
                        move_loaded(s, true)
                    }))
                    .child(btn("text-back", "Send to back", false, t, |s| {
                        move_loaded(s, false)
                    })),
            );
    }
    instance_edit = instance_edit
        .child(
            row()
                .key("text-nudge-pad")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("text-nudge-l", "←", false, t, |s| {
                    nudge_loaded(s, -NUDGE_STEP, 0.0)
                }))
                .child(btn("text-nudge-u", "↑", false, t, |s| {
                    nudge_loaded(s, 0.0, -NUDGE_STEP)
                }))
                .child(btn("text-nudge-d", "↓", false, t, |s| {
                    nudge_loaded(s, 0.0, NUDGE_STEP)
                }))
                .child(btn("text-nudge-r", "→", false, t, |s| {
                    nudge_loaded(s, NUDGE_STEP, 0.0)
                })),
        )
        .child(
            row()
                .key("text-rotate-row")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("text-rot-l", "Rotate −15°", false, t, |s| {
                    rotate_loaded(s, -ROTATE_STEP_RAD)
                }))
                .child(btn("text-rot-r", "Rotate +15°", false, t, |s| {
                    rotate_loaded(s, ROTATE_STEP_RAD)
                })),
        )
        .child(text_color_row(s, t))
        .child(
            row()
                .key("text-instance-size")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("text-isize-dn", "Size −", false, t, |s| {
                    resize_loaded(s, -1)
                }))
                .child(
                    text(loaded_size)
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("text-isize-up", "Size +", false, t, |s| {
                    resize_loaded(s, 1)
                })),
        );

    let mut list = col().key("text-list").gap(2.0);
    if rows.is_empty() {
        list = list.child(
            text("No text segments on this figure.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        for r in &rows {
            let selected = core.selected_node == Some(r.node);
            list = list.child(
                text(format!(
                    "{}: “{}” · {:.0}",
                    r.name, r.content, r.size
                ))
                .font(type_::LABEL)
                .color(if selected { t.text } else { t.text_secondary }),
            );
        }
    }

    col()
        .key("text-tool")
        .w_full()
        .gap(6.0)
        .child(
            text("Text")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text_input(&s.text_tool.field, type_::BODY.lh(26.0))
                .key("text-input")
                .single_line()
                .w_full()
                .fill(t.card)
                .radius(6.0)
                .px(8.0)
                .py(4.0)
                .color(t.text)
                .placeholder("Segment text", t.text_secondary)
                .on_edit(|s: &mut State| &mut s.text_tool.field)
                .on_submit(|s: &mut State, _tx: &mut Tx| add_from_field(s)),
        )
        .child(
            row()
                .key("text-actions")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("text-seg", "Add text", false, t, |s| add_from_field(s)))
                .child(btn("text-size-down", "Size −", false, t, |s| {
                    step_from_button(s, -1)
                }))
                .child(
                    text(format!("{shown_size:.0}"))
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("text-size-up", "Size +", false, t, |s| {
                    step_from_button(s, 1)
                })),
        )
        .child(list)
        .child(texts_list)
        .child(instance_edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::{InstanceKey, TextInstance};

    fn push_instance(core: &mut EditorCore, content: &str, size: f32, keys: usize) {
        core.project.texts.push(TextInstance {
            content: content.into(),
            size,
            color: 0xFF_FFFF,
            keys: (0..keys)
                .map(|k| InstanceKey {
                    frame: k as f32,
                    x: 0.0,
                    y: 0.0,
                    scale: 1.0,
                    angle: 0.0,
                })
                .collect(),
        });
    }

    fn core_with_node(node: usize) -> EditorCore {
        let mut core = EditorCore::new();
        core.selected_node = Some(node);
        core
    }

    fn media_of(core: &EditorCore, node: usize) -> Option<(String, f32)> {
        match &core.project.figures[0].def.nodes[node].media {
            Some(SegmentMedia::Text { content, size }) => Some((content.clone(), *size)),
            _ => None,
        }
    }

    #[test]
    fn step_size_steps_and_clamps() {
        assert_eq!(step_size(28.0, 1), 32.0);
        assert_eq!(step_size(28.0, -1), 24.0);
        assert_eq!(step_size(SIZE_MAX, 1), SIZE_MAX);
        assert_eq!(step_size(SIZE_MIN, -1), SIZE_MIN);
        assert_eq!(step_size(1000.0, 1), SIZE_MAX);
    }

    #[test]
    fn add_text_empty_content_is_status_only_noop() {
        let mut core = core_with_node(1);
        let status_before = core.status.clone();
        add_text_to_core(&mut core, "", 28.0);
        assert_eq!(core.status, "Type some text first.");
        assert_ne!(core.status, status_before);
        assert!(media_of(&core, 1).is_none(), "nothing was written");
    }

    #[test]
    fn add_text_without_selection_is_status_only_noop() {
        let mut core = EditorCore::new(); // selected_node: None
        add_text_to_core(&mut core, "Hello", 28.0);
        assert_eq!(core.status, "Click a joint first, then Add text.");
        assert!(text_segments(&core).is_empty());
    }

    #[test]
    fn add_text_with_stale_node_is_status_only_noop() {
        let mut core = core_with_node(999);
        add_text_to_core(&mut core, "Hello", 28.0);
        assert_eq!(core.status, "No such segment.");
    }

    #[test]
    fn add_text_writes_content_at_chosen_size() {
        let mut core = core_with_node(1);
        add_text_to_core(&mut core, "Hi", 44.0);
        assert_eq!(core.status, "Text set.");
        assert_eq!(media_of(&core, 1), Some(("Hi".to_string(), 44.0)));
    }

    #[test]
    fn add_text_replaces_existing_content_and_size() {
        let mut core = core_with_node(1);
        add_text_to_core(&mut core, "Old", 20.0);
        add_text_to_core(&mut core, "New text", 60.0);
        assert_eq!(media_of(&core, 1), Some(("New text".to_string(), 60.0)));
    }

    #[test]
    fn add_text_control_only_content_never_clears_existing_media() {
        let mut core = core_with_node(1);
        add_text_to_core(&mut core, "Keep", 28.0);
        // A field full of control characters pushes nothing
        // into the session; the empty buffer must NOT be
        // committed (that would delete the media).
        add_text_to_core(&mut core, "\n\t", 28.0);
        assert_eq!(core.status, "Type some text first.");
        assert_eq!(media_of(&core, 1), Some(("Keep".to_string(), 28.0)));
    }

    #[test]
    fn add_text_is_one_undo_step() {
        let mut core = core_with_node(1);
        add_text_to_core(&mut core, "Hi", 44.0);
        core.undo();
        assert!(media_of(&core, 1).is_none(), "one undo removes the add");
    }

    #[test]
    fn text_segments_lists_selected_figure_in_node_order() {
        let mut core = EditorCore::new();
        core.selected_node = Some(2);
        add_text_to_core(&mut core, "Second", 16.0);
        core.selected_node = Some(1);
        add_text_to_core(&mut core, "First", 28.0);
        let rows = text_segments(&core);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].node, 1);
        assert_eq!(rows[0].content, "First");
        assert_eq!(rows[0].size, 28.0);
        assert_eq!(rows[1].node, 2);
        assert_eq!(rows[1].content, "Second");
        assert!(!rows[0].name.is_empty(), "node name carried through");
    }

    #[test]
    fn preview_keeps_short_and_exact_fit_content_whole() {
        assert_eq!(preview(""), "");
        assert_eq!(preview("Hello"), "Hello");
        let exact = "a".repeat(PREVIEW_CHARS);
        assert_eq!(preview(&exact), exact, "exactly at the cap is not truncated");
    }

    #[test]
    fn preview_truncates_long_content_with_an_ellipsis() {
        let long = "a".repeat(PREVIEW_CHARS + 5);
        let shown = preview(&long);
        assert_eq!(shown, format!("{}…", "a".repeat(PREVIEW_CHARS)));
        assert_eq!(shown.chars().count(), PREVIEW_CHARS + 1);
    }

    #[test]
    fn preview_counts_characters_not_bytes() {
        // Each é is two bytes in UTF-8: a byte-based cut at
        // 18 would split or miscount; the char-based one
        // keeps 18 whole characters.
        let long = "é".repeat(PREVIEW_CHARS + 2);
        let shown = preview(&long);
        assert_eq!(shown, format!("{}…", "é".repeat(PREVIEW_CHARS)));
    }

    #[test]
    fn text_preview_keeps_short_content_unchanged() {
        assert_eq!(text_preview("Hello"), "Hello");
        assert_eq!(text_preview("Hi"), "Hi");
    }

    #[test]
    fn text_preview_empty_content_stays_empty() {
        assert_eq!(text_preview(""), "");
    }

    #[test]
    fn text_preview_keeps_exactly_fourteen_characters_unchanged() {
        let exact = "a".repeat(TEXT_PREVIEW_CHARS);
        assert_eq!(exact.chars().count(), 14);
        assert_eq!(text_preview(&exact), exact, "exactly at the cap is not truncated");
    }

    #[test]
    fn text_preview_truncates_fifteen_characters_with_an_ellipsis() {
        let fifteen = "a".repeat(TEXT_PREVIEW_CHARS + 1);
        let shown = text_preview(&fifteen);
        assert_eq!(shown, format!("{}…", "a".repeat(TEXT_PREVIEW_CHARS)));
        assert_eq!(shown.chars().count(), TEXT_PREVIEW_CHARS + 1);
    }

    #[test]
    fn text_preview_counts_multibyte_characters_as_characters() {
        // Each é is two bytes in UTF-8: a byte-based cut
        // at 14 would split or miscount; the char-based
        // one keeps 14 whole characters.
        let long = "é".repeat(TEXT_PREVIEW_CHARS + 1);
        let shown = text_preview(&long);
        assert_eq!(shown, format!("{}…", "é".repeat(TEXT_PREVIEW_CHARS)));
        // A short multi-byte string under the cap is kept
        // whole, accents and all.
        assert_eq!(text_preview("héllo wörld"), "héllo wörld");
    }

    #[test]
    fn text_preview_flattens_newlines_to_spaces() {
        assert_eq!(text_preview("Hello\nworld"), "Hello world");
        assert_eq!(text_preview("one\r\ntwo"), "one  two", "each break char becomes a space");
        // Flattening happens before the cap: the flattened
        // text is what gets counted and cut.
        assert_eq!(
            text_preview("line one\nline two here"),
            "line one line …",
            "flattened to 22 chars, then cut to the first 14 plus an ellipsis"
        );
    }

    #[test]
    fn text_instances_rows_preview_is_the_single_line_text_preview() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "First line\nand a good deal more after it", 24.0, 1);
        let rows = text_instances(&core);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].preview,
            text_preview("First line\nand a good deal more after it"),
            "the row preview is the §36 preview of the content"
        );
        assert!(
            !rows[0].preview.contains('\n'),
            "a row preview never carries a line break"
        );
        assert_eq!(rows[0].preview, "First line and…");
    }

    #[test]
    fn text_instances_rows_carry_real_index_preview_keys_and_size() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "First label", 24.0, 2);
        push_instance(&mut core, "A much longer second label", 48.0, 1);
        let rows = text_instances(&core);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[0].preview, "First label");
        assert_eq!(rows[0].keys, 2);
        assert_eq!(rows[0].size, 24.0);
        assert_eq!(rows[1].index, 1);
        assert_eq!(rows[1].preview, text_preview("A much longer second label"));
        assert_eq!(rows[1].keys, 1);
        assert_eq!(rows[1].size, 48.0);
    }

    #[test]
    fn delete_text_instance_removes_the_addressed_instance() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "A", 20.0, 1);
        push_instance(&mut core, "B", 20.0, 1);
        push_instance(&mut core, "C", 20.0, 1);
        delete_text_instance(&mut core, 1);
        assert_eq!(core.status, "Text deleted.");
        let contents: Vec<Option<String>> = (0..3)
            .map(|i| instance_content(&core, i))
            .collect();
        assert_eq!(
            contents,
            vec![Some("A".to_string()), Some("C".to_string()), None],
            "B is gone, A and C keep their order"
        );
    }

    #[test]
    fn delete_text_instance_is_one_undo_step() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "Keep me", 20.0, 1);
        delete_text_instance(&mut core, 0);
        assert!(core.project.texts.is_empty());
        core.undo();
        assert_eq!(core.project.texts.len(), 1, "one undo restores the delete");
        assert_eq!(core.project.texts[0].content, "Keep me");
    }

    #[test]
    fn delete_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "A", 20.0, 1);
        let depth_before = core.undo_depth();
        delete_text_instance(&mut core, 7);
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.project.texts.len(), 1, "nothing was removed");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn instance_content_returns_full_text_and_none_out_of_range() {
        let mut core = EditorCore::new();
        let full = "A much longer second label";
        push_instance(&mut core, full, 48.0, 1);
        assert_eq!(instance_content(&core, 0), Some(full.to_string()));
        assert_eq!(instance_content(&core, 1), None);
    }

    #[test]
    fn selected_text_size_and_display_size_follow_selection() {
        let mut core = core_with_node(1);
        assert_eq!(selected_text_size(&core), None);
        assert_eq!(display_size(&core, 36.0), 36.0, "pending size shown");
        add_text_to_core(&mut core, "Hi", 52.0);
        assert_eq!(selected_text_size(&core), Some(52.0));
        assert_eq!(display_size(&core, 36.0), 52.0, "real size wins");
    }

    /// Instance fixture with real key positions: `keys`
    /// are `(frame, x, y)` triples; scale and angle are
    /// fixed non-trivial values so tests can prove a
    /// duplicate copies them verbatim.
    fn push_instance_pos(
        core: &mut EditorCore,
        content: &str,
        size: f32,
        keys: &[(f32, f32, f32)],
    ) {
        core.project.texts.push(TextInstance {
            content: content.into(),
            size,
            color: 0xFF_204060,
            keys: keys
                .iter()
                .map(|&(frame, x, y)| InstanceKey {
                    frame,
                    x,
                    y,
                    scale: 1.5,
                    angle: 30.0,
                })
                .collect(),
        });
    }

    fn key_positions(core: &EditorCore, index: usize) -> Vec<(f32, f32)> {
        core.project.texts[index]
            .keys
            .iter()
            .map(|k| (k.x, k.y))
            .collect()
    }

    #[test]
    fn duplicated_instance_shifts_every_key_and_copies_the_rest() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "Label", 36.0, &[(0.0, 10.0, 20.0), (4.0, 50.0, 60.0)]);
        let copy = duplicated_instance(&core.project.texts[0]);
        assert_eq!(copy.content, "Label");
        assert_eq!(copy.size, 36.0);
        assert_eq!(copy.color, 0xFF_204060);
        assert_eq!(copy.keys.len(), 2);
        assert_eq!(
            (copy.keys[0].x, copy.keys[0].y),
            (10.0 + DUP_OFFSET, 20.0 + DUP_OFFSET),
            "first key shifted by the duplicate offset"
        );
        assert_eq!(
            (copy.keys[1].x, copy.keys[1].y),
            (50.0 + DUP_OFFSET, 60.0 + DUP_OFFSET),
            "every key shifts, not just the first"
        );
        assert_eq!(copy.keys[1].frame, 4.0, "timing copied verbatim");
        assert_eq!(copy.keys[1].scale, 1.5, "scale copied verbatim");
        assert_eq!(copy.keys[1].angle, 30.0, "angle copied verbatim");
        assert_eq!(
            key_positions(&core, 0),
            vec![(10.0, 20.0), (50.0, 60.0)],
            "the original is not moved"
        );
    }

    #[test]
    fn duplicate_text_instance_inserts_offset_clone_after_original() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        push_instance_pos(&mut core, "B", 20.0, &[(0.0, 90.0, 90.0)]);
        let depth_before = core.undo_depth();
        assert!(duplicate_text_instance(&mut core, 0));
        assert_eq!(core.status, "Text duplicated.");
        assert_eq!(core.undo_depth(), depth_before + 1, "one snapshot per duplicate");
        let contents: Vec<&str> = core
            .project
            .texts
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(contents, ["A", "A", "B"], "the copy lands right after its original");
        assert_eq!(key_positions(&core, 0), vec![(10.0, 20.0)], "original unmoved");
        assert_eq!(
            key_positions(&core, 1),
            vec![(10.0 + DUP_OFFSET, 20.0 + DUP_OFFSET)],
            "copy offset from the original"
        );
        assert_eq!(key_positions(&core, 2), vec![(90.0, 90.0)], "later instances unmoved");
        core.undo();
        assert_eq!(core.project.texts.len(), 2, "one undo removes the duplicate");
    }

    #[test]
    fn duplicate_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!duplicate_text_instance(&mut core, 7));
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.project.texts.len(), 1, "nothing was inserted");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn nudge_text_instance_accumulates_deltas_across_presses() {
        let mut core = EditorCore::new();
        push_instance_pos(
            &mut core,
            "A",
            20.0,
            &[(0.0, 100.0, 100.0), (2.0, 120.0, 90.0)],
        );
        let depth_before = core.undo_depth();
        assert!(nudge_text_instance(&mut core, 0, NUDGE_STEP, 0.0));
        assert!(nudge_text_instance(&mut core, 0, NUDGE_STEP, 0.0));
        assert!(nudge_text_instance(&mut core, 0, 0.0, -NUDGE_STEP));
        assert_eq!(core.status, "Text nudged.");
        assert_eq!(
            key_positions(&core, 0),
            vec![(108.0, 96.0), (128.0, 86.0)],
            "two +x presses and one −y press accumulate on every key"
        );
        assert_eq!(
            core.undo_depth(),
            depth_before + 3,
            "one snapshot per press — presses do not coalesce"
        );
        let key = core.project.texts[0].keys[1];
        assert_eq!(key.frame, 2.0, "a nudge never changes timing");
        assert_eq!(key.scale, 1.5, "a nudge never changes scale");
        assert_eq!(key.angle, 30.0, "a nudge never changes angle");
    }

    #[test]
    fn nudge_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!nudge_text_instance(&mut core, 7, NUDGE_STEP, 0.0));
        assert_eq!(core.status, "No such text.");
        assert_eq!(key_positions(&core, 0), vec![(10.0, 20.0)], "nothing moved");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn nudge_text_instance_without_keys_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "Keyless", 20.0, 0);
        assert!(core.project.texts[0].keys.is_empty());
        let depth_before = core.undo_depth();
        assert!(!nudge_text_instance(&mut core, 0, NUDGE_STEP, 0.0));
        assert_eq!(core.status, "Text has no position to move.");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn resize_text_instance_steps_like_the_segment_stepper() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 28.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(resize_text_instance(&mut core, 0, 1));
        assert_eq!(core.project.texts[0].size, 28.0 + SIZE_STEP);
        assert_eq!(core.status, "Text size 32.");
        assert_eq!(core.undo_depth(), depth_before + 1, "one snapshot per resize");
        assert!(resize_text_instance(&mut core, 0, -1));
        assert_eq!(core.project.texts[0].size, 28.0);
        core.undo();
        assert_eq!(core.project.texts[0].size, 32.0, "one undo steps the size back once");
    }

    #[test]
    fn resize_text_instance_clamps_without_snapshotting_at_the_bounds() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", SIZE_MAX, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!resize_text_instance(&mut core, 0, 1), "already at the max");
        assert_eq!(core.project.texts[0].size, SIZE_MAX);
        assert_eq!(core.status, "Text size 160.");
        core.project.texts[0].size = SIZE_MIN;
        assert!(!resize_text_instance(&mut core, 0, -1), "already at the min");
        assert_eq!(core.project.texts[0].size, SIZE_MIN);
        assert_eq!(
            core.undo_depth(),
            depth_before,
            "a press that changes nothing snapshots nothing"
        );
    }

    #[test]
    fn resize_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 28.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!resize_text_instance(&mut core, 7, 1));
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.project.texts[0].size, 28.0, "nothing was resized");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn update_text_instance_writes_content_and_leaves_the_rest_untouched() {
        let mut core = EditorCore::new();
        push_instance_pos(
            &mut core,
            "Old label",
            36.0,
            &[(0.0, 10.0, 20.0), (4.0, 50.0, 60.0)],
        );
        let depth_before = core.undo_depth();
        assert!(update_text_instance(&mut core, 0, "New label"));
        assert_eq!(core.status, "Text 1 updated.");
        assert_eq!(core.project.texts[0].content, "New label");
        assert_eq!(core.project.texts[0].size, 36.0, "size untouched");
        assert_eq!(core.project.texts[0].color, 0xFF_204060, "colour untouched");
        assert_eq!(
            key_positions(&core, 0),
            vec![(10.0, 20.0), (50.0, 60.0)],
            "every key's position untouched"
        );
        let key = core.project.texts[0].keys[1];
        assert_eq!(key.frame, 4.0, "key timing untouched");
        assert_eq!(key.scale, 1.5, "key scale untouched");
        assert_eq!(key.angle, 30.0, "key angle untouched");
        assert_eq!(
            text_instances(&core)[0].preview, "New label",
            "row previews read the live content, so the edit shows at once"
        );
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the write-back"
        );
        core.undo();
        assert_eq!(
            core.project.texts[0].content, "Old label",
            "one undo restores the old content"
        );
    }

    #[test]
    fn update_text_instance_numbers_the_status_by_the_instance() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        push_instance_pos(&mut core, "B", 20.0, &[(0.0, 90.0, 90.0)]);
        assert!(update_text_instance(&mut core, 1, "B edited"));
        assert_eq!(core.status, "Text 2 updated.");
        assert_eq!(core.project.texts[0].content, "A", "the other instance is untouched");
        assert_eq!(core.project.texts[1].content, "B edited");
    }

    #[test]
    fn update_text_instance_unchanged_content_snapshots_nothing() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "Same", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!update_text_instance(&mut core, 0, "Same"));
        assert_eq!(core.status, "Text unchanged.");
        assert_eq!(core.project.texts[0].content, "Same");
        assert_eq!(
            core.undo_depth(),
            depth_before,
            "an unchanged write-back snapshots nothing"
        );
    }

    #[test]
    fn update_text_instance_out_of_range_is_status_only_noop() {
        // The stale-loaded case: `loaded` can outlive the
        // instance it names, and the handler passes the
        // index straight to this op — it must refuse
        // without panicking and without writing.
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!update_text_instance(&mut core, 7, "B"));
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.project.texts.len(), 1, "no instance was created");
        assert_eq!(core.project.texts[0].content, "A", "nothing was written");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    /// Instance fixture with distinct per-key angles:
    /// `keys` are `(frame, x, y, angle)` tuples, so a
    /// test can prove a rotate adds its delta to every
    /// key's own angle, not just the first key's.
    fn push_instance_angled(
        core: &mut EditorCore,
        content: &str,
        keys: &[(f32, f32, f32, f32)],
    ) {
        core.project.texts.push(TextInstance {
            content: content.into(),
            size: 24.0,
            color: 0xFF_102030,
            keys: keys
                .iter()
                .map(|&(frame, x, y, angle)| InstanceKey {
                    frame,
                    x,
                    y,
                    scale: 1.25,
                    angle,
                })
                .collect(),
        });
    }

    fn key_angles(core: &EditorCore, index: usize) -> Vec<f32> {
        core.project.texts[index]
            .keys
            .iter()
            .map(|k| k.angle)
            .collect()
    }

    #[test]
    fn rotate_text_instance_turns_every_key_and_touches_nothing_else() {
        let mut core = EditorCore::new();
        // Angles and delta are exact binary fractions,
        // so the sums compare exactly.
        push_instance_angled(
            &mut core,
            "Tilted",
            &[(0.0, 10.0, 20.0, 0.0), (2.0, 30.0, 40.0, 0.25), (5.0, 50.0, 60.0, -0.5)],
        );
        let depth_before = core.undo_depth();
        assert!(rotate_text_instance(&mut core, 0, 0.125));
        assert_eq!(core.status, "Text 1 rotated.");
        assert_eq!(
            key_angles(&core, 0),
            vec![0.125, 0.375, -0.375],
            "every key's angle gains the delta, not just the first"
        );
        let inst = &core.project.texts[0];
        assert_eq!(inst.content, "Tilted", "content untouched");
        assert_eq!(inst.size, 24.0, "size untouched");
        assert_eq!(inst.color, 0xFF_102030, "colour untouched");
        assert_eq!(
            key_positions(&core, 0),
            vec![(10.0, 20.0), (30.0, 40.0), (50.0, 60.0)],
            "every key's position untouched"
        );
        for (i, key) in inst.keys.iter().enumerate() {
            assert_eq!(key.frame, [0.0, 2.0, 5.0][i], "key timing untouched");
            assert_eq!(key.scale, 1.25, "key scale untouched");
        }
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the rotate"
        );
    }

    #[test]
    fn rotate_text_instance_keeps_equal_angles_equal() {
        let mut core = EditorCore::new();
        // The position fixture gives every key angle
        // 30.0: a uniform rotation keeps them equal.
        push_instance_pos(
            &mut core,
            "Level",
            20.0,
            &[(0.0, 10.0, 20.0), (4.0, 50.0, 60.0)],
        );
        assert!(rotate_text_instance(&mut core, 0, ROTATE_STEP_RAD));
        let angles = key_angles(&core, 0);
        assert_eq!(angles[0], angles[1], "equal angles stay equal under one delta");
        assert_eq!(angles[0], 30.0 + ROTATE_STEP_RAD);
        // And back: the opposite step restores the
        // original angle exactly (same addition reversed
        // is not guaranteed in floats, so compare the
        // pair's equality, which the uniform delta does
        // guarantee).
        assert!(rotate_text_instance(&mut core, 0, -ROTATE_STEP_RAD));
        let back = key_angles(&core, 0);
        assert_eq!(back[0], back[1], "still equal after rotating back");
    }

    #[test]
    fn rotate_text_instance_is_one_undo_step() {
        let mut core = EditorCore::new();
        push_instance_angled(
            &mut core,
            "Undo me",
            &[(0.0, 10.0, 20.0, 0.25), (3.0, 40.0, 50.0, -0.25)],
        );
        assert!(rotate_text_instance(&mut core, 0, 0.5));
        assert_eq!(key_angles(&core, 0), vec![0.75, 0.25]);
        core.undo();
        assert_eq!(
            key_angles(&core, 0),
            vec![0.25, -0.25],
            "one undo restores every key's angle at once"
        );
    }

    #[test]
    fn rotate_text_instance_numbers_the_status_by_the_instance() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        push_instance_pos(&mut core, "B", 20.0, &[(0.0, 90.0, 90.0)]);
        assert!(rotate_text_instance(&mut core, 1, ROTATE_STEP_RAD));
        assert_eq!(core.status, "Text 2 rotated.");
        assert_eq!(key_angles(&core, 0), vec![30.0], "the other instance is untouched");
        assert_eq!(key_angles(&core, 1), vec![30.0 + ROTATE_STEP_RAD]);
    }

    #[test]
    fn rotate_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!rotate_text_instance(&mut core, 7, ROTATE_STEP_RAD));
        assert_eq!(core.status, "No such text.");
        assert_eq!(key_angles(&core, 0), vec![30.0], "nothing was rotated");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn set_text_color_writes_the_colour_and_nothing_else() {
        let mut core = EditorCore::new();
        // The fixture's colour is 0xFF_102030.
        push_instance_angled(
            &mut core,
            "Coloured",
            &[(0.0, 10.0, 20.0, 0.25), (3.0, 40.0, 50.0, -0.25)],
        );
        let depth_before = core.undo_depth();
        assert!(set_text_color(&mut core, 0, TEXT_PALETTE[3]));
        assert_eq!(core.status, "Text 1 colour set to #7BE495.");
        let inst = &core.project.texts[0];
        assert_eq!(inst.color, 0x7BE495, "the swatch colour is written verbatim");
        assert_eq!(inst.content, "Coloured", "content untouched");
        assert_eq!(inst.size, 24.0, "size untouched");
        assert_eq!(
            key_angles(&core, 0),
            vec![0.25, -0.25],
            "transform keys untouched"
        );
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the recolour"
        );
        core.undo();
        assert_eq!(
            core.project.texts[0].color, 0xFF_102030,
            "one undo restores the old colour"
        );
    }

    #[test]
    fn set_text_color_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        let depth_before = core.undo_depth();
        assert!(!set_text_color(&mut core, 7, TEXT_PALETTE[0]));
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn set_loaded_color_recolours_the_loaded_instance_only() {
        let mut s = crate::init();
        push_instance(&mut s.core, "First", 24.0, 1);
        push_instance(&mut s.core, "Second", 24.0, 1);
        // Nothing loaded: the press refuses through
        // loaded_index and recolours nothing.
        set_loaded_color(&mut s, TEXT_PALETTE[0]);
        assert_eq!(s.core.status, "Load a text first.");
        assert_eq!(s.core.project.texts[0].color, 0xFF_FFFF);
        assert_eq!(s.core.project.texts[1].color, 0xFF_FFFF);
        // Loaded = the second instance: only it changes.
        s.text_tool.loaded = Some(1);
        set_loaded_color(&mut s, TEXT_PALETTE[0]);
        assert_eq!(s.core.status, "Text 2 colour set to #FF5D5D.");
        assert_eq!(
            s.core.project.texts[0].color, 0xFF_FFFF,
            "the unloaded instance keeps its colour"
        );
        assert_eq!(s.core.project.texts[1].color, TEXT_PALETTE[0]);
    }

    #[test]
    fn text_palette_is_the_particles_palette() {
        // One swatch language across the shell: the
        // values are the particles panel's, exactly.
        assert_eq!(
            TEXT_PALETTE,
            crate::rune_sec::particles_panel::PARTICLE_PALETTE
        );
        let mut sorted = TEXT_PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "every swatch colour is distinct");
    }

    #[test]
    fn text_tool_builds_with_and_without_a_loaded_instance() {
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        let _ = text_tool(&s, &t);
        push_instance(&mut s.core, "Loaded", 24.0, 1);
        s.text_tool.loaded = Some(0);
        let _ = text_tool(&s, &t);
    }

    #[test]
    fn rotate_text_instance_without_keys_still_rotates() {
        // The documented decision (module docs, Phase K
        // §30): rotation mirrors duplicate, not the
        // nudge — a keyless instance is cloned by
        // duplicate under one snapshot without refusing,
        // so a keyless rotate likewise snapshots once,
        // reports success and returns true; the per-key
        // loop is a no-op over the empty track.
        let mut core = EditorCore::new();
        push_instance(&mut core, "Keyless", 20.0, 0);
        assert!(core.project.texts[0].keys.is_empty());
        let depth_before = core.undo_depth();
        assert!(rotate_text_instance(&mut core, 0, ROTATE_STEP_RAD));
        assert_eq!(core.status, "Text 1 rotated.");
        assert!(core.project.texts[0].keys.is_empty(), "still no keys, nothing invented");
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "one snapshot, like a keyless duplicate"
        );
        core.undo();
        assert_eq!(core.project.texts.len(), 1, "the undo step is the empty rotate");
    }

    #[test]
    fn rotate_step_is_fifteen_degrees() {
        assert_eq!(ROTATE_STEP_RAD, std::f32::consts::PI / 12.0);
        assert!(
            (ROTATE_STEP_RAD.to_degrees() - 15.0).abs() < 0.001,
            "the step the two buttons are labelled with"
        );
    }

    #[test]
    fn loaded_index_fixups_follow_removals_and_insertions() {
        assert_eq!(loaded_after_removal(None, 0), None);
        assert_eq!(loaded_after_removal(Some(2), 2), None, "the loaded instance itself was deleted");
        assert_eq!(loaded_after_removal(Some(3), 1), Some(2), "after the gap shifts down");
        assert_eq!(loaded_after_removal(Some(0), 1), Some(0), "before the gap is unmoved");
        assert_eq!(loaded_after_insertion(None, 1), None);
        assert_eq!(loaded_after_insertion(Some(0), 1), Some(0), "before the gap is unmoved");
        assert_eq!(loaded_after_insertion(Some(1), 1), Some(2), "at the gap shifts up");
        assert_eq!(loaded_after_insertion(Some(3), 1), Some(4), "after the gap shifts up");
    }

    #[test]
    fn moved_index_targets_the_draw_order_ends() {
        // The front of the stack is the vec's end.
        assert_eq!(moved_index(3, 0, true), Some(2));
        assert_eq!(moved_index(3, 1, true), Some(2));
        assert_eq!(moved_index(5, 2, true), Some(4));
        // The back of the stack is index 0.
        assert_eq!(moved_index(3, 1, false), Some(0));
        assert_eq!(moved_index(3, 2, false), Some(0));
        assert_eq!(moved_index(5, 4, false), Some(0));
    }

    #[test]
    fn moved_index_is_none_when_there_is_nowhere_to_move() {
        assert_eq!(moved_index(3, 2, true), None, "already at the front");
        assert_eq!(moved_index(3, 0, false), None, "already at the back");
        assert_eq!(moved_index(1, 0, true), None, "a lone instance is both ends at once");
        assert_eq!(moved_index(1, 0, false), None, "a lone instance is both ends at once");
        assert_eq!(moved_index(3, 3, true), None, "index out of range");
        assert_eq!(moved_index(3, 7, false), None, "index out of range");
        assert_eq!(moved_index(0, 0, true), None, "an empty vec has no front");
        assert_eq!(moved_index(0, 0, false), None, "an empty vec has no back");
    }

    #[test]
    fn move_text_instance_to_the_front_keeps_the_others_in_order() {
        let mut core = EditorCore::new();
        push_instance_pos(&mut core, "A", 20.0, &[(0.0, 10.0, 20.0)]);
        push_instance_pos(&mut core, "B", 20.0, &[(0.0, 30.0, 40.0)]);
        push_instance_pos(&mut core, "C", 20.0, &[(0.0, 50.0, 60.0)]);
        push_instance_pos(&mut core, "D", 20.0, &[(0.0, 70.0, 80.0)]);
        let depth_before = core.undo_depth();
        assert!(move_text_instance(&mut core, 0, true));
        assert_eq!(core.status, "Text 4 moved to the front.");
        let contents: Vec<&str> = core
            .project
            .texts
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(
            contents,
            ["B", "C", "D", "A"],
            "A goes last (drawn on top); B, C and D keep their relative order"
        );
        assert_eq!(
            key_positions(&core, 3),
            vec![(10.0, 20.0)],
            "the moved instance rides along verbatim"
        );
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the move"
        );
        core.undo();
        let contents: Vec<&str> = core
            .project
            .texts
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(contents, ["A", "B", "C", "D"], "one undo restores the order");
    }

    #[test]
    fn move_text_instance_to_the_back_keeps_the_others_in_order() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "A", 20.0, 1);
        push_instance(&mut core, "B", 20.0, 1);
        push_instance(&mut core, "C", 20.0, 1);
        let depth_before = core.undo_depth();
        assert!(move_text_instance(&mut core, 2, false));
        assert_eq!(core.status, "Text 1 moved to the back.");
        let contents: Vec<&str> = core
            .project
            .texts
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(
            contents,
            ["C", "A", "B"],
            "C goes first (drawn underneath); A and B keep their relative order"
        );
        assert_eq!(
            core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the move"
        );
    }

    #[test]
    fn move_text_instance_already_at_an_end_snapshots_nothing() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "A", 20.0, 1);
        push_instance(&mut core, "B", 20.0, 1);
        let depth_before = core.undo_depth();
        assert!(!move_text_instance(&mut core, 1, true), "B is already at the front");
        assert_eq!(core.status, "Already at the front.");
        assert!(!move_text_instance(&mut core, 0, false), "A is already at the back");
        assert_eq!(core.status, "Already at the back.");
        assert_eq!(
            core.undo_depth(),
            depth_before,
            "a press that moves nothing snapshots nothing"
        );
        let contents: Vec<&str> = core
            .project
            .texts
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(contents, ["A", "B"], "the order is untouched");
    }

    #[test]
    fn move_text_instance_out_of_range_is_status_only_noop() {
        let mut core = EditorCore::new();
        push_instance(&mut core, "A", 20.0, 1);
        let depth_before = core.undo_depth();
        assert!(!move_text_instance(&mut core, 7, true));
        assert_eq!(core.status, "No such text.");
        assert!(!move_text_instance(&mut core, 7, false));
        assert_eq!(core.status, "No such text.");
        assert_eq!(core.project.texts.len(), 1, "nothing moved or vanished");
        assert_eq!(core.undo_depth(), depth_before, "nothing was snapshotted");
    }

    #[test]
    fn move_loaded_front_then_back_follows_the_instance() {
        let mut s = crate::init();
        push_instance(&mut s.core, "A", 24.0, 1);
        push_instance(&mut s.core, "B", 24.0, 1);
        push_instance(&mut s.core, "C", 24.0, 1);
        let order = |s: &State| -> Vec<&str> {
            s.core
                .project
                .texts
                .iter()
                .map(|t| t.content.as_str())
                .collect()
        };
        // Nothing loaded: the press refuses through
        // loaded_index and reorders nothing.
        let depth_before = s.core.undo_depth();
        move_loaded(&mut s, true);
        assert_eq!(s.core.status, "Load a text first.");
        assert_eq!(order(&s), ["A", "B", "C"]);
        assert_eq!(s.core.undo_depth(), depth_before, "nothing was snapshotted");

        // Load the middle one and bring it to the front:
        // it lands at the vec's end and `loaded` follows.
        s.text_tool.loaded = Some(1);
        move_loaded(&mut s, true);
        assert_eq!(s.core.status, "Text 3 moved to the front.");
        assert_eq!(order(&s), ["A", "C", "B"], "B is drawn last, on top");
        assert_eq!(s.text_tool.loaded, Some(2), "loaded follows B to its new index");
        assert_eq!(
            s.core.undo_depth(),
            depth_before + 1,
            "exactly one snapshot for the move"
        );

        // Already at the front: no snapshot, no reorder,
        // and `loaded` stays where it was.
        move_loaded(&mut s, true);
        assert_eq!(s.core.status, "Already at the front.");
        assert_eq!(order(&s), ["A", "C", "B"]);
        assert_eq!(s.text_tool.loaded, Some(2));
        assert_eq!(s.core.undo_depth(), depth_before + 1, "nothing more was snapshotted");

        // Send it to the back from there: index 0, and
        // `loaded` follows again.
        move_loaded(&mut s, false);
        assert_eq!(s.core.status, "Text 1 moved to the back.");
        assert_eq!(order(&s), ["B", "A", "C"], "B is drawn first, underneath");
        assert_eq!(s.text_tool.loaded, Some(0), "loaded follows B to the back");
        assert_eq!(
            s.core.undo_depth(),
            depth_before + 2,
            "one more snapshot for the second move"
        );

        // Already at the back: no snapshot.
        move_loaded(&mut s, false);
        assert_eq!(s.core.status, "Already at the back.");
        assert_eq!(order(&s), ["B", "A", "C"]);
        assert_eq!(s.text_tool.loaded, Some(0));
        assert_eq!(s.core.undo_depth(), depth_before + 2, "nothing more was snapshotted");
    }
}
