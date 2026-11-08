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
//! The `EditorCore` methods the Rune shell needs —
//! [`EditorCore::start_playback_audio`],
//! [`EditorCore::stop_playback_audio`],
//! [`EditorCore::sync_playback_audio_on_wrap`] and
//! [`EditorCore::playback_audio_active`] — delegating to that
//! same player and mix. Start uses the GPUI shell's exact
//! playhead convention: `current` frame over `fps` (floored at
//! 1), so the audio starts at the frame the user sees selected.
//! The sync method closes the loop-wrap limit (Phase G): when
//! the shell's per-tick frame tracking shows the playhead
//! wrapped while playing, it restarts the audio from the
//! playhead, so each visual loop gets its own audio pass.
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
//! (`audio.rs` documents this), so within one pass the audio
//! and the picture can drift apart by however much their
//! clocks disagree.
//!
//! The loop limit is closed for the wrap case:
//! [`EditorCore::sync_playback_audio_on_wrap`] restarts the
//! audio whenever the visual playhead wraps — at the end of
//! the timeline, or at the end of a loop range (the restart
//! starts from the wrapped-to frame, so a range that begins
//! mid-timeline restarts its audio at the range start, which
//! is where the picture is). Two things remain open, plainly:
//!
//! - **No scrub-follow.** Seeking or scrubbing while playing
//!   does not re-sync the audio; only pressing Play and the
//!   loop wrap restart it. A seek leaves the audio playing
//!   its original course until it ends or playback stops.
//! - **A wrap inside a single tick can be missed.** The
//!   detection compares whole frames between ticks; a tick
//!   that advanced the playhead by a full loop or more and
//!   landed on a frame at or after the previous one is
//!   indistinguishable from ordinary forward motion. At the
//!   shell's per-frame tick rate this needs a stall of a
//!   whole loop's duration, but it is not impossible.

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

/// Did the playhead wrap between two poll ticks? Pure decision
/// behind [`EditorCore::sync_playback_audio_on_wrap`].
///
/// The core's wrap semantics (`advance_play` in `editor.rs`):
/// the fractional playhead advances monotonically and is folded
/// back by the timeline length — or, with a loop range set, by
/// the range's span — so the integer `current` frame it lands
/// on *decreases* exactly when a wrap happened: from the last
/// frame (or the range end) back to frame 0 (or the range
/// start). A decrease while playing is therefore the wrap
/// signal, and nothing else in normal playback produces one —
/// forward motion never lowers `current`, and a paused playhead
/// does not move at all.
///
/// The guards keep stale or degenerate inputs from reading as
/// a wrap: both ticks must be playing (a frame drop across a
/// pause is a seek the user made while stopped, and Play
/// already restarts the audio for it), and the timeline needs
/// at least two frames — with zero frames `advance_play` never
/// runs, and with one frame `current` is always 0, so no
/// decrease can occur.
fn playhead_wrapped(
    was_playing: bool,
    now_playing: bool,
    prev_frame: usize,
    cur_frame: usize,
    frame_count: usize,
) -> bool {
    was_playing && now_playing && frame_count > 1 && cur_frame < prev_frame
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

    /// Re-sync the audio after a loop wrap: if the playhead
    /// wrapped between the shell's previous poll tick
    /// (`prev_frame`) and this one (`cur_frame`) while playing
    /// — the decision lives in `playhead_wrapped` — and sound
    /// is actually running, restart it from the top of the
    /// wrapped-to position via stop + start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame (frame 0 after a full loop,
    /// the range start after a loop-range wrap), succeeds
    /// silently when nothing is audible there, and reports a
    /// player launch failure on `core.status`.
    ///
    /// When there was no wrap, or no audio is active (nothing
    /// audible, or the player already exited), this is a
    /// no-op and writes nothing to the status line. The shell
    /// calls it every poll tick with the frame it saw last
    /// tick and `self.current`; the per-tick cost when nothing
    /// wraps is the comparison plus one process-liveness
    /// check.
    ///
    /// Both of the helper's playing flags are this core's
    /// current `playing`: the call happens inside the same
    /// tick that advanced the playhead, so the state that
    /// produced the motion and the state now are one. The
    /// two-flag guard still earns its keep at this level —
    /// on a stopped tick `playing` is false and nothing is
    /// considered, and a seek made while stopped is followed
    /// by Play's own fresh start, so a redundant restart on
    /// the first playing tick after one would start from the
    /// same current frame anyway.
    pub fn sync_playback_audio_on_wrap(&mut self, prev_frame: usize, cur_frame: usize) {
        let wrapped = playhead_wrapped(
            self.playing,
            self.playing,
            prev_frame,
            cur_frame,
            self.project.frames.len(),
        );
        if wrapped && self.playback_audio_active() {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
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

    #[test]
    fn wrapped_detects_the_end_of_timeline_wrap() {
        // Last frame → frame 0, the `advance_play` no-range
        // fold (playhead crossing the frame count).
        assert!(playhead_wrapped(true, true, 9, 0, 10));
        assert!(playhead_wrapped(true, true, 1, 0, 2));
        // A loop-range fold lands on the range start, not
        // necessarily 0 — still a decrease, still a wrap.
        assert!(playhead_wrapped(true, true, 7, 3, 10));
    }

    #[test]
    fn wrapped_ignores_ordinary_forward_motion() {
        assert!(!playhead_wrapped(true, true, 3, 4, 10));
        // Holding on the same frame (a long-duration frame the
        // playhead has not crossed yet) is not a wrap.
        assert!(!playhead_wrapped(true, true, 4, 4, 10));
        assert!(!playhead_wrapped(true, true, 0, 0, 10));
        assert!(!playhead_wrapped(true, true, 0, 9, 10));
    }

    #[test]
    fn wrapped_requires_playing_on_both_ticks() {
        // Paused now: the playhead did not move this tick, so
        // any decrease is a seek made while stopped — Play
        // restarts the audio for those, not the wrap sync.
        assert!(!playhead_wrapped(true, false, 9, 0, 10));
        // Not playing last tick either (the pass just started;
        // start_playback_audio already fired for it).
        assert!(!playhead_wrapped(false, true, 9, 0, 10));
        assert!(!playhead_wrapped(false, false, 9, 0, 10));
    }

    #[test]
    fn wrapped_needs_at_least_two_frames() {
        // One frame: `current` is always 0, no decrease can
        // occur — and stale indices must not read as one.
        assert!(!playhead_wrapped(true, true, 0, 0, 1));
        assert!(!playhead_wrapped(true, true, 3, 0, 1));
        // Zero frames: advance_play returns before moving.
        assert!(!playhead_wrapped(true, true, 0, 0, 0));
        assert!(!playhead_wrapped(true, true, 3, 0, 0));
    }

    #[test]
    fn sync_without_audio_is_a_silent_no_op() {
        // No clips → the player is never active, so even a
        // textbook wrap shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.playing = true;
        let last = core.frame_count() - 1;
        assert!(last > 0, "demo project should have several frames");
        core.current = 0;
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_wrap(last, core.current);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Ordinary advance and a paused wrap shape alike.
        core.sync_playback_audio_on_wrap(0, 1);
        core.playing = false;
        core.sync_playback_audio_on_wrap(last, 0);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
    }
}
