//! Audio editor backend: minimal PCM16 WAV I/O, waveform peaks,
//! a real multi-clip mixdown, and playback scheduling.
//!
//! Std-only, no decoding crates: WAV files are parsed and written
//! by hand (RIFF/WAVE, PCM format 1, 16-bit, mono or stereo).
//! Samples are interleaved `f32` normalised to -1.0..=1.0.

/// Decoded WAV audio: interleaved samples, normalised to -1..=1.
#[derive(Clone, Debug, PartialEq)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl WavData {
    /// Number of sample frames (one frame = one sample per channel).
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    /// Duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f32 / self.sample_rate as f32
        }
    }
}

/// Read a PCM16 WAV file (mono or stereo) from `path`.
pub fn read_wav(path: &str) -> Result<WavData, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    parse_wav(&bytes)
}

/// Parse WAV bytes: RIFF header, `fmt ` chunk, `data` chunk.
/// Unknown chunks are skipped; chunk padding to even sizes is honoured.
fn parse_wav(bytes: &[u8]) -> Result<WavData, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut fmt: Option<(u16, u32, u16)> = None; // (channels, rate, bits)
    let mut data: Option<&[u8]> = None;
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body_start = pos + 8;
        let body_end = (body_start + size).min(bytes.len());
        match id {
            b"fmt " => {
                if size < 16 {
                    return Err("fmt chunk too small".into());
                }
                let b = &bytes[body_start..body_end];
                let format = u16::from_le_bytes(b[0..2].try_into().unwrap());
                if format != 1 {
                    return Err(format!("unsupported WAV format {format} (need PCM=1)"));
                }
                let channels = u16::from_le_bytes(b[2..4].try_into().unwrap());
                let rate = u32::from_le_bytes(b[4..8].try_into().unwrap());
                let bits = u16::from_le_bytes(b[14..16].try_into().unwrap());
                fmt = Some((channels, rate, bits));
            }
            b"data" => data = Some(&bytes[body_start..body_end]),
            _ => {}
        }
        // Chunks are padded to even sizes.
        pos = body_start + size + (size % 2);
    }
    let (channels, rate, bits) = fmt.ok_or("missing fmt chunk")?;
    if bits != 16 {
        return Err(format!("unsupported bit depth {bits} (need 16)"));
    }
    if channels != 1 && channels != 2 {
        return Err(format!("unsupported channel count {channels} (need 1 or 2)"));
    }
    let raw = data.ok_or("missing data chunk")?;
    let mut samples = Vec::with_capacity(raw.len() / 2);
    for pair in raw.chunks_exact(2) {
        samples.push(i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0);
    }
    Ok(WavData {
        sample_rate: rate,
        channels,
        samples,
    })
}

/// Serialise to a canonical 44-byte-header PCM16 WAV byte stream.
pub fn write_wav(data: &WavData) -> Vec<u8> {
    let channels = data.channels.max(1);
    let byte_rate = data.sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let data_len = (data.samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&data.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in &data.samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Max-abs amplitude per bucket across all channels, for waveform
/// display. `buckets == 0` yields an empty vec; empty audio yields
/// all-zero buckets so the display still has the right shape.
pub fn peaks(data: &WavData, buckets: usize) -> Vec<f32> {
    if buckets == 0 {
        return Vec::new();
    }
    let frames = data.frames();
    let mut out = vec![0.0f32; buckets];
    if frames == 0 {
        return out;
    }
    let ch = data.channels as usize;
    for (i, slot) in out.iter_mut().enumerate() {
        let start = i * frames / buckets;
        let end = ((i + 1) * frames / buckets).max(start + 1).min(frames);
        let mut max = 0.0f32;
        for f in start..end {
            for c in 0..ch {
                let v = data.samples[f * ch + c].abs();
                if v > max {
                    max = v;
                }
            }
        }
        *slot = max;
    }
    out
}

/// One clip in a mixdown: a WAV file placed on the timeline.
#[derive(Clone, Debug)]
pub struct MixClip {
    pub path: String,
    /// Timeline offset in seconds (negative values clamp to 0).
    pub start_secs: f32,
    /// Linear gain multiplier (1.0 = unity).
    pub gain: f32,
    /// Linear fade-in length in seconds (0 = none).
    pub fade_in: f32,
    /// Linear fade-out length in seconds (0 = none).
    pub fade_out: f32,
}

/// Convert `src` to `out_channels` channels and `out_rate`,
/// resampling by linear interpolation. Channel mapping: mono is
/// duplicated to stereo; stereo is averaged down to mono.
fn convert(src: &WavData, out_rate: u32, out_channels: u16) -> Vec<f32> {
    let src_frames = src.frames();
    if src_frames == 0 {
        return Vec::new();
    }
    let out_frames = ((src_frames as f64 * out_rate as f64 / src.sample_rate as f64).round()
        as usize)
        .max(1);
    let sc = src.channels as usize;
    let oc = out_channels as usize;
    let mut out = vec![0.0f32; out_frames * oc];
    for j in 0..out_frames {
        // Source position for this output frame, in source frames.
        let pos = j as f32 * src.sample_rate as f32 / out_rate as f32;
        let i0 = (pos.floor() as usize).min(src_frames - 1);
        let i1 = (i0 + 1).min(src_frames - 1);
        let frac = pos - pos.floor();
        for c in 0..oc {
            let sample = |frame: usize| -> f32 {
                if src.channels == out_channels {
                    src.samples[frame * sc + c]
                } else if src.channels == 1 {
                    src.samples[frame] // mono -> any channel
                } else {
                    // stereo -> mono: average
                    (src.samples[frame * sc] + src.samples[frame * sc + 1]) / 2.0
                }
            };
            out[j * oc + c] = sample(i0) * (1.0 - frac) + sample(i1) * frac;
        }
    }
    out
}

/// Mix clips into one WAV. Each clip is placed at its offset,
/// resampled (linear interpolation) if its rate differs, given
/// gain and linear fades, summed, and hard-clamped to -1..=1.
/// Output rate = max input rate; output channels = max input
/// channels. An empty clip list is an error.
pub fn mix(clips: &[MixClip]) -> Result<WavData, String> {
    if clips.is_empty() {
        return Err("mix: no clips".into());
    }
    let mut loaded: Vec<WavData> = Vec::with_capacity(clips.len());
    for clip in clips {
        loaded.push(read_wav(&clip.path)?);
    }
    let out_rate = loaded.iter().map(|w| w.sample_rate).max().unwrap_or(44100);
    let out_channels = loaded.iter().map(|w| w.channels).max().unwrap_or(1);
    let oc = out_channels as usize;

    // Total length: the latest clip end wins.
    let mut total_frames = 0usize;
    let mut placed: Vec<(usize, Vec<f32>)> = Vec::with_capacity(clips.len());
    for (clip, wav) in clips.iter().zip(loaded.iter()) {
        let buf = convert(wav, out_rate, out_channels);
        let clip_frames = buf.len() / oc;
        let offset = (clip.start_secs.max(0.0) * out_rate as f32).round() as usize;
        total_frames = total_frames.max(offset + clip_frames);
        placed.push((offset, buf));
    }

    let mut out = vec![0.0f32; total_frames * oc];
    for (clip, (offset, buf)) in clips.iter().zip(placed.iter()) {
        let clip_frames = buf.len() / oc;
        let fade_in_frames = (clip.fade_in.max(0.0) * out_rate as f32).round() as usize;
        let fade_out_frames = (clip.fade_out.max(0.0) * out_rate as f32).round() as usize;
        for j in 0..clip_frames {
            let mut amp = clip.gain;
            if fade_in_frames > 0 && j < fade_in_frames {
                amp *= j as f32 / fade_in_frames as f32;
            }
            if fade_out_frames > 0 {
                let remaining = clip_frames - j;
                if remaining <= fade_out_frames {
                    amp *= remaining as f32 / fade_out_frames as f32;
                }
            }
            if amp == 0.0 {
                continue;
            }
            let dst = (offset + j) * oc;
            for c in 0..oc {
                out[dst + c] += buf[j * oc + c] * amp;
            }
        }
    }
    for s in out.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
    Ok(WavData {
        sample_rate: out_rate,
        channels: out_channels,
        samples: out,
    })
}

/// Which project audio clips are sounding at `playhead_secs`, and
/// at what offset (seconds) into each clip — the schedule the app
/// needs for multi-clip playback.
///
/// A clip starts at `start_frame / fps` seconds and sounds until
/// its duration elapses. `durations` resolves a clip path to its
/// duration in seconds; clips whose duration is unknown (`None`)
/// are omitted, since their end cannot be scheduled. Returns
/// `(clip_index, offset_secs)` pairs in clip order.
pub fn audible_at(
    clips: &[crate::model::AudioClip],
    playhead_secs: f32,
    fps: f32,
    durations: &dyn Fn(&str) -> Option<f32>,
) -> Vec<(usize, f32)> {
    let fps = if fps > 0.0 { fps } else { 1.0 };
    let mut out = Vec::new();
    for (i, clip) in clips.iter().enumerate() {
        let start = clip.start_frame as f32 / fps;
        if playhead_secs < start {
            continue;
        }
        if let Some(dur) = durations(&clip.path) {
            if playhead_secs < start + dur {
                out.push((i, playhead_secs - start));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AudioClip;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-audio-edit-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(data: &WavData, dir: &std::path::Path, name: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, write_wav(data)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn const_wav(rate: u32, secs: f32, value: f32) -> WavData {
        let n = (rate as f32 * secs) as usize;
        WavData {
            sample_rate: rate,
            channels: 1,
            samples: vec![value; n],
        }
    }

    #[test]
    fn wav_roundtrip_preserves_samples() {
        let dir = temp_dir("roundtrip");
        let src = WavData {
            sample_rate: 22050,
            channels: 2,
            samples: vec![0.0, 0.5, -0.5, 0.25, -0.25, 1.0, -1.0, 0.1],
        };
        let path = write_file(&src, &dir, "rt.wav");
        let back = read_wav(&path).unwrap();
        assert_eq!(back.sample_rate, 22050);
        assert_eq!(back.channels, 2);
        assert_eq!(back.samples.len(), src.samples.len());
        for (a, b) in src.samples.iter().zip(back.samples.iter()) {
            assert!((a - b).abs() < 0.001, "{a} vs {b}");
        }
    }

    #[test]
    fn peaks_of_sine_burst() {
        // 1 second at 8000 Hz: silence, then a 0.8-amplitude sine
        // burst in the middle half.
        let rate = 8000u32;
        let mut samples = vec![0.0f32; rate as usize];
        for (i, s) in samples.iter_mut().enumerate().take(6000).skip(2000) {
            *s = 0.8 * (i as f32 * 0.3).sin();
        }
        let data = WavData {
            sample_rate: rate,
            channels: 1,
            samples,
        };
        let p = peaks(&data, 8);
        assert_eq!(p.len(), 8);
        assert_eq!(p[0], 0.0);
        assert!(p[3] > 0.75, "burst bucket should peak near 0.8, got {}", p[3]);
        assert!(p[4] > 0.75);
        assert_eq!(p[7], 0.0);
        assert!(peaks(&data, 0).is_empty());
    }

    #[test]
    fn mix_places_offset_clips() {
        let dir = temp_dir("mix-offset");
        let a = write_file(&const_wav(8000, 0.1, 0.5), &dir, "a.wav");
        let b = write_file(&const_wav(8000, 0.1, 0.25), &dir, "b.wav");
        let out = mix(&[
            MixClip {
                path: a,
                start_secs: 0.0,
                gain: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
            },
            MixClip {
                path: b,
                start_secs: 0.05,
                gain: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
            },
        ])
        .unwrap();
        assert_eq!(out.sample_rate, 8000);
        // 0.15 s total = 1200 frames.
        assert_eq!(out.frames(), 1200);
        let at = |f: usize| out.samples[f];
        assert!((at(0) - 0.5).abs() < 0.01, "A alone: {}", at(0));
        assert!((at(600) - 0.75).abs() < 0.01, "A+B overlap: {}", at(600));
        assert!((at(1000) - 0.25).abs() < 0.01, "B alone: {}", at(1000));
    }

    #[test]
    fn mix_fades_and_gain_shape_amplitude() {
        let dir = temp_dir("mix-fades");
        let a = write_file(&const_wav(8000, 0.1, 1.0), &dir, "a.wav");
        let out = mix(&[MixClip {
            path: a,
            start_secs: 0.0,
            gain: 0.5,
            fade_in: 0.05,  // 400 frames
            fade_out: 0.05, // last 400 frames
        }])
        .unwrap();
        let at = |f: usize| out.samples[f];
        assert_eq!(at(0), 0.0, "fade-in starts at zero");
        assert!((at(200) - 0.25).abs() < 0.01, "mid fade-in: {}", at(200));
        assert!((at(400) - 0.5).abs() < 0.02, "fade-in done, gain only: {}", at(400));
        assert!(at(799) < 0.01, "fade-out ends near zero: {}", at(799));
    }

    #[test]
    fn mix_resamples_and_clamps() {
        let dir = temp_dir("mix-resample");
        // A loud 4000 Hz clip + a loud 8000 Hz clip: output rate is
        // 8000, the 4k clip is stretched to twice the frames, and
        // the sum hard-clamps at 1.0.
        let a = write_file(&const_wav(4000, 0.1, 0.9), &dir, "a.wav");
        let b = write_file(&const_wav(8000, 0.1, 0.9), &dir, "b.wav");
        let out = mix(&[
            MixClip {
                path: a,
                start_secs: 0.0,
                gain: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
            },
            MixClip {
                path: b,
                start_secs: 0.0,
                gain: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
            },
        ])
        .unwrap();
        assert_eq!(out.sample_rate, 8000);
        assert_eq!(out.frames(), 800);
        assert!(out.samples.iter().all(|s| *s <= 1.0));
        assert!((out.samples[400] - 1.0).abs() < 0.001, "clamped sum");
    }

    #[test]
    fn mix_empty_is_error() {
        assert!(mix(&[]).is_err());
    }

    #[test]
    fn audible_at_picks_right_clips_and_offsets() {
        let clips = vec![
            AudioClip {
                path: "a.wav".into(),
                start_frame: 0,
            },
            AudioClip {
                path: "b.wav".into(),
                start_frame: 24,
            },
            AudioClip {
                path: "c.wav".into(),
                start_frame: 48,
            },
        ];
        // All clips last 1.5 s; fps 24 -> starts at 0s, 1s, 2s.
        let durations = |_: &str| Some(1.5f32);
        let at = |t: f32| audible_at(&clips, t, 24.0, &durations);
        assert_eq!(at(0.5), vec![(0, 0.5)]);
        let v = at(1.2);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, 0);
        assert!((v[0].1 - 1.2).abs() < 0.001);
        assert_eq!(v[1].0, 1);
        assert!((v[1].1 - 0.2).abs() < 0.001);
        // b ended at 2.5; at 2.6 only c sounds.
        let v = at(2.6);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, 2);
        assert!((v[0].1 - 0.6).abs() < 0.001);
        // Before everything / after everything.
        assert!(at(-1.0).is_empty());
        assert!(at(4.0).is_empty());
        // Unknown duration -> clip omitted.
        let none_durations = |_: &str| None;
        assert!(audible_at(&clips, 0.5, 24.0, &none_durations).is_empty());
    }
}
