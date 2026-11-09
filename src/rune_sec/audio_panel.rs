//! Audio panel for the Rune editor shell (Phase D,
//! section 15 of the GPUI→Rune migration). Declared from
//! `main.rs` only — bin-crate code, not part of the engine
//! library.
//!
//! The panel is the Rune form of the GPUI app's `audio_panel`:
//! a summary line, one row per clip (name, start frame,
//! per-clip gain, per-clip ops incl. Delete), an operations
//! toolbar, and an add-clip row with a file-name entry.
//!
//! Phase G (this section) gives the panel a per-clip
//! SELECTION: pressing a clip row selects it
//! (`AudioPanelState::selected`, the `fx_clips` row pattern),
//! and the operations toolbar — Fade, Reverse, the gain
//! stepper, Earlier/Later, Normalise — aims at the selected
//! clip through the indexed `audio_ops` variants
//! (`audio_fade_at`, `audio_reverse_at`,
//! `audio_clip_gain_step_at`, `audio_move_at`). With no live
//! selection the toolbar falls back to the LAST clip, the
//! pre-Phase-G behaviour, and the line under "Operations"
//! says which clip is targeted. (The row Delete and the
//! other per-row ops were always index-based.)
//!
//! Phase H completes the retarget: the remaining toolbar
//! effects — Echo, Lowpass, Trim silence, Split silences,
//! Pitch, Compress — now aim at the effective target too,
//! through §37's indexed `audio_ops` variants
//! (`audio_echo_at`, `audio_lowpass_at`,
//! `audio_trim_silence_at`, `audio_split_silences_at`,
//! `audio_pitch_at`, `audio_compress_at`), falling back to
//! the `editor.rs` `_last` ops when nothing is selected.
//! No panel op is last-clip-only any more; Mixdown stays
//! whole-project by nature. Phase H also puts Font Awesome
//! glyphs on the controls whose meaning a glyph states
//! honestly — Add clip (`music`), the per-row Split@cur
//! (`scissors`) and Delete (`trash-can`, icon-only), and
//! Fade (`chart-line`: the fade is an amplitude ramp, in
//! and out — a rising line, not a loudness mark) — via
//! `icon_btn` / `icon_only_btn`; every key is unchanged.
//!
//! Phase I adds a selected-clip READOUT (`clip_readout`):
//! a muted line under the clip rows naming the effective
//! target's file name, start frame and gain at a glance,
//! so the toolbar's aim is visible beside the rows
//! themselves and not only in the line under "Operations".
//!
//! Phase J adds a project-wide NORMALISE ALL row under
//! the operations toolbar (`EditorCore::
//! normalise_all_clips`, `audio_ops`): one press sets
//! every clip's gain so each clip's own measured peak
//! lands on 0.9 — one snapshot for the whole pass,
//! silent or unreadable clips skipped. It is
//! deliberately NOT a `*_target` control: the row
//! selection does not aim it, so it sits in its own row
//! labelled "Whole project", and the row exists only
//! while the project has clips at all.
//!
//! Phase L extends the readout: when the engine's
//! duration probe (`audio_ops::clip_duration_secs`)
//! resolves for the target clip, the line gains a
//! ` · {d:.1}s` segment; when it does not, the line is
//! byte-identical to the Phase I line.
//!
//! Phase M adds a per-clip MUTE toggle to the operations
//! toolbar (`audio-mute`): it flips the effective
//! target's `AudioClip.muted` through
//! `audio_ops::audio_toggle_muted_at`, and its label and
//! glyph state the action a press performs — "Mute"
//! (`volume-xmark`) while the target is audible,
//! "Unmute" (`volume-high`) while it is muted. The mute
//! state is visible wherever a clip is named: the rows
//! carry a ` (muted)` marker (`clip_row_label`) and the
//! readout gains a ` · muted` segment (`clip_readout`).
//!
//! Phase N restyles the panel onto the design kit
//! (charter "card + hairline"): the root is
//! `design::panel_col`, the header is a `design::
//! panel_title` cluster (`music` glyph, per P13 §3b)
//! over a kit divider, the clip rows are
//! `design::list_row` shells (the kit selected fill
//! replaces the hand-rolled control-track fill), the
//! section captions are `design::section_label`, the
//! empty state is `design::empty_state`, and the gain
//! readout is `design::stepper_value`. The DSP buttons
//! gain the Font Awesome glyphs P13 assigns them —
//! Tier A: Normalise / per-row Norm (`volume-high`),
//! Echo (`repeat`), Trim silence (`scissors`), Reverse
//! (`left-right`), Earlier / Later (`chevron-left` /
//! `chevron-right`), Mixdown (`layer-group`); Tier B:
//! Lowpass (`filter`), Split silences (`wave-square`),
//! Pitch +2 (`arrow-trend-up`), Compress (`compress`),
//! per-row Xfade (`shuffle`). Fade keeps its Phase H
//! `chart-line`, Mute its volume pair, Duplicate its
//! `copy`. Every key, label and handler is unchanged;
//! the header count rides as the title cluster's meta
//! (`{n} clip(s)`), the charter §2 shape.
//!
//! Every control calls a real `EditorCore` op (evidence in
//! the handler comments). Phase E closed the gaps Phase D
//! had to leave open — fade, reverse, gain, delete and move
//! now exist as core ops in `audio_ops.rs`
//! (`audio_fade_last`, `audio_reverse_last`,
//! `audio_set_gain_last` / `audio_clip_gain_step`,
//! `delete_audio_clip`, `audio_move_last`), materialising
//! the `audio_edit` DSP (`fade`, `audio_edit.rs:176`;
//! `gain`, `:213`; `reverse`, `:225`) the way the older
//! `_last` effect ops do. Per-clip gain is therefore no
//! longer display-only: the toolbar stepper steps the last
//! clip's `AudioClip.gain` (`model.rs:505`), the field
//! playback and mixdown honour.
//!
//! UI mission Wave 4 (audit S14) reworks the panel in
//! four ways. (1) The operations toolbar is no longer one
//! 17-control horizontal scroll strip mixing scopes
//! (D1): the clip-scope controls sit in three labelled,
//! wrapping groups — "Effects", "Clip", "Position &
//! gain" — under the existing "Operations" header, and
//! Mixdown moves into the "Whole project" row beside
//! Normalise all, the Phase J rule for project-scope
//! ops. Every key, label and handler is verbatim; the
//! two new row keys (`audio-ops-clip`,
//! `audio-ops-position`) are additive containers. (2)
//! Clip health is visible (D2): each clip's file is
//! probed (`probe_clip`) and a clip whose file is
//! missing, not a WAV, or unreadable carries a warning
//! glyph and a suffix on its row, plus a segment on the
//! readout — a healthy silent clip carries none of it,
//! so "silent", "missing" and "unsupported" are no
//! longer indistinguishable. (3) The probe is cached
//! (D4): decoded duration/peak live in
//! `AudioPanelState::probe_cache`, keyed by path and
//! stamped with the file's length + mtime (the
//! `waveform_view` peaks-cache pattern), so a render
//! stats each file but decodes only on change — during
//! playback the panel no longer re-decodes every WAV
//! every frame. No `read_wav` call remains in this
//! file's view path. (4) The effects' fixed parameters
//! are stated before the fact (D3): a caption under the
//! Effects group (`effects_settings_line`) names the
//! values the one-click ops apply. Real parameter
//! steppers remain engine-blocked — the indexed ops in
//! `audio_ops` take no parameter arguments (the values
//! are constants there: `FADE_IN_SECS`/`FADE_OUT_SECS`,
//! echo 0.18/0.35, lowpass 1200 Hz, pitch +2, compress
//! 0.5/4.0) — so the panel labels honestly instead of
//! inventing controls no op would honour (audit I-4 is
//! the ops-side follow-up).
//!
//! Wave 9 (deep polish) fixes three label/layout
//! defects the earlier waves left, all proven from the
//! kit's own metrics (content budget 316 pt = the
//! 340 pt right-column card minus `panel_col`'s
//! 2 × 12 pt padding; LABEL advances measured off
//! headless tree dumps at ≈6 pt/char):
//! (1) Scope read from labels alone was weaker than
//! the Wave 4 split intended: the three clip-scope
//! groups sat under a bare "Operations" header whose
//! only scope statement was the target line, the
//! middle group was named just "Clip" (true of every
//! group), and "Whole project" rode INSIDE its button
//! row instead of heading it like its siblings. The
//! umbrella now reads "Operations — target clip", the
//! middle group is "Mute & duplicate" (its contents,
//! in the "Position & gain" compound style), and
//! "Whole project" heads its row from above. Label
//! strings only — no key, glyph or handler changed,
//! and the effects caption still only NAMES the fixed
//! settings (no steppers invented).
//! (2) Every free-text line in the panel (summary,
//! target line, readout, effects caption) was a plain
//! unwrapped `text()`: Rune text does not wrap unless
//! `.wrap()` is set, and an unwrapped line wider than
//! its card spills past the edge (the Wave 2
//! `empty_state` root-fix class). The effects caption
//! is 144 chars ≈ 880 pt — it spilled on EVERY render.
//! All four lines now take the kit prose treatment
//! (`.wrap().w_full()`, the settings `note_line`
//! pattern); a line that fits renders exactly as
//! before.
//! (3) A clip row's fixed load — row padding, gaps,
//! Norm / Split@cur / Xfade / Delete at kit button
//! metrics — totals ≈309 pt for a healthy non-last
//! clip (≈329 pt with the warning glyph, PAST the
//! card), leaving the growing name label ≈7 pt: its
//! unwrapped glyphs spilled under the buttons for
//! any realistic file name. The label now
//! `.truncate()`s (the row-label pattern of
//! `keyframe_tools` / `timeline_marks`) and the row
//! `.flex_wrap()`s (the Wave 4 toolbar-row pattern),
//! so a crowded row wraps its buttons under the name
//! instead of crushing it or pushing Delete past the
//! card edge. Row key and per-row handlers unchanged.
//!
//! Wave 10 (round-2 audit) closes the one feedback gap
//! the earlier phases left: the toolbar renders in the
//! empty state too (by design — the build test below
//! says its buttons "report 'no clips' via status when
//! pressed"), and every `_last`-dispatched button does
//! exactly that through its op ("Audio fade failed:
//! there are no audio clips."). But the three handlers
//! with no `_last` variant — `normalize_target`,
//! `duplicate_target`, `mute_target` — matched on
//! `effective_target` and did NOTHING on an empty
//! project: no status, no state change, a press with
//! zero feedback. They now set the same family line
//! ("Audio normalise failed: there are no audio
//! clips." / "Audio duplicate failed: …" / "Audio mute
//! failed: …") — the `_last` ops' own sentence with
//! each op's verb, spoken by the handler because the
//! indexed ops have no empty-project form to speak
//! through. Everything else the round-2 audit checked
//! — probe cost per frame (one stat per clip, decode
//! only on change), scope labels, the kit stepper
//! readout, kit empty states, row widths against the
//! 316 pt budget — was verified clean at source and is
//! unchanged.
//!
//! Wave 12 (E4) adds the frame readout the Wave 11
//! §6.6 remainder asked for. Source finding first: this
//! panel has NO seconds trim control — its only trim
//! is the one-click Trim silence (a sample-threshold
//! op, no user seconds value). The seconds trim
//! (`EditorCore::audio_trim`, start/end in seconds)
//! is engine + MCP only — `audio_ops.rs` says so
//! verbatim ("an engine op with no panel button") —
//! so there is no trim field here to annotate, and
//! inventing one would be a new control, out of E4's
//! display-only scope. What the panel DOES show in
//! seconds is each clip's probed duration (the rows'
//! suffix and the readout's segment) — the value a
//! user times a trim against. Both now carry the
//! duration's span in timeline frames beside the
//! seconds (`secs_to_timeline_frames`), computed from
//! the UNROUNDED probed duration with the engine's
//! own conversion, so the number printed is the
//! number the engine's trim/split ops will produce.
//! Display only: no key, control or handler changed;
//! the readout keeps `audio-readout`, the rows keep
//! `audio-clip-{i}`, and both lines already carry the
//! Wave 9 wrap/truncate treatment, so the longer text
//! cannot spill the card.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{row, text, text_input, El, TextEdit, Tx};
use stykanimate_rs::audio_edit;
use stykanimate_rs::audio_ops;
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::AudioClip;

use crate::chrome_view::btn;
use crate::rune_sec::app_theme;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
use crate::State;

/// Panel-owned state: the add-clip file-name entry (Rune
/// keeps text-field state in app state — audit Q5).
///
/// INTEGRATOR: `crate::State` needs
/// `pub audio_panel: AudioPanelState`, initialised with
/// `AudioPanelState::new()`.
pub struct AudioPanelState {
    /// File name typed in the add-clip row. Empty means
    /// "let `EditorCore::add_audio_clip` pick the first
    /// audio file in `./audio/`", exactly its own behaviour.
    pub name: TextEdit,
    /// Phase G: the clip row the user last pressed, if any
    /// — the operations toolbar's target. `None` (or a
    /// stale index after the project shrank) means the
    /// toolbar falls back to the last clip; see
    /// `effective_target`.
    pub selected: Option<usize>,
    /// Wave 4 (audit D4): the decoded-probe cache —
    /// path → (file length, mtime, probe) — behind a
    /// `RefCell` because the view builds from `&State`
    /// (the `home_panel` cache-field pattern). A render
    /// stats each clip's file and serves the probe from
    /// here while the stamp matches; the decoder runs
    /// only when a file appears or changes. See
    /// `probe_clip`.
    pub probe_cache: RefCell<ProbeCache>,
}

impl AudioPanelState {
    pub fn new() -> Self {
        AudioPanelState {
            name: TextEdit::new(""),
            selected: None,
            probe_cache: RefCell::new(HashMap::new()),
        }
    }
}

impl Default for AudioPanelState {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Pure helpers (unit-tested below) ----

/// Display name for a clip path: the file name when the
/// path has one, otherwise the path itself (the GPUI
/// panel's rule).
pub fn clip_display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Whether a gain counts as non-unity (the summary/row
/// threshold the GPUI panel used).
pub fn gain_is_non_unity(gain: f32) -> bool {
    (gain - 1.0).abs() > 0.001
}

/// `" · gain 1.50"` for a non-unity gain, `""` at unity.
pub fn gain_suffix(gain: f32) -> String {
    if gain_is_non_unity(gain) {
        format!(" · gain {gain:.2}")
    } else {
        String::new()
    }
}

/// The gain the toolbar stepper acts on: the LAST clip's
/// gain, or `None` when the project has no clips.
pub fn last_gain(clips: &[AudioClip]) -> Option<f32> {
    clips.last().map(|c| c.gain)
}

/// The gain stepper's readout: `"Gain 1.10"` for the last
/// clip's current gain, `"Gain —"` when there are no clips
/// (the stepper ops then report "no clips" via their own
/// status text, like the other `_last` buttons).
pub fn gain_readout(clips: &[AudioClip]) -> String {
    match last_gain(clips) {
        Some(gain) => format!("Gain {gain:.2}"),
        None => "Gain —".to_string(),
    }
}

/// Phase G: whether `selected` still names a clip — the
/// selection is LIVE only while its index is in range. A
/// selection left over from a longer project (the project
/// shrank under it) is stale and behaves as no selection.
pub fn selection_is_live(selected: Option<usize>, clip_count: usize) -> bool {
    selected.is_some_and(|i| i < clip_count)
}

/// Phase G: the clip index the operations toolbar acts
/// on: the selected clip when the selection is live,
/// otherwise the LAST clip (the pre-Phase-G behaviour).
/// `None` only when the project has no clips at all.
pub fn effective_target(selected: Option<usize>, clip_count: usize) -> Option<usize> {
    if selection_is_live(selected, clip_count) {
        selected
    } else {
        clip_count.checked_sub(1)
    }
}

/// Phase M: whether the effectively-targeted clip is
/// muted — the live selection's clip, or the last clip
/// under the fallback (see `effective_target`). No clips
/// means nothing is muted: `false`, so the toolbar's
/// toggle shows its audible form.
pub fn target_muted(clips: &[AudioClip], selected: Option<usize>) -> bool {
    effective_target(selected, clips.len())
        .map(|i| clips[i].muted)
        .unwrap_or(false)
}

/// Phase M: the Mute toggle's (icon, label) for a clip's
/// mute state. The pair names the ACTION a press
/// performs, not the current state: an audible clip
/// offers "Mute" (the `volume-xmark` glyph — sound
/// crossed out), a muted clip offers "Unmute" (the
/// `volume-high` glyph — sound restored). Both glyphs
/// are in the icon table; `icon_btn` would degrade to a
/// plain button if one ever went missing.
pub fn mute_toggle_parts(muted: bool) -> (&'static str, &'static str) {
    if muted {
        ("volume-high", "Unmute")
    } else {
        ("volume-xmark", "Mute")
    }
}

/// `" (muted)"` for a muted clip, `""` for an audible
/// one — the row label's mute marker.
pub fn muted_suffix(muted: bool) -> &'static str {
    if muted { " (muted)" } else { "" }
}

/// The gain the toolbar stepper acts on now: the
/// effective target's gain (see `effective_target`), or
/// `None` when the project has no clips.
pub fn target_gain(clips: &[AudioClip], selected: Option<usize>) -> Option<f32> {
    effective_target(selected, clips.len()).map(|i| clips[i].gain)
}

/// The gain stepper's readout for the current target:
/// `"Gain 1.10"` for the target clip's gain, `"Gain —"`
/// when there are no clips.
pub fn target_gain_readout(clips: &[AudioClip], selected: Option<usize>) -> String {
    match target_gain(clips, selected) {
        Some(gain) => format!("Gain {gain:.2}"),
        None => "Gain —".to_string(),
    }
}

/// The line under "Operations" naming the toolbar's
/// target: the clip's name when a live selection aims
/// the toolbar, an explicit "last clip" wording when the
/// fallback is in effect, and a no-clips wording when
/// there is nothing to aim at.
pub fn target_line(clips: &[AudioClip], selected: Option<usize>) -> String {
    let count = clips.len();
    match effective_target(selected, count) {
        None => "Target: none — no audio clips".to_string(),
        Some(i) if selection_is_live(selected, count) => format!(
            "Target: {} (clip {} of {})",
            clip_display_name(&clips[i].path),
            i + 1,
            count
        ),
        Some(i) => format!(
            "Target: last clip — {} (clip {} of {})",
            clip_display_name(&clips[i].path),
            i + 1,
            count
        ),
    }
}

/// A clip path's final component, splitting on BOTH
/// separators: clips are typed by hand in the add-clip
/// row, so a Windows-style path (`audio\theme.wav`) must
/// trim the same way a POSIX one does — `Path::file_name`
/// alone treats `\` as an ordinary character on Linux.
/// An empty path (or one ending in a separator) yields
/// the empty string.
pub fn clip_file_name(path: &str) -> &str {
    path.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(path)
}

/// Phase I: the selected-clip readout — one line naming
/// the EFFECTIVE target's facts at a glance: its 1-based
/// clip number, file name, start frame (shown 1-based,
/// like the rows' `f{n}`) and gain at two decimals (always
/// shown here, unlike the rows' non-unity-only suffix —
/// the readout's job is the full picture). The target is
/// the same `effective_target` the DSP buttons use, so
/// the line always names the clip an effect would hit:
/// the live selection, or the last clip as the fallback.
/// `None` when the project has no clips.
///
/// Phase L appends the clip's duration (` · {d:.1}s`)
/// when the engine's probe resolves for the target —
/// the file reads back as a WAV. When it does not
/// resolve (missing or unreadable file) the line is
/// exactly the Phase I line: no duration segment, no
/// placeholder.
///
/// Phase M appends ` · muted` (after the duration
/// segment, when one is present) when the target clip
/// is muted; an audible target's line is unchanged.
///
/// Wave 4 (audit D2/D4): the duration now comes from
/// the panel's cached probe (`probe_clip`) instead of
/// a fresh decode per render, and a non-healthy target
/// gains a closing health segment (` · missing file` /
/// ` · unsupported format` / ` · unreadable file`) so
/// the readout states WHY a clip has no duration. A
/// `None` probe (no probe taken) renders the Phase I
/// line, as before.
///
/// Wave 12 (E4): the duration segment gains the
/// duration's timeline-frame span in parentheses —
/// ` · {d:.1}s (12 frames)` — from the core's own
/// `project.fps` at build time (see
/// `duration_frames_suffix`), so the seconds a trim
/// is timed against and the frames they land on read
/// on one line, and the span moves when the project's
/// fps changes.
pub fn clip_readout(
    core: &EditorCore,
    selected: Option<usize>,
    probe: Option<&ClipProbe>,
) -> Option<String> {
    let clips = &core.project.audio;
    let i = effective_target(selected, clips.len())?;
    let clip = &clips[i];
    let mut line = format!(
        "Clip {}: {} · starts frame {} · gain {:.2}",
        i + 1,
        clip_file_name(&clip.path),
        clip.start_frame + 1,
        clip.gain
    );
    if let Some(d) = probe.and_then(|p| p.duration_secs) {
        // Wave 12 (E4): the frame span rides inside the
        // duration segment, converted at the project's
        // fps — read from the core on every build, so a
        // fps change re-reads on the next render.
        line += &format!(" · {d:.1}s{}", duration_frames_suffix(d, core.project.fps));
    }
    if clip.muted {
        line += " · muted";
    }
    if let Some(p) = probe {
        line += &health_segment(p.health);
    }
    Some(line)
}

/// Repair a selection after clip `deleted` was removed
/// and `len_after` clips remain (the `fx_clips`
/// `selection_after_delete` rule, mirrored): before the
/// gap keeps its index, after the gap shifts down one,
/// the deleted clip itself lands on the clip that slid
/// into its slot (or the new last clip), and an empty
/// list clears the selection.
pub fn selection_after_delete(
    selected: Option<usize>,
    deleted: usize,
    len_after: usize,
) -> Option<usize> {
    let sel = selected?;
    if len_after == 0 {
        return None;
    }
    if sel < deleted {
        Some(sel)
    } else if sel > deleted {
        Some(sel - 1)
    } else {
        Some(deleted.min(len_after - 1))
    }
}

/// One clip row's label: `{name} · f{start+1}` plus the
/// gain suffix when non-unity (the GPUI row format)
/// and, since Phase M, the ` (muted)` marker when the
/// clip is muted.
pub fn clip_row_label(clip: &AudioClip) -> String {
    format!(
        "{} · f{}{}{}",
        clip_display_name(&clip.path),
        clip.start_frame + 1,
        gain_suffix(clip.gain),
        muted_suffix(clip.muted)
    )
}

/// Wave 12 (E4): a seconds value as a count of
/// TIMELINE frames at the project's fps — the engine's
/// own seconds→frames conversion, mirrored exactly.
/// The evidence chain: `Project.fps` (`model.rs:742`,
/// default 12.0) is read from the core at render time,
/// so the readout tracks fps changes with no stored
/// state; `EditorCore::audio_trim` advances a trimmed
/// clip's start frame by `(start_secs * fps).round()`
/// with `fps = project.fps.max(1.0)`
/// (`editor.rs:3440-3442`), and a clip's length in
/// frames is the same shape — the split op's
/// `dur_frames = (duration_secs * fps).round()`
/// (`editor.rs:3484`), the waveform strip's
/// `clip_len_frames` (`waveform_view.rs:482-486`).
/// The rule is ROUND (half away from zero), never
/// floor: at 12 fps a 0.05 s value is 0.6 frames,
/// which the engine rounds to frame 1 — a floor or
/// truncation readout would print 0 and disagree with
/// the trim the engine then performs. Non-positive
/// seconds read as 0 frames (the `.max(0.0)` clamp
/// the trim shift itself applies).
pub fn secs_to_timeline_frames(secs: f32, fps: f32) -> usize {
    (secs * fps.max(1.0)).round().max(0.0) as usize
}

/// `" (12 frames)"` — the frame span a duration in
/// seconds covers at the project's fps (see
/// `secs_to_timeline_frames`) — or `" (1 frame)"` for
/// a span of exactly one. The parenthetical rides
/// INSIDE the duration segment it annotates, on both
/// surfaces that print a duration (the rows' suffix
/// and the readout), so the seconds and the frames
/// they land on are never shown apart.
pub fn duration_frames_suffix(duration_secs: f32, fps: f32) -> String {
    match secs_to_timeline_frames(duration_secs, fps) {
        1 => " (1 frame)".to_string(),
        n => format!(" ({n} frames)"),
    }
}

/// `" · {d:.1} s (12 frames)"` — the clip's duration
/// in seconds plus its timeline-frame span at the
/// project's fps (Wave 12, E4: see
/// `duration_frames_suffix`) — when its probe resolved
/// one (the file read back as a WAV with frames), `""`
/// when it did not (the row then shows name + start,
/// plus the health suffix when the probe found a
/// problem). Wave 4: probe-fed, no filesystem access
/// of its own — the pre-Wave-4 form decoded the file
/// with `audio_edit::read_wav` on every render (audit
/// D4). The frame span is computed from the UNROUNDED
/// probed duration — the same input the engine's own
/// conversions take — not from the 0.1 s display value.
pub fn probe_duration_suffix(probe: &ClipProbe, fps: f32) -> String {
    match probe.duration_secs {
        Some(d) => format!(" · {d:.1} s{}", duration_frames_suffix(d, fps)),
        None => String::new(),
    }
}

/// The summary line's wording. `total_secs` is `Some` when
/// at least one clip's WAV read back; `None` degrades the
/// line to the bare clip count (the GPUI panel's rule).
pub fn format_summary(count: usize, total_secs: Option<f32>, gained: usize) -> String {
    match total_secs {
        Some(total) => format!(
            "{count} clips · total {total:.1} s · {gained} with gain ≠ 1.0"
        ),
        None => format!("{count} clips"),
    }
}

/// The summary line for a clip list: sums the probed
/// durations of the clips whose files read back as WAV
/// and counts the non-unity gains. Clips without a
/// probed duration (missing, unsupported, unreadable)
/// are skipped; when nothing probed a duration the line
/// degrades to the bare count. Wave 4: the durations
/// come from the caller's probes (`probe_clip`, cached
/// in `AudioPanelState`) — the pre-Wave-4 form decoded
/// every file with `audio_edit::read_wav` on every
/// render (audit D4). `probes` is index-aligned with
/// `clips`; a short `probes` slice simply sums what it
/// has.
pub fn summary_line(clips: &[AudioClip], probes: &[ClipProbe]) -> String {
    let mut total_secs = 0.0f32;
    let mut any_readable = false;
    for probe in probes {
        if let Some(d) = probe.duration_secs {
            total_secs += d;
            any_readable = true;
        }
    }
    let gained = clips.iter().filter(|c| gain_is_non_unity(c.gain)).count();
    format_summary(
        clips.len(),
        if any_readable { Some(total_secs) } else { None },
        gained,
    )
}

// ---- Clip probes (Wave 4: audit D2 health + D4 cache) ----

/// The probe cache in `AudioPanelState`: clip path →
/// (file length, mtime, probe). Length + mtime together
/// catch rewrites (an edit that keeps the exact byte
/// length still moves the mtime) — the `waveform_view`
/// peaks-cache key discipline.
pub type ProbeCache = HashMap<String, (u64, SystemTime, ClipProbe)>;

/// Cap on cached probes. A project carries a handful of
/// clips; when an insert finds the map full it is
/// cleared wholesale (the `waveform_view` discipline) —
/// the worst case is one re-probe per clip.
const PROBE_CACHE_CAP: usize = 32;

/// What a clip's file probe found. The three non-healthy
/// states are exactly the ones the pre-Wave-4 UI rendered
/// identically to a healthy silent clip (audit D2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipHealth {
    /// The file exists and read back as a WAV.
    Healthy,
    /// No file at the clip's path (the stat failed).
    Missing,
    /// The file exists but is not a WAV (`.mp3`, `.ogg`,
    /// …): every probe in the app is WAV-only, so the
    /// clip can play but shows no duration or waveform.
    Unsupported,
    /// The file exists and claims to be a WAV but the
    /// decoder rejected it (corrupt, or not really WAV).
    Unreadable,
}

/// One clip's probe result: its health, and — for a
/// healthy clip — the decoded duration and peak. The
/// duration follows the `audio_ops::clip_duration_secs`
/// rule (`None` for a zero sample rate or zero frames),
/// so the row/readout durations match the engine
/// probe's exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipProbe {
    pub health: ClipHealth,
    pub duration_secs: Option<f32>,
    pub peak: Option<f32>,
}

impl ClipProbe {
    /// The probe for a file that cannot even be statted:
    /// missing, nothing decoded.
    fn missing() -> Self {
        ClipProbe {
            health: ClipHealth::Missing,
            duration_secs: None,
            peak: None,
        }
    }
}

/// The short noun phrase naming a non-healthy state, or
/// `None` for a healthy clip: "missing file" /
/// "unsupported format" / "unreadable file". Shared by
/// the row suffix and the readout segment so both
/// surfaces use one vocabulary.
pub fn health_word(health: ClipHealth) -> Option<&'static str> {
    match health {
        ClipHealth::Healthy => None,
        ClipHealth::Missing => Some("missing file"),
        ClipHealth::Unsupported => Some("unsupported format"),
        ClipHealth::Unreadable => Some("unreadable file"),
    }
}

/// `" (missing file)"` for a non-healthy clip, `""` for a
/// healthy one — the row label's health marker, in the
/// parenthetical style of the ` (muted)` marker.
pub fn health_suffix(health: ClipHealth) -> String {
    match health_word(health) {
        Some(word) => format!(" ({word})"),
        None => String::new(),
    }
}

/// `" · missing file"` for a non-healthy clip, `""` for
/// a healthy one — the readout's health segment, in its
/// ` · ` segment style.
pub fn health_segment(health: ClipHealth) -> String {
    match health_word(health) {
        Some(word) => format!(" · {word}"),
        None => String::new(),
    }
}

/// A file's cache stamp: (length, mtime) from a fresh
/// stat. `None` when the file cannot be statted or its
/// mtime read (usually: it does not exist).
fn file_stamp(path: &str) -> Option<(u64, SystemTime)> {
    let md = std::fs::metadata(path).ok()?;
    Some((md.len(), md.modified().ok()?))
}

/// Whether a path names a WAV by extension
/// (case-insensitive) — the only format the app's probes
/// (`audio_edit::read_wav`) understand.
fn is_wav_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

/// Classify a clip whose file is known to exist (the
/// caller statted it): non-WAV by extension is
/// `Unsupported` without touching the decoder; a WAV
/// the decoder accepts is `Healthy` with its duration
/// and peak; one it rejects is `Unreadable`.
fn classify_clip(clip: &AudioClip) -> ClipProbe {
    if !is_wav_path(&clip.path) {
        return ClipProbe {
            health: ClipHealth::Unsupported,
            duration_secs: None,
            peak: None,
        };
    }
    match audio_edit::read_wav(&clip.path) {
        Ok(wav) => {
            let duration_secs = if wav.sample_rate == 0 || wav.frames() == 0 {
                None
            } else {
                Some(wav.frames() as f32 / wav.sample_rate as f32)
            };
            ClipProbe {
                health: ClipHealth::Healthy,
                duration_secs,
                peak: Some(audio_edit::peak(&wav)),
            }
        }
        Err(_) => ClipProbe {
            health: ClipHealth::Unreadable,
            duration_secs: None,
            peak: None,
        },
    }
}

/// Probe one clip, serving from `cache` while the file
/// is unchanged. Discipline (mirroring `waveform_view`'s
/// peaks cache): stat first — a file that cannot be
/// statted reports `Missing` and never reaches the
/// cache or the decoder, so a file that appears later
/// probes fresh; a stamp match serves the cached probe
/// without decoding; otherwise the clip is classified
/// (decoding at most once per file change) and the
/// result cached under the fresh stamp. The one race —
/// the file changing between the stat and the read —
/// self-corrects on the next call, whose stat misses
/// the stored stamp.
pub fn probe_clip(cache: &RefCell<ProbeCache>, clip: &AudioClip) -> ClipProbe {
    let Some(stamp) = file_stamp(&clip.path) else {
        return ClipProbe::missing();
    };
    if let Some((len, mtime, probe)) = cache.borrow().get(&clip.path) {
        if (*len, *mtime) == stamp {
            return probe.clone();
        }
    }
    let probe = classify_clip(clip);
    let mut cache = cache.borrow_mut();
    if cache.len() >= PROBE_CACHE_CAP && !cache.contains_key(&clip.path) {
        cache.clear();
    }
    cache.insert(clip.path.clone(), (stamp.0, stamp.1, probe.clone()));
    probe
}

/// The fixed settings the one-click effects apply,
/// stated before the fact (audit D3): fade length, echo
/// delay/feedback, lowpass cutoff, pitch shift and
/// compressor threshold/ratio are constants inside the
/// `audio_ops` ops — the panel cannot offer parameter
/// controls the ops would not honour, so it names the
/// values instead (audit I-4 parameterises the ops).
/// Every number here is the op's own: fade 0.25 s in /
/// 0.25 s out (`FADE_IN_SECS` / `FADE_OUT_SECS`), echo
/// 0.18 s / 0.35, lowpass 1200 Hz, pitch +2 semitones,
/// compress threshold 0.5 ratio 4.0.
pub fn effects_settings_line() -> &'static str {
    "Fixed settings — fade 0.25 s in/out · echo 0.18 s delay, 35% feedback · lowpass 1200 Hz · pitch +2 semitones · compress 0.5 threshold, 4:1 ratio"
}

// ---- Handlers ----

/// Add-clip: with a typed name, push a clip for that path —
/// the exact construction `EditorCore::add_audio_clip`
/// performs (`editor.rs:2604`: snapshot, then
/// `AudioClip { path, start_frame: current, gain: 1.0 }`,
/// then the same status line), and the same push the MCP
/// `add_audio_clip` tool makes (`mcp_tools.rs:924`). With
/// an empty entry, defer to the op itself, which picks the
/// first `.wav/.mp3/.ogg` in `./audio/`. The entry clears
/// either way, so a second press never re-adds a stale name.
fn add_clip(s: &mut State) {
    let name = s.audio_panel.name.text.trim().to_string();
    if name.is_empty() {
        s.core.add_audio_clip();
    } else {
        let status = format!(
            "Audio clip added at frame {}: {name}",
            s.core.current + 1
        );
        s.core.snapshot();
        s.core.project.audio.push(AudioClip {
            path: name,
            start_frame: s.core.current,
            gain: 1.0,
            muted: false,
        });
        s.core.status = status;
    }
    s.audio_panel.name.clear();
}

/// Enter in the file-name field adds the clip (the field's
/// `on_submit`).
fn submit_audio_name(s: &mut State, _tx: &mut Tx) {
    add_clip(s);
}

/// The panel selection as a live clip index: `Some`
/// only while `AudioPanelState::selected` names an
/// existing clip (see `selection_is_live`).
fn live_target(s: &State) -> Option<usize> {
    let count = s.core.project.audio.len();
    if selection_is_live(s.audio_panel.selected, count) {
        s.audio_panel.selected
    } else {
        None
    }
}

/// `EditorCore::audio_normalize` on the toolbar's target
/// clip (the op is index-based, `editor.rs:2743`): the
/// selected clip when the selection is live, the last
/// clip otherwise (Phase G; previously always the last).
/// Wave 10: with no clips there is no target and no
/// `_last` variant to report through, so the handler
/// sets the op family's no-clips line itself (see the
/// module docs) — the press is never silent.
fn normalize_target(s: &mut State) {
    match effective_target(s.audio_panel.selected, s.core.project.audio.len()) {
        Some(i) => s.core.audio_normalize(i),
        None => {
            s.core.status = "Audio normalise failed: there are no audio clips.".into();
        }
    }
}

/// Phase J: normalise EVERY clip in the project —
/// `EditorCore::normalise_all_clips` at the fixed 0.9
/// target peak (the `audio_ops` op: one snapshot when
/// anything changes, silent/unreadable/on-target clips
/// skipped, its own status either way). Project-wide by
/// nature, so — unlike the `*_target` handlers — it
/// never reads the row selection.
fn normalise_all(s: &mut State) {
    s.core.normalise_all_clips(0.9);
}

/// Phase K (integrator wiring for audio_ops \§35):
/// duplicate the toolbar's target clip — the selected
/// clip when the selection is live, the last clip
/// otherwise (the `effective_target` discipline every
/// other per-clip op here uses). The op inserts the copy
/// directly after the original at the same start frame
/// under one snapshot; on success the copy becomes the
/// selection so the user can move it with Earlier/Later.
/// On the op's refusal (`None`) it has already set the
/// status and nothing changes here. Wave 10: with no
/// clips there is no target to refuse, so the handler
/// sets the op family's no-clips line itself (see the
/// module docs) — the press is never silent.
fn duplicate_target(s: &mut State) {
    match effective_target(s.audio_panel.selected, s.core.project.audio.len()) {
        Some(i) => {
            if let Some(new_index) = s.core.audio_duplicate_clip_at(i) {
                s.audio_panel.selected = Some(new_index);
            }
        }
        None => {
            s.core.status = "Audio duplicate failed: there are no audio clips.".into();
        }
    }
}

/// Phase M: mute or unmute the toolbar's target clip —
/// the selected clip when the selection is live, the
/// last clip otherwise (the `effective_target`
/// resolution `normalize_target` and `duplicate_target`
/// use). With no clips there is no target to call the
/// indexed toggle on (there is no `_last` mute variant),
/// so — Wave 10 — the handler sets the op family's
/// no-clips line itself (see the module docs) instead of
/// the Phase M silent no-op; the button itself still
/// renders live in every state, exactly like the rest of
/// the toolbar, whose buttons never render inert.
/// DEPENDENCY (Phase M): calls
/// `audio_ops::audio_toggle_muted_at` from the parallel
/// `audio_ops` section — until that lands this file does
/// not compile, by design.
fn mute_target(s: &mut State) {
    match effective_target(s.audio_panel.selected, s.core.project.audio.len()) {
        Some(i) => {
            audio_ops::audio_toggle_muted_at(&mut s.core, i);
        }
        None => {
            s.core.status = "Audio mute failed: there are no audio clips.".into();
        }
    }
}

/// Fade the target clip: the indexed op when a clip is
/// selected, the `_last` op otherwise (which also covers
/// the no-clips status line). DEPENDENCY (Phase G): calls
/// `EditorCore::audio_fade_at` from the parallel
/// `audio_ops` section — until that lands this file does
/// not compile, by design.
fn fade_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_fade_at(i),
        None => s.core.audio_fade_last(),
    }
}

/// Reverse the target clip; same dispatch as
/// `fade_target`. Calls `EditorCore::audio_reverse_at`
/// (Phase G `audio_ops` dependency, as above).
fn reverse_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_reverse_at(i),
        None => s.core.audio_reverse_last(),
    }
}

/// Step the target clip's gain by `delta`; same dispatch
/// as `fade_target`. Calls
/// `EditorCore::audio_clip_gain_step_at` (Phase G
/// `audio_ops` dependency, as above).
fn gain_step_target(s: &mut State, delta: f32) {
    match live_target(s) {
        Some(i) => s.core.audio_clip_gain_step_at(i, delta),
        None => s.core.audio_clip_gain_step(delta),
    }
}

/// Move the target clip by `delta_frames`; same dispatch
/// as `fade_target`. Calls `EditorCore::audio_move_at`
/// (Phase G `audio_ops` dependency, as above).
fn move_target(s: &mut State, delta_frames: i32) {
    match live_target(s) {
        Some(i) => s.core.audio_move_at(i, delta_frames),
        None => s.core.audio_move_last(delta_frames),
    }
}

/// Echo the target clip; same dispatch as `fade_target`.
/// DEPENDENCY (Phase H): calls `EditorCore::audio_echo_at`
/// from the parallel `audio_ops` section (§37) — until
/// that lands this file does not compile, by design. The
/// no-selection fallback keeps the `editor.rs` `_last` op,
/// whose no-clips status is unchanged.
fn echo_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_echo_at(i),
        None => s.core.audio_echo_last(),
    }
}

/// Low-pass the target clip; same dispatch as
/// `echo_target`. Calls `EditorCore::audio_lowpass_at`
/// (Phase H `audio_ops` dependency, as above).
fn lowpass_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_lowpass_at(i),
        None => s.core.audio_lowpass_last(),
    }
}

/// Trim the target clip's leading/trailing silence; same
/// dispatch as `echo_target`. Calls
/// `EditorCore::audio_trim_silence_at` (Phase H
/// `audio_ops` dependency, as above).
fn trim_silence_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_trim_silence_at(i),
        None => s.core.audio_trim_silence_last(),
    }
}

/// Split the target clip at its silences; same dispatch
/// as `echo_target`. Calls
/// `EditorCore::audio_split_silences_at` (Phase H
/// `audio_ops` dependency, as above). The fallback keeps
/// the `editor.rs` op's own name,
/// `audio_split_at_silences_last`.
fn split_silences_target(s: &mut State) {
    match live_target(s) {
        // The indexed op reports how many clips the
        // split produced; the panel surface discards it
        // (the op writes its own status line).
        Some(i) => {
            s.core.audio_split_silences_at(i);
        }
        None => s.core.audio_split_at_silences_last(),
    }
}

/// Pitch-shift the target clip; same dispatch as
/// `echo_target`. Calls `EditorCore::audio_pitch_at`
/// (Phase H `audio_ops` dependency, as above) with the
/// index ONLY: verified against the `editor.rs` original
/// — `audio_pitch_last` takes no amount argument; the
/// +2 semitones is fixed inside the op
/// (`audio_edit::pitch_shift(&mut wav, 2.0)`), so the
/// indexed twin's signature is the `_last` signature plus
/// the leading index, with no semitones parameter.
fn pitch_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_pitch_at(i),
        None => s.core.audio_pitch_last(),
    }
}

/// Compress the target clip; same dispatch as
/// `echo_target`. Calls `EditorCore::audio_compress_at`
/// (Phase H `audio_ops` dependency, as above).
fn compress_target(s: &mut State) {
    match live_target(s) {
        Some(i) => s.core.audio_compress_at(i),
        None => s.core.audio_compress_last(),
    }
}

/// A clip row's Delete: the (already index-based)
/// `EditorCore::delete_audio_clip`, then the selection is
/// repaired so it keeps naming the same clip where it can
/// (see `selection_after_delete`).
fn delete_row_clip(s: &mut State, i: usize) {
    s.core.delete_audio_clip(i);
    let len_after = s.core.project.audio.len();
    s.audio_panel.selected = selection_after_delete(s.audio_panel.selected, i, len_after);
}

// ---- View ----

/// The audio panel: header + summary, clip rows with the
/// per-clip ops the engine defines by index (Normalise,
/// Split at the current frame, Crossfade into the next
/// clip, Delete), the operations toolbar (Phase G: aimed
/// at the selected clip row, falling back to the last
/// clip; Phase H: every effect aimed, none last-clip-only),
/// and the add-clip row. Element keys reuse the
/// GPUI control ids where they existed; the Phase E keys
/// (`audio-fade`, `audio-reverse`, `audio-gain-*`,
/// `audio-move-*`, `audio-del-{i}`) are new; Phase G adds
/// no keys — row selection rides the existing
/// `audio-clip-{i}` row keys. Phase H adds no keys either:
/// Split@cur, Fade and Add clip gain glyphs and Delete
/// goes icon-only under their existing keys. Phase I adds
/// exactly one key, `audio-readout`, for the selected-clip
/// readout line under the rows. Phase J adds the
/// `audio-normalise` button in its own `audio-project-ops`
/// row under the toolbar (non-empty branch only). Phase M
/// adds the `audio-mute` toggle to the operations toolbar,
/// beside Fade / Reverse / Duplicate. Wave 4 regroups the
/// toolbar into labelled wrapping groups under "Operations"
/// (Effects keeps the `audio-ops` row key; `audio-ops-clip`
/// and `audio-ops-position` are new additive container
/// keys), moves `mixdown` into the `audio-project-ops` row,
/// and feeds the summary, rows and readout from the cached
/// clip probes.
pub fn audio_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let clips = &core.project.audio;
    let selected = s.audio_panel.selected;

    // Phase N: the kit card root + header (the `music`
    // glyph is P13 §3b's for this panel; the clip count
    // rides as the title cluster's meta, charter §2),
    // over the kit divider. Root key unchanged.
    let mut panel = design::panel_col("audio-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("music"),
                    "Audio",
                    Some(&format!("{} clip(s)", clips.len())),
                    t,
                )),
        )
        .child(design::divider(t));

    if clips.is_empty() {
        panel = panel.child(design::empty_state(
            "music",
            "No audio clips in this project.",
            t,
        ));
    } else {
        // Wave 4 (audit D4): probe every clip once per
        // render through the panel cache — a stat per
        // clip, a decode only when a file changed — and
        // let the summary, rows and readout all read the
        // probes (`probes` is index-aligned with `clips`).
        let probes: Vec<ClipProbe> = clips
            .iter()
            .map(|clip| probe_clip(&s.audio_panel.probe_cache, clip))
            .collect();
        panel = panel.child(
            // Wave 9: kit prose treatment — the line's
            // numbers are unbounded (clip count, total
            // duration), so it wraps inside the card
            // instead of spilling past its edge.
            text(summary_line(clips, &probes))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
        panel = panel.child(design::section_label("Clips", t));
        for (i, clip) in clips.iter().enumerate() {
            // Phase G: a row press selects the clip for the
            // operations toolbar (the `fx_clips` lane
            // pattern — the row carries the fill, the inner
            // buttons stay the press targets inside their
            // own boxes). Phase N: the row shell is the
            // kit `list_row` — the selected row carries
            // the kit selected fill (accent at 16%),
            // replacing the hand-rolled control-track fill.
            let is_selected = selected == Some(i);
            let probe = &probes[i];
            let mut clip_row = design::list_row(&format!("audio-clip-{i}"), is_selected, t)
                .w_full()
                // Wave 9: the row's fixed load (Norm /
                // Split@cur / Xfade / Delete at kit
                // metrics) leaves the name label only a
                // few points on a non-last clip — the row
                // wraps its buttons under the name rather
                // than crush it or spill past the card
                // (the toolbar rows' Wave 4 pattern).
                .flex_wrap()
                .on_press(move |s: &mut State| {
                    s.audio_panel.selected = Some(i);
                })
                .child(
                    text(format!(
                        "{}{}{}",
                        clip_row_label(clip),
                        // Wave 12 (E4): fps from the core,
                        // so the row's frame span tracks
                        // the project's fps like the
                        // readout's does.
                        probe_duration_suffix(probe, core.project.fps),
                        health_suffix(probe.health)
                    ))
                    .font(type_::BODY)
                    .color(t.text)
                    .grow(1.0)
                    // Wave 9: elide inside the label's own
                    // box instead of painting unwrapped
                    // glyphs under the row's buttons (the
                    // `keyframe_tools` row-label pattern).
                    .truncate(),
                );
            // Wave 4 (audit D2): a non-healthy clip —
            // missing, unsupported or unreadable file —
            // carries the warning glyph beside its name;
            // a healthy clip (silent ones included)
            // carries nothing, so the three failure
            // states no longer look like silence.
            if probe.health != ClipHealth::Healthy {
                clip_row = clip_row.child(icon_el(
                    "triangle-exclamation",
                    12.0,
                    app_theme::warning_on(t),
                ));
            }
            clip_row = clip_row
                .child(icon_btn(
                    &format!("audio-norm-{i}"),
                    "volume-high",
                    "Norm",
                    false,
                    t,
                    move |s| s.core.audio_normalize(i),
                ))
                .child(icon_btn(
                    &format!("audio-split-{i}"),
                    "scissors",
                    "Split@cur",
                    false,
                    t,
                    move |s| {
                        let at = s.core.current;
                        s.core.audio_split(i, at)
                    },
                ));
            if i + 1 < clips.len() {
                clip_row = clip_row.child(icon_btn(
                    &format!("audio-xfade-{i}"),
                    "shuffle",
                    "Xfade",
                    false,
                    t,
                    move |s| s.core.audio_crossfade(i),
                ));
            }
            clip_row = clip_row.child(icon_only_btn(
                &format!("audio-del-{i}"),
                "trash-can",
                "Delete clip",
                26.0, false,
                t,
                move |s| delete_row_clip(s, i),
            ));
            panel = panel.child(clip_row);
        }
        // Phase I: the selected-clip readout sits directly
        // under the rows it describes (and above the
        // Operations toolbar below) — the effective
        // target's name, start frame and gain at a glance.
        // Inside the non-empty branch on purpose: with no
        // clips the panel keeps its existing empty state
        // and no readout element is rendered at all.
        let target_probe = effective_target(selected, clips.len()).map(|i| &probes[i]);
        if let Some(readout) = clip_readout(core, selected, target_probe) {
            panel = panel.child(
                // Wave 9: the readout embeds the target's
                // (unbounded) file name plus up to four
                // segments — it wraps inside the card
                // instead of spilling (kit prose pattern).
                text(readout)
                    .key("audio-readout")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );
        }
    }

    // Operations toolbar. Phase G aims Fade, Reverse,
    // the gain stepper, Earlier/Later and Normalise at
    // the SELECTED clip (the indexed `audio_ops` variants,
    // dispatched by the `*_target` handlers), falling back
    // to the last clip when nothing is selected — the
    // target line under the header says which. Phase H
    // aims the remaining effects (Echo, Lowpass, Trim
    // silence, Split silences, Pitch, Compress) the same
    // way, through §37's indexed variants; no toolbar op
    // is last-clip-only. Mixdown is whole-project.
    // Wave 9: the umbrella header states the scope the
    // target line then specifies — the three groups
    // under it (Effects / Mute & duplicate / Position &
    // gain) all act on this one clip, in contrast to
    // the Whole project row below.
    panel = panel.child(design::section_label("Operations — target clip", t));
    panel = panel.child(
        // Wave 9: the line embeds the target's
        // (unbounded) file name — kit prose treatment,
        // wraps inside the card instead of spilling.
        text(target_line(clips, selected))
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
    );
    // Phase M: the Mute toggle's face follows the
    // effective target's mute state — the press performs
    // the named action (see `mute_toggle_parts`). With no
    // clips there is no target and the button shows its
    // audible "Mute" form; it renders live regardless,
    // like every toolbar button here.
    let (mute_icon, mute_label) = mute_toggle_parts(target_muted(clips, selected));
    // Wave 4 (audit D1): the toolbar's groups. The
    // pre-Wave-4 form was ONE `.scroll_x()` row of 17
    // controls in a 316 pt column — most of it hidden
    // behind horizontal scrolling with no affordance,
    // four kinds of control undifferentiated, and
    // whole-project Mixdown seated among clip-scope
    // controls. Now the clip-scope controls sit in three
    // labelled wrapping groups and Mixdown sits in the
    // Whole project row below. Every key, label, glyph
    // and handler is verbatim; only the row containers
    // changed (`audio-ops` stays on the Effects row;
    // `audio-ops-clip` / `audio-ops-position` are new
    // additive container keys).
    panel = panel.child(design::section_label("Effects", t));
    panel = panel.child(
        row()
            .key("audio-ops")
            .gap(6.0)
            .center_y()
            .flex_wrap()
            .w_full()
            // Phase N: the DSP buttons carry the glyphs
            // P13 assigns them (Tier A + Tier B tables) —
            // keys, labels and handlers verbatim.
            .child(icon_btn("audio-norm-last", "volume-high", "Normalise", false, t, normalize_target))
            .child(icon_btn("audio-echo", "repeat", "Echo", false, t, echo_target))
            .child(icon_btn("audio-lowpass", "filter", "Lowpass", false, t, lowpass_target))
            .child(icon_btn("audio-trimsil", "scissors", "Trim silence", false, t, trim_silence_target))
            .child(icon_btn("audio-split-sil", "wave-square", "Split silences", false, t, split_silences_target))
            .child(icon_btn("audio-pitch", "arrow-trend-up", "Pitch +2", false, t, pitch_target))
            .child(icon_btn("audio-comp", "compress", "Compress", false, t, compress_target))
            // Fade is an amplitude ramp (in and out), so
            // its glyph is the rising `chart-line`, not a
            // loudness mark (Phase H).
            .child(icon_btn("audio-fade", "chart-line", "Fade", false, t, fade_target))
            .child(icon_btn("audio-reverse", "left-right", "Reverse", false, t, reverse_target)),
    );
    // Wave 4 (audit D3): the effects' fixed parameters,
    // stated before the fact — the values were previously
    // learnable only from the after-the-fact status line.
    // Wave 9: the caption is 144 chars — as a plain
    // unwrapped line it measured ≈880 pt against the
    // 316 pt card and spilled on every render; it now
    // takes the kit prose treatment and wraps. The
    // wording is unchanged: it still only NAMES the
    // fixed settings the ops apply.
    panel = panel.child(
        text(effects_settings_line())
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
    );
    // Wave 9: the group is named for its contents (the
    // "Position & gain" compound style) — bare "Clip"
    // said nothing the other two group headers don't.
    panel = panel.child(design::section_label("Mute & duplicate", t));
    panel = panel.child(
        row()
            .key("audio-ops-clip")
            .gap(6.0)
            .center_y()
            .flex_wrap()
            .w_full()
            .child(icon_btn("audio-mute", mute_icon, mute_label, false, t, mute_target))
            .child(icon_btn(
                "audio-duplicate",
                "copy",
                "Duplicate",
                false,
                t,
                duplicate_target,
            )),
    );
    panel = panel.child(design::section_label("Position & gain", t));
    panel = panel.child(
        row()
            .key("audio-ops-position")
            .gap(6.0)
            .center_y()
            .flex_wrap()
            .w_full()
            .child(btn("audio-gain-dn", "Gain -", false, t, |s| {
                gain_step_target(s, -0.1);
            }))
            // Phase N: the gain readout is the kit
            // stepper value (centred mono, min-width 34);
            // the readout string itself is unchanged.
            // The − / + buttons keep their `btn` form and
            // labels; the readout carries the name.
            .child(design::stepper_value(
                &target_gain_readout(clips, selected),
                t,
            ))
            .child(btn("audio-gain-up", "Gain +", false, t, |s| {
                gain_step_target(s, 0.1);
            }))
            .child(icon_btn("audio-move-earlier", "chevron-left", "Earlier", false, t, |s| {
                move_target(s, -1);
            }))
            .child(icon_btn("audio-move-later", "chevron-right", "Later", false, t, |s| {
                move_target(s, 1);
            })),
    );

    // Phase J: the project-wide normalise row. Every
    // control in the toolbar above is aimed by the row
    // selection (or its last-clip fallback); this one is
    // not — it normalises ALL clips at once — so it gets
    // its own row under a "Whole project" label rather
    // than a seat in the toolbar, and it is only built
    // while clips exist (with none, there is nothing to
    // measure). It renders live whenever it exists: the
    // op itself reports when nothing needed changing.
    if !clips.is_empty() {
        // Wave 9: "Whole project" heads its row from
        // above, like every other group header in the
        // panel — it rode INSIDE the row before, which
        // broke the parallel structure (and spent
        // ≈91 pt of the row's 316 pt budget on the
        // label; the two buttons alone measure ≈205 pt).
        panel = panel.child(design::section_label("Whole project", t));
        panel = panel.child(
            row()
                .key("audio-project-ops")
                .gap(6.0)
                .center_y()
                .child(icon_btn(
                    "audio-normalise",
                    "volume-high",
                    "Normalise all",
                    false,
                    t,
                    normalise_all,
                ))
                // Wave 4 (audit D1): Mixdown joins this
                // row — it is whole-project by nature (it
                // renders every clip to one file), and the
                // Phase J rule that seated Normalise all
                // here applies to it equally; it sat in
                // the clip-scope toolbar until now. Key,
                // glyph, label and handler verbatim. Like
                // Normalise all it now renders only while
                // clips exist — a mixdown with no clips
                // has nothing to render.
                .child(icon_btn("mixdown", "layer-group", "Mixdown", false, t, |s| {
                    s.core.mixdown_audio()
                })),
        );
    }

    // Add-clip row: file-name entry + Add. Enter in the
    // field submits too.
    panel = panel.child(
        row()
            .key("audio-add-row")
            .w_full()
            .center_y()
            .gap(6.0)
            .child(
                text_input(&s.audio_panel.name, type_::BODY)
                    .key("audio-name")
                    .single_line()
                    .grow(1.0)
                    .color(t.text)
                    .placeholder(
                        "Audio file name — blank adds the first file in ./audio/",
                        t.text_secondary,
                    )
                    .on_edit(|s: &mut State| &mut s.audio_panel.name)
                    .on_submit(submit_audio_name),
            )
            .child(icon_btn("add-audio", "music", "Add clip", false, t, |s| add_clip(s))),
    );

    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::editor::EditorCore;

    fn clip(path: &str, start_frame: usize, gain: f32) -> AudioClip {
        AudioClip {
            path: path.to_string(),
            start_frame,
            gain,
            muted: false,
        }
    }

    /// A clip factory variant with the mute flag set, for
    /// the Phase M tests.
    fn muted_clip(path: &str, start_frame: usize, gain: f32) -> AudioClip {
        AudioClip {
            muted: true,
            ..clip(path, start_frame, gain)
        }
    }

    /// Wave 4: probe a core's clip through a throwaway
    /// cache — the readout tests' stand-in for the panel
    /// state's own `probe_cache`.
    fn probe_at(core: &EditorCore, i: usize) -> ClipProbe {
        probe_clip(&RefCell::new(ProbeCache::new()), &core.project.audio[i])
    }

    #[test]
    fn panel_state_starts_with_an_empty_name() {
        assert_eq!(AudioPanelState::new().name.text, "");
        assert_eq!(AudioPanelState::default().name.text, "");
        // Phase G: nothing is selected until a row is
        // pressed, in both constructors.
        assert_eq!(AudioPanelState::new().selected, None);
        assert_eq!(AudioPanelState::default().selected, None);
        // Wave 4: the probe cache starts empty in both
        // constructors.
        assert!(AudioPanelState::new().probe_cache.borrow().is_empty());
        assert!(AudioPanelState::default().probe_cache.borrow().is_empty());
    }

    #[test]
    fn effective_target_prefers_a_live_selection() {
        // Valid selections name themselves.
        assert_eq!(effective_target(Some(0), 3), Some(0));
        assert_eq!(effective_target(Some(1), 3), Some(1));
        assert_eq!(effective_target(Some(2), 3), Some(2));
    }

    #[test]
    fn effective_target_falls_back_to_the_last_clip() {
        // Nothing selected: the last clip (pre-Phase-G
        // behaviour).
        assert_eq!(effective_target(None, 3), Some(2));
        assert_eq!(effective_target(None, 1), Some(0));
        // A stale selection (the project shrank under it)
        // behaves as no selection.
        assert_eq!(effective_target(Some(3), 3), Some(2));
        assert_eq!(effective_target(Some(9), 1), Some(0));
    }

    #[test]
    fn effective_target_is_none_for_an_empty_project() {
        assert_eq!(effective_target(None, 0), None);
        assert_eq!(effective_target(Some(0), 0), None);
        assert_eq!(effective_target(Some(4), 0), None);
    }

    #[test]
    fn selection_is_live_only_while_in_range() {
        assert!(!selection_is_live(None, 3));
        assert!(selection_is_live(Some(0), 3));
        assert!(selection_is_live(Some(2), 3));
        assert!(!selection_is_live(Some(3), 3));
        assert!(!selection_is_live(Some(0), 0));
    }

    #[test]
    fn target_line_names_the_pick_the_fallback_and_the_void() {
        let clips = vec![clip("a/theme.wav", 0, 1.0), clip("a/sting.wav", 9, 1.0)];
        // A live selection is named directly.
        assert_eq!(
            target_line(&clips, Some(0)),
            "Target: theme.wav (clip 1 of 2)"
        );
        assert_eq!(
            target_line(&clips, Some(1)),
            "Target: sting.wav (clip 2 of 2)"
        );
        // No selection — and a stale one — say "last clip".
        assert_eq!(
            target_line(&clips, None),
            "Target: last clip — sting.wav (clip 2 of 2)"
        );
        assert_eq!(
            target_line(&clips, Some(7)),
            "Target: last clip — sting.wav (clip 2 of 2)"
        );
        assert_eq!(target_line(&[], None), "Target: none — no audio clips");
        assert_eq!(target_line(&[], Some(0)), "Target: none — no audio clips");
    }

    #[test]
    fn clip_file_name_trims_both_separators() {
        // POSIX and Windows separators trim alike, mixed
        // paths take the final component, and a bare name
        // (or an empty path) passes through untouched.
        assert_eq!(clip_file_name("audio/theme.wav"), "theme.wav");
        assert_eq!(clip_file_name("audio\\theme.wav"), "theme.wav");
        assert_eq!(clip_file_name("a/b\\c/sting.ogg"), "sting.ogg");
        assert_eq!(clip_file_name("theme.wav"), "theme.wav");
        assert_eq!(clip_file_name(""), "");
    }

    #[test]
    fn clip_readout_formats_the_effective_target() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("audio/theme.wav", 0, 1.0));
        core.project.audio.push(clip("audio\\sting.wav", 29, 0.5));
        // A live selection names that clip: 1-based number
        // and start frame (start_frame 29 shows as frame
        // 30, the rows' f{n} convention), the file name
        // trimmed on either separator, and the gain always
        // shown at two decimals — unity included. Neither
        // file exists, so Wave 4's health segment closes
        // each line (" · missing file").
        assert_eq!(
            clip_readout(&core, Some(1), Some(&probe_at(&core, 1))),
            Some("Clip 2: sting.wav · starts frame 30 · gain 0.50 · missing file".to_string())
        );
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some("Clip 1: theme.wav · starts frame 1 · gain 1.00 · missing file".to_string())
        );
        // With NO probe taken the line is the Phase I
        // line exactly — no duration, no health segment.
        assert_eq!(
            clip_readout(&core, Some(1), None),
            Some("Clip 2: sting.wav · starts frame 30 · gain 0.50".to_string())
        );
    }

    #[test]
    fn clip_readout_falls_back_like_the_dsp_buttons() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("a.wav", 0, 2.0));
        core.project.audio.push(clip("b.wav", 3, 0.25));
        // No selection: the readout names the LAST clip,
        // the toolbar's own fallback (and its probe —
        // b.wav does not exist, so the health segment
        // rides along).
        assert_eq!(
            clip_readout(&core, None, Some(&probe_at(&core, 1))),
            Some("Clip 2: b.wav · starts frame 4 · gain 0.25 · missing file".to_string())
        );
        // A stale selection (past the end after the
        // project shrank) falls back the same way, via
        // effective_target — it never names a dead index.
        assert_eq!(
            clip_readout(&core, Some(9), Some(&probe_at(&core, 1))),
            Some("Clip 2: b.wav · starts frame 4 · gain 0.25 · missing file".to_string())
        );
    }

    #[test]
    fn clip_readout_is_none_without_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        assert_eq!(clip_readout(&core, None, None), None);
        assert_eq!(clip_readout(&core, Some(0), None), None);
    }

    #[test]
    fn clip_readout_appends_duration_for_a_readable_wav() {
        // One second of silence at 44.1 kHz, written with
        // the engine's own WAV writer (the
        // `duration_suffix_reads_readable_wavs_only`
        // fixture pattern).
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-readout-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&path.to_string_lossy(), 4, 0.5));
        // The probe resolves, so the line gains the
        // duration segment: start frame 4 shows as frame
        // 5, and 44 100 samples at 44.1 kHz is 1.0 s.
        // Wave 12 (E4): the segment carries the frame
        // span too — 1.0 s at the demo project's 12 fps
        // is 12 frames. A healthy clip gains NO health
        // segment.
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some(format!(
                "Clip 1: {} · starts frame 5 · gain 0.50 · 1.0s (12 frames)",
                path.file_name().unwrap().to_string_lossy()
            ))
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn clip_readout_marks_a_missing_file_instead_of_a_duration() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("no/such/file.wav", 0, 1.0));
        // Wave 4 (audit D2): the probe cannot resolve a
        // path that does not exist — where the Phase L
        // line simply omitted the duration, the line now
        // says WHY: the closing health segment names the
        // missing file.
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some("Clip 1: file.wav · starts frame 1 · gain 1.00 · missing file".to_string())
        );
    }

    #[test]
    fn clip_readout_marks_a_muted_target() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("audio/theme.wav", 0, 1.0));
        core.project.audio.push(muted_clip("audio\\sting.wav", 29, 0.5));
        // The muted segment closes the line when the
        // target is muted (here the live selection names
        // the muted clip), while an audible target's line
        // is the Phase I line byte-for-byte. Neither file
        // exists, so Wave 4's health segment follows the
        // muted segment on both lines.
        assert_eq!(
            clip_readout(&core, Some(1), Some(&probe_at(&core, 1))),
            Some("Clip 2: sting.wav · starts frame 30 · gain 0.50 · muted · missing file".to_string())
        );
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some("Clip 1: theme.wav · starts frame 1 · gain 1.00 · missing file".to_string())
        );
        // The fallback names the last clip, so with no
        // selection the muted marker rides along.
        assert_eq!(
            clip_readout(&core, None, Some(&probe_at(&core, 1))),
            Some("Clip 2: sting.wav · starts frame 30 · gain 0.50 · muted · missing file".to_string())
        );
    }

    #[test]
    fn clip_readout_marks_muted_after_the_duration_segment() {
        // Muted AND readable as WAV: the marker lands
        // after the Phase L duration segment, at the very
        // end of the line.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-readout-muted-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(muted_clip(&path.to_string_lossy(), 4, 0.5));
        // Healthy file: duration (with its Wave 12
        // frame span), then muted, and NO health
        // segment.
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some(format!(
                "Clip 1: {} · starts frame 5 · gain 0.50 · 1.0s (12 frames) · muted",
                path.file_name().unwrap().to_string_lossy()
            ))
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mute_toggle_parts_name_the_action_not_the_state() {
        // Audible: the press MUTES, so the button says so
        // and shows the crossed-out speaker.
        assert_eq!(mute_toggle_parts(false), ("volume-xmark", "Mute"));
        // Muted: the press restores the sound.
        assert_eq!(mute_toggle_parts(true), ("volume-high", "Unmute"));
    }

    #[test]
    fn target_muted_reads_the_effective_target() {
        let clips = vec![clip("a.wav", 0, 1.0), muted_clip("b.wav", 3, 1.0)];
        // A live selection names its own clip's state.
        assert!(!target_muted(&clips, Some(0)));
        assert!(target_muted(&clips, Some(1)));
        // No selection — or a stale one — reads the LAST
        // clip, the toolbar's own fallback.
        assert!(target_muted(&clips, None));
        assert!(target_muted(&clips, Some(9)));
        // No clips: nothing is muted, the toggle shows
        // its audible form.
        assert!(!target_muted(&[], None));
        assert!(!target_muted(&[], Some(0)));
    }

    #[test]
    fn muted_suffix_marks_only_muted_clips() {
        assert_eq!(muted_suffix(true), " (muted)");
        assert_eq!(muted_suffix(false), "");
    }

    #[test]
    fn target_gain_readout_follows_the_target() {
        let clips = vec![clip("a.wav", 0, 2.0), clip("b.wav", 3, 0.5)];
        assert_eq!(target_gain(&clips, Some(0)), Some(2.0));
        assert_eq!(target_gain_readout(&clips, Some(0)), "Gain 2.00");
        // Fallback (none / stale) reads the LAST clip,
        // exactly the old `gain_readout` values.
        assert_eq!(target_gain_readout(&clips, None), "Gain 0.50");
        assert_eq!(target_gain_readout(&clips, Some(9)), "Gain 0.50");
        assert_eq!(target_gain_readout(&[], None), "Gain —");
        assert_eq!(target_gain(&[], Some(0)), None);
    }

    #[test]
    fn selection_after_delete_repairs_each_position() {
        // Nothing selected stays nothing.
        assert_eq!(selection_after_delete(None, 0, 3), None);
        // Empty list clears the selection.
        assert_eq!(selection_after_delete(Some(0), 0, 0), None);
        // Before the gap: untouched.
        assert_eq!(selection_after_delete(Some(0), 2, 4), Some(0));
        // After the gap: shifts down one.
        assert_eq!(selection_after_delete(Some(3), 1, 4), Some(2));
        // The deleted clip itself: the clip that slid in,
        // or the new last clip when the tail was deleted.
        assert_eq!(selection_after_delete(Some(1), 1, 4), Some(1));
        assert_eq!(selection_after_delete(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn duration_suffix_reads_the_probe() {
        // Wave 4: the suffix is probe-fed. A missing file
        // probes no duration — the row shows name + start
        // (plus the health suffix, tested separately).
        let cache = RefCell::new(ProbeCache::new());
        let missing = probe_clip(&cache, &clip("no/such/file.wav", 0, 1.0));
        assert_eq!(probe_duration_suffix(&missing, 12.0), "");
        // One second of silence at 44.1 kHz, written with
        // the engine's own WAV writer, probes 1.0 s.
        // Wave 12 (E4): at 12 fps the suffix carries the
        // frame span beside the seconds.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-dur-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let c = clip(&path.to_string_lossy(), 4, 1.0);
        let healthy = probe_clip(&cache, &c);
        assert_eq!(probe_duration_suffix(&healthy, 12.0), " · 1.0 s (12 frames)");
        assert_eq!(probe_duration_suffix(&healthy, 24.0), " · 1.0 s (24 frames)");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn secs_to_timeline_frames_uses_the_engines_round_rule() {
        // Wave 12 (E4): the conversion is the engine
        // trim's own — `(secs * fps.max(1.0)).round()`
        // (`editor.rs:3440-3442`) — NOT a floor. The
        // discriminating cases are the sub-frame ones:
        // 0.05 s at 12 fps is 0.6 frames, which the
        // engine rounds to frame 1; a floor/truncation
        // readout would print 0 and disagree with the
        // trim the engine then performs.
        assert_eq!(secs_to_timeline_frames(1.0, 12.0), 12);
        assert_eq!(secs_to_timeline_frames(0.1, 12.0), 1);
        assert_eq!(secs_to_timeline_frames(0.05, 12.0), 1);
        // The half-frame mark rounds away from zero,
        // like `f32::round`: 0.125 s at 12 fps is
        // exactly 1.5 frames → 2.
        assert_eq!(secs_to_timeline_frames(0.125, 12.0), 2);
        // The span tracks the fps it is given.
        assert_eq!(secs_to_timeline_frames(1.0, 24.0), 24);
        assert_eq!(secs_to_timeline_frames(2.5, 24.0), 60);
        // The trim's own clamps: fps is floored at 1.0
        // (`max(1.0)`), and a non-positive seconds value
        // reads as 0 frames (`.max(0.0)`).
        assert_eq!(secs_to_timeline_frames(2.0, 0.0), 2);
        assert_eq!(secs_to_timeline_frames(0.0, 12.0), 0);
        assert_eq!(secs_to_timeline_frames(-0.5, 12.0), 0);
    }

    #[test]
    fn duration_frames_suffix_singularises_one_frame() {
        assert_eq!(duration_frames_suffix(1.0, 12.0), " (12 frames)");
        assert_eq!(duration_frames_suffix(0.05, 12.0), " (1 frame)");
        assert_eq!(duration_frames_suffix(0.0, 12.0), " (0 frames)");
    }

    #[test]
    fn clip_readout_frame_span_tracks_the_project_fps() {
        // Wave 12 (E4): the readout converts at the
        // core's fps on every build — the same 1.0 s
        // clip reads 12 frames in a 12 fps project and
        // 24 in a 24 fps one, with no stored state.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-readout-fps-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&path.to_string_lossy(), 0, 1.0));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some(format!(
                "Clip 1: {name} · starts frame 1 · gain 1.00 · 1.0s (12 frames)"
            ))
        );
        core.project.fps = 24.0;
        assert_eq!(
            clip_readout(&core, Some(0), Some(&probe_at(&core, 0))),
            Some(format!(
                "Clip 1: {name} · starts frame 1 · gain 1.00 · 1.0s (24 frames)"
            ))
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn health_words_name_each_failure_state() {
        // One vocabulary for both surfaces: the row's
        // parenthetical suffix and the readout's segment.
        assert_eq!(health_word(ClipHealth::Healthy), None);
        assert_eq!(health_suffix(ClipHealth::Healthy), "");
        assert_eq!(health_segment(ClipHealth::Healthy), "");
        assert_eq!(health_suffix(ClipHealth::Missing), " (missing file)");
        assert_eq!(health_segment(ClipHealth::Missing), " · missing file");
        assert_eq!(
            health_suffix(ClipHealth::Unsupported),
            " (unsupported format)"
        );
        assert_eq!(
            health_segment(ClipHealth::Unsupported),
            " · unsupported format"
        );
        assert_eq!(
            health_suffix(ClipHealth::Unreadable),
            " (unreadable file)"
        );
        assert_eq!(
            health_segment(ClipHealth::Unreadable),
            " · unreadable file"
        );
    }

    #[test]
    fn probe_classifies_each_health_state() {
        let cache = RefCell::new(ProbeCache::new());
        let pid = std::process::id();
        let dir = std::env::temp_dir();

        // Missing: nothing at the path — no duration, no
        // peak, and the health word is what the row and
        // readout will show.
        let missing = probe_clip(&cache, &clip("no/such/file.wav", 0, 1.0));
        assert_eq!(missing.health, ClipHealth::Missing);
        assert_eq!(missing.duration_secs, None);
        assert_eq!(missing.peak, None);

        // Unsupported: the file EXISTS but is not a WAV
        // (the add flow accepts .mp3/.ogg; the probes are
        // WAV-only), so it classifies by extension without
        // decoding.
        let mp3 = dir.join(format!("stykanimate-rune-audio-panel-probe-{pid}.mp3"));
        std::fs::write(&mp3, b"not really an mp3").unwrap();
        let unsupported = probe_clip(&cache, &clip(&mp3.to_string_lossy(), 0, 1.0));
        assert_eq!(unsupported.health, ClipHealth::Unsupported);
        assert_eq!(unsupported.duration_secs, None);
        let _ = std::fs::remove_file(&mp3);

        // Unreadable: a .wav path whose bytes the decoder
        // rejects.
        let bad = dir.join(format!("stykanimate-rune-audio-panel-probe-{pid}.wav"));
        std::fs::write(&bad, b"this is not a wav file at all").unwrap();
        let unreadable = probe_clip(&cache, &clip(&bad.to_string_lossy(), 0, 1.0));
        assert_eq!(unreadable.health, ClipHealth::Unreadable);
        assert_eq!(unreadable.duration_secs, None);
        let _ = std::fs::remove_file(&bad);

        // Healthy: a real WAV decodes — duration and peak
        // ride along. Half-amplitude samples peak at 0.5
        // (within the PCM16 round trip's tolerance).
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 44_100],
        };
        let good = dir.join(format!("stykanimate-rune-audio-panel-probe-good-{pid}.wav"));
        std::fs::write(&good, audio_edit::write_wav(&wav)).unwrap();
        let healthy = probe_clip(&cache, &clip(&good.to_string_lossy(), 0, 1.0));
        assert_eq!(healthy.health, ClipHealth::Healthy);
        assert_eq!(healthy.duration_secs, Some(1.0));
        let peak = healthy.peak.expect("a healthy probe carries the peak");
        assert!((peak - 0.5).abs() < 0.001, "peak {peak}");
        // A healthy SILENT clip is Healthy with peak 0 —
        // distinguishable from Missing, which is the
        // audit D2 point.
        let silence = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let silent_path =
            dir.join(format!("stykanimate-rune-audio-panel-probe-silent-{pid}.wav"));
        std::fs::write(&silent_path, audio_edit::write_wav(&silence)).unwrap();
        let silent = probe_clip(&cache, &clip(&silent_path.to_string_lossy(), 0, 1.0));
        assert_eq!(silent.health, ClipHealth::Healthy);
        assert_eq!(silent.peak, Some(0.0));
        assert_eq!(health_suffix(silent.health), "");
        let _ = std::fs::remove_file(&good);
        let _ = std::fs::remove_file(&silent_path);
    }

    #[test]
    fn probe_cache_tracks_the_file_not_the_render() {
        // The audit D4 contract: an unchanged file is
        // served from the cache, a changed file is
        // re-probed, and a missing file is never cached
        // (it probes fresh when it appears).
        let cache = RefCell::new(ProbeCache::new());
        let pid = std::process::id();
        let path = std::env::temp_dir()
            .join(format!("stykanimate-rune-audio-panel-cache-{pid}.wav"));
        let path_str = path.to_string_lossy().into_owned();
        let c = clip(&path_str, 0, 1.0);

        // Absent: Missing, and the cache stays empty —
        // the miss is not memoised.
        assert_eq!(probe_clip(&cache, &c).health, ClipHealth::Missing);
        assert!(cache.borrow().is_empty(), "a miss is never cached");

        // The file appears: the very next probe decodes
        // it (1 s), and a repeat probe returns the cached
        // value unchanged.
        let one_sec = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        std::fs::write(&path, audio_edit::write_wav(&one_sec)).unwrap();
        let first = probe_clip(&cache, &c);
        assert_eq!(first.duration_secs, Some(1.0));
        assert_eq!(probe_clip(&cache, &c), first, "unchanged file, cached probe");

        // The file is rewritten longer: the (len, mtime)
        // stamp moves, so the probe re-decodes — 2 s now.
        let two_secs = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 88_200],
        };
        std::fs::write(&path, audio_edit::write_wav(&two_secs)).unwrap();
        assert_eq!(probe_clip(&cache, &c).duration_secs, Some(2.0));

        // The file vanishes: Missing again (the stat runs
        // before the cache lookup, so the stale entry
        // never resurrects it).
        let _ = std::fs::remove_file(&path);
        assert_eq!(probe_clip(&cache, &c).health, ClipHealth::Missing);
    }

    #[test]
    fn panel_prose_lines_cannot_fit_one_card_line() {
        // Wave 9 premise, pinned: the panel's content
        // budget is 316 pt (the 340 pt card minus
        // `panel_col`'s 2 × 12 pt padding) and LABEL
        // advances measure ≈6 pt/char off headless tree
        // dumps, so a one-line LABEL string busts the
        // card past ≈52 chars. The effects caption is a
        // FIXED string far past that — as an unwrapped
        // line it spilled on every render — and the
        // target line / readout embed unbounded user
        // file names on top of fixed segments already
        // near the limit. That is why all three (plus
        // the summary) carry `.wrap().w_full()` in the
        // view. If a future edit shortens these strings
        // under the limit, this test fails and the wrap
        // question must be re-asked on fresh numbers.
        let caption = effects_settings_line();
        assert!(
            caption.chars().count() > 120,
            "caption is {} chars",
            caption.chars().count()
        );
        let clips = vec![clip(
            "audio/my-very-long-background-music-track.wav",
            0,
            1.0,
        )];
        let line = target_line(&clips, None);
        assert!(
            line.chars().count() > 52,
            "target line is {} chars: {line}",
            line.chars().count()
        );
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(
            "audio/my-very-long-background-music-track.wav",
            29,
            0.5,
        ));
        let readout = clip_readout(&core, None, None).expect("one clip reads out");
        assert!(
            readout.chars().count() > 52,
            "readout is {} chars: {readout}",
            readout.chars().count()
        );
    }

    #[test]
    fn effects_settings_line_states_the_ops_fixed_values() {
        // The caption is the panel's honest answer to
        // audit D3 until the ops take parameters (I-4):
        // every value is the op constant in `audio_ops`.
        let line = effects_settings_line();
        assert!(line.contains("fade 0.25 s in/out"), "{line}");
        assert!(line.contains("echo 0.18 s delay, 35% feedback"), "{line}");
        assert!(line.contains("lowpass 1200 Hz"), "{line}");
        assert!(line.contains("pitch +2 semitones"), "{line}");
        assert!(line.contains("compress 0.5 threshold, 4:1 ratio"), "{line}");
    }

    #[test]
    fn audio_panel_builds_with_unhealthy_clips() {
        // Wave 4: rows for a missing file, an unsupported
        // (.mp3) file and a healthy file build side by
        // side — the warning-glyph branch and the health
        // suffixes are exercised, and the readout targets
        // the (missing) last clip.
        let t = rune::tok::theme(true);
        let pid = std::process::id();
        let dir = std::env::temp_dir();
        let mp3 = dir.join(format!("stykanimate-rune-audio-panel-build-{pid}.mp3"));
        std::fs::write(&mp3, b"junk").unwrap();
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 800],
        };
        let good = dir.join(format!("stykanimate-rune-audio-panel-build-{pid}.wav"));
        std::fs::write(&good, audio_edit::write_wav(&wav)).unwrap();

        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&good.to_string_lossy(), 0, 1.0));
        core.project.audio.push(clip(&mp3.to_string_lossy(), 9, 1.0));
        core.project.audio.push(clip("no/such/missing.wav", 30, 1.0));
        let _ = audio_panel(&state_with(core), &t);
        let _ = std::fs::remove_file(&mp3);
        let _ = std::fs::remove_file(&good);
    }

    #[test]
    fn display_name_takes_the_file_name() {
        assert_eq!(clip_display_name("audio/music.wav"), "music.wav");
        assert_eq!(clip_display_name("music.wav"), "music.wav");
        assert_eq!(clip_display_name("/a/b/c.ogg"), "c.ogg");
        assert_eq!(clip_display_name(""), "");
    }

    #[test]
    fn gain_suffix_only_for_non_unity_gain() {
        assert_eq!(gain_suffix(1.0), "");
        assert_eq!(gain_suffix(1.0005), "");
        assert_eq!(gain_suffix(1.5), " · gain 1.50");
        assert_eq!(gain_suffix(0.25), " · gain 0.25");
    }

    #[test]
    fn row_label_shows_name_start_frame_and_gain() {
        assert_eq!(clip_row_label(&clip("a/theme.wav", 0, 1.0)), "theme.wav · f1");
        assert_eq!(
            clip_row_label(&clip("a/theme.wav", 4, 2.0)),
            "theme.wav · f5 · gain 2.00"
        );
    }

    #[test]
    fn row_label_marks_a_muted_clip() {
        assert_eq!(
            clip_row_label(&muted_clip("a/theme.wav", 0, 1.0)),
            "theme.wav · f1 (muted)"
        );
        // The marker closes the label, after the gain
        // suffix when one is present.
        assert_eq!(
            clip_row_label(&muted_clip("a/theme.wav", 4, 2.0)),
            "theme.wav · f5 · gain 2.00 (muted)"
        );
    }

    #[test]
    fn summary_degrades_to_a_bare_count() {
        assert_eq!(format_summary(0, None, 0), "0 clips");
        assert_eq!(format_summary(2, None, 1), "2 clips");
    }

    #[test]
    fn summary_reports_total_and_gained_count() {
        assert_eq!(
            format_summary(3, Some(12.34), 1),
            "3 clips · total 12.3 s · 1 with gain ≠ 1.0"
        );
        assert_eq!(
            format_summary(1, Some(0.0), 0),
            "1 clips · total 0.0 s · 0 with gain ≠ 1.0"
        );
    }

    #[test]
    fn summary_line_skips_unreadable_files() {
        let clips = vec![
            clip("no/such/file-a.wav", 0, 1.0),
            clip("no/such/file-b.wav", 9, 0.5),
        ];
        let cache = RefCell::new(ProbeCache::new());
        let probes: Vec<ClipProbe> = clips.iter().map(|c| probe_clip(&cache, c)).collect();
        assert_eq!(summary_line(&clips, &probes), "2 clips");
        assert_eq!(summary_line(&[], &[]), "0 clips");
    }

    #[test]
    fn summary_line_sums_readable_wav_durations() {
        // One second of silence at 44.1 kHz, written with
        // the engine's own WAV writer.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 44_100],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let clips = vec![
            clip(&path.to_string_lossy(), 0, 1.0),
            clip("no/such/file.wav", 30, 2.0),
        ];
        let cache = RefCell::new(ProbeCache::new());
        let probes: Vec<ClipProbe> = clips.iter().map(|c| probe_clip(&cache, c)).collect();
        assert_eq!(
            summary_line(&clips, &probes),
            "2 clips · total 1.0 s · 1 with gain ≠ 1.0"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gain_readout_shows_the_last_clips_gain() {
        assert_eq!(gain_readout(&[]), "Gain —");
        assert_eq!(gain_readout(&[clip("a.wav", 0, 1.0)]), "Gain 1.00");
        // The readout follows the LAST clip (the stepper's
        // target), not the first.
        assert_eq!(
            gain_readout(&[clip("a.wav", 0, 2.0), clip("b.wav", 3, 0.5)]),
            "Gain 0.50"
        );
        assert_eq!(last_gain(&[]), None);
        assert_eq!(last_gain(&[clip("a.wav", 0, 0.75)]), Some(0.75));
    }

    #[test]
    fn gain_step_calls_update_the_readout_on_a_real_core() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("fake/theme.wav", 0, 1.0));
        assert_eq!(gain_readout(&core.project.audio), "Gain 1.00");

        // The exact step the Gain + button sends.
        core.audio_clip_gain_step(0.1);
        let gain = core.project.audio[0].gain;
        assert!((gain - 1.1).abs() < 1e-6, "unexpected gain {gain}");
        assert_eq!(gain_readout(&core.project.audio), "Gain 1.10");

        // Two Gain - presses land at 0.90.
        core.audio_clip_gain_step(-0.1);
        core.audio_clip_gain_step(-0.1);
        assert_eq!(gain_readout(&core.project.audio), "Gain 0.90");
    }

    #[test]
    fn delete_audio_clip_removes_the_indexed_clip() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("a/first.wav", 0, 1.0));
        core.project.audio.push(clip("a/second.wav", 5, 0.5));

        // The exact call the row's Delete button sends.
        core.delete_audio_clip(0);
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.project.audio[0].path, "a/second.wav");
        // The readout now targets the surviving last clip.
        assert_eq!(gain_readout(&core.project.audio), "Gain 0.50");
    }

    /// A shell `State` over the given core, field-for-field
    /// the literal `clips_panel`'s build test uses.
    fn state_with(core: EditorCore) -> State {
        State {
            core,
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    #[test]
    fn audio_panel_builds_toolbar_with_and_without_clips() {
        let t = rune::tok::theme(true);
        // No clips: the empty-state line plus the toolbar
        // (its `_last` buttons report "no clips" via status
        // when pressed) must still build.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let _ = audio_panel(&state_with(core), &t);

        // Clips present: rows (with Delete) and the full
        // toolbar incl. Fade / Reverse / gain stepper /
        // Earlier / Later build against the populated list.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("fake/theme.wav", 0, 1.25));
        core.project.audio.push(clip("fake/sting.wav", 9, 1.0));
        let _ = audio_panel(&state_with(core), &t);
    }

    #[test]
    fn audio_panel_builds_with_a_selected_row() {
        // Phase G: with a live selection the panel builds
        // the highlighted row + the selected target line;
        // with a STALE selection it builds the fallback
        // (last-clip) form instead of panicking.
        let t = rune::tok::theme(true);
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("fake/theme.wav", 0, 1.25));
        core.project.audio.push(clip("fake/sting.wav", 9, 1.0));
        let mut s = state_with(core);
        s.audio_panel.selected = Some(0);
        let _ = audio_panel(&s, &t);
        s.audio_panel.selected = Some(9);
        let _ = audio_panel(&s, &t);
    }

    #[test]
    fn echo_target_hits_the_selected_clip_not_the_last() {
        // Phase H: Echo used to be a `_last` op — with the
        // FIRST of two clips selected it would have echoed
        // clip 2. The retargeted handler must echo clip 1.
        // The op's observable effect (the `editor.rs`
        // materialise-and-repoint pattern): the clip's path
        // moves to the derived `.echo.wav` product and the
        // status names the clip by its 1-based index.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 800],
        };
        let pid = std::process::id();
        let first = std::env::temp_dir()
            .join(format!("stykanimate-rune-audio-panel-echo-a-{pid}.wav"));
        let second = std::env::temp_dir()
            .join(format!("stykanimate-rune-audio-panel-echo-b-{pid}.wav"));
        std::fs::write(&first, audio_edit::write_wav(&wav)).unwrap();
        std::fs::write(&second, audio_edit::write_wav(&wav)).unwrap();
        let first_path = first.to_string_lossy().into_owned();
        let second_path = second.to_string_lossy().into_owned();

        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&first_path, 0, 1.0));
        core.project.audio.push(clip(&second_path, 30, 1.0));
        let mut s = state_with(core);
        s.audio_panel.selected = Some(0);
        echo_target(&mut s);

        assert!(
            s.core.status.contains("Echo applied to audio clip 1"),
            "status: {}",
            s.core.status
        );
        let new_first = s.core.project.audio[0].path.clone();
        assert_ne!(new_first, first_path, "the selected clip is repointed");
        assert!(
            new_first.contains(".echo.wav"),
            "selected clip path: {new_first}"
        );
        assert_eq!(
            s.core.project.audio[1].path, second_path,
            "the unselected last clip is untouched"
        );
        let _ = std::fs::remove_file(&first);
        let _ = std::fs::remove_file(&second);
        let _ = std::fs::remove_file(&new_first);
    }

    #[test]
    fn audio_panel_builds_in_both_normalise_row_branches() {
        // Phase J: the "Normalise all" row is constructed
        // only under the `!clips.is_empty()` gate in
        // `audio_panel`, so the panel must build cleanly
        // in both shapes — no clips (no project row) and
        // clips present (row built). Key-level presence
        // in a built tree is the Runtime tests' domain
        // (`El` exposes no key/children inspection outside
        // `rune` itself), so — as with this file's other
        // builder tests — building both branches is the
        // assertion available here; the button's handler
        // is dispatch-tested below.
        let t = rune::tok::theme(true);
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let _ = audio_panel(&state_with(core), &t);

        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("fake/theme.wav", 0, 1.0));
        let _ = audio_panel(&state_with(core), &t);
    }

    #[test]
    fn normalise_all_handler_reports_a_silent_project_as_settled() {
        // Phase J dispatch, the nothing-to-do case: a
        // silent file's peak is 0 and silence cannot be
        // scaled onto the target, so the op skips it —
        // the status is the op's own "nothing needed"
        // line, no snapshot is taken (undo depth
        // unchanged) and the gain is untouched.
        let wav = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 800],
        };
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rune-audio-panel-normall-silent-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, audio_edit::write_wav(&wav)).unwrap();
        let clip_path = path.to_string_lossy().into_owned();

        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&clip_path, 0, 0.5));
        let mut s = state_with(core);
        let undo_before = s.core.undo_depth();
        normalise_all(&mut s);

        assert_eq!(
            s.core.status,
            "No audio clips needed normalising to peak 0.90."
        );
        assert_eq!(s.core.undo_depth(), undo_before, "no snapshot taken");
        assert_eq!(s.core.project.audio[0].gain, 0.5, "gain untouched");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn normalise_all_handler_reaches_every_clip_not_the_selection() {
        // Phase J dispatch, the working case: the FIRST
        // clip is selected and silent, the LAST is silent
        // too, and the MIDDLE one needs its gain scaled —
        // a selection-aimed or last-clip handler would
        // leave the middle clip alone; the project-wide
        // handler must scale it, in exactly one undo step,
        // and leave the silent clips' gains untouched.
        let pid = std::process::id();
        let dir = std::env::temp_dir();
        let silent_a =
            dir.join(format!("stykanimate-rune-audio-panel-normall-a-{pid}.wav"));
        let mid =
            dir.join(format!("stykanimate-rune-audio-panel-normall-b-{pid}.wav"));
        let silent_c =
            dir.join(format!("stykanimate-rune-audio-panel-normall-c-{pid}.wav"));
        let silence = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 800],
        };
        let half = audio_edit::WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 800],
        };
        std::fs::write(&silent_a, audio_edit::write_wav(&silence)).unwrap();
        std::fs::write(&mid, audio_edit::write_wav(&half)).unwrap();
        std::fs::write(&silent_c, audio_edit::write_wav(&silence)).unwrap();

        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip(&silent_a.to_string_lossy(), 0, 0.7));
        core.project.audio.push(clip(&mid.to_string_lossy(), 10, 1.0));
        core.project.audio.push(clip(&silent_c.to_string_lossy(), 20, 0.3));
        let mut s = state_with(core);
        s.audio_panel.selected = Some(0);
        let undo_before = s.core.undo_depth();
        normalise_all(&mut s);

        assert_eq!(s.core.status, "Normalised 1 audio clips to peak 0.90.");
        // 1.0 × (0.9 / 0.5) = 1.8, within the PCM16
        // round trip's tolerance.
        let gain = s.core.project.audio[1].gain;
        assert!((gain - 1.8).abs() < 0.01, "middle clip gain {gain}");
        assert_eq!(s.core.project.audio[0].gain, 0.7, "silent clip skipped");
        assert_eq!(s.core.project.audio[2].gain, 0.3, "silent clip skipped");
        assert_eq!(
            s.core.undo_depth(),
            undo_before + 1,
            "the whole pass is one snapshot"
        );
        let _ = std::fs::remove_file(&silent_a);
        let _ = std::fs::remove_file(&mid);
        let _ = std::fs::remove_file(&silent_c);
    }

    #[test]
    fn target_handlers_report_no_clips_instead_of_silence() {
        // Wave 10: the toolbar renders on an empty
        // project, and its `_last`-dispatched buttons
        // report through their ops ("Audio fade failed:
        // there are no audio clips."). The three handlers
        // with no `_last` variant — Normalise, Duplicate,
        // Mute — used to do NOTHING on an empty project:
        // a press with zero feedback. Each now sets the
        // op family's no-clips line itself, and nothing
        // else changes (no clip appears, no selection).
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let mut s = state_with(core);
        normalize_target(&mut s);
        assert_eq!(
            s.core.status,
            "Audio normalise failed: there are no audio clips."
        );
        duplicate_target(&mut s);
        assert_eq!(
            s.core.status,
            "Audio duplicate failed: there are no audio clips."
        );
        mute_target(&mut s);
        assert_eq!(
            s.core.status,
            "Audio mute failed: there are no audio clips."
        );
        assert!(s.core.project.audio.is_empty());
        assert_eq!(s.audio_panel.selected, None);
    }

    #[test]
    fn mute_target_toggles_the_effective_target() {
        // Phase M dispatch, in the `*_target` handler
        // test pattern: with the FIRST of two clips
        // selected, the toggle must flip clip 1 only — a
        // last-clip handler would flip clip 2 instead.
        // (Assertions read the model flag, not the op's
        // status wording, which is the `audio_ops`
        // section's own surface.)
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(clip("fake/theme.wav", 0, 1.0));
        core.project.audio.push(clip("fake/sting.wav", 9, 1.0));
        let mut s = state_with(core);
        s.audio_panel.selected = Some(0);
        mute_target(&mut s);
        assert!(s.core.project.audio[0].muted, "selected clip muted");
        assert!(!s.core.project.audio[1].muted, "last clip untouched");
        // A second press on the same target unmutes it.
        mute_target(&mut s);
        assert!(!s.core.project.audio[0].muted, "second press unmutes");

        // No selection: the fallback target is the LAST
        // clip (the `effective_target` discipline).
        s.audio_panel.selected = None;
        mute_target(&mut s);
        assert!(!s.core.project.audio[0].muted, "first clip untouched");
        assert!(s.core.project.audio[1].muted, "last clip muted by fallback");

        // No clips at all: no target, no panic, nothing
        // to flip.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let mut s = state_with(core);
        mute_target(&mut s);
        assert!(s.core.project.audio.is_empty());
    }
}
