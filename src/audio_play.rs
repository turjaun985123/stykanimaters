//! Playback-audio glue between [`EditorCore`] and the engine's
//! existing audio player (Rune Phase F).
//!
//! ## What already existed
//!
//! The engine has had a real output player since Wave 10:
//! [`crate::audio::AudioPlayer`] spawns the system `ffplay`
//! (which ships with the ffmpeg the exporter already depends
//! on) and its `play_project` entry point plays *every* clip
//! audible at a playhead, rendering a mix with
//! [`crate::audio_edit::mix`] when more than one clip sounds —
//! the same mixer the MP4 export muxes with (`export.rs` calls
//! `audio_edit::mix` for its audio track). What the Rune
//! migration removed was not the player but its *owner*: in the
//! GPUI shell the app state held the `AudioPlayer` and its
//! `toggle_play` called `play_project` on Play and `stop` when
//! playback ended. Phase B deleted that shell, and no Rune code
//! has called the player since — which is why pressing Play in
//! the Rune editor has been silent.
//!
//! ## What this module adds
//!
//! The three `EditorCore` methods the Rune shell needs —
//! [`EditorCore::start_playback_audio`],
//! [`EditorCore::stop_playback_audio`] and
//! [`EditorCore::playback_audio_active`] — delegating to that
//! same player and mix. Start uses the GPUI shell's exact
//! playhead convention: `current` frame over `fps` (floored at
//! 1), so the audio starts at the frame the user sees selected.
//!
//! ## Where the player lives, and why
//!
//! `EditorCore` cannot grow a field from this file (the struct
//! is defined in `editor.rs`), and a process handle would not
//! belong in the core's snapshot model anyway, so the player
//! lives in a thread-local beside the glue — the same ownership
//! shape as the GPUI shell's app state, one level down. The
//! editor is a single-threaded UI, so the UI thread's player is
//! the only one the shell can ever reach; as a side benefit,
//! each unit-test thread gets an isolated player. The player's
//! own `Drop` stops its `ffplay` process, including when a
//! thread exits.
//!
//! ## Honest limits (inherited, not new)
//!
//! The player does not synchronise against a sample clock
//! (`audio.rs` documents this): if the visual playhead loops,
//! the audio does not loop with it — it plays the audible span
//! once from the start position and ends. Seeking or scrubbing
//! while playing does not move the audio. The GPUI shell had
//! exactly these limits; closing them needs a clock-synced
//! player, which is a different piece of work.

use std::cell::RefCell;

use crate::audio::AudioPlayer;
use crate::editor::EditorCore;

thread_local! {
    /// The one playback player for this thread (see the module
    /// docs for why it lives here rather than in `EditorCore`).
    static PLAYER: RefCell<AudioPlayer> = RefCell::new(AudioPlayer::new());
}

/// The playhead position `start_playback_audio` starts from, in
/// seconds: the current frame over the project frame rate, with
/// the rate floored at 1 fps — the GPUI shell's formula, and the
/// same defensive floor the MP4 audio mux uses
/// (`audio::clip_offset_secs`).
fn playhead_secs(core: &EditorCore) -> f32 {
    core.current as f32 / core.project.fps.max(1.0)
}

impl EditorCore {
    /// Start hearing the project: every audio clip audible at
    /// the current frame is played together through
    /// [`AudioPlayer::play_project`] — a lone unity-gain clip
    /// plays directly, anything else is rendered through
    /// [`crate::audio_edit::mix`] first (the MP4 export's mix).
    ///
    /// Any sound a previous start left running is replaced
    /// (`play_project` stops first). When nothing is audible at
    /// the playhead — including a project with no clips, or
    /// clips whose files are missing — this succeeds silently
    /// and [`EditorCore::playback_audio_active`] stays false.
    /// A failure to launch the player is reported on
    /// `core.status`; success writes nothing there.
    pub fn start_playback_audio(&mut self) {
        let fps = self.project.fps.max(1.0);
        let result = PLAYER.with(|player| {
            player
                .borrow_mut()
                .play_project(&self.project.audio, fps, playhead_secs(self))
        });
        if let Err(error) = result {
            self.status = format!("Audio playback failed: {error}.");
        }
    }

    /// Stop any sound a previous
    /// [`EditorCore::start_playback_audio`] started. Safe to
    /// call when nothing is playing.
    pub fn stop_playback_audio(&mut self) {
        PLAYER.with(|player| player.borrow_mut().stop());
    }

    /// True while the player's `ffplay` process is still
    /// running. Note this is process liveness, not audibility:
    /// on a machine with no audio device the process may exit
    /// immediately after a successful start, and this returns
    /// false even though the start itself succeeded.
    pub fn playback_audio_active(&self) -> bool {
        PLAYER.with(|player| player.borrow_mut().is_playing())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{ffplay_available, playback_plan, PlaybackPlan};
    use crate::audio_edit::{mix, read_wav, write_wav, WavData};
    use crate::model::AudioClip;

    /// Write a mono WAV of constant `amplitude`, `secs` long at
    /// 8 kHz, to a unique temp path and return the path.
    fn const_wav_path(tag: &str, amplitude: f32, secs: f32) -> String {
        let wav = WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: vec![amplitude; (8_000.0 * secs) as usize],
        };
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rs-audio-play-{tag}-{}-{unique}.wav",
            std::process::id()
        ));
        std::fs::write(&path, write_wav(&wav)).expect("write temp WAV fixture");
        path.to_string_lossy().into_owned()
    }

    fn clip(path: &str, start_frame: usize) -> AudioClip {
        AudioClip {
            path: path.to_string(),
            start_frame,
            gain: 1.0,
        }
    }

    #[test]
    fn playhead_secs_uses_current_frame_over_fps() {
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.current = 48;
        assert_eq!(playhead_secs(&core), 2.0);
        core.current = 0;
        assert_eq!(playhead_secs(&core), 0.0);
        // Non-positive rates floor at 1 fps rather than dividing
        // by zero (the mux's convention).
        core.project.fps = 0.0;
        core.current = 48;
        assert_eq!(playhead_secs(&core), 48.0);
    }

    #[test]
    fn start_with_no_clips_is_silent_and_inactive() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.status = "Sentinel.".into();
        core.start_playback_audio();
        // The success path writes nothing to the status line,
        // and with nothing audible no process is running.
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Stop is safe and leaves the flag down.
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn start_with_missing_clip_files_is_silent_not_an_error() {
        // The plan probe cannot resolve a duration for a missing
        // file, so the clip is excluded and the plan is Silent —
        // the same degradation `play_project` documents.
        let mut core = EditorCore::new();
        core.project.audio = vec![clip("/nonexistent/stykanimate-nope.wav", 0)];
        core.status = "Sentinel.".into();
        core.start_playback_audio();
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn mix_at_a_playhead_contains_each_clip_at_its_offset() {
        // The exact composition `start_playback_audio` triggers
        // through `play_project`: `playback_plan` at the
        // playhead, then `audio_edit::mix` (the export's mixer).
        // Clip A (0.5) starts at frame 0; clip B (0.25) at frame
        // 48 — 2.0s in at 24 fps. Both files last 4s.
        let a = const_wav_path("mix-a", 0.5, 4.0);
        let b = const_wav_path("mix-b", 0.25, 4.0);
        let clips = vec![clip(&a, 0), clip(&b, 48)];
        let durations = |path: &str| read_wav(path).ok().map(|w| w.duration_secs());
        match playback_plan(&clips, 3.0, 24.0, &durations) {
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => {
                assert_eq!(offset_secs, 3.0);
                let mixed = mix(&mix_clips).expect("mix renders");
                assert_eq!(mixed.sample_rate, 8_000);
                // Latest end wins: B ends at 2.0 + 4.0 = 6.0s.
                assert_eq!(mixed.samples.len(), 48_000);
                let at = |t: f32| mixed.samples[(t * 8_000.0) as usize + 4_000];
                // 1.0s: only A sounds.
                assert!((at(1.0) - 0.5).abs() < 1e-3, "at 1.0s: {}", at(1.0));
                // 3.0s (the playhead): A + B summed.
                assert!((at(3.0) - 0.75).abs() < 1e-3, "at 3.0s: {}", at(3.0));
                // 5.0s: A has ended, only B remains.
                assert!((at(5.0) - 0.25).abs() < 1e-3, "at 5.0s: {}", at(5.0));
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn start_and_stop_with_a_real_clip() {
        if !ffplay_available() {
            eprintln!("skipping real-clip playback test: ffplay is not available");
            return;
        }
        // Silence, so the test makes no sound on machines that
        // do have an audio device; 2s at unity gain takes the
        // player's Direct path.
        let path = const_wav_path("real", 0.0, 2.0);
        let mut core = EditorCore::new();
        core.project.audio = vec![clip(&path, 0)];
        core.current = 0;
        core.start_playback_audio();
        // ffplay may exit at once on a device-less machine, so
        // liveness here is not asserted — the portable facts are
        // that the start reported no failure and that stop
        // leaves the player inactive.
        assert!(
            !core.status.starts_with("Audio playback failed"),
            "unexpected status: {}",
            core.status
        );
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
        let _ = std::fs::remove_file(&path);
    }
}
