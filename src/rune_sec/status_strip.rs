//! Status strip for the Rune editor shell (Phase D, section
//! 27): a segmented, separator-divided replacement for the
//! one-line `chrome_view::status_text`. Pure builder — it
//! reads `EditorCore` and returns an element tree; it
//! attaches no handlers and mutates nothing.
//!
//! Every value shown is read live from the core:
//!
//! - project name — `core.project.name`
//!   (`model.rs:682`), with the project's total
//!   runtime appended (` · {runtime}`, Phase I — see
//!   [`runtime_label`]) and a ` ●` suffix while the
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
//! - audio state (Phase G, conditional) —
//!   `core.playback_audio_active()` (`audio_play.rs:211`),
//!   shown only while the playback player is live
//! - loop range (Phase G, conditional) —
//!   `core.loop_range` (`editor.rs:61`), shown only
//!   while a loop is set
//! - stroke / particle-event selection (Phase G,
//!   conditional) — the shell's `State::stroke_selected`
//!   / `State::particle_selected` (`main.rs`), visible
//!   only through [`status_strip_state`]: that state
//!   lives on `State`, not on the core
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
//!
//! ## Phase G conditional segments
//!
//! Phase G adds information without breaking the
//! discipline above: four more *fixed* segments, each
//! present only while its condition holds, appended
//! after the play segment and before the tail — audio
//! state ([`audio_segment`]), loop range
//! ([`loop_segment`]), stroke selection
//! ([`stroke_segment`]) and particle-event selection
//! ([`particle_segment`]), composed by
//! [`extra_segments`]. Each is built by the same
//! [`segment`] helper, so each is floored at its own
//! label's [`est_text_width`]; even the worst case
//! (all four present at their longest plausible
//! labels) adds ≈ 42 characters of floored width, and
//! the tail remains the single flexible absorber, so
//! the no-overlap argument is unchanged. The original
//! nine keep their order and their text sources.
//!
//! The audio and loop segments read core state only,
//! so both builders show them. The two selection
//! segments read shell state the core-only builder
//! cannot see: [`status_strip`] delegates with no
//! selections, and the shell calls
//! [`status_strip_state`], which passes them through.
//!
//! ## Phase H — one glyph, and one refusal
//!
//! Phase H leads the loop segment with a `repeat`
//! icon ([`segment_with_icon`]): 11 pt tall, and
//! because `repeat`'s view box is square (512 × 512)
//! exactly 11 pt wide, plus a 4 pt gap — 15 pt of
//! additive width inside a segment whose floor stays
//! the text-only [`est_text_width`]. The absorption
//! check, stated as the average advance the label may
//! have before glyph + text would outpaint the floor:
//! `(floor − 15) / chars`. For the longest pinned loop
//! label, `Loop 999–1000` (floor 13 × 8.0 = 104 pt),
//! the budget is ≈ 6.85 pt/char (0.57 em). The classes
//! actually in the label run below it — letters at the
//! documented ≈ 0.5 em, digits at ≈ 0.55 em, capitals
//! ≈ 0.67 em for the one `L` — totalling ≈ 84 pt of
//! text, ≈ 99 pt with the glyph, ≈ 5 pt to spare; even
//! at a pessimistic 0.6 em tabular-digit advance the
//! total is ≈ 103 pt, still inside, and overflow
//! would need digits averaging over ≈ 0.61 em, beyond
//! common UI fonts. Longer labels only amortise
//! better: the overhead is fixed while the floor
//! grows per character.
//!
//! The audio segment gets NO glyph, deliberately.
//! `Audio on` is only 8 chars (floor 64 pt), and the
//! honest `volume-high` glyph is wider than its 11 pt
//! height suggests — its view box is 640 wide, so it
//! paints 13.75 pt wide; glyph + gap ≈ 17.75 pt over
//! 8 chars drops the budget to ≈ 5.8 pt/char
//! (0.48 em), below even the documented *typical*
//! glyph advance of 0.5 em (see [`LABEL_CHAR_W`]) —
//! the floor cannot absorb it safely, so the segment
//! stays text-only. The stroke and particle segments
//! stay text-only too: a selection index has no
//! honest glyph.
//!
//! ## Phase I — runtime inside the project segment
//!
//! The strip never showed the project's total
//! runtime. Phase I appends it to the project
//! segment's text (`{name} · {runtime}`, see
//! [`project_text`] / [`runtime_label`]) rather than
//! adding a tenth fixed segment: no new divider, no
//! new floor competing with the tail, and the width
//! cost is only the label's own characters. It is
//! free under the width discipline because
//! [`segment`] computes its floor from the final
//! label string via [`est_text_width`] — no width
//! anywhere in this module is hard-coded per segment —
//! so the project segment's floor grows by exactly
//! the runtime label's estimated width (≈ 5 chars,
//! ≈ 40 pt) and the tail remains the single absorber.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::State;

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

/// Total-runtime label (Phase I): the project's whole
/// playback length formatted `m:ss` — minutes
/// unpadded, seconds zero-padded, so seven seconds
/// reads `0:07` and minutes keep counting past the
/// hour (`65:00`), the same convention as
/// [`timecode`].
///
/// The length itself is the engine's own total,
/// `Project::total_seconds()` (`model.rs:855`): the
/// sum of every frame's `duration` divided by the
/// project fps. A frame's `duration` is a multiplier
/// in frame periods — `adjust_frame_duration`
/// (`editor.rs:1728`) scales it and formats it as
/// `{:.2}x`, clamped to 0.25..=8.0 — so a frame with
/// duration 2.0 at 12 fps lasts 2/12 s, and
/// `total_seconds` is exactly Σ duration / fps (with
/// fps floored at 1.0 inside the model fn). The total
/// is rounded once, half away from zero, before the
/// split into minutes and seconds — the strip's
/// established rounding (see [`timecode`]) — so a
/// 2.5 s project reads `0:03`. An empty project sums
/// to zero and reads `0:00`; a non-finite total (only
/// reachable from a hand-edited project file) also
/// reads `0:00` rather than painting a nonsense
/// number.
pub fn runtime_label(core: &EditorCore) -> String {
    let secs = core.project.total_seconds();
    if !secs.is_finite() || secs <= 0.0 {
        return "0:00".to_string();
    }
    let total = secs.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Project segment: the name, the total runtime
/// appended as ` · {runtime}` (Phase I, see
/// [`runtime_label`]), plus a ` ●` suffix while the
/// project is dirty. The dirty signal is the core's
/// public edit counter, `edits_since_autosave()`
/// (`editor.rs:1587`) — edits counted since the core's
/// last autosave stamp (the autosave writes a recovery
/// snapshot every so many edits and re-stamps; the
/// project section's autosave line reads the same
/// counter). Dirty is exactly `edits_since_autosave() >
/// 0`; no other channel (status text, file mtimes) is
/// consulted.
///
/// The runtime rides inside this segment instead of
/// becoming a new one: [`segment`] floors the segment
/// at [`est_text_width`] of the whole label, so the
/// floor widens by the label's own width and the
/// strip gains no new fixed segment.
pub fn project_text(core: &EditorCore) -> String {
    let base = format!("{} · {}", core.project.name, runtime_label(core));
    if core.edits_since_autosave() > 0 {
        format!("{base} ●")
    } else {
        base
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

/// Audio segment (Phase G): `Audio on` while the
/// shell's playback player is live — the caller reads
/// `core.playback_audio_active()` (`audio_play.rs:211`),
/// the ffplay-process liveness the Phase F glue
/// exposes. `None` while inactive: silence is the
/// common case, and a permanent "Audio off" segment
/// would spend floored width on nothing.
pub fn audio_segment(active: bool) -> Option<String> {
    if active {
        Some("Audio on".to_string())
    } else {
        None
    }
}

/// Loop segment (Phase G): `Loop {a}–{b}` for the
/// core's loop range (`core.loop_range`,
/// `editor.rs:61`) — the 0-based inclusive frame
/// indices `set_loop_range` stores, displayed 1-based
/// (`a + 1`, `b + 1`), the same convention as
/// [`frame_text`] and the timeline chips, joined by
/// an en dash. `None` while no loop is set.
pub fn loop_segment(range: Option<(usize, usize)>) -> Option<String> {
    range.map(|(a, b)| format!("Loop {}–{}", a + 1, b + 1))
}

/// Stroke-selection segment (Phase G): `Stroke {i + 1}`
/// while the shell has a stroke selected
/// (`State::stroke_selected`), the index displayed
/// 1-based like every other position in the strip.
/// `None` with no selection.
pub fn stroke_segment(selected: Option<usize>) -> Option<String> {
    selected.map(|i| format!("Stroke {}", i + 1))
}

/// Particle-event-selection segment (Phase G):
/// `Particle {i + 1}` while the shell has a particle
/// event selected (`State::particle_selected`),
/// displayed 1-based. `None` with no selection.
pub fn particle_segment(selected: Option<usize>) -> Option<String> {
    selected.map(|i| format!("Particle {}", i + 1))
}

/// The Phase G conditional segment labels, in strip
/// order (audio, loop, stroke, particle), each present
/// only while its condition holds. Pure composition of
/// the four helpers above over the live core plus the
/// shell's two selection fields; the builder turns
/// each label into a floored fixed segment through the
/// same [`segment`] path as the original nine.
pub fn extra_segments(
    core: &EditorCore,
    stroke_selected: Option<usize>,
    particle_selected: Option<usize>,
) -> Vec<String> {
    [
        audio_segment(core.playback_audio_active()),
        loop_segment(core.loop_range),
        stroke_segment(stroke_selected),
        particle_segment(particle_selected),
    ]
    .into_iter()
    .flatten()
    .collect()
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

/// Hex for glyph painting beside a strip label:
/// `icon_el` takes a `0xRRGGBB` value where the label
/// itself takes the theme's `Color`. The strip always
/// builds with `tok::theme(true)` (see
/// [`status_strip_state`]), whose text colour is the
/// 2.0 palette's primary text, `#F1F3F7`.
const GLYPH_TEXT: u32 = 0xF1F3F7;

/// One fixed strip segment with a leading icon glyph
/// (Phase H): a centred row of the glyph — 11 pt tall,
/// width from the icon's own aspect — a 4 pt gap, and
/// the label text, floored at the SAME
/// [`est_text_width`] a plain [`segment`] would use.
/// The floor stays computed from the text alone; the
/// glyph is additive width inside the floored segment,
/// absorbed by the floor's conservatism. Only a
/// segment whose label is long enough to amortise the
/// glyph may use this — the loop segment, per the
/// width check in the module docs (the audio segment
/// fails that check and stays text-only).
fn segment_with_icon(label: String, icon: &str, t: &Theme) -> El {
    let floor = est_text_width(&label);
    row()
        .gap(4.0)
        .center_y()
        .min_w(floor)
        .child(crate::rune_sec::icons::icon_el_rgb(icon, 11.0, GLYPH_TEXT))
        .child(text(label).font(type_::LABEL).color(t.text))
}

/// Hairline divider between segments. Floored at its
/// own 1 pt so width pressure cannot shrink it away to
/// nothing (the tail segment absorbs the shortage).
fn divider(t: &Theme) -> El {
    el().size(1.0, 12.0).min_w(1.0).fill(t.control_track)
}

/// The status strip from the core alone: every segment
/// the core can describe, including the Phase G audio
/// and loop segments, but without the shell's stroke /
/// particle-event selection segments (that state lives
/// on `State`, not on the core). The delegating wrapper
/// over the shared composer; the shell itself calls
/// [`status_strip_state`].
pub fn status_strip(core: &EditorCore) -> El {
    build_strip(core, None, None)
}

/// The status strip: project name (with the dirty dot),
/// frame position, timecode, fps, zoom, selection, undo
/// depth, play state, the Phase G conditional segments
/// (audio, loop range, stroke selection, particle-event
/// selection) and the core's current status message,
/// divided by hairlines. The fixed segments are
/// width-floored and the status tail is the single
/// flexible, eliding segment, so the segments can never
/// overlap however narrow the strip gets (see the
/// module docs' width-discipline section).
/// Keyed `status-strip` so headless `--tree` checks can
/// find it. Theme follows the shell's current convention
/// (`main.rs` builds its view with `tok::theme(true)`).
pub fn status_strip_state(s: &State) -> El {
    build_strip(&s.core, s.stroke_selected, s.particle_selected)
}

/// Shared composer behind [`status_strip`] and
/// [`status_strip_state`]. The Phase G segments are
/// appended after the play segment and before the tail,
/// each only while its condition holds, each floored
/// through the same [`segment`] / [`est_text_width`]
/// path as the original nine; the tail stays the row's
/// single flexible member.
fn build_strip(
    core: &EditorCore,
    stroke_selected: Option<usize>,
    particle_selected: Option<usize>,
) -> El {
    let t = tok::theme(true);
    let mut segments: Vec<El> = vec![
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
    ];
    // Phase G conditional segments: after the original
    // fixed segments, before the tail, in audio / loop /
    // stroke / particle order. Each is floored at its own
    // label's estimated width exactly like the original
    // fixed segments; each appears only while its
    // condition holds, so the strip's usual width is
    // unchanged. Phase H: the loop segment is built by
    // `segment_with_icon` instead — same label, same
    // floor, plus the leading `repeat` glyph whose
    // absorption the module docs verify. The branch is
    // chosen by comparing against this module's own
    // `loop_segment` output, so it tracks any future
    // formatting change; no other extra label can
    // collide ("Audio on", "Stroke {n}", "Particle {n}").
    let loop_label = loop_segment(core.loop_range);
    for label in extra_segments(core, stroke_selected, particle_selected) {
        if Some(&label) == loop_label.as_ref() {
            segments.push(segment_with_icon(label, "repeat", &t));
        } else {
            segments.push(segment(label, &t));
        }
    }
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
    segments.push(
        text(fit_hint(&core.status, MAX_STATUS_CHARS))
            .font(type_::LABEL)
            .color(t.text_secondary)
            .grow(1.0)
            .truncate(),
    );
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
        // Phase I: the segment now carries the runtime
        // too — the demo is 5 frames of duration 1.0 at
        // 12 fps, 5/12 s, which rounds to 0:00 — so the
        // pin is the name plus ` · 0:00`, no dot.
        let core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        assert_eq!(
            project_text(&core),
            format!("{} · 0:00", core.project.name)
        );
    }

    #[test]
    fn project_text_gains_the_dot_after_an_edit() {
        // add_frame snapshots, which is an undoable edit
        // and bumps the counter the marker reads. Phase
        // I: the runtime sits between the name and the
        // dot — 6 frames of duration 1.0 at 12 fps is
        // exactly 0.5 s, rounding half away from zero
        // to 0:01.
        let mut core = EditorCore::new();
        core.add_frame();
        assert!(core.edits_since_autosave() > 0);
        assert_eq!(
            project_text(&core),
            format!("{} · 0:01 ●", core.project.name)
        );
    }

    // ---- Phase I runtime label ----

    /// A core whose project runs at `fps` with the
    /// given per-frame durations (the demo's frames,
    /// truncated to the list's length — the runtime
    /// only reads durations and fps).
    fn core_with_runtime(fps: f32, durations: &[f32]) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.frames.truncate(durations.len());
        for (frame, d) in core.project.frames.iter_mut().zip(durations) {
            frame.duration = *d;
        }
        core
    }

    #[test]
    fn runtime_label_empty_project_is_zero() {
        let mut core = EditorCore::new();
        core.project.frames.clear();
        assert_eq!(runtime_label(&core), "0:00");
    }

    #[test]
    fn runtime_label_zero_pads_seconds_under_a_minute() {
        // 5 × 14.0 frame periods at 10 fps = 7.0 s.
        let core = core_with_runtime(10.0, &[14.0, 14.0, 14.0, 14.0, 14.0]);
        assert_eq!(runtime_label(&core), "0:07");
    }

    #[test]
    fn runtime_label_counts_duration_as_frame_periods() {
        // The model's own example (model.rs,
        // durations_expand_and_total_time): durations
        // 2 + 3 + 1 + 1 + 1 at 12 fps total 8/12 s,
        // which rounds to one second.
        let core = core_with_runtime(12.0, &[2.0, 3.0, 1.0, 1.0, 1.0]);
        assert_eq!(runtime_label(&core), "0:01");
        // A single frame held for 24 periods at 12 fps
        // lasts exactly two seconds: duration is a
        // multiplier on the frame period, not a count
        // of frames.
        let core = core_with_runtime(12.0, &[24.0]);
        assert_eq!(runtime_label(&core), "0:02");
    }

    #[test]
    fn runtime_label_fractional_durations_round_half_away_from_zero() {
        // 25 periods at 10 fps = exactly 2.5 s, and the
        // exact half rounds away from zero (the strip's
        // timecode rounding), not down.
        let core = core_with_runtime(10.0, &[25.0]);
        assert_eq!(runtime_label(&core), "0:03");
        // Two quarter-speed halves: 2.5 + 2.5 periods
        // at 10 fps = exactly 0.5 s -> 0:01.
        let core = core_with_runtime(10.0, &[2.5, 2.5]);
        assert_eq!(runtime_label(&core), "0:01");
        // Just under the half rounds down: 24 periods
        // at 10 fps = 2.4 s.
        let core = core_with_runtime(10.0, &[24.0]);
        assert_eq!(runtime_label(&core), "0:02");
    }

    #[test]
    fn runtime_label_minutes_unpadded_seconds_padded() {
        // 7540 periods at 10 fps = 754 s = 12 min 34 s.
        let core = core_with_runtime(10.0, &[7540.0]);
        assert_eq!(runtime_label(&core), "12:34");
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

    // ---- Phase G conditional segments ----

    #[test]
    fn audio_segment_only_while_active() {
        assert_eq!(audio_segment(true), Some("Audio on".to_string()));
        assert_eq!(audio_segment(false), None);
    }

    #[test]
    fn loop_segment_is_one_based_with_an_en_dash() {
        assert_eq!(loop_segment(None), None);
        assert_eq!(loop_segment(Some((0, 4))), Some("Loop 1–5".to_string()));
        // A single-frame loop names the same frame twice.
        assert_eq!(loop_segment(Some((2, 2))), Some("Loop 3–3".to_string()));
    }

    #[test]
    fn stroke_segment_is_one_based() {
        assert_eq!(stroke_segment(None), None);
        assert_eq!(stroke_segment(Some(0)), Some("Stroke 1".to_string()));
        assert_eq!(stroke_segment(Some(41)), Some("Stroke 42".to_string()));
    }

    #[test]
    fn particle_segment_is_one_based() {
        assert_eq!(particle_segment(None), None);
        assert_eq!(particle_segment(Some(0)), Some("Particle 1".to_string()));
        assert_eq!(
            particle_segment(Some(998)),
            Some("Particle 999".to_string())
        );
    }

    #[test]
    fn extra_segments_empty_on_a_fresh_core() {
        // No audio playing, no loop set, no selections:
        // the Phase G block contributes nothing and the
        // strip is exactly the Phase F nine plus tail.
        let core = EditorCore::new();
        assert!(!core.playback_audio_active());
        assert_eq!(core.loop_range, None);
        assert!(extra_segments(&core, None, None).is_empty());
    }

    #[test]
    fn extra_segments_come_in_strip_order() {
        // Audio is inactive on a fresh core (it cannot
        // be started without a player), so the order is
        // proven over the three remaining conditions:
        // loop, then stroke, then particle.
        let mut core = EditorCore::new();
        core.loop_range = Some((0, 4));
        assert_eq!(
            extra_segments(&core, Some(2), Some(0)),
            vec![
                "Loop 1–5".to_string(),
                "Stroke 3".to_string(),
                "Particle 1".to_string(),
            ]
        );
    }

    #[test]
    fn extra_segment_floors_use_the_same_est_text_width_path() {
        // The builder floors every extra through
        // `segment()`, i.e. at `est_text_width` of its
        // own label. The longest plausible particle
        // label must therefore floor at exactly
        // `est_text_width` of that label — the same
        // path, so an extra can never be compressed
        // below its painted width either.
        let label = particle_segment(Some(998)).unwrap();
        assert_eq!(label, "Particle 999");
        assert_eq!(est_text_width(&label), est_text_width("Particle 999"));
        assert_eq!(est_text_width(&label), 12.0 * LABEL_CHAR_W);
        // The widest loop label shape floors the same
        // way: 13 characters including the en dash.
        let label = loop_segment(Some((998, 999))).unwrap();
        assert_eq!(label, "Loop 999–1000");
        assert_eq!(est_text_width(&label), 13.0 * LABEL_CHAR_W);
    }

    #[test]
    fn strip_builds_with_core_extras_present() {
        // The core-only wrapper shows the core-readable
        // extras (here a loop range) without panicking.
        let mut core = EditorCore::new();
        core.loop_range = Some((1, 3));
        let _ = status_strip(&core);
        assert_eq!(
            extra_segments(&core, None, None),
            vec!["Loop 2–4".to_string()]
        );
    }

    #[test]
    fn strip_builds_with_the_longest_loop_label() {
        // Phase H: the loop segment's icon branch must
        // build at the longest pinned label too — the
        // label whose floor absorption the module docs
        // verify — and the glyph leaves the label string
        // itself unchanged.
        let mut core = EditorCore::new();
        core.loop_range = Some((998, 999));
        let _ = status_strip(&core);
        assert_eq!(
            extra_segments(&core, None, None),
            vec!["Loop 999–1000".to_string()]
        );
        // The absorption budget the module docs state:
        // the text-only floor (104 pt) still covers the
        // glyph (11 pt) + its gap (4 pt) + the label's
        // painted width at the documented advances
        // (≈ 84 pt for this label's glyph classes).
        assert!(est_text_width("Loop 999–1000") > 11.0 + 4.0 + 84.0);
    }
}
