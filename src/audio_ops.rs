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
//! The file-materialising ops (`audio_fade_at`,
//! `audio_reverse_at`) follow the `audio_echo_last`
//! pattern from `editor.rs` exactly: read the clip's
//! WAV, apply the `audio_edit` function, write the product
//! beside the source as `<stem>.<suffix>.wav` (the source is
//! never overwritten), take ONE undo snapshot, repoint the
//! clip, and set the status in the same voice. The field ops
//! (gain / delete / move) snapshot once and mutate the
//! project directly — gain needs no materialised file
//! because `AudioClip::gain` is honoured wherever clips are
//! mixed (playback and the MP4/WebM mux; see `model.rs`).
//!
//! Phase G (section 3): every op is now INDEXED, following
//! the `fx_ops` convention — the primary implementation
//! takes the clip index (`audio_fade_at`,
//! `audio_reverse_at`, `audio_set_gain_at`,
//! `audio_clip_gain_step_at`, `delete_audio_clip_at`,
//! `audio_move_at`), an out-of-range index is a status-only
//! refusal ("… failed: no clip N.") that takes NO snapshot
//! and changes nothing, and an in-range op snapshots exactly
//! once. The Phase E names are kept as thin wrappers over
//! the indexed primaries: the `*_last` ops delegate to the
//! primary at `len - 1` (an empty project keeps the Phase E
//! "there are no audio clips." status), and
//! `delete_audio_clip` — which already took an index in
//! Phase E — delegates to `delete_audio_clip_at`. Wrapper
//! statuses stay byte-identical to Phase E; MCP and the
//! panel's existing buttons are unaffected.
//!
//! `audio_derived_path` below is a copy of the private helper
//! of the same name in `editor.rs`: it is private to that
//! module, so this file cannot call it — the body is kept
//! identical so edit products land in the same place either
//! way.

use std::path::PathBuf;

use crate::editor::EditorCore;

/// Fixed fade lengths for `audio_fade_at` — the one-click
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
    /// Phase G: apply a linear fade-in and fade-out
    /// (0.25 s each, fixed like the other one-click ops'
    /// parameters) to audio clip `index` (0-based). Same
    /// materialise-and-repoint pattern as `audio_echo_last`;
    /// the product is `<stem>.fade.wav` and the source file
    /// is never overwritten. One snapshot on success; an
    /// out-of-range index only sets the status.
    pub fn audio_fade_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio fade failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
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
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Fade ({FADE_IN_SECS:.2}s in, {FADE_OUT_SECS:.2}s out) applied to audio clip {}: {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase E: fade the LAST audio clip. Thin wrapper over
    /// [`Self::audio_fade_at`] at `len - 1`; an empty project
    /// keeps the Phase E status.
    pub fn audio_fade_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio fade failed: there are no audio clips.".into();
            return;
        };
        self.audio_fade_at(clip_idx);
    }

    /// Phase G: reverse audio clip `index` (0-based;
    /// backwards playback). Same materialise-and-repoint
    /// pattern as `audio_echo_last`; the product is
    /// `<stem>.reverse.wav` and the source file is never
    /// overwritten. One snapshot on success; an
    /// out-of-range index only sets the status.
    pub fn audio_reverse_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio reverse failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
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
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Reverse applied to audio clip {}: {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase E: reverse the LAST audio clip. Thin wrapper
    /// over [`Self::audio_reverse_at`] at `len - 1`; an
    /// empty project keeps the Phase E status.
    pub fn audio_reverse_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio reverse failed: there are no audio clips.".into();
            return;
        };
        self.audio_reverse_at(clip_idx);
    }

    /// Phase G: set audio clip `index`'s gain (0-based; a
    /// straight field write — no file is materialised,
    /// because the gain is applied wherever clips are mixed:
    /// playback and the MP4/WebM mux). The value is clamped
    /// to 0.0..=2.0 (0 = silent, 1 = unity, 2 = double); a
    /// NaN request falls back to unity rather than poisoning
    /// the mix. One snapshot; an out-of-range index only
    /// sets the status.
    pub fn audio_set_gain_at(&mut self, index: usize, gain: f32) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio gain failed: no clip {}.", index + 1);
            return;
        }
        let gain = if gain.is_nan() {
            1.0
        } else {
            gain.clamp(0.0, 2.0)
        };
        self.snapshot();
        self.project.audio[index].gain = gain;
        self.status = format!("Audio clip {} gain set to {gain:.2}.", index + 1);
    }

    /// Phase E: set the LAST audio clip's gain. Thin wrapper
    /// over [`Self::audio_set_gain_at`] at `len - 1`; an
    /// empty project keeps the Phase E status.
    pub fn audio_set_gain_last(&mut self, gain: f32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio gain failed: there are no audio clips.".into();
            return;
        };
        self.audio_set_gain_at(clip_idx, gain);
    }

    /// Phase G: step audio clip `index`'s gain by `delta`
    /// (0-based; the panel's −/+ buttons for the selected
    /// clip). Delegates to [`Self::audio_set_gain_at`], so
    /// the clamp, the single snapshot and the status all
    /// come from there. An out-of-range index only sets the
    /// status.
    pub fn audio_clip_gain_step_at(&mut self, index: usize, delta: f32) {
        let Some(clip) = self.project.audio.get(index) else {
            self.status = format!("Audio gain failed: no clip {}.", index + 1);
            return;
        };
        let gain = clip.gain + delta;
        self.audio_set_gain_at(index, gain);
    }

    /// Phase E: step the LAST audio clip's gain by `delta`.
    /// Thin wrapper over [`Self::audio_clip_gain_step_at`]
    /// at `len - 1`; an empty project keeps the Phase E
    /// status.
    pub fn audio_clip_gain_step(&mut self, delta: f32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio gain failed: there are no audio clips.".into();
            return;
        };
        self.audio_clip_gain_step_at(clip_idx, delta);
    }

    /// Phase G: delete audio clip `index` (0-based) from the
    /// timeline. The clip's file is left on disk — only the
    /// timeline entry is removed. One snapshot; an
    /// out-of-range index is a status-only no-op.
    pub fn delete_audio_clip_at(&mut self, index: usize) {
        let Some(clip) = self.project.audio.get(index).cloned() else {
            self.status = format!("Audio delete failed: no clip {}.", index + 1);
            return;
        };
        self.snapshot();
        self.project.audio.remove(index);
        self.status = format!("Audio clip {} deleted: {}.", index + 1, clip.path);
    }

    /// Phase E name for [`Self::delete_audio_clip_at`] —
    /// delete already took an index in Phase E, so this is
    /// a plain delegation kept for existing callers (the
    /// audio panel's per-row delete button, MCP).
    pub fn delete_audio_clip(&mut self, index: usize) {
        self.delete_audio_clip_at(index);
    }

    /// Phase G: move audio clip `index` (0-based) by
    /// `delta_frames` on the timeline (negative = earlier).
    /// The start frame clamps at 0. One snapshot; an
    /// out-of-range index only sets the status.
    pub fn audio_move_at(&mut self, index: usize, delta_frames: i32) {
        let Some(clip) = self.project.audio.get(index) else {
            self.status = format!("Audio move failed: no clip {}.", index + 1);
            return;
        };
        let start = clip.start_frame as i64 + delta_frames as i64;
        let start = start.max(0) as usize;
        self.snapshot();
        self.project.audio[index].start_frame = start;
        self.status = format!(
            "Audio clip {} moved to frame {}.",
            index + 1,
            start + 1
        );
    }

    /// Phase E: move the LAST audio clip by `delta_frames`.
    /// Thin wrapper over [`Self::audio_move_at`] at
    /// `len - 1`; an empty project keeps the Phase E status.
    pub fn audio_move_last(&mut self, delta_frames: i32) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio move failed: there are no audio clips.".into();
            return;
        };
        self.audio_move_at(clip_idx, delta_frames);
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

    // ---- Phase G: indexed variants ----

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. Mirrors the `fx_ops`
    /// test helper (`Project` has no `PartialEq`, so this is
    /// how "untouched" and cross-core equality are asserted).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    /// Clip paths in timeline order. `AudioClip` has no
    /// name field, so the path is a clip's identity — this
    /// is the order/identity assertion for the multi-clip
    /// tests.
    fn paths(core: &EditorCore) -> Vec<&str> {
        core.project.audio.iter().map(|c| c.path.as_str()).collect()
    }

    /// A scratch directory unique to one test: tests run in
    /// parallel threads inside one process, so the tag — not
    /// just the pid — keeps two file tests apart.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-audio-ops-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a one-second mono WAV whose samples ramp from
    /// -0.5 up towards +0.5, so a fade (first frame driven
    /// to 0, mid-fade frame halved) and a reverse (the ramp
    /// flips end for end) both leave checkable traces in the
    /// product file. Returns the path as a `String`.
    fn write_ramp_wav(dir: &PathBuf, name: &str) -> String {
        let samples: Vec<f32> = (0..8000).map(|i| i as f32 / 8000.0 - 0.5).collect();
        let wav = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let path = dir.join(name);
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn fade_at_targets_the_chosen_clip() {
        let dir = temp_dir("fade-at");
        let a = write_ramp_wav(&dir, "a.wav");
        let b = write_ramp_wav(&dir, "b.wav");
        let c = write_ramp_wav(&dir, "c.wav");
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4), clip(&c, 8)]);
        let edits = core.edits_since_autosave();
        core.audio_fade_at(0);
        // Only clip 1 was repointed, at its own fade product;
        // clips 2 and 3 kept their paths and their order.
        let expected = dir.join("a.fade.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str(), c.as_str()]);
        assert_eq!(
            core.status,
            format!("Fade (0.25s in, 0.25s out) applied to audio clip 1: {expected}.")
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The product really is the fade of clip 1's file:
        // frame 0 is silenced by the fade-in, a mid-fade-in
        // frame is halved, a frame past both fades is whole.
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert_eq!(product.samples[0], 0.0);
        assert!((product.samples[1000] - (-0.1875)).abs() < 0.001);
        assert!((product.samples[3000] - (-0.125)).abs() < 0.001);
        // The source file was never overwritten.
        let source = crate::audio_edit::read_wav(&a).unwrap();
        assert_eq!(source.samples[0], -0.5);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reverse_at_targets_the_chosen_clip() {
        let dir = temp_dir("reverse-at");
        let a = write_ramp_wav(&dir, "a.wav");
        let b = write_ramp_wav(&dir, "b.wav");
        let c = write_ramp_wav(&dir, "c.wav");
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4), clip(&c, 8)]);
        let edits = core.edits_since_autosave();
        core.audio_reverse_at(1);
        let expected = dir.join("b.reverse.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![a.as_str(), expected.as_str(), c.as_str()]);
        assert_eq!(core.status, format!("Reverse applied to audio clip 2: {expected}."));
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The product is clip 2's ramp flipped end for end.
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert!((product.samples[0] - 0.4999).abs() < 0.001);
        assert!((product.samples[7999] - (-0.5)).abs() < 0.001);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_ops_at_out_of_range_are_status_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.audio_fade_at(2);
        assert_eq!(core.status, "Audio fade failed: no clip 3.");
        core.audio_reverse_at(9);
        assert_eq!(core.status, "Audio reverse failed: no clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // Same refusal voice on an empty project.
        let mut empty = EditorCore::new();
        empty.audio_fade_at(0);
        assert_eq!(empty.status, "Audio fade failed: no clip 1.");
        empty.audio_reverse_at(0);
        assert_eq!(empty.status, "Audio reverse failed: no clip 1.");
    }

    #[test]
    fn fade_last_matches_fade_at_last_index() {
        // Wrapper equivalence: both cores point at the SAME
        // source files, so the last-clip wrapper and the
        // indexed primary at len - 1 must produce identical
        // projects, statuses and undo depths.
        let dir = temp_dir("fade-equiv");
        let a = write_ramp_wav(&dir, "a.wav");
        let b = write_ramp_wav(&dir, "b.wav");
        let make = || core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let mut via_last = make();
        let mut via_at = make();
        via_last.audio_fade_last();
        via_at.audio_fade_at(1);
        assert_eq!(project_json(&via_last), project_json(&via_at));
        assert_eq!(via_last.status, via_at.status);
        assert_eq!(via_last.edits_since_autosave(), via_at.edits_since_autosave());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_gain_at_writes_the_chosen_clip_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4), clip("c.wav", 8)]);
        let edits = core.edits_since_autosave();
        core.audio_set_gain_at(0, 1.5);
        assert_eq!(core.project.audio[0].gain, 1.5);
        assert_eq!(core.project.audio[1].gain, 1.0);
        assert_eq!(core.project.audio[2].gain, 1.0);
        assert_eq!(core.status, "Audio clip 1 gain set to 1.50.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Clamp and NaN fallback apply per clip, as for _last.
        core.audio_set_gain_at(2, 3.5);
        assert_eq!(core.project.audio[2].gain, 2.0);
        core.audio_set_gain_at(2, f32::NAN);
        assert_eq!(core.project.audio[2].gain, 1.0);
        assert_eq!(paths(&core), vec!["a.wav", "b.wav", "c.wav"], "order untouched");
    }

    #[test]
    fn gain_step_at_moves_the_chosen_clip_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4), clip("c.wav", 8)]);
        let edits = core.edits_since_autosave();
        core.audio_clip_gain_step_at(1, 0.25);
        assert_eq!(core.project.audio[0].gain, 1.0);
        assert_eq!(core.project.audio[1].gain, 1.25);
        assert_eq!(core.project.audio[2].gain, 1.0);
        assert_eq!(core.status, "Audio clip 2 gain set to 1.25.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn gain_at_out_of_range_is_status_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.audio_set_gain_at(2, 1.5);
        assert_eq!(core.status, "Audio gain failed: no clip 3.");
        core.audio_clip_gain_step_at(5, 0.1);
        assert_eq!(core.status, "Audio gain failed: no clip 6.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn delete_at_removes_the_chosen_clip_and_keeps_order() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4), clip("c.wav", 8)]);
        let edits = core.edits_since_autosave();
        core.delete_audio_clip_at(0);
        assert_eq!(paths(&core), vec!["b.wav", "c.wav"]);
        assert_eq!(core.status, "Audio clip 1 deleted: a.wav.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The one snapshot is a real undo step.
        core.undo();
        assert_eq!(core.project.audio.len(), 3);
        assert_eq!(paths(&core), vec!["a.wav", "b.wav", "c.wav"]);
    }

    #[test]
    fn delete_wrapper_matches_delete_at() {
        // Wrapper equivalence: the Phase E name and the _at
        // primary must land two identical projects in
        // identical states — same clips, same status, same
        // undo depth — on success and on refusal alike.
        let make = || core_with(vec![clip("a.wav", 0), clip("b.wav", 4), clip("c.wav", 8)]);
        let mut via_name = make();
        let mut via_at = make();
        via_name.delete_audio_clip(1);
        via_at.delete_audio_clip_at(1);
        assert_eq!(project_json(&via_name), project_json(&via_at));
        assert_eq!(via_name.status, via_at.status);
        assert_eq!(via_name.status, "Audio clip 2 deleted: b.wav.");
        assert_eq!(via_name.edits_since_autosave(), via_at.edits_since_autosave());
        via_name.delete_audio_clip(7);
        via_at.delete_audio_clip_at(7);
        assert_eq!(via_name.status, "Audio delete failed: no clip 8.");
        assert_eq!(via_name.status, via_at.status);
        assert_eq!(project_json(&via_name), project_json(&via_at));
    }

    #[test]
    fn move_at_shifts_the_chosen_clip_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 10), clip("c.wav", 20)]);
        let edits = core.edits_since_autosave();
        core.audio_move_at(1, 5);
        assert_eq!(core.project.audio[0].start_frame, 0);
        assert_eq!(core.project.audio[1].start_frame, 15);
        assert_eq!(core.project.audio[2].start_frame, 20);
        assert_eq!(core.status, "Audio clip 2 moved to frame 16.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Moving earlier past frame 0 clamps, as for _last.
        core.audio_move_at(1, -25);
        assert_eq!(core.project.audio[1].start_frame, 0);
        assert_eq!(core.status, "Audio clip 2 moved to frame 1.");
        assert_eq!(paths(&core), vec!["a.wav", "b.wav", "c.wav"], "order untouched");
    }

    #[test]
    fn move_at_out_of_range_is_status_only() {
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.audio_move_at(3, 5);
        assert_eq!(core.status, "Audio move failed: no clip 4.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }
}
