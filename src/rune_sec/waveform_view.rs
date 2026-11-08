//! Waveform view for the Rune editor shell (Phase D,
//! section 16 of the GPUI→Rune migration): one canvas drawing
//! the last audio clip's waveform, with a playhead line at the
//! current frame. Bin-crate code — not part of the engine
//! library — and deliberately free of `crate::` references so it
//! can be mounted wherever the shell declares it.
//!
//! ## Data source (no decoding is invented here)
//!
//! The GPUI app obtained peaks exactly one way, and this module
//! keeps that path: `probe_audio_peaks` (deleted `src/app.rs`
//! at commit `1ea3633`, line 5403) read the clip's file with
//! `crate::audio_edit::read_wav` (`src/audio_edit.rs:41`) and
//! reduced it with `crate::audio_edit::peaks`
//! (`src/audio_edit.rs:537` — max-abs amplitude per bucket
//! across all channels), keeping `WavData::duration_secs`
//! (`src/audio_edit.rs:31`) alongside. The view-side peaks cache
//! the GPUI app kept lived in the deleted `EditorView`, not in
//! the core, so — like the GPUI panel's overview strip, which
//! re-read the WAV on every render — this builder reads the
//! file fresh. An unreadable file yields no waveform (centre
//! line only), the GPUI panel's "no error UI" behaviour.
//!
//! Which clip: the core's audio ops are the `audio_*_last`
//! family (e.g. `src/editor.rs:2913`), so the strip shows
//! `project.audio.last()` — the same clip those ops would edit.
//!
//! ## Drawing
//!
//! Peaks are requested at 4× the display column count and
//! reduced by [`reduce_columns`] to one `(min, max)` pair per
//! column. Each column draws two mirrored bars: the outer bar
//! at `max` (accent, translucent) and the inner bar at `min`
//! (accent, opaque) — so both envelope extremes stay visible
//! after reduction. Amplitudes are scaled by the clip's `gain`
//! (`src/model.rs:505`, `AudioClip::gain`) and clamped to the
//! strip's half-height. The playhead fraction mirrors the GPUI
//! strip's `AudioStripData::playhead` computation: the current
//! frame mapped into the clip's span
//! `start_frame .. start_frame + duration_secs * fps`.

use rune::{canvas, Color, El, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::AudioClip;

/// Waveform accent (the 2.0 palette's `C_ACCENT`).
const WAVE_HEX: u32 = 0x6D8CFF;
/// Centre line / border tone (the 2.0 palette's border).
const CENTRE_HEX: u32 = 0x303744;
/// Playhead tone (the stage's selected-joint amber).
const PLAYHEAD_HEX: u32 = 0xFFC65D;

/// Convert a `0xRRGGBB` colour to a Rune colour (the same
/// helper `stage_view` carries; duplicated so this section
/// file is self-contained).
fn hex_color(rgb: u32) -> Option<Color> {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF))
}

/// Reduce a peak slice to exactly `columns` `(min, max)` pairs
/// by integer partition: column `i` covers
/// `peaks[i * len / columns .. (i + 1) * len / columns]`, so
/// every peak lands in exactly one column (the partition the
/// deleted `ui::waveform::downsample_peaks` used). Deliberate
/// edge cases:
///
/// - `columns == 0` yields an empty vec.
/// - An empty `peaks` slice yields `columns` `(0.0, 0.0)`
///   pairs, so a silent/unknown clip still has the right shape.
/// - When `peaks` is shorter than `columns`, some columns cover
///   no peaks at all; they read `(0.0, 0.0)`.
/// - Non-finite peaks are read as `0.0` rather than poisoning
///   the min/max fold.
pub fn reduce_columns(peaks: &[f32], columns: usize) -> Vec<(f32, f32)> {
    if columns == 0 {
        return Vec::new();
    }
    let len = peaks.len();
    let mut out = Vec::with_capacity(columns);
    for i in 0..columns {
        let start = i * len / columns;
        let end = (i + 1) * len / columns;
        let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in &peaks[start..end] {
            let v = if v.is_finite() { v } else { 0.0 };
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
        out.push(if min <= max { (min, max) } else { (0.0, 0.0) });
    }
    out
}

/// The vertical span `(top, bottom)`, in element points, of a
/// mirrored waveform bar for amplitude `amp` scaled by the
/// clip's `gain`, centred on `mid` with at most `half` points
/// of travel. The gained amplitude is clamped to `0.0..=1.0`,
/// so an over-unity gain saturates at the strip edge instead
/// of drawing outside it.
pub fn bar_span(amp: f32, gain: f32, mid: f32, half: f32) -> (f32, f32) {
    let a = (amp * gain).clamp(0.0, 1.0) * half;
    (mid - a, mid + a)
}

/// Playhead position as a fraction of the strip width, when
/// the editor's current frame falls inside the clip's span
/// (`start_frame .. start_frame + duration_secs * fps`,
/// duration rounded to whole frames) — the GPUI strip's
/// computation, verbatim in structure. `None` when the clip
/// has no positive frame span or the frame is outside it.
pub fn playhead_fraction(
    clip: &AudioClip,
    duration_secs: f32,
    fps: f32,
    current: usize,
) -> Option<f32> {
    let dur_frames = (duration_secs * fps).round() as i64;
    let start = clip.start_frame as i64;
    let current = current as i64;
    if dur_frames > 0 && current >= start && current < start + dur_frames {
        Some((current - start) as f32 / dur_frames as f32)
    } else {
        None
    }
}

/// Probe one clip's peaks the way the GPUI app did: read the
/// WAV via `audio_edit::read_wav`, reduce with
/// `audio_edit::peaks` at `buckets` buckets, and return the
/// peaks plus the clip's duration in seconds. `None` when the
/// file cannot be read (missing, not PCM16 WAV, …) — callers
/// draw no waveform for that clip.
pub fn clip_peaks(clip: &AudioClip, buckets: usize) -> Option<(Vec<f32>, f32)> {
    let data = stykanimate_rs::audio_edit::read_wav(&clip.path).ok()?;
    let duration_secs = data.duration_secs();
    Some((stykanimate_rs::audio_edit::peaks(&data, buckets), duration_secs))
}

/// The waveform strip as one `Shape` in canvas-element points,
/// for the last audio clip in the project. Layout: one column
/// per ~3 points of width; bars are drawn with
/// [`LineCap::Butt`] so a bar's ends land exactly on its
/// amplitude (round caps would overshoot by half the bar
/// width). With no clips — or an unreadable clip file — the
/// shape is just the centre line, so the strip reads as an
/// empty audio lane rather than a hole in the layout.
pub fn waveform_shape(core: &EditorCore, el_size: [f32; 2]) -> Shape {
    let (w, h) = (el_size[0], el_size[1]);
    let mut shape = Shape::new();
    if w <= 0.0 || h <= 0.0 {
        return shape;
    }
    let mid = h / 2.0;
    let centre = hex_color(CENTRE_HEX).unwrap_or(Color::WHITE);
    shape = shape.line([0.0, mid], [w, mid], 1.0, LineCap::Butt, centre);

    let Some(clip) = core.project.audio.last() else {
        return shape;
    };
    // One column per ~3 points (2 pt bar + 1 pt gap), capped
    // so a very wide strip stays cheap; peaks probed at 4× the
    // column count so the reduction still sees local extremes.
    let columns = ((w / 3.0).floor() as usize).clamp(1, 512);
    let buckets = (columns * 4).clamp(96, 2048);
    let Some((peaks, duration_secs)) = clip_peaks(clip, buckets) else {
        return shape;
    };
    let cols = reduce_columns(&peaks, columns);
    let accent = hex_color(WAVE_HEX).unwrap_or(Color::WHITE);
    let outer = accent.alpha(0.5);
    let pitch = w / columns as f32;
    let bar_w = (pitch - 1.0).max(1.0);
    for (i, &(min, max)) in cols.iter().enumerate() {
        let x = (i as f32 + 0.5) * pitch;
        let (top, bottom) = bar_span(max, clip.gain, mid, mid);
        shape = shape.line([x, top], [x, bottom], bar_w, LineCap::Butt, outer);
        let (top, bottom) = bar_span(min, clip.gain, mid, mid);
        shape = shape.line([x, top], [x, bottom], bar_w, LineCap::Butt, accent);
    }
    if let Some(frac) = playhead_fraction(clip, duration_secs, core.project.fps, core.current) {
        let x = frac * w;
        let playhead = hex_color(PLAYHEAD_HEX).unwrap_or(Color::WHITE);
        shape = shape.line([x, 0.0], [x, h], 2.0, LineCap::Butt, playhead);
    }
    shape
}

/// The waveform strip as a canvas element (key `waveform`, so
/// headless `--tree` checks and the shell's layout reads can
/// find it), sized by the caller like the stage canvas.
pub fn waveform_el(core: &EditorCore, el_size: [f32; 2]) -> El {
    canvas(waveform_shape(core, el_size)).key("waveform")
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::audio_edit::{peaks, WavData};

    fn clip(start_frame: usize, gain: f32) -> AudioClip {
        AudioClip {
            path: "unused.wav".to_string(),
            start_frame,
            gain,
        }
    }

    #[test]
    fn reduce_columns_hand_computed() {
        // Integer partition of 4 peaks into 2 columns:
        // [0.0, 1.0] and [0.5, 0.25].
        let cols = reduce_columns(&[0.0, 1.0, 0.5, 0.25], 2);
        assert_eq!(cols, vec![(0.0, 1.0), (0.25, 0.5)]);
        // Identity when columns == len.
        let cols = reduce_columns(&[0.3, 0.9], 2);
        assert_eq!(cols, vec![(0.3, 0.3), (0.9, 0.9)]);
    }

    #[test]
    fn reduce_columns_flat_input_is_flat() {
        let cols = reduce_columns(&[0.2; 7], 3);
        assert_eq!(cols, vec![(0.2, 0.2); 3]);
    }

    #[test]
    fn reduce_columns_edge_cases() {
        assert!(reduce_columns(&[1.0, 2.0], 0).is_empty());
        // Empty peaks: right shape, all silence.
        assert_eq!(reduce_columns(&[], 4), vec![(0.0, 0.0); 4]);
        // Fewer peaks than columns: uncovered columns are 0.
        let cols = reduce_columns(&[0.6], 3);
        assert_eq!(cols.len(), 3);
        assert!(cols.iter().all(|&c| c == (0.0, 0.0) || c == (0.6, 0.6)));
        assert!(cols.iter().any(|&c| c == (0.6, 0.6)));
        // Non-finite peaks read as 0.0, not NaN poisoning.
        let cols = reduce_columns(&[f32::NAN, 0.5], 1);
        assert_eq!(cols, vec![(0.0, 0.5)]);
    }

    #[test]
    fn reduce_columns_of_sine_peaks() {
        // One second of 440 Hz sine at amplitude 0.8, mono.
        let samples: Vec<f32> = (0..8000)
            .map(|i| {
                let t = i as f32 / 8000.0;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.8
            })
            .collect();
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let p = peaks(&wav, 10);
        assert_eq!(p.len(), 10);
        // Every bucket spans many cycles, so each peak sits
        // just under the 0.8 crest (discrete sampling).
        for v in &p {
            assert!(*v > 0.75 && *v <= 0.8001, "sine peak out of range: {v}");
        }
        let cols = reduce_columns(&p, 5);
        assert_eq!(cols.len(), 5);
        for (min, max) in cols {
            assert!(min > 0.75 && max <= 0.8001 && max - min < 0.05);
        }
    }

    #[test]
    fn reduce_columns_of_flat_peaks() {
        // Constant 0.3 DC: every bucket peaks at exactly 0.3.
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.3; 4000],
        };
        let p = peaks(&wav, 8);
        assert!(p.iter().all(|&v| (v - 0.3).abs() < 1e-6));
        let cols = reduce_columns(&p, 4);
        assert!(cols.iter().all(|&c| c == (0.3, 0.3)));
    }

    #[test]
    fn bar_span_scales_and_clamps_gain() {
        // Unity gain: amplitude maps linearly onto the half.
        assert_eq!(bar_span(0.5, 1.0, 24.0, 24.0), (12.0, 36.0));
        // Gain 2 doubles until it saturates at the strip edge.
        assert_eq!(bar_span(0.25, 2.0, 24.0, 24.0), (12.0, 36.0));
        assert_eq!(bar_span(0.75, 2.0, 24.0, 24.0), (0.0, 48.0));
        // Zero gain / zero amplitude collapse to the centre.
        assert_eq!(bar_span(0.9, 0.0, 24.0, 24.0), (24.0, 24.0));
        assert_eq!(bar_span(0.0, 3.0, 24.0, 24.0), (24.0, 24.0));
    }

    #[test]
    fn playhead_fraction_inside_and_outside_the_span() {
        let c = clip(10, 1.0);
        // 2 s at 30 fps = 60 frames: span is 10..70.
        assert_eq!(playhead_fraction(&c, 2.0, 30.0, 10), Some(0.0));
        assert_eq!(playhead_fraction(&c, 2.0, 30.0, 40), Some(0.5));
        assert_eq!(playhead_fraction(&c, 2.0, 30.0, 9), None);
        assert_eq!(playhead_fraction(&c, 2.0, 30.0, 70), None);
        // Zero-length clips have no span.
        assert_eq!(playhead_fraction(&c, 0.0, 30.0, 10), None);
    }
}
