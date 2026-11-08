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
//! other per-row ops were always index-based; the remaining
//! toolbar effects — Echo, Lowpass, Trim silence, Split
//! silences, Pitch, Compress — still act on the last clip:
//! no indexed variants of those `editor.rs` ops exist.)
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

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, text_input, El, TextEdit, Tx};
use stykanimate_rs::audio_edit;
use stykanimate_rs::model::AudioClip;

use crate::chrome_view::btn;
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
}

impl AudioPanelState {
    pub fn new() -> Self {
        AudioPanelState {
            name: TextEdit::new(""),
            selected: None,
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
/// gain suffix when non-unity (the GPUI row format).
pub fn clip_row_label(clip: &AudioClip) -> String {
    format!(
        "{} · f{}{}",
        clip_display_name(&clip.path),
        clip.start_frame + 1,
        gain_suffix(clip.gain)
    )
}

/// `" · {d:.1} s"` — the clip's duration — when its file
/// reads back as WAV, `""` when it does not (the row then
/// shows name + start only). Reads the filesystem, like
/// `summary_line` does.
pub fn clip_duration_suffix(clip: &AudioClip) -> String {
    match audio_edit::read_wav(&clip.path) {
        Ok(wav) => format!(" · {:.1} s", wav.duration_secs()),
        Err(_) => String::new(),
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

/// The summary line for a clip list: sums the durations of
/// the clips whose files read back as WAV
/// (`audio_edit::read_wav`, `audio_edit.rs:41`) and counts
/// the non-unity gains. Unreadable files are skipped
/// silently; when nothing reads back the line degrades to
/// the bare count. (Reads the filesystem, like the GPUI
/// panel's render-time summary did.)
pub fn summary_line(clips: &[AudioClip]) -> String {
    let mut total_secs = 0.0f32;
    let mut any_readable = false;
    for clip in clips {
        if let Ok(wav) = audio_edit::read_wav(&clip.path) {
            total_secs += wav.duration_secs();
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
/// No-op when the project has no clips.
fn normalize_target(s: &mut State) {
    if let Some(i) = effective_target(s.audio_panel.selected, s.core.project.audio.len()) {
        s.core.audio_normalize(i);
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
/// clip), and the add-clip row. Element keys reuse the
/// GPUI control ids where they existed; the Phase E keys
/// (`audio-fade`, `audio-reverse`, `audio-gain-*`,
/// `audio-move-*`, `audio-del-{i}`) are new; Phase G adds
/// no keys — row selection rides the existing
/// `audio-clip-{i}` row keys.
pub fn audio_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let clips = &core.project.audio;
    let selected = s.audio_panel.selected;

    let mut panel = col()
        .key("audio-panel")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .w_full()
                .center_y()
                .child(text("Audio").font(type_::TITLE).color(t.text))
                .child(el().grow(1.0))
                .child(
                    text(format!("Audio — {} clip(s)", clips.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        );

    if clips.is_empty() {
        panel = panel.child(
            text("No audio clips in this project.")
                .font(type_::BODY)
                .color(t.text_secondary),
        );
    } else {
        panel = panel.child(
            text(summary_line(clips))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
        panel = panel.child(text("Clips").font(type_::LABEL).color(t.text_secondary));
        for (i, clip) in clips.iter().enumerate() {
            // Phase G: a row press selects the clip for the
            // operations toolbar (the `fx_clips` lane
            // pattern — the row carries the fill, the inner
            // buttons stay the press targets inside their
            // own boxes); the selected row is filled with
            // the control-track colour.
            let is_selected = selected == Some(i);
            let mut clip_row = row()
                .key(format!("audio-clip-{i}"))
                .w_full()
                .center_y()
                .gap(6.0)
                .px(8.0)
                .py(5.0)
                .radius(6.0)
                .fill(if is_selected { t.control_track } else { t.card })
                .hover(if is_selected {
                    t.control_track
                } else {
                    t.control_off
                })
                .on_press(move |s: &mut State| {
                    s.audio_panel.selected = Some(i);
                })
                .child(
                    text(format!(
                        "{}{}",
                        clip_row_label(clip),
                        clip_duration_suffix(clip)
                    ))
                    .font(type_::BODY)
                    .color(t.text)
                    .grow(1.0),
                )
                .child(btn(&format!("audio-norm-{i}"), "Norm", false, t, move |s| {
                    s.core.audio_normalize(i)
                }))
                .child(btn(
                    &format!("audio-split-{i}"),
                    "Split@cur",
                    false,
                    t,
                    move |s| {
                        let at = s.core.current;
                        s.core.audio_split(i, at)
                    },
                ));
            if i + 1 < clips.len() {
                clip_row = clip_row.child(btn(
                    &format!("audio-xfade-{i}"),
                    "Xfade",
                    false,
                    t,
                    move |s| s.core.audio_crossfade(i),
                ));
            }
            clip_row = clip_row.child(btn(
                &format!("audio-del-{i}"),
                "Delete",
                false,
                t,
                move |s| delete_row_clip(s, i),
            ));
            panel = panel.child(clip_row);
        }
    }

    // Operations toolbar. Phase G aims Fade, Reverse,
    // the gain stepper, Earlier/Later and Normalise at
    // the SELECTED clip (the indexed `audio_ops` variants,
    // dispatched by the `*_target` handlers), falling back
    // to the last clip when nothing is selected — the
    // target line under the header says which. The
    // remaining effects (Echo, Lowpass, Trim silence,
    // Split silences, Pitch, Compress) are still the
    // engine's `_last` ops: no indexed variants exist.
    // Mixdown is whole-project.
    panel = panel.child(text("Operations").font(type_::LABEL).color(t.text_secondary));
    panel = panel.child(
        text(target_line(clips, selected))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    panel = panel.child(
        row()
            .key("audio-ops")
            .gap(6.0)
            .center_y()
            .child(btn("audio-norm-last", "Normalise", false, t, normalize_target))
            .child(btn("audio-echo", "Echo", false, t, |s| {
                s.core.audio_echo_last()
            }))
            .child(btn("audio-lowpass", "Lowpass", false, t, |s| {
                s.core.audio_lowpass_last()
            }))
            .child(btn("audio-trimsil", "Trim silence", false, t, |s| {
                s.core.audio_trim_silence_last()
            }))
            .child(btn("audio-split-sil", "Split silences", false, t, |s| {
                s.core.audio_split_at_silences_last()
            }))
            .child(btn("audio-pitch", "Pitch +2", false, t, |s| {
                s.core.audio_pitch_last()
            }))
            .child(btn("audio-comp", "Compress", false, t, |s| {
                s.core.audio_compress_last()
            }))
            .child(btn("audio-fade", "Fade", false, t, fade_target))
            .child(btn("audio-reverse", "Reverse", false, t, reverse_target))
            .child(btn("audio-gain-dn", "Gain -", false, t, |s| {
                gain_step_target(s, -0.1);
            }))
            .child(
                text(target_gain_readout(clips, selected))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            )
            .child(btn("audio-gain-up", "Gain +", false, t, |s| {
                gain_step_target(s, 0.1);
            }))
            .child(btn("audio-move-earlier", "Earlier", false, t, |s| {
                move_target(s, -1);
            }))
            .child(btn("audio-move-later", "Later", false, t, |s| {
                move_target(s, 1);
            }))
            .child(btn("mixdown", "Mixdown", false, t, |s| {
                s.core.mixdown_audio()
            }))
            .scroll_x()
            .w_full(),
    );

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
            .child(btn("add-audio", "Add clip", false, t, |s| add_clip(s))),
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
        }
    }

    #[test]
    fn panel_state_starts_with_an_empty_name() {
        assert_eq!(AudioPanelState::new().name.text, "");
        assert_eq!(AudioPanelState::default().name.text, "");
        // Phase G: nothing is selected until a row is
        // pressed, in both constructors.
        assert_eq!(AudioPanelState::new().selected, None);
        assert_eq!(AudioPanelState::default().selected, None);
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
    fn duration_suffix_reads_readable_wavs_only() {
        // Unreadable path: no suffix, the row shows name +
        // start only.
        assert_eq!(clip_duration_suffix(&clip("no/such/file.wav", 0, 1.0)), "");
        // One second of silence at 44.1 kHz, written with
        // the engine's own WAV writer.
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
        assert_eq!(clip_duration_suffix(&c), " · 1.0 s");
        let _ = std::fs::remove_file(&path);
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
        assert_eq!(summary_line(&clips), "2 clips");
        assert_eq!(summary_line(&[]), "0 clips");
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
        assert_eq!(
            summary_line(&clips),
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
loop_playback: true,
            was_playing: false,
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
}
