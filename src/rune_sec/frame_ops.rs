//! Frames toolbar section for the Rune editor shell
//! (Phase D, section 8/40 — frame ops).
//!
//! Inbetween, single-frame shift, reverse, motion-path
//! smoothing, the single-frame and range clipboards, and
//! per-frame duration steppers — every control wired
//! to the real `EditorCore` op the GPUI app's "Frames" tool
//! group (and Edit cluster) called, with the same semantics and,
//! where one existed, the same element key. The loop range is
//! only READ here (Reverse's range rule and the section
//! readout); loop EDITING lives in the timeline section, its
//! single home since UI-mission Wave 5 — no loop-editing
//! control may be added to this section.
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
//!
//! "Reverse all" (Phase J) sits beside "Reverse" in the
//! main row and is the deliberate exception to the range
//! model above: it always reverses the whole timeline,
//! ignoring both the loop range and the timeline
//! selection — that is what its label promises, while
//! "Reverse" keeps the loop-range rule for the range
//! itself. It renders through the Phase I inert/live icon
//! pair, gated by [`reverse_all_enabled`]: below two
//! frames there is no order to change.
//!
//! "Swap with previous" / "Swap with next" (Phase M) sit
//! beside "Reverse all" and exchange the current frame
//! with its neighbour through the lib op
//! `frame_swap::swap_frame_with_neighbour`, which leaves
//! `current` numerically unchanged while the two frames'
//! contents exchange. They render through the same
//! inert/live icon pair, gated by [`swap_prev_enabled`] /
//! [`swap_next_enabled`]: at the timeline's ends there is
//! no neighbour to swap with.
//!
//! Phase N adoption: the section is a kit card —
//! `design::panel_col` under the section's existing
//! `frames` key (which moves from the main row to the
//! card root; the key set is unchanged), a
//! `design::panel_title` header over a `design::divider`,
//! and kit section labels. The local `fbtn` helper is
//! retired onto the button family: live sites render
//! through `chrome_view::btn`, and muted sites through
//! `chrome_view::btn_muted_live` — `fbtn`'s muted branch
//! never dropped its handler (the press still ran the op
//! and its refusal status was the designed feedback), so
//! by P05's migration rule every muted site is
//! muted-live and none becomes inert; the only visual
//! change is the resting fill (`control_off` → the
//! family's card fill, P05 §4). The duration and
//! Insert-N readouts become `design::stepper_value` in
//! the charter stepper order (label, −, value, +). The
//! icon buttons' Phase I inert/live pairs are already
//! family and are untouched, as is the main row's
//! `flex_wrap` (the Phase M pressability fix).
//!
//! Wave P (P49, toolbar density): the main row's single
//! wrapping row of ~17 controls — the wall of small
//! buttons the user's screenshot verdict named — is
//! regrouped into labelled wrapping groups (Arrange /
//! Path / Clipboard / Dur) on the kit idiom the audio
//! operations toolbar's Wave 4 regroup established.
//! Arrangement only: every key, label, glyph, handler
//! and gating predicate is verbatim; the group rows
//! carry additive container keys only. The "This frame"
//! and "Insert" rows already read as single-purpose
//! labelled groups in the charter stepper order and are
//! unchanged.
//!
//! Wave T (T3): every label in the section resolves
//! through `i18n_wire::tr` under the `frame_*` keys
//! (IA-T3-1) — the buttons, the group headings, the
//! panel title and the "This frame" / "Insert" row
//! labels, English byte-identical. Status sentences
//! and pure notation (×0.5 / ×2, the − / + steppers)
//! stay literal, as in the subtitle/camera/3D panels.
//!
//! Wave U (U6): the composed readouts the Wave T pass
//! left behind now resolve through `{slot}` template
//! keys (IA-U6-1), substituted with `.replace()` at
//! the composing site (the camera `follow_label`
//! precedent) — the Sel/Loop range readout
//! (`frame_sel_*` / `frame_loop_*`), the duration
//! multiplier (`frame_duration_value`) and the
//! Insert-N count (`frame_insert_count`), English
//! byte-identical. Status sentences and the remaining
//! notation (the `—` no-frame marker, the two-decimal
//! number format inside the duration slot) stay
//! literal.

use rune::tok::{type_, Theme};
use rune::{row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Frame;

use crate::chrome_view::{btn, btn_muted_live};
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Whether the timeline card is collapsed to its
    /// frame-chips row (Focused Wave, D1). The timeline
    /// section's chrome state: `timeline_marks` reads it
    /// on every build — collapsed hides the ruler, the
    /// marker row, the clips lane, the dopesheet lane
    /// and the loop row, and keeps the chips, the old
    /// GPUI shell's collapsed form (the status strip is
    /// a separate centre child and always renders). Its
    /// persisted mirror is
    /// `LayoutPrefs::timeline_collapsed`: `main.rs`'s
    /// `init` seeds this field from the prefs and `poll`
    /// writes changes back, so this field is the live
    /// source of truth and the prefs field never leads.
    /// It lives here, not on `State`, because the
    /// timeline section already owns this struct and the
    /// `State` field census is integrator-gated.
    pub timeline_collapsed: bool,
    /// Whether the timeline card's marker row is shown
    /// (Roadmap Wave 1, RM12 — F-11's timeline space
    /// model). One flag per data-gated lane of the card:
    /// this one, [`FrameOpsState::show_clips_lane`],
    /// [`FrameOpsState::show_dopesheet_lane`] and
    /// [`FrameOpsState::show_subtitle_lane`]. A flag is
    /// the USER's half of its lane's mount rule — the
    /// lane mounts only when its data exists AND its
    /// flag is set (the rule is
    /// `timeline_marks::lane_mounted`; the builder is
    /// the only reader). The ruler and the frame chips
    /// carry no flag: they are the card's spine, mounted
    /// in every form (the collapsed card is chips-only).
    /// All four default to shown, so a state that
    /// predates the flags behaves exactly as the card
    /// always has. They are flipped by the card's own
    /// lane row (`timeline_marks::lane_toggles_row`) and
    /// by nothing else; a flip is a view preference —
    /// no snapshot, no dirty mark, the
    /// `timeline_collapsed` discipline.
    ///
    /// Session-only for now: the persisted mirror in
    /// `LayoutPrefs` is recorded as IA-RM12-2 (this wave
    /// the prefs struct region belongs to RM10's F-9
    /// seat), so until that lands a restart restores
    /// every lane — the same asymmetry `State::show_right`
    /// has always had. They live here, beside
    /// `timeline_collapsed`, for the reason that field
    /// records.
    pub show_marker_lane: bool,
    /// The clips lane's visibility flag — see
    /// [`FrameOpsState::show_marker_lane`].
    pub show_clips_lane: bool,
    /// The dopesheet lane's visibility flag — see
    /// [`FrameOpsState::show_marker_lane`]. The lane's
    /// other gate is the Animate workspace (its builder
    /// is mounted from the timeline builder's call
    /// site); this flag is the user's half only.
    pub show_dopesheet_lane: bool,
    /// The subtitle lane's visibility flag — see
    /// [`FrameOpsState::show_marker_lane`].
    pub show_subtitle_lane: bool,
    /// The timeline's chip width in points (Roadmap
    /// Wave 1, RM13 — P-1 timeline zoom). The ONE zoom
    /// value every timeline-geometry consumer reads
    /// through [`FrameOpsState::chip_w`], which clamps
    /// to the timeline's zoom range; the raw field is
    /// written only through [`FrameOpsState::set_zoom`]
    /// / [`FrameOpsState::zoom_step`], the zoom row and
    /// the `init` seed. Its persisted mirror is
    /// `LayoutPrefs::timeline_zoom` (the
    /// `timeline_collapsed` discipline: this field is
    /// the live source of truth, the prefs field never
    /// leads). Defaults to `timeline_marks::CHIP_W` —
    /// the pre-zoom geometry, so a state that predates
    /// the zoom behaves exactly as the card always has.
    pub timeline_zoom: f32,
    /// Whether the timeline follows the playhead during
    /// playback (Roadmap Wave 1, RM13 — P-1): while
    /// playing, `main.rs`'s `poll` scrolls the strip to
    /// keep the current frame's chip in view (the pure
    /// half is `timeline_marks::chip_visible`). Its
    /// persisted mirror is `LayoutPrefs::timeline_follow`.
    pub timeline_follow: bool,
}

impl Default for FrameOpsState {
    fn default() -> Self {
        FrameOpsState {
            range_sel: None,
            frame_copied: false,
            range_copied: false,
            insert_count: INSERT_COUNT_DEFAULT,
            timeline_collapsed: false,
            show_marker_lane: true,
            show_clips_lane: true,
            show_dopesheet_lane: true,
            show_subtitle_lane: true,
            timeline_zoom: crate::rune_sec::timeline_marks::CHIP_W,
            timeline_follow: false,
        }
    }
}

/// One data-gated lane of the timeline card (Roadmap
/// Wave 1, RM12 — F-11's timeline space model): the
/// unit the per-lane visibility flags on
/// [`FrameOpsState`] address. The ruler and the frame
/// chips are not lanes — they are the card's spine and
/// carry no flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineLane {
    /// The marker row (`timeline-markers`): camera /
    /// graph markers, data-gated on markers existing.
    Markers,
    /// The clips lane (`timeline-clips`): audio / FX
    /// clip spans, data-gated on clips existing.
    Clips,
    /// The dopesheet lane (`dopesheet-lane`):
    /// workspace-gated on Animate.
    Dopesheet,
    /// The subtitle lane (`subtitle-lane`): cue
    /// blocks, data-gated on cues existing.
    Subtitles,
}

impl FrameOpsState {
    /// Whether the user has `lane` shown. The lane's
    /// data gate is the other half of the mount rule
    /// (see `timeline_marks::lane_mounted`).
    pub fn lane_shown(&self, lane: TimelineLane) -> bool {
        match lane {
            TimelineLane::Markers => self.show_marker_lane,
            TimelineLane::Clips => self.show_clips_lane,
            TimelineLane::Dopesheet => self.show_dopesheet_lane,
            TimelineLane::Subtitles => self.show_subtitle_lane,
        }
    }

    /// Flip `lane`'s visibility flag and return the new
    /// value. A view preference: no snapshot, no dirty
    /// mark — the `timeline_collapsed` discipline.
    pub fn toggle_lane(&mut self, lane: TimelineLane) -> bool {
        let flag = match lane {
            TimelineLane::Markers => &mut self.show_marker_lane,
            TimelineLane::Clips => &mut self.show_clips_lane,
            TimelineLane::Dopesheet => &mut self.show_dopesheet_lane,
            TimelineLane::Subtitles => &mut self.show_subtitle_lane,
        };
        *flag = !*flag;
        *flag
    }

    /// The live chip width in points: the stored zoom
    /// clamped to the timeline's zoom range
    /// (`timeline_marks::ZOOM_MIN_W..=ZOOM_MAX_W`), or
    /// the pre-zoom `timeline_marks::CHIP_W` when the
    /// stored value is not finite (a hand-edited prefs
    /// file is the only way that happens —
    /// [`FrameOpsState::set_zoom`] refuses non-finite
    /// writes). Every geometry consumer reads through
    /// here, never the raw field.
    pub fn chip_w(&self) -> f32 {
        if self.timeline_zoom.is_finite() {
            self.timeline_zoom.clamp(
                crate::rune_sec::timeline_marks::ZOOM_MIN_W,
                crate::rune_sec::timeline_marks::ZOOM_MAX_W,
            )
        } else {
            crate::rune_sec::timeline_marks::CHIP_W
        }
    }

    /// Set the zoom to `w` points per chip, clamped to
    /// the zoom range; a non-finite `w` keeps the
    /// current value. A view preference: no snapshot,
    /// no dirty mark — the `timeline_collapsed`
    /// discipline; `poll` persists it.
    pub fn set_zoom(&mut self, w: f32) {
        if w.is_finite() {
            self.timeline_zoom = w.clamp(
                crate::rune_sec::timeline_marks::ZOOM_MIN_W,
                crate::rune_sec::timeline_marks::ZOOM_MAX_W,
            );
        }
    }

    /// Step the zoom one notch: `dir > 0` zooms in,
    /// `dir < 0` zooms out, by the zoom step factor
    /// (`timeline_marks::ZOOM_STEP`); `dir == 0` does
    /// nothing.
    pub fn zoom_step(&mut self, dir: i32) {
        if dir > 0 {
            self.set_zoom(self.timeline_zoom * crate::rune_sec::timeline_marks::ZOOM_STEP);
        } else if dir < 0 {
            self.set_zoom(self.timeline_zoom / crate::rune_sec::timeline_marks::ZOOM_STEP);
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

/// Reverse-all's render-time guard (Phase J): reversing the
/// whole timeline needs at least two frames to change
/// anything; below that the press would be a no-op, so the
/// button renders inert. Unlike [`can_reverse`] — the
/// "Reverse" button's predicate, which follows
/// [`reverse_args`]' loop-range rule — this guard looks at
/// the whole timeline only, because Reverse all always
/// acts on the whole timeline (see [`reverse_all`]).
pub fn reverse_all_enabled(core: &EditorCore) -> bool {
    core.frame_count() >= 2
}

/// Reverse the WHOLE timeline — the "Reverse all" button's
/// dispatch. Deliberately NOT [`reverse_args`]: a set loop
/// range and any timeline selection are both ignored, so
/// the button always does what its label says. ("Reverse"
/// keeps the loop-range rule for the range itself.)
pub fn reverse_all(core: &mut EditorCore) {
    core.reverse_range(0, core.frame_count());
}

/// Swap-with-previous's render-time guard (Phase M): the
/// current frame needs a frame before it to swap with, so
/// the button renders inert on frame 0 — and on an empty
/// project, where `current` is 0 as well.
pub fn swap_prev_enabled(core: &EditorCore) -> bool {
    core.current > 0
}

/// Swap-with-next's render-time guard (Phase M): the
/// current frame needs a frame after it to swap with, so
/// the button renders inert on the last frame — and on an
/// empty project, where there is no next frame either.
pub fn swap_next_enabled(core: &EditorCore) -> bool {
    core.current + 1 < core.frame_count()
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
/// Wave U (U6): the readout resolves through the
/// `frame_duration_value` template (`{value}×`) — the
/// two-decimal number is the `{value}` slot, formatted
/// here, and the `×` rides in the template so a locale
/// can place it differently. English byte-identical
/// once IA-U6-1 lands.
pub fn format_duration(core: &EditorCore, d: f32) -> String {
    i18n_wire::tr(core, "frame_duration_value").replace("{value}", &format!("{d:.2}"))
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
        Some(d) => format_duration(core, d),
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
    // Wave U (U6): the readout resolves through the
    // `frame_sel_*` / `frame_loop_*` template keys
    // (IA-U6-1), slots replaced here at the composing
    // site — the single-frame and range forms are
    // separate keys, as their shapes differ (`Sel 3`
    // vs `Sel 3–5`). English byte-identical once the
    // IA lands.
    if let Some((lo, hi)) = selected_range(core, ops) {
        return Some(if lo == hi {
            i18n_wire::tr(core, "frame_sel_one").replace("{frame}", &(lo + 1).to_string())
        } else {
            i18n_wire::tr(core, "frame_sel_range")
                .replace("{from}", &(lo + 1).to_string())
                .replace("{to}", &(hi + 1).to_string())
        });
    }
    if let Some((a, b)) = core.loop_range {
        return Some(if a == b {
            i18n_wire::tr(core, "frame_loop_one").replace("{frame}", &(a + 1).to_string())
        } else {
            i18n_wire::tr(core, "frame_loop_range")
                .replace("{from}", &(a + 1).to_string())
                .replace("{to}", &(b + 1).to_string())
        });
    }
    None
}

/// The "Frames" toolbar section: the controls in GPUI
/// tool-group order, seated in labelled wrapping groups
/// (Wave P, P49 — the kit grouping idiom the audio
/// operations toolbar's Wave 4 regroup established):
/// "Arrange" (Inbetween / Shift / Reverse / Swap),
/// "Path" (Smooth path), "Clipboard" (the frame and
/// range clipboards plus Delete range, with the Sel/Loop
/// readout seated under the group) and "Dur" (the
/// duration steppers with their readout) — then the
/// "This frame" row, carrying the current frame's
/// duration multiplier with ×0.5 / ×2 / Reset 1.0
/// controls (Phase F), and the "Insert" row, carrying
/// the Insert-N count stepper and the batch insert
/// button (Phase G). Every control's key, label, glyph
/// and handler is verbatim from the pre-regroup single
/// wrapping row; only the row containers changed (the
/// group rows carry the additive container keys
/// `frames-arrange` / `frames-path` / `frames-clipboard`
/// / `frames-dur`).
pub fn frames_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let ops = &s.frame_ops;
    let n = core.frame_count();

    // Wave P (P49): the pre-regroup main row carried ~17
    // controls in GPUI tool-group order in ONE wrapping
    // row — far wider than the side column it lives in,
    // so it wrapped into an undifferentiated wall of
    // small buttons (the density the user's screenshot
    // verdict named). The controls now sit in labelled
    // wrapping groups, the kit idiom the audio toolbar's
    // Wave 4 regroup established: a `design::section_label`
    // heading a keyed `flex_wrap` row of only that group's
    // controls (the panel assembly below seats the labels
    // and rows). Wrapping itself stays — it keeps every
    // button inside the column (and pressable);
    // unwrapped, a row's tail overflowed past the panel
    // edge and its buttons could not be pressed at all
    // (Phase M's swap test proved this against the
    // clipped row). Group rows take the grouped-row gap
    // (6.0, as in the audio and keyframe groups); the
    // panel's own gap separates label/row pairs.
    let arrange = row()
        .key("frames-arrange")
        .w_full()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child({
            // Muted-live dispatch (Phase N): fbtn kept the
            // handler in both states, so the gated form is
            // btn_muted_live, never inert — the handler is
            // bound once and moved into whichever branch
            // renders.
            let press = |s: &mut State| {
                s.core.insert_inbetween();
            };
            if can_inbetween(core) {
                btn("inbetween", &i18n_wire::tr(core, "frame_inbetween"), false, t, press)
            } else {
                btn_muted_live("inbetween", &i18n_wire::tr(core, "frame_inbetween"), false, t, press)
            }
        })
        .child({
            let press = |s: &mut State| {
                let cur = s.core.current;
                if !s.core.shift_frames(cur, 1, -1) {
                    s.core.status = "First frame: cannot shift it earlier.".into();
                }
            };
            if can_shift_back(core) {
                btn("shift-back", &i18n_wire::tr(core, "frame_shift_back"), false, t, press)
            } else {
                btn_muted_live("shift-back", &i18n_wire::tr(core, "frame_shift_back"), false, t, press)
            }
        })
        .child({
            let press = |s: &mut State| {
                let cur = s.core.current;
                if !s.core.shift_frames(cur, 1, 1) {
                    s.core.status = "Last frame: cannot shift it later.".into();
                }
            };
            if can_shift_fwd(core) {
                btn("shift-fwd", &i18n_wire::tr(core, "frame_shift_fwd"), false, t, press)
            } else {
                btn_muted_live("shift-fwd", &i18n_wire::tr(core, "frame_shift_fwd"), false, t, press)
            }
        })
        .child({
            let press = |s: &mut State| {
                let (from, count) = reverse_args(&s.core);
                s.core.reverse_range(from, count);
            };
            if can_reverse(core) {
                btn("reverse-range", &i18n_wire::tr(core, "frame_reverse"), false, t, press)
            } else {
                btn_muted_live("reverse-range", &i18n_wire::tr(core, "frame_reverse"), false, t, press)
            }
        })
        .child(if reverse_all_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn(
                "frames-reverse-all",
                "arrow-right-arrow-left",
                &i18n_wire::tr(core, "frame_reverse_all"),
                false,
                t,
                |s| reverse_all(&mut s.core),
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert(
                "frames-reverse-all",
                "arrow-right-arrow-left",
                &i18n_wire::tr(core, "frame_reverse_all"),
                t,
            )
        })
        .child(if swap_prev_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn(
                "fop-swap-prev",
                "chevron-left",
                &i18n_wire::tr(core, "frame_swap_prev"),
                false,
                t,
                |s| {
                    stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut s.core, -1);
                },
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert(
                "fop-swap-prev",
                "chevron-left",
                &i18n_wire::tr(core, "frame_swap_prev"),
                t,
            )
        })
        .child(if swap_next_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn(
                "fop-swap-next",
                "chevron-right",
                &i18n_wire::tr(core, "frame_swap_next"),
                false,
                t,
                |s| {
                    stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut s.core, 1);
                },
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert(
                "fop-swap-next",
                "chevron-right",
                &i18n_wire::tr(core, "frame_swap_next"),
                t,
            )
        });

    let path = row()
        .key("frames-path")
        .w_full()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child({
            let press = |s: &mut State| {
                let fi = s.core.selected_figure;
                s.core.smooth_path_roots(fi);
            };
            if can_smooth(core) {
                btn("smooth-path", &i18n_wire::tr(core, "frame_smooth_path"), false, t, press)
            } else {
                btn_muted_live("smooth-path", &i18n_wire::tr(core, "frame_smooth_path"), false, t, press)
            }
        });

    let clipboard = row()
        .key("frames-clipboard")
        .w_full()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child(if copy_frame_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn("copy-frame", "copy", &i18n_wire::tr(core, "frame_copy_frame"), false, t, |s| {
                if s.core.frame_count() > 0 {
                    s.core.copy_frame();
                    s.frame_ops.frame_copied = true;
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("copy-frame", "copy", &i18n_wire::tr(core, "frame_copy_frame"), t)
        })
        .child(if paste_frame_enabled(ops) {
            crate::rune_sec::icon_btn::icon_btn("paste-frame", "clipboard", &i18n_wire::tr(core, "frame_paste_frame"), false, t, |s| {
                s.core.paste_frame();
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("paste-frame", "clipboard", &i18n_wire::tr(core, "frame_paste_frame"), t)
        })
        .child(if copy_range_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn("range-copy", "list", &i18n_wire::tr(core, "frame_copy_range"), false, t, |s| {
                let (from, count) = copy_range_args(&s.core, &s.frame_ops);
                if s.core.copy_range(from, count) {
                    s.frame_ops.range_copied = true;
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("range-copy", "list", &i18n_wire::tr(core, "frame_copy_range"), t)
        })
        .child(if paste_range_enabled(ops) {
            crate::rune_sec::icon_btn::icon_btn("range-paste", "list", &i18n_wire::tr(core, "frame_paste_range"), false, t, |s| {
                let at = s.core.current;
                let before = s.core.status.clone();
                if !s.core.paste_range(at) && s.core.status == before {
                    s.core.status = "Nothing to paste — copy a range first.".into();
                }
            })
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("range-paste", "list", &i18n_wire::tr(core, "frame_paste_range"), t)
        })
        .child(if delete_range_enabled(core, ops) {
            crate::rune_sec::icon_btn::icon_btn(
                "range-del",
                "trash-can",
                &i18n_wire::tr(core, "frame_delete_range"),
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
            crate::rune_sec::icon_btn::icon_btn_inert("range-del", "trash-can", &i18n_wire::tr(core, "frame_delete_range"), t)
        });

    // The duration steppers keep their charter order
    // (−, value, +) in their own group row; the "Dur"
    // label that rode inline in the old wrapping row
    // heads this group from above instead (its text is
    // verbatim — see the panel assembly below), the
    // parallel structure every other kit group uses.
    let dur = row()
        .key("frames-dur")
        .w_full()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child({
            let press = |s: &mut State| {
                s.core.adjust_frame_duration(DUR_DOWN_FACTOR);
            };
            if can_duration_down(core) {
                btn("dur-dn", &i18n_wire::tr(core, "frame_dur_down"), false, t, press)
            } else {
                btn_muted_live("dur-dn", &i18n_wire::tr(core, "frame_dur_down"), false, t, press)
            }
        })
        .child(design::stepper_value(&duration_label(core), t))
        .child({
            let press = |s: &mut State| {
                s.core.adjust_frame_duration(DUR_UP_FACTOR);
            };
            if can_duration_up(core) {
                btn("dur-up", &i18n_wire::tr(core, "frame_dur_up"), false, t, press)
            } else {
                btn_muted_live("dur-up", &i18n_wire::tr(core, "frame_dur_up"), false, t, press)
            }
        });

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
        .child(design::section_label(&i18n_wire::tr(core, "frame_this_frame"), t).grow(1.0))
        .child(design::stepper_value(&duration_label(core), t))
        .child({
            let press = |s: &mut State| {
                s.core.adjust_frame_duration(DUR_DOWN_FACTOR);
            };
            if can_duration_down(core) {
                btn("fop-dur-half", "×0.5", false, t, press)
            } else {
                btn_muted_live("fop-dur-half", "×0.5", false, t, press)
            }
        })
        .child({
            let press = |s: &mut State| {
                s.core.adjust_frame_duration(DUR_UP_FACTOR);
            };
            if can_duration_up(core) {
                btn("fop-dur-double", "×2", false, t, press)
            } else {
                btn_muted_live("fop-dur-double", "×2", false, t, press)
            }
        })
        .child({
            let press = |s: &mut State| {
                set_duration_here(&mut s.core, 1.0);
            };
            if can_duration_reset(core) {
                btn("fop-dur-reset", &i18n_wire::tr(core, "frame_dur_reset"), false, t, press)
            } else {
                btn_muted_live("fop-dur-reset", &i18n_wire::tr(core, "frame_dur_reset"), false, t, press)
            }
        });

    // "Insert" row: the count stepper keeps
    // FrameOpsState.insert_count inside 1..=60 (plain presses,
    // no hold-repeat); the Insert button runs the batch insert
    // as one undo step (see insert_frames_after_current).
    let insert_row = row()
        .key("frames-insert")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(design::section_label(&i18n_wire::tr(core, "frame_insert"), t).grow(1.0))
        .child({
            // Muted-live, as everywhere fbtn was muted:
            // the handler stays (a press at the bound runs
            // step_insert_count, which clamps — and heals
            // a stale out-of-range stored value).
            let press = |s: &mut State| {
                s.frame_ops.insert_count = step_insert_count(s.frame_ops.insert_count, -1);
            };
            if ops.insert_count <= INSERT_COUNT_MIN {
                btn_muted_live("fop-ins-n-dn", "-", false, t, press)
            } else {
                btn("fop-ins-n-dn", "-", false, t, press)
            }
        })
        .child(design::stepper_value(
            &i18n_wire::tr(core, "frame_insert_count")
                .replace("{count}", &clamp_insert_count(ops.insert_count).to_string()),
            t,
        ))
        .child({
            let press = |s: &mut State| {
                s.frame_ops.insert_count = step_insert_count(s.frame_ops.insert_count, 1);
            };
            if ops.insert_count >= INSERT_COUNT_MAX {
                btn_muted_live("fop-ins-n-up", "+", false, t, press)
            } else {
                btn("fop-ins-n-up", "+", false, t, press)
            }
        })
        .child(if insert_frames_enabled(core) {
            crate::rune_sec::icon_btn::icon_btn(
                "fop-ins-n",
                "plus",
                &i18n_wire::tr(core, "frame_insert_frames"),
                false,
                t,
                |s| {
                    let count = s.frame_ops.insert_count;
                    insert_frames_after_current(&mut s.core, count);
                },
            )
        } else {
            crate::rune_sec::icon_btn::icon_btn_inert("fop-ins-n", "plus", &i18n_wire::tr(core, "frame_insert_frames"), t)
        });

    let mut panel = design::panel_col("frames", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, &i18n_wire::tr(core, "frame_title"), None, t)),
        )
        .child(design::divider(t))
        .child(design::section_label(&i18n_wire::tr(core, "frame_arrange"), t))
        .child(arrange)
        .child(design::section_label(&i18n_wire::tr(core, "frame_path"), t))
        .child(path)
        .child(design::section_label(&i18n_wire::tr(core, "frame_clipboard"), t))
        .child(clipboard);
    // The Sel/Loop readout names the range the Clipboard
    // group's range ops act on, so it seats directly
    // under that group (it rode at the tail of the old
    // wrapping row, after the duration steppers — the
    // readout's text and styling are unchanged).
    if let Some(label) = range_label(core, ops) {
        panel = panel.child(text(label).font(type_::LABEL).color(t.text_secondary));
    }
    panel
        .child(design::section_label(&i18n_wire::tr(core, "frame_dur"), t))
        .child(dur)
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
    fn default_state_has_expanded_timeline() {
        // Focused Wave (D1): the collapsed flag starts
        // false — a State built without prefs seeding
        // (every test literal in the tree) renders the
        // timeline exactly as before the wiring.
        assert!(!FrameOpsState::default().timeline_collapsed);
        let seeded = FrameOpsState {
            timeline_collapsed: true,
            ..Default::default()
        };
        assert!(seeded.timeline_collapsed);
        assert_eq!(seeded.insert_count, INSERT_COUNT_DEFAULT);
    }

    #[test]
    fn default_state_shows_every_timeline_lane() {
        // Roadmap Wave 1 (RM12): the per-lane flags
        // start shown — a State built without any lane
        // wiring renders the card exactly as it did
        // before the space model existed.
        let ops = FrameOpsState::default();
        for lane in [
            TimelineLane::Markers,
            TimelineLane::Clips,
            TimelineLane::Dopesheet,
            TimelineLane::Subtitles,
        ] {
            assert!(ops.lane_shown(lane), "{lane:?} starts shown");
        }
    }

    #[test]
    fn toggle_lane_flips_one_lane_and_reports_the_new_value() {
        // Roadmap Wave 1 (RM12): a toggle touches its
        // own flag only, returns the value it landed on,
        // and a second toggle restores the default.
        let mut ops = FrameOpsState::default();
        assert!(!ops.toggle_lane(TimelineLane::Clips));
        assert!(!ops.lane_shown(TimelineLane::Clips));
        assert!(ops.lane_shown(TimelineLane::Markers));
        assert!(ops.lane_shown(TimelineLane::Dopesheet));
        assert!(ops.lane_shown(TimelineLane::Subtitles));
        assert!(ops.toggle_lane(TimelineLane::Clips));
        assert!(ops.lane_shown(TimelineLane::Clips));
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
        // Wave U (U6): format_duration resolves through
        // the `frame_duration_value` template, so — like
        // the i18n pin below — these asserts pass only
        // once IA-U6-1's keys land at integration (until
        // then `tr` falls back to the key itself, the Q7
        // pattern).
        let core = EditorCore::new();
        assert_eq!(format_duration(&core, 1.0), "1.00×");
        assert_eq!(format_duration(&core, 0.25), "0.25×");
        assert_eq!(format_duration(&core, 2.5), "2.50×");
        assert_eq!(format_duration(&core, 8.0), "8.00×");
        // duration_label routes through the same format.
        assert_eq!(duration_label(&core), format_duration(&core, 1.0));
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

    #[test]
    fn reverse_all_guard_needs_two_frames() {
        // Truth table for Reverse all: inert below two
        // frames (no order to change), live from two up —
        // and, unlike can_reverse, never swayed by a loop
        // range, because the button ignores the loop range.
        let mut core = EditorCore::new();
        assert!(reverse_all_enabled(&core), "the demo's five frames reverse");
        core.project.frames.truncate(2);
        assert!(reverse_all_enabled(&core), "two frames are the minimum");
        core.project.frames.truncate(1);
        assert!(!reverse_all_enabled(&core), "one frame has no order");
        core.project.frames.clear();
        assert!(!reverse_all_enabled(&core), "no frames, nothing to reverse");
        // A one-frame loop range disables "Reverse" (via
        // reverse_args) but must not disable Reverse all.
        let mut core = EditorCore::new();
        core.set_loop_range(2, 2);
        assert!(!can_reverse(&core), "the loop rule sees one frame");
        assert!(reverse_all_enabled(&core), "Reverse all sees the timeline");
    }

    #[test]
    fn reverse_all_reverses_the_whole_timeline() {
        // A three-frame project, frames tagged by duration
        // (the distinguishing per-frame value, as in the
        // shift test above): the dispatch must move the
        // Frame values themselves, first to last.
        let mut core = EditorCore::new();
        core.project.frames.truncate(3);
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.duration = 1.0 + i as f32;
        }
        reverse_all(&mut core);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, [3.0, 2.0, 1.0], "frame order fully flipped");
        assert_eq!(core.frame_count(), 3, "reversing adds and removes nothing");
        // A second press restores the original order.
        reverse_all(&mut core);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, [1.0, 2.0, 3.0], "reverse is its own inverse");
    }

    #[test]
    fn reverse_all_ignores_the_loop_range_while_reverse_keeps_it() {
        // The existing range behaviour is untouched: with a
        // loop range set, "Reverse" (through reverse_args)
        // flips only the loop's frames...
        let mut core = EditorCore::new();
        core.project.frames.truncate(3);
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.duration = 1.0 + i as f32;
        }
        core.set_loop_range(1, 2);
        let (from, count) = reverse_args(&core);
        core.reverse_range(from, count);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(
            durations,
            [1.0, 3.0, 2.0],
            "the loop-range reverse leaves frame 1 in place"
        );
        // ...while Reverse all, pressed with that same loop
        // range still set, flips every frame regardless.
        reverse_all(&mut core);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(
            durations,
            [2.0, 3.0, 1.0],
            "Reverse all flips the whole timeline, loop range set or not"
        );
    }

    #[test]
    fn swap_guards_follow_the_current_frame() {
        // Truth table for the two Swap buttons (Phase M): a
        // direction is live exactly while the current frame
        // has a neighbour on that side.
        let mut core = EditorCore::new();
        core.project.frames.truncate(3);
        core.current = 0;
        assert!(!swap_prev_enabled(&core), "frame 0 has no previous frame");
        assert!(swap_next_enabled(&core), "frame 0 has a next frame");
        core.current = 1;
        assert!(swap_prev_enabled(&core), "the middle frame has a previous");
        assert!(swap_next_enabled(&core), "the middle frame has a next");
        core.current = 2;
        assert!(swap_prev_enabled(&core), "the last frame has a previous");
        assert!(!swap_next_enabled(&core), "the last frame has no next frame");
        // A single frame has no neighbour on either side.
        core.project.frames.truncate(1);
        core.current = 0;
        assert!(!swap_prev_enabled(&core), "one frame: no previous");
        assert!(!swap_next_enabled(&core), "one frame: no next");
        // Neither does an empty project.
        core.project.frames.clear();
        assert!(!swap_prev_enabled(&core), "no frames: no previous");
        assert!(!swap_next_enabled(&core), "no frames: no next");
    }

    #[test]
    fn swap_buttons_exchange_content_and_keep_current() {
        // Handler level, through the lib op the buttons
        // call: on a three-frame project tagged by duration,
        // swapping at the middle frame exchanges the two
        // frames' contents while `current` stays numerically
        // where it was — the playhead does not chase the
        // moved frame (the op's contract).
        let mut core = EditorCore::new();
        core.project.frames.truncate(3);
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.duration = 1.0 + i as f32;
        }
        core.current = 1;
        assert!(stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut core, -1));
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(
            durations,
            [2.0, 1.0, 3.0],
            "the current frame's content swapped with the previous frame's"
        );
        assert_eq!(core.current, 1, "the playhead's index does not move");
        // Swapping the other way acts on the frames now at
        // indices 1 and 2 (the op follows the index, not a
        // frame identity), again without moving `current`.
        assert!(stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut core, 1));
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, [2.0, 3.0, 1.0], "the second swap exchanges indices 1 and 2");
        assert_eq!(core.current, 1, "still on frame 1 after both swaps");
        assert_eq!(core.frame_count(), 3, "swapping adds and removes nothing");
    }

    /// Wave Q (Q15, completing P49): the regrouped Frames
    /// section constructs — the four labelled groups
    /// (Arrange / Path / Clipboard / Dur), the "This
    /// frame" and "Insert" rows, and the panel assembly —
    /// on the demo project under both themes, with and
    /// without the Sel/Loop readout that now seats under
    /// the Clipboard group. Construction is the assertable
    /// level (element internals are opaque — the
    /// chrome_view / audio_panel precedent); the regroup's
    /// key census (zero loss, four additive container
    /// keys) is recorded in the Q15 findings.
    #[test]
    fn frames_section_constructs_regrouped_under_both_themes() {
        let mut s = crate::init();
        assert!(s.core.frame_count() > 0, "the demo project has frames");
        let _ = frames_section(&s, &rune::tok::theme(true));
        let _ = frames_section(&s, &rune::tok::theme(false));
        // With a loop range the readout exists, so the
        // panel assembly takes its extra child.
        s.core.loop_range = Some((0, 0));
        assert!(range_label(&s.core, &s.frame_ops).is_some());
        let _ = frames_section(&s, &rune::tok::theme(true));
    }

    #[test]
    fn frame_ops_i18n_keys_exist_and_match_the_english_labels() {
        // Wave T (T3): every label in the section resolves
        // through i18n_wire::tr under a `frame_*` key. This
        // pin passes only once IA-T3-1's keys land in
        // src/i18n.rs at integration — until then tr falls
        // back to the key itself (the Q7 pattern) and both
        // asserts fail by design.
        let core = EditorCore::new();
        for (key, english) in [
            ("frame_inbetween", "Inbetween"),
            ("frame_shift_back", "< Shift"),
            ("frame_shift_fwd", "Shift >"),
            ("frame_reverse", "Reverse"),
            ("frame_reverse_all", "Reverse all"),
            ("frame_swap_prev", "Swap with previous"),
            ("frame_swap_next", "Swap with next"),
            ("frame_smooth_path", "Smooth path"),
            ("frame_copy_frame", "Copy frame"),
            ("frame_paste_frame", "Paste frame"),
            ("frame_copy_range", "Copy range"),
            ("frame_paste_range", "Paste range"),
            ("frame_delete_range", "Delete range"),
            ("frame_dur_down", "Dur -"),
            ("frame_dur_up", "Dur +"),
            ("frame_this_frame", "This frame"),
            ("frame_dur_reset", "Reset 1.0"),
            ("frame_insert", "Insert"),
            ("frame_insert_frames", "Insert frames"),
            ("frame_title", "Frames"),
            ("frame_arrange", "Arrange"),
            ("frame_path", "Path"),
            ("frame_clipboard", "Clipboard"),
            ("frame_dur", "Dur"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
        // Wave U (U6, IA-U6-1): the composed readouts —
        // the Sel/Loop range readout, the duration
        // multiplier and the Insert-N count — resolve
        // through `{slot}` template keys, pinned here in
        // their stored form, slots intact (the camera
        // `camera_following_figure` precedent). This
        // section passes only once IA-U6-1 lands, like
        // the label half above.
        for (key, template) in [
            ("frame_sel_one", "Sel {frame}"),
            ("frame_sel_range", "Sel {from}–{to}"),
            ("frame_loop_one", "Loop {frame}"),
            ("frame_loop_range", "Loop {from}–{to}"),
            ("frame_duration_value", "{value}×"),
            ("frame_insert_count", "{count} frames"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), template);
        }
        // Composed spot checks through the public
        // helpers: the exact pre-wire sentences. (The
        // Insert-N count composes inline in
        // `frames_section` — the audio header-count
        // idiom — so it is pinned by its stored
        // template above.)
        let mut core = EditorCore::new();
        core.set_loop_range(1, 3);
        assert_eq!(
            range_label(&core, &FrameOpsState::default()),
            Some("Loop 2–4".to_string())
        );
        assert_eq!(
            range_label(&core, &ops_with_sel(0, 0)),
            Some("Sel 1".to_string())
        );
        assert_eq!(duration_label(&core), "1.00×");
    }

    // ---- timeline zoom state (Roadmap Wave 1, RM13 — P-1) ----

    #[test]
    fn zoom_defaults_to_the_pre_zoom_geometry() {
        let ops = FrameOpsState::default();
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::CHIP_W);
        assert!(!ops.timeline_follow);
    }

    #[test]
    fn zoom_step_round_trips_and_clamps_at_both_ends() {
        let mut ops = FrameOpsState::default();
        ops.zoom_step(1);
        let in_w = ops.chip_w();
        assert!(in_w > crate::rune_sec::timeline_marks::CHIP_W);
        ops.zoom_step(-1);
        assert!((ops.chip_w() - crate::rune_sec::timeline_marks::CHIP_W).abs() < 0.01);
        for _ in 0..40 {
            ops.zoom_step(1);
        }
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MAX_W);
        for _ in 0..80 {
            ops.zoom_step(-1);
        }
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MIN_W);
        ops.zoom_step(0);
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MIN_W);
    }

    #[test]
    fn set_zoom_clamps_and_keeps_the_old_value_on_non_finite() {
        let mut ops = FrameOpsState::default();
        ops.set_zoom(1000.0);
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MAX_W);
        ops.set_zoom(1.0);
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MIN_W);
        ops.set_zoom(f32::NAN);
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::ZOOM_MIN_W);
        ops.timeline_zoom = f32::INFINITY;
        assert_eq!(ops.chip_w(), crate::rune_sec::timeline_marks::CHIP_W);
    }
}
