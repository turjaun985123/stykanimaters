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
//! [`EditorCore::sync_playback_audio_on_wrap`],
//! [`EditorCore::sync_playback_audio_on_seek`],
//! [`EditorCore::sync_playback_audio_on_drift`] and
//! [`EditorCore::playback_audio_active`] — delegating to that
//! same player and mix. Start uses the GPUI shell's exact
//! playhead convention: `current` frame over `fps` (floored at
//! 1), so the audio starts at the frame the user sees selected.
//! The wrap sync closes the loop-wrap limit (Phase G): when
//! the shell's per-tick frame tracking shows the playhead
//! wrapped while playing, it restarts the audio from the
//! playhead, so each visual loop gets its own audio pass. The
//! seek sync closes the scrub-follow limit (Phase I): when
//! the tracking shows the playhead jumping forward while
//! playing, it restarts the audio from the new playhead, so a
//! seek is heard from where the picture landed. The drift
//! sync (Phase J) bounds the last gap — the missing
//! sample-clock sync: every start records a playback clock
//! (wall-clock instant + the frame the sound started at),
//! and when the shell's fractional playhead has wandered
//! further from the clock's prediction than
//! [`DRIFT_TOLERANCE_FRAMES`], the mix restarts at the
//! playhead, re-basing the clock.
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
//! (`audio.rs` documents this). Phase J's drift sync bounds
//! what that costs: within one pass the audio and the picture
//! can still drift apart, but only until the gap exceeds
//! [`DRIFT_TOLERANCE_FRAMES`] (2.5 frames), at which point
//! [`EditorCore::sync_playback_audio_on_drift`] restarts the
//! mix at the playhead. The correction is a restart, not a
//! nudge — a listener may hear the seam — and two things
//! about it are approximate by construction:
//!
//! - **The clock's prediction ignores per-frame durations.**
//!   The expected frame advances at the project's `fps`,
//!   while `advance_play` divides by the current frame's
//!   duration multiplier, so during a frame whose duration
//!   is not 1.0 the playhead legitimately moves at a
//!   different rate from real time while the audio does
//!   not wait. A long slow frame can therefore trip the
//!   tolerance on its own; the restart then re-bases both
//!   clocks at the frame the user is looking at, which is
//!   the least-wrong anchor available.
//! - **The prediction is not folded.** The expected frame
//!   counts up from the start of the current pass without
//!   wrapping; the wrap and seek syncs restart (and so
//!   re-base the clock) on the same discontinuities that
//!   fold the playhead, so within a pass the two stay
//!   comparable.
//!
//! The loop limit is closed for the wrap case:
//! [`EditorCore::sync_playback_audio_on_wrap`] restarts the
//! audio whenever the visual playhead wraps — at the end of
//! the timeline, or at the end of a loop range (the restart
//! starts from the wrapped-to frame, so a range that begins
//! mid-timeline restarts its audio at the range start, which
//! is where the picture is).
//!
//! The scrub-follow limit is closed in both directions
//! (Phase I): a backward seek while playing lowers `current`,
//! which the wrap restart already reads as its signal, and
//! [`EditorCore::sync_playback_audio_on_seek`] restarts the
//! audio whenever a tick shows the playhead jumping forward
//! by more than one frame. The shell's poll calls the two
//! syncs side by side each tick (integrator wiring). One
//! thing remains open, plainly:
//!
//! - **A wrap inside a single tick can still be missed in
//!   one narrow shape.** The detection compares whole frames
//!   between ticks; a tick that advanced the playhead by a
//!   full loop or more and landed on the previous frame or
//!   the one after it is indistinguishable from holding or
//!   ordinary forward motion, so no restart fires. Landing
//!   further ahead is caught by the seek restart instead —
//!   it restarts from the landed frame either way, which is
//!   where the picture is. At the shell's per-frame tick
//!   rate this needs a stall of a whole loop's duration,
//!   but it is not impossible.

use std::cell::RefCell;
use std::time::Instant;

use crate::audio::AudioPlayer;
use crate::editor::EditorCore;

thread_local! {
    /// The one playback player for this thread (see the module
    /// docs for why it lives here rather than in `EditorCore`).
    static PLAYER: RefCell<AudioPlayer> = RefCell::new(AudioPlayer::new());

    /// The playback clock for the current pass: the wall-clock
    /// instant the running sound started, and the frame it
    /// started at. Written by
    /// [`EditorCore::start_playback_audio`] on every successful
    /// start (so the wrap, seek and drift restarts — all of
    /// which funnel through stop + start — re-base it), and
    /// cleared by [`EditorCore::stop_playback_audio`]. It lives
    /// beside the player for the same reason the player does:
    /// it is process state, not project state, and does not
    /// belong in the core's snapshot model.
    static CLOCK: RefCell<Option<(Instant, f32)>> = const { RefCell::new(None) };
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

/// Did the playhead jump forward between two poll ticks? Pure
/// decision behind [`EditorCore::sync_playback_audio_on_seek`].
///
/// Normal playback advances `current` by exactly one frame
/// per tick (or holds it, on a long-duration frame), so a
/// tick that lands more than one frame ahead is a
/// discontinuity: the user seeked or scrubbed forward, and
/// the audio — still playing its original course — no longer
/// matches the picture. Backward motion is deliberately not
/// this function's business: a decrease is the wrap signal
/// `playhead_wrapped` reads, and a backward seek while
/// playing produces exactly that decrease, so the wrap
/// restart already re-syncs it.
///
/// A stall counts too: a tick delayed long enough for the
/// playhead to skip two or more frames reads as a seek. That
/// is desirable, not a false positive — the two clocks were
/// free to drift across the stall (there is no sample-clock
/// sync; see the module docs), and restarting re-anchors the
/// audio to where the picture actually landed.
pub fn playhead_seeked(prev_frame: usize, cur_frame: usize) -> bool {
    cur_frame > prev_frame && cur_frame - prev_frame > 1
}

/// How far apart, in frames, the picture and the sound may
/// drift before [`EditorCore::sync_playback_audio_on_drift`]
/// restarts the mix. Two and a half frames is a little under
/// a tenth of a second at the rates this editor targets —
/// large enough that ordinary scheduling jitter between the
/// shell's poll ticks and the player's process never trips
/// it, small enough that a listener is unlikely to notice
/// lip-sync error before the correction lands.
pub const DRIFT_TOLERANCE_FRAMES: f32 = 2.5;

/// The signed drift between where the playback clock says
/// the sound should be (`expected`) and where the picture
/// actually is (`actual`), in frames: positive when the
/// playhead is ahead of the sound, negative when it trails
/// it. Pure arithmetic behind
/// [`EditorCore::sync_playback_audio_on_drift`].
pub fn drift_frames(expected: f32, actual: f32) -> f32 {
    actual - expected
}

/// Has the drift grown past [`DRIFT_TOLERANCE_FRAMES`] in
/// either direction? The comparison is strict: a drift of
/// exactly the tolerance is still within bounds.
pub fn drift_exceeded(expected: f32, actual: f32) -> bool {
    drift_frames(expected, actual).abs() > DRIFT_TOLERANCE_FRAMES
}

/// Where the playback clock predicts the playhead should be
/// `elapsed_secs` after a pass started at `start_frame`: the
/// start frame plus elapsed time at the project rate, the
/// rate floored at 1 fps exactly as `playhead_secs` floors
/// it. The prediction is deliberately not folded by the
/// timeline length or a loop range — restarts re-base the
/// clock on those discontinuities (see the module docs), so
/// within one pass a plain count-up is the honest shape.
fn expected_frame(start_frame: f32, elapsed_secs: f32, fps: f32) -> f32 {
    start_frame + elapsed_secs * fps.max(1.0)
}

/// The drift restart decision, pure: restart only while
/// playing, only while sound is actually running, and only
/// when a playback clock exists to predict from and its
/// prediction has been exceeded. `expected` is `None` when
/// no start has recorded a clock (or a stop cleared it).
fn drift_restart_needed(
    playing: bool,
    audio_active: bool,
    expected: Option<f32>,
    actual: f32,
) -> bool {
    playing
        && audio_active
        && expected.is_some_and(|expected| drift_exceeded(expected, actual))
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
        } else {
            // Re-base the playback clock. The sound starts at
            // the integer `current` frame (`playhead_secs`'
            // convention), so that — not the fractional
            // playhead, which may sit part-way through the
            // frame — is the frame the clock counts from.
            // This is the only start path: the wrap, seek and
            // drift syncs all restart through stop + start,
            // so every pass re-bases here. A start where
            // nothing is audible also records a clock, but it
            // can do no harm: the drift sync checks
            // `playback_audio_active` before it ever reads
            // the clock, and the next start overwrites it.
            CLOCK.with(|clock| {
                *clock.borrow_mut() = Some((Instant::now(), self.current as f32));
            });
        }
    }

    /// Stop any sound a previous
    /// [`EditorCore::start_playback_audio`] started. Safe to
    /// call when nothing is playing. Also clears the playback
    /// clock: with no pass running there is nothing for the
    /// drift sync to predict from.
    pub fn stop_playback_audio(&mut self) {
        PLAYER.with(|player| player.borrow_mut().stop());
        CLOCK.with(|clock| *clock.borrow_mut() = None);
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

    /// Re-sync the audio after a forward seek: if the playhead
    /// jumped forward between the shell's previous poll tick
    /// (`prev_frame`) and this one (`cur_frame`) while playing
    /// — the decision lives in `playhead_seeked` — and sound
    /// is actually running, restart it from the new position
    /// via stop + start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame (the frame the seek landed
    /// on), succeeds silently when nothing is audible there,
    /// and reports a player launch failure on `core.status`.
    /// Backward seeks need no path of their own here: they
    /// lower `current`, which
    /// [`EditorCore::sync_playback_audio_on_wrap`] already
    /// reads as its wrap signal and restarts for.
    ///
    /// When there was no jump, playback is stopped, or no
    /// audio is active (nothing audible, or the player already
    /// exited), this is a no-op and writes nothing to the
    /// status line. The shell calls it every poll tick beside
    /// the wrap sync, with the frame it saw last tick and
    /// `self.current`; the per-tick cost when nothing jumps is
    /// the comparison plus one process-liveness check.
    ///
    /// The `playing` guard mirrors the wrap sync's: the call
    /// happens inside the same tick that advanced the
    /// playhead, so the state that produced the motion is the
    /// state now. A seek made while stopped is followed by
    /// Play's own fresh start, so no restart is owed for it.
    pub fn sync_playback_audio_on_seek(&mut self, prev_frame: usize, cur_frame: usize) {
        let seeked = self.playing && playhead_seeked(prev_frame, cur_frame);
        if seeked && self.playback_audio_active() {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
    }

    /// Re-sync the audio after clock drift: if the fractional
    /// playhead (`core.playhead` — the shell's sub-frame
    /// position, of which `current` is the floor) has wandered
    /// further from the playback clock's prediction than
    /// [`DRIFT_TOLERANCE_FRAMES`] while playing and sound is
    /// actually running — the decision lives in
    /// `drift_restart_needed`, over the pure `expected_frame`
    /// and `drift_exceeded` — restart the mix at the playhead
    /// via stop + start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame, succeeds silently when
    /// nothing is audible there, reports a player launch
    /// failure on `core.status`, and re-bases the playback
    /// clock, so a pass that keeps drifting is corrected at
    /// most once per tolerance of accumulated error rather
    /// than on every tick.
    ///
    /// When the drift is within tolerance, playback is
    /// stopped, no audio is active, or no clock exists (no
    /// successful start since the last stop), this is a no-op
    /// and writes nothing to the status line. The shell calls
    /// it from poll beside the wrap and seek syncs, throttled
    /// to ticks where the frame changed (integrator wiring);
    /// the per-call cost is one clock read plus one
    /// process-liveness check.
    pub fn sync_playback_audio_on_drift(&mut self) {
        let clock = CLOCK.with(|clock| *clock.borrow());
        let expected = clock.map(|(started, start_frame)| {
            expected_frame(start_frame, started.elapsed().as_secs_f32(), self.project.fps)
        });
        if drift_restart_needed(self.playing, self.playback_audio_active(), expected, self.playhead)
        {
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

    #[test]
    fn seeked_detects_forward_jumps() {
        // Two or more frames ahead in one tick: a seek, a
        // scrub, or a stall long enough to skip frames.
        assert!(playhead_seeked(3, 5));
        assert!(playhead_seeked(0, 2));
        assert!(playhead_seeked(4, 10));
        assert!(playhead_seeked(0, 9));
    }

    #[test]
    fn seeked_ignores_advance_holds_and_backward_motion() {
        // Exactly one frame ahead is ordinary advance.
        assert!(!playhead_seeked(3, 4));
        assert!(!playhead_seeked(0, 1));
        // Holding on the same frame (a long-duration frame the
        // playhead has not crossed yet) is not a seek.
        assert!(!playhead_seeked(4, 4));
        assert!(!playhead_seeked(0, 0));
        // Backward motion is the wrap path's business: the
        // full-timeline fold and the loop-range fold alike.
        assert!(!playhead_seeked(9, 0));
        assert!(!playhead_seeked(7, 3));
    }

    #[test]
    fn drift_frames_is_the_signed_gap() {
        // Positive: the playhead is ahead of the sound.
        assert_eq!(drift_frames(10.0, 12.5), 2.5);
        // Negative: the playhead trails the sound.
        assert_eq!(drift_frames(10.0, 7.5), -2.5);
        assert_eq!(drift_frames(4.0, 4.0), 0.0);
        assert_eq!(drift_frames(0.0, 0.25), 0.25);
    }

    #[test]
    fn drift_exceeded_only_past_the_tolerance() {
        // Exactly at the tolerance is still within bounds, in
        // both directions (the comparison is strict).
        assert!(!drift_exceeded(10.0, 10.0 + DRIFT_TOLERANCE_FRAMES));
        assert!(!drift_exceeded(10.0, 10.0 - DRIFT_TOLERANCE_FRAMES));
        assert!(!drift_exceeded(10.0, 10.0));
        assert!(!drift_exceeded(10.0, 11.0));
        // A hair past it, in either direction, is exceeded.
        assert!(drift_exceeded(10.0, 10.0 + DRIFT_TOLERANCE_FRAMES + 0.01));
        assert!(drift_exceeded(10.0, 10.0 - DRIFT_TOLERANCE_FRAMES - 0.01));
        assert!(drift_exceeded(0.0, 100.0));
        assert!(drift_exceeded(100.0, 0.0));
    }

    #[test]
    fn expected_frame_counts_up_from_the_start() {
        // Two seconds at 24 fps from frame 10 lands on 58.
        assert_eq!(expected_frame(10.0, 2.0, 24.0), 58.0);
        assert_eq!(expected_frame(0.0, 0.0, 24.0), 0.0);
        assert_eq!(expected_frame(3.0, 0.5, 12.0), 9.0);
        // Non-positive rates floor at 1 fps, the same
        // defensive floor `playhead_secs` uses.
        assert_eq!(expected_frame(10.0, 2.0, 0.0), 12.0);
    }

    #[test]
    fn drift_restart_needs_playing_active_audio_a_clock_and_real_drift() {
        // The one shape that restarts: playing, sound running,
        // a clock predicting 10.0, the playhead far from it.
        assert!(drift_restart_needed(true, true, Some(10.0), 20.0));
        assert!(drift_restart_needed(true, true, Some(10.0), 0.0));
        // Within tolerance: no restart.
        assert!(!drift_restart_needed(true, true, Some(10.0), 12.5));
        assert!(!drift_restart_needed(true, true, Some(10.0), 10.0));
        // Paused: the playhead is not supposed to be moving,
        // and Play makes its own fresh start.
        assert!(!drift_restart_needed(false, true, Some(10.0), 20.0));
        // No sound running: nothing to re-sync, however large
        // the nominal gap (the clock may be a silent start's).
        assert!(!drift_restart_needed(true, false, Some(10.0), 20.0));
        // No clock: no start has recorded one (or a stop
        // cleared it), so there is nothing to predict from.
        assert!(!drift_restart_needed(true, true, None, 20.0));
    }

    #[test]
    fn start_records_the_clock_and_stop_clears_it() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        // A fresh core's thread has no clock.
        assert!(CLOCK.with(|clock| clock.borrow().is_none()));
        core.current = 3;
        core.start_playback_audio();
        // The clock is based at the integer frame the sound
        // starts from — `current`, not the fractional
        // playhead — even though nothing was audible.
        let recorded = CLOCK.with(|clock| *clock.borrow());
        let (started, start_frame) = recorded.expect("start records a clock");
        assert_eq!(start_frame, 3.0);
        assert!(started.elapsed().as_secs_f32() < 5.0);
        core.stop_playback_audio();
        assert!(CLOCK.with(|clock| clock.borrow().is_none()));
    }

    #[test]
    fn drift_sync_without_audio_is_a_silent_no_op() {
        // Plant a clock predicting far from the playhead:
        // started 10 s ago at frame 0 of a 24 fps project, so
        // the prediction is frame 240 while the picture sits
        // at frame 0 — drift far past tolerance. With no clips
        // the player is never active, so even this textbook
        // drift shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.fps = 24.0;
        core.playing = true;
        core.playhead = 0.0;
        CLOCK.with(|clock| {
            *clock.borrow_mut() = Some((Instant::now() - std::time::Duration::from_secs(10), 0.0));
        });
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_drift();
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // A no-op does not consume the clock either.
        assert!(CLOCK.with(|clock| clock.borrow().is_some()));
        core.stop_playback_audio();
    }

    #[test]
    fn seek_sync_without_audio_is_a_silent_no_op() {
        // No clips → the player is never active, so even a
        // textbook seek shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.playing = true;
        core.current = 3;
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_seek(0, core.current);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Ordinary advance and a paused seek shape alike.
        core.sync_playback_audio_on_seek(2, 3);
        core.playing = false;
        core.sync_playback_audio_on_seek(0, 3);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
    }
}
