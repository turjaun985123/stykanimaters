//! Frames toolbar section for the Rune editor shell
//! (Phase D, section 8/40 — frame ops).
//!
//! Inbetween, single-frame shift, reverse, motion-path
//! smoothing, the single-frame and range clipboards, the loop
//! range, and per-frame duration steppers — every control wired
//! to the real `EditorCore` op the GPUI app's "Frames" tool
//! group (and Edit cluster) called, with the same semantics and,
//! where one existed, the same element key.
//!
//! Range model: the core's only range state is
//! `EditorCore::loop_range` (`editor.rs:61`). The op arguments
//! the GPUI handlers computed from their view-side selection
//! live here in [`FrameOpsState`], mirroring the GPUI
//! `EditorView::range_sel` field: `(anchor, focus)`, set by the
//! timeline section's shift-click and consumed by Copy/Delete
//! range. Reverse follows the GPUI rule exactly — the loop
//! range when one is set, the whole timeline otherwise.
//!
//! The "This frame" row (Phase F) sets ONE frame's duration —
//! the current frame's `model::Frame::duration` hold-time
//! multiplier. The model carries no allowed range; the only
//! clamp in the engine is the one `adjust_frame_duration`
//! applies, [0.25, 8.0], mirrored by [`clamp_duration`]. The
//! ×0.5 / ×2 buttons write through that op (relative factors,
//! clamped, status line). No core op sets an absolute duration,
//! so Reset 1.0 writes through [`set_duration_here`], which
//! performs the same field write the MCP `set_frame_duration`
//! tool performs (`core.project.frames[i].duration = value`,
//! mcp_tools.rs), passed through [`clamp_duration`] and given
//! the op's status wording. Neither path snapshots — duration
//! edits are not undoable in this core, exactly like the
//! steppers and the MCP tools.
//!
//! Integrator wiring (main.rs, not edited here): declare the
//! module, add `pub frame_ops: FrameOpsState` to `State`
//! (initialised with `FrameOpsState::default()`), and add
//! `.child(frame_ops::frames_section(s, &t))` to the view.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::State;

/// Duration clamp the core applies in
/// `EditorCore::adjust_frame_duration` (`editor.rs:1677`).
/// Mirrored here only so the steppers can render muted at the
/// bounds; the op itself remains the authority.
pub const DUR_MIN: f32 = 0.25;
/// See [`DUR_MIN`].
pub const DUR_MAX: f32 = 8.0;
/// Stepper factors — the GPUI "Dur -" / "Dur +" buttons called
/// `adjust_frame_duration(0.5)` / `(2.0)`.
pub const DUR_DOWN_FACTOR: f32 = 0.5;
/// See [`DUR_DOWN_FACTOR`].
pub const DUR_UP_FACTOR: f32 = 2.0;

/// UI-side frame-ops state (see the module docs). Stored on the
/// shell's `State` as `frame_ops`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameOpsState {
    /// Timeline range selection as `(anchor, focus)` — the
    /// frame where a shift-click selection started and the
    /// frame it currently extends to. Either order is valid;
    /// [`selected_range`] normalises. Mirrors the GPUI view's
    /// `range_sel`. Written by the timeline section, read and
    /// (after a successful Delete range) cleared here.
    pub range_sel: Option<(usize, usize)>,
    /// True once this UI has run Copy frame. The core's
    /// single-frame `clipboard` is a private field with no
    /// accessor (`editor.rs:53`), so Paste frame's muted state
    /// cannot be read from the core; this flag records the copy
    /// instead. `paste_frame` itself still reports honestly via
    /// the status line if the clipboard is ever empty.
    pub frame_copied: bool,
    /// True once this UI has run Copy range — same reasoning as
    /// [`FrameOpsState::frame_copied`] for the private
    /// `frame_clipboard` (`editor.rs:56`).
    pub range_copied: bool,
}

/// The selected range as ordered, inclusive `(lo, hi)`,
/// clamped to the timeline; `None` when nothing is selected or
/// the project has no frames. A stale selection (frames deleted
/// since) clamps instead of vanishing, matching how the core
/// clamps its own loop range during playback.
pub fn selected_range(core: &EditorCore, ops: &FrameOpsState) -> Option<(usize, usize)> {
    let n = core.frame_count();
    if n == 0 {
        return None;
    }
    let (anchor, focus) = ops.range_sel?;
    let lo = anchor.min(focus).min(n - 1);
    let hi = anchor.max(focus).min(n - 1);
    Some((lo, hi))
}

/// `(from, count)` for Copy range: the selection when one
/// exists, otherwise the current frame on its own — the GPUI
/// handler's fallback.
pub fn copy_range_args(core: &EditorCore, ops: &FrameOpsState) -> (usize, usize) {
    match selected_range(core, ops) {
        Some((lo, hi)) => (lo, hi - lo + 1),
        None => (core.current, 1),
    }
}

/// `(from, count)` for Reverse: the loop range when one is set,
/// the whole timeline otherwise — the GPUI handler's rule.
pub fn reverse_args(core: &EditorCore) -> (usize, usize) {
    match core.loop_range {
        Some((a, b)) => (a, b - a + 1),
        None => (0, core.frame_count()),
    }
}

/// Inbetween inserts between the current frame and the next;
/// on the last frame the op only sets a status message.
pub fn can_inbetween(core: &EditorCore) -> bool {
    core.current + 1 < core.frame_count()
}

/// The GPUI shift buttons moved the current frame alone and
/// refused at the timeline edges.
pub fn can_shift_back(core: &EditorCore) -> bool {
    core.current > 0
}

/// See [`can_shift_back`].
pub fn can_shift_fwd(core: &EditorCore) -> bool {
    core.current + 1 < core.frame_count()
}

/// `reverse_range` is a no-op below two frames.
pub fn can_reverse(core: &EditorCore) -> bool {
    let (_, count) = reverse_args(core);
    count >= 2
}

/// `smooth_path_roots` needs at least three frames and a
/// figure to smooth.
pub fn can_smooth(core: &EditorCore) -> bool {
    core.frame_count() >= 3 && !core.project.figures.is_empty()
}

/// Clamp a duration multiplier to the engine's allowed range,
/// [0.25, 8.0] — the clamp `EditorCore::adjust_frame_duration`
/// applies (the model itself carries no range). Pure mirror so
/// the absolute write in [`set_duration_here`] lands in the
/// same range the relative op would produce.
pub fn clamp_duration(d: f32) -> f32 {
    d.clamp(DUR_MIN, DUR_MAX)
}

/// Format a duration multiplier for a readout, e.g. `1.00×`.
/// The single format source for every duration label here.
pub fn format_duration(d: f32) -> String {
    format!("{d:.2}×")
}

/// The current frame's hold-time multiplier, if there is a
/// current frame.
pub fn duration_here(core: &EditorCore) -> Option<f32> {
    core.project.frames.get(core.current).map(|f| f.duration)
}

/// At the core's lower clamp another halving changes nothing.
pub fn can_duration_down(core: &EditorCore) -> bool {
    duration_here(core).map(|d| d > DUR_MIN).unwrap_or(false)
}

/// At the core's upper clamp another doubling changes nothing.
pub fn can_duration_up(core: &EditorCore) -> bool {
    duration_here(core).map(|d| d < DUR_MAX).unwrap_or(false)
}

/// Duration readout for the current frame, e.g. `1.00×`
/// (`model::Frame::duration` is a hold-time multiplier).
pub fn duration_label(core: &EditorCore) -> String {
    match duration_here(core) {
        Some(d) => format_duration(d),
        None => "—".to_string(),
    }
}

/// Reset changes nothing when the current frame already sits
/// at 1.0 (or there is no current frame).
pub fn can_duration_reset(core: &EditorCore) -> bool {
    duration_here(core)
        .map(|d| (d - 1.0).abs() > f32::EPSILON)
        .unwrap_or(false)
}

/// Set the current frame's duration to an absolute multiplier —
/// the write the "This frame" row's Reset button performs. No
/// core op sets an absolute duration (`adjust_frame_duration`
/// only multiplies), so this performs the field write the MCP
/// `set_frame_duration` tool performs, with two shell-side
/// refinements: the value passes through [`clamp_duration`]
/// (the MCP tool validates positive-only; the row stays inside
/// the range the relative op enforces) and the status line
/// mirrors `adjust_frame_duration`'s wording. Like both of
/// those paths it takes no snapshot — the edit is not undoable.
/// A missing current frame reports via the status line instead
/// of indexing blindly (the op would panic on an empty project).
pub fn set_duration_here(core: &mut EditorCore, value: f32) {
    let Some(frame) = core.project.frames.get_mut(core.current) else {
        core.status = "No frame to set a duration on.".into();
        return;
    };
    frame.duration = clamp_duration(value);
    core.status = format!(
        "Frame {} duration {:.2}x.",
        core.current + 1,
        frame.duration
    );
}

/// Which range the range ops will act on, for the section's
/// readout: the selection wins, then the loop range; `None`
/// when neither exists (range ops then fall back per their own
/// rules — see [`copy_range_args`] and [`reverse_args`]).
pub fn range_label(core: &EditorCore, ops: &FrameOpsState) -> Option<String> {
    if let Some((lo, hi)) = selected_range(core, ops) {
        return Some(if lo == hi {
            format!("Sel {}", lo + 1)
        } else {
            format!("Sel {}–{}", lo + 1, hi + 1)
        });
    }
    if let Some((a, b)) = core.loop_range {
        return Some(if a == b {
            format!("Loop {}", a + 1)
        } else {
            format!("Loop {}–{}", a + 1, b + 1)
        });
    }
    None
}

/// One section button: chrome_view::btn's look (accent when
/// `active`), plus a muted rendering for controls whose op
/// would no-op from the current core state. Muted buttons stay
/// fully wired — the press still runs the handler (and the
/// op's own status message) — they only lose the hover/press
/// highlight and take the secondary text colour.
fn fbtn(
    key: &str,
    label: &str,
    active: bool,
    muted: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    let (fill, hover, press, ink) = if active {
        (accent, accent, accent, Color::WHITE)
    } else if muted {
        (t.control_off, t.control_off, t.control_off, t.text_secondary)
    } else {
        (t.card, t.control_track, t.control_off, t.text)
    };
    el()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(fill)
        .hover(hover)
        .press(press)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(ink))
}

/// The "Frames" toolbar section: the main row in GPUI
/// tool-group order — Inbetween / Shift / Reverse / Smooth
/// path, the clipboards, the loop endpoints, then the duration
/// steppers with their readout — over a second row, "This
/// frame", carrying the current frame's duration multiplier
/// with ×0.5 / ×2 / Reset 1.0 controls (Phase F).
pub fn frames_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let ops = &s.frame_ops;
    let n = core.frame_count();
    let loop_on = core.loop_range.is_some();

    let mut section = row()
        .key("frames")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("Frames")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(fbtn("inbetween", "Inbetween", false, !can_inbetween(core), t, |s| {
            s.core.insert_inbetween();
        }))
        .child(fbtn("shift-back", "< Shift", false, !can_shift_back(core), t, |s| {
            let cur = s.core.current;
            if !s.core.shift_frames(cur, 1, -1) {
                s.core.status = "First frame: cannot shift it earlier.".into();
            }
        }))
        .child(fbtn("shift-fwd", "Shift >", false, !can_shift_fwd(core), t, |s| {
            let cur = s.core.current;
            if !s.core.shift_frames(cur, 1, 1) {
                s.core.status = "Last frame: cannot shift it later.".into();
            }
        }))
        .child(fbtn("reverse-range", "Reverse", false, !can_reverse(core), t, |s| {
            let (from, count) = reverse_args(&s.core);
            s.core.reverse_range(from, count);
        }))
        .child(fbtn("smooth-path", "Smooth path", false, !can_smooth(core), t, |s| {
            let fi = s.core.selected_figure;
            s.core.smooth_path_roots(fi);
        }))
        .child(fbtn("copy-frame", "Copy frame", false, n == 0, t, |s| {
            if s.core.frame_count() > 0 {
                s.core.copy_frame();
                s.frame_ops.frame_copied = true;
            }
        }))
        .child(fbtn("paste-frame", "Paste frame", false, !ops.frame_copied, t, |s| {
            s.core.paste_frame();
        }))
        .child(fbtn("range-copy", "Copy range", false, n == 0, t, |s| {
            let (from, count) = copy_range_args(&s.core, &s.frame_ops);
            if s.core.copy_range(from, count) {
                s.frame_ops.range_copied = true;
            }
        }))
        .child(fbtn("range-paste", "Paste range", false, !ops.range_copied, t, |s| {
            let at = s.core.current;
            let before = s.core.status.clone();
            if !s.core.paste_range(at) && s.core.status == before {
                s.core.status = "Nothing to paste — copy a range first.".into();
            }
        }))
        .child(fbtn(
            "range-del",
            "Delete range",
            false,
            selected_range(core, ops).is_none(),
            t,
            |s| {
                if let Some((lo, hi)) = selected_range(&s.core, &s.frame_ops) {
                    if s.core.delete_range(lo, hi - lo + 1) {
                        s.frame_ops.range_sel = None;
                    }
                }
            },
        ))
        .child(fbtn("loop-start", "Loop start", loop_on, n == 0, t, |s| {
            let cur = s.core.current;
            let (_, b) = s.core.loop_range.unwrap_or((cur, cur));
            s.core.set_loop_range(cur, b);
        }))
        .child(fbtn("loop-end", "Loop end", loop_on, n == 0, t, |s| {
            let cur = s.core.current;
            let (a, _) = s.core.loop_range.unwrap_or((cur, cur));
            s.core.set_loop_range(a, cur);
        }))
        .child(fbtn("loop-clear", "Loop clear", false, !loop_on, t, |s| {
            s.core.clear_loop_range();
        }))
        .child(
            text(format!("Dur {}", duration_label(core)))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(fbtn("dur-dn", "Dur -", false, !can_duration_down(core), t, |s| {
            s.core.adjust_frame_duration(DUR_DOWN_FACTOR);
        }))
        .child(fbtn("dur-up", "Dur +", false, !can_duration_up(core), t, |s| {
            s.core.adjust_frame_duration(DUR_UP_FACTOR);
        }));

    if let Some(label) = range_label(core, ops) {
        section = section.child(text(label).font(type_::LABEL).color(t.text_secondary));
    }

    // "This frame" row: the current frame's duration as an
    // absolute control (the main row's steppers only multiply
    // from wherever the value happens to be). ×0.5 / ×2 write
    // through the core's relative op; Reset writes the
    // absolute 1.0 via set_duration_here (see its docs).
    let this_frame = row()
        .key("frames-this")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("This frame")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(duration_label(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(fbtn("fop-dur-half", "×0.5", false, !can_duration_down(core), t, |s| {
            s.core.adjust_frame_duration(DUR_DOWN_FACTOR);
        }))
        .child(fbtn("fop-dur-double", "×2", false, !can_duration_up(core), t, |s| {
            s.core.adjust_frame_duration(DUR_UP_FACTOR);
        }))
        .child(fbtn(
            "fop-dur-reset",
            "Reset 1.0",
            false,
            !can_duration_reset(core),
            t,
            |s| {
                set_duration_here(&mut s.core, 1.0);
            },
        ));

    col().w_full().gap(6.0).child(section).child(this_frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops_with_sel(anchor: usize, focus: usize) -> FrameOpsState {
        FrameOpsState {
            range_sel: Some((anchor, focus)),
            ..Default::default()
        }
    }

    #[test]
    fn default_state_has_no_selection_and_nothing_copied() {
        let ops = FrameOpsState::default();
        assert_eq!(ops.range_sel, None);
        assert!(!ops.frame_copied);
        assert!(!ops.range_copied);
    }

    #[test]
    fn selected_range_is_none_without_selection() {
        let core = EditorCore::new();
        assert_eq!(selected_range(&core, &FrameOpsState::default()), None);
    }

    #[test]
    fn selected_range_orders_anchor_and_focus() {
        let core = EditorCore::new();
        assert_eq!(selected_range(&core, &ops_with_sel(3, 1)), Some((1, 3)));
        assert_eq!(selected_range(&core, &ops_with_sel(1, 3)), Some((1, 3)));
        assert_eq!(selected_range(&core, &ops_with_sel(2, 2)), Some((2, 2)));
    }

    #[test]
    fn selected_range_clamps_a_stale_selection() {
        let core = EditorCore::new();
        let n = core.frame_count();
        assert_eq!(
            selected_range(&core, &ops_with_sel(0, 999)),
            Some((0, n - 1)),
            "focus beyond the timeline clamps to the last frame"
        );
    }

    #[test]
    fn copy_range_args_fall_back_to_the_current_frame() {
        let mut core = EditorCore::new();
        core.goto(2);
        assert_eq!(copy_range_args(&core, &FrameOpsState::default()), (2, 1));
    }

    #[test]
    fn copy_range_args_use_the_selection() {
        let core = EditorCore::new();
        assert_eq!(copy_range_args(&core, &ops_with_sel(3, 1)), (1, 3));
    }

    #[test]
    fn reverse_args_cover_the_timeline_without_a_loop() {
        let core = EditorCore::new();
        assert_eq!(reverse_args(&core), (0, core.frame_count()));
    }

    #[test]
    fn reverse_args_use_the_loop_range_when_set() {
        let mut core = EditorCore::new();
        core.set_loop_range(1, 3);
        assert_eq!(reverse_args(&core), (1, 3));
        // A single-frame loop reverses nothing.
        core.set_loop_range(2, 2);
        assert!(!can_reverse(&core));
    }

    #[test]
    fn inbetween_and_shift_edges_follow_the_current_frame() {
        let mut core = EditorCore::new();
        assert!(can_inbetween(&core));
        assert!(!can_shift_back(&core));
        assert!(can_shift_fwd(&core));
        core.goto(core.frame_count() - 1);
        assert!(!can_inbetween(&core));
        assert!(can_shift_back(&core));
        assert!(!can_shift_fwd(&core));
    }

    #[test]
    fn smooth_needs_three_frames() {
        let mut core = EditorCore::new();
        assert!(can_smooth(&core));
        while core.frame_count() > 2 {
            core.current = 0;
            core.delete_frame();
        }
        assert_eq!(core.frame_count(), 2);
        assert!(!can_smooth(&core));
        // Two frames still reverse.
        assert!(can_reverse(&core));
    }

    #[test]
    fn duration_steppers_mute_at_the_core_clamps() {
        let mut core = EditorCore::new();
        assert_eq!(duration_label(&core), "1.00×");
        assert!(can_duration_down(&core));
        assert!(can_duration_up(&core));
        core.project.frames[core.current].duration = DUR_MIN;
        assert!(!can_duration_down(&core));
        assert!(can_duration_up(&core));
        core.project.frames[core.current].duration = DUR_MAX;
        assert!(can_duration_down(&core));
        assert!(!can_duration_up(&core));
        assert_eq!(duration_label(&core), "8.00×");
    }

    #[test]
    fn clamp_duration_mirrors_the_core_ops_range() {
        assert_eq!(clamp_duration(1.0), 1.0);
        assert_eq!(clamp_duration(0.5), 0.5);
        assert_eq!(clamp_duration(0.1), DUR_MIN, "below the floor clamps up");
        assert_eq!(clamp_duration(0.0), DUR_MIN);
        assert_eq!(clamp_duration(100.0), DUR_MAX, "above the ceiling clamps down");
        assert_eq!(clamp_duration(DUR_MIN), DUR_MIN);
        assert_eq!(clamp_duration(DUR_MAX), DUR_MAX);
    }

    #[test]
    fn format_duration_is_two_decimals_with_a_times_sign() {
        assert_eq!(format_duration(1.0), "1.00×");
        assert_eq!(format_duration(0.25), "0.25×");
        assert_eq!(format_duration(2.5), "2.50×");
        assert_eq!(format_duration(8.0), "8.00×");
        // duration_label routes through the same format.
        let core = EditorCore::new();
        assert_eq!(duration_label(&core), format_duration(1.0));
    }

    #[test]
    fn reset_availability_tracks_distance_from_one() {
        let mut core = EditorCore::new();
        assert!(!can_duration_reset(&core), "a fresh frame sits at 1.0");
        core.project.frames[core.current].duration = 2.0;
        assert!(can_duration_reset(&core));
        core.project.frames[core.current].duration = 0.5;
        assert!(can_duration_reset(&core));
        core.project.frames[core.current].duration = 1.0;
        assert!(!can_duration_reset(&core));
    }

    #[test]
    fn set_duration_here_writes_only_the_current_frame() {
        let mut core = EditorCore::new();
        core.goto(1);
        core.project.frames[1].duration = 4.0;
        set_duration_here(&mut core, 1.0);
        assert_eq!(core.project.frames[1].duration, 1.0, "reset lands on 1.0");
        assert_eq!(
            core.project.frames[0].duration, 1.0,
            "neighbouring frames are untouched"
        );
        assert_eq!(core.status, "Frame 2 duration 1.00x.");
        // It is an absolute set, not a factor: from 0.25 the
        // same call still lands exactly on the requested value.
        core.project.frames[1].duration = 0.25;
        set_duration_here(&mut core, 3.0);
        assert_eq!(core.project.frames[1].duration, 3.0);
    }

    #[test]
    fn set_duration_here_clamps_like_the_relative_op() {
        let mut core = EditorCore::new();
        set_duration_here(&mut core, 100.0);
        assert_eq!(core.project.frames[core.current].duration, DUR_MAX);
        set_duration_here(&mut core, 0.01);
        assert_eq!(core.project.frames[core.current].duration, DUR_MIN);
        // And the row's full gesture composes: double to the
        // ceiling through the op, then reset back to 1.0.
        core.project.frames[core.current].duration = 4.0;
        core.adjust_frame_duration(DUR_UP_FACTOR);
        assert_eq!(core.project.frames[core.current].duration, DUR_MAX);
        assert!(can_duration_reset(&core));
        set_duration_here(&mut core, 1.0);
        assert_eq!(core.project.frames[core.current].duration, 1.0);
        assert!(!can_duration_reset(&core));
    }

    #[test]
    fn range_label_prefers_selection_then_loop() {
        let mut core = EditorCore::new();
        let ops = FrameOpsState::default();
        assert_eq!(range_label(&core, &ops), None);
        core.set_loop_range(1, 3);
        assert_eq!(range_label(&core, &ops), Some("Loop 2–4".to_string()));
        assert_eq!(
            range_label(&core, &ops_with_sel(0, 1)),
            Some("Sel 1–2".to_string())
        );
        core.set_loop_range(2, 2);
        assert_eq!(range_label(&core, &ops), Some("Loop 3".to_string()));
    }

    #[test]
    fn handler_math_shifts_the_current_frame() {
        // The shift handlers call shift_frames(cur, 1, ±1) with a
        // status fallback; tag frames by duration to see them move.
        let mut core = EditorCore::new();
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.duration = 1.0 + i as f32;
        }
        core.goto(1);
        assert!(core.shift_frames(core.current, 1, -1));
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations[0], 2.0, "frame 2 moved to position 1");
        assert_eq!(durations[1], 1.0);
        assert_eq!(core.current, 0, "current follows its frame");
        assert!(!core.shift_frames(core.current, 1, -1), "edge shift refuses");
    }

    #[test]
    fn handler_math_copies_and_pastes_a_selected_range() {
        let mut core = EditorCore::new();
        let ops = ops_with_sel(0, 2);
        let (from, count) = copy_range_args(&core, &ops);
        assert!(core.copy_range(from, count));
        let before = core.frame_count();
        assert!(core.paste_range(core.current));
        assert_eq!(core.frame_count(), before + 3);
    }
}
