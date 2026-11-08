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
//! Phase H (section 37): the remaining last-clip-only DSP
//! effects get indexed primaries here too —
//! `audio_echo_at`, `audio_lowpass_at`,
//! `audio_trim_silence_at`, `audio_split_silences_at`,
//! `audio_pitch_at`, `audio_compress_at`. Their originals
//! live in `editor.rs` (`audio_echo_last` and family,
//! Waves 11–13) and are NOT touched: each primary below
//! mirrors its `editor.rs` body step for step — same
//! `audio_edit` DSP call with the same fixed parameters,
//! same product suffix, same success/failure status text
//! with the clip number taken from the index — so the DSP
//! maths exists exactly once, in `audio_edit`. The panel
//! can now aim these effects at the selected clip instead
//! of always the last one.
//!
//! Phase I (section 4): [`clip_peaks`] moves the per-clip
//! waveform peak computation out of the Rune waveform panel
//! and into the library, so a selected-clip view can ask for
//! one clip's peaks by index instead of re-deriving them in
//! panel code. It reuses the one loader path the panel has
//! always used (see its note below).
//!
//! Phase J (section 7): [`EditorCore::normalise_all_clips`]
//! brings every clip to a common peak in one undo step. It
//! measures each clip with [`clip_peaks`] (gain not applied,
//! as there) and scales each gain by `target / peak` through
//! the pure helper [`normalised_gain`], which applies the
//! gain family's own sanitising — the 0.0..=2.0 clamp and
//! the NaN fallback of [`EditorCore::audio_set_gain_at`].
//! One snapshot covers the whole operation, and only when
//! at least one clip actually changes; silent and
//! unreadable clips are skipped with their gains untouched.
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

/// Phase I (section 4): the peak amplitude per bucket across
/// audio clip `index`'s own samples, for a selected-clip
/// waveform view.
///
/// The loading path is exactly the one the Rune waveform
/// panel (`rune_sec/waveform_view.rs`) has always used: the
/// clip's file is read with [`crate::audio_edit::read_wav`]
/// and reduced with [`crate::audio_edit::peaks`] — no other
/// decoder and no other reduction. Normalisation therefore
/// matches the existing waveform code exactly: `read_wav`
/// decodes PCM16 samples to -1.0..=1.0, and `peaks` takes
/// the max-abs amplitude per bucket across all channels, so
/// every returned value lies in 0.0..=1.0. The clip's `gain`
/// is NOT applied here — the panel scales by gain at draw
/// time, as it does for the project waveform today.
///
/// Pure and read-only: it takes `&EditorCore`, takes no
/// snapshot and writes no status, so probing a waveform can
/// never dirty the undo stack.
///
/// Every failure reads as an empty vec, so the panel can
/// fall back to its project view: `buckets == 0`, an
/// out-of-range `index`, a missing or unreadable file (the
/// same error tolerance as the panel — `read_wav` failing
/// for any reason, missing file and not-a-WAV alike), or a
/// clip whose file decodes to no samples at all. That last
/// case is deliberately stricter than `audio_edit::peaks`
/// on its own, which would return all-zero buckets for
/// empty audio: here an empty clip has no waveform to show,
/// so it falls back like the other failures.
pub fn clip_peaks(core: &EditorCore, index: usize, buckets: usize) -> Vec<f32> {
    if buckets == 0 {
        return Vec::new();
    }
    let Some(clip) = core.project.audio.get(index) else {
        return Vec::new();
    };
    let Ok(data) = crate::audio_edit::read_wav(&clip.path) else {
        return Vec::new();
    };
    if data.samples.is_empty() {
        return Vec::new();
    }
    crate::audio_edit::peaks(&data, buckets)
}

/// Bucket count [`EditorCore::normalise_all_clips`] measures
/// each clip at. The maximum over the buckets is the clip's
/// true peak at any count — [`crate::audio_edit::peaks`] is
/// an exact max-abs reduction, not an estimate — so the
/// count only decides how much intermediate data is built;
/// 512 is fine-grained without being wasteful.
const NORMALISE_PEAK_BUCKETS: usize = 512;

/// How close a computed gain must sit to the current gain
/// for [`normalised_gain`] to call it unchanged. Gains are
/// shown to two decimals in the status line, so a change
/// smaller than this could never be seen or heard.
const GAIN_CHANGE_EPSILON: f32 = 0.001;

/// Phase J (section 7): the gain that would bring a clip
/// whose (gain-free) sample peak is `peak` onto
/// `target_peak`: `current_gain * target / peak`, sanitised
/// exactly as [`EditorCore::audio_set_gain_at`] sanitises
/// its input — a NaN result falls back to unity and the
/// value is clamped to 0.0..=2.0. Because the scaling is a
/// ratio applied to the gain, a clip at unity gain lands
/// its effective peak exactly on target; a clip with gain
/// `g` lands at `g * target`.
///
/// Returns `None` — the caller must leave the gain
/// untouched — when there is nothing honest to do:
///
/// - `peak` is not positive (a silent clip, or a defensive
///   non-positive value; scaling by it would divide by
///   zero or flip the sign), or
/// - the sanitised result already equals `current_gain`
///   within [`GAIN_CHANGE_EPSILON`]. The clamp makes this
///   bite in both directions: a clip already at the 2.0
///   ceiling whose raw scaling would exceed it needs no
///   write, because the clamped result IS its current
///   gain; a clip whose raw scaling lands past the ceiling
///   still gets `Some(2.0)` — a partial normalisation,
///   as far towards the target as the gain range allows.
///
/// Pure: no files, no state — the decision is unit-testable
/// without fixtures.
pub fn normalised_gain(current_gain: f32, peak: f32, target: f32) -> Option<f32> {
    if !(peak > 0.0) {
        return None;
    }
    let scaled = current_gain * (target / peak);
    let clamped = if scaled.is_nan() {
        1.0
    } else {
        scaled.clamp(0.0, 2.0)
    };
    if (clamped - current_gain).abs() <= GAIN_CHANGE_EPSILON {
        None
    } else {
        Some(clamped)
    }
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

    /// Phase J: normalise EVERY audio clip to a common peak:
    /// scale each clip's gain by `target_peak / peak`, where
    /// `peak` is the clip's own sample peak measured with
    /// [`clip_peaks`] at [`NORMALISE_PEAK_BUCKETS`] buckets
    /// (the gain is not part of the measurement, as there).
    /// The maths — including the 0.0..=2.0 clamp and the
    /// NaN fallback — is [`normalised_gain`]'s, the same
    /// sanitising [`Self::audio_set_gain_at`] applies, but
    /// the writes are made here rather than by delegating
    /// per clip: the setter snapshots and reports once per
    /// call, and a bulk normalise must be ONE undo step.
    ///
    /// The measurement pass is read-only and completes
    /// before anything is written: if at least one clip
    /// changes, exactly one snapshot is taken and every new
    /// gain is applied behind it; if nothing changes, no
    /// snapshot is taken. Clips that cannot be measured —
    /// silent (peak 0), missing or unreadable files, and
    /// clips already on target — are skipped with their
    /// gains untouched.
    ///
    /// Returns the number of clips adjusted.
    pub fn normalise_all_clips(&mut self, target_peak: f32) -> usize {
        let mut changes: Vec<(usize, f32)> = Vec::new();
        for index in 0..self.project.audio.len() {
            let peaks = clip_peaks(self, index, NORMALISE_PEAK_BUCKETS);
            if peaks.is_empty() {
                continue;
            }
            let peak = peaks.iter().copied().fold(0.0_f32, f32::max);
            let current = self.project.audio[index].gain;
            if let Some(gain) = normalised_gain(current, peak, target_peak) {
                changes.push((index, gain));
            }
        }
        if changes.is_empty() {
            self.status =
                format!("No audio clips needed normalising to peak {target_peak:.2}.");
            return 0;
        }
        self.snapshot();
        let adjusted = changes.len();
        for (index, gain) in changes {
            self.project.audio[index].gain = gain;
        }
        self.status = format!("Normalised {adjusted} audio clips to peak {target_peak:.2}.");
        adjusted
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

    // ---- Phase H: indexed DSP effects ----
    //
    // Indexed primaries for the Wave 11–13 effects whose
    // originals (`audio_echo_last` and family) live in
    // `editor.rs` and hardcode the last clip. Each body
    // mirrors its `editor.rs` original step for step — the
    // same `audio_edit` DSP with the same fixed parameters,
    // the same product suffix, the same status text — with
    // the clip number taken from `index`. An out-of-range
    // index is the status-only refusal this file's Phase G
    // family established; no snapshot is taken.

    /// Phase H: apply a feedback echo (0.18 s delay, 0.35
    /// feedback — the fixed parameters of
    /// `EditorCore::audio_echo_last` in `editor.rs`) to
    /// audio clip `index` (0-based). The processed audio is
    /// written beside the source as `<stem>.echo.wav`, the
    /// source file is never overwritten, and only after the
    /// write does the clip point to the new file (behind one
    /// undo snapshot). An out-of-range index only sets the
    /// status.
    pub fn audio_echo_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio echo failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio echo failed: {e}");
                return;
            }
        };
        crate::audio_edit::echo(&mut wav, 0.18, 0.35);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".echo.wav");
        if out == source {
            self.status = "Audio echo failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!("Audio echo failed: could not write {}: {e}", out.display());
            return;
        }
        self.snapshot();
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Echo applied to audio clip {} (0.18s delay, 35% feedback): {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase H: apply a 1200 Hz low-pass filter (the fixed
    /// cutoff of `EditorCore::audio_lowpass_last` in
    /// `editor.rs`) to audio clip `index` (0-based). Same
    /// materialise-and-repoint pattern as
    /// [`Self::audio_echo_at`]; the product is
    /// `<stem>.lowpass.wav`. An out-of-range index only sets
    /// the status.
    pub fn audio_lowpass_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio low-pass failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio low-pass failed: {e}");
                return;
            }
        };
        crate::audio_edit::lowpass(&mut wav, 1200.0);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".lowpass.wav");
        if out == source {
            self.status = "Audio low-pass failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio low-pass failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Low-pass filter (1200 Hz) applied to audio clip {}: {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase H: trim leading and trailing silence (samples
    /// below 0.02) from audio clip `index` (0-based) — the
    /// indexed form of `EditorCore::audio_trim_silence_last`
    /// in `editor.rs`. Same materialise-and-repoint pattern
    /// as [`Self::audio_echo_at`]; the product is
    /// `<stem>.silence.wav`. When the clip has no silence at
    /// either edge there is nothing to write: no file is
    /// created, no snapshot is taken, and the status says
    /// so. An out-of-range index only sets the status.
    pub fn audio_trim_silence_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio trim silence failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio trim silence failed: {e}");
                return;
            }
        };
        let (lead, trail) = crate::audio_edit::trim_silence(&mut wav, 0.02);
        if lead + trail == 0 {
            self.status = format!("No silence to trim from audio clip {}.", index + 1);
            return;
        }
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".silence.wav");
        if out == source {
            self.status = "Audio trim silence failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio trim silence failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Trimmed {} silent frame(s) ({} at the start, {} at the end) from audio clip {}: {}.",
            lead + trail,
            lead,
            trail,
            index + 1,
            out.display()
        );
    }

    /// Phase H: pitch-shift audio clip `index` (0-based) up
    /// 2 semitones — the fixed shift of
    /// `EditorCore::audio_pitch_last` in `editor.rs`, which
    /// takes no parameter, so neither does this primary (a
    /// rate shift: pitch and duration change together, the
    /// stored sample rate is untouched). Same
    /// materialise-and-repoint pattern as
    /// [`Self::audio_echo_at`]; the product is
    /// `<stem>.pitch.wav` and the source file is never
    /// overwritten. An out-of-range index only sets the
    /// status.
    pub fn audio_pitch_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio pitch shift failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio pitch shift failed: {e}");
                return;
            }
        };
        crate::audio_edit::pitch_shift(&mut wav, 2.0);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".pitch.wav");
        if out == source {
            self.status = "Audio pitch shift failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio pitch shift failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Pitch shift (+2 semitones) applied to audio clip {}: {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase H: run a compressor (threshold 0.5, ratio 4.0 —
    /// the fixed parameters of
    /// `EditorCore::audio_compress_last` in `editor.rs`,
    /// which takes no parameters, so neither does this
    /// primary) over audio clip `index` (0-based). Same
    /// materialise-and-repoint pattern as
    /// [`Self::audio_echo_at`]; the product is
    /// `<stem>.comp.wav` and the source file is never
    /// overwritten. An out-of-range index only sets the
    /// status.
    pub fn audio_compress_at(&mut self, index: usize) {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio compress failed: no clip {}.", index + 1);
            return;
        }
        let clip = self.project.audio[index].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio compress failed: {e}");
                return;
            }
        };
        crate::audio_edit::compress(&mut wav, 0.5, 4.0);
        let source = PathBuf::from(&clip.path);
        let out = audio_derived_path(&source, ".comp.wav");
        if out == source {
            self.status = "Audio compress failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio compress failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot();
        self.project.audio[index].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Compressor (threshold 0.5, ratio 4.0) applied to audio clip {}: {}.",
            index + 1,
            out.display()
        );
    }

    /// Phase H: split audio clip `index` (0-based) wherever
    /// silence (below 0.02) lasts at least a quarter second —
    /// the indexed form of
    /// `EditorCore::audio_split_at_silences_last` in
    /// `editor.rs`. Each sounding range is materialised
    /// beside the source as `<stem>.seg1.wav`,
    /// `<stem>.seg2.wav`, ... — a segment clip pointing at
    /// the original file would restart the audio from its
    /// beginning, because `AudioClip` has no source-offset
    /// field. The first segment replaces clip `index` and
    /// the remaining segments are inserted immediately after
    /// it, each starting at the original start plus
    /// `round(range_start_secs * fps)` and keeping the
    /// original clip's gain. One snapshot, taken only after
    /// every segment file is written. Fewer than two ranges
    /// (no qualifying silence) is a no-op: no files are
    /// written and no snapshot is taken.
    ///
    /// Returns the number of clips ADDED to the timeline —
    /// the segment count minus one (the first segment
    /// replaces the split clip rather than adding to it) —
    /// or 0 when nothing was split (refusal or no-op).
    pub fn audio_split_silences_at(&mut self, index: usize) -> usize {
        if self.project.audio.get(index).is_none() {
            self.status = format!("Audio split at silences failed: no clip {}.", index + 1);
            return 0;
        }
        let clip = self.project.audio[index].clone();
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio split at silences failed: {e}");
                return 0;
            }
        };
        let rate = wav.sample_rate;
        let min_silence = (rate as f32 * 0.25) as usize;
        let ranges = crate::audio_edit::split_at_silences(&wav, 0.02, min_silence);
        if ranges.len() < 2 {
            self.status = format!(
                "Audio clip {} has no silence of 0.25s or more to split at.",
                index + 1
            );
            return 0;
        }
        let source = PathBuf::from(&clip.path);
        let ch = wav.channels as usize;
        let fps = self.project.fps.max(1.0);
        let mut segments: Vec<crate::model::AudioClip> =
            Vec::with_capacity(ranges.len());
        for (i, &(start, end)) in ranges.iter().enumerate() {
            let seg = crate::audio_edit::WavData {
                sample_rate: wav.sample_rate,
                channels: wav.channels,
                samples: wav.samples[start * ch..end * ch].to_vec(),
            };
            let out = audio_derived_path(&source, &format!(".seg{}.wav", i + 1));
            if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&seg)) {
                self.status = format!(
                    "Audio split at silences failed: could not write {}: {e}",
                    out.display()
                );
                return 0;
            }
            let start_secs = start as f32 / rate.max(1) as f32;
            let shift = (start_secs * fps).round().max(0.0) as usize;
            segments.push(crate::model::AudioClip {
                path: out.to_string_lossy().into_owned(),
                start_frame: clip.start_frame.saturating_add(shift),
                gain: clip.gain,
            });
        }
        let n = segments.len();
        self.snapshot();
        // The first segment replaces the split clip; the rest
        // are inserted immediately after it, in order (the
        // placement `audio_split` uses for its second half).
        let mut segments = segments.into_iter();
        self.project.audio[index] =
            segments.next().expect("at least two ranges were checked");
        for (j, seg) in segments.enumerate() {
            self.project.audio.insert(index + 1 + j, seg);
        }
        self.status = format!(
            "Audio clip {} split at silences into {n} clips.",
            index + 1
        );
        n - 1
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

    // ---- Phase H: indexed DSP effects ----

    /// Write a mono 8 kHz WAV with exactly the given samples
    /// to `dir/name`. Returns the path as a `String`.
    fn write_samples_wav(dir: &PathBuf, name: &str, samples: Vec<f32>) -> String {
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
    fn echo_at_targets_the_chosen_clip() {
        let dir = temp_dir("echo-at");
        let a = write_samples_wav(&dir, "a.wav", vec![0.5; 4000]);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_echo_at(0);
        // Only clip 1 was repointed, at its own echo product.
        let expected = dir.join("a.echo.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str()]);
        assert_eq!(
            core.status,
            format!("Echo applied to audio clip 1 (0.18s delay, 35% feedback): {expected}.")
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The product really is the echo of clip 1's file:
        // the delay is 0.18 s = 1440 frames at 8 kHz, so
        // frame 2000 gains 35% of frame 560 (still 0.5) and
        // frame 2880 gains 35% of the already-echoed frame
        // 1440 (0.5 + 0.35 * 0.5).
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert!((product.samples[0] - 0.5).abs() < 0.001);
        assert!((product.samples[2000] - 0.675).abs() < 0.001);
        assert!((product.samples[2880] - 0.73625).abs() < 0.001);
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert_eq!(other.samples.len(), 100);
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        let source = crate::audio_edit::read_wav(&a).unwrap();
        assert!((source.samples[2000] - 0.5).abs() < 0.001);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lowpass_at_targets_the_chosen_clip() {
        let dir = temp_dir("lowpass-at");
        // Nyquist alternation: the harshest thing a 1200 Hz
        // one-pole at 8 kHz can be shown to smooth.
        let alternating: Vec<f32> = (0..2000)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let a = write_samples_wav(&dir, "a.wav", alternating);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_lowpass_at(0);
        let expected = dir.join("a.lowpass.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str()]);
        assert_eq!(
            core.status,
            format!("Low-pass filter (1200 Hz) applied to audio clip 1: {expected}.")
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The filter state seeds on the first sample (frame 0
        // passes unchanged), frame 1 moves one alpha towards
        // -0.5, and the steady-state swing is ~0.22, well
        // under the unfiltered 0.5.
        let alpha = 1.0 - (-2.0 * std::f32::consts::PI * 1200.0 / 8000.0).exp();
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert!((product.samples[0] - 0.5).abs() < 0.001);
        assert!((product.samples[1] - (0.5 - alpha)).abs() < 0.002);
        assert!(product.samples[1998].abs() > 0.15);
        assert!(product.samples[1998].abs() < 0.26);
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        let source = crate::audio_edit::read_wav(&a).unwrap();
        assert!((source.samples[1] - (-0.5)).abs() < 0.001);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn trim_silence_at_targets_the_chosen_clip() {
        let dir = temp_dir("trim-at");
        let mut samples = vec![0.0; 1000];
        samples.extend(vec![0.5; 2000]);
        samples.extend(vec![0.0; 1000]);
        let a = write_samples_wav(&dir, "a.wav", samples);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_trim_silence_at(0);
        let expected = dir.join("a.silence.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str()]);
        assert_eq!(
            core.status,
            format!(
                "Trimmed 2000 silent frame(s) (1000 at the start, 1000 at the end) \
                 from audio clip 1: {expected}."
            )
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The product is exactly the sounding middle.
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert_eq!(product.frames(), 2000);
        assert!((product.samples[0] - 0.5).abs() < 0.001);
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        assert_eq!(crate::audio_edit::read_wav(&a).unwrap().frames(), 4000);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn trim_silence_at_noops_when_nothing_to_trim() {
        let dir = temp_dir("trim-noop");
        let a = write_samples_wav(&dir, "a.wav", vec![0.5; 2000]);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_trim_silence_at(0);
        assert_eq!(core.status, "No silence to trim from audio clip 1.");
        assert_eq!(paths(&core), vec![a.as_str(), b.as_str()], "no repoint");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert!(
            !dir.join("a.silence.wav").exists(),
            "no product file is written"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pitch_at_targets_the_chosen_clip() {
        let dir = temp_dir("pitch-at");
        let a = write_samples_wav(&dir, "a.wav", vec![0.5; 8000]);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_pitch_at(0);
        let expected = dir.join("a.pitch.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str()]);
        assert_eq!(
            core.status,
            format!("Pitch shift (+2 semitones) applied to audio clip 1: {expected}.")
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // +2 semitones replays at 2^(2/12) ≈ 1.1225× rate:
        // round(8000 / 1.1225) = 7127 frames, and a constant
        // signal interpolates to itself.
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert_eq!(product.frames(), 7127);
        assert!((product.samples[100] - 0.5).abs() < 0.001);
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        assert_eq!(crate::audio_edit::read_wav(&a).unwrap().frames(), 8000);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn compress_at_targets_the_chosen_clip() {
        let dir = temp_dir("compress-at");
        let a = write_samples_wav(&dir, "a.wav", vec![0.9; 8000]);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 0), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        core.audio_compress_at(0);
        let expected = dir.join("a.comp.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![expected.as_str(), b.as_str()]);
        assert_eq!(
            core.status,
            format!(
                "Compressor (threshold 0.5, ratio 4.0) applied to audio clip 1: {expected}."
            )
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Compression keeps the frame count. The envelope
        // starts at 0, so frame 0 (envelope ≈ 0.106 after one
        // attack step, under the 0.5 threshold) passes at
        // unity; by frame 4000 the envelope has settled at
        // 0.9 and the gain is (0.5 / 0.9)^(1 - 1/4) ≈ 0.6436,
        // for ≈ 0.579 out.
        let product = crate::audio_edit::read_wav(&expected).unwrap();
        assert_eq!(product.frames(), 8000);
        assert!((product.samples[0] - 0.9).abs() < 0.001);
        assert!((product.samples[4000] - 0.579).abs() < 0.005);
        assert!(product.samples[4000] < 0.9, "the loud clip was turned down");
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        let source = crate::audio_edit::read_wav(&a).unwrap();
        assert!((source.samples[4000] - 0.9).abs() < 0.001);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn split_silences_at_targets_the_chosen_clip() {
        let dir = temp_dir("split-at");
        // 0.5 s burst, 0.5 s silence, 0.5 s burst at 8 kHz:
        // the gap clears the 0.25 s minimum, so the sounding
        // ranges are exactly the two bursts.
        let mut samples = vec![0.5; 4000];
        samples.extend(vec![0.0; 4000]);
        samples.extend(vec![0.5; 4000]);
        let a = write_samples_wav(&dir, "a.wav", samples);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 10), clip(&b, 4)]);
        core.project.fps = 30.0;
        core.project.audio[0].gain = 0.8;
        let edits = core.edits_since_autosave();
        let added = core.audio_split_silences_at(0);
        assert_eq!(added, 1, "two segments replace one clip: one added");
        assert_eq!(core.status, "Audio clip 1 split at silences into 2 clips.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The segments land where clip 1 was, in order, and
        // the old clip 2 rides after them untouched.
        let seg1 = dir.join("a.seg1.wav").to_string_lossy().into_owned();
        let seg2 = dir.join("a.seg2.wav").to_string_lossy().into_owned();
        assert_eq!(paths(&core), vec![seg1.as_str(), seg2.as_str(), b.as_str()]);
        // Segment 1 starts where the clip started; segment 2
        // is offset by its range start (8000 frames = 1.0 s
        // = 30 frames at 30 fps). Both keep the split clip's
        // gain; the untouched clip keeps its own.
        assert_eq!(core.project.audio[0].start_frame, 10);
        assert_eq!(core.project.audio[1].start_frame, 40);
        assert_eq!(core.project.audio[2].start_frame, 4);
        assert_eq!(core.project.audio[0].gain, 0.8);
        assert_eq!(core.project.audio[1].gain, 0.8);
        assert_eq!(core.project.audio[2].gain, 1.0);
        for seg in [&seg1, &seg2] {
            let back = crate::audio_edit::read_wav(seg).unwrap();
            assert_eq!(back.frames(), 4000, "each burst is one segment");
        }
        // Clip 2's file and clip 1's source are untouched.
        let other = crate::audio_edit::read_wav(&b).unwrap();
        assert!((other.samples[0] - 0.25).abs() < 0.001);
        assert_eq!(crate::audio_edit::read_wav(&a).unwrap().frames(), 12000);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn split_silences_at_noops_without_a_long_gap() {
        let dir = temp_dir("split-noop");
        let a = write_samples_wav(&dir, "a.wav", vec![0.5; 8000]);
        let b = write_samples_wav(&dir, "b.wav", vec![0.25; 100]);
        let mut core = core_with(vec![clip(&a, 7), clip(&b, 4)]);
        let edits = core.edits_since_autosave();
        let added = core.audio_split_silences_at(0);
        assert_eq!(added, 0);
        assert_eq!(
            core.status,
            "Audio clip 1 has no silence of 0.25s or more to split at."
        );
        assert_eq!(paths(&core), vec![a.as_str(), b.as_str()], "no repoint");
        assert_eq!(core.project.audio.len(), 2);
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert!(!dir.join("a.seg1.wav").exists(), "no segment files written");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dsp_ops_at_out_of_range_are_status_only() {
        // The refusal precedes any file access, so fake paths
        // are fine here.
        let mut core = core_with(vec![clip("a.wav", 0), clip("b.wav", 4)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.audio_echo_at(2);
        assert_eq!(core.status, "Audio echo failed: no clip 3.");
        core.audio_lowpass_at(2);
        assert_eq!(core.status, "Audio low-pass failed: no clip 3.");
        core.audio_trim_silence_at(2);
        assert_eq!(core.status, "Audio trim silence failed: no clip 3.");
        core.audio_pitch_at(2);
        assert_eq!(core.status, "Audio pitch shift failed: no clip 3.");
        core.audio_compress_at(2);
        assert_eq!(core.status, "Audio compress failed: no clip 3.");
        assert_eq!(core.audio_split_silences_at(2), 0);
        assert_eq!(core.status, "Audio split at silences failed: no clip 3.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // Same refusal voice on an empty project.
        let mut empty = EditorCore::new();
        empty.audio_echo_at(0);
        assert_eq!(empty.status, "Audio echo failed: no clip 1.");
        assert_eq!(empty.audio_split_silences_at(0), 0);
        assert_eq!(
            empty.status,
            "Audio split at silences failed: no clip 1."
        );
    }

    // ---- Phase I: per-clip waveform peaks ----

    #[test]
    fn clip_peaks_reports_the_loud_second_half() {
        let dir = temp_dir("peaks-loud-half");
        // One second at 8 kHz: a quiet first half at 0.1 and
        // a loud second half at -0.8 — negative on purpose,
        // so the test proves the peaks are max-ABS, matching
        // `audio_edit::peaks`, not max-signed.
        let mut samples = vec![0.1; 4000];
        samples.extend(vec![-0.8; 4000]);
        let a = write_samples_wav(&dir, "a.wav", samples);
        let mut core = core_with(vec![clip(&a, 0)]);
        core.status = "untouched".into();
        let edits = core.edits_since_autosave();
        // Four buckets of 2000 frames each: two quiet, two loud.
        let peaks = clip_peaks(&core, 0, 4);
        assert_eq!(peaks.len(), 4);
        let expected = [0.1, 0.1, 0.8, 0.8];
        for (got, want) in peaks.iter().zip(expected) {
            assert!((got - want).abs() < 0.001, "peak {got}, expected {want}");
            assert!((0.0..=1.0).contains(got), "peak {got} outside 0.0..=1.0");
        }
        // Two buckets collapse to one peak per half.
        let peaks = clip_peaks(&core, 0, 2);
        assert_eq!(peaks.len(), 2);
        assert!((peaks[0] - 0.1).abs() < 0.001);
        assert!((peaks[1] - 0.8).abs() < 0.001);
        // Pure and read-only: no status write, no snapshot.
        assert_eq!(core.status, "untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clip_peaks_uses_the_indexed_clips_own_samples() {
        let dir = temp_dir("peaks-indexed");
        let quiet = write_samples_wav(&dir, "quiet.wav", vec![0.2; 4000]);
        let loud = write_samples_wav(&dir, "loud.wav", vec![0.9; 4000]);
        let core = core_with(vec![clip(&quiet, 0), clip(&loud, 4)]);
        // Each index peaks from its own file, not the last
        // clip's — the whole point of the indexed helper.
        let first = clip_peaks(&core, 0, 2);
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|&v| (v - 0.2).abs() < 0.001));
        let second = clip_peaks(&core, 1, 2);
        assert_eq!(second.len(), 2);
        assert!(second.iter().all(|&v| (v - 0.9).abs() < 0.001));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clip_peaks_empty_for_out_of_range_and_zero_buckets() {
        let dir = temp_dir("peaks-refusals");
        let a = write_samples_wav(&dir, "a.wav", vec![0.5; 4000]);
        let core = core_with(vec![clip(&a, 0)]);
        assert!(clip_peaks(&core, 1, 8).is_empty(), "index past the end");
        assert!(clip_peaks(&core, 99, 8).is_empty(), "index far past the end");
        assert!(clip_peaks(&core, 0, 0).is_empty(), "zero buckets");
        // No clips at all: even index 0 is out of range.
        let empty = EditorCore::new();
        assert!(clip_peaks(&empty, 0, 8).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clip_peaks_empty_for_missing_or_unreadable_file() {
        let dir = temp_dir("peaks-unreadable");
        // A path that was never written.
        let missing = dir.join("missing.wav").to_string_lossy().into_owned();
        let core = core_with(vec![clip(&missing, 0)]);
        assert!(clip_peaks(&core, 0, 8).is_empty(), "missing file");
        // A file that exists but is not a WAV at all: the
        // same `read_wav` error tolerance the panel has.
        let junk = dir.join("junk.wav");
        std::fs::write(&junk, b"this is not a wav file").unwrap();
        let core = core_with(vec![clip(&junk.to_string_lossy(), 0)]);
        assert!(clip_peaks(&core, 0, 8).is_empty(), "unreadable file");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clip_peaks_empty_for_a_clip_with_no_samples() {
        let dir = temp_dir("peaks-empty");
        // A well-formed WAV with an empty data chunk decodes
        // successfully but has no samples: it reads as no
        // waveform (an empty vec), NOT as all-zero buckets.
        let a = write_samples_wav(&dir, "a.wav", Vec::new());
        let core = core_with(vec![clip(&a, 0)]);
        assert!(clip_peaks(&core, 0, 8).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- Phase J: normalise all clips ----

    #[test]
    fn normalised_gain_table() {
        // (current gain, peak, target, expected) — the pure
        // decision behind `normalise_all_clips`, no files
        // involved. `Some` values compare within 0.001.
        let cases: [(f32, f32, f32, Option<f32>); 8] = [
            // A silent clip has no peak to scale by.
            (1.0, 0.0, 0.9, None),
            // A non-positive peak is refused defensively:
            // real peaks are max-abs, so this only guards
            // the division.
            (1.0, -0.5, 0.9, None),
            // Already on target: the ratio is 1.
            (1.0, 0.9, 0.9, None),
            // Within the epsilon of no change (the scaled
            // gain would be 0.9995, 0.0005 away).
            (1.0, 1.0, 0.9995, None),
            // A quiet clip is boosted towards the target.
            (1.0, 0.5, 0.9, Some(1.8)),
            // A loud clip is reduced towards the target.
            (1.5, 0.9, 0.45, Some(0.75)),
            // Clamp interaction, partial: the raw scaling is
            // 6.0, past the gain ceiling, so the clip lands
            // on 2.0 — as far towards the target as the
            // 0.0..=2.0 range allows, short of the peak the
            // target asked for.
            (1.5, 0.25, 1.0, Some(2.0)),
            // Clamp interaction, no-op: the raw scaling is
            // 8.0, but clamped it equals the current gain,
            // so there is nothing to write.
            (2.0, 0.25, 1.0, None),
        ];
        for (current, peak, target, expected) in cases {
            let got = normalised_gain(current, peak, target);
            match (got, expected) {
                (None, None) => {}
                (Some(g), Some(want)) => assert!(
                    (g - want).abs() < 0.001,
                    "normalised_gain({current}, {peak}, {target}) = {g}, expected {want}"
                ),
                _ => panic!(
                    "normalised_gain({current}, {peak}, {target}) = {got:?}, expected {expected:?}"
                ),
            }
        }
    }

    #[test]
    fn normalised_gain_scales_from_the_current_gain() {
        // The ratio multiplies the CURRENT gain rather than
        // replacing it: the same file peak and target scale
        // two different gains by the same factor, so clips
        // keep their relative balance and only a clip at
        // unity lands its effective peak exactly on target.
        assert_eq!(normalised_gain(1.0, 0.5, 0.5), None, "ratio 1 at unity");
        assert_eq!(normalised_gain(2.0, 0.5, 0.5), None, "ratio 1 at any gain");
        let halved = normalised_gain(1.0, 0.5, 0.25).unwrap();
        assert!((halved - 0.5).abs() < 0.001, "1.0 * (0.25 / 0.5) = 0.5");
        let halved_too = normalised_gain(0.5, 0.5, 0.25).unwrap();
        assert!(
            (halved_too - 0.25).abs() < 0.001,
            "0.5 * (0.25 / 0.5) = 0.25 — the same factor, half the result"
        );
    }

    #[test]
    fn normalise_all_clips_scales_each_gain_once() {
        let dir = temp_dir("normalise-all");
        // Constant-amplitude files whose peaks survive the
        // PCM16 round trip exactly: 0.25, 0.75 and 0.5.
        let quiet = write_samples_wav(&dir, "quiet.wav", vec![0.25; 2000]);
        let loud = write_samples_wav(&dir, "loud.wav", vec![0.75; 2000]);
        let settled = write_samples_wav(&dir, "settled.wav", vec![0.5; 2000]);
        let silent = write_samples_wav(&dir, "silent.wav", vec![0.0; 2000]);
        let missing = dir.join("missing.wav").to_string_lossy().into_owned();
        let mut core = core_with(vec![
            clip(&quiet, 0),
            clip(&loud, 4),
            clip(&settled, 8),
            clip(&silent, 12),
            clip(&missing, 16),
        ]);
        core.project.audio[0].gain = 0.8;
        core.project.audio[3].gain = 0.7;
        core.project.audio[4].gain = 0.3;
        let edits = core.edits_since_autosave();
        let adjusted = core.normalise_all_clips(0.5);
        // The quiet clip is boosted (0.8 * 0.5 / 0.25 = 1.6)
        // and the loud one cut (1.0 * 0.5 / 0.75 = 0.667);
        // the settled clip already peaks on target, and the
        // silent and missing clips cannot be measured — all
        // three keep their gains.
        assert_eq!(adjusted, 2);
        assert!((core.project.audio[0].gain - 1.6).abs() < 0.001);
        assert!((core.project.audio[1].gain - 0.6667).abs() < 0.002);
        assert_eq!(core.project.audio[2].gain, 1.0, "already on target");
        assert_eq!(core.project.audio[3].gain, 0.7, "silent clip skipped");
        assert_eq!(core.project.audio[4].gain, 0.3, "missing file skipped");
        assert_eq!(core.status, "Normalised 2 audio clips to peak 0.50.");
        // The whole operation is ONE undo step.
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        core.undo();
        assert_eq!(core.project.audio[0].gain, 0.8, "undo restores the boost");
        assert_eq!(core.project.audio[1].gain, 1.0, "undo restores the cut");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn normalise_all_clips_noops_when_nothing_needs_changing() {
        let dir = temp_dir("normalise-noop");
        let settled = write_samples_wav(&dir, "settled.wav", vec![0.5; 2000]);
        let silent = write_samples_wav(&dir, "silent.wav", vec![0.0; 2000]);
        let missing = dir.join("missing.wav").to_string_lossy().into_owned();
        let mut core = core_with(vec![
            clip(&settled, 0),
            clip(&silent, 4),
            clip(&missing, 8),
        ]);
        core.project.audio[1].gain = 0.7;
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        let adjusted = core.normalise_all_clips(0.5);
        assert_eq!(adjusted, 0);
        assert_eq!(
            core.status,
            "No audio clips needed normalising to peak 0.50."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // An empty project is the same nothing-to-do.
        let mut empty = EditorCore::new();
        assert_eq!(empty.normalise_all_clips(0.9), 0);
        assert_eq!(
            empty.status,
            "No audio clips needed normalising to peak 0.90."
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
