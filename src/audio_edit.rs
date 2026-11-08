//! Audio editor backend: minimal PCM16 WAV I/O, waveform peaks,
//! a real multi-clip mixdown, playback scheduling, and editing
//! operations: peak/RMS metering, normalisation, trim, split,
//! crossfade joins, timeline clip splitting, and effects (fade,
//! gain, reverse, echo, lowpass, silence trim, silence
//! segmentation, pitch shift, compression).
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

/// Peak amplitude (max absolute sample) over all channels.
/// Empty audio peaks at 0.0.
pub fn peak(data: &WavData) -> f32 {
    data.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// RMS (root-mean-square) amplitude over all channels — a loudness
/// proxy. Empty audio has RMS 0.0.
pub fn rms(data: &WavData) -> f32 {
    if data.samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = data.samples.iter().map(|s| s * s).sum();
    (sum / data.samples.len() as f32).sqrt()
}

/// Scale all samples in place so the peak equals `target_peak`,
/// and return the gain that was applied.
///
/// `target_peak` is clamped to 0.0..=1.0. Silent input (peak 0,
/// including empty audio) is a no-op and returns 1.0 (unity gain) —
/// there is nothing to scale. A target of 0.0 silences the data
/// and returns 0.0.
pub fn normalize(data: &mut WavData, target_peak: f32) -> f32 {
    let target = target_peak.clamp(0.0, 1.0);
    let p = peak(data);
    if p == 0.0 {
        return 1.0;
    }
    let gain = target / p;
    for s in data.samples.iter_mut() {
        *s *= gain;
    }
    gain
}

/// Apply linear fade-in and fade-out ramps in place.
///
/// Lengths are converted with `secs_to_frame` — rounded to frames
/// and clamped to the clip — so an over-long fade simply spans the
/// whole clip, negative lengths clamp to zero, and a call with both
/// lengths zero is a no-op. The ramp follows `mix`'s fade
/// convention: a fade-in of `F` frames scales frame `j` (`j < F`)
/// by `j / F`, so the first frame is silent and full level resumes
/// at frame `F`; a fade-out scales the last `F` frames by their
/// remaining length, so the final frame lands at `1 / F` of its
/// level (≈ silence). Where the two ramps overlap they multiply.
/// The ramp is per frame: every channel of a frame is scaled by
/// the same factor.
pub fn fade(data: &mut WavData, fade_in_secs: f32, fade_out_secs: f32) {
    let frames = data.frames();
    let ch = data.channels as usize;
    if frames == 0 || ch == 0 {
        return;
    }
    let fade_in = secs_to_frame(data, fade_in_secs);
    let fade_out = secs_to_frame(data, fade_out_secs);
    if fade_in == 0 && fade_out == 0 {
        return;
    }
    for j in 0..frames {
        let mut amp = 1.0f32;
        if fade_in > 0 && j < fade_in {
            amp *= j as f32 / fade_in as f32;
        }
        if fade_out > 0 {
            let remaining = frames - j;
            if remaining <= fade_out {
                amp *= remaining as f32 / fade_out as f32;
            }
        }
        if amp == 1.0 {
            continue;
        }
        for c in 0..ch {
            data.samples[j * ch + c] *= amp;
        }
    }
}

/// Apply a constant gain in decibels in place: every sample is
/// multiplied by `10^(db / 20)` (+6 dB ≈ ×2, −6 dB ≈ ×0.5,
/// 0 dB = unity) and hard-clamped to -1..=1 — the same clamp
/// `mix` puts on its sum. Returns the linear multiplier that was
/// applied, like `normalize` returns its gain. Empty audio is a
/// no-op apart from the return value.
pub fn gain(data: &mut WavData, db: f32) -> f32 {
    let factor = 10f32.powf(db / 20.0);
    for s in data.samples.iter_mut() {
        *s = (*s * factor).clamp(-1.0, 1.0);
    }
    factor
}

/// Reverse the clip in place — the classic backwards-playback
/// effect. Frames trade places; the channels inside each frame
/// keep their order, so a stereo clip's left/right are not
/// swapped. Empty and single-frame clips are unchanged.
pub fn reverse(data: &mut WavData) {
    let ch = data.channels as usize;
    if ch == 0 {
        return;
    }
    let frames = data.frames();
    for j in 0..frames / 2 {
        let a = j * ch;
        let b = (frames - 1 - j) * ch;
        for c in 0..ch {
            data.samples.swap(a + c, b + c);
        }
    }
}

/// Feedback-delay echo in place: every frame is repeated
/// `delay_secs` later at `decay` of its level, and the repeats
/// themselves feed back, so an impulse rings as a decaying train
/// (1, decay, decay², …) until it runs off the end of the buffer —
/// the buffer never grows.
///
/// `delay_secs` is rounded to whole frames and clamps to a minimum
/// of 0.01 s; `decay` clamps to 0.0..=0.9 (even 0.9 still decays,
/// so the train always dies out). The delay is per channel: a
/// frame's echo is sourced only from the same channel
/// `delay_frames` earlier, never from a neighbouring channel.
///
/// The sum is NOT clamped: a loud source with a high decay can
/// push samples past ±1.0. Like `fade` and `normalize`, this op
/// leaves headroom to the caller — `write_wav` clamps on the way
/// out. Empty audio is a no-op.
pub fn echo(w: &mut WavData, delay_secs: f32, decay: f32) {
    let ch = w.channels as usize;
    if w.frames() == 0 || ch == 0 {
        return;
    }
    let decay = decay.clamp(0.0, 0.9);
    if decay == 0.0 {
        return;
    }
    let delay_frames = ((delay_secs.max(0.01) * w.sample_rate as f32).round() as usize).max(1);
    let stride = delay_frames * ch;
    for i in stride..w.samples.len() {
        let src = w.samples[i - stride];
        w.samples[i] += decay * src;
    }
}

/// One-pole lowpass filter in place: `y += alpha * (x - y)` per
/// channel, with `alpha = 1 - exp(-2π * cutoff / rate)` — the
/// classic RC smoothing filter. Content above `cutoff_hz` is
/// rolled off; `cutoff_hz` clamps to 20 Hz..=rate/2.
///
/// State is kept per channel (channels never bleed) and seeded
/// with each channel's first sample, so a constant (DC) signal
/// passes unchanged from the very first frame instead of ramping
/// up from zero. Empty audio and a zero sample rate are no-ops.
pub fn lowpass(w: &mut WavData, cutoff_hz: f32) {
    let ch = w.channels as usize;
    let frames = w.frames();
    if frames == 0 || ch == 0 || w.sample_rate == 0 {
        return;
    }
    let rate = w.sample_rate as f32;
    let cutoff = cutoff_hz.clamp(20.0, rate / 2.0);
    let alpha = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / rate).exp();
    let mut y: Vec<f32> = (0..ch).map(|c| w.samples[c]).collect();
    for j in 0..frames {
        for c in 0..ch {
            let i = j * ch + c;
            y[c] += alpha * (w.samples[i] - y[c]);
            w.samples[i] = y[c];
        }
    }
}

/// Peak absolute sample across the channels of frame `j` of
/// `w` — the per-frame loudness measure behind the silence
/// definition `trim_silence` and `split_at_silences` share: a
/// frame is silent exactly when this peak is below the caller's
/// threshold.
fn frame_peak(w: &WavData, ch: usize, j: usize) -> f32 {
    (0..ch).fold(0.0f32, |m, c| m.max(w.samples[j * ch + c].abs()))
}

/// Trim silent frames from both ends in place and report how many
/// frames were removed: `(front, back)`.
///
/// A frame counts as silent when its loudest channel is below
/// `threshold` (max |sample| < threshold), so a frame survives if
/// ANY channel reaches the threshold. Interior silence is kept —
/// only the leading and trailing runs are removed.
///
/// Guard: if every frame is silent the buffer is left completely
/// unchanged and `(0, 0)` is returned — this op never empties a
/// clip. (A threshold of 0 or less therefore trims nothing, since
/// no frame's peak is below it.) Empty audio returns `(0, 0)`.
pub fn trim_silence(w: &mut WavData, threshold: f32) -> (usize, usize) {
    let ch = w.channels as usize;
    let frames = w.frames();
    if frames == 0 || ch == 0 {
        return (0, 0);
    }
    let Some(first) = (0..frames).find(|&j| frame_peak(w, ch, j) >= threshold) else {
        return (0, 0); // All frames silent: leave the buffer as is.
    };
    let last = (0..frames)
        .rev()
        .find(|&j| frame_peak(w, ch, j) >= threshold)
        .unwrap();
    let front = first;
    let back = frames - 1 - last;
    if front == 0 && back == 0 {
        return (0, 0);
    }
    w.samples.drain((frames - back) * ch..);
    w.samples.drain(..front * ch);
    (front, back)
}

/// Find the sounding segments of a clip: the `(start, end)`
/// frame ranges (end exclusive) that contain non-silent audio,
/// splitting wherever a run of silent frames lasts at least
/// `min_silence_frames`.
///
/// A frame is silent when its loudest channel is below
/// `threshold` — the definition `trim_silence` uses (see
/// `frame_peak`), so a frame exactly at the threshold counts as
/// sounding. Leading and trailing silence belong to no range,
/// and a silent gap shorter than `min_silence_frames` does not
/// split: the surrounding sound merges into one range that spans
/// the gap, gap frames included. A `min_silence_frames` of 0
/// behaves as 1.
///
/// Pure analysis — the data is not modified; an editor op
/// materialises the segments (e.g. with `trim`). All-silent or
/// empty input yields an empty vec; audio with no qualifying
/// silence yields the single range `(0, frames)`.
pub fn split_at_silences(
    w: &WavData,
    threshold: f32,
    min_silence_frames: usize,
) -> Vec<(usize, usize)> {
    let ch = w.channels as usize;
    let frames = w.frames();
    if frames == 0 || ch == 0 {
        return Vec::new();
    }
    let min_gap = min_silence_frames.max(1);
    let mut ranges = Vec::new();
    // Start of the sounding segment being accumulated, the end
    // (exclusive) of its most recent sounding frame, and the
    // silent run length since that frame.
    let mut seg_start: Option<usize> = None;
    let mut seg_end = 0usize;
    let mut gap = 0usize;
    for j in 0..frames {
        if frame_peak(w, ch, j) >= threshold {
            if seg_start.is_none() {
                seg_start = Some(j);
            }
            seg_end = j + 1;
            gap = 0;
        } else if seg_start.is_some() {
            gap += 1;
            if gap >= min_gap {
                ranges.push((seg_start.take().unwrap(), seg_end));
                gap = 0;
            }
        }
    }
    if let Some(start) = seg_start {
        ranges.push((start, seg_end));
    }
    ranges
}

/// Resampling pitch shift in place: the buffer is replayed at
/// `2^(semitones / 12)` times its original rate, so positive
/// semitones raise the pitch and shorten the clip, negative
/// semitones lower it and lengthen the clip — the classic
/// "chipmunk / slow-down" shift. Pitch and duration change
/// together; the stored sample rate is untouched and no
/// time-stretching is attempted.
///
/// `semitones` clamps to -24..=24 (two octaves each way); 0.0
/// leaves the data unchanged. The output has
/// `round(frames / factor)` frames, at least 1. Resampling is by
/// linear interpolation between source frames, per channel: an
/// output frame reads only its own channel's source frames
/// (frame-stride indexing, as in `echo`), so stereo channels
/// never bleed. Empty audio is a no-op.
pub fn pitch_shift(w: &mut WavData, semitones: f32) {
    let ch = w.channels as usize;
    let frames = w.frames();
    if frames == 0 || ch == 0 {
        return;
    }
    let semitones = semitones.clamp(-24.0, 24.0);
    if semitones == 0.0 {
        return;
    }
    let factor = 2f32.powf(semitones / 12.0);
    let out_frames = ((frames as f32 / factor).round() as usize).max(1);
    let mut out = vec![0.0f32; out_frames * ch];
    for j in 0..out_frames {
        // Source position for this output frame, in source frames.
        let pos = j as f32 * factor;
        let i0 = (pos.floor() as usize).min(frames - 1);
        let i1 = (i0 + 1).min(frames - 1);
        let frac = pos - pos.floor();
        for c in 0..ch {
            let a = w.samples[i0 * ch + c];
            let b = w.samples[i1 * ch + c];
            out[j * ch + c] = a * (1.0 - frac) + b * frac;
        }
    }
    w.samples = out;
}

/// Feed-forward dynamic-range compression in place: a one-pole
/// envelope follower tracks the per-frame level (the loudest
/// channel's absolute value) and any frame whose envelope exceeds
/// `threshold` is turned down by `(threshold / envelope)^(1 -
/// 1/ratio)`; frames at or below the threshold pass at unity
/// gain. The same gain is applied to every channel of a frame,
/// so the stereo image is preserved.
///
/// The follower attacks with a ~1 ms time constant and releases
/// with a ~100 ms one, both derived from the file's sample rate.
/// `threshold` clamps to 0.05..=1.0 and `ratio` to 1.0..=20.0 (a
/// ratio of 1.0 makes the gain exponent 0, i.e. no compression).
/// The output is NOT hard-clamped — peaks can still exceed ±1.0
/// during the attack — leaving headroom to the caller, as with
/// `echo`; `write_wav` clamps on the way out. Empty audio and a
/// zero sample rate are no-ops.
pub fn compress(w: &mut WavData, threshold: f32, ratio: f32) {
    let ch = w.channels as usize;
    let frames = w.frames();
    if frames == 0 || ch == 0 || w.sample_rate == 0 {
        return;
    }
    let threshold = threshold.clamp(0.05, 1.0);
    let ratio = ratio.clamp(1.0, 20.0);
    let rate = w.sample_rate as f32;
    let attack = 1.0 - (-1.0 / (0.001 * rate)).exp();
    let release = 1.0 - (-1.0 / (0.100 * rate)).exp();
    let exponent = 1.0 - 1.0 / ratio;
    let mut env = 0.0f32;
    for j in 0..frames {
        let base = j * ch;
        let level = (0..ch).fold(0.0f32, |m, c| m.max(w.samples[base + c].abs()));
        let coeff = if level > env { attack } else { release };
        env += coeff * (level - env);
        if env > threshold {
            let g = (threshold / env).powf(exponent);
            for c in 0..ch {
                w.samples[base + c] *= g;
            }
        }
    }
}

/// Seconds to a frame index in `data`, rounded and clamped to the
/// data range — the same rounding `mix` uses for clip offsets.
fn secs_to_frame(data: &WavData, secs: f32) -> usize {
    let f = (secs.max(0.0) * data.sample_rate as f32).round() as usize;
    f.min(data.frames())
}

/// Frame-accurate slice `[start_secs, end_secs)` of `data`,
/// clamped to the data range. If `end <= start` after clamping the
/// result is an empty (0-frame) `WavData` with the same rate and
/// channel count.
pub fn trim(data: &WavData, start_secs: f32, end_secs: f32) -> WavData {
    let start = secs_to_frame(data, start_secs);
    let end = secs_to_frame(data, end_secs);
    let ch = data.channels as usize;
    let samples = if end <= start || ch == 0 {
        Vec::new()
    } else {
        data.samples[start * ch..end * ch].to_vec()
    };
    WavData {
        sample_rate: data.sample_rate,
        channels: data.channels,
        samples,
    }
}

/// Split `data` into two halves at `at_secs` (clamped to the data
/// range). The halves concatenate back to the original
/// sample-for-sample: `split_wav(d, t).0.samples ++ .1.samples ==
/// d.samples`.
pub fn split_wav(data: &WavData, at_secs: f32) -> (WavData, WavData) {
    let at = secs_to_frame(data, at_secs);
    let ch = data.channels as usize;
    let cut = (at * ch).min(data.samples.len());
    let mk = |samples: Vec<f32>| WavData {
        sample_rate: data.sample_rate,
        channels: data.channels,
        samples,
    };
    (
        mk(data.samples[..cut].to_vec()),
        mk(data.samples[cut..].to_vec()),
    )
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

/// Join `a` and `b` with a linear crossfade: `b` starts
/// `fade_secs` before `a` ends, `a` ramps down while `b` ramps up,
/// and the total length is `a + b - fade` frames.
///
/// The output takes `a`'s sample rate and channel count; `b` is
/// converted to match (resampled by linear interpolation, channels
/// mapped mono<->stereo) via the same `convert` helper `mix` uses.
/// The ramp follows `mix`'s fade convention: at fade frame `j` of
/// `F`, progress is `j / F`, so the first fade frame is still pure
/// `a` and full `b` begins with the first frame after the overlap.
/// Output is hard-clamped to -1..=1.
///
/// Errors: either side has no frames, `a` has a zero sample rate,
/// or the fade is longer than either side (after conversion).
pub fn crossfade_mix(a: &WavData, b: &WavData, fade_secs: f32) -> Result<WavData, String> {
    if a.sample_rate == 0 {
        return Err("crossfade: bad sample rate".into());
    }
    let a_frames = a.frames();
    if a_frames == 0 {
        return Err("crossfade: first clip is empty".into());
    }
    let b_buf = convert(b, a.sample_rate, a.channels);
    let ch = a.channels as usize;
    let b_frames = if ch == 0 { 0 } else { b_buf.len() / ch };
    if b_frames == 0 {
        return Err("crossfade: second clip is empty".into());
    }
    let fade = (fade_secs.max(0.0) * a.sample_rate as f32).round() as usize;
    if fade > a_frames || fade > b_frames {
        return Err(format!(
            "crossfade: fade of {fade} frames exceeds a source ({a_frames}/{b_frames} frames)"
        ));
    }
    let total = a_frames + b_frames - fade;
    let mut out = vec![0.0f32; total * ch];
    // a verbatim up to and including its tail into the fade region,
    // b verbatim after the overlap; the overlap is summed below.
    out[..a_frames * ch].copy_from_slice(&a.samples[..a_frames * ch]);
    let b_start = a_frames - fade;
    for j in fade..b_frames {
        let dst = (b_start + j) * ch;
        out[dst..dst + ch].copy_from_slice(&b_buf[j * ch..j * ch + ch]);
    }
    if fade > 0 {
        for j in 0..fade {
            let p = j as f32 / fade as f32;
            let ai = (b_start + j) * ch;
            for c in 0..ch {
                out[ai + c] = a.samples[ai + c] * (1.0 - p) + b_buf[j * ch + c] * p;
            }
        }
    }
    for s in out.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
    Ok(WavData {
        sample_rate: a.sample_rate,
        channels: a.channels,
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

/// Split a timeline clip placement at `at_frame` into two clips.
/// `dur_frames` is the clip's duration in frames, probed by the
/// caller (e.g. from `read_wav` at the project fps) — `AudioClip`
/// itself stores no duration. Returns `Some((first, second))` only
/// when `at_frame` lies strictly inside the clip's span
/// (`start_frame < at_frame < start_frame + dur_frames`); the first
/// clip keeps the original start, the second starts at `at_frame`.
///
/// Both halves keep the same `path`: this splits the *timeline
/// placement* only. Actually cutting the audio file is `trim`'s
/// job, and `AudioClip` has no source-offset field, so playback of
/// the second half still begins at the file's start — a model
/// limitation callers must be aware of.
pub fn split_clip(
    clip: &crate::model::AudioClip,
    at_frame: usize,
    dur_frames: usize,
) -> Option<(crate::model::AudioClip, crate::model::AudioClip)> {
    if at_frame <= clip.start_frame || at_frame >= clip.start_frame + dur_frames {
        return None;
    }
    Some((
        crate::model::AudioClip {
            path: clip.path.clone(),
            start_frame: clip.start_frame,
            gain: clip.gain,
            muted: false,
        },
        crate::model::AudioClip {
            path: clip.path.clone(),
            start_frame: at_frame,
            gain: clip.gain,
            muted: false,
        },
    ))
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
                gain: 1.0,
                muted: false,
            },
            AudioClip {
                path: "b.wav".into(),
                start_frame: 24,
                gain: 1.0,
                muted: false,
            },
            AudioClip {
                path: "c.wav".into(),
                start_frame: 48,
                gain: 1.0,
                muted: false,
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

    fn ramp_wav(rate: u32, frames: usize) -> WavData {
        // Distinct value per frame so slice boundaries are checkable.
        WavData {
            sample_rate: rate,
            channels: 1,
            samples: (0..frames).map(|i| i as f32 * 0.0001).collect(),
        }
    }

    #[test]
    fn peak_and_rms_of_known_signal() {
        // Exact arithmetic on a small vector:
        // squares sum to 0.5625 over 4 samples -> mean 0.140625,
        // rms = sqrt(0.140625) = 0.375 exactly.
        let data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.5, -0.5, 0.25],
        };
        assert_eq!(peak(&data), 0.5);
        assert!((rms(&data) - 0.375).abs() < 1e-6, "rms {}", rms(&data));

        // A dense 0.8-amplitude sine: peak near 0.8, rms near 0.8/sqrt(2).
        let rate = 44100u32;
        let samples: Vec<f32> = (0..rate as usize)
            .map(|i| 0.8 * (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / rate as f32).sin())
            .collect();
        let sine = WavData {
            sample_rate: rate,
            channels: 1,
            samples,
        };
        assert!((peak(&sine) - 0.8).abs() < 0.001, "peak {}", peak(&sine));
        assert!(
            (rms(&sine) - 0.8 / std::f32::consts::SQRT_2).abs() < 0.001,
            "rms {}",
            rms(&sine)
        );

        let empty = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: Vec::new(),
        };
        assert_eq!(peak(&empty), 0.0);
        assert_eq!(rms(&empty), 0.0);
    }

    #[test]
    fn normalize_scales_to_target_peak() {
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.25, -0.125, 0.1, -0.05],
        };
        let gain = normalize(&mut data, 0.9);
        assert!((gain - 3.6).abs() < 1e-5, "gain {gain}");
        assert!((peak(&data) - 0.9).abs() < 1e-6);
        assert!((data.samples[1] - (-0.45)).abs() < 1e-6);

        // Target above 1.0 clamps to full scale.
        let mut data2 = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.5, -0.25],
        };
        let gain2 = normalize(&mut data2, 2.0);
        assert!((gain2 - 2.0).abs() < 1e-6);
        assert!((peak(&data2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_silent_is_noop_returning_unity() {
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.0, 0.0],
        };
        assert_eq!(normalize(&mut data, 0.9), 1.0);
        assert_eq!(data.samples, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn fade_ramps_edges_to_zero_and_keeps_middle_at_full_scale() {
        // 1 s of constant 1.0 at 8000 Hz; 0.25 s fades = 2000 frames.
        let mut data = const_wav(8000, 1.0, 1.0);
        fade(&mut data, 0.25, 0.25);
        let at = |f: usize| data.samples[f];
        assert_eq!(at(0), 0.0, "fade-in edge is silent");
        assert!((at(1000) - 0.5).abs() < 1e-6, "halfway up: {}", at(1000));
        assert_eq!(at(2000), 1.0, "fade-in complete");
        assert_eq!(at(4000), 1.0, "middle is untouched full scale");
        assert_eq!(at(6000), 1.0, "fade-out begins at full scale");
        assert!(
            at(7999).abs() < 0.001,
            "fade-out edge is ~silent: {}",
            at(7999)
        );
    }

    #[test]
    fn fade_zero_lengths_leave_samples_untouched() {
        let mut data = const_wav(8000, 0.1, 0.5);
        let before = data.samples.clone();
        fade(&mut data, 0.0, 0.0);
        fade(&mut data, -1.0, -1.0); // negative lengths clamp to zero
        assert_eq!(data.samples, before);
    }

    #[test]
    fn fade_longer_than_the_clip_clamps_to_the_buffer() {
        // A 5 s fade-in requested on a 1 s clip becomes a ramp over
        // the whole clip instead of running past the end.
        let mut data = const_wav(8000, 1.0, 1.0);
        fade(&mut data, 5.0, 0.0);
        assert_eq!(data.samples[0], 0.0);
        assert!((data.samples[4000] - 0.5).abs() < 1e-6);
        assert!(
            (data.samples[7999] - 7999.0 / 8000.0).abs() < 1e-5,
            "ramp stops at the buffer end: {}",
            data.samples[7999]
        );
    }

    #[test]
    fn gain_plus_6db_roughly_doubles_and_clips() {
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.25, -0.25, 0.6, -0.6],
        };
        let factor = gain(&mut data, 6.0);
        assert!((factor - 1.995_262_3).abs() < 0.001, "factor {factor}");
        assert!(
            (data.samples[0] - 0.5).abs() < 0.002,
            "0.25 at +6 dB should land near 0.5: {}",
            data.samples[0]
        );
        assert!((data.samples[1] + 0.5).abs() < 0.002);
        // 0.6 * ~2 = ~1.2 hard-clips to full scale, both signs.
        assert_eq!(data.samples[2], 1.0);
        assert_eq!(data.samples[3], -1.0);
    }

    #[test]
    fn gain_minus_6db_roughly_halves_and_zero_db_is_unity() {
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.5, -0.5],
        };
        let factor = gain(&mut data, -6.0);
        assert!((factor - 0.501_187_2).abs() < 0.001, "factor {factor}");
        assert!((data.samples[0] - 0.25).abs() < 0.002);
        assert!((data.samples[1] + 0.25).abs() < 0.002);

        let mut data2 = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.5, -0.5],
        };
        assert_eq!(gain(&mut data2, 0.0), 1.0);
        assert_eq!(data2.samples, vec![0.5, -0.5]);
    }

    #[test]
    fn reverse_flips_frame_order() {
        // ramp_wav values are distinct per frame (i * 0.0001).
        let mut data = ramp_wav(8000, 100);
        let original = data.samples.clone();
        reverse(&mut data);
        let expected: Vec<f32> = original.iter().rev().copied().collect();
        assert_eq!(data.samples, expected);
        assert_eq!(data.samples[0], original[99]);
        assert_eq!(data.samples[99], 0.0);
        // Reversing twice restores the original exactly.
        reverse(&mut data);
        assert_eq!(data.samples, original);
    }

    #[test]
    fn reverse_keeps_stereo_channels_within_their_frame() {
        // Frames (0.1, 0.2), (0.3, 0.4), (0.5, 0.6): time reverses
        // but left/right inside each frame must not swap.
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        };
        reverse(&mut data);
        assert_eq!(data.samples, vec![0.5, 0.6, 0.3, 0.4, 0.1, 0.2]);
    }

    #[test]
    fn fade_gain_reverse_on_empty_audio_are_safe() {
        for channels in [1u16, 2] {
            let mut data = WavData {
                sample_rate: 8000,
                channels,
                samples: Vec::new(),
            };
            fade(&mut data, 0.5, 0.5);
            assert_eq!(gain(&mut data, 12.0), 10f32.powf(12.0 / 20.0));
            reverse(&mut data);
            assert!(data.samples.is_empty());
            assert_eq!(data.frames(), 0);
        }
    }

    #[test]
    fn trim_is_frame_accurate_and_clamped() {
        let data = ramp_wav(8000, 8000); // 1 second
        let t = trim(&data, 0.25, 0.5);
        assert_eq!(t.frames(), 2000);
        assert_eq!(t.sample_rate, 8000);
        assert_eq!(t.channels, 1);
        assert_eq!(t.samples[0], data.samples[2000]);
        assert_eq!(t.samples[1999], data.samples[3999]);

        // Out-of-range requests clamp to the data.
        assert_eq!(trim(&data, -1.0, 99.0).frames(), 8000);
        // end <= start -> empty, but rate/channels preserved.
        let empty = trim(&data, 0.5, 0.5);
        assert_eq!(empty.frames(), 0);
        assert_eq!(empty.sample_rate, 8000);
        assert_eq!(empty.channels, 1);
        assert_eq!(trim(&data, 0.75, 0.25).frames(), 0);
    }

    #[test]
    fn split_wav_concatenates_back_exactly() {
        let data = ramp_wav(8000, 8000);
        let (a, b) = split_wav(&data, 0.3);
        assert_eq!(a.frames(), 2400);
        assert_eq!(b.frames(), 5600);
        let mut joined = a.samples.clone();
        joined.extend_from_slice(&b.samples);
        assert_eq!(joined, data.samples, "halves must reassemble exactly");

        // Splits at the extremes leave one side empty.
        let (a0, b0) = split_wav(&data, 0.0);
        assert_eq!(a0.frames(), 0);
        assert_eq!(b0.samples, data.samples);
        let (a1, b1) = split_wav(&data, 5.0);
        assert_eq!(a1.samples, data.samples);
        assert_eq!(b1.frames(), 0);
    }

    #[test]
    fn crossfade_midpoint_averages_and_length_formula_holds() {
        // 0.2 s each at 8000 Hz, fade 0.1 s (800 frames):
        // total = 1600 + 1600 - 800 = 2400 frames.
        let a = const_wav(8000, 0.2, 0.4);
        let b = const_wav(8000, 0.2, 0.8);
        let out = crossfade_mix(&a, &b, 0.1).unwrap();
        assert_eq!(out.frames(), 2400);
        assert_eq!(out.sample_rate, 8000);
        let at = |f: usize| out.samples[f];
        assert!((at(0) - 0.4).abs() < 1e-6, "pure a before fade: {}", at(0));
        assert!((at(799) - 0.4).abs() < 1e-6);
        // Fade runs frames 800..1600; frame 1200 is p = 0.5 exactly.
        assert!(
            (at(1200) - 0.6).abs() < 1e-6,
            "midpoint should average the sources: {}",
            at(1200)
        );
        assert!((at(1600) - 0.8).abs() < 1e-6, "pure b after fade: {}", at(1600));
        assert!((at(2399) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn crossfade_rejects_overlong_fade_and_converts_channels() {
        let a = const_wav(8000, 0.2, 0.4);
        let b = const_wav(8000, 0.2, 0.8);
        assert!(crossfade_mix(&a, &b, 0.3).is_err(), "fade longer than a source");

        // Stereo b (0.8 on both channels) into mono a: averaged to
        // the same 0.8, so the midpoint still averages 0.4 and 0.8.
        let stereo_b = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.8; 1600 * 2],
        };
        let out = crossfade_mix(&a, &stereo_b, 0.1).unwrap();
        assert_eq!(out.channels, 1);
        assert_eq!(out.frames(), 2400);
        assert!((out.samples[1200] - 0.6).abs() < 1e-5);
    }

    #[test]
    fn split_clip_boundaries() {
        // Clip spans frames 24..72 (start 24, probed duration 48).
        let clip = AudioClip {
            path: "x.wav".into(),
            start_frame: 24,
            gain: 1.0,
            muted: false,
        };
        let (first, second) = split_clip(&clip, 48, 48).unwrap();
        assert_eq!(first.start_frame, 24);
        assert_eq!(first.path, "x.wav");
        assert_eq!(second.start_frame, 48);
        assert_eq!(second.path, "x.wav");

        // At the start, at the end, past the end, before the start:
        // none of these are strictly inside the span.
        assert!(split_clip(&clip, 24, 48).is_none());
        assert!(split_clip(&clip, 72, 48).is_none());
        assert!(split_clip(&clip, 200, 48).is_none());
        assert!(split_clip(&clip, 10, 48).is_none());
        // Zero duration has no inside.
        assert!(split_clip(&clip, 48, 0).is_none());
    }

    #[test]
    fn echo_repeats_an_impulse_with_decay_and_no_channel_bleed() {
        // Stereo at 8000 Hz; the minimum delay (0.01 s) is 80
        // frames. A left-only impulse rings in the left channel
        // only: 1.0, then 0.5, then 0.25 — the second repeat
        // exists only because the first repeat feeds back.
        let frames = 240;
        let mut samples = vec![0.0f32; frames * 2];
        samples[0] = 1.0; // frame 0, left
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples,
        };
        echo(&mut data, 0.01, 0.5);
        let left = |f: usize| data.samples[f * 2];
        assert_eq!(left(0), 1.0);
        assert_eq!(left(80), 0.5);
        assert_eq!(left(160), 0.25);
        // Nothing rings between the repeats.
        assert_eq!(left(79), 0.0);
        assert_eq!(left(81), 0.0);
        // Right channel indices are the odd ones: all still zero.
        assert!(
            data.samples.iter().skip(1).step_by(2).all(|s| *s == 0.0),
            "right channel must stay silent — no cross-channel bleed"
        );
    }

    #[test]
    fn echo_clamps_delay_and_decay() {
        // delay_secs 0 clamps up to the 0.01 s minimum (80 frames
        // at 8000 Hz) and decay 2.0 clamps down to 0.9.
        let mut samples = vec![0.0f32; 200];
        samples[0] = 1.0;
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        echo(&mut data, 0.0, 2.0);
        assert!(
            (data.samples[80] - 0.9).abs() < 1e-6,
            "clamped echo at frame 80: {}",
            data.samples[80]
        );
        assert_eq!(data.samples[79], 0.0);
    }

    #[test]
    fn lowpass_passes_a_constant_signal_unchanged() {
        // State is seeded with the first sample, so DC passes from
        // frame 0 even at a low cutoff.
        let mut data = const_wav(44100, 0.1, 0.7);
        lowpass(&mut data, 100.0);
        assert!(
            data.samples.iter().all(|s| (s - 0.7).abs() < 1e-6),
            "constant signal must pass ~unchanged"
        );
    }

    #[test]
    fn lowpass_strongly_attenuates_a_nyquist_alternation() {
        // +1/-1 every sample is the fastest possible alternation
        // (Nyquist). Through a 100 Hz one-pole at 44100 Hz the
        // steady-state amplitude is alpha/(2-alpha) ≈ 0.007.
        let rate = 44100u32;
        let samples: Vec<f32> = (0..rate as usize)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut data = WavData {
            sample_rate: rate,
            channels: 1,
            samples,
        };
        lowpass(&mut data, 100.0);
        let tail_peak = data.samples[rate as usize - 1000..]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            tail_peak < 0.02,
            "alternation should be ~gone after settling, tail peak {tail_peak}"
        );
    }

    #[test]
    fn trim_silence_removes_exact_counts() {
        // Mono: 3 silent frames, 2 loud, 2 silent.
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.0, 0.0, 0.5, 0.25, 0.0, 0.0],
        };
        assert_eq!(trim_silence(&mut data, 0.01), (3, 2));
        assert_eq!(data.samples, vec![0.5, 0.25]);

        // Stereo: a frame survives if ANY channel reaches the
        // threshold. Frame 1 peaks at 0.005 < 0.01, so it is
        // trimmed despite being non-zero.
        let mut stereo = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![
                0.0, 0.0, // silent
                0.0, 0.005, // below threshold -> silent
                0.3, 0.0, // loud (left channel only)
                0.0, 0.0, // silent
            ],
        };
        assert_eq!(trim_silence(&mut stereo, 0.01), (2, 1));
        assert_eq!(stereo.samples, vec![0.3, 0.0]);
        assert_eq!(stereo.frames(), 1);
    }

    #[test]
    fn trim_silence_never_empties_a_clip() {
        // All-silent input is left completely unchanged.
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.001, 0.0, 0.0],
        };
        let before = data.samples.clone();
        assert_eq!(trim_silence(&mut data, 0.01), (0, 0));
        assert_eq!(data.samples, before);

        // No silence at the edges -> nothing removed, and the
        // interior silent frame is kept.
        let mut loud = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.5, 0.0, 0.5],
        };
        assert_eq!(trim_silence(&mut loud, 0.01), (0, 0));
        assert_eq!(loud.samples, vec![0.5, 0.0, 0.5]);
    }

    #[test]
    fn echo_lowpass_trim_silence_on_empty_audio_are_safe() {
        for channels in [1u16, 2] {
            let mut data = WavData {
                sample_rate: 8000,
                channels,
                samples: Vec::new(),
            };
            echo(&mut data, 0.3, 0.5);
            lowpass(&mut data, 1000.0);
            assert_eq!(trim_silence(&mut data, 0.01), (0, 0));
            assert!(data.samples.is_empty());
        }
    }

    #[test]
    fn pitch_shift_zero_semitones_is_identity() {
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.1, -0.2, 0.3, -0.4, 0.5, -0.6],
        };
        let before = data.samples.clone();
        pitch_shift(&mut data, 0.0);
        assert_eq!(data.samples, before);
        assert_eq!(data.frames(), 3);
    }

    #[test]
    fn pitch_shift_up_an_octave_halves_frames_and_picks_source_frames() {
        // Mono ramp 0.0, 0.1, ..., 0.7. +12 semitones doubles the
        // replay rate, so output frame j reads source frame 2j
        // exactly (fraction 0): [0.0, 0.2, 0.4, 0.6].
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: (0..8).map(|i| i as f32 * 0.1).collect(),
        };
        pitch_shift(&mut data, 12.0);
        assert_eq!(data.frames(), 4, "8 frames up an octave -> 4");
        let expected = [0.0f32, 0.2, 0.4, 0.6];
        for (got, want) in data.samples.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
        }
    }

    #[test]
    fn pitch_shift_down_an_octave_doubles_frames_and_interpolates() {
        // [0.0, 1.0] at half rate gives 4 frames at source
        // positions 0, 0.5, 1.0, 1.5; the last position clamps to
        // the final source frame: [0.0, 0.5, 1.0, 1.0].
        let mut data = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 1.0],
        };
        pitch_shift(&mut data, -12.0);
        assert_eq!(data.frames(), 4, "2 frames down an octave -> 4");
        let expected = [0.0f32, 0.5, 1.0, 1.0];
        for (got, want) in data.samples.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
        }
    }

    #[test]
    fn pitch_shift_clamps_semitones_to_two_octaves() {
        // +48 semitones clamps to +24: factor 4, so 8 frames -> 2,
        // reading source frames 0 and 4.
        let mut up = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: (0..8).map(|i| i as f32 * 0.1).collect(),
        };
        pitch_shift(&mut up, 48.0);
        assert_eq!(up.frames(), 2);
        assert!((up.samples[0] - 0.0).abs() < 1e-6);
        assert!((up.samples[1] - 0.4).abs() < 1e-6);
        // -48 clamps to -24: factor 1/4, so 8 frames -> 32.
        let mut down = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: (0..8).map(|i| i as f32 * 0.1).collect(),
        };
        pitch_shift(&mut down, -48.0);
        assert_eq!(down.frames(), 32);
    }

    #[test]
    fn pitch_shift_stereo_keeps_channels_separate() {
        // Left ramps 0.0..0.3, right sits at 0.9. Up an octave the
        // output keeps source frames 0 and 2, each channel read
        // from its own source channel only.
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.0, 0.9, 0.1, 0.9, 0.2, 0.9, 0.3, 0.9],
        };
        pitch_shift(&mut data, 12.0);
        assert_eq!(data.frames(), 2);
        let expected = [0.0f32, 0.9, 0.2, 0.9];
        for (got, want) in data.samples.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
        }
    }

    #[test]
    fn compress_leaves_a_quiet_signal_untouched() {
        // Constant 0.2 never pushes the envelope past the 0.4
        // threshold, so every frame passes at unity gain and the
        // samples are bit-identical.
        let mut data = const_wav(8000, 0.1, 0.2);
        compress(&mut data, 0.4, 4.0);
        assert!(data.samples.iter().all(|s| *s == 0.2));
    }

    #[test]
    fn compress_reduces_a_loud_constant_signal_by_the_expected_gain() {
        // Constant 0.8, threshold 0.4, ratio 2. Once the envelope
        // settles at 0.8 the gain is (0.4 / 0.8)^(1 - 1/2) =
        // sqrt(0.5) ≈ 0.7071, so the output settles at ≈ 0.5657.
        let mut data = const_wav(8000, 0.1, 0.8);
        compress(&mut data, 0.4, 2.0);
        // Frame 0: the ~1 ms attack at 8000 Hz lifts the envelope
        // only to 0.8 * (1 - exp(-0.125)) ≈ 0.094 < 0.4, so the
        // first frame still passes at unity gain.
        assert_eq!(data.samples[0], 0.8);
        let last = *data.samples.last().unwrap();
        assert!(
            (last - 0.5657).abs() < 0.005,
            "settled output should be ≈ 0.5657, got {last}"
        );
        // Every settled frame is reduced, none is boosted or zeroed.
        assert!(data.samples[400..].iter().all(|s| *s > 0.5 && *s < 0.8));
    }

    #[test]
    fn compress_stereo_applies_one_gain_to_both_channels() {
        // Frames (0.8, 0.4): the frame level is the max, 0.8, so
        // both channels take the loud-signal gain sqrt(0.5) once
        // settled — left ≈ 0.5657, right ≈ 0.2828 — and the 2:1
        // channel balance is preserved.
        let frames = 800;
        let mut samples = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            samples.push(0.8);
            samples.push(0.4);
        }
        let mut data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples,
        };
        compress(&mut data, 0.4, 2.0);
        let (l, r) = (data.samples[(frames - 1) * 2], data.samples[(frames - 1) * 2 + 1]);
        assert!((l - 0.5657).abs() < 0.005, "left settled: {l}");
        assert!((r - 0.2828).abs() < 0.005, "right settled: {r}");
        assert!((l / r - 2.0).abs() < 0.01, "balance preserved: {}", l / r);
    }

    #[test]
    fn pitch_shift_and_compress_on_empty_audio_are_safe() {
        for channels in [1u16, 2] {
            let mut data = WavData {
                sample_rate: 8000,
                channels,
                samples: Vec::new(),
            };
            pitch_shift(&mut data, 12.0);
            pitch_shift(&mut data, -12.0);
            compress(&mut data, 0.4, 4.0);
            assert!(data.samples.is_empty());
        }
    }

    fn mono_frames(values: &[f32]) -> WavData {
        WavData {
            sample_rate: 8000,
            channels: 1,
            samples: values.to_vec(),
        }
    }

    #[test]
    fn split_at_silences_finds_bursts_separated_by_a_long_gap() {
        // Threshold 0.1, minimum silence 3. Layout by frame:
        //   0..2   silence (leading)
        //   2..5   burst A
        //   5..9   silence, 4 frames >= 3 -> split
        //   9..11  burst B
        //   11..13 silence (trailing)
        let data = mono_frames(&[
            0.0, 0.0, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.6, 0.6, 0.0, 0.0,
        ]);
        let before = data.samples.clone();
        assert_eq!(split_at_silences(&data, 0.1, 3), vec![(2, 5), (9, 11)]);
        // Pure analysis: the buffer is untouched.
        assert_eq!(data.samples, before);
    }

    #[test]
    fn split_at_silences_merges_across_a_gap_below_the_minimum() {
        // A 2-frame silent gap with a minimum of 3 does not
        // split: one range spans the gap frames too.
        let data = mono_frames(&[0.5, 0.5, 0.0, 0.0, 0.5, 0.5]);
        assert_eq!(split_at_silences(&data, 0.1, 3), vec![(0, 6)]);
        // A gap of exactly the minimum does split.
        let data = mono_frames(&[0.5, 0.0, 0.0, 0.0, 0.5]);
        assert_eq!(split_at_silences(&data, 0.1, 3), vec![(0, 1), (4, 5)]);
    }

    #[test]
    fn split_at_silences_excludes_leading_and_trailing_silence() {
        // Even with a minimum no gap could ever reach, the
        // ranges cover only the sounding frames.
        let data = mono_frames(&[0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0]);
        assert_eq!(split_at_silences(&data, 0.1, 100), vec![(3, 5)]);
    }

    #[test]
    fn split_at_silences_all_silent_and_empty_inputs_are_empty() {
        // Below-threshold noise still counts as silence.
        let data = mono_frames(&[0.0, 0.05, 0.0, 0.09]);
        assert!(split_at_silences(&data, 0.1, 1).is_empty());
        let empty = mono_frames(&[]);
        assert!(split_at_silences(&empty, 0.1, 1).is_empty());
    }

    #[test]
    fn split_at_silences_never_silent_audio_is_one_full_range() {
        let data = mono_frames(&[0.5, 0.5, 0.5, 0.5, 0.5]);
        assert_eq!(split_at_silences(&data, 0.1, 2), vec![(0, 5)]);
    }

    #[test]
    fn split_at_silences_treats_a_frame_at_the_threshold_as_sounding() {
        // Peaks of exactly 0.5 with threshold 0.5 are NOT silent
        // (silence is peak < threshold, as in trim_silence), so
        // the 2-frame gap between them reaches the minimum of 2
        // and splits.
        let data = mono_frames(&[0.5, 0.0, 0.0, 0.5]);
        assert_eq!(split_at_silences(&data, 0.5, 2), vec![(0, 1), (3, 4)]);
        // A minimum of 0 behaves as 1: a lone silent frame
        // splits.
        let data = mono_frames(&[0.5, 0.0, 0.5]);
        assert_eq!(split_at_silences(&data, 0.1, 0), vec![(0, 1), (2, 3)]);
    }

    #[test]
    fn split_at_silences_stereo_uses_the_loudest_channel() {
        // A frame is silent only when BOTH channels are below
        // the threshold. Frames: silent, right-only loud,
        // silent, silent, left-only loud; the 2-frame gap
        // reaches the minimum of 2 and splits.
        let data = WavData {
            sample_rate: 8000,
            channels: 2,
            samples: vec![
                0.0, 0.0, // silent
                0.0, 0.8, // loud (right only)
                0.0, 0.0, // silent
                0.0, 0.0, // silent
                0.7, 0.0, // loud (left only)
            ],
        };
        assert_eq!(split_at_silences(&data, 0.1, 2), vec![(1, 2), (4, 5)]);
    }
}
