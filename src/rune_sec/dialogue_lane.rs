//! The dialogue lane for the Rune editor shell (Roadmap
//! Wave 2, W2-12 — P-15's dialogue half): the episode's
//! dialogue lines drawn as blocks inside the timeline
//! card, under the subtitle lane — one block per subtitle
//! cue, carrying its speaker and its text, under a band
//! of chapter segments that names the chapter each line
//! belongs to.
//!
//! ## What a dialogue line IS here
//!
//! A dialogue line is a subtitle cue, read as dialogue:
//! the binding is the cue itself (its window, its text,
//! its speaker credit) plus the chapter its first frame
//! falls in ([`cue_chapter`] — W2-11's model,
//! `Project::chapter_index_at` at the cue's start frame,
//! the note W2-11 filed for this seat). There is NO
//! second dialogue store and NO dialogue editor: editing
//! a line's text, timing or speaker stays in the
//! Subtitles panel, and a block press lands on the
//! panel's own selection channel (see Interaction). This
//! lane is the reading surface — the episode's script
//! laid over its timeline — not a second place to write.
//!
//! ## Geometry (reused, never re-derived)
//!
//! Every span→pixels question delegates: a line's rect
//! IS the subtitle lane's cue rect ([`line_rect`] →
//! `subtitle_lane::cue_rect` → `timeline_marks`'
//! `span_rect`), a chapter segment's rect is the same
//! `span_rect` over the chapter's half-open range
//! ([`chapter_rect`]), and the row packing is the
//! subtitle lane's first-fit rule ([`line_rows`] →
//! `cue_rows`; [`chapter_rows`] is the same rule over
//! chapter rects, chapter-typed). The pitch is the
//! timeline's live one (`FrameOpsState::chip_w()` —
//! Roadmap Wave 1, RM13's zoom), threaded like every
//! other lane: zoom the timeline and the band, the
//! blocks and their boundaries all move together, a cue
//! boundary is a block edge exactly where the subtitle
//! lane draws it. The lane has NO name gutter, for the
//! clips lane's reason: a span lane that shifts its spans
//! off their frames is lying.
//!
//! ## The chapter band
//!
//! When the project has chapters, a band of segments
//! rides above the line rows: one segment per chapter
//! spanning `[start_frame, end_frame)`, titled where it
//! is wide enough to carry its title (the subtitle
//! lane's two-frame text threshold). Chapters may
//! overlap (the model allows it), so segments first-fit
//! onto band rows by their drawn rects, exactly like
//! cue blocks; a chapter that draws nothing (degenerate
//! window, or wholly past the last frame) takes no row.
//! The segment covering the playhead wears the selected
//! tint — the band answers "which chapter are we in" at
//! a glance — and a segment press seeks the playhead to
//! the chapter's first frame ([`press_chapter`]).
//! Chapters are structure with no shell selection field,
//! so a press moves the playhead and nothing else: no
//! selection is invented for them, no undo step is spent
//! (playhead and view state are runtime, the timeline
//! press discipline).
//!
//! ## Interaction: click-to-seek, one selection channel
//!
//! A block press is [`press_line`], which IS the
//! subtitle lane's `press_cue`: select the cue on the
//! shell's subtitle selection (`State::subtitle_selected`
//! — the ONE channel the Subtitles panel reads) and seek
//! the playhead to the cue's first frame. Clicking a
//! dialogue line therefore opens that cue in the panel
//! for editing — the lane borrows the panel's selection
//! rather than growing a parallel one that could
//! disagree with it. A selected line wears the same
//! selected tint the subtitle lane gives its block. No
//! drag: moving a cue's window is the subtitle lane's
//! gesture (Wave R); two lanes dragging the same cue
//! would be two write paths for one window, and this
//! lane is read-side by brief.
//!
//! ## Mounting and labels
//!
//! The builder returns `None` when the project has
//! neither cues nor chapters ([`dialogue_present`] —
//! the lane's data gate, evaluated by the mount site's
//! `lane_mounted` rule like every data-gated lane).
//! With chapters but no cues it renders the header and
//! the band over an empty state — one line naming where
//! dialogue comes from (`dialogue-lane-empty`). The
//! empty state carries NO Add button, deliberately: the
//! subtitle lane one row up already offers the panel's
//! add, and a second creation door on the reading
//! surface would blur the lane's read-side brief. The
//! header is the subtitle lane's construction —
//! `design::section_label` over the lane, the dopesheet's
//! `frame_readout` at the right — and its word goes
//! through the catalogue: `i18n_wire::tr(core,
//! "dialogue")` (the key is IA-W2-12-4's, filed with
//! this seat; until it lands the header renders the key
//! text, the Q7 pattern). A block's label is the
//! subtitle lane's [`block_label`] verbatim — the
//! speaker prefixed as `Name: ` for a credited cue,
//! the raw text byte for byte otherwise — so a line
//! reads identically in both lanes. A chapter segment's
//! label is the chapter's stored title, nothing added.
//!
//! ## Integrator wiring (filed as IAs, not landed here)
//!
//! IA-W2-12-1: `pub mod dialogue_lane;` in
//! `rune_sec/mod.rs`. IA-W2-12-2: `TimelineLane::Dialogue`
//! + `show_dialogue_lane` in `frame_ops`. IA-W2-12-3:
//! the mount + toggle plumbing in `timeline_marks`
//! (stacked under the subtitle lane). IA-W2-12-4: the
//! `dialogue` + `dialogue_lane_empty` catalogue keys
//! ×28. Until the wiring lands this module is compiled
//! by nothing and the closing frame is unchanged — the
//! headless prediction in this seat's findings is for
//! the integrated tree.
//!
//! ## Keys
//!
//! Counted literals: `dialogue-lane` on the lane root,
//! `dialogue-lane-empty` on the empty state's row.
//! Function-carried families (returned from [`line_key`]
//! / [`chapter_key`] and reaching `.key()` as variables —
//! the `subtitle_lane::block_key` artefact class, outside
//! the gate counter, distinctness pinned by the tests
//! below): `dialogue-line-{i}` (the cue's stored index)
//! on each block, `dialogue-chapter-{j}` (the chapter's
//! episode-order index) on each band segment.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::model::{Chapter, Project};
use stykanimate_rs::subtitles::SubtitleCue;

use crate::rune_sec::design;
use crate::rune_sec::dopesheet::frame_readout;
use crate::rune_sec::subtitle_lane::{
    block_label, block_shows_text, cue_rect, cue_row_count, cue_rows, press_cue,
};
use crate::rune_sec::timeline_marks::{span_rect, timeline_width, ClipSpan, SpanKind};
use crate::State;

/// Height of the lane's header row ("Dialogue" + the
/// frame readout): the subtitle lane's header measure
/// (`subtitle_lane::SUB_HEADER_H`) — one LABEL line plus
/// air.
pub const DLG_HEADER_H: f32 = 18.0;

/// Height of one dialogue block row — and of the blocks
/// in it: the subtitle lane's block-row measure
/// (`subtitle_lane::SUB_ROW_H`), a LABEL line of line
/// text plus breathing room.
pub const DLG_ROW_H: f32 = 18.0;

/// Height of one chapter band row — and of the segments
/// in it: the same LABEL-line measure as the block rows
/// (a segment carries a LABEL title, nothing smaller).
pub const DLG_BAND_H: f32 = 18.0;

/// Vertical gap between the lane's stacked rows: the
/// subtitle lane's sub-row gap measure
/// (`subtitle_lane::SUB_ROW_GAP`).
pub const DLG_ROW_GAP: f32 = 2.0;

/// A dialogue block's key: `dialogue-line-{i}`, `i` the
/// cue's index in `project.subtitles` (stored order —
/// the index the shared selection and the panel speak).
pub fn line_key(i: usize) -> String {
    format!("dialogue-line-{i}")
}

/// A chapter segment's key: `dialogue-chapter-{j}`, `j`
/// the chapter's index in `project.chapters` (episode
/// order — the index [`cue_chapter`] returns).
pub fn chapter_key(j: usize) -> String {
    format!("dialogue-chapter-{j}")
}

/// A dialogue line's rectangle in timeline points, as
/// `[x, width]`: the cue's rect, delegated verbatim to
/// the subtitle lane's [`cue_rect`] — a line sits under
/// its frames exactly where its cue block sits, at the
/// timeline's live pitch, clamped at the timeline's
/// right edge by the shared span maths.
pub fn line_rect(cue: &SubtitleCue, frame_count: usize, chip_w: f32) -> [f32; 2] {
    cue_rect(cue, frame_count, chip_w)
}

/// A dialogue line's display label: the subtitle lane's
/// [`block_label`] verbatim — the speaker prefixed as
/// `Name: ` for a credited cue, the raw text byte for
/// byte otherwise. Display only, composed fresh at
/// render, stored nowhere.
pub fn line_label(cue: &SubtitleCue) -> String {
    block_label(cue)
}

/// Whether a block of `width` points carries its line's
/// text at chip width `chip_w`: the subtitle lane's
/// two-frame threshold, delegated.
pub fn line_shows_text(width: f32, chip_w: f32) -> bool {
    block_shows_text(width, chip_w)
}

/// Which sub-row each line paints on: the subtitle
/// lane's first-fit packing, delegated verbatim
/// ([`cue_rows`] — stored order, judged on the drawn
/// rects; `None` for a cue that draws nothing).
pub fn line_rows(cues: &[SubtitleCue], frame_count: usize, chip_w: f32) -> Vec<Option<usize>> {
    cue_rows(cues, frame_count, chip_w)
}

/// How many sub-rows `rows` (from [`line_rows`]) needs:
/// the subtitle lane's count, delegated.
pub fn line_row_count(rows: &[Option<usize>]) -> usize {
    cue_row_count(rows)
}

/// The chapter a dialogue line belongs to: the chapter
/// covering the cue's FIRST frame, as an index into
/// `project.chapters` in episode order — W2-11's filed
/// binding for this seat (`chapter_index_at` at
/// `cue.start_frame`). A cue starting in a gap between
/// chapters belongs to none (`None`); a cue that starts
/// inside a chapter and runs past its end still belongs
/// to that chapter — the binding is where the line
/// begins, the chapter a reader is in when the line is
/// spoken. Overlapping chapters resolve to the first in
/// episode order, the model's own rule.
pub fn cue_chapter(project: &Project, cue: &SubtitleCue) -> Option<usize> {
    project.chapter_index_at(cue.start_frame)
}

/// A chapter segment's rectangle in timeline points, as
/// `[x, width]`: the chapter's half-open range
/// `[start_frame, end_frame)` through the timeline's own
/// span maths ([`span_rect`], the [`cue_rect`]
/// delegation's carrier shape — the `kind` field is the
/// carrier `span_rect` never reads). Both edges clamp
/// to the timeline's extent; a degenerate range, a range
/// wholly past the last frame, or a zero-frame project
/// gives width 0.
pub fn chapter_rect(chapter: &Chapter, frame_count: usize, chip_w: f32) -> [f32; 2] {
    let carrier = ClipSpan {
        kind: SpanKind::Audio,
        from_frame: chapter.start_frame as f32,
        to_frame: chapter.end_frame as f32,
        muted_or_hidden: false,
    };
    span_rect(&carrier, frame_count, chip_w)
}

/// Which band row each chapter paints on: first-fit in
/// episode order, judged on the drawn [`chapter_rect`]s
/// — a chapter takes the first row whose last segment
/// ends at or before it starts, so edge-touching
/// chapters share a row and overlapping ones stack.
/// This is `cue_rows`' rule re-applied to chapter rects
/// (the clips lane's split-by-kind does not exist here:
/// chapters have one kind). `None` for a chapter that
/// draws nothing (zero width): it takes no row.
pub fn chapter_rows(chapters: &[Chapter], frame_count: usize, chip_w: f32) -> Vec<Option<usize>> {
    let mut rows: Vec<Option<usize>> = vec![None; chapters.len()];
    let mut ends: Vec<f32> = Vec::new();
    for (j, chapter) in chapters.iter().enumerate() {
        let [x, w] = chapter_rect(chapter, frame_count, chip_w);
        if w <= 0.0 {
            continue;
        }
        let end = x + w;
        let local = match ends.iter().position(|&e| e <= x) {
            Some(k) => {
                ends[k] = end;
                k
            }
            None => {
                ends.push(end);
                ends.len() - 1
            }
        };
        rows[j] = Some(local);
    }
    rows
}

/// How many band rows `rows` (from [`chapter_rows`])
/// needs: the highest assigned row plus one, or 0 when
/// no chapter draws.
pub fn chapter_row_count(rows: &[Option<usize>]) -> usize {
    rows.iter().flatten().max().map_or(0, |m| m + 1)
}

/// The lane's data gate: the project carries dialogue
/// or structure to read — at least one cue (a dialogue
/// line) or at least one chapter (a band to lay lines
/// under, and the empty state to explain their absence).
/// The mount site's `lane_mounted` rule consumes this
/// beside the collapsed gate and the user's lane flag.
pub fn dialogue_present(project: &Project) -> bool {
    !project.subtitles.is_empty() || !project.chapters.is_empty()
}

/// What a block press does: the subtitle lane's
/// [`press_cue`] verbatim — select cue `i` on the
/// shell's ONE subtitle selection and seek the playhead
/// to the cue's first frame, so the Subtitles panel
/// shows the line the user clicked, ready to edit. No
/// undo step (selection and playhead are runtime state),
/// and a stale `i` does nothing at all.
pub fn press_line(s: &mut State, i: usize) {
    press_cue(s, i);
}

/// What a chapter segment press does (factored out of
/// the closure so it is unit-testable): seek the
/// playhead to chapter `j`'s first frame.
/// `EditorCore::goto` is bounds-safe, so a chapter
/// starting past a shrunken end seeks nowhere. Chapters
/// carry no shell selection, so nothing else is
/// written — no selection, no snapshot, no status: the
/// press is pure navigation, the timeline-press
/// discipline. A stale `j` does nothing at all.
pub fn press_chapter(s: &mut State, j: usize) {
    let Some(chapter) = s.core.project.chapters.get(j) else {
        return;
    };
    let start = chapter.start_frame;
    s.core.goto(start);
}

/// The dialogue lane: header, the chapter band (when
/// any chapter draws), then one sub-row of line blocks
/// per [`line_rows`] row — or, on a chaptered but
/// cue-less project, the band over the empty state.
/// `None` when the project has neither cues nor
/// chapters ([`dialogue_present`]). Built to be
/// appended to the timeline's scrolled content column
/// under the subtitle lane — see the module docs for
/// the wiring (IA-W2-12-3).
pub fn dialogue_lane(s: &State, t: &Theme) -> Option<El> {
    let core = &s.core;
    if !dialogue_present(&core.project) {
        return None;
    }
    let cues = &core.project.subtitles;
    let chapters = &core.project.chapters;
    let frame_count = core.frame_count();
    let chip_w = s.frame_ops.chip_w();

    // The header row: the lane's name at the left, the
    // current-frame readout at the right — the subtitle
    // lane's construction, sized to the timeline's width
    // so the readout lands over the last frame column.
    let mut header = row().gap(8.0).center_y();
    if frame_count > 0 {
        header = header.size(timeline_width(frame_count, chip_w), DLG_HEADER_H);
    } else {
        header = header.h(DLG_HEADER_H);
    }
    let header = header
        .child(design::section_label(
            &crate::rune_sec::i18n_wire::tr(core, "dialogue"),
            t,
        ))
        .child(el().grow(1.0))
        .child(
            text(frame_readout(core.current, frame_count))
                .font(type_::MONO_CAPTION.mono())
                .color(t.text_secondary),
        );

    let mut lane = col().key("dialogue-lane").gap(DLG_ROW_GAP).child(header);

    // The chapter band: segments in episode order over
    // their ranges, stacked by the first-fit rows. The
    // chapter covering the playhead wears the selected
    // tint; every segment presses to its chapter's
    // first frame.
    let band = chapter_rows(chapters, frame_count, chip_w);
    let playhead_chapter = core.project.chapter_index_at(core.current);
    for r in 0..chapter_row_count(&band) {
        lane = lane.child(band_row(s, t, &band, r, playhead_chapter));
    }

    if cues.is_empty() {
        // The empty state: chapters exist (the data gate
        // passed on them) but no line has been written —
        // one line naming where dialogue comes from. No
        // Add button: creation lives in the Subtitles
        // panel, whose lane one row up carries the add
        // (see the module docs).
        lane = lane.child(
            row().key("dialogue-lane-empty").gap(8.0).center_y().child(
                text(crate::rune_sec::i18n_wire::tr(core, "dialogue_lane_empty"))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .grow(1.0)
                    .truncate(),
            ),
        );
        return Some(lane);
    }
    let rows = line_rows(cues, frame_count, chip_w);
    for r in 0..line_row_count(&rows) {
        lane = lane.child(line_row(s, t, &rows, r));
    }
    Some(lane)
}

/// One band row of chapter segments: spacers carry each
/// segment to its [`chapter_rect`] x (the row's gap is
/// 0 — the rects ARE the layout), segments in episode
/// order (within one row their x is non-decreasing, by
/// [`chapter_rows`]' fit rule, so a single left-to-right
/// cursor places them all) — the subtitle lane's
/// block-row construction, segment-shaped.
fn band_row(
    s: &State,
    t: &Theme,
    rows: &[Option<usize>],
    r: usize,
    playhead_chapter: Option<usize>,
) -> El {
    let core = &s.core;
    let frame_count = core.frame_count();
    let chip_w = s.frame_ops.chip_w();
    let mut row_el = row().gap(0.0).h(DLG_BAND_H).center_y();
    let mut cursor = 0.0f32;
    for (j, chapter) in core.project.chapters.iter().enumerate() {
        if rows[j] != Some(r) {
            continue;
        }
        let [x, w] = chapter_rect(chapter, frame_count, chip_w);
        if x > cursor {
            row_el = row_el.child(el().size(x - cursor, DLG_BAND_H));
        }
        cursor = x + w;
        let mut segment = row()
            .key(chapter_key(j))
            .size(w, DLG_BAND_H)
            .px(4.0)
            .center_y()
            .radius(design::CONTROL_RADIUS)
            .fill(if playhead_chapter == Some(j) {
                design::selected_fill()
            } else {
                t.control_off
            })
            .hover(t.hover)
            .press(t.press)
            .on_press(move |s: &mut State| press_chapter(s, j));
        if block_shows_text(w, chip_w) && !chapter.title.is_empty() {
            segment = segment.child(
                text(chapter.title.as_str())
                    .font(type_::LABEL)
                    .color(t.text)
                    .truncate(),
            );
        }
        row_el = row_el.child(segment);
    }
    row_el
}

/// One sub-row of dialogue blocks: the subtitle lane's
/// block row with a press instead of a drag — spacers
/// carry each block to its [`line_rect`] x, blocks in
/// stored order, the shared selection's cue wearing the
/// selected tint, the line's label inside where the
/// block is wide enough to carry it.
fn line_row(s: &State, t: &Theme, rows: &[Option<usize>], r: usize) -> El {
    let core = &s.core;
    let frame_count = core.frame_count();
    let chip_w = s.frame_ops.chip_w();
    let mut row_el = row().gap(0.0).h(DLG_ROW_H).center_y();
    let mut cursor = 0.0f32;
    for (i, cue) in core.project.subtitles.iter().enumerate() {
        if rows[i] != Some(r) {
            continue;
        }
        let [x, w] = line_rect(cue, frame_count, chip_w);
        if x > cursor {
            row_el = row_el.child(el().size(x - cursor, DLG_ROW_H));
        }
        cursor = x + w;
        let selected = s.subtitle_selected == Some(i);
        let mut block = row()
            .key(line_key(i))
            .size(w, DLG_ROW_H)
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
            .on_press(move |s: &mut State| press_line(s, i));
        if line_shows_text(w, chip_w) && !cue.text.is_empty() {
            block = block.child(
                text(line_label(cue))
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
    use crate::rune_sec::timeline_marks::{span_rect, ClipSpan, SpanKind, CHIP_W};
    use stykanimate_rs::model::{Frame, Pose};

    fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    fn credited_cue(start: usize, end: usize, text: &str, name: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: Some(name.to_string()),
        }
    }

    fn chapter(start: usize, end: usize, title: &str) -> Chapter {
        Chapter {
            title: title.to_string(),
            start_frame: start,
            end_frame: end,
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

    /// A state with `frames` frames, the given cues and
    /// the given chapters.
    fn dlg_state(frames: usize, cues: Vec<SubtitleCue>, chapters: Vec<Chapter>) -> State {
        let mut s = crate::init();
        s.core.project.frames = (0..frames).map(|_| frame()).collect();
        s.core.project.subtitles = cues;
        s.core.project.chapters = chapters;
        s
    }

    #[test]
    fn keys_follow_the_scheme() {
        assert_eq!(line_key(0), "dialogue-line-0");
        assert_eq!(line_key(7), "dialogue-line-7");
        assert_eq!(chapter_key(0), "dialogue-chapter-0");
        assert_eq!(chapter_key(3), "dialogue-chapter-3");
        // The two families can never collide with each
        // other or with the subtitle lane's blocks.
        assert_ne!(line_key(0), chapter_key(0));
        assert_ne!(line_key(0), crate::rune_sec::subtitle_lane::block_key(0));
    }

    #[test]
    fn line_rect_is_the_cue_rect_at_every_window() {
        // The delegation is the contract: a line's rect
        // IS its cue block's rect, so the two lanes can
        // never disagree about where a line sits.
        for (start, end, count) in [(0, 1, 5), (2, 5, 5), (3, 9, 5), (7, 9, 5), (0, 0, 3)] {
            let c = cue(start, end, "x");
            assert_eq!(
                line_rect(&c, count, CHIP_W),
                crate::rune_sec::subtitle_lane::cue_rect(&c, count, CHIP_W),
                "window {start}..{end} over {count} frames"
            );
        }
        // And the zoom threads through it: at half the
        // default chip width the pitch is 15 + 4 = 19,
        // so the window sits at 2 * 19 with width
        // 5 * 19 - 4 - 38.
        let c = cue(2, 5, "x");
        assert_eq!(line_rect(&c, 10, CHIP_W / 2.0), [38.0, 53.0]);
    }

    #[test]
    fn chapter_rect_agrees_with_span_rect_and_clamps() {
        for (from, to, count) in [(0, 3, 5), (2, 5, 5), (3, 9, 5), (0, 0, 3)] {
            let span = ClipSpan {
                kind: SpanKind::Fx,
                from_frame: from as f32,
                to_frame: to as f32,
                muted_or_hidden: false,
            };
            assert_eq!(
                chapter_rect(&chapter(from, to, "c"), count, CHIP_W),
                span_rect(&span, count, CHIP_W),
                "range {from}..{to} over {count} frames"
            );
        }
        // Running past the end stops at the last chip
        // (total width over 5 frames: 5 * 34 - 4 = 166).
        assert_eq!(chapter_rect(&chapter(3, 9, "c"), 5, CHIP_W), [102.0, 64.0]);
        // Wholly past the end: zero width, at the end.
        assert_eq!(chapter_rect(&chapter(7, 9, "c"), 5, CHIP_W), [166.0, 0.0]);
        // A degenerate range draws nothing; no frames,
        // no rect.
        assert_eq!(chapter_rect(&chapter(3, 3, "c"), 5, CHIP_W), [102.0, 0.0]);
        assert_eq!(chapter_rect(&chapter(0, 2, "c"), 0, CHIP_W), [0.0, 0.0]);
    }

    #[test]
    fn chapter_rows_share_when_separate_and_stack_when_overlapping() {
        let chapters = vec![chapter(0, 2, "a"), chapter(2, 4, "b"), chapter(5, 6, "c")];
        assert_eq!(chapter_rows(&chapters, 10, CHIP_W), vec![Some(0), Some(0), Some(0)]);
        assert_eq!(chapter_row_count(&chapter_rows(&chapters, 10, CHIP_W)), 1);

        // Overlapping chapters stack; a later chapter
        // fits back onto the first row once it clears.
        let chapters = vec![chapter(0, 4, "a"), chapter(2, 6, "b"), chapter(4, 8, "c")];
        assert_eq!(chapter_rows(&chapters, 10, CHIP_W), vec![Some(0), Some(1), Some(0)]);
        assert_eq!(chapter_row_count(&chapter_rows(&chapters, 10, CHIP_W)), 2);
    }

    #[test]
    fn chapter_rows_give_no_row_to_chapters_that_draw_nothing() {
        let chapters = vec![chapter(0, 2, "a"), chapter(3, 3, "empty"), chapter(9, 12, "past")];
        let rows = chapter_rows(&chapters, 5, CHIP_W);
        assert_eq!(rows, vec![Some(0), None, None]);
        assert_eq!(chapter_row_count(&rows), 1);
        assert_eq!(chapter_row_count(&[]), 0);
    }

    #[test]
    fn cue_chapter_binds_by_the_cues_start_frame() {
        let mut s = dlg_state(
            20,
            Vec::new(),
            vec![chapter(0, 5, "Opening"), chapter(8, 12, "The Road")],
        );
        let project = &s.core.project;
        // A cue starting inside a chapter binds to it.
        assert_eq!(cue_chapter(project, &cue(2, 4, "x")), Some(0));
        assert_eq!(cue_chapter(project, &cue(9, 20, "x")), Some(1));
        // A cue starting in the gap belongs to none.
        assert_eq!(cue_chapter(project, &cue(6, 7, "x")), None);
        // Half-open: starting exactly at a chapter's end
        // is not starting in it.
        assert_eq!(cue_chapter(project, &cue(5, 6, "x")), None);
        assert_eq!(cue_chapter(project, &cue(12, 14, "x")), None);
        // A cue that starts inside and runs past the end
        // still binds where it begins.
        assert_eq!(cue_chapter(project, &cue(4, 15, "x")), Some(0));
        // No chapters: nothing binds.
        s.core.project.chapters = Vec::new();
        assert_eq!(cue_chapter(&s.core.project, &cue(0, 2, "x")), None);
    }

    #[test]
    fn cue_chapter_overlap_resolves_to_the_first_in_episode_order() {
        let s = dlg_state(
            20,
            Vec::new(),
            vec![chapter(0, 10, "Outer"), chapter(3, 6, "Inner")],
        );
        assert_eq!(cue_chapter(&s.core.project, &cue(4, 5, "x")), Some(0));
    }

    #[test]
    fn line_label_is_the_subtitle_lanes_composition() {
        // Credited: the `Name: ` prefix, the panel rows'
        // composition, shared with the subtitle lane.
        assert_eq!(
            line_label(&credited_cue(0, 10, "The long road home.", "Adult Son")),
            "Adult Son: The long road home."
        );
        // Uncredited: the raw text, byte for byte.
        assert_eq!(line_label(&cue(0, 10, "one\ntwo")), "one\ntwo");
        assert_eq!(line_label(&cue(0, 10, "")), "");
        // The text threshold is the subtitle lane's too.
        assert!(!line_shows_text(CHIP_W, CHIP_W));
        assert!(line_shows_text(200.0, CHIP_W));
    }

    #[test]
    fn line_label_carries_a_credited_multiline_line_verbatim() {
        // Roadmap Wave 2 (W2-17): the last label
        // combination — credited AND multi-line. The
        // prefix rides the first line and the stored
        // break survives byte for byte, exactly the
        // subtitle lane's composition the lane delegates
        // to (and, through it, the renderers' reading).
        assert_eq!(
            line_label(&credited_cue(0, 10, "one\ntwo", "Adult Son")),
            "Adult Son: one\ntwo"
        );
        assert_eq!(
            line_label(&credited_cue(0, 10, "one\ntwo", "Adult Son")),
            crate::rune_sec::subtitle_lane::block_label(&credited_cue(
                0,
                10,
                "one\ntwo",
                "Adult Son"
            ))
        );
    }

    #[test]
    fn line_rows_delegate_to_the_cue_packing() {
        let cues = vec![cue(0, 4, "a"), cue(2, 6, "b"), cue(4, 8, "c")];
        assert_eq!(line_rows(&cues, 10, CHIP_W), cue_rows(&cues, 10, CHIP_W));
        assert_eq!(line_rows(&cues, 10, CHIP_W), vec![Some(0), Some(1), Some(0)]);
        assert_eq!(line_row_count(&line_rows(&cues, 10, CHIP_W)), 2);
        // Pitch threading: the same windows at half
        // zoom pack identically (packing is judged on
        // drawn rects, and every rect halves together).
        assert_eq!(
            line_rows(&cues, 10, CHIP_W / 2.0),
            vec![Some(0), Some(1), Some(0)]
        );
    }

    #[test]
    fn press_line_selects_and_seeks_on_the_shared_selection() {
        let mut s = dlg_state(10, vec![cue(0, 2, "a"), cue(4, 7, "b")], Vec::new());
        press_line(&mut s, 1);
        assert_eq!(s.subtitle_selected, Some(1), "the ONE subtitle selection");
        assert_eq!(s.core.current, 4, "the playhead seeks the line's first frame");
        assert_eq!(s.core.undo_depth(), 0, "a press spends no undo step");
        // A stale index does nothing at all.
        press_line(&mut s, 9);
        assert_eq!(s.subtitle_selected, Some(1));
        assert_eq!(s.core.current, 4);
    }

    #[test]
    fn press_chapter_seeks_and_writes_nothing_else() {
        let mut s = dlg_state(
            20,
            vec![cue(0, 2, "a")],
            vec![chapter(0, 5, "Opening"), chapter(8, 12, "The Road")],
        );
        press_chapter(&mut s, 1);
        assert_eq!(s.core.current, 8, "the playhead seeks the chapter's first frame");
        assert_eq!(s.subtitle_selected, None, "no selection is invented for a chapter");
        assert_eq!(s.core.undo_depth(), 0, "navigation spends no undo step");
        // A stale index does nothing at all.
        press_chapter(&mut s, 7);
        assert_eq!(s.core.current, 8);
    }

    #[test]
    fn dialogue_present_is_cues_or_chapters() {
        let s = dlg_state(10, Vec::new(), Vec::new());
        assert!(!dialogue_present(&s.core.project));
        let s = dlg_state(10, vec![cue(0, 2, "a")], Vec::new());
        assert!(dialogue_present(&s.core.project));
        let s = dlg_state(10, Vec::new(), vec![chapter(0, 5, "Opening")]);
        assert!(dialogue_present(&s.core.project));
    }

    #[test]
    fn lane_constructs_on_every_state_branch() {
        // Element trees are not readable from unit tests
        // — construction is the assertable level here
        // (the design kit's own discipline); the
        // presented-tree proof belongs to the headless
        // snaps (W2-77's dialogue-lane scenes).
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // Neither cues nor chapters: no lane.
            let s = dlg_state(10, Vec::new(), Vec::new());
            assert!(dialogue_lane(&s, &t).is_none());
            // Cues only: header + line rows, no band.
            let s = dlg_state(10, vec![cue(1, 4, "Hello.")], Vec::new());
            assert!(dialogue_lane(&s, &t).is_some());
            // Chapters only: header + band + empty state.
            let s = dlg_state(10, Vec::new(), vec![chapter(0, 10, "Opening")]);
            assert!(dialogue_lane(&s, &t).is_some());
            // Both, with overlap stacking in both
            // systems and a selected line among them.
            let mut s = dlg_state(
                10,
                vec![cue(0, 6, "a"), cue(2, 8, "b")],
                vec![chapter(0, 6, "One"), chapter(3, 10, "Two")],
            );
            s.subtitle_selected = Some(1);
            assert!(dialogue_lane(&s, &t).is_some());
            // A chapter wholly past the end draws no
            // segment but the lane still constructs.
            let s = dlg_state(5, vec![cue(0, 2, "a")], vec![chapter(9, 12, "Past")]);
            assert!(dialogue_lane(&s, &t).is_some());
            // No frames at all: the header's bare
            // branch, nothing draws.
            let s = dlg_state(0, vec![cue(0, 2, "a")], vec![chapter(0, 2, "c")]);
            assert!(dialogue_lane(&s, &t).is_some());
            // A stale selection tints nothing and breaks
            // nothing.
            let mut s = dlg_state(10, vec![cue(0, 2, "a")], Vec::new());
            s.subtitle_selected = Some(99);
            assert!(dialogue_lane(&s, &t).is_some());
        }
    }
}
