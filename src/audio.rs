//! Small audio playback helper backed by the system `ffplay`.
//!
//! `ffplay` ships with ffmpeg, which StykAnimate-RS already relies on
//! for video export. Playback is deliberately kept at the process
//! boundary: this module can start a clip at a timeline offset, stop
//! it, and report whether the player process is still running. It
//! does not synchronize against a sample clock.
//!
//! Two entry points exist. [`AudioPlayer::play`] plays one file
//! directly. [`AudioPlayer::play_project`] plays *every* clip that
//! is audible at the playhead, mixed together: when two or more
//! clips are audible it renders a mix with
//! [`crate::audio_edit::mix`] into a temporary WAV and plays that.
//! The mix is rendered at play time — clips are decoded once to
//! probe their durations and again by the mixer — so starting
//! playback on a project with long or numerous clips costs a decode
//! and mix pass, and the mix buffer is held in memory at the
//! project's sample rate. Clip files that are missing or unreadable
//! are skipped; if mixing fails, playback falls back to the first
//! audible clip rather than staying silent.

use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Return true when an `ffplay` executable can be launched and
/// successfully reports its version.
pub fn ffplay_available() -> bool {
    Command::new("ffplay")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Convert a timeline frame to the matching audio offset in seconds.
///
/// The project's frame rates are positive. As in the MP4 audio mux,
/// non-positive rates are treated defensively as 1 fps rather than
/// producing an infinite or otherwise unusable offset.
pub fn clip_offset_secs(start_frame: usize, fps: f32) -> f32 {
    start_frame as f32 / fps.max(1.0)
}

/// What playback should do for the clips audible at a playhead.
///
/// Produced by [`playback_plan`]; consumed by
/// [`AudioPlayer::play_project`].
#[derive(Clone, Debug)]
pub enum PlaybackPlan {
    /// Nothing is audible at the playhead — or every clip that
    /// would be audible is missing, unreadable or muted.
    Silent,
    /// Exactly one audible clip, at unity gain: play its file
    /// directly, starting `offset_secs` into the file. A lone clip
    /// whose gain is not 1.0 takes the `Mixed` path instead —
    /// ffplay cannot scale a directly played file's volume, so
    /// the gain is applied by rendering a one-clip mix.
    Direct { path: String, offset_secs: f32 },
    /// Two or more audible clips — or a single clip with a
    /// non-unity gain: render `mix_clips` into one WAV (each
    /// clip scaled by its `gain`) and play it starting
    /// `offset_secs` in. The mix starts at
    /// timeline zero (each clip sits at its absolute timeline
    /// offset), so the playhead position *is* the offset into the
    /// mix; the offset into any one file is
    /// `offset_secs - that clip's start_secs`.
    Mixed {
        mix_clips: Vec<crate::audio_edit::MixClip>,
        offset_secs: f32,
    },
}

/// Decide how to play `clips` at `playhead_secs`.
///
/// A clip is audible when the playhead lies inside its span
/// (`start_frame / fps` until start + duration) — the same rule as
/// [`crate::audio_edit::audible_at`]. `durations` resolves a clip
/// path to its duration in seconds; it must return `None` for
/// files that are missing or unreadable, and such clips are
/// excluded, since their end cannot be scheduled. Muted clips are
/// excluded on the same path: a muted clip never sounds directly
/// and never joins a mix, so a playhead covered only by muted
/// clips plans `Silent`.
///
/// This function is pure (the only contact with the filesystem is
/// through the `durations` probe), so the decision logic is
/// unit-testable without audio files or a player process.
pub fn playback_plan(
    clips: &[crate::model::AudioClip],
    playhead_secs: f32,
    fps: f32,
    durations: &dyn Fn(&str) -> Option<f32>,
) -> PlaybackPlan {
    // Match audible_at's frame-rate convention exactly.
    let fps = if fps > 0.0 { fps } else { 1.0 };
    // `audible_at` is the single classification of which clips
    // are eligible at the playhead (span + readable file); the
    // mute filter narrows that same list, so every plan shape
    // below — Direct, Mixed, and the mix-failure fallback in
    // `play_project`, which replays `mix_clips[0]` — inherits it
    // without a second classification anywhere.
    let audible: Vec<(usize, f32)> =
        crate::audio_edit::audible_at(clips, playhead_secs, fps, durations)
            .into_iter()
            .filter(|&(index, _)| !clips[index].muted)
            .collect();
    let to_mix_clip = |index: usize| crate::audio_edit::MixClip {
        path: clips[index].path.clone(),
        start_secs: clips[index].start_frame as f32 / fps,
        gain: clips[index].gain,
        fade_in: 0.0,
        fade_out: 0.0,
    };
    match audible.len() {
        0 => PlaybackPlan::Silent,
        1 => {
            let (index, offset_secs) = audible[0];
            if clips[index].gain != 1.0 {
                // ffplay has no per-file volume flag in the direct
                // invocation, so a gained lone clip routes through
                // the mix path (a one-clip mix) to honour its gain.
                PlaybackPlan::Mixed {
                    mix_clips: vec![to_mix_clip(index)],
                    offset_secs: playhead_secs.max(0.0),
                }
            } else {
                PlaybackPlan::Direct {
                    path: clips[index].path.clone(),
                    offset_secs,
                }
            }
        }
        _ => PlaybackPlan::Mixed {
            mix_clips: audible.iter().map(|&(index, _)| to_mix_clip(index)).collect(),
            offset_secs: playhead_secs.max(0.0),
        },
    }
}

/// Unique temporary path for a rendered playback mix.
fn mix_temp_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "stykanimate-rs-playback-mix-{}-{unique}.wav",
        std::process::id()
    ))
}

/// An audio player owning at most one `ffplay` process.
///
/// [`AudioPlayer::play`] plays a single file directly;
/// [`AudioPlayer::play_project`] plays every clip audible at the
/// playhead, rendering a temporary mix file when several clips
/// sound at once. The mix file is tracked in `mix_file` and
/// deleted when playback stops or is replaced.
pub struct AudioPlayer {
    child: Option<Child>,
    mix_file: Option<PathBuf>,
}

impl AudioPlayer {
    /// Create an idle player.
    pub fn new() -> Self {
        Self {
            child: None,
            mix_file: None,
        }
    }

    /// Play `path`, starting `offset_secs` into the audio file.
    ///
    /// Any clip that is already playing is stopped first. A successful
    /// return means the `ffplay` process was spawned; on a machine
    /// without a usable audio device, that process may still exit
    /// immediately after spawning.
    pub fn play(&mut self, path: &str, offset_secs: f32) -> io::Result<()> {
        self.stop();

        let offset = offset_secs.max(0.0);
        let child = Command::new("ffplay")
            .arg("-nodisp")
            .arg("-autoexit")
            .arg("-loglevel")
            .arg("quiet")
            .arg("-ss")
            .arg(offset.to_string())
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.child = Some(child);
        Ok(())
    }

    /// Play every clip in `clips` that is audible at
    /// `playhead_secs`, mixed together.
    ///
    /// `fps` converts each clip's `start_frame` to a timeline
    /// offset. The decision logic is [`playback_plan`]: with no
    /// audible clip this stops the player and succeeds silently;
    /// with exactly one at unity gain, the clip's file is played
    /// directly (the same path as [`AudioPlayer::play`]); with
    /// two or more — or a single clip whose gain is not 1.0 — a
    /// mix is rendered with [`crate::audio_edit::mix`] to a
    /// temporary WAV and that is played instead. Clip files that
    /// cannot be decoded are skipped. If the mix itself fails,
    /// playback falls back to the first audible clip.
    pub fn play_project(
        &mut self,
        clips: &[crate::model::AudioClip],
        fps: f32,
        playhead_secs: f32,
    ) -> io::Result<()> {
        self.stop();

        // Probe each clip by decoding it: a file that does not
        // decode has no schedulable duration and is excluded.
        let durations = |path: &str| {
            crate::audio_edit::read_wav(path)
                .ok()
                .map(|wav| wav.duration_secs())
        };

        match playback_plan(clips, playhead_secs, fps, &durations) {
            PlaybackPlan::Silent => Ok(()),
            PlaybackPlan::Direct { path, offset_secs } => self.play(&path, offset_secs),
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => match crate::audio_edit::mix(&mix_clips) {
                Ok(mixed) => {
                    let path = mix_temp_path();
                    std::fs::write(&path, crate::audio_edit::write_wav(&mixed))?;
                    let path_str = path.to_string_lossy().into_owned();
                    let result = self.play(&path_str, offset_secs);
                    if result.is_ok() {
                        self.mix_file = Some(path);
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                    result
                }
                Err(_) => {
                    // Mixing failed (for example a clip became
                    // unreadable between probing and mixing): play
                    // the first audible clip directly rather than
                    // staying silent.
                    let first = &mix_clips[0];
                    let offset = (offset_secs - first.start_secs).max(0.0);
                    let path = first.path.clone();
                    self.play(&path, offset)
                }
            },
        }
    }

    /// Stop the current clip, if any, and wait for its process to exit.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // The process may already have exited, in which case kill
            // can report an error. Either way, reap it so no zombie
            // process is left behind.
            let _ = child.kill();
            let _ = child.wait();
        }
        self.clear_mix_file();
    }

    /// Delete the temporary mix file, if one is being tracked.
    fn clear_mix_file(&mut self) {
        if let Some(path) = self.mix_file.take() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Return true while the owned `ffplay` process is still running.
    pub fn is_playing(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };

        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                self.child = None;
                self.clear_mix_file();
                false
            }
            Err(_) => {
                self.child = None;
                self.clear_mix_file();
                false
            }
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ffplay_availability_check_does_not_panic() {
        let _available: bool = ffplay_available();
    }

    #[test]
    fn clip_offset_uses_frame_rate() {
        assert_eq!(clip_offset_secs(0, 24.0), 0.0);
        assert_eq!(clip_offset_secs(24, 24.0), 1.0);
        assert_eq!(clip_offset_secs(36, 24.0), 1.5);
        assert_eq!(clip_offset_secs(10, 0.0), 10.0);
    }

    fn clip(path: &str, start_frame: usize) -> crate::model::AudioClip {
        crate::model::AudioClip {
            path: path.to_string(),
            start_frame,
            gain: 1.0,
            muted: false,
        }
    }

    fn muted_clip(path: &str, start_frame: usize) -> crate::model::AudioClip {
        crate::model::AudioClip {
            path: path.to_string(),
            start_frame,
            gain: 1.0,
            muted: true,
        }
    }

    #[test]
    fn playback_plan_mixes_two_audible_clips_with_timeline_offsets() {
        // Clip A starts at frame 0, clip B at frame 48; at 24 fps
        // that is 0.0s and 2.0s on the timeline. Both last 10s.
        let clips = vec![clip("a.wav", 0), clip("b.wav", 48)];
        let durations = |path: &str| match path {
            "a.wav" | "b.wav" => Some(10.0),
            _ => None,
        };
        // Playhead at 3.0s: A is 3.0s in, B is 1.0s in; both sound.
        match playback_plan(&clips, 3.0, 24.0, &durations) {
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => {
                assert_eq!(mix_clips.len(), 2);
                assert_eq!(mix_clips[0].path, "a.wav");
                assert_eq!(mix_clips[0].start_secs, 0.0);
                assert_eq!(mix_clips[1].path, "b.wav");
                assert_eq!(mix_clips[1].start_secs, 2.0);
                // The mix starts at timeline zero, so the playhead
                // position is the offset into the mix...
                assert_eq!(offset_secs, 3.0);
                // ...and each file's own offset is the playhead
                // minus that clip's timeline start.
                assert_eq!(offset_secs - mix_clips[0].start_secs, 3.0);
                assert_eq!(offset_secs - mix_clips[1].start_secs, 1.0);
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
    }

    #[test]
    fn playback_plan_single_audible_clip_plays_directly() {
        let clips = vec![clip("a.wav", 24), clip("b.wav", 240)];
        let durations = |_: &str| Some(4.0);
        // Playhead at 2.0s (24 fps): inside A (1.0s..5.0s), but B
        // does not start until 10.0s, so only A sounds.
        match playback_plan(&clips, 2.0, 24.0, &durations) {
            PlaybackPlan::Direct { path, offset_secs } => {
                assert_eq!(path, "a.wav");
                assert_eq!(offset_secs, 1.0);
            }
            other => panic!("expected a Direct plan, got {other:?}"),
        }
    }

    #[test]
    fn playback_plan_excludes_missing_files() {
        // Both clips cover the playhead, but b.wav cannot be probed
        // (missing or unreadable), leaving only A — which degrades
        // the plan from Mixed to Direct instead of failing.
        let clips = vec![clip("a.wav", 0), clip("b.wav", 0)];
        let durations = |path: &str| match path {
            "a.wav" => Some(10.0),
            _ => None,
        };
        match playback_plan(&clips, 1.0, 24.0, &durations) {
            PlaybackPlan::Direct { path, offset_secs } => {
                assert_eq!(path, "a.wav");
                assert_eq!(offset_secs, 1.0);
            }
            other => panic!("expected a Direct plan, got {other:?}"),
        }
    }

    #[test]
    fn playback_plan_silent_when_nothing_is_audible() {
        let durations = |_: &str| Some(4.0);
        // No clips at all.
        assert!(matches!(
            playback_plan(&[], 0.0, 24.0, &durations),
            PlaybackPlan::Silent
        ));
        // Playhead before the clip starts.
        let clips = vec![clip("a.wav", 48)];
        assert!(matches!(
            playback_plan(&clips, 1.0, 24.0, &durations),
            PlaybackPlan::Silent
        ));
        // Playhead after the clip ends (starts 2.0s, lasts 4.0s).
        assert!(matches!(
            playback_plan(&clips, 7.0, 24.0, &durations),
            PlaybackPlan::Silent
        ));
    }

    #[test]
    fn playback_plan_muted_single_clip_is_silent() {
        // The only clip covers the playhead and its file is
        // readable, but it is muted: nothing may sound.
        let clips = vec![muted_clip("a.wav", 0)];
        let durations = |_: &str| Some(10.0);
        assert!(matches!(
            playback_plan(&clips, 1.0, 24.0, &durations),
            PlaybackPlan::Silent
        ));
    }

    #[test]
    fn playback_plan_all_muted_is_silent() {
        // Several muted clips overlapping the playhead still
        // plan Silent — muting is not outvoted by clip count.
        let clips = vec![muted_clip("a.wav", 0), muted_clip("b.wav", 24)];
        let durations = |_: &str| Some(10.0);
        assert!(matches!(
            playback_plan(&clips, 2.0, 24.0, &durations),
            PlaybackPlan::Silent
        ));
    }

    #[test]
    fn playback_plan_muted_plus_audible_plays_the_audible_directly() {
        // Both clips cover the playhead; with A muted, B is the
        // only audible clip, so it plays directly — the muted
        // clip must not drag the plan into a mix.
        let clips = vec![muted_clip("a.wav", 0), clip("b.wav", 0)];
        let durations = |_: &str| Some(10.0);
        match playback_plan(&clips, 1.0, 24.0, &durations) {
            PlaybackPlan::Direct { path, offset_secs } => {
                assert_eq!(path, "b.wav");
                assert_eq!(offset_secs, 1.0);
            }
            other => panic!("expected a Direct plan, got {other:?}"),
        }
    }

    #[test]
    fn playback_plan_mix_excludes_muted_clips() {
        // Three clips cover the playhead, but the middle one is
        // muted: the mix holds exactly the two audible clips, in
        // timeline order, and the muted path is absent.
        let clips = vec![
            clip("a.wav", 0),
            muted_clip("b.wav", 24),
            clip("c.wav", 48),
        ];
        let durations = |_: &str| Some(10.0);
        match playback_plan(&clips, 3.0, 24.0, &durations) {
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => {
                assert_eq!(mix_clips.len(), 2);
                assert_eq!(mix_clips[0].path, "a.wav");
                assert_eq!(mix_clips[0].start_secs, 0.0);
                assert_eq!(mix_clips[1].path, "c.wav");
                assert_eq!(mix_clips[1].start_secs, 2.0);
                assert!(mix_clips.iter().all(|c| c.path != "b.wav"));
                assert_eq!(offset_secs, 3.0);
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
    }

    #[test]
    fn playback_smoke_test_spawns_and_stops() {
        if !ffplay_available() {
            eprintln!("skipping audio playback smoke test: ffplay is not available");
            return;
        }

        let path = silent_wav_path();
        write_silent_wav(&path).expect("failed to write silent WAV fixture");

        let mut player = AudioPlayer::new();
        let result = player.play(path.to_str().expect("temporary path is not UTF-8"), 0.0);

        // Spawning is the portable assertion here: a headless machine
        // may have no audio device, so ffplay can legitimately exit
        // immediately after a successful spawn.
        assert!(result.is_ok(), "ffplay failed to spawn: {result:?}");
        let _ = player.is_playing();

        player.stop();
        assert!(!player.is_playing());

        let _ = fs::remove_file(path);
    }

    fn silent_wav_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "stykanimate-rs-audio-test-{}-{unique}.wav",
            std::process::id()
        ))
    }

    fn write_silent_wav(path: &PathBuf) -> io::Result<()> {
        const SAMPLE_RATE: u32 = 8_000;
        const SAMPLE_COUNT: u32 = 1_600; // 0.2 seconds of mono silence.
        const CHANNELS: u16 = 1;
        const BITS_PER_SAMPLE: u16 = 16;
        const BYTES_PER_SAMPLE: u32 = (BITS_PER_SAMPLE / 8) as u32;
        let data_size = SAMPLE_COUNT * BYTES_PER_SAMPLE;
        let byte_rate = SAMPLE_RATE * CHANNELS as u32 * BYTES_PER_SAMPLE;
        let block_align = (CHANNELS * (BITS_PER_SAMPLE / 8)) as u16;

        let mut wav = Vec::with_capacity(44 + data_size as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&CHANNELS.to_le_bytes());
        wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.resize(44 + data_size as usize, 0);

        fs::File::create(path)?.write_all(&wav)
    }
}

#[cfg(test)]
mod gain_tests {
    use super::{playback_plan, PlaybackPlan};
    use crate::audio_edit::{mix, write_wav, WavData};
    use crate::model::AudioClip;

    fn clip(path: &str, start_frame: usize, gain: f32) -> AudioClip {
        AudioClip {
            path: path.to_string(),
            start_frame,
            gain,
            muted: false,
        }
    }

    /// Write a one-second mono WAV of constant `amplitude` to a
    /// unique temp path and return the path as a String.
    fn const_wav_path(tag: &str, amplitude: f32) -> String {
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![amplitude; 8000],
        };
        let path = std::env::temp_dir().join(format!(
            "styk-gain-{tag}-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn mixed_plan_scales_each_clip_by_its_gain() {
        // Two real clips, both constant 0.5: at unity they would
        // sum to 1.0; with the second at gain 0.5 the mix must
        // sit at 0.5 + 0.25 = 0.75 — the gained clip contributes
        // at half amplitude.
        let a = const_wav_path("mix-a", 0.5);
        let b = const_wav_path("mix-b", 0.5);
        let clips = vec![clip(&a, 0, 1.0), clip(&b, 0, 0.5)];
        let durations = |_: &str| Some(1.0);
        match playback_plan(&clips, 0.5, 24.0, &durations) {
            PlaybackPlan::Mixed { mix_clips, .. } => {
                assert_eq!(mix_clips.len(), 2);
                assert_eq!(mix_clips[0].gain, 1.0);
                assert_eq!(mix_clips[1].gain, 0.5);
                let mixed = mix(&mix_clips).unwrap();
                assert!(!mixed.samples.is_empty());
                for s in &mixed.samples {
                    assert!((s - 0.75).abs() < 1e-3, "expected ~0.75, got {s}");
                }
                // The gained clip alone at 0.5 vs at unity: half.
                let half = mix(&mix_clips[1..]).unwrap();
                for s in &half.samples {
                    assert!((s - 0.25).abs() < 1e-3, "expected ~0.25, got {s}");
                }
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn single_gained_clip_routes_to_mixed_not_direct() {
        // ffplay cannot scale a directly played file, so a lone
        // clip with gain 0.5 must take the one-clip Mixed path.
        let clips = vec![clip("a.wav", 24, 0.5)];
        let durations = |_: &str| Some(4.0);
        match playback_plan(&clips, 2.0, 24.0, &durations) {
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => {
                assert_eq!(mix_clips.len(), 1);
                assert_eq!(mix_clips[0].path, "a.wav");
                assert_eq!(mix_clips[0].gain, 0.5);
                assert_eq!(mix_clips[0].start_secs, 1.0);
                assert_eq!(offset_secs, 2.0);
            }
            other => panic!("expected a one-clip Mixed plan, got {other:?}"),
        }
    }

    #[test]
    fn unity_gain_plans_keep_their_existing_shapes() {
        let durations = |_: &str| Some(4.0);
        // A lone unity-gain clip still plays directly.
        let clips = vec![clip("a.wav", 24, 1.0)];
        assert!(matches!(
            playback_plan(&clips, 2.0, 24.0, &durations),
            PlaybackPlan::Direct { .. }
        ));
        // Two unity-gain clips mix with both gains at 1.0.
        let clips = vec![clip("a.wav", 0, 1.0), clip("b.wav", 0, 1.0)];
        match playback_plan(&clips, 1.0, 24.0, &durations) {
            PlaybackPlan::Mixed { mix_clips, .. } => {
                assert!(mix_clips.iter().all(|c| c.gain == 1.0));
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
    }
}
