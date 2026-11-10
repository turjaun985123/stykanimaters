//! The subtitle lane for the Rune editor shell (Wave P,
//! P32) — the project's subtitle cues drawn as blocks
//! inside the timeline card, under the clips lane and the
//! dopesheet: one block per cue, spanning the frames the
//! cue covers, its text inside the block where the block
//! is wide enough to carry it.
//!
//! ## Geometry (reused, never re-derived)
//!
//! A cue is a span, and the timeline already has exactly
//! one span→pixels maths: [`span_rect`] in
//! `timeline_marks`, which the clips lane's audio/FX bars
//! are drawn with. [`cue_rect`] delegates to it verbatim —
//! the frame window is carried in a [`ClipSpan`], whose
//! `kind` field `span_rect` never reads (its body consumes
//! only `from_frame` / `to_frame` / the frame count; the
//! kind-driven consumers — `span_rgb`, `clip_lane_rows` —
//! are NOT fed cue spans: the colour here is the lane's
//! own and the row packing below works on the drawn rects
//! directly). The pitch ([`CHIP_PITCH`]) is imported from
//! `timeline_marks`, never re-typed, so a cue block sits
//! under its frames exactly as a clip bar does — the lane
//! has NO name gutter, for the clips lane's reason: a span
//! lane that shifts its spans off their frames is lying.
//!
//! One pitch is all there is: the timeline has no zoom
//! levels (its chip geometry is the fixed
//! `CHIP_W`/[`CHIP_GAP`] pair; zoom in this shell is a
//! stage-view concept). What varies is the frame count,
//! which moves the clamp at the timeline's right edge —
//! the pins below cover both.
//!
//! ## Rows
//!
//! Cues may overlap (the core's `subtitles_at` returns
//! every live cue, stacked), so one row cannot hold them
//! all: blocks are first-fit onto sub-rows in stored
//! order, judged on the DRAWN rects — a cue takes the
//! first row whose last block ends at or before it starts,
//! so blocks merely touching at an edge still share a
//! row. That is `clip_lane_rows`' rule, re-applied to cue
//! rects (the clips version splits rows by span kind;
//! cues have one kind, so no split exists to mirror). A
//! cue whose rect is zero-width (degenerate window, or
//! wholly past the last frame) draws nothing and takes
//! no row — the clips lane's zero-width rule.
//!
//! ## Interaction: click-to-select, and the drag verdict
//!
//! A block press selects its cue and seeks the playhead
//! to the cue's first frame ([`press_cue`] — the
//! dopesheet cell's select-plus-goto, with the selection
//! half landing on the shell's subtitle selection:
//! `State::subtitle_selected`, an `Option<usize>` into
//! `project.subtitles`, the `State::fx_selected` /
//! `stroke_selected` / `particle_selected` idiom). That
//! field is the ONE channel the P31 Subtitles panel reads
//! — the panel join is a shared field, not a conversion.
//! Selection and playhead are runtime state, so a press
//! spends no undo step, exactly like every other timeline
//! press. A stale selection (the project shrank under it)
//! tints nothing — [`selection_is_live`] is the guard the
//! panel shares (the audio panel's helper of the same
//! name and shape).
//!
//! Drag-to-move is NOT built, and the gap is exact: the
//! timeline has no per-block drag machinery to extend.
//! Its drags are strip-level — one `on_drag` on the chips
//! row (scrub / loop edges, `timeline_marks`) and one on
//! the ruler (seek) — both converting a pointer x to a
//! frame for a WHOLE-strip gesture; the dopesheet's cells
//! are press-only and there is no key-drag in the timeline
//! at all (the graph canvas's key drag is a different
//! surface with its own hit-test space). A cue drag would
//! need (a) a per-block drag element with grab-offset
//! state (which cue, grabbed how far into its window —
//! shell state that does not exist), and (b) a move op
//! whose undo composes per gesture: the core contract's
//! `update_subtitle_cue` spends one undo step per call,
//! so live drag events would flood the undo stack unless
//! a snapshot-once gesture composition (the camera dial's,
//! hand-built for one scalar in `camera_panel`) were
//! rebuilt for blocks. Both are new infrastructure, not
//! an extension — named here as the remainder for a later
//! wave rather than faked with press-only "drag".
//!
//! ## Mounting and labels
//!
//! The lane exists only when the project HAS cues (the
//! clips lane's `Option` discipline): a project without
//! subtitles spends no timeline height on an empty strip.
//! The header label is the dopesheet's idiom —
//! `design::section_label` over the lane, the frame
//! readout (the dopesheet's own [`frame_readout`]) at the
//! right — with one stated deviation: the word
//! "Subtitles" is a plain string, not an i18n key, because
//! no `subtitles` key exists in the 28 tables yet;
//! registering one (all 28 languages, SHELL_KEYS census)
//! is filed as an integrator action rather than smuggled
//! into this lane's diff.
//!
//! ## Integrator wiring (NOT done here)
//!
//! `rune_sec/mod.rs`, `src/main.rs` and
//! `timeline_marks.rs` are integrator surfaces; the exact
//! actions are filed in the P32 findings. In brief:
//! declare `pub mod subtitle_lane;`; add
//! `pub subtitle_selected: Option<usize>` to `State` plus
//! its `None` seed in `init`; append
//! `subtitle_lane(s, t)` to the timeline's scrolled
//! content column after the dopesheet, gated on
//! `!collapsed` only (the clips lane's gate — the lane
//! must be visible in the Audio workspace, where the P31
//! panel lives, not just Animate).
//!
//! ## Keys (all additive, Wave P)
//!
//! `subtitle-lane` on the lane root, [`block_key`]
//! (`subtitle-cue-{i}`, the cue's stored index) on each
//! block.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::subtitles::SubtitleCue;

use crate::rune_sec::design;
use crate::rune_sec::dopesheet::frame_readout;
use crate::rune_sec::timeline_marks::{
    span_rect, timeline_width, ClipSpan, SpanKind, CHIP_GAP, CHIP_PITCH,
};
use crate::State;

/// Height of the lane's header row ("Subtitles" + the
/// frame readout): the dopesheet header's measure
/// ([`crate::rune_sec::dopesheet::DOPE_HEADER_H`]) — one
/// LABEL line plus air.
pub const SUB_HEADER_H: f32 = 18.0;

/// Height of one block sub-row — and of the blocks in
/// it: a LABEL line of cue text plus breathing room,
/// where the clips lane's 8 pt bars carry no text at
/// all. One point slimmer than the dopesheet's rows
/// (20), which must also fit a press diamond.
pub const SUB_ROW_H: f32 = 18.0;

/// Vertical gap between two stacked block sub-rows: the
/// clips lane's [`crate::rune_sec::timeline_marks::CLIP_ROW_GAP`]
/// measure.
pub const SUB_ROW_GAP: f32 = 2.0;

/// The narrowest block, in points, that carries its
/// cue's text: two frames' extent — a one-frame block
/// (30 pt) is a tick, and text truncated into a tick
/// would show an ellipsis and nothing else, which says
/// less than the block alone. At two frames the block
/// fits a short word whole or a longer cue's readable
/// start.
pub const SUB_TEXT_MIN_W: f32 = 2.0 * CHIP_PITCH - CHIP_GAP;

/// A cue block's key: `subtitle-cue-{i}`, `i` the cue's
/// index in `project.subtitles` (stored order — the
/// index the selection and the panel both speak).
pub fn block_key(i: usize) -> String {
    format!("subtitle-cue-{i}")
}

/// A cue's rectangle in timeline points, as `[x, width]`:
/// the cue's half-open window `start_frame..end_frame`
/// through the timeline's own span maths ([`span_rect`],
/// delegated to verbatim — see the module docs for the
/// carrier note). Both edges clamp to the timeline's
/// extent; a degenerate window, a window wholly past the
/// last frame, or a zero-frame project gives width 0.
pub fn cue_rect(cue: &SubtitleCue, frame_count: usize) -> [f32; 2] {
    let carrier = ClipSpan {
        kind: SpanKind::Audio,
        from_frame: cue.start_frame as f32,
        to_frame: cue.end_frame as f32,
        muted_or_hidden: false,
    };
    span_rect(&carrier, frame_count)
}

/// Whether a block of `width` points carries its cue's
/// text (see [`SUB_TEXT_MIN_W`]).
pub fn block_shows_text(width: f32) -> bool {
    width >= SUB_TEXT_MIN_W
}

/// Which sub-row each cue paints on: first-fit in stored
/// order, judged on the drawn [`cue_rect`]s (a cue takes
/// the first row whose last block ends at or before the
/// cue starts, so edge-touching cues share a row).
/// `None` for a cue that draws nothing (zero width): it
/// takes no row.
pub fn cue_rows(cues: &[SubtitleCue], frame_count: usize) -> Vec<Option<usize>> {
    let mut rows: Vec<Option<usize>> = vec![None; cues.len()];
    let mut ends: Vec<f32> = Vec::new();
    for (i, cue) in cues.iter().enumerate() {
        let [x, w] = cue_rect(cue, frame_count);
        if w <= 0.0 {
            continue;
        }
        let end = x + w;
        let local = match ends.iter().position(|&e| e <= x) {
            Some(j) => {
                ends[j] = end;
                j
            }
            None => {
                ends.push(end);
                ends.len() - 1
            }
        };
        rows[i] = Some(local);
    }
    rows
}

/// How many sub-rows `rows` (from [`cue_rows`]) needs:
/// the highest assigned row plus one, or 0 when no cue
/// draws.
pub fn cue_row_count(rows: &[Option<usize>]) -> usize {
    rows.iter().flatten().max().map_or(0, |m| m + 1)
}

/// Whether `selected` still names a cue in a project
/// with `cue_count` cues — the audio panel's
/// `selection_is_live`, same name and shape, so the P31
/// panel guards its reads the way the audio panel does.
pub fn selection_is_live(selected: Option<usize>, cue_count: usize) -> bool {
    selected.is_some_and(|i| i < cue_count)
}

/// What a block press does (factored out of the closure
/// so it is unit-testable without a runtime gesture —
/// the timeline's press-handler discipline): select cue
/// `i` on the shell's subtitle selection, and seek the
/// playhead to the cue's first frame. `EditorCore::goto`
/// is bounds-safe, so a cue starting past the (shrunken)
/// end selects without moving the playhead. A stale `i`
/// (past the last cue) does nothing at all — no selection
/// is better than a selection naming no cue.
pub fn press_cue(s: &mut State, i: usize) {
    let Some(cue) = s.core.project.subtitles.get(i) else {
        return;
    };
    let start = cue.start_frame;
    s.subtitle_selected = Some(i);
    s.core.goto(start);
}

/// The subtitle lane: header, then one sub-row of blocks
/// per [`cue_rows`] row. `None` when the project has no
/// cues at all (the lane exists only when it has
/// something to show — the clips lane's rule). Built to
/// be appended to the timeline's scrolled content column
/// — see the module docs for the wiring.
pub fn subtitle_lane(s: &State, t: &Theme) -> Option<El> {
    let core = &s.core;
    let cues = &core.project.subtitles;
    if cues.is_empty() {
        return None;
    }
    let frame_count = core.frame_count();
    let rows = cue_rows(cues, frame_count);

    // The header row: the lane's name at the left, the
    // current-frame readout at the right — the
    // dopesheet header's construction, sized to the
    // timeline's width so the readout lands over the
    // last frame column.
    let mut header = row().gap(8.0).center_y();
    if frame_count > 0 {
        header = header.size(timeline_width(frame_count), SUB_HEADER_H);
    } else {
        header = header.h(SUB_HEADER_H);
    }
    let header = header
        .child(design::section_label("Subtitles", t))
        .child(el().grow(1.0))
        .child(
            text(frame_readout(core.current, frame_count))
                .font(type_::MONO_CAPTION.mono())
                .color(t.text_secondary),
        );

    let mut lane = col().key("subtitle-lane").gap(SUB_ROW_GAP).child(header);
    for r in 0..cue_row_count(&rows) {
        lane = lane.child(block_row(s, t, &rows, r));
    }
    Some(lane)
}

/// One sub-row of blocks: spacers carry each block to
/// its [`cue_rect`] x (the row's gap is 0 — the rects
/// ARE the layout), blocks in stored order (within one
/// row their x is non-decreasing, by [`cue_rows`]' fit
/// rule, so a single left-to-right cursor places them
/// all).
fn block_row(s: &State, t: &Theme, rows: &[Option<usize>], r: usize) -> El {
    let core = &s.core;
    let frame_count = core.frame_count();
    let mut row_el = row().gap(0.0).h(SUB_ROW_H).center_y();
    let mut cursor = 0.0f32;
    for (i, cue) in core.project.subtitles.iter().enumerate() {
        if rows[i] != Some(r) {
            continue;
        }
        let [x, w] = cue_rect(cue, frame_count);
        if x > cursor {
            row_el = row_el.child(el().size(x - cursor, SUB_ROW_H));
        }
        cursor = x + w;
        let selected = s.subtitle_selected == Some(i);
        let mut block = row()
            .key(block_key(i))
            .size(w, SUB_ROW_H)
            .px(4.0)
            .center_y()
            .radius(design::CONTROL_RADIUS)
            .fill(if selected {
                design::selected_fill()
            } else {
                t.control_off
            })
            .hover(t.hover)
            .press(t.press)
            .on_press(move |s: &mut State| press_cue(s, i));
        if block_shows_text(w) && !cue.text.is_empty() {
            block = block.child(
                text(cue.text.clone())
                    .font(type_::LABEL)
                    .color(t.text)
                    .truncate(),
            );
        }
        row_el = row_el.child(block);
    }
    row_el
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rune_sec::timeline_marks::CHIP_W;
    use stykanimate_rs::model::{Frame, Pose};

    fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
        }
    }

    fn frame() -> Frame {
        Frame {
            poses: Vec::<Pose>::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }
    }

    /// A state with `frames` frames and the given cues.
    fn cue_state(frames: usize, cues: Vec<SubtitleCue>) -> State {
        let mut s = crate::init();
        s.core.project.frames = (0..frames).map(|_| frame()).collect();
        s.core.project.subtitles = cues;
        s
    }

    #[test]
    fn block_key_follows_the_scheme() {
        assert_eq!(block_key(0), "subtitle-cue-0");
        assert_eq!(block_key(7), "subtitle-cue-7");
    }

    #[test]
    fn cue_rect_is_the_span_maths_at_the_timeline_pitch() {
        // Chip f starts at f * 34; a window's right edge
        // is the end of its last chip (end * 34 - 4).
        assert_eq!(cue_rect(&cue(0, 1, ""), 5), [0.0, CHIP_W]);
        assert_eq!(cue_rect(&cue(0, 3, ""), 5), [0.0, 3.0 * CHIP_PITCH - CHIP_GAP]);
        assert_eq!(cue_rect(&cue(2, 5, ""), 5), [68.0, 98.0]);
        assert_eq!(
            cue_rect(&cue(4, 5, ""), 5),
            [4.0 * CHIP_PITCH, CHIP_W],
            "the last frame's cue is one chip wide at the right edge"
        );
        // A long timeline: the same pitch, no drift.
        assert_eq!(cue_rect(&cue(50, 60, ""), 100), [1700.0, 336.0]);
    }

    #[test]
    fn cue_rect_agrees_with_span_rect_on_the_same_window() {
        // The delegation is the contract: for any window,
        // the cue rect IS the clips span rect.
        for (from, to, count) in [(0, 1, 5), (2, 5, 5), (3, 9, 5), (7, 9, 5), (0, 0, 3)] {
            let span = ClipSpan {
                kind: SpanKind::Fx,
                from_frame: from as f32,
                to_frame: to as f32,
                muted_or_hidden: false,
            };
            assert_eq!(
                cue_rect(&cue(from, to, ""), count),
                span_rect(&span, count),
                "window {from}..{to} over {count} frames"
            );
        }
    }

    #[test]
    fn cue_rect_clamps_at_the_timeline_end() {
        // Total width over 5 frames: 5 * 34 - 4 = 166.
        assert_eq!(timeline_width(5), 166.0);
        // Running past the end stops at the last chip.
        assert_eq!(cue_rect(&cue(3, 9, ""), 5), [102.0, 64.0]);
        // Wholly past the end: zero width, at the end.
        assert_eq!(cue_rect(&cue(7, 9, ""), 5), [166.0, 0.0]);
        // No frames, no rect.
        assert_eq!(cue_rect(&cue(0, 2, ""), 0), [0.0, 0.0]);
        // A degenerate window draws nothing.
        assert_eq!(cue_rect(&cue(3, 3, ""), 5), [102.0, 0.0]);
        assert_eq!(cue_rect(&cue(4, 2, ""), 5), [136.0, 0.0]);
    }

    #[test]
    fn text_shows_only_on_blocks_wide_enough_to_carry_it() {
        assert!(!block_shows_text(CHIP_W), "a one-frame tick carries no text");
        assert!(!block_shows_text(SUB_TEXT_MIN_W - 1.0));
        assert!(block_shows_text(SUB_TEXT_MIN_W));
        assert!(block_shows_text(200.0));
        // The threshold is exactly two frames' extent.
        assert_eq!(SUB_TEXT_MIN_W, cue_rect(&cue(0, 2, ""), 9)[1]);
    }

    #[test]
    fn cue_rows_share_when_separate_and_stack_when_overlapping() {
        let cues = vec![cue(0, 2, "a"), cue(2, 4, "b"), cue(5, 6, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(0), Some(0)]);
        assert_eq!(cue_row_count(&cue_rows(&cues, 10)), 1);

        // The middle cue overlaps the first; the third
        // fits back onto the first row once it ends.
        let cues = vec![cue(0, 4, "a"), cue(2, 6, "b"), cue(4, 8, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(1), Some(0)]);
        assert_eq!(cue_row_count(&cue_rows(&cues, 10)), 2);

        // Three mutually overlapping cues stack three deep.
        let cues = vec![cue(0, 6, "a"), cue(1, 5, "b"), cue(2, 3, "c")];
        assert_eq!(cue_rows(&cues, 10), vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn cue_rows_give_no_row_to_cues_that_draw_nothing() {
        let cues = vec![cue(0, 2, "a"), cue(3, 3, "empty"), cue(9, 12, "past")];
        let rows = cue_rows(&cues, 5);
        assert_eq!(rows, vec![Some(0), None, None]);
        assert_eq!(cue_row_count(&rows), 1);
        assert_eq!(cue_row_count(&[]), 0);
    }

    #[test]
    fn selection_liveness_matches_the_audio_panel_rule() {
        assert!(selection_is_live(Some(0), 1));
        assert!(selection_is_live(Some(2), 3));
        assert!(!selection_is_live(Some(3), 3));
        assert!(!selection_is_live(Some(0), 0));
        assert!(!selection_is_live(None, 3));
    }

    #[test]
    fn press_cue_selects_and_seeks_to_the_cue_start() {
        let mut s = cue_state(10, vec![cue(0, 2, "a"), cue(4, 7, "b")]);
        press_cue(&mut s, 1);
        assert_eq!(s.subtitle_selected, Some(1));
        assert_eq!(s.core.current, 4, "the playhead seeks the cue's first frame");
        press_cue(&mut s, 0);
        assert_eq!(s.subtitle_selected, Some(0));
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn press_cue_with_a_start_past_the_end_selects_without_seeking() {
        // The project shrank under the cue: goto's own
        // bounds rule leaves the playhead where it was.
        let mut s = cue_state(3, vec![cue(8, 10, "stale")]);
        s.core.goto(2);
        press_cue(&mut s, 0);
        assert_eq!(s.subtitle_selected, Some(0));
        assert_eq!(s.core.current, 2);
    }

    #[test]
    fn press_cue_ignores_a_stale_index() {
        let mut s = cue_state(10, vec![cue(0, 2, "a")]);
        press_cue(&mut s, 5);
        assert_eq!(s.subtitle_selected, None);
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn lane_constructs_on_every_state_branch() {
        // Element trees are not readable from unit tests
        // — construction is the assertable level here
        // (the design kit's own discipline); the
        // presented-tree proof belongs to the headless
        // snaps (P33's seeded-cue proof covers this lane's
        // stage sibling; the lane itself rides the
        // timeline's existing snap coverage).
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // No cues: no lane.
            let s = cue_state(10, Vec::new());
            assert!(subtitle_lane(&s, &t).is_none());
            // One cue.
            let s = cue_state(10, vec![cue(1, 4, "Hello.")]);
            assert!(subtitle_lane(&s, &t).is_some());
            // Overlapping cues stack; a selected cue is
            // among them.
            let mut s = cue_state(10, vec![cue(0, 6, "a"), cue(2, 8, "b"), cue(3, 4, "c")]);
            s.subtitle_selected = Some(1);
            assert!(subtitle_lane(&s, &t).is_some());
            // Cues but no frames: the header's bare
            // branch, and every cue draws nothing.
            let s = cue_state(0, vec![cue(0, 2, "a")]);
            assert!(subtitle_lane(&s, &t).is_some());
            // A stale selection tints nothing and breaks
            // nothing.
            let mut s = cue_state(10, vec![cue(0, 2, "a")]);
            s.subtitle_selected = Some(99);
            assert!(subtitle_lane(&s, &t).is_some());
        }
    }
}
