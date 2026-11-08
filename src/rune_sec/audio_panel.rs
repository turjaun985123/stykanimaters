//! Audio panel for the Rune editor shell (Phase D,
//! section 15 of the GPUI→Rune migration). Declared from
//! `main.rs` only — bin-crate code, not part of the engine
//! library.
//!
//! The panel is the Rune form of the GPUI app's `audio_panel`:
//! a summary line, one row per clip (name, start frame,
//! per-clip gain), an operations toolbar acting on the LAST
//! clip, and an add-clip row with a file-name entry.
//!
//! Every control calls a real `EditorCore` op (evidence in
//! the handler comments). Three operations the old brief
//! listed have **no** `EditorCore` op and are therefore NOT
//! buttons here — the DSP exists in `audio_edit` (`fade`,
//! `audio_edit.rs:176`; `gain`, `:213`; `reverse`, `:225`)
//! but nothing materialises it (no snapshot / repoint /
//! status op), and there is no delete-clip op at all
//! (nothing removes a single clip from `project.audio`).
//! Wiring them needs new engine ops, which this section may
//! not write. Per-clip gain is likewise display-only: the
//! `AudioClip.gain` field exists (`model.rs:505`) and is
//! honoured by playback/mixdown, but no op mutates it.

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
}

impl AudioPanelState {
    pub fn new() -> Self {
        AudioPanelState {
            name: TextEdit::new(""),
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

/// `EditorCore::audio_normalize` on the last clip (the op
/// is index-based, `editor.rs:2641`); no-op when the
/// project has no clips — the last-clip effect buttons
/// below report that case through their own status text.
fn normalize_last(s: &mut State) {
    if let Some(i) = s.core.project.audio.len().checked_sub(1) {
        s.core.audio_normalize(i);
    }
}

// ---- View ----

/// The audio panel: header + summary, clip rows with the
/// per-clip ops the engine defines by index (Normalise,
/// Split at the current frame, Crossfade into the next
/// clip), the last-clip operations toolbar, and the
/// add-clip row. Element keys reuse the GPUI control ids.
pub fn audio_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let clips = &core.project.audio;

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
            let mut clip_row = row()
                .key(format!("audio-clip-{i}"))
                .w_full()
                .center_y()
                .gap(6.0)
                .child(
                    text(clip_row_label(clip))
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
            panel = panel.child(clip_row);
        }
    }

    // Operations on the LAST clip — the engine's `_last`
    // effect ops, plus index-based Normalise aimed at the
    // last clip and the whole-project Mixdown.
    panel = panel.child(text("Operations").font(type_::LABEL).color(t.text_secondary));
    panel = panel.child(
        row()
            .key("audio-ops")
            .gap(6.0)
            .center_y()
            .child(btn("audio-norm-last", "Normalise", false, t, normalize_last))
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
}
