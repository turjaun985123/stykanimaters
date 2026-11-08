//! A progress-steps strip: one dot-and-label cluster per step
//! of a multi-stage flow (import → pose → export, a first-run
//! wizard, a multi-part render), joined by short connector bars.
//!
//! The strip answers exactly one question — where are we in the
//! flow? Steps before the current one read as done (green dot),
//! the current step is accented with ink text, and the steps
//! still to come sit quiet in the neutral grey. Presentation
//! only, like the rest of the kit: plain data in, a `Div` out,
//! no listeners, no ids and no editor knowledge. The caller owns
//! the flow and passes its step labels plus the current index;
//! [`step_state`] holds the one comparison that decides a step's
//! look, kept pure so it is testable without a window.

use crate::ui_kit::{C_ACCENT, C_BORDER, C_CELL, C_INK, C_MUTED};
use gpui::*;

/// Dot colour for a finished step: the same green
/// [`crate::ui_badge`]'s `Tone::Success` paints, so a completed
/// step and a success badge agree.
const C_DONE: u32 = 0x2E7D4F;
/// Dot colour for a step still to come: the neutral grey
/// `info_chip` uses when inactive.
const C_TODO: u32 = C_CELL;

/// Where one step stands relative to the current step of the
/// flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepState {
    /// Already passed.
    Done,
    /// The step the user is on now.
    Current,
    /// Not reached yet.
    Todo,
}

/// The state of step `index` when `current` is the active step:
/// steps before it are [`StepState::Done`], the step itself is
/// [`StepState::Current`], steps after it are [`StepState::Todo`].
///
/// `current` may legally point past the last step (a flow that
/// has run to completion): every index is then before it, so
/// every step reads Done.
pub fn step_state(index: usize, current: usize) -> StepState {
    match index.cmp(&current) {
        std::cmp::Ordering::Less => StepState::Done,
        std::cmp::Ordering::Equal => StepState::Current,
        std::cmp::Ordering::Greater => StepState::Todo,
    }
}

/// The dot colour for a step state, as 0xRRGGBB: green for done,
/// the kit accent for current, neutral grey for todo. Pure data,
/// split out from the builder so the mapping is testable without
/// a window.
pub fn step_dot_color(state: StepState) -> u32 {
    match state {
        StepState::Done => C_DONE,
        StepState::Current => C_ACCENT,
        StepState::Todo => C_TODO,
    }
}

/// The strip itself: one cluster per label in `labels` — an 8px
/// dot in [`step_dot_color`] beside the label at 11px (ink for
/// the current step, muted otherwise) — with a 12px-wide, 1px
/// connector bar in the border colour between clusters (never
/// after the last one). An empty `labels` gives an empty row.
///
/// `current` past the end is legal and marks every step Done —
/// the natural "flow finished" rendering. Labels are copied into
/// owned strings: elements must own their text, so the borrowed
/// `&str`s are never stored in the tree.
pub fn steps_row(labels: &[&str], current: usize) -> Div {
    let mut row = div().flex().items_center().gap(px(6.0));
    let last = labels.len().saturating_sub(1);
    for (i, label) in labels.iter().enumerate() {
        let state = step_state(i, current);
        let fg = if state == StepState::Current {
            C_INK
        } else {
            C_MUTED
        };
        row = row.child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded(px(999.0))
                        .bg(rgb(step_dot_color(state))),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(fg))
                        .child(label.to_string()),
                ),
        );
        if i < last {
            row = row.child(div().w(px(12.0)).h(px(1.0)).bg(rgb(C_BORDER)));
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::{step_dot_color, step_state, steps_row, StepState};
    use crate::ui_kit::C_CELL;

    #[test]
    fn step_state_compares_index_against_current() {
        // Before the current step: done.
        assert_eq!(step_state(0, 2), StepState::Done);
        assert_eq!(step_state(1, 2), StepState::Done);
        // The current step itself.
        assert_eq!(step_state(2, 2), StepState::Current);
        assert_eq!(step_state(0, 0), StepState::Current);
        // After the current step: still to do.
        assert_eq!(step_state(3, 2), StepState::Todo);
        assert_eq!(step_state(1, 0), StepState::Todo);
    }

    #[test]
    fn step_state_with_current_past_the_end_is_all_done() {
        // A finished flow: current beyond the last index marks
        // every step Done rather than panicking or wrapping.
        for i in 0..3 {
            assert_eq!(step_state(i, 99), StepState::Done);
        }
    }

    #[test]
    fn step_dot_color_matches_the_kit_palette() {
        assert_eq!(step_dot_color(StepState::Done), 0x2E7D4F);
        assert_eq!(step_dot_color(StepState::Current), 0x3D5A99);
        assert_eq!(step_dot_color(StepState::Todo), C_CELL);
    }

    #[test]
    fn steps_row_builds_for_any_current() {
        // Div construction needs no window in this codebase; the
        // point of these calls is that none of them panic —
        // including a current index past the last step and an
        // empty flow.
        let _ = steps_row(&["Import", "Pose", "Export"], 1);
        let _ = steps_row(&["Import", "Pose", "Export"], 99);
        let _ = steps_row(&["Only step"], 0);
        let _ = steps_row(&[], 0);
    }
}
