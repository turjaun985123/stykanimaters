//! History panel (Roadmap Wave 1, RM24, P-14 — audit
//! A15 H2): the labelled undo history as a list the
//! user can read and jump through, in the Animate
//! workspace's right column (mounted beside the Frames
//! section; the mount itself is IA-RM24-1 — main.rs is
//! an integrator surface).
//!
//! The rows are the core's two history stacks, newest
//! first, with the current position marked between
//! them: entries above the marker are the redo future
//! (dimmed), entries below are the undo past. Pressing
//! a row jumps straight to it through
//! `EditorCore::history_jump` — the steps between are
//! full undo/redo restores, so a jump costs exactly
//! the steps it crosses, no more.
//!
//! Row keys are `history-row-{offset}` where the
//! offset counts from the current position (negative
//! into the past, positive into the future, 0 on the
//! marker row itself) — variable-carried keys, the
//! `graph-chan-{i}` precedent, not counted keys. The
//! card's own key, `history-panel`, is the seat's one
//! counted element key.
//!
//! Copy: the title resolves through the i18n wire
//! (key `history`, filed as IA-RM24-2 — until the
//! integrator splices it ×28 the wire falls back to
//! the key itself, the shell's adopt-ahead idiom);
//! the empty-state line is a plain literal, the
//! graph panel's empty-state precedent.

use rune::tok::{type_, Theme};
use rune::{col, text, El, ScrollSpec};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::State;

/// How tall the row list may grow before it scrolls —
/// the side column is shared with the Frames section,
/// and a full 60-entry history must not push it off
/// the workspace.
const HISTORY_LIST_MAX_H: f32 = 264.0;

/// One History row, as data (the panel's testable
/// model): the entry's label, how many history steps
/// a press jumps from the current position (negative
/// = undo, positive = redo, 0 = the marker row), and
/// whether this row IS the current position.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRow {
    pub label: String,
    pub jump: i32,
    pub current: bool,
}

/// The panel's rows in display order (newest first):
/// the redo future above the marker — nearest future
/// closest to it — then the marker, then the undo
/// past, nearest first. Pure over the core's label
/// accessors, so the order is unit-testable without a
/// tree.
pub fn history_rows(core: &EditorCore) -> Vec<HistoryRow> {
    let mut rows = Vec::new();
    // Redo stack, oldest-first as stored: its LAST
    // entry is the next redo (nearest the marker), its
    // FIRST is the furthest future (the top row).
    let redo = core.redo_labels();
    for (i, label) in redo.iter().enumerate() {
        rows.push(HistoryRow {
            label: label.clone(),
            jump: (redo.len() - i) as i32,
            current: false,
        });
    }
    rows.push(HistoryRow {
        label: i18n_wire::tr(core, "history_current"),
        jump: 0,
        current: true,
    });
    // Undo stack, oldest-first as stored: display
    // reversed, so the next undo sits directly under
    // the marker.
    let undo = core.undo_labels();
    for (i, label) in undo.iter().enumerate().rev() {
        rows.push(HistoryRow {
            label: label.clone(),
            jump: -((undo.len() - i) as i32),
            current: false,
        });
    }
    rows
}

/// The History card: header, then either the empty
/// state (a fresh core has no history to show) or the
/// scrollable row list.
pub fn history_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let mut card = design::panel_col("history-panel", t).child(design::panel_title(
        Some("clock-rotate-left"),
        &i18n_wire::tr(core, "history"),
        None,
        t,
    ));
    if core.undo_depth() == 0 && core.redo_depth() == 0 {
        return card.child(design::empty_state(
            "clock-rotate-left",
            "Nothing to undo yet — edits you make will appear here, named.",
            t,
        ));
    }
    let mut list = col().gap(4.0);
    for row in history_rows(core) {
        let offset = row.jump;
        let mut el = design::list_row(&format!("history-row-{offset}"), row.current, t).w_full();
        if !row.current {
            el = el.on_press(move |s: &mut State| {
                s.core.history_jump(offset);
            });
        }
        // The marker row names the present; future rows
        // dim (they have not happened yet), past rows
        // read as plain entries.
        let (label_color, font) = if row.current {
            (t.text, type_::LABEL)
        } else if offset > 0 {
            (t.text_secondary, type_::LABEL)
        } else {
            (t.text, type_::LABEL)
        };
        list = list.child(
            el.child(text(if row.current {
                format!("● {}", row.label)
            } else {
                row.label.clone()
            })
            .font(font)
            .color(label_color)
            .grow(1.0)),
        );
    }
    card.child(
        list.scroll(ScrollSpec {
            stick_bottom: false,
        })
        .min_h(0.0)
        .max_h(HISTORY_LIST_MAX_H),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A core with three labelled edits, the newest
    /// undone once: undo [Add frame, Delete frame],
    /// redo [Move joint].
    fn walked_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.snapshot_labeled("Add frame");
        core.snapshot_labeled("Delete frame");
        core.snapshot_labeled("Move joint");
        core.undo();
        core
    }

    #[test]
    fn history_rows_order_future_marker_past() {
        let core = walked_core();
        let rows = history_rows(&core);
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            // The marker row carries the RESOLVED string for
            // `history_current`, not the key itself.
            vec!["Move joint", "Current position", "Delete frame", "Add frame"]
        );
        let jumps: Vec<i32> = rows.iter().map(|r| r.jump).collect();
        assert_eq!(jumps, vec![1, 0, -1, -2]);
        assert!(rows[1].current);
        assert_eq!(rows.iter().filter(|r| r.current).count(), 1);
    }

    #[test]
    fn history_rows_on_a_fresh_core_are_just_the_marker() {
        let core = EditorCore::new();
        let rows = history_rows(&core);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].current);
        assert_eq!(rows[0].jump, 0);
    }

    #[test]
    fn history_section_builds_empty_and_populated() {
        let t = crate::rune_sec::app_theme::app_theme(true);
        let mut s = crate::init();
        let _ = history_section(&s, &t);
        s.core.snapshot_labeled("Add frame");
        let _ = history_section(&s, &t);
        s.core.undo();
        let _ = history_section(&s, &t);
    }
}
