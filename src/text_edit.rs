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

use crate::model::{Project, SegmentMedia};

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
}
