//! Help & shortcuts overlay: pure data + a stateless panel
//! builder, in the [`crate::ui_kit`] style.
//!
//! The shortcut rows in [`shortcut_rows`] are a transcription of
//! the root `on_key_down` handler in `app.rs` — every row names a
//! key combination that handler actually matches, with the action
//! it performs. The handler has two modes: the global match, and
//! the text-authoring capture that takes over the keyboard while
//! a text segment is being edited (rows for that mode say so in
//! their description). Keep this list in sync when the handler
//! changes; do not document shortcuts that do not exist.
//!
//! [`group_rows`] gives a one-line, factual description of each
//! labelled tool-group panel in the toolbar.

use crate::ui_kit::*;
use gpui::*;

/// The keyboard shortcuts the root key handler implements, as
/// (key label, action) rows: the global bindings first, then the
/// text-authoring capture bindings.
pub fn shortcut_rows() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Space", "Play / pause"),
        ("Left", "Previous frame (wraps around)"),
        ("Right", "Next frame (wraps around)"),
        ("Ctrl+Z", "Undo"),
        ("Ctrl+Shift+Z", "Redo"),
        ("Ctrl+Y", "Redo"),
        ("N", "Add a frame after the current one"),
        ("I", "Insert an inbetween frame"),
        ("O", "Toggle onion skin"),
        ("C", "Toggle camera mode"),
        ("Enter", "Text editing: confirm and set the text"),
        ("Esc", "Text editing: cancel without setting the text"),
        ("Backspace", "Text editing: delete the character before the cursor"),
        ("Delete", "Text editing: delete the character after the cursor"),
        ("Left", "Text editing: move the cursor left"),
        ("Right", "Text editing: move the cursor right"),
        ("Home", "Text editing: move the cursor to the start"),
        ("End", "Text editing: move the cursor to the end"),
        ("Any character key", "Text editing: type that character"),
    ]
}

/// One-line descriptions of the toolbar's tool-group panels, as
/// (group name, what it does) rows, in toolbar order.
pub fn group_rows() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Transport", "Play/pause, add, duplicate and delete frames, onion skin, loop range"),
        ("Figure", "Add a new stick figure to the project"),
        ("Frames", "Insert inbetweens; shift, reverse and smooth a range of frames"),
        ("Segments", "Add or delete segments; edit length, thickness, colour, shape and bend"),
        ("Figures", "Add preset figures: quadruped and mini"),
        ("Draw", "Freehand drawing: draw mode, brush presets, brush/eraser tool, clear drawing"),
        ("Camera", "Camera mode and zoom, clear camera keys, camera readout, GIF export"),
        ("Playback", "Playback fps, frame duration, tween playback and easing"),
        ("Effects", "Particle effects on the selected figure, motion paths, camera follow"),
        ("Library & Clips", "Figure library (cycle, insert, save) and movieclips (make, insert)"),
        ("Export", "Export MP4, WebM, spritesheet, SVG, Pivot .piv and .stk"),
        ("Projects", "Save, cycle and open named projects, with native file dialogs"),
        ("Import", "Import Pivot .piv and .stk files"),
        ("View", "Light/dark theme and onion-skin range"),
        ("Audio & Text", "Add audio clips, author text segments, key the animation graph"),
        ("3D", "3D posing: rotate joints, orbit, bake poses and animations to 2D, 3D library"),
        ("Scene", "Weather effects, camera rotation, clear graph channels"),
        ("Audio Ops", "Normalise, split and crossfade audio clips; mixdown"),
        ("Formats", "Export/import Pivot v2, v4 and v5, APNG, JPEG, Stick Nodes .nodes and .stknds"),
        ("Stage & Language", "Stage width and height, interface language, per-frame easing"),
        ("Effects 2", "Beam, aura and trail effect clips"),
        ("Home", "Home screen (saved projects, recents) and recovery snapshot"),
        ("Segment Style", "Segment end caps, taper, gradient, image media and clearing media"),
        ("Strokes", "Smooth the current frame's drawing strokes"),
    ]
}

/// One keyboard-shortcut row: a small pill holding the key label,
/// then the action it performs.
fn shortcut_row(key: &str, action: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(999.0))
                .bg(rgb(0x2C313A))
                .border_1()
                .border_color(rgb(C_BORDER))
                .text_size(px(11.0))
                .text_color(rgb(C_INK))
                .child(key.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child(action.to_string()),
        )
}

/// One tool-group row: the group's name, then its description.
fn group_row(name: &str, description: &str) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(6.0))
        .child(
            div()
                .w(px(150.0))
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(name.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child(description.to_string()),
        )
}

/// The help panel: two columns inside the standard bordered
/// panel container — keyboard shortcuts on the left, the tool
/// groups guide on the right. Stateless: callers pass the rows
/// from [`shortcut_rows`] and [`group_rows`].
pub fn help_panel(shortcuts: &[(&str, &str)], groups: &[(&str, &str)]) -> Div {
    let mut shortcuts_col = div()
        .flex_col()
        .flex_1()
        .gap(px(4.0))
        .child(section_label("Keyboard shortcuts"))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child("Rows marked \u{201c}Text editing\u{201d} apply while a text segment is being authored; the keyboard is captured then."),
        );
    for (key, action) in shortcuts {
        shortcuts_col = shortcuts_col.child(shortcut_row(key, action));
    }
    let mut groups_col = div()
        .flex_col()
        .flex_1()
        .gap(px(4.0))
        .child(section_label("Tool groups"));
    for (name, description) in groups {
        groups_col = groups_col.child(group_row(name, description));
    }
    div()
        .flex_col()
        .w_full()
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .p(px(8.0))
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(shortcuts_col)
                .child(groups_col),
        )
}

#[cfg(test)]
mod tests {
    use super::{group_rows, shortcut_rows};

    #[test]
    fn shortcut_rows_has_exactly_19_rows() {
        assert_eq!(shortcut_rows().len(), 19);
    }

    #[test]
    fn shortcut_rows_spot_checks() {
        let rows = shortcut_rows();
        // First global binding and the undo binding, exactly as
        // the key handler implements them.
        assert_eq!(rows[0], ("Space", "Play / pause"));
        assert!(rows.contains(&("Ctrl+Z", "Undo")));
        assert!(rows.contains(&("Ctrl+Shift+Z", "Redo")));
        // Left/Right appear twice: once global, once in the
        // text-authoring capture.
        assert_eq!(rows.iter().filter(|(k, _)| *k == "Left").count(), 2);
        assert_eq!(rows.iter().filter(|(k, _)| *k == "Right").count(), 2);
    }

    #[test]
    fn shortcut_rows_globals_come_before_text_editing_rows() {
        let rows = shortcut_rows();
        // The 10 global bindings come first; every row after
        // them is a text-authoring row whose action says so.
        for (key, action) in &rows[..10] {
            assert!(
                !action.starts_with("Text editing:"),
                "global row {key} is marked as text editing"
            );
        }
        for (key, action) in &rows[10..] {
            assert!(
                action.starts_with("Text editing:"),
                "text-editing row {key} is not marked as such"
            );
        }
    }

    #[test]
    fn group_rows_covers_all_tool_groups() {
        let rows = group_rows();
        assert_eq!(rows.len(), 24);
        assert_eq!(rows.first().map(|(name, _)| *name), Some("Transport"));
        assert_eq!(rows.last().map(|(name, _)| *name), Some("Strokes"));
    }

    #[test]
    fn group_names_are_unique() {
        let rows = group_rows();
        for (i, (name, _)) in rows.iter().enumerate() {
            assert!(
                !rows[..i].iter().any(|(n, _)| n == name),
                "duplicate tool-group name: {name}"
            );
        }
    }

    #[test]
    fn no_empty_strings_anywhere() {
        for (key, action) in shortcut_rows() {
            assert!(!key.trim().is_empty(), "empty shortcut key");
            assert!(!action.trim().is_empty(), "empty action for {key}");
        }
        for (name, description) in group_rows() {
            assert!(!name.trim().is_empty(), "empty group name");
            assert!(
                !description.trim().is_empty(),
                "empty description for {name}"
            );
        }
    }
}
