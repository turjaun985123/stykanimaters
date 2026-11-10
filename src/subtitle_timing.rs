//! Dialogue timing assistant: reading-time measurement
//! and fix PROPOSALS for subtitle cues (Roadmap Wave 2,
//! W2-16 — the proposal half of the work RM21's P-11
//! measurement began).
//!
//! ## The model (all of it — there is no hidden half)
//!
//! A cue's reading load is measured exactly the way
//! RM21's panel feedback measures it: the text's
//! characters excluding line breaks (a break is layout,
//! not reading) over the half-open window's seconds at
//! the project rate — characters per second (CPS).
//!
//! A cue FITS its text when its window holds the frames
//! the text needs at the reader's pace:
//!
//! ```text
//! needed_frames(chars) = max(
//!     ceil(chars / target_cps * fps),   // the speed term
//!     ceil(min_seconds * fps),          // the floor term
//! )
//! ```
//!
//! The two parameters are the whole model, and both are
//! ADJUSTABLE (the panel steps them; the MCP tools take
//! them as arguments):
//!
//! * `target_cps` — the reading speed the fixes aim
//!   for, default 17.0 chars/s. It is a pace the author
//!   sets, NOT a warning threshold: this module renders
//!   no pass/fail verdict on any cue, flags nothing, and
//!   changes nothing on its own. (The standing D-9
//!   question — a default CPS *warning* threshold — is
//!   deliberately untouched: a proposal says "at YOUR
//!   target pace this cue needs N more frames, here is
//!   a concrete fix", never "this cue is too fast".)
//! * `min_seconds` — the shortest time any cue should
//!   sit on screen, default 1.0 s, so a two-word cue in
//!   a six-frame flash still earns a fix proposal.
//!
//! ## The fixes, in preference order
//!
//! For each cue whose window is shorter than its need,
//! the first applicable fix is proposed:
//!
//! 1. **Extend** — the following dead air (the gap to
//!    the next cue's start; unbounded for the last cue)
//!    covers the need: the cue's end moves to
//!    `start + needed_frames`. One cue stays one cue.
//! 2. **Split** — the text is dense (its speed term
//!    alone exceeds the window) and a word boundary
//!    splits it into two cues whose combined need fits
//!    the span the original window plus dead air
//!    offers: the pair replaces the cue inside that
//!    span. See the verbatim rule below.
//! 3. **Split + shift** — the pair's combined need
//!    exceeds the offered span: the pair takes the time
//!    it needs and every FOLLOWING cue shifts later by
//!    the deficit (`shift_following_by`), preserving
//!    their windows and gaps. The chain always has room
//!    at the end of the list — cue frames are not
//!    clamped to the timeline (the core ops' rule) — so
//!    this fix always exists when a split does. Its
//!    cost is stated on the proposal, never hidden.
//! 4. **Extend (partial)** — no split exists (a single
//!    long word, say) but some dead air follows: the
//!    cue grows by what air exists. The proposal's
//!    after-CPS stays above the target and says so in
//!    its numbers — a partial fix, honestly labelled by
//!    arithmetic rather than adjectives.
//!
//! A cue no fix can help (dense text, no boundary, no
//! dead air) is counted in the report's `stuck` — the
//! assistant reports it instead of faking a fix.
//!
//! ## The verbatim rule (a hard user rule)
//!
//! Splitting NEVER alters a character of dialogue. A
//! split point is a single space whose neighbours are
//! both non-space, non-break characters; the left cue
//! takes the text before it, the right cue the text
//! after it, so `left + " " + right` reproduces the
//! original text EXACTLY (pinned by test). Line breaks
//! are never split points (a break replaced by a space
//! would be an alteration). Both halves inherit the
//! cue's speaker. Extends and shifts touch no text at
//! all.
//!
//! ## Discipline
//!
//! Everything here is pure: proposals are computed from
//! a cue slice, and [`apply_proposals`] turns a chosen
//! subset into a new list positionally (indices name
//! the measured list; replacements land where their cue
//! sat, chain shifts accumulate in index order), the
//! result re-sorted by the list's standing rule. The
//! applying surfaces (the panel, the MCP tools) own the
//! undo accounting — one labelled snapshot per apply
//! press, on the bulk discipline RM21's shift-all /
//! retime established — this module spends nothing.

use crate::subtitles::{sort_subtitle_cues, SubtitleCue};

/// The default target reading speed, in characters per
/// second: a mid-range adult reading pace for captions.
/// A model parameter the author adjusts — NOT a warning
/// threshold (see the module docs).
pub const DEFAULT_TARGET_CPS: f32 = 17.0;

/// The default minimum on-screen time for any cue, in
/// seconds: below this a cue is a flash, however little
/// text it carries.
pub const DEFAULT_MIN_SECONDS: f32 = 1.0;

/// The target speed's adjustable range (the panel
/// steppers' and the argument validation's bounds):
/// below 5 chars/s is slower than any caption pace,
/// above 40 faster than any reader.
pub const TARGET_CPS_MIN: f32 = 5.0;
pub const TARGET_CPS_MAX: f32 = 40.0;

/// The minimum-duration parameter's upper bound: a
/// floor past three seconds would stretch every short
/// cue into the next one.
pub const MIN_SECONDS_MAX: f32 = 3.0;

/// The reading model's two adjustable parameters (the
/// module docs carry the full statement of the model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingParams {
    /// Reading speed the fixes aim for, chars/s.
    pub target_cps: f32,
    /// Shortest time any cue should sit, seconds.
    pub min_seconds: f32,
}

impl Default for TimingParams {
    fn default() -> Self {
        TimingParams {
            target_cps: DEFAULT_TARGET_CPS,
            min_seconds: DEFAULT_MIN_SECONDS,
        }
    }
}

impl TimingParams {
    /// The parameters held to their honest ranges: a
    /// non-finite or out-of-range value falls back /
    /// clamps rather than poisoning the arithmetic (a
    /// zero or negative target would divide by zero;
    /// the surfaces step inside the ranges already, so
    /// this is the backstop for hand-passed arguments).
    pub fn sanitised(self) -> TimingParams {
        let target_cps = if self.target_cps.is_finite() {
            self.target_cps.clamp(TARGET_CPS_MIN, TARGET_CPS_MAX)
        } else {
            DEFAULT_TARGET_CPS
        };
        let min_seconds = if self.min_seconds.is_finite() {
            self.min_seconds.clamp(0.0, MIN_SECONDS_MAX)
        } else {
            DEFAULT_MIN_SECONDS
        };
        TimingParams {
            target_cps,
            min_seconds,
        }
    }
}

/// A text's readable character count: Unicode scalars
/// with line breaks excluded — RM21's panel measure
/// (`cue_cps` there), restated here so the lib, the
/// panel and the MCP tools all count identically.
pub fn cue_char_count(text: &str) -> usize {
    text.chars().filter(|&c| c != '\n').count()
}

/// Whether `fps` names a usable time domain — the rate
/// guard every time calculation in the subtitle stack
/// shares (no rate, no measurement).
fn rate_ok(fps: f32) -> bool {
    fps > 0.0 && fps.is_finite()
}

/// A cue's measured reading speed at `fps`: readable
/// characters over the half-open window's seconds. A
/// rate with no time domain measures 0.0 (the display
/// guard, not a verdict).
pub fn cue_cps(cue: &SubtitleCue, fps: f32) -> f32 {
    if !rate_ok(fps) {
        return 0.0;
    }
    let seconds = (cue.end_frame - cue.start_frame) as f32 / fps;
    if seconds <= 0.0 {
        return 0.0;
    }
    cue_char_count(&cue.text) as f32 / seconds
}

/// The frames `chars` readable characters need at
/// `fps` under `params`: the speed term and the floor
/// term of the module doc's formula, the larger of the
/// two, and never less than one frame (a cue exists or
/// it does not). No usable rate needs nothing — the
/// report over such a rate is empty by construction.
pub fn needed_frames(chars: usize, params: &TimingParams, fps: f32) -> usize {
    if !rate_ok(fps) {
        return 0;
    }
    let by_speed = (chars as f32 / params.target_cps * fps).ceil();
    let by_floor = (params.min_seconds * fps).ceil();
    let frames = by_speed.max(by_floor);
    if frames < 1.0 {
        1
    } else {
        frames as usize
    }
}

/// Which fix a proposal makes (the module docs'
/// preference order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalKind {
    /// The cue's end moves later (fully or partially —
    /// the proposal's `cps_after` tells which).
    Extend,
    /// The cue becomes two cues at a word boundary,
    /// possibly with the following chain shifted
    /// (`shift_following_by` > 0).
    Split,
}

/// One concrete, applicable fix for one cue of the
/// measured list. Applying it replaces the cue at
/// `cue_index` with `replacement` (one cue for an
/// extend, two for a split) and — when
/// `shift_following_by` is non-zero — moves every
/// later cue that many frames later.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    /// Index of the cue this fix addresses, in the
    /// list the report was computed from.
    pub cue_index: usize,
    /// The fix's shape.
    pub kind: ProposalKind,
    /// The cue(s) that replace the addressed cue, in
    /// window order, against the measured list's frame
    /// domain (an applying surface adds any shift an
    /// earlier applied proposal owes them — see
    /// [`apply_proposals`]).
    pub replacement: Vec<SubtitleCue>,
    /// Extra frames every cue AFTER the addressed one
    /// moves later when this fix applies (the split +
    /// shift chain cost; 0 for every other fix).
    pub shift_following_by: usize,
    /// The addressed cue's readable character count.
    pub chars: usize,
    /// The addressed cue's measured CPS before the fix.
    pub cps_before: f32,
    /// The reading speed the fix lands at: the extended
    /// cue's CPS, or for a split the SLOWER half's CPS
    /// (the pair is only as readable as its denser
    /// half).
    pub cps_after: f32,
}

/// A measurement pass over a cue list: the fixes
/// proposed, in cue order, and how many needy cues no
/// fix could help.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimingReport {
    /// One proposal per fixable needy cue, cue order.
    pub proposals: Vec<Proposal>,
    /// Cues whose window is shorter than their need
    /// but which no fix could help (dense unsplittable
    /// text with no following dead air).
    pub stuck: usize,
}

/// Plan a split of `cue` at the best word boundary, or
/// `None` when no boundary yields two valid halves.
/// "Best" minimises the pair's combined need
/// (`needed_frames` of the two halves — the time the
/// pair must occupy); ties break toward the more
/// balanced character split, then toward the earlier
/// boundary, so the plan is deterministic. The pair is
/// allocated exactly its combined need — no more time
/// than the text earns, so a chain shift stays as
/// small as the fix allows — with the boundary frame
/// giving the left half exactly its own need.
fn plan_split(
    cue: &SubtitleCue,
    params: &TimingParams,
    fps: f32,
) -> Option<(SubtitleCue, SubtitleCue, usize)> {
    let text = &cue.text;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut best: Option<(usize, usize, usize, usize)> = None; // (pair_need, imbalance, byte_idx, left_chars)
    for w in 1..chars.len().saturating_sub(1) {
        let (byte_idx, ch) = chars[w];
        if ch != ' ' {
            continue;
        }
        let prev = chars[w - 1].1;
        let next = chars[w + 1].1;
        if prev == ' ' || prev == '\n' || next == ' ' || next == '\n' {
            continue;
        }
        let left = &text[..byte_idx];
        let right = &text[byte_idx + 1..];
        let left_chars = cue_char_count(left);
        let right_chars = cue_char_count(right);
        let pair_need =
            needed_frames(left_chars, params, fps) + needed_frames(right_chars, params, fps);
        let imbalance = left_chars.abs_diff(right_chars);
        let better = match best {
            None => true,
            Some((bp, bi, _, _)) => pair_need < bp || (pair_need == bp && imbalance < bi),
        };
        if better {
            best = Some((pair_need, imbalance, byte_idx, left_chars));
        }
    }
    let (pair_need, _, byte_idx, left_chars) = best?;
    let left_text = &text[..byte_idx];
    let right_text = &text[byte_idx + 1..];
    let left_frames = needed_frames(left_chars, params, fps);
    let start = cue.start_frame;
    let left_cue = SubtitleCue {
        start_frame: start,
        end_frame: start + left_frames,
        text: left_text.to_string(),
        speaker: cue.speaker.clone(),
    };
    let right_cue = SubtitleCue {
        start_frame: start + left_frames,
        end_frame: start + pair_need,
        text: right_text.to_string(),
        speaker: cue.speaker.clone(),
    };
    // The halves must be cues the ops would accept
    // (non-empty windows, non-whitespace text) — the
    // boundary rule already guarantees both, and this
    // check keeps a hand-authored edge (a text of
    // nothing but spaces around one word) honest.
    if SubtitleCue::validate(left_cue.start_frame, left_cue.end_frame, &left_cue.text).is_err()
        || SubtitleCue::validate(right_cue.start_frame, right_cue.end_frame, &right_cue.text)
            .is_err()
    {
        return None;
    }
    Some((left_cue, right_cue, pair_need))
}

/// Measure every cue in `cues` (the list in its kept,
/// sorted order) at `fps` under `params` and propose
/// the first applicable fix for each cue whose window
/// is shorter than its text's need — the preference
/// order of the module docs. Pure: nothing is changed,
/// nothing is judged; a cue at or under the author's
/// target pace earns no proposal and no remark.
pub fn timing_report(cues: &[SubtitleCue], fps: f32, params: &TimingParams) -> TimingReport {
    let mut report = TimingReport::default();
    if !rate_ok(fps) {
        return report;
    }
    let params = params.sanitised();
    for (i, cue) in cues.iter().enumerate() {
        let duration = cue.end_frame - cue.start_frame;
        let chars = cue_char_count(&cue.text);
        let need = needed_frames(chars, &params, fps);
        if need <= duration {
            continue;
        }
        let cps_before = cue_cps(cue, fps);
        // The dead air after this cue: frames between
        // its end and the next cue's start (the list is
        // sorted, so that is the following entry), or
        // unbounded for the last cue — cue frames are
        // not clamped to the timeline.
        let gap = cues
            .get(i + 1)
            .map(|next| next.start_frame.saturating_sub(cue.end_frame));
        let avail = gap.map(|g| duration + g).unwrap_or(usize::MAX);
        // 1. Extend, when the air covers the need.
        if avail >= need {
            let mut grown = cue.clone();
            grown.end_frame = cue.start_frame + need;
            report.proposals.push(Proposal {
                cue_index: i,
                kind: ProposalKind::Extend,
                cps_after: cue_cps(&grown, fps),
                replacement: vec![grown],
                shift_following_by: 0,
                chars,
                cps_before,
            });
            continue;
        }
        // 2./3. Split, when the text is dense (its speed
        // term alone overflows the window — a merely
        // sub-floor flash cue is not a split candidate)
        // and a boundary plan exists.
        let speed_need = (chars as f32 / params.target_cps * fps).ceil() as usize;
        if speed_need > duration {
            if let Some((left, right, pair_need)) = plan_split(cue, &params, fps) {
                let shift = if pair_need > avail {
                    // The pair runs past the next cue's
                    // start: the chain owes the overrun.
                    // (pair_need > avail implies a next
                    // cue exists — the last cue's avail
                    // is unbounded.)
                    let pair_end = cue.start_frame + pair_need;
                    pair_end.saturating_sub(cues[i + 1].start_frame)
                } else {
                    0
                };
                let cps_after = cue_cps(&left, fps).max(cue_cps(&right, fps));
                report.proposals.push(Proposal {
                    cue_index: i,
                    kind: ProposalKind::Split,
                    replacement: vec![left, right],
                    shift_following_by: shift,
                    chars,
                    cps_before,
                    cps_after,
                });
                continue;
            }
        }
        // 4. Partial extend, when some air exists.
        if let Some(g) = gap {
            if g > 0 {
                let mut grown = cue.clone();
                grown.end_frame = cue.start_frame + avail;
                report.proposals.push(Proposal {
                    cue_index: i,
                    kind: ProposalKind::Extend,
                    cps_after: cue_cps(&grown, fps),
                    replacement: vec![grown],
                    shift_following_by: 0,
                    chars,
                    cps_before,
                });
                continue;
            }
        }
        // No fix exists for this cue: report it.
        report.stuck += 1;
    }
    report
}

/// Apply `proposals` (any subset of a report computed
/// from THIS `cues` list, in any order) and return the
/// new list, re-sorted by the standing rule. Positional
/// discipline: each addressed cue is replaced where it
/// sat; a split + shift proposal's `shift_following_by`
/// accumulates onto every later cue — INCLUDING the
/// replacements of later proposals in the same batch,
/// whose windows were computed against the unshifted
/// list. Unaddressed cues keep their text, speaker and
/// duration; only a chain shift moves them.
pub fn apply_proposals(cues: &[SubtitleCue], proposals: &[Proposal]) -> Vec<SubtitleCue> {
    let mut out: Vec<SubtitleCue> = Vec::with_capacity(cues.len() + proposals.len());
    let mut shift = 0usize;
    for (i, cue) in cues.iter().enumerate() {
        match proposals.iter().find(|p| p.cue_index == i) {
            Some(p) => {
                for mut repl in p.replacement.iter().cloned() {
                    repl.start_frame += shift;
                    repl.end_frame += shift;
                    out.push(repl);
                }
                shift += p.shift_following_by;
            }
            None => {
                let mut moved = cue.clone();
                moved.start_frame += shift;
                moved.end_frame += shift;
                out.push(moved);
            }
        }
    }
    sort_subtitle_cues(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All timing pins run at 24 fps explicitly (the
    /// timing-test rule).
    const FPS: f32 = 24.0;

    fn cue(start: usize, end: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    fn params() -> TimingParams {
        TimingParams::default()
    }

    #[test]
    fn needed_frames_is_the_documented_formula() {
        let p = params();
        // Speed term: 34 chars at 17 chars/s = 2.0 s =
        // 48 frames at 24 fps; the floor (1.0 s = 24)
        // does not bind.
        assert_eq!(needed_frames(34, &p, FPS), 48);
        // 17 chars = exactly 1.0 s = 24 frames; speed
        // and floor agree.
        assert_eq!(needed_frames(17, &p, FPS), 24);
        // 18 chars tips the speed term past the floor:
        // ceil(18 / 17 * 24) = ceil(25.41) = 26.
        assert_eq!(needed_frames(18, &p, FPS), 26);
        // A whisper of text still earns the floor.
        assert_eq!(needed_frames(2, &p, FPS), 24);
        // No usable rate, no need.
        assert_eq!(needed_frames(34, &p, 0.0), 0);
    }

    #[test]
    fn params_sanitise_holds_the_ranges() {
        let wild = TimingParams {
            target_cps: 400.0,
            min_seconds: f32::NAN,
        }
        .sanitised();
        assert_eq!(wild.target_cps, TARGET_CPS_MAX);
        assert_eq!(wild.min_seconds, DEFAULT_MIN_SECONDS);
        let zero = TimingParams {
            target_cps: 0.0,
            min_seconds: -1.0,
        }
        .sanitised();
        assert_eq!(zero.target_cps, TARGET_CPS_MIN);
        assert_eq!(zero.min_seconds, 0.0);
    }

    #[test]
    fn a_comfortable_cue_earns_no_proposal() {
        // 34 chars over 48 frames = 17.0 chars/s,
        // exactly the target pace, duration at need.
        let cues = vec![cue(0, 48, "aaaa bbbb cccc dddd eeee ffff gggg")];
        assert_eq!(cue_char_count(&cues[0].text), 34);
        let report = timing_report(&cues, FPS, &params());
        assert!(report.proposals.is_empty());
        assert_eq!(report.stuck, 0);
    }

    #[test]
    fn extend_uses_the_following_dead_air() {
        // 34 chars in 24 frames (34 chars/s) with the
        // next cue far away: extend to the full need.
        let cues = vec![
            cue(0, 24, "aaaa bbbb cccc dddd eeee ffff gggg"),
            cue(96, 120, "later"),
        ];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.cue_index, 0);
        assert_eq!(p.kind, ProposalKind::Extend);
        assert_eq!(p.shift_following_by, 0);
        assert_eq!(p.replacement, vec![cue(0, 48, "aaaa bbbb cccc dddd eeee ffff gggg")]);
        assert_eq!(p.cps_before, 34.0);
        assert_eq!(p.cps_after, 17.0);
        // The neighbour was comfortable and unaddressed.
        let after = apply_proposals(&cues, &report.proposals);
        assert_eq!(after.len(), 2);
        assert_eq!(after[1], cue(96, 120, "later"));
    }

    #[test]
    fn a_flash_cue_extends_to_the_floor() {
        // Two characters in 6 frames: the pace is fine
        // (8 chars/s) but the floor term (24 frames)
        // exceeds the window — an extend, never a split.
        let cues = vec![cue(0, 6, "Hi"), cue(48, 72, "there")];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.kind, ProposalKind::Extend);
        assert_eq!(p.replacement[0].end_frame, 24);
    }

    #[test]
    fn extend_beats_split_when_the_air_covers_the_need() {
        // Preference-order pin: 39 chars in 24 frames
        // need 56; the next cue leaves 48 frames of air,
        // so the offered span (72) covers the need and
        // the fix is an EXTEND — a split is proposed
        // only when the air runs out (the next tests).
        let cues = vec![
            cue(0, 24, "aaaa bbbb cccc dddd eeee ffff gggg hhhh"),
            cue(72, 96, "later"),
        ];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        assert_eq!(report.proposals[0].kind, ProposalKind::Extend);
    }

    #[test]
    fn split_at_a_word_boundary_when_air_runs_out() {
        // 39 chars need 56 frames; window 24 + air 16 =
        // span 40 < 56, so no extend. Halves of 19 chars
        // need 27 each: pair need 54 > 40 — the chain
        // owes 54 - 40... the pair ends at 54, the next
        // cue starts at 40, so the shift is 14.
        let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
        let cues = vec![cue(0, 24, text), cue(40, 64, "next")];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.kind, ProposalKind::Split);
        assert_eq!(p.replacement.len(), 2);
        assert_eq!(p.shift_following_by, 14);
        // Verbatim: the halves rejoin to the original
        // with exactly the split space.
        let joined = format!(
            "{} {}",
            p.replacement[0].text, p.replacement[1].text
        );
        assert_eq!(joined, text);
        // Both halves fit the target pace.
        assert!(p.cps_after <= 17.0, "after {} <= target", p.cps_after);
        // The pair is contiguous and starts where the
        // cue started.
        assert_eq!(p.replacement[0].start_frame, 0);
        assert_eq!(p.replacement[0].end_frame, p.replacement[1].start_frame);
        assert_eq!(p.replacement[1].end_frame, 54);
        // Applying shifts the following cue by the
        // stated deficit and nothing else about it.
        let after = apply_proposals(&cues, &report.proposals);
        assert_eq!(after.len(), 3);
        assert_eq!(after[2], cue(54, 78, "next"));
    }

    #[test]
    fn split_without_a_chain_when_the_pair_fits_the_span() {
        // Window 24 + air 32 = span 56. The whole need
        // is 56 — extend would need avail >= need, and
        // 56 >= 56 holds, so this fixture narrows the
        // air by a frame: span 55 < 56 defeats extend,
        // while the pair (27 + 27 = 54) fits the span.
        let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
        let cues = vec![cue(0, 24, text), cue(55, 79, "next")];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.kind, ProposalKind::Split);
        assert_eq!(p.shift_following_by, 0);
        assert_eq!(p.replacement[1].end_frame, 54);
        let after = apply_proposals(&cues, &report.proposals);
        assert_eq!(after[2], cue(55, 79, "next"), "the neighbour never moved");
    }

    #[test]
    fn splits_never_alter_a_character() {
        // A battery of shapes: multi-space runs and a
        // line break are never split points, and every
        // proposed split rejoins verbatim.
        let texts = [
            "one two three four five six seven eight nine ten",
            "Hello world.  This  has  wide  gaps in it here",
            "First line here\nsecond line follows right after",
            "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj",
        ];
        for text in texts {
            let cues = vec![cue(0, 12, text), cue(13, 37, "next")];
            let report = timing_report(&cues, FPS, &params());
            for p in &report.proposals {
                if p.kind == ProposalKind::Split {
                    let joined = format!(
                        "{} {}",
                        p.replacement[0].text, p.replacement[1].text
                    );
                    assert_eq!(joined, text, "split of {text:?} stays verbatim");
                    assert!(!p.replacement[0].text.ends_with(' '));
                    assert!(!p.replacement[1].text.starts_with(' '));
                }
            }
        }
    }

    #[test]
    fn a_dense_unsplittable_cue_with_no_air_is_stuck_not_faked() {
        // One long word, boxed in by the next cue: no
        // boundary, no air — the report counts it.
        let cues = vec![
            cue(0, 24, "antidisestablishmentarianismrocks"),
            cue(24, 48, "next"),
        ];
        let report = timing_report(&cues, FPS, &params());
        assert!(report.proposals.is_empty());
        assert_eq!(report.stuck, 1);
    }

    #[test]
    fn partial_extend_spends_what_air_exists() {
        // The same unsplittable word, but 12 frames of
        // air follow: a partial extend, its after-CPS
        // honestly still above target.
        let cues = vec![
            cue(0, 24, "antidisestablishmentarianismrocks"),
            cue(36, 60, "next"),
        ];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 1);
        let p = &report.proposals[0];
        assert_eq!(p.kind, ProposalKind::Extend);
        assert_eq!(p.replacement[0].end_frame, 36);
        assert!(p.cps_after > 17.0, "partial fix says so: {}", p.cps_after);
        assert_eq!(report.stuck, 0);
    }

    #[test]
    fn speakers_ride_both_halves_of_a_split() {
        let mut credited = cue(0, 24, "aaaa bbbb cccc dddd eeee ffff gggg hhhh");
        credited.speaker = Some("NOVA".to_string());
        let cues = vec![credited, cue(40, 64, "next")];
        let report = timing_report(&cues, FPS, &params());
        let p = &report.proposals[0];
        assert_eq!(p.kind, ProposalKind::Split);
        assert_eq!(p.replacement[0].speaker.as_deref(), Some("NOVA"));
        assert_eq!(p.replacement[1].speaker.as_deref(), Some("NOVA"));
    }

    #[test]
    fn chain_shifts_accumulate_across_a_batch() {
        // Two dense cues in a row, each splitting with a
        // chain shift: applying both moves the tail cue
        // by BOTH deficits, and the second pair lands
        // shifted by the first's.
        let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
        let cues = vec![cue(0, 24, text), cue(40, 64, text), cue(80, 104, "tail")];
        let report = timing_report(&cues, FPS, &params());
        assert_eq!(report.proposals.len(), 2);
        let total_shift: usize = report.proposals.iter().map(|p| p.shift_following_by).sum();
        assert!(total_shift > 0);
        let after = apply_proposals(&cues, &report.proposals);
        assert_eq!(after.len(), 5);
        let tail = after.iter().find(|c| c.text == "tail").unwrap();
        assert_eq!(tail.start_frame, 80 + total_shift);
        assert_eq!(tail.end_frame, 104 + total_shift);
        // No text was harmed.
        let rejoined: Vec<String> = after
            .iter()
            .filter(|c| c.text != "tail")
            .map(|c| c.text.clone())
            .collect();
        assert_eq!(
            rejoined,
            vec![
                "aaaa bbbb cccc dddd".to_string(),
                "eeee ffff gggg hhhh".to_string(),
                "aaaa bbbb cccc dddd".to_string(),
                "eeee ffff gggg hhhh".to_string(),
            ]
        );
    }

    #[test]
    fn no_rate_no_report() {
        let cues = vec![cue(0, 24, "aaaa bbbb cccc dddd eeee ffff gggg")];
        let report = timing_report(&cues, 0.0, &params());
        assert!(report.proposals.is_empty());
        assert_eq!(report.stuck, 0);
    }

    #[test]
    fn measurement_matches_the_panels_cps_measure() {
        // The panel's cue_cps counts characters
        // excluding line breaks over the window: a
        // two-line cue of 11 + 11 chars over 1 s at
        // 24 fps measures 22 chars/s here too.
        let c = cue(0, 24, "hello world\nhello again");
        assert_eq!(cue_char_count(&c.text), 22);
        assert_eq!(cue_cps(&c, FPS), 22.0);
    }
}
