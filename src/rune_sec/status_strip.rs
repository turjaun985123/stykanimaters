//! Status strip for the Rune editor shell (Phase D, section
//! 27): a segmented, separator-divided replacement for the
//! one-line `chrome_view::status_text`. Pure builder — it
//! reads `EditorCore` and returns an element tree; it
//! attaches no handlers and mutates nothing.
//!
//! Every value shown is read live from the core:
//!
//! - project name — `core.project.name`
//!   (`model.rs:682`), with a ` ●` suffix while the
//!   project has edits the autosave has not captured
//!   (see [`project_text`])
//! - frame position — `core.current` + 1 over
//!   `core.frame_count()` (`editor.rs:33`, `editor.rs:192`)
//! - timecode — current frame at `core.project.fps`
//!   (`model.rs:683`), via the pure [`timecode`] fn,
//!   rounded exactly as the playback panel's readout
//!   (`rune_sec::playback_controls::format_timecode`)
//! - fps — `core.project.fps`, formatted as in chrome
//! - zoom — `core.camera_here().2` (`editor.rs:530`), the
//!   camera key zoom at the current frame
//! - selection — `core.selected_figure` /
//!   `core.selected_node` (`editor.rs:34-35`) with the
//!   figure's `FigureInstance::name` (`model.rs:404`)
//! - undo depth — `core.undo_depth()` /
//!   `core.redo_depth()` (`editor.rs:250`, `editor.rs:255`),
//!   the Phase E accessors over the private stacks
//!   (see [`undo_text`])
//! - play state — `core.playing` (`editor.rs:38`)
//! - last status message — `core.status` (`editor.rs:42`),
//!   the trailing segment chrome's line ended with, kept so
//!   Save/Load feedback stays visible
//!
//! Phase E closed the two gaps Phase D had to leave open:
//! the counters behind the dirty marker and the undo
//! readout were private then. The dirty signal is the
//! public `edits_since_autosave()` (`editor.rs:1587`) —
//! the same counter the project section's autosave line
//! reads — and Phase E section 7 added the
//! `undo_depth()` / `redo_depth()` accessors. Both are
//! read live here; nothing is derived from side channels
//! such as sniffing `core.status`.
//!
//! ## Width discipline (Phase F fix)
//!
//! On a real 1440×900 screen the strip's tail painted
//! *over* the neighbouring text. Root cause, verified
//! against the pinned framework source: the old builder
//! put nine plain `text()` segments in a `w_full` row
//! with no sizing at all. Rune text defaults to one
//! unwrapped, untruncated line (`wrap: false`,
//! `truncate: false` — `crates/rune/src/el.rs:649-651`),
//! so its glyphs always paint at full natural width;
//! but taffy's flex items all default to
//! `flex_shrink: 1.0` (taffy `style/mod.rs:563`) with
//! the CSS automatic minimum size — min-content, i.e.
//! roughly the longest word (`compute/flexbox.rs`
//! §4.5). When the centre column is narrower than the
//! segments' total (at 1440 wide the column is ≈ 740 pt
//! and the initial status sentence alone is 67 chars),
//! every box shrinks toward word width while its glyphs
//! keep painting full-width — the tail worst of all,
//! spilling across the dividers and the column edge.
//!
//! The fix gives the row exactly one flexible member:
//!
//! - every fixed segment is floored at a conservative
//!   estimate of its own content width
//!   ([`est_text_width`]), so the shrink pass can never
//!   compress it below what its glyphs paint;
//! - the trailing status segment is the single
//!   absorber: `grow(1)` takes leftover width,
//!   Rune's one-line `.truncate()` elides the painted
//!   line with "…" to the box it actually gets (real
//!   font metrics, refit on resize), and [`fit_hint`]
//!   caps the string itself as a backstop so the line
//!   is bounded even before layout runs.
//!
//! All nine segments remain, in the same order, with
//! the same text sources.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, El};
use stykanimate_rs::editor::EditorCore;

/// Timecode of a frame at a project frame rate, formatted
/// `m:ss.f` (minutes unpadded, seconds zero-padded, one
/// tenths digit). Frame 0 is time zero. A non-positive or
/// non-finite fps cannot time anything and yields
/// `"0:00.0"`.
///
/// The maths is the playback panel's, ported exactly
/// (`playback_controls::format_timecode`, itself the GPUI
/// timecode chip's): the frame's time in seconds is
/// rounded once, to total tenths — half away from zero —
/// before being split into digits, so the seconds and
/// tenths digits can never disagree and a value a hair
/// under a minute (59.97 s) reads `1:00.0`, never
/// `0:60.0`. Phase D floored here instead, which made
/// the strip disagree with the playback readout by a
/// tenth on boundary frames (frame 7 at 12 fps is
/// 0.583 s: floor `0:00.5`, rounded `0:00.6`); Phase E
/// unifies on the playback rounding.
pub fn timecode(frame: usize, fps: f32) -> String {
    if !(fps > 0.0) || !fps.is_finite() {
        return "0:00.0".to_string();
    }
    let tenths = (frame as f32 / fps * 10.0).round() as u64;
    format!(
        "{}:{:02}.{}",
        tenths / 600,
        (tenths % 600) / 10,
        tenths % 10
    )
}

/// Project-name segment: the name, plus a ` ●` suffix
/// while the project is dirty. The signal is the core's
/// public edit counter, `edits_since_autosave()`
/// (`editor.rs:1587`) — edits counted since the core's
/// last autosave stamp (the autosave writes a recovery
/// snapshot every so many edits and re-stamps; the
/// project section's autosave line reads the same
/// counter). Dirty is exactly `edits_since_autosave() >
/// 0`; no other channel (status text, file mtimes) is
/// consulted.
pub fn project_text(core: &EditorCore) -> String {
    if core.edits_since_autosave() > 0 {
        format!("{} ●", core.project.name)
    } else {
        core.project.name.clone()
    }
}

/// Undo-depth segment: `Undo {n}` from
/// `core.undo_depth()` (`editor.rs:250`). When the redo
/// stack is non-empty the redo depth is appended to the
/// same segment — `Undo {n} · Redo {m}` — rather than
/// shown as its own segment: redo is only ever non-empty
/// right after undos, and a separate segment appearing
/// and disappearing would shift every later segment in
/// the strip sideways each time.
pub fn undo_text(core: &EditorCore) -> String {
    let redo = core.redo_depth();
    if redo > 0 {
        format!("Undo {} · Redo {}", core.undo_depth(), redo)
    } else {
        format!("Undo {}", core.undo_depth())
    }
}

/// Fps readout, formatted exactly as chrome's status line
/// does it: whole rates without a fraction (`"12"`),
/// fractional rates with one decimal (`"30.0"` for 29.97).
pub fn fps_text(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    }
}

/// Frame position segment: `Frame {current + 1}/{total}`
/// (1-based, the convention chrome's line and the timeline
/// chips both use).
pub fn frame_text(core: &EditorCore) -> String {
    format!("Frame {}/{}", core.current + 1, core.frame_count())
}

/// Zoom segment from the camera key at the current frame:
/// `Zoom {pct}%`, rounded the way chrome's line rounds it.
pub fn zoom_text(core: &EditorCore) -> String {
    let pct = (core.camera_here().2 * 100.0).round() as i64;
    format!("Zoom {pct}%")
}

/// Selection summary. With a node selected:
/// `"{figure} › node {n}"` — the node index as stored,
/// matching the core's own status phrasing
/// (`"figure {} node {}"`, `editor.rs:1149`). With only a
/// figure selected, the figure name alone. With no figure
/// at all (empty project or a stale index), `"No
/// selection"`.
pub fn selection_text(core: &EditorCore) -> String {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return "No selection".to_string();
    };
    match core.selected_node {
        Some(n) => format!("{} › node {}", fig.name, n),
        None => fig.name.clone(),
    }
}

/// Play-state segment: a state glyph plus the word, so the
/// meaning survives even if a glyph falls back in the
/// embedded font. `▶` while playing (media rolling),
/// `❚❚` while paused (bars held).
pub fn play_text(playing: bool) -> &'static str {
    if playing {
        "▶ Playing"
    } else {
        "❚❚ Paused"
    }
}

/// Conservative per-character advance (points) assumed
/// for the strip's label font: `type_::LABEL` is 12 pt
/// at weight 500 (`crates/rune/src/tok.rs:139`), where a
/// typical glyph advances ≈ 0.5 em (6 pt) and digits
/// ≈ 0.55 em. 8.0 pt (0.67 em) sits above the average
/// for mixed prose, digits and the strip's symbol
/// glyphs (`●`, `›`, `❚❚`), so a width estimated with it
/// overstates the true painted width — the safe
/// direction for a floor (see [`est_text_width`]). Only
/// a string made almost entirely of the widest glyphs
/// (capital `W`, `m`) could beat it, and then only under
/// simultaneous width pressure.
pub const LABEL_CHAR_W: f32 = 8.0;

/// Estimated painted width of `s` in the strip's label
/// font: character count × [`LABEL_CHAR_W`]. Deliberately
/// conservative (an overestimate) — it is used as a
/// minimum-width floor for the fixed segments, and a
/// floor only protects against overlap while it is at
/// least the true content width. The slack it adds is
/// absorbed by the flexible tail segment, never painted
/// over a neighbour.
pub fn est_text_width(s: &str) -> f32 {
    s.chars().count() as f32 * LABEL_CHAR_W
}

/// Cap applied to the status string itself (see
/// [`fit_hint`]). Generous on purpose — the framework's
/// own ellipsis does the precise fitting to the real
/// box; this only bounds pathological strings (long
/// file paths in error messages) before layout. The
/// core's initial hint is 67 chars, a typical status
/// far fewer.
pub const MAX_STATUS_CHARS: usize = 96;

/// Fit `hint` into at most `max_chars` characters,
/// ending with "…" when anything was cut. Pure and
/// total: the result never exceeds `max_chars`
/// characters (the ellipsis counts as one), is always
/// valid UTF-8 (cuts fall between characters), and a
/// hint that already fits is returned unchanged.
///
/// The cut prefers a word boundary: if the last space
/// inside the budget falls in the budget's second half,
/// the string is cut there instead of mid-word (a space
/// earlier than that would waste most of the budget, so
/// a hard character cut wins). Trailing spaces are
/// trimmed before the ellipsis. Degenerate budgets:
/// `max_chars == 0` yields an empty string and
/// `max_chars == 1` yields just "…".
pub fn fit_hint(hint: &str, max_chars: usize) -> String {
    let chars: Vec<char> = hint.chars().collect();
    if chars.len() <= max_chars {
        return hint.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    // One character of the budget is the ellipsis.
    let budget = max_chars - 1;
    let mut keep = budget;
    if let Some(pos) = chars[..budget].iter().rposition(|c| *c == ' ') {
        if pos >= budget / 2 {
            keep = pos;
        }
    }
    let mut out: String = chars[..keep].iter().collect();
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('…');
    out
}

/// One fixed strip segment: a single label-styled text,
/// floored at its estimated content width so the row's
/// shrink pass can never compress its box below the
/// width its glyphs paint (see the module docs).
fn segment(label: String, t: &Theme) -> El {
    let floor = est_text_width(&label);
    text(label).font(type_::LABEL).color(t.text).min_w(floor)
}

/// Hairline divider between segments. Floored at its
/// own 1 pt so width pressure cannot shrink it away to
/// nothing (the tail segment absorbs the shortage).
fn divider(t: &Theme) -> El {
    el().size(1.0, 12.0).min_w(1.0).fill(t.control_track)
}

/// The status strip: project name (with the dirty dot),
/// frame position, timecode, fps, zoom, selection, undo
/// depth, play state and the core's current status
/// message, divided by hairlines. The fixed segments
/// are width-floored and the status tail is the single
/// flexible, eliding segment, so the segments can never
/// overlap however narrow the strip gets (see the
/// module docs' width-discipline section).
/// Keyed `status-strip` so headless `--tree` checks can
/// find it. Theme follows the shell's current convention
/// (`main.rs` builds its view with `tok::theme(true)`).
pub fn status_strip(core: &EditorCore) -> El {
    let t = tok::theme(true);
    let segments: Vec<El> = vec![
        segment(project_text(core), &t),
        segment(frame_text(core), &t),
        segment(timecode(core.current, core.project.fps), &t),
        segment(format!("{} fps", fps_text(core.project.fps)), &t),
        segment(zoom_text(core), &t),
        segment(selection_text(core), &t),
        segment(undo_text(core), &t),
        // The play segment lights up in the accent colour
        // while playback is rolling. Fixed like the
        // others: floored at its estimated width.
        text(play_text(core.playing))
            .font(type_::LABEL)
            .color(if core.playing {
                tok::accent(0, true)
            } else {
                t.text
            })
            .min_w(est_text_width(play_text(core.playing))),
        // Chrome's line ended with the core's status
        // message; keep it, dimmed, as the tail segment —
        // and make it the row's single flexible member.
        // `grow(1)` gives it whatever width the fixed
        // segments leave; `.truncate()` puts it in
        // Rune's one-line mode (flex_shrink 1, minimum
        // width 0 — `crates/rune/src/el/text.rs`,
        // `one_line`), so under shortage its box, alone,
        // gives way and the painted line is elided with
        // "…" to that box at real font metrics. The
        // string is pre-capped by `fit_hint` as a
        // backstop. Fixed segments sit on their floors
        // and cannot be compressed, so no segment's
        // glyphs can ever reach a neighbour's box.
        text(fit_hint(&core.status, MAX_STATUS_CHARS))
            .font(type_::LABEL)
            .color(t.text_secondary)
            .grow(1.0)
            .truncate(),
    ];
    let mut bar = row().key("status-strip").w_full().gap(8.0).center_y();
    for (i, seg) in segments.into_iter().enumerate() {
        if i > 0 {
            bar = bar.child(divider(&t));
        }
        bar = bar.child(seg);
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_frame_zero_is_zero() {
        assert_eq!(timecode(0, 12.0), "0:00.0");
        assert_eq!(timecode(0, 60.0), "0:00.0");
    }

    #[test]
    fn timecode_whole_seconds() {
        assert_eq!(timecode(12, 12.0), "0:01.0");
        assert_eq!(timecode(60, 30.0), "0:02.0");
    }

    #[test]
    fn timecode_tenths_digit() {
        // 150 frames at 12 fps = 12.5 s exactly.
        assert_eq!(timecode(150, 12.0), "0:12.5");
        // 29 frames at 10 fps = 2.9 s; the division must
        // not land a hair under the whole tenth.
        assert_eq!(timecode(29, 10.0), "0:02.9");
        // 7 frames at 30 fps = 0.233 s -> 2.33 tenths,
        // rounds down to 2.
        assert_eq!(timecode(7, 30.0), "0:00.2");
    }

    #[test]
    fn timecode_rounds_half_away_from_zero_like_playback() {
        // The Phase E unification cases: under the old
        // floor maths each of these read one tenth lower.
        // Frame 7 at 12 fps = 0.583 s -> 5.83 tenths -> 6.
        assert_eq!(timecode(7, 12.0), "0:00.6");
        // Frame 8 at 30 fps = 0.267 s -> 2.67 tenths -> 3.
        assert_eq!(timecode(8, 30.0), "0:00.3");
        // 3723 frames at 60 fps = 62.05 s -> exactly
        // 620.5 tenths in f32, and the exact half rounds
        // away from zero (the playback chip semantics),
        // so this is 1:02.1, not the floored 1:02.0.
        assert_eq!(timecode(3723, 60.0), "1:02.1");
        // Rounding carries across the minute boundary:
        // 1799 frames at 30 fps = 59.97 s -> 599.7
        // tenths -> 600 -> 1:00.0, never 0:60.0.
        assert_eq!(timecode(1799, 30.0), "1:00.0");
    }

    #[test]
    fn timecode_rolls_into_minutes() {
        // A full hour of 10 fps frames keeps counting
        // minutes rather than rolling to hours.
        assert_eq!(timecode(36_000, 10.0), "60:00.0");
    }

    #[test]
    fn timecode_unusable_fps_is_zero() {
        assert_eq!(timecode(100, 0.0), "0:00.0");
        assert_eq!(timecode(100, -12.0), "0:00.0");
        assert_eq!(timecode(100, f32::NAN), "0:00.0");
    }

    #[test]
    fn fps_text_whole_and_fractional() {
        assert_eq!(fps_text(12.0), "12");
        assert_eq!(fps_text(29.97), "30.0");
        assert_eq!(fps_text(23.976), "24.0");
    }

    #[test]
    fn frame_text_is_one_based() {
        let core = EditorCore::new();
        let txt = frame_text(&core);
        assert!(txt.starts_with("Frame 1/"), "unexpected: {txt}");
        assert!(txt.ends_with(&core.frame_count().to_string()));
    }

    #[test]
    fn zoom_text_defaults_to_100_percent() {
        let core = EditorCore::new();
        assert_eq!(zoom_text(&core), "Zoom 100%");
    }

    #[test]
    fn selection_text_figure_only() {
        let core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        assert_eq!(selection_text(&core), core.project.figures[0].name);
    }

    #[test]
    fn selection_text_figure_and_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(3);
        let name = core.project.figures[0].name.clone();
        assert_eq!(selection_text(&core), format!("{name} › node 3"));
    }

    #[test]
    fn selection_text_no_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(selection_text(&core), "No selection");
    }

    #[test]
    fn play_text_reflects_state() {
        assert_eq!(play_text(true), "▶ Playing");
        assert_eq!(play_text(false), "❚❚ Paused");
    }

    #[test]
    fn project_text_has_no_dot_on_a_fresh_core() {
        // A fresh core has counted no edits, so the
        // autosave-relative dirty signal reads clean.
        let core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        assert_eq!(project_text(&core), core.project.name);
    }

    #[test]
    fn project_text_gains_the_dot_after_an_edit() {
        // add_frame snapshots, which is an undoable edit
        // and bumps the counter the marker reads.
        let mut core = EditorCore::new();
        core.add_frame();
        assert!(core.edits_since_autosave() > 0);
        assert_eq!(
            project_text(&core),
            format!("{} ●", core.project.name)
        );
    }

    #[test]
    fn undo_text_starts_at_zero_without_redo() {
        let core = EditorCore::new();
        assert_eq!(undo_text(&core), "Undo 0");
    }

    #[test]
    fn undo_text_counts_edits_and_shows_redo_after_undo() {
        let mut core = EditorCore::new();
        core.add_frame();
        assert_eq!(undo_text(&core), "Undo 1");
        // Undoing moves the snapshot to the redo stack;
        // the redo depth joins the same segment.
        core.undo();
        assert_eq!(undo_text(&core), "Undo 0 · Redo 1");
    }

    // ---- Width discipline (Phase F) ----

    #[test]
    fn fit_hint_returns_a_fitting_hint_unchanged() {
        assert_eq!(fit_hint("Saved.", 96), "Saved.");
        assert_eq!(fit_hint("short", 5), "short");
        // Exactly at the limit is a fit, not a cut.
        assert_eq!(fit_hint("12345", 5), "12345");
        // The core's initial hint (67 chars) passes the
        // production cap untouched.
        let core = EditorCore::new();
        assert_eq!(core.status.chars().count(), 67);
        assert_eq!(fit_hint(&core.status, MAX_STATUS_CHARS), core.status);
    }

    #[test]
    fn fit_hint_truncates_with_an_ellipsis_at_a_word_boundary() {
        // Budget 19 chars + ellipsis: the kept prefix is
        // "Drag a joint to pos" and its last space (char
        // 15, before "pose") sits in the budget's second
        // half, so the cut lands there — "Drag a joint
        // to…" — rather than mid-word at "pos".
        let hint = "Drag a joint to pose. Hands and feet use IK; drag the hips to move.";
        let fitted = fit_hint(hint, 20);
        assert_eq!(fitted, "Drag a joint to…");
        assert!(fitted.chars().count() <= 20);
    }

    #[test]
    fn fit_hint_hard_cuts_when_no_usable_space() {
        // No space at all: cut at the budget.
        assert_eq!(fit_hint("abcdefghij", 5), "abcd…");
        // A space in the first half of the budget would
        // waste it, so the hard cut wins.
        assert_eq!(fit_hint("a bcdefghij", 6), "a bcd…");
    }

    #[test]
    fn fit_hint_empty_and_tiny_budgets() {
        assert_eq!(fit_hint("", 10), "");
        assert_eq!(fit_hint("", 0), "");
        assert_eq!(fit_hint("anything", 0), "");
        assert_eq!(fit_hint("anything", 1), "…");
        assert_eq!(fit_hint("abcdef", 2), "a…");
    }

    #[test]
    fn fit_hint_counts_characters_not_bytes() {
        // `❚` is three bytes in UTF-8; a byte-based cut
        // would split it or misjudge the length.
        let fitted = fit_hint("❚❚ Paused ❚❚ Paused", 6);
        assert_eq!(fitted, "❚❚…");
        assert_eq!(fitted.chars().count(), 3);
        // Any result respects the cap, multibyte or not.
        for max in 0..12 {
            let out = fit_hint("●●●● ●●●● ●●●●", max);
            assert!(out.chars().count() <= max, "max {max}: {out}");
        }
    }

    #[test]
    fn est_text_width_scales_with_character_count() {
        assert_eq!(est_text_width(""), 0.0);
        assert_eq!(est_text_width("Undo 0"), 6.0 * LABEL_CHAR_W);
        // Multibyte glyphs count once each.
        assert_eq!(est_text_width("❚❚ Paused"), 9.0 * LABEL_CHAR_W);
        // The floor for a plausible longest fixed segment
        // stays well under the strip's real width, so the
        // floors only bite under genuine pressure.
        assert!(est_text_width("Undo 0 · Redo 1") < 130.0);
    }

    #[test]
    fn strip_builds_with_a_pathologically_long_status() {
        // A status far past the cap must not panic the
        // builder; the tail is capped before layout.
        let mut core = EditorCore::new();
        core.status = "x".repeat(400);
        let _ = status_strip(&core);
        assert_eq!(
            fit_hint(&core.status, MAX_STATUS_CHARS).chars().count(),
            MAX_STATUS_CHARS
        );
    }
}
