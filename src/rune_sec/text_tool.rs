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

use rune::tok::{type_, Theme};
use rune::{col, row, text, text_input, El, TextEdit as FieldEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::SegmentMedia;
use stykanimate_rs::text_edit::{TextEdit as SegTextEdit, DEFAULT_TEXT_SIZE};

use crate::chrome_view::btn;
use crate::State;

/// Smallest glyph height the stepper allows (stage units).
pub const SIZE_MIN: f32 = 4.0;
/// Largest glyph height the stepper allows (stage units).
pub const SIZE_MAX: f32 = 160.0;
/// Glyph-height change per stepper press (stage units).
pub const SIZE_STEP: f32 = 4.0;

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
}

impl Default for TextToolState {
    fn default() -> Self {
        TextToolState {
            field: FieldEdit::default(),
            size: DEFAULT_TEXT_SIZE,
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

/// One free-standing text instance (`Project.texts`), for
/// the "Existing texts" list: its real index in the vec
/// (the row's buttons act on the instance it names, even
/// as deletions shift the indices of the rest), a content
/// preview, how many transform keys it carries, and its
/// glyph height.
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
            preview: preview(&inst.content),
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
fn load_instance(s: &mut State, index: usize) {
    let Some(inst) = s.core.project.texts.get(index) else {
        return;
    };
    let content = inst.content.clone();
    let size = inst.size;
    s.text_tool.field.replace_text(content);
    s.text_tool.size = size.clamp(SIZE_MIN, SIZE_MAX);
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
/// the selected figure's existing text segments, and the
/// project's free-standing text instances.
pub fn text_tool(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let rows = text_segments(core);
    let shown_size = display_size(core, s.text_tool.size);

    // Free-standing text instances: one row per instance —
    // a load button carrying the content preview (clicking
    // loads it into the field above for re-stamping), its
    // key count and glyph height, and a Delete button.
    // Both buttons carry the instance's real vec index.
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
                        delete_text_instance(&mut s.core, i)
                    })),
            );
        }
    }

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
        assert_eq!(rows[1].preview, preview("A much longer second label"));
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
}
