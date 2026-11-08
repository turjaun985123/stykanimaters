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
//!
//! The "Insert" row (Phase G) inserts N blank frames after
//! the current frame as ONE undo step. "Blank" means exactly
//! what one press of the core's `add_frame` creates — a frame
//! carrying the current frame's poses and duration, with
//! easing reset to `None` and bends emptied — repeated N
//! times; see [`insert_frames_after_current`] for why the
//! batch result is content-identical to N single presses and
//! differs only in undo granularity and in leaving `current`
//! on the original frame.
//!
//! The icon buttons (Phase I, section 25): the Phase H
//! conversion to `icon_btn` lost the would-no-op muted tint
//! the old `fbtn` rendering gave the clipboard, range-delete
//! and insert buttons, because `icon_btn` had no muted form.
//! They now render through the shared
//! `icon_btn::icon_btn_inert` whenever their guard — the
//! `*_enabled` helpers below, each decided from `State` /
//! core state at render time — fails, and through the live
//! `icon_btn` when it passes. Key, icon and label are the
//! same in both forms. The inert form carries NO press
//! handler, so a click on a gated button does nothing at
//! all, not even a refusal status; the live form's handler
//! is byte-for-byte the Phase H one. The plain `fbtn`
//! buttons keep their older muted-but-wired form.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Frame;

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

/// Smallest Insert-N count: inserting zero frames would be a
/// no-op that still spent an undo step.
pub const INSERT_COUNT_MIN: usize = 1;
/// Largest Insert-N count. A whole minute of frames at the
/// demo's 12 fps is 720; 60 is a generous single gesture that
/// still keeps one accidental press recoverable in one undo.
pub const INSERT_COUNT_MAX: usize = 60;
/// Insert-N count a fresh [`FrameOpsState`] starts with.
pub const INSERT_COUNT_DEFAULT: usize = 4;

/// UI-side frame-ops state (see the module docs). Stored on the
/// shell's `State` as `frame_ops`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// How many blank frames the Insert button will add
    /// (Phase G). Kept inside
    /// [`INSERT_COUNT_MIN`]..=[`INSERT_COUNT_MAX`] by
    /// [`step_insert_count`]; defaults to
    /// [`INSERT_COUNT_DEFAULT`]. Not part of the project —
    /// a view-side preference like the range selection.
    pub insert_count: usize,
}

impl Default for FrameOpsState {
    fn default() -> Self {
        FrameOpsState {
            range_sel: None,
            frame_copied: false,
            range_copied: false,
            insert_count: INSERT_COUNT_DEFAULT,
        }
    }
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

/// Clamp an Insert-N count to
/// [`INSERT_COUNT_MIN`]..=[`INSERT_COUNT_MAX`]. The single
/// authority for the range: the stepper, the batch insert and
/// the readout all pass through it.
pub fn clamp_insert_count(count: usize) -> usize {
    count.clamp(INSERT_COUNT_MIN, INSERT_COUNT_MAX)
}

/// One stepper step on the Insert-N count: the stored value is
/// clamped first (so a stale out-of-range value heals), then
/// stepped by `delta` (±1 from the buttons) and clamped again,
/// so stepping at a bound stays at the bound.
pub fn step_insert_count(count: usize, delta: i32) -> usize {
    let stepped = clamp_insert_count(count) as i64 + delta as i64;
    stepped.clamp(INSERT_COUNT_MIN as i64, INSERT_COUNT_MAX as i64) as usize
}

/// The Insert button can act only when there is a current
/// frame to insert after.
pub fn can_insert_frames(core: &EditorCore) -> bool {
    core.project.frames.get(core.current).is_some()
}

/// Copy frame's render-time guard (Phase I): the handler
/// itself no-ops on an empty timeline — it only copies when
/// `frame_count() > 0` — so the button renders inert then.
/// With at least one frame the copy always lands.
pub fn copy_frame_enabled(core: &EditorCore) -> bool {
    core.frame_count() > 0
}

/// Paste frame's render-time guard (Phase I): the core's
/// single-frame clipboard is private with no accessor, so
/// the honest render-time signal is this UI's own
/// [`FrameOpsState::frame_copied`] flag, set by Copy frame
/// in this panel. Until it is set, `paste_frame` could only
/// answer "Nothing copied yet.", so the button renders
/// inert. (A paste can still be refused after a copy if the
/// copied frame's figure count no longer matches; that
/// lives inside the private clipboard and is not checkable
/// at render time, so the live handler keeps that refusal.)
pub fn paste_frame_enabled(ops: &FrameOpsState) -> bool {
    ops.frame_copied
}

/// Copy range's render-time guard (Phase I): the handler's
/// arguments fall back to the current frame alone when no
/// range is selected (see [`copy_range_args`]), and
/// `copy_range` refuses only an empty or out-of-range
/// block, so any non-empty timeline can copy. On an empty
/// timeline the press would change nothing, so the button
/// renders inert.
pub fn copy_range_enabled(core: &EditorCore) -> bool {
    core.frame_count() > 0
}

/// Paste range's render-time guard (Phase I): the mirror
/// of [`paste_frame_enabled`] for the range clipboard,
/// tracked by [`FrameOpsState::range_copied`]. Until a
/// Copy range has landed, `paste_range` could only refuse,
/// so the button renders inert. (The figure-count refusal,
/// as with Paste frame, is not render-checkable and stays
/// with the live handler.)
pub fn paste_range_enabled(ops: &FrameOpsState) -> bool {
    ops.range_copied
}

/// Delete range's render-time guard (Phase I): the handler
/// acts only when a selection exists, and `delete_range`
/// refuses to delete every frame, so the button is live
/// exactly when [`selected_range`] yields a block strictly
/// smaller than the whole timeline. No selection — or a
/// selection covering every frame, including a one-frame
/// project's only frame — renders inert.
pub fn delete_range_enabled(core: &EditorCore, ops: &FrameOpsState) -> bool {
    match selected_range(core, ops) {
        Some((lo, hi)) => hi - lo + 1 < core.frame_count(),
        None => false,
    }
}

/// Insert frames' render-time guard (Phase I): the batch
/// insert refuses without a current frame (see
/// [`insert_frames_after_current`]), which is exactly
/// [`can_insert_frames`]; the wrapper keeps one named
/// guard per icon button.
pub fn insert_frames_enabled(core: &EditorCore) -> bool {
    can_insert_frames(core)
}

/// Insert `count` blank frames directly after the current
/// frame, as ONE undo step, leaving `current` on the original
/// frame. Returns how many frames were inserted (0 when there
/// is no current frame; the status line says why and no
/// snapshot is taken). `count` is clamped through
/// [`clamp_insert_count`].
///
/// What "blank" means, from `EditorCore::add_frame`
/// (editor.rs): a frame whose `poses` and `duration` are
/// cloned from the then-current frame, with `easing: None` and
/// `bends: Vec::new()` — NOT a whole-frame clone. Pressing
/// +Frame N times chains: each press snapshots, inserts that
/// frame after the old current and advances `current` onto
/// it, so press k+1 clones press k's frame — which carries
/// the same poses and duration as the original. Every press
/// therefore inserts a frame identical to the one built here,
/// so building the template once and splicing N copies in
/// yields a project content-identical to the N presses; the
/// only differences are the intended ones (one snapshot
/// instead of N, `current` unmoved instead of advanced, one
/// summary status instead of N).
///
/// Everything that must grow with the timeline, enumerated
/// from the model: `Project::frames` (model.rs:688) is the
/// ONLY per-frame vector on `Project`. Inside each
/// `model::Frame`, `poses` (model.rs:519, parallel to
/// `Project::figures`) is cloned per inserted frame,
/// `duration` (model.rs:522) is copied, and `easing`
/// (model.rs:526) / `bends` (model.rs:531) take the reset
/// values `add_frame` itself writes — so one inserted `Frame`
/// value carries the complete per-frame payload and the
/// splice grows all of it at once. Nothing else on `Project`
/// is indexed by frame position: `camera` (model.rs:690),
/// `particles` (model.rs:701), `audio` (model.rs:705),
/// `fx_clips` (model.rs:709), `channels` (model.rs:714),
/// `sprites` (model.rs:716) and `texts` (model.rs:718) are
/// sparse collections whose entries carry their own absolute
/// frame numbers, and `add_frame` leaves every one of them
/// untouched — so the batch does too, matching single-press
/// behaviour exactly (known core behaviour included: a
/// camera key after the insertion point keeps its absolute
/// frame number under both).
pub fn insert_frames_after_current(core: &mut EditorCore, count: usize) -> usize {
    let n = clamp_insert_count(count);
    let Some(source) = core.project.frames.get(core.current) else {
        core.status = "No frame to insert after.".into();
        return 0;
    };
    // Exactly the Frame value add_frame constructs: poses and
    // duration cloned from the current frame, easing reset to
    // None, bends emptied (see the docs above).
    let blank = Frame {
        poses: source.poses.clone(),
        duration: source.duration,
        easing: None,
        bends: Vec::new(),
    };
    let at = core.current + 1;
    let after = core.current;
    core.snapshot();
    let blanks: Vec<Frame> = (0..n).map(|_| blank.clone()).collect();
    // Collecting the Splice forces the replacement to complete;
    // the replaced range is empty, so nothing is removed.
    let _removed: Vec<Frame> = core.project.frames.splice(at..at, blanks).collect();
    core.status = format!("Inserted {n} frames after frame {}.", after + 1);
    n
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
/// with ×0.5 / ×2 / Reset 1.0 controls (Phase F), and a third
/// row, "Insert", carrying the Insert-N count stepper and the
/// batch insert button (Phase G).
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
        .child(if copy_frame_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn("copy-frame", "copy", "Copy frame", false, t, |s| {
                if s.core.frame_count() > 0 {
                    s.core.copy_frame();
                    s.frame_ops.frame_copied = true;
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("copy-frame", "copy", "Copy frame", t)
        })
        .child(if paste_frame_enabled(ops) {
            crate::rune_sec::icon_btn::icon_btn("paste-frame", "clipboard", "Paste frame", false, t, |s| {
                s.core.paste_frame();
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("paste-frame", "clipboard", "Paste frame", t)
        })
        .child(if copy_range_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn("range-copy", "list", "Copy range", false, t, |s| {
                let (from, count) = copy_range_args(&s.core, &s.frame_ops);
                if s.core.copy_range(from, count) {
                    s.frame_ops.range_copied = true;
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("range-copy", "list", "Copy range", t)
        })
        .child(if paste_range_enabled(ops) {
            crate::rune_sec::icon_btn::icon_btn("range-paste", "list", "Paste range", false, t, |s| {
                let at = s.core.current;
                let before = s.core.status.clone();
                if !s.core.paste_range(at) && s.core.status == before {
                    s.core.status = "Nothing to paste — copy a range first.".into();
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("range-paste", "list", "Paste range", t)
        })
        .child(if delete_range_enabled(core, ops) {
            crate::rune_sec::icon_btn::icon_btn(
                "range-del",
                "trash-can",
                "Delete range",
                false,
                t,
                |s| {
                    if let Some((lo, hi)) = selected_range(&s.core, &s.frame_ops) {
                        if s.core.delete_range(lo, hi - lo + 1) {
                            s.frame_ops.range_sel = None;
                        }
                    }
                },
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("range-del", "trash-can", "Delete range", t)
        })
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
        .child(crate::rune_sec::icons::icon_el("clock", 12.0, t.text_secondary))
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

    // "Insert" row: the count stepper keeps
    // FrameOpsState.insert_count inside 1..=60 (plain presses,
    // no hold-repeat); the Insert button runs the batch insert
    // as one undo step (see insert_frames_after_current).
    let insert_row = row()
        .key("frames-insert")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text("Insert")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(fbtn(
            "fop-ins-n-dn",
            "-",
            false,
            ops.insert_count <= INSERT_COUNT_MIN,
            t,
            |s| {
                s.frame_ops.insert_count = step_insert_count(s.frame_ops.insert_count, -1);
            },
        ))
        .child(
            text(format!("{} frames", clamp_insert_count(ops.insert_count)))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(fbtn(
            "fop-ins-n-up",
            "+",
            false,
            ops.insert_count >= INSERT_COUNT_MAX,
            t,
            |s| {
                s.frame_ops.insert_count = step_insert_count(s.frame_ops.insert_count, 1);
            },
        ))
        .child(if insert_frames_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn(
                "fop-ins-n",
                "plus",
                "Insert frames",
                false,
                t,
                |s| {
                    let count = s.frame_ops.insert_count;
                    insert_frames_after_current(&mut s.core, count);
                },
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("fop-ins-n", "plus", "Insert frames", t)
        });

    col()
        .w_full()
        .gap(6.0)
        .child(section)
        .child(this_frame)
        .child(insert_row)
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
        assert_eq!(
            ops.insert_count, INSERT_COUNT_DEFAULT,
            "the Insert-N stepper starts at the documented default"
        );
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

    #[test]
    fn insert_count_clamps_to_one_through_sixty() {
        assert_eq!(clamp_insert_count(0), INSERT_COUNT_MIN, "zero clamps up");
        assert_eq!(clamp_insert_count(1), 1);
        assert_eq!(clamp_insert_count(INSERT_COUNT_DEFAULT), 4);
        assert_eq!(clamp_insert_count(60), 60);
        assert_eq!(clamp_insert_count(61), INSERT_COUNT_MAX, "above clamps down");
        assert_eq!(clamp_insert_count(usize::MAX), INSERT_COUNT_MAX);
    }

    #[test]
    fn insert_count_stepper_steps_by_one_and_sticks_at_the_bounds() {
        assert_eq!(step_insert_count(4, -1), 3);
        assert_eq!(step_insert_count(4, 1), 5);
        assert_eq!(step_insert_count(INSERT_COUNT_MIN, -1), INSERT_COUNT_MIN);
        assert_eq!(step_insert_count(INSERT_COUNT_MAX, 1), INSERT_COUNT_MAX);
        // A stale out-of-range stored value heals on the next step.
        assert_eq!(step_insert_count(0, -1), INSERT_COUNT_MIN);
        assert_eq!(step_insert_count(999, 1), INSERT_COUNT_MAX);
    }

    #[test]
    fn batch_insert_content_matches_repeated_add_frame() {
        // Two identical cores; the source frame is given a
        // distinctive duration, easing and bends so the test
        // proves WHICH parts add_frame copies (poses +
        // duration) and which it resets (easing, bends).
        let mut batch = EditorCore::new();
        let mut singles = EditorCore::new();
        for core in [&mut batch, &mut singles] {
            core.goto(1);
            core.project.frames[1].duration = 2.5;
            core.project.frames[1].easing = Some(stykanimate_rs::model::Easing::EaseIn);
            core.project.frames[1].bends = vec![vec![0.25, -0.5]];
        }
        assert_eq!(insert_frames_after_current(&mut batch, 4), 4);
        for _ in 0..4 {
            singles.add_frame();
        }
        assert_eq!(batch.frame_count(), singles.frame_count());
        assert_eq!(
            serde_json::to_string(&batch.project).unwrap(),
            serde_json::to_string(&singles.project).unwrap(),
            "batch content = N single adds, exactly"
        );
        // The inserted frames sit directly after the original
        // current frame, carrying its poses + duration with
        // easing/bends reset, as add_frame constructs them.
        for f in &batch.project.frames[2..6] {
            assert_eq!(f.duration, 2.5);
            assert_eq!(f.easing, None);
            assert!(f.bends.is_empty());
        }
        // The accepted differences: current stays on the
        // original frame (single adds advance onto each new
        // frame) and the status is the batch summary.
        assert_eq!(batch.current, 1);
        assert_eq!(singles.current, 5);
        assert_eq!(batch.status, "Inserted 4 frames after frame 2.");
    }

    #[test]
    fn batch_insert_is_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        let before = serde_json::to_string(&core.project).unwrap();
        let depth_before = core.undo_depth();
        assert_eq!(insert_frames_after_current(&mut core, 6), 6);
        assert_eq!(core.undo_depth(), depth_before + 1, "one snapshot for the batch");
        core.undo();
        assert_eq!(
            serde_json::to_string(&core.project).unwrap(),
            before,
            "a single undo removes the whole batch"
        );
        // Contrast: the same six frames via single adds cost
        // six undo steps.
        let mut singles = EditorCore::new();
        let d = singles.undo_depth();
        for _ in 0..6 {
            singles.add_frame();
        }
        assert_eq!(singles.undo_depth(), d + 6);
    }

    #[test]
    fn batch_insert_at_the_last_frame_appends() {
        let mut core = EditorCore::new();
        let last = core.frame_count() - 1;
        core.goto(last);
        assert_eq!(insert_frames_after_current(&mut core, 3), 3);
        assert_eq!(core.frame_count(), last + 1 + 3);
        assert_eq!(core.current, last, "current stays on the original last frame");
        assert_eq!(
            core.status,
            format!("Inserted 3 frames after frame {}.", last + 1)
        );
    }

    #[test]
    fn batch_insert_clamps_its_count_like_the_stepper() {
        let mut core = EditorCore::new();
        let before = core.frame_count();
        assert_eq!(insert_frames_after_current(&mut core, 10_000), INSERT_COUNT_MAX);
        assert_eq!(core.frame_count(), before + INSERT_COUNT_MAX);
    }

    #[test]
    fn batch_insert_without_a_current_frame_refuses_without_a_snapshot() {
        let mut core = EditorCore::new();
        core.project.frames.clear();
        assert!(!can_insert_frames(&core));
        let depth = core.undo_depth();
        assert_eq!(insert_frames_after_current(&mut core, 4), 0);
        assert_eq!(core.undo_depth(), depth, "a refused insert spends no undo step");
        assert_eq!(core.status, "No frame to insert after.");
    }

    #[test]
    fn copy_frame_guard_needs_at_least_one_frame() {
        // Truth table for Copy frame's inert decision: live
        // whenever a frame exists to copy, inert on an empty
        // timeline (where the handler itself no-ops).
        let mut core = EditorCore::new();
        assert!(copy_frame_enabled(&core), "the demo timeline can copy");
        core.project.frames.clear();
        assert!(!copy_frame_enabled(&core), "nothing to copy from no frames");
    }

    #[test]
    fn paste_frame_guard_follows_the_frame_copied_flag() {
        // Truth table for Paste frame: inert until this panel
        // has run Copy frame, live from then on.
        let mut ops = FrameOpsState::default();
        assert!(!paste_frame_enabled(&ops), "nothing copied yet");
        ops.frame_copied = true;
        assert!(paste_frame_enabled(&ops), "a copy in this panel arms paste");
        ops.frame_copied = false;
        assert!(!paste_frame_enabled(&ops), "the flag is the whole guard");
    }

    #[test]
    fn copy_range_guard_needs_at_least_one_frame() {
        // Truth table for Copy range: with no selection the
        // handler falls back to the current frame, so any
        // non-empty timeline is live; empty is inert.
        let mut core = EditorCore::new();
        assert!(copy_range_enabled(&core), "the current frame alone copies");
        core.project.frames.clear();
        assert!(!copy_range_enabled(&core), "no frame to fall back to");
    }

    #[test]
    fn paste_range_guard_follows_the_range_copied_flag() {
        // Truth table for Paste range: inert until this panel
        // has run Copy range, live from then on — and the
        // single-frame flag does not arm it.
        let mut ops = FrameOpsState::default();
        assert!(!paste_range_enabled(&ops), "nothing copied yet");
        ops.frame_copied = true;
        assert!(
            !paste_range_enabled(&ops),
            "Copy frame arms Paste frame, not Paste range"
        );
        ops.range_copied = true;
        assert!(paste_range_enabled(&ops), "a range copy arms range paste");
    }

    #[test]
    fn delete_range_guard_needs_a_partial_selection() {
        // Truth table for Delete range over the demo's five
        // frames: no selection is inert, a strict sub-range
        // is live, the whole timeline is inert (the core
        // refuses to delete every frame).
        let mut core = EditorCore::new();
        assert_eq!(core.frame_count(), 5, "the demo timeline this table assumes");
        assert!(
            !delete_range_enabled(&core, &FrameOpsState::default()),
            "no selection, nothing for the handler to act on"
        );
        assert!(
            delete_range_enabled(&core, &ops_with_sel(1, 3)),
            "a three-frame selection in five frames deletes"
        );
        assert!(
            delete_range_enabled(&core, &ops_with_sel(2, 2)),
            "a single selected frame deletes while others remain"
        );
        assert!(
            !delete_range_enabled(&core, &ops_with_sel(0, 4)),
            "selecting every frame would delete the whole timeline"
        );
        // A one-frame project: its only frame can never be
        // range-deleted, selected or not.
        core.project.frames.truncate(1);
        assert!(
            !delete_range_enabled(&core, &ops_with_sel(0, 0)),
            "the only frame is the whole timeline"
        );
        assert!(
            !delete_range_enabled(&core, &FrameOpsState::default()),
            "and no selection is still inert"
        );
    }

    #[test]
    fn insert_frames_guard_needs_a_current_frame() {
        // Truth table for Insert frames: live while a current
        // frame exists to insert after, inert on an empty
        // timeline (where the batch insert refuses).
        let mut core = EditorCore::new();
        assert!(insert_frames_enabled(&core));
        assert_eq!(insert_frames_enabled(&core), can_insert_frames(&core));
        core.project.frames.clear();
        assert!(!insert_frames_enabled(&core), "no frame to insert after");
    }
}
