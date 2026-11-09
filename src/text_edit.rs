//! Text authoring for text segments: a small editing state machine
//! the app drives from keyboard events.
//!
//! `SegmentMedia::Text` segments render (see `media.rs`), but until
//! now there was no way to type the text. [`TextEdit`] is a
//! UI-agnostic editing session over one figure node's text: begin a
//! session, feed it printable characters and cursor movements, then
//! [`TextEdit::commit`] to write the result back into the project.
//!
//! Cancellation needs no API: **`commit` is the only function here
//! that mutates a [`Project`]**, so dropping a `TextEdit` without
//! committing cancels the edit and the project is untouched.
//!
//! All positions are **char indices, never byte indices**, so
//! multi-byte characters (e.g. `λ`, `é`) insert, delete and are
//! stepped over atomically.
//!
//! The Wave 12 (C7) section near the end of this file collects
//! the shared emitter helpers built on top of this model —
//! [`xml_escape`], [`normalise_text_content`] and the raster
//! text metrics ([`raster_text_metrics`]) — so the SVG writer
//! (and any future emitter) places and escapes text exactly
//! the way the rasterizer does.

use crate::editor::EditorCore;
use crate::model::{InstanceKey, Project, SegmentMedia, TextInstance};
use serde_json::{json, Value};

/// Text size used on commit when the node had no previous text size.
pub const DEFAULT_TEXT_SIZE: f32 = 28.0;

/// One in-progress text edit, bound to a figure node by index.
///
/// The indices are re-validated at commit time: if the figure or
/// node has gone away since [`TextEdit::begin`], commit fails
/// cleanly instead of panicking.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    pub figure: usize,
    pub node: usize,
    pub buffer: String,
    /// Cursor position as a char index in `0..=buffer.chars().count()`.
    pub cursor: usize,
}

impl TextEdit {
    /// Start editing the text of `project.figures[figure].def.nodes[node]`.
    ///
    /// If the node already carries `SegmentMedia::Text`, the buffer
    /// starts from its content (cursor at the end); otherwise the
    /// buffer starts empty. Returns `None` for out-of-range indices.
    pub fn begin(project: &Project, figure: usize, node: usize) -> Option<TextEdit> {
        let def_node = project.figures.get(figure)?.def.nodes.get(node)?;
        let buffer = match &def_node.media {
            Some(SegmentMedia::Text { content, .. }) => content.clone(),
            _ => String::new(),
        };
        let cursor = buffer.chars().count();
        Some(TextEdit {
            figure,
            node,
            buffer,
            cursor,
        })
    }

    /// Number of chars in the buffer (the maximum cursor position).
    pub fn char_len(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte index of the char at `char_idx` (clamped to the buffer end).
    fn byte_of(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.buffer.len())
    }

    /// Insert a printable character at the cursor and advance past it.
    /// Control characters (newline, tab, …) are ignored; space and
    /// any other printable char — including multi-byte ones — work.
    pub fn push_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let at = self.byte_of(self.cursor);
        self.buffer.insert(at, c);
        self.cursor += 1;
    }

    /// Delete the char before the cursor (no-op at the start).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let at = self.byte_of(self.cursor);
        // `at` is a char boundary by construction (see byte_of).
        self.buffer.remove(at);
    }

    /// Delete the char at the cursor (no-op at the end).
    pub fn delete(&mut self) {
        if self.cursor >= self.char_len() {
            return;
        }
        let at = self.byte_of(self.cursor);
        self.buffer.remove(at);
    }

    /// Move the cursor by `delta` chars (negative = left), clamped
    /// to `0..=char_len`.
    pub fn move_cursor(&mut self, delta: i32) {
        let len = self.char_len() as i64;
        let next = self.cursor as i64 + delta as i64;
        self.cursor = next.clamp(0, len) as usize;
    }

    /// Move the cursor to the start of the buffer.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end of the buffer.
    pub fn end(&mut self) {
        self.cursor = self.char_len();
    }

    /// The buffer with a `|` cursor marker inserted at the cursor,
    /// for display in a status line while editing.
    pub fn display(&self) -> String {
        let mut out = String::with_capacity(self.buffer.len() + 1);
        for (i, c) in self.buffer.chars().enumerate() {
            if i == self.cursor {
                out.push('|');
            }
            out.push(c);
        }
        if self.cursor >= self.char_len() {
            out.push('|');
        }
        out
    }

    /// Write the edit back onto the node it began on.
    ///
    /// A non-empty buffer becomes `SegmentMedia::Text` with the
    /// buffer as content, preserving the node's existing text size
    /// when it already had text media (otherwise
    /// [`DEFAULT_TEXT_SIZE`]). An **empty buffer removes the node's
    /// media instead** — clearing the text is how a text segment is
    /// deleted. (Consequence: beginning an edit on a node that holds
    /// an image and committing without typing clears that image.)
    ///
    /// Returns `false` — and changes nothing — if the figure or node
    /// index has gone stale since `begin`.
    pub fn commit(&self, project: &mut Project) -> bool {
        let Some(node) = project
            .figures
            .get_mut(self.figure)
            .and_then(|f| f.def.nodes.get_mut(self.node))
        else {
            return false;
        };
        if self.buffer.is_empty() {
            node.media = None;
        } else {
            let size = match &node.media {
                Some(SegmentMedia::Text { size, .. }) => *size,
                _ => DEFAULT_TEXT_SIZE,
            };
            node.media = Some(SegmentMedia::Text {
                content: self.buffer.clone(),
                size,
            });
        }
        true
    }
}

// ---------------------------------------------------------------------
// Wave 11 (B10): text instance + segment-text ops and MCP handlers.
//
// The text tool panel (`rune_sec/text_tool.rs`, bin crate) mutates
// `Project.texts` and segment text through panel-local free
// functions, because no `EditorCore` op exists for text instances.
// The MCP registry had NO text tools at all, so the ops are
// implemented here — in the library, where MCP handlers live —
// with the panel's exact semantics (same guards, same statuses,
// same snapshot discipline: bounds check first, exactly one
// `snapshot()` per applied change, status-only refusals that
// snapshot nothing). Panel sources are cited per op; where a
// return type differs from the panel's (noted per op) it is a
// reporting superset only — the mutation is identical.
// ---------------------------------------------------------------------

/// Smallest text glyph height, in stage units (the text
/// panel's `SIZE_MIN`, `text_tool.rs:301`).
pub const TEXT_SIZE_MIN: f32 = 4.0;

/// Largest text glyph height, in stage units (the text
/// panel's `SIZE_MAX`, `text_tool.rs:303`).
pub const TEXT_SIZE_MAX: f32 = 160.0;

/// How far a duplicated text instance shifts on both axes
/// (the text panel's `DUP_OFFSET`, `text_tool.rs:310`).
pub const TEXT_DUP_OFFSET: f32 = 12.0;

/// Shift transform `keys` by `(dx, dy)` stage units; frame,
/// scale and angle of every key are untouched (the panel's
/// `offset_keys`, `text_tool.rs:501`). Pure.
pub fn offset_keys(keys: &mut [InstanceKey], dx: f32, dy: f32) {
    for key in keys {
        key.x += dx;
        key.y += dy;
    }
}

/// Add `delta_rad` to the angle of every transform key (the
/// panel's `rotate_keys`, `text_tool.rs:521`). Pure.
pub fn rotate_keys(keys: &mut [InstanceKey], delta_rad: f32) {
    for key in keys {
        key.angle += delta_rad;
    }
}

/// The clone a duplicate inserts: `inst` verbatim, then
/// shifted by [`TEXT_DUP_OFFSET`] on both axes (the panel's
/// `duplicated_instance`, `text_tool.rs:534`). Pure.
pub fn duplicated_instance(inst: &TextInstance) -> TextInstance {
    let mut copy = inst.clone();
    offset_keys(&mut copy.keys, TEXT_DUP_OFFSET, TEXT_DUP_OFFSET);
    copy
}

/// Where the instance at `index` in a vec of `len` instances
/// lands when sent to an end of the draw order: texts paint
/// in vec order, so the front of the stack is the vec's END
/// and `to_front` targets `len - 1`; the back is index 0.
/// `None` when there is nowhere to move to (the panel's
/// `moved_index`, `text_tool.rs:718`). Pure.
pub fn moved_index(len: usize, index: usize, to_front: bool) -> Option<usize> {
    if index >= len {
        return None;
    }
    let target = if to_front { len - 1 } else { 0 };
    (index != target).then_some(target)
}

/// Create a free-standing text instance with a single
/// transform key at `(frame, x, y)` (scale 1, angle 0),
/// under exactly one `snapshot()`.
///
/// The panel cannot create instances — today they arrive
/// via Stick Nodes import (see the `text_tool.rs` module
/// docs) — so this op is the MCP side completing the
/// surface the import path proves the model supports; it
/// is flagged as such in the tool's registry description.
/// `size` is clamped to `TEXT_SIZE_MIN..=TEXT_SIZE_MAX`
/// here so no caller can store a size the panel's steppers
/// would refuse to hold. Returns the new instance's index.
pub fn add_text_instance(
    core: &mut EditorCore,
    content: &str,
    size: f32,
    color: u32,
    frame: f32,
    x: f32,
    y: f32,
) -> usize {
    core.snapshot();
    core.project.texts.push(TextInstance {
        content: content.to_string(),
        size: size.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX),
        color,
        keys: vec![InstanceKey { frame, x, y, scale: 1.0, angle: 0.0 }],
    });
    let index = core.project.texts.len() - 1;
    core.status = format!("Text {} added.", index + 1);
    index
}

/// Write `content` into the text instance at `index` (the
/// panel's `update_text_instance`, `text_tool.rs:631`):
/// only the content changes — keys, size and colour are
/// untouched. One `snapshot()` when the content actually
/// changes. Status-only refusals: a stale index ("No such
/// text."), empty content ("Type some text first." — an
/// empty write must never blank a text), unchanged content
/// ("Text unchanged."). Returns whether the content changed.
pub fn update_text_instance(core: &mut EditorCore, index: usize, content: &str) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    if content.is_empty() {
        core.status = "Type some text first.".into();
        return false;
    }
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

/// Delete the text instance at `index` under exactly one
/// `snapshot()` (the panel's `delete_text_instance`,
/// `text_tool.rs:488`; the panel's version returns `()`,
/// this one also reports whether it deleted). A stale
/// index is a status-only no-op.
pub fn delete_text_instance(core: &mut EditorCore, index: usize) -> bool {
    if index >= core.project.texts.len() {
        core.status = "No such text.".into();
        return false;
    }
    core.snapshot();
    core.project.texts.remove(index);
    core.status = "Text deleted.".into();
    true
}

/// Duplicate the text instance at `index`: insert
/// [`duplicated_instance`]'s clone directly after the
/// original, under exactly one `snapshot()` (the panel's
/// `duplicate_text_instance`, `text_tool.rs:547`). Returns
/// the copy's index; a stale index is a status-only refusal.
pub fn duplicate_text_instance(core: &mut EditorCore, index: usize) -> Option<usize> {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return None;
    };
    let copy = duplicated_instance(inst);
    core.snapshot();
    core.project.texts.insert(index + 1, copy);
    core.status = "Text duplicated.".into();
    Some(index + 1)
}

/// Nudge the text instance at `index` by `(dx, dy)` stage
/// units: every transform key shifts by the same delta (the
/// panel's `nudge_text_instance`, `text_tool.rs:568`). One
/// `snapshot()` when it moves. Status-only refusals: a
/// stale index, or an instance with no keys — it has no
/// position to move. Returns whether the instance moved.
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
        offset_keys(&mut inst.keys, dx, dy);
    }
    core.status = "Text nudged.".into();
    true
}

/// Set the text instance at `index`'s glyph height to
/// `size`, clamped to `TEXT_SIZE_MIN..=TEXT_SIZE_MAX` — the
/// range the panel's size steppers and `load_instance`
/// clamp to (`text_tool.rs:595, 963`). The panel reaches
/// sizes by ±4 steps; this op is the absolute form of the
/// same write, in the MCP idiom (cf. `set_node_opacity` vs
/// the panel's opacity stepper). One `snapshot()` when the
/// clamped size differs from the stored one; at the stored
/// size it reports the size and snapshots nothing (the
/// panel stepper's at-clamp discipline). Returns whether
/// the size changed.
pub fn set_text_instance_size(core: &mut EditorCore, index: usize, size: f32) -> bool {
    let Some(inst) = core.project.texts.get(index) else {
        core.status = "No such text.".into();
        return false;
    };
    let next = size.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX);
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

/// Rotate the text instance at `index` by `delta_rad`
/// radians: every transform key's angle gains the delta
/// (the panel's `rotate_text_instance`, `text_tool.rs:670`).
/// One `snapshot()`. An instance with no keys still counts
/// as rotated — the panel's deliberate choice, mirrored.
/// Returns whether the instance was rotated.
pub fn rotate_text_instance(core: &mut EditorCore, index: usize, delta_rad: f32) -> bool {
    if core.project.texts.get(index).is_none() {
        core.status = "No such text.".into();
        return false;
    }
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        rotate_keys(&mut inst.keys, delta_rad);
    }
    core.status = format!("Text {} rotated.", index + 1);
    true
}

/// Set the text instance at `index`'s colour (the panel's
/// `set_text_color`, `text_tool.rs:695`): only the colour
/// changes. Exactly one `snapshot()`; a stale index
/// refuses status-only. Returns whether the colour was set.
pub fn set_text_instance_color(core: &mut EditorCore, index: usize, color: u32) -> bool {
    if core.project.texts.get(index).is_none() {
        core.status = "No such text.".into();
        return false;
    }
    core.snapshot();
    if let Some(inst) = core.project.texts.get_mut(index) {
        inst.color = color;
    }
    core.status = format!("Text {} colour set to #{color:06X}.", index + 1);
    true
}

/// Move the text instance at `index` to an end of the draw
/// order (the panel's `move_text_instance`,
/// `text_tool.rs:746`): `to_front` sends it to the end of
/// `Project.texts` (drawn last = on top), otherwise to
/// index 0; every other instance keeps its relative order
/// and the instance rides along verbatim. One `snapshot()`
/// when it moves. Status-only refusals: a stale index, or
/// the instance already at the target end. Returns whether
/// the instance moved.
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

/// Put `content` on the segment of figure `fi`, node `ni`,
/// at glyph height `size` when given — the panel's
/// `add_text_to_core` (`text_tool.rs:989`) addressed by
/// index instead of by selection: an existing text on the
/// node is replaced, never appended to, through the
/// [`TextEdit`] session (clear, type, commit). Status-only
/// refusals: empty content, content with no printable
/// characters (the session drops control characters, so
/// committing it would delete the node's media — the
/// panel's own guard), a stale node ("No such segment.").
/// One `snapshot()` covers the whole write (taken before
/// commit, exactly as the panel takes it); a given `size`
/// is clamped to `TEXT_SIZE_MIN..=TEXT_SIZE_MAX` and
/// written onto the fresh media under that same snapshot,
/// because `commit` has no size parameter. Without a
/// `size`, commit's own rule stands: an existing text size
/// is preserved, a fresh text gets [`DEFAULT_TEXT_SIZE`].
/// Returns whether the text was set.
pub fn set_segment_text(
    core: &mut EditorCore,
    fi: usize,
    ni: usize,
    content: &str,
    size: Option<f32>,
) -> bool {
    if content.is_empty() {
        core.status = "Type some text first.".into();
        return false;
    }
    let Some(mut session) = TextEdit::begin(&core.project, fi, ni) else {
        core.status = "No such segment.".into();
        return false;
    };
    while session.char_len() > 0 {
        session.backspace();
    }
    for c in content.chars() {
        session.push_char(c);
    }
    if session.char_len() == 0 {
        core.status = "Type some text first.".into();
        return false;
    }
    core.snapshot();
    if !session.commit(&mut core.project) {
        core.status = "Text target no longer exists.".into();
        return false;
    }
    if let Some(size) = size {
        if let Some(def) = core
            .project
            .figures
            .get_mut(fi)
            .and_then(|f| f.def.nodes.get_mut(ni))
        {
            if let Some(SegmentMedia::Text { size: sz, .. }) = &mut def.media {
                *sz = size.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX);
            }
        }
    }
    core.status = "Text set.".into();
    true
}

/// Set the glyph height of the text already on figure
/// `fi`, node `ni`'s segment, clamped to
/// `TEXT_SIZE_MIN..=TEXT_SIZE_MAX` — the write the panel's
/// size stepper performs live on an existing text segment
/// (`text_tool.rs` `step_from_button`). One `snapshot()`
/// when the clamped size differs from the stored one; a
/// node carrying no text (or a stale index) refuses
/// status-only. Returns whether the size changed.
pub fn set_segment_text_size(core: &mut EditorCore, fi: usize, ni: usize, size: f32) -> bool {
    let current = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
        .map(|n| &n.media)
    {
        Some(Some(SegmentMedia::Text { size, .. })) => *size,
        _ => {
            core.status = "No text on this segment.".into();
            return false;
        }
    };
    let next = size.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX);
    if next == current {
        core.status = format!("Text size {next:.0}.");
        return false;
    }
    core.snapshot();
    if let Some(def) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        if let Some(SegmentMedia::Text { size: sz, .. }) = &mut def.media {
            *sz = next;
        }
    }
    core.status = format!("Text size {next:.0}.");
    true
}

// ---------------------------------------------------------------------
// Wave 12 (C7): shared text helpers for emitters.
//
// Everything an exporter needs to write text that agrees with
// the rasterizer, collected in one place:
//
// - [`xml_escape`] — XML-safe text content (see its docs for
//   the control-character rule and how it differs from the
//   older five-character helper pinned in `svg.rs`).
// - [`normalise_text_content`] — the content normalisation
//   the editing session applies, which emitters must respect
//   (and the storage paths that do NOT apply it).
// - [`raster_text_metrics`] / [`glyph_cell_centre`] /
//   [`rotate_text_offset`] — the raster's placement math,
//   reproduced exactly (see the metrics docs: these are the
//   raster's fixed-grid conventions, NOT real font metrics).
// - [`text_color_hex`] — the stored colour as `#RRGGBB`.
//
// Colour and rotation conventions (from `media.rs`, the
// raster path `export.rs` composites through
// `media::composite_media` at `export.rs:666-690`): colours
// are stored as `u32` `0xRRGGBB` with NO alpha — the raster
// paints text fully opaque (`media.rs:328-333`). A segment
// text's colour is `node.color.unwrap_or(fig.color)`
// (`media.rs:143`); a text instance carries its own `color`
// (`media.rs:193`). Angles are DEGREES in the y-down stage
// space, positive = clockwise on screen; a segment text runs
// along `atan2(dy, dx)` of its segment (`media.rs:140`), a
// text instance uses its key's `angle` verbatim
// (`media.rs:190`). Because SVG's coordinate space is also
// y-down, an SVG `rotate(angle cx cy)` transform turns the
// same way as the raster's rotation — no sign flip.
// ---------------------------------------------------------------------

/// Glyph grid width, in glyph pixels: the raster's built-in
/// font is a 3×5 bitmap (`media.rs:364`, glyph loops at
/// `media.rs:336-341`; the stage mirrors it at
/// `rune_sec/stage_text.rs:139`).
pub const GLYPH_COLS: usize = 3;

/// Glyph grid height, in glyph pixels — also the divisor
/// that turns a text `size` (glyph height, stage units) into
/// the raster's cell size (`media.rs:321`;
/// `rune_sec/stage_text.rs:140,189`).
pub const GLYPH_ROWS: usize = 5;

/// Horizontal advance per character, in cells: 3 glyph
/// columns + 1 gap column, the same for EVERY character
/// (`media.rs:322`; `rune_sec/stage_text.rs:143`).
pub const GLYPH_ADVANCE_CELLS: f32 = 4.0;

/// Escape `text` for interpolation into XML text content or
/// a quoted attribute value (SVG emission is the first
/// consumer).
///
/// The five XML-significant characters escape as entities,
/// `&` first in effect (it introduces every other entity, so
/// a pre-written `&lt;` double-escapes to `&amp;lt;`):
/// `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`,
/// `"` → `&quot;`, `'` → `&apos;`.
///
/// **Control characters are replaced by a single space
/// each** (never dropped, never passed through raw):
///
/// - XML 1.0 forbids most C0 controls outright — even as
///   character references — so one raw control byte would
///   make the whole SVG file unparseable. Passing them
///   through is not an option. (This is where this helper
///   deliberately differs from `svg::xml_escape`, which
///   escapes the same five characters but passes controls
///   through verbatim; that helper was pinned before any
///   user text was emitted. Emitters of user text should
///   use THIS one.)
/// - Tab / LF / CR are legal XML, but they are controls
///   too and get the same treatment for uniformity: stored
///   text is single-line by model rule (see
///   [`normalise_text_content`]), so a newline in stored
///   content is import residue, not a line break — and SVG
///   `<text>` would not break a line for it anyway.
/// - A space specifically — rather than dropping the char
///   or substituting U+FFFD — because the raster gives a
///   control character a BLANK glyph of full advance
///   (`media.rs:431-434` paints nothing, while
///   `media.rs:322` counts it in the width): a space
///   reproduces both the raster's blank look and its exact
///   width, and preserves the character count, so
///   [`raster_text_metrics`] on the escaped string still
///   measures the string that was stored. U+FFFD would
///   paint a visible replacement glyph the raster never
///   shows; dropping would shift every later glyph left.
///
/// "Control" is [`char::is_control`] (Unicode category Cc:
/// U+0000–U+001F and U+007F–U+009F) — the same predicate
/// the editing session drops on input ([`TextEdit::push_char`]).
/// All other characters, including multi-byte ones, pass
/// through unchanged.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\'' => out += "&apos;",
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// The content normalisation the model applies on edit —
/// what an emitter can assume about session-authored text,
/// and what it must defensively handle everywhere else.
///
/// The editing session ([`TextEdit::push_char`]) **drops
/// every control character** ([`char::is_control`]: newline,
/// tab, DEL, …) and keeps everything else verbatim,
/// including spaces and multi-byte characters. Stored text
/// is therefore single-line: there are NO line breaks in
/// this model, and an emitter must never split content into
/// multiple lines. This fn reproduces that filter exactly,
/// for emitters (or importers) that want session semantics
/// on a raw string.
///
/// The empty-text rule: an EMPTY result means "no text".
/// Committing an empty buffer REMOVES the node's media
/// ([`TextEdit::commit`]), and the text ops refuse empty
/// content outright ("Type some text first." —
/// [`update_text_instance`], [`txt_check_content`]). An
/// emitter should skip empty content rather than emit an
/// empty element.
///
/// Paths that do NOT normalise — an emitter WILL meet
/// control characters in stored content from these, which
/// is why [`xml_escape`] handles them instead of assuming
/// them away:
///
/// - [`add_text_instance`] stores its `content` argument
///   verbatim (the handlers only require it to be non-empty
///   with at least one printable character).
/// - [`update_text_instance`] likewise stores verbatim.
/// - Text instances arriving via Stick Nodes / .piv import
///   are stored as decoded, never passed through a session.
pub fn normalise_text_content(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_control()).collect()
}

/// The raster's text placement metrics for one string at one
/// glyph height, reproduced EXACTLY from
/// `media::composite_text` (`media.rs:307-362`) — the
/// rasterizer path `export.rs` composites text through.
///
/// **These are the raster's conventions, not real font
/// metrics.** The raster draws a built-in 3×5 bitmap font:
/// every character — wide, narrow, multi-byte, even a
/// control character — occupies exactly
/// [`GLYPH_ADVANCE_CELLS`] cells of advance, lowercase folds
/// to uppercase, and any character outside the small bitmap
/// charset renders as `?` (`media.rs:364-367,429-435`). An
/// SVG `<text>` in a real font will NOT match these metrics
/// glyph-for-glyph; what they give an emitter is the same
/// anchor geometry the raster uses — total width, height
/// and centre — so a `<text>` placed with `text-anchor=
/// "middle"` at the item position, at font-size `size`,
/// sits where the raster's label sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterTextMetrics {
    /// Characters in the content, counted with
    /// `chars().count()` — the raster collects
    /// `content.chars()` and measures that (`media.rs:317`).
    pub char_count: usize,
    /// Stage units per glyph pixel: `size / 5`
    /// (`media.rs:321`).
    pub cell: f32,
    /// Total advance width in stage units:
    /// `(char_count * 4 - 1) * cell` (`media.rs:322`). Zero
    /// for empty content (the formula's `- 1` must not make
    /// it negative — the raster draws nothing at all for an
    /// empty string, `media.rs:318`).
    pub width: f32,
    /// Glyph height in stage units: exactly `size` (5 cells
    /// × `size / 5`). Zero for empty content, which the
    /// raster does not draw.
    pub height: f32,
    /// X of the first glyph cell's left edge, in CELL units,
    /// relative to the text centre: `-(char_count * 4 - 1)
    /// / 2` (`media.rs:323` — the raster centres the string
    /// on its position). Multiply by [`Self::cell`] for
    /// stage units; equals `-width / 2` there.
    pub start_cells: f32,
}

/// Measure `content` at glyph height `size` with the
/// raster's formula (see [`RasterTextMetrics`]). A
/// non-positive `size` draws nothing in the raster
/// (`media.rs:318`); the metrics are still returned by
/// formula, so callers that care should check `size > 0.0`
/// exactly as the raster does.
pub fn raster_text_metrics(content: &str, size: f32) -> RasterTextMetrics {
    let char_count = content.chars().count();
    let cell = size / GLYPH_ROWS as f32;
    if char_count == 0 {
        return RasterTextMetrics {
            char_count,
            cell,
            width: 0.0,
            height: 0.0,
            start_cells: 0.0,
        };
    }
    let total_cells = char_count as f32 * GLYPH_ADVANCE_CELLS - 1.0;
    RasterTextMetrics {
        char_count,
        cell,
        width: total_cells * cell,
        height: size,
        start_cells: -total_cells / 2.0,
    }
}

/// Just the raster's advance width for `content` at glyph
/// height `size` — [`raster_text_metrics`]'s `width` field,
/// for callers that need nothing else.
pub fn raster_text_width(content: &str, size: f32) -> f32 {
    raster_text_metrics(content, size).width
}

/// The centre of one glyph cell, in the text's LOCAL frame
/// (stage units, relative to the text centre, before
/// rotation) — `media.rs:341-342` verbatim:
///
/// ```text
/// lx = (start_cells + char_index * 4 + col + 0.5) * cell
/// ly = (row - 2 + 0.5) * cell
/// ```
///
/// Note the vertical asymmetry, reproduced deliberately:
/// row centres run from `-1.5 * cell` (row 0) to
/// `+2.5 * cell` (row 4), so the glyph box spans
/// `-2 * cell..3 * cell` — half a cell BELOW centre. That
/// is the raster's (and the stage's) shared convention, not
/// a bug to fix in an emitter. `col` is `0..GLYPH_COLS`,
/// `row` is `0..GLYPH_ROWS`; out-of-range indices are not
/// checked, exactly as the raster's loops never produce
/// them.
pub fn glyph_cell_centre(
    metrics: &RasterTextMetrics,
    char_index: usize,
    col: usize,
    row: usize,
) -> [f32; 2] {
    let lx = (metrics.start_cells
        + char_index as f32 * GLYPH_ADVANCE_CELLS
        + col as f32
        + 0.5)
        * metrics.cell;
    // (GLYPH_ROWS - 1) / 2 == 2.0, the raster's literal.
    let ly = (row as f32 - (GLYPH_ROWS as f32 - 1.0) / 2.0 + 0.5) * metrics.cell;
    [lx, ly]
}

/// Rotate a local text offset by `angle_deg` into stage
/// space — `media.rs:324-325,346-349` verbatim: the angle
/// is degrees, converted with `to_radians()`, applied as
/// `[lx·cos − ly·sin, lx·sin + ly·cos]` in the y-down stage
/// space (positive angles turn clockwise on screen). Add
/// the text item's position to the result for the stage-
/// space point. For SVG, a `rotate(angle cx cy)` transform
/// about the item position performs this same rotation —
/// both spaces are y-down, so the sign already agrees.
pub fn rotate_text_offset(local: [f32; 2], angle_deg: f32) -> [f32; 2] {
    let rad = angle_deg.to_radians();
    let (cs, sn) = (rad.cos(), rad.sin());
    [
        local[0] * cs - local[1] * sn,
        local[0] * sn + local[1] * cs,
    ]
}

/// A stored text colour (`0xRRGGBB`, alpha never stored —
/// the raster paints fully opaque, `media.rs:328-333`) as
/// an SVG/CSS `#RRGGBB` string: the value masked to 24 bits
/// and formatted uppercase, byte-for-byte the output of
/// `svg.rs`'s private `hex()` (`svg.rs:33-35`), shared here
/// so text emission formats colours identically.
pub fn text_color_hex(color: u32) -> String {
    format!("#{:06X}", color & 0xFF_FFFF)
}

// ---------------------------------------------------------------------
// Local argument helpers for the handlers in this file (the
// registry's own helpers are private to `mcp_tools.rs`). Every
// one returns Err(String) on bad input, in the registry's
// wording, so no handler can panic on malformed arguments.
//
// Handler error convention (the registry's): Err for a
// malformed request or an index that addresses nothing;
// Ok with "changed": false for a request the panel's own
// guards turn into a no-op (unchanged content, already at a
// z-order end, a keyless nudge) — the panel status wording
// rides along in "status" either way.
// ---------------------------------------------------------------------

fn txt_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn txt_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn txt_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match txt_get(args, key) {
        Some(v) => txt_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn txt_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match txt_get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("argument '{key}' must be a number")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn txt_opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match txt_get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| Some(f as f32))
            .ok_or_else(|| format!("argument '{key}' must be a number")),
        None => Ok(None),
    }
}

fn txt_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match txt_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn txt_req_bool(args: &Value, key: &str) -> Result<bool, String> {
    match txt_get(args, key) {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Colours travel as `0xRRGGBB` integers (registry convention).
fn txt_opt_color(args: &Value, key: &str) -> Result<Option<u32>, String> {
    match txt_get(args, key) {
        Some(v) => {
            let n = txt_as_usize(v, key)?;
            u32::try_from(n)
                .map(Some)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Ok(None),
    }
}

fn txt_req_color(args: &Value, key: &str) -> Result<u32, String> {
    txt_opt_color(args, key)?.ok_or_else(|| format!("missing required argument '{key}'"))
}

fn txt_check_node(core: &EditorCore, fi: usize, ni: usize) -> Result<(), String> {
    let figures = core.project.figures.len();
    if fi >= figures {
        return Err(format!(
            "figure index {fi} out of range (project has {figures} figure(s))"
        ));
    }
    let n = core.project.figures[fi].def.nodes.len();
    if ni >= n {
        return Err(format!(
            "node index {ni} out of range (figure {fi} has {n} node(s))"
        ));
    }
    Ok(())
}

fn txt_check_text(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.texts.len();
    if index >= n {
        return Err(format!(
            "text index {index} out of range (project has {n} text(s))"
        ));
    }
    Ok(())
}

/// Refuse content the text session could not store: empty,
/// or only control characters (the session drops those, so
/// committing would erase instead of write — the panel's
/// `add_text_to_core` guard, as an argument error).
fn txt_check_content(content: &str) -> Result<(), String> {
    if content.is_empty() {
        return Err(
            "argument 'content' must not be empty (the text panel refuses to blank a text: \"Type some text first.\")"
                .into(),
        );
    }
    if !content.chars().any(|c| !c.is_control()) {
        return Err(
            "argument 'content' has no printable characters (the text session drops control characters, so nothing would be stored)"
                .into(),
        );
    }
    Ok(())
}

/// An optional text size argument, validated: when present
/// it must be a positive number (the ops clamp it into
/// `TEXT_SIZE_MIN..=TEXT_SIZE_MAX`).
fn txt_opt_size(args: &Value) -> Result<Option<f32>, String> {
    match txt_opt_f32(args, "size")? {
        Some(s) => {
            if s <= 0.0 {
                return Err("argument 'size' must be positive".into());
            }
            Ok(Some(s))
        }
        None => Ok(None),
    }
}

/// One text instance as a JSON object (the `list_texts`
/// shape; reused by the instance handlers' responses where
/// useful).
fn txt_instance_json(index: usize, inst: &TextInstance) -> Value {
    json!({
        "index": index,
        "content": inst.content,
        "size": inst.size,
        "color": inst.color,
        "keys": inst.keys.iter().map(|k| json!({
            "frame": k.frame, "x": k.x, "y": k.y,
            "scale": k.scale, "angle": k.angle,
        })).collect::<Vec<_>>(),
    })
}

/// MCP handler: list every text in the project — the text
/// panel's two lists in one read. `segment_texts` covers
/// every figure's nodes (the panel lists the selected
/// figure's; this read is a superset), `instances` is
/// `Project.texts` in draw order (index 0 paints first,
/// the last index is on top).
pub fn tool_list_texts(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let mut segment_texts = Vec::new();
    for (fi, fig) in core.project.figures.iter().enumerate() {
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            if let Some(SegmentMedia::Text { content, size }) = &node.media {
                segment_texts.push(json!({
                    "figure": fi,
                    "node": ni,
                    "content": content,
                    "size": size,
                }));
            }
        }
    }
    let instances: Vec<Value> = core
        .project
        .texts
        .iter()
        .enumerate()
        .map(|(i, inst)| txt_instance_json(i, inst))
        .collect();
    Ok(json!({
        "ok": true,
        "segment_texts": segment_texts,
        "instances": instances,
    }))
}

/// MCP handler: set (or replace) the text on one segment —
/// the panel's Add text, addressed by (figure, node). An
/// optional `size` is clamped to 4–160; without one, an
/// existing text size is preserved and a fresh text gets
/// the session default (28). The response reads the stored
/// content back — control characters in `content` (e.g.
/// newlines) are dropped by the text session, so the read-
/// back, not the request, is the truth about what stored.
pub fn tool_set_segment_text(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = txt_req_usize(args, "figure")?;
    let ni = txt_req_usize(args, "node")?;
    txt_check_node(core, fi, ni)?;
    let content = txt_req_str(args, "content")?;
    txt_check_content(content)?;
    let size = txt_opt_size(args)?;
    if !set_segment_text(core, fi, ni, content, size) {
        return Err(core.status.clone());
    }
    match &core.project.figures[fi].def.nodes[ni].media {
        Some(SegmentMedia::Text { content, size }) => Ok(json!({
            "ok": true,
            "figure": fi,
            "node": ni,
            "content": content,
            "size": size,
            "status": core.status,
        })),
        _ => Err("text was not stored on the segment".into()),
    }
}

/// MCP handler: set the glyph height of the text already on
/// one segment (clamped to 4–160). A segment carrying no
/// text is a no-op with "changed": false, the panel
/// stepper's refusal in response form.
pub fn tool_set_segment_text_size(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = txt_req_usize(args, "figure")?;
    let ni = txt_req_usize(args, "node")?;
    txt_check_node(core, fi, ni)?;
    let size = txt_req_f32(args, "size")?;
    if size <= 0.0 {
        return Err("argument 'size' must be positive".into());
    }
    let changed = set_segment_text_size(core, fi, ni, size);
    let stored = match &core.project.figures[fi].def.nodes[ni].media {
        Some(SegmentMedia::Text { size, .. }) => Some(*size),
        _ => None,
    };
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "changed": changed,
        "size": stored,
        "status": core.status,
    }))
}

/// MCP handler: remove the text from one segment (the
/// engine's `clear_node_media`, restricted to text: a
/// segment carrying an image is left untouched and reports
/// "changed": false — clearing media that is not text is
/// not a text tool's business). One undo step when a text
/// is actually cleared.
pub fn tool_clear_segment_text(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = txt_req_usize(args, "figure")?;
    let ni = txt_req_usize(args, "node")?;
    txt_check_node(core, fi, ni)?;
    match &core.project.figures[fi].def.nodes[ni].media {
        Some(SegmentMedia::Text { .. }) => {
            core.selected_figure = fi;
            core.selected_node = Some(ni);
            core.clear_node_media();
            Ok(json!({
                "ok": true,
                "figure": fi,
                "node": ni,
                "changed": true,
                "status": core.status,
            }))
        }
        Some(SegmentMedia::Image { .. }) => Ok(json!({
            "ok": true,
            "figure": fi,
            "node": ni,
            "changed": false,
            "status": "Segment carries an image, not text; the text tool leaves it untouched.",
        })),
        None => Ok(json!({
            "ok": true,
            "figure": fi,
            "node": ni,
            "changed": false,
            "status": "No text on this segment.",
        })),
    }
}

/// MCP handler: create a free-standing text instance with
/// one transform key. Defaults: size 28 (the text session
/// default), colour white, the key on the current frame at
/// the stage centre. The panel cannot create instances
/// (they arrive via import) — this tool completes that
/// side of the surface for MCP, and says so in its
/// registry description.
pub fn tool_add_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let content = txt_req_str(args, "content")?;
    txt_check_content(content)?;
    let size = txt_opt_size(args)?.unwrap_or(DEFAULT_TEXT_SIZE);
    let color = txt_opt_color(args, "color")?.unwrap_or(0xFF_FFFF);
    let frame = txt_opt_f32(args, "frame")?.unwrap_or(core.current as f32);
    if frame < 0.0 {
        return Err("argument 'frame' must not be negative".into());
    }
    let x = txt_opt_f32(args, "x")?.unwrap_or(core.project.stage[0] / 2.0);
    let y = txt_opt_f32(args, "y")?.unwrap_or(core.project.stage[1] / 2.0);
    let index = add_text_instance(core, content, size, color, frame, x, y);
    Ok(json!({
        "ok": true,
        "index": index,
        "instance": txt_instance_json(index, &core.project.texts[index]),
        "status": core.status,
    }))
}

/// MCP handler: rewrite one text instance's content (the
/// panel's Update text). Empty content is an argument
/// error — an update must never blank a text; unchanged
/// content is Ok with "changed": false.
pub fn tool_update_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let content = txt_req_str(args, "content")?;
    txt_check_content(content)?;
    let changed = update_text_instance(core, index, content);
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "content": core.project.texts[index].content,
        "status": core.status,
    }))
}

/// MCP handler: delete one text instance (one undo step).
pub fn tool_delete_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let changed = delete_text_instance(core, index);
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "texts": core.project.texts.len(),
        "status": core.status,
    }))
}

/// MCP handler: duplicate one text instance (the copy lands
/// directly after the original, shifted +12/+12 like the
/// panel's Duplicate).
pub fn tool_duplicate_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    match duplicate_text_instance(core, index) {
        Some(new_index) => Ok(json!({
            "ok": true,
            "index": index,
            "new_index": new_index,
            "status": core.status,
        })),
        None => Err(core.status.clone()),
    }
}

/// MCP handler: nudge one text instance by (dx, dy) stage
/// units — every transform key shifts (the panel's nudge
/// pad moves in ±4 steps; MCP takes the delta directly,
/// like the FX move tools). A keyless instance has no
/// position: Ok with "changed": false.
pub fn tool_nudge_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let dx = txt_req_f32(args, "dx")?;
    let dy = txt_req_f32(args, "dy")?;
    let changed = nudge_text_instance(core, index, dx, dy);
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "status": core.status,
    }))
}

/// MCP handler: set one text instance's glyph height
/// (clamped to 4–160). Ok with "changed": false when the
/// stored size already equals the clamped request.
pub fn tool_set_text_instance_size(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let size = txt_req_f32(args, "size")?;
    if size <= 0.0 {
        return Err("argument 'size' must be positive".into());
    }
    let changed = set_text_instance_size(core, index, size);
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "size": core.project.texts[index].size,
        "status": core.status,
    }))
}

/// MCP handler: rotate one text instance by `degrees` —
/// every transform key's angle gains the same amount (the
/// panel rotates in ±15° steps; MCP takes the angle
/// directly). A keyless instance still counts as rotated,
/// the panel's deliberate choice.
pub fn tool_rotate_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let degrees = txt_req_f32(args, "degrees")?;
    if !degrees.is_finite() {
        return Err("argument 'degrees' must be a number".into());
    }
    let changed = rotate_text_instance(core, index, degrees.to_radians());
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "degrees": degrees,
        "status": core.status,
    }))
}

/// MCP handler: set one text instance's colour (0xRRGGBB),
/// one undo step.
pub fn tool_set_text_instance_color(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let color = txt_req_color(args, "color")?;
    let changed = set_text_instance_color(core, index, color);
    Ok(json!({
        "ok": true,
        "index": index,
        "changed": changed,
        "color": core.project.texts[index].color,
        "status": core.status,
    }))
}

/// MCP handler: send one text instance to an end of the
/// draw order — `to_front: true` draws it on top of the
/// other texts, `false` puts it behind them (the panel's
/// z-order buttons). Already at that end: Ok with
/// "changed": false.
pub fn tool_move_text_instance(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = txt_req_usize(args, "index")?;
    txt_check_text(core, index)?;
    let to_front = txt_req_bool(args, "to_front")?;
    let changed = move_text_instance(core, index, to_front);
    let new_index = if changed {
        if to_front {
            core.project.texts.len() - 1
        } else {
            0
        }
    } else {
        index
    };
    Ok(json!({
        "ok": true,
        "index": index,
        "new_index": new_index,
        "changed": changed,
        "status": core.status,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Demo project whose figure 0, node 1 carries text "Hi λ" at size 40.
    fn project_with_text() -> Project {
        let mut p = Project::new_demo();
        p.figures[0].def.nodes[1].media = Some(SegmentMedia::Text {
            content: "Hi λ".to_string(),
            size: 40.0,
        });
        p
    }

    fn text_of(p: &Project, figure: usize, node: usize) -> Option<(String, f32)> {
        match &p.figures[figure].def.nodes[node].media {
            Some(SegmentMedia::Text { content, size }) => Some((content.clone(), *size)),
            _ => None,
        }
    }

    #[test]
    fn begin_from_existing_text() {
        let p = project_with_text();
        let e = TextEdit::begin(&p, 0, 1).expect("valid indices");
        assert_eq!(e.buffer, "Hi λ");
        assert_eq!(e.cursor, 4, "cursor at end, counted in chars");
        assert_eq!(e.char_len(), 4);
    }

    #[test]
    fn begin_empty_for_plain_node() {
        let p = Project::new_demo();
        let e = TextEdit::begin(&p, 0, 0).expect("valid indices");
        assert_eq!(e.buffer, "");
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn begin_rejects_invalid_indices() {
        let p = Project::new_demo();
        assert!(TextEdit::begin(&p, 99, 0).is_none());
        assert!(TextEdit::begin(&p, 0, 999).is_none());
    }

    #[test]
    fn typing_inserts_at_cursor() {
        let p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        e.push_char('a');
        e.push_char('c');
        e.move_cursor(-1);
        e.push_char('b');
        assert_eq!(e.buffer, "abc");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn control_chars_are_ignored_space_is_not() {
        let p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        e.push_char('\n');
        e.push_char('\t');
        e.push_char('\u{7}');
        assert_eq!(e.buffer, "");
        e.push_char(' ');
        assert_eq!(e.buffer, " ");
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn backspace_and_delete_remove_whole_chars() {
        let p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        for c in "aλb".chars() {
            e.push_char(c);
        }
        e.backspace(); // removes 'b'
        assert_eq!(e.buffer, "aλ");
        assert_eq!(e.cursor, 2);
        e.backspace(); // removes the multi-byte 'λ' atomically
        assert_eq!(e.buffer, "a");
        assert_eq!(e.cursor, 1);
        e.backspace();
        e.backspace(); // no-op at start
        assert_eq!(e.buffer, "");
        // delete() removes at the cursor
        for c in "éx".chars() {
            e.push_char(c);
        }
        e.home();
        e.delete(); // removes 'é' atomically
        assert_eq!(e.buffer, "x");
        e.end();
        e.delete(); // no-op at end
        assert_eq!(e.buffer, "x");
    }

    #[test]
    fn cursor_moves_clamp_to_buffer() {
        let p = project_with_text();
        let mut e = TextEdit::begin(&p, 0, 1).unwrap(); // "Hi λ", len 4
        e.move_cursor(-100);
        assert_eq!(e.cursor, 0);
        e.move_cursor(2);
        assert_eq!(e.cursor, 2);
        e.move_cursor(100);
        assert_eq!(e.cursor, 4);
        e.home();
        assert_eq!(e.cursor, 0);
        e.end();
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn display_marks_cursor_by_char() {
        let p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        assert_eq!(e.display(), "|");
        e.push_char('λ');
        e.push_char('x');
        assert_eq!(e.display(), "λx|");
        e.home();
        assert_eq!(e.display(), "|λx");
        e.move_cursor(1);
        assert_eq!(e.display(), "λ|x");
    }

    #[test]
    fn commit_writes_and_preserves_size() {
        let mut p = project_with_text();
        let mut e = TextEdit::begin(&p, 0, 1).unwrap();
        e.push_char('!');
        assert!(e.commit(&mut p));
        assert_eq!(text_of(&p, 0, 1), Some(("Hi λ!".to_string(), 40.0)));
    }

    #[test]
    fn commit_uses_default_size_without_prior_text() {
        let mut p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        e.push_char('A');
        assert!(e.commit(&mut p));
        assert_eq!(text_of(&p, 0, 0), Some(("A".to_string(), DEFAULT_TEXT_SIZE)));
    }

    #[test]
    fn commit_empty_buffer_clears_media() {
        let mut p = project_with_text();
        let mut e = TextEdit::begin(&p, 0, 1).unwrap();
        while e.char_len() > 0 {
            e.backspace();
        }
        assert!(e.commit(&mut p));
        assert!(p.figures[0].def.nodes[1].media.is_none());
    }

    #[test]
    fn commit_with_stale_indices_fails_cleanly() {
        let mut p = project_with_text();
        let e = TextEdit::begin(&p, 0, 1).unwrap();
        // Figure removed after begin.
        let mut gone = p.clone();
        gone.figures.clear();
        assert!(!e.commit(&mut gone));
        // Node removed after begin.
        p.figures[0].def.nodes.clear();
        assert!(!e.commit(&mut p));
    }

    #[test]
    fn dropping_without_commit_leaves_project_untouched() {
        let p = project_with_text();
        {
            let mut e = TextEdit::begin(&p, 0, 1).unwrap();
            e.push_char('Z');
            e.backspace();
            e.push_char('?');
            // dropped here without commit = cancel
        }
        assert_eq!(text_of(&p, 0, 1), Some(("Hi λ".to_string(), 40.0)));
    }

    #[test]
    fn unicode_round_trip_through_commit() {
        let mut p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        for c in "λé—ok".chars() {
            e.push_char(c);
        }
        assert_eq!(e.char_len(), 5);
        assert!(e.commit(&mut p));
        assert_eq!(text_of(&p, 0, 0).map(|(s, _)| s), Some("λé—ok".to_string()));
    }

    // ---- Wave 11 (B10): MCP text parity ----

    fn core_with_instance(content: &str, keys: usize) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.texts.push(TextInstance {
            content: content.into(),
            size: 20.0,
            color: 0xFF_FFFF,
            keys: (0..keys)
                .map(|k| InstanceKey {
                    frame: k as f32,
                    x: 10.0,
                    y: 20.0,
                    scale: 1.0,
                    angle: 0.0,
                })
                .collect(),
        });
        core
    }

    #[test]
    fn add_text_instance_defaults_and_clamps() {
        let mut core = EditorCore::new();
        let out = tool_add_text_instance(
            &mut core,
            &serde_json::json!({ "content": "Hello" }),
        )
        .unwrap();
        assert_eq!(out["index"], 0);
        let inst = &core.project.texts[0];
        assert_eq!(inst.content, "Hello");
        assert_eq!(inst.size, DEFAULT_TEXT_SIZE, "session default size");
        assert_eq!(inst.color, 0xFF_FFFF);
        assert_eq!(inst.keys.len(), 1);
        assert_eq!(inst.keys[0].frame, core.current as f32);
        assert_eq!(inst.keys[0].x, core.project.stage[0] / 2.0);
        // Oversize clamps into the panel's range.
        let out = tool_add_text_instance(
            &mut core,
            &serde_json::json!({ "content": "Big", "size": 999.0 }),
        )
        .unwrap();
        assert_eq!(out["instance"]["size"], TEXT_SIZE_MAX);
        // Empty and control-only content are argument errors.
        assert!(tool_add_text_instance(
            &mut core,
            &serde_json::json!({ "content": "" }),
        )
        .is_err());
        assert!(tool_add_text_instance(
            &mut core,
            &serde_json::json!({ "content": "\n\t" }),
        )
        .is_err());
        assert_eq!(core.project.texts.len(), 2, "refusals create nothing");
    }

    #[test]
    fn update_text_instance_guards_match_the_panel() {
        let mut core = core_with_instance("Old", 2);
        // InstanceKey has no PartialEq; capture the key
        // fields as tuples for the untouched-keys check.
        let key_fields = |core: &EditorCore| -> Vec<(f32, f32, f32, f32, f32)> {
            core.project.texts[0]
                .keys
                .iter()
                .map(|k| (k.frame, k.x, k.y, k.scale, k.angle))
                .collect()
        };
        let keys_before = key_fields(&core);
        // Unchanged: a no-op, not an error.
        let out = tool_update_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "content": "Old" }),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(out["status"], "Text unchanged.");
        // Empty content can never blank a text.
        assert!(tool_update_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "content": "" }),
        )
        .is_err());
        // A real update touches content only.
        let out = tool_update_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "content": "New" }),
        )
        .unwrap();
        assert_eq!(out["changed"], true);
        assert_eq!(core.project.texts[0].content, "New");
        assert_eq!(key_fields(&core), keys_before, "keys survive a content edit");
        assert_eq!(core.project.texts[0].size, 20.0);
        // Stale index is an error with the count.
        let err = tool_update_text_instance(
            &mut core,
            &serde_json::json!({ "index": 7, "content": "X" }),
        )
        .unwrap_err();
        assert!(err.contains("project has 1 text(s)"), "{err}");
    }

    #[test]
    fn instance_mutation_ops_follow_panel_semantics() {
        let mut core = core_with_instance("A", 1);
        core.project.texts[0].keys[0].x = 100.0;
        core.project.texts[0].keys[0].y = 50.0;

        // Duplicate: verbatim + (12, 12), inserted after.
        let out = tool_duplicate_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0 }),
        )
        .unwrap();
        assert_eq!(out["new_index"], 1);
        assert_eq!(core.project.texts[1].keys[0].x, 112.0);
        assert_eq!(core.project.texts[1].keys[0].y, 62.0);

        // Nudge shifts every key.
        let out = tool_nudge_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "dx": 4.0, "dy": -4.0 }),
        )
        .unwrap();
        assert_eq!(out["changed"], true);
        assert_eq!(core.project.texts[0].keys[0].x, 104.0);
        assert_eq!(core.project.texts[0].keys[0].y, 46.0);

        // Rotate adds to every key's angle.
        tool_rotate_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "degrees": 15.0 }),
        )
        .unwrap();
        assert!((core.project.texts[0].keys[0].angle - 15.0_f32.to_radians()).abs() < 1e-6);

        // Colour set.
        let out = tool_set_text_instance_color(
            &mut core,
            &serde_json::json!({ "index": 0, "color": 0x112233 }),
        )
        .unwrap();
        assert_eq!(out["color"], 0x112233);

        // Size: absolute set clamps; same size is a no-op.
        let out = tool_set_text_instance_size(
            &mut core,
            &serde_json::json!({ "index": 0, "size": 1000.0 }),
        )
        .unwrap();
        assert_eq!(out["size"], TEXT_SIZE_MAX);
        let out = tool_set_text_instance_size(
            &mut core,
            &serde_json::json!({ "index": 0, "size": 1000.0 }),
        )
        .unwrap();
        assert_eq!(out["changed"], false, "already at the clamped size");

        // Z-order: instance 0 to the front swaps the pair;
        // repeating it is the panel's honest no-op.
        let out = tool_move_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "to_front": true }),
        )
        .unwrap();
        assert_eq!(out["new_index"], 1);
        assert_eq!(core.project.texts[1].content, "A");
        let out = tool_move_text_instance(
            &mut core,
            &serde_json::json!({ "index": 1, "to_front": true }),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(out["status"], "Already at the front.");

        // Delete removes exactly the addressed instance.
        let out = tool_delete_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0 }),
        )
        .unwrap();
        assert_eq!(out["texts"], 1);
        assert_eq!(core.project.texts[0].content, "A");
    }

    #[test]
    fn keyless_instance_nudge_refuses_but_rotate_counts() {
        let mut core = core_with_instance("Float", 0);
        let out = tool_nudge_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "dx": 4.0, "dy": 4.0 }),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(out["status"], "Text has no position to move.");
        // The panel counts a keyless rotate as rotated.
        let out = tool_rotate_text_instance(
            &mut core,
            &serde_json::json!({ "index": 0, "degrees": 15.0 }),
        )
        .unwrap();
        assert_eq!(out["changed"], true);
    }

    #[test]
    fn segment_text_ops_round_trip() {
        let mut core = EditorCore::new();
        // Fresh node: session default size without a size arg.
        let out = tool_set_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "content": "Seg" }),
        )
        .unwrap();
        assert_eq!(out["content"], "Seg");
        assert_eq!(out["size"], DEFAULT_TEXT_SIZE);
        // Replace (never append), with an explicit size.
        let out = tool_set_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "content": "New", "size": 40.0 }),
        )
        .unwrap();
        assert_eq!(out["content"], "New");
        assert_eq!(out["size"], 40.0);
        // Replace without a size preserves the stored size.
        let out = tool_set_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "content": "Again" }),
        )
        .unwrap();
        assert_eq!(out["size"], 40.0);
        // Control characters are dropped by the session; the
        // response's read-back shows what actually stored.
        let out = tool_set_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "content": "A\nB" }),
        )
        .unwrap();
        assert_eq!(out["content"], "AB");
        // Size op: clamps, and refuses on a textless node.
        let out = tool_set_segment_text_size(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "size": 1000.0 }),
        )
        .unwrap();
        assert_eq!(out["size"], TEXT_SIZE_MAX);
        let out = tool_set_segment_text_size(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 2, "size": 40.0 }),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(out["status"], "No text on this segment.");
        // Clear removes the text.
        let out = tool_clear_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1 }),
        )
        .unwrap();
        assert_eq!(out["changed"], true);
        assert!(core.project.figures[0].def.nodes[1].media.is_none());
    }

    #[test]
    fn clear_segment_text_leaves_images_alone() {
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 0 });
        let out = tool_clear_segment_text(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1 }),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert!(matches!(
            core.project.figures[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 0 })
        ));
    }

    #[test]
    fn list_texts_reports_both_kinds() {
        let mut core = core_with_instance("Label", 1);
        core.project.figures[0].def.nodes[1].media = Some(SegmentMedia::Text {
            content: "Seg".into(),
            size: 33.0,
        });
        let out = tool_list_texts(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["segment_texts"][0]["figure"], 0);
        assert_eq!(out["segment_texts"][0]["node"], 1);
        assert_eq!(out["segment_texts"][0]["content"], "Seg");
        assert_eq!(out["segment_texts"][0]["size"], 33.0);
        assert_eq!(out["instances"][0]["content"], "Label");
        assert_eq!(out["instances"][0]["keys"][0]["x"], 10.0);
    }

    // ---- Wave 12 (C7): emitter helpers ----

    #[test]
    fn xml_escape_covers_the_five_significant_characters() {
        assert_eq!(xml_escape("plain text 123"), "plain text 123");
        assert_eq!(
            xml_escape("a<b>c&d\"e'f"),
            "a&lt;b&gt;c&amp;d&quot;e&apos;f"
        );
        // `&` escapes first: a pre-written entity double-escapes
        // instead of passing through as markup.
        assert_eq!(xml_escape("&lt;"), "&amp;lt;");
        assert_eq!(xml_escape(""), "");
    }

    #[test]
    fn xml_escape_replaces_controls_with_spaces_preserving_count() {
        // XML 1.0 forbids most C0 controls outright; the raster
        // gives each a blank glyph of full advance, so a space
        // keeps both the look and the measured width — and the
        // character count, which the metrics count.
        let raw = "A\nB\tC\u{0}\u{7}\u{7f}\u{85}D";
        let escaped = xml_escape(raw);
        assert_eq!(escaped, "A B C    D");
        assert_eq!(escaped.chars().count(), raw.chars().count());
        assert!(!escaped.chars().any(|c| c.is_control()));
        // Multi-byte characters pass through untouched.
        assert_eq!(xml_escape("λé—"), "λé—");
    }

    #[test]
    fn normalise_text_content_matches_the_session_filter() {
        // push_char drops controls, keeps everything else —
        // including spaces and multi-byte characters.
        assert_eq!(normalise_text_content("A\nB\tC\u{7}"), "ABC");
        assert_eq!(normalise_text_content(" λ é "), " λ é ");
        // Control-only and empty input normalise to the
        // model's "no text" state.
        assert_eq!(normalise_text_content("\n\t"), "");
        assert_eq!(normalise_text_content(""), "");
        // And a session fed the raw string stores exactly the
        // normalised form.
        let p = Project::new_demo();
        let mut e = TextEdit::begin(&p, 0, 0).unwrap();
        for c in "A\nB\tC".chars() {
            e.push_char(c);
        }
        assert_eq!(e.buffer, normalise_text_content("A\nB\tC"));
    }

    #[test]
    fn raster_metrics_reproduce_the_raster_formula() {
        // size 25 -> cell = 25 / 5 = 5 stage units.
        let m = raster_text_metrics("A", 25.0);
        assert_eq!(m.char_count, 1);
        assert_eq!(m.cell, 5.0);
        assert_eq!(m.width, 15.0, "one char: (1*4 - 1) cells * 5");
        assert_eq!(m.height, 25.0, "height is exactly the size");
        assert_eq!(m.start_cells, -1.5);
        let m = raster_text_metrics("ABC", 25.0);
        assert_eq!(m.width, 55.0, "three chars: (3*4 - 1) cells * 5");
        assert_eq!(m.start_cells, -5.5);
        assert_eq!(raster_text_width("ABC", 25.0), 55.0);
        // Every char counts equally — multi-byte, spaces and
        // even control characters (the raster counts them in
        // the width and paints them blank).
        assert_eq!(raster_text_metrics("λ λ", 25.0).char_count, 3);
        assert_eq!(raster_text_width("A\nB", 25.0), 55.0);
        // Empty content: the raster draws nothing; width must
        // not go negative from the formula's -1.
        let m = raster_text_metrics("", 25.0);
        assert_eq!(m.char_count, 0);
        assert_eq!(m.width, 0.0);
        assert_eq!(m.height, 0.0);
    }

    #[test]
    fn glyph_cell_centre_matches_the_raster_offsets() {
        let m = raster_text_metrics("A", 25.0); // cell 5, start -1.5
        // Top-left cell centre: (-1.5 + 0 + 0 + 0.5) * 5,
        // (0 - 2 + 0.5) * 5.
        assert_eq!(glyph_cell_centre(&m, 0, 0, 0), [-5.0, -7.5]);
        // Bottom-right cell: x symmetric, y at +2.5 cells —
        // the raster's half-cell downward bias, pinned.
        assert_eq!(glyph_cell_centre(&m, 0, 2, 4), [5.0, 12.5]);
        // Second character starts 4 cells after the first:
        // start -3.5 cells, so (-3.5 + 4 + 0 + 0.5) * 5 = 5.
        let m2 = raster_text_metrics("AB", 25.0);
        assert_eq!(glyph_cell_centre(&m2, 1, 0, 2)[0], 5.0);
    }

    #[test]
    fn rotate_text_offset_is_the_y_down_rotation() {
        assert_eq!(rotate_text_offset([3.0, -4.0], 0.0), [3.0, -4.0]);
        // +90 degrees in y-down space turns +x onto +y
        // (clockwise on screen), as media.rs applies it.
        let r = rotate_text_offset([1.0, 0.0], 90.0);
        assert!(r[0].abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6, "{r:?}");
        let r = rotate_text_offset([0.0, 1.0], 90.0);
        assert!((r[0] + 1.0).abs() < 1e-6 && r[1].abs() < 1e-6, "{r:?}");
    }

    #[test]
    fn text_color_hex_masks_and_formats_like_svg_hex() {
        assert_eq!(text_color_hex(0x112233), "#112233");
        assert_eq!(text_color_hex(0xFF112233), "#112233", "alpha bits masked off");
        assert_eq!(text_color_hex(0x00FF00), "#00FF00");
        assert_eq!(text_color_hex(0), "#000000");
    }
}
