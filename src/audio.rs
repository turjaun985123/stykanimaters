//! Small audio playback helper backed by the system `ffplay`.
//!
//! `ffplay` ships with ffmpeg, which StykAnimate-RS already relies on
//! for video export. Playback is deliberately kept at the process
//! boundary: this module can start a clip at a timeline offset, stop
//! it, and report whether the player process is still running. It
//! does not attempt to mix audio or synchronize against a sample
//! clock.

use std::io;
use std::process::{Child, Command, Stdio};

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

/// A single-clip audio player owning at most one `ffplay` process.
pub struct AudioPlayer {
    child: Option<Child>,
}

impl AudioPlayer {
    /// Create an idle player.
    pub fn new() -> Self {
        Self { child: None }
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

    /// Stop the current clip, if any, and wait for its process to exit.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // The process may already have exited, in which case kill
            // can report an error. Either way, reap it so no zombie
            // process is left behind.
            let _ = child.kill();
            let _ = child.wait();
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
                false
            }
            Err(_) => {
                self.child = None;
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
