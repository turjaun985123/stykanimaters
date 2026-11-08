//! Phase E (section 8): the audio engine ops Phase D's audio
//! panel found missing — fade, reverse, per-clip gain and
//! clip delete/move for `EditorCore`.
//!
//! Phase D section 15 verified there were no `EditorCore` ops
//! for gain, fade, reverse or delete-clip: only the raw DSP
//! existed in `audio_edit`, with no materialising op (no
//! snapshot / repoint / status), and nothing anywhere removed
//! a single clip from `project.audio`. This module closes
//! that gap so the Rune audio panel can wire real buttons.
//!
//! The file-materialising ops (`audio_fade_last`,
//! `audio_reverse_last`) follow the `audio_echo_last`
//! pattern from `editor.rs` exactly: read the last clip's
//! WAV, apply the `audio_edit` function, write the product
//! beside the source as `<stem>.<suffix>.wav` (the source is
//! never overwritten), take ONE undo snapshot, repoint the
//! clip, and set the status in the same voice. The field ops
//! (gain / delete / move) snapshot once and mutate the
//! project directly — gain needs no materialised file
//! because `AudioClip::gain` is honoured wherever clips are
//! mixed (playback and the MP4/WebM mux; see `model.rs`).
//!
//! `audio_derived_path` below is a copy of the private helper
//! of the same name in `editor.rs`: it is private to that
//! module, so this file cannot call it — the body is kept
//! identical so edit products land in the same place either
//! way.

use std::path::PathBuf;

use crate::editor::EditorCore;

/// Fixed fade lengths for `audio_fade_last` — the one-click
/// fade, in the spirit of the other fixed-parameter `*_last`
/// ops (echo 0.18 s / 35 %, low-pass 1200 Hz, pitch +2).
/// No MCP tool exposes `audio_edit::fade`, so there is no
/// MCP default to mirror.
const FADE_IN_SECS: f32 = 0.25;
const FADE_OUT_SECS: f32 = 0.25;

/// Path for an audio edit product: same directory as
/// `source`, with the source file stem plus `suffix` (for
/// example `.fade.wav`). The suffix always changes the file
/// name, so an edit product cannot accidentally be its own
/// source. Mirrors `EditorCore::audio_derived_path`
/// (private to `editor.rs`).
fn audio_derived_path(source: &PathBuf, suffix: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    let mut out = source.clone();
    out.set_file_name(format!("{stem}{suffix}"));
    out
}

impl EditorCore {
    /// Phase E: apply a linear fade-in and fade-out
    /// (0.25 s each, fixed like the other `*_last` ops'
    /// parameters) to the LAST audio clip. Same
    /// materialise-and-repoint pattern as `audio_echo_last`;
    /// the product is `<stem>.fade.wav` and the source file
    /// is never overwritten.
    pub fn audio_fade_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio fade failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio fade failed: {e}");
                return;
            }
        };
        crate::audio_edit::fade(&mut wav, FADE_IN_SECS, FADE_OUT_SECS);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".fade.wav");
        if out == source {
            self.status = "Audio fade failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!("Audio fade failed: could not write {}: {e}", out.display());
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Fade ({FADE_IN_SECS:.2}s in, {FADE_OUT_SECS:.2}s out) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Phase E: reverse the LAST audio clip (backwards
    /// playback). Same materialise-and-repoint pattern as
    /// `audio_echo_last`; the product is `<stem>.reverse.wav`
    /// and the source file is never overwritten.
    pub fn audio_reverse_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio reverse failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio reverse failed: {e}");
                return;
            }
        };
        crate::audio_edit::reverse(&mut wav);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".reverse.wav");
        if out == source {
            self.status = "Audio reverse failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio reverse failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Reverse applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Phase E: set the LAST audio clip's gain (a straight
    /// field write — no file is materialised, because the
    /// gain is applied wherever clips are mixed: playback
    /// and the MP4/WebM mux). The value is clamped to
    /// 0.0..=2.0 (0 = silent, 1 = unity, 2 = double); a NaN
    /// request falls back to unity rather than poisoning
    /// the mix. One snapshot.
    pub fn audio_set_gain_last(&mut self, gain: f32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio gain failed: there are no audio clips.".into();
            return;
        };
        let gain = if gain.is_nan() {
            1.0
        } else {
            gain.clamp(0.0, 2.0)
        };
        self.snapshot();
        self.project.audio[clip_idx].gain = gain;
        self.status = format!("Audio clip {} gain set to {gain:.2}.", clip_idx + 1);
    }

    /// Phase E: step the LAST audio clip's gain by `delta`
    /// (the panel's −/+ buttons). Delegates to
    /// `audio_set_gain_last`, so the clamp, the single
    /// snapshot and the status all come from there.
    pub fn audio_clip_gain_step(&mut self, delta: f32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio gain failed: there are no audio clips.".into();
            return;
        };
        let gain = self.project.audio[clip_idx].gain + delta;
        self.audio_set_gain_last(gain);
    }

    /// Phase E: delete audio clip `index` (0-based) from the
    /// timeline. The clip's file is left on disk — only the
    /// timeline entry is removed. One snapshot; an
    /// out-of-range index is a status-only no-op.
    pub fn delete_audio_clip(&mut self, index: usize) {
        let Some(clip) = self.project.audio.get(index).cloned() else {
            self.status = format!("Audio delete failed: no clip {}.", index + 1);
            return;
        };
        self.snapshot();
        self.project.audio.remove(index);
        self.status = format!("Audio clip {} deleted: {}.", index + 1, clip.path);
    }

    /// Phase E: move the LAST audio clip by `delta_frames`
    /// on the timeline (negative = earlier). The start frame
    /// clamps at 0. One snapshot.
    pub fn audio_move_last(&mut self, delta_frames: i32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio move failed: there are no audio clips.".into();
            return;
        };
        let start = self.project.audio[clip_idx].start_frame as i64 + delta_frames as i64;
        let start = start.max(0) as usize;
        self.snapshot();
        self.project.audio[clip_idx].start_frame = start;
        self.status = format!(
            "Audio clip {} moved to frame {}.",
            clip_idx + 1,
            start + 1
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AudioClip;

    fn clip(path: &str, start_frame: usize) -> AudioClip {
        AudioClip {
            path: path.into(),
            start_frame,
            gain: 1.0,
        }
    }

    /// A core whose project carries the given fake clips —
    /// the field ops never touch the files, so no fixture
    /// WAVs are needed.
    fn core_with(clips: Vec<AudioClip>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio = clips;
        core
    }

    #[test]
    fn set_gain_writes_the_last_clip_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4)]);
        core.audio_set_gain_last(1.5);
        assert_eq!(core.project.audio[0].gain, 1.0);
        assert_eq!(core.project.audio[1].gain, 1.5);
        assert_eq!(core.status, "Audio clip 2 gain set to 1.50.");
    }

    #[test]
    fn set_gain_clamps_to_zero_to_two() {
        let mut core = core_with(vec![clip("a.wav", 0)]);
        core.audio_set_gain_last(3.5);
        assert_eq!(core.project.audio[0].gain, 2.0);
        core.audio_set_gain_last(-0.25);
        assert_eq!(core.project.audio[0].gain, 0.0);
        core.audio_set_gain_last(f32::NAN);
        assert_eq!(core.project.audio[0].gain, 1.0);
    }

    #[test]
    fn gain_step_moves_relative_to_current() {
        let mut core = core_with(vec![clip("a.wav", 0)]);
        core.audio_clip_gain_step(0.25);
        assert_eq!(core.project.audio[0].gain, 1.25);
        core.audio_clip_gain_step(-2.0);
        assert_eq!(core.project.audio[0].gain, 0.0);
    }

    #[test]
    fn gain_ops_without_clips_only_set_status() {
        let mut core = EditorCore::new();
        core.audio_set_gain_last(1.5);
        assert_eq!(core.status, "Audio gain failed: there are no audio clips.");
        core.audio_clip_gain_step(0.1);
        assert_eq!(core.status, "Audio gain failed: there are no audio clips.");
        assert!(core.project.audio.is_empty());
    }

    #[test]
    fn delete_removes_the_right_clip() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4), clip("c.wav", 8)]);
        core.delete_audio_clip(1);
        assert_eq!(core.project.audio.len(), 2);
        assert_eq!(core.project.audio[0].path, "a.wav");
        assert_eq!(core.project.audio[1].path, "c.wav");
        assert_eq!(core.status, "Audio clip 2 deleted: b.wav.");
    }

    #[test]
    fn delete_out_of_range_is_a_no_op() {
        let mut core = core_with(vec![clip("a.wav", 0)]);
        core.delete_audio_clip(5);
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.status, "Audio delete failed: no clip 6.");
    }

    #[test]
    fn move_shifts_and_clamps_at_zero() {
        let mut core = core_with(vec![clip("a.wav", 10)]);
        core.audio_move_last(5);
        assert_eq!(core.project.audio[0].start_frame, 15);
        assert_eq!(core.status, "Audio clip 1 moved to frame 16.");
        core.audio_move_last(-25);
        assert_eq!(core.project.audio[0].start_frame, 0);
        assert_eq!(core.status, "Audio clip 1 moved to frame 1.");
    }

    #[test]
    fn move_without_clips_only_sets_status() {
        let mut core = EditorCore::new();
        core.audio_move_last(3);
        assert_eq!(core.status, "Audio move failed: there are no audio clips.");
    }

    #[test]
    fn file_ops_without_clips_only_set_status() {
        // The materialising ops are exercised only on their
        // no-clips path here — no fixture files (per brief).
        let mut core = EditorCore::new();
        core.audio_fade_last();
        assert_eq!(core.status, "Audio fade failed: there are no audio clips.");
        core.audio_reverse_last();
        assert_eq!(core.status, "Audio reverse failed: there are no audio clips.");
    }

    #[test]
    fn derived_path_swaps_the_file_name() {
        let out = audio_derived_path(&PathBuf::from("audio/theme.wav"), ".fade.wav");
        assert_eq!(out, PathBuf::from("audio/theme.fade.wav"));
        let out = audio_derived_path(&PathBuf::from("song"), ".reverse.wav");
        assert_eq!(out, PathBuf::from("song.reverse.wav"));
    }
}
