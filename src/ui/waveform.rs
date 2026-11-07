//! Audio waveform strip: a canvas that draws one vertical
//! bar per peak bucket, with the already-played portion in
//! the accent colour.
//!
//! Presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`]). The
//! audio panel in `app.rs` pairs the strip with a bucket
//! count and a playback progress fraction; where the peaks
//! come from is the caller's business.
//!
//! The data half ([`downsample_peaks`]) is a pure function
//! over a plain sample slice. For a whole [`crate::audio_edit::WavData`]
//! (multi-channel, frame-based) prefer
//! [`crate::audio_edit::peaks`]; reach for this one when the
//! samples are already a single flat slice — a mono clip, one
//! extracted channel, a mixdown preview.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_MUTED};

/// Max-absolute amplitude per bucket over a flat sample
/// slice: bucket `i` covers
/// `samples[i * len / buckets .. (i + 1) * len / buckets]`
/// (integer partition — every sample lands in exactly one
/// bucket) and reports the largest `abs()` in its range.
///
/// Edge cases, all deliberate:
/// - `buckets == 0` yields an empty vec.
/// - An empty slice yields `buckets` zeros.
/// - A slice shorter than `buckets` leaves some buckets
///   covering no samples at all; those read `0.0`, so the
///   output always has exactly `buckets` entries and a
///   short clip still draws a full-width (mostly silent)
///   strip.
pub fn downsample_peaks(samples: &[f32], buckets: usize) -> Vec<f32> {
    if buckets == 0 {
        return Vec::new();
    }
    let len = samples.len();
    let mut out = vec![0.0f32; buckets];
    if len == 0 {
        return out;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        let start = i * len / buckets;
        let end = (i + 1) * len / buckets;
        let mut max = 0.0f32;
        for v in &samples[start..end] {
            let a = v.abs();
            if a > max {
                max = a;
            }
        }
        *slot = max;
    }
    out
}

/// A full-width, 48px-tall canvas drawing `peaks` as
/// centred vertical bars: one bar per bucket, bar height
/// proportional to the bucket's peak (clamped to 0..=1).
/// Bars whose centre falls within `progress` (the played
/// fraction of the strip, clamped to 0..=1) are drawn in
/// `C_ACCENT`; the rest in `C_MUTED`, over a faint centre
/// line in `C_BORDER`.
pub fn waveform_canvas(peaks: Vec<f32>, progress: f32) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, _state, window, _cx| {
            paint_waveform(window, bounds, &peaks, progress);
        },
    )
    .w_full()
    .h(px(48.0))
}

/// Paint the waveform into `bounds`, following the graph
/// panel canvas's idiom (`PathBuilder` + `window.paint_path`):
/// one stroke path per colour, bars as subpaths.
fn paint_waveform(window: &mut Window, bounds: Bounds<Pixels>, peaks: &[f32], progress: f32) {
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w <= 0.0 || h <= 0.0 || peaks.is_empty() {
        return;
    }
    let progress = if progress.is_nan() {
        0.0
    } else {
        progress.clamp(0.0, 1.0)
    };
    // Inset so the tallest bars never clip the canvas edge.
    let pad = 3.0;
    let cy = h / 2.0;
    let max_half = cy - pad;
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    // Centre line the bars sit on.
    let mut axis = PathBuilder::stroke(px(1.0));
    axis.move_to(at(pad, cy));
    axis.line_to(at(w - pad, cy));
    if let Ok(path) = axis.build() {
        window.paint_path(path, rgb(C_BORDER));
    }
    // Bars: each bucket owns an equal slot of the width;
    // the stroke is 70% of the slot (at least 1px) so
    // neighbouring bars stay visually separate.
    let n = peaks.len() as f32;
    let slot = w / n;
    let bar_w = (slot * 0.7).max(1.0);
    let mut played = PathBuilder::stroke(px(bar_w));
    let mut rest = PathBuilder::stroke(px(bar_w));
    for (i, peak) in peaks.iter().enumerate() {
        let x = (i as f32 + 0.5) * slot;
        // A silent bucket still gets a small tick so the
        // strip reads as a waveform, not a gap.
        let half = (peak.clamp(0.0, 1.0) * max_half).max(0.75);
        let pb = if (i as f32 + 0.5) / n <= progress {
            &mut played
        } else {
            &mut rest
        };
        pb.move_to(at(x, cy - half));
        pb.line_to(at(x, cy + half));
    }
    if let Ok(path) = rest.build() {
        window.paint_path(path, rgb(C_MUTED));
    }
    if let Ok(path) = played.build() {
        window.paint_path(path, rgb(C_ACCENT));
    }
}

#[cfg(test)]
mod tests {
    use super::downsample_peaks;

    #[test]
    fn zero_buckets_is_empty() {
        assert!(downsample_peaks(&[0.5, -0.5], 0).is_empty());
        assert!(downsample_peaks(&[], 0).is_empty());
    }

    #[test]
    fn empty_samples_yield_zero_buckets() {
        assert_eq!(downsample_peaks(&[], 4), vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn reports_max_absolute_value_per_bucket() {
        // Dyadic values, exact in binary floating point.
        // Bucket 0 sees 0.125 and -0.75 -> 0.75; bucket 1
        // sees 0.25 and -0.5 -> 0.5. Sign never survives:
        // the peak is an amplitude.
        let samples = [0.125, -0.75, 0.25, -0.5];
        assert_eq!(downsample_peaks(&samples, 2), vec![0.75, 0.5]);
        // One bucket is the global max-abs.
        assert_eq!(downsample_peaks(&samples, 1), vec![0.75]);
        // One bucket per sample echoes the amplitudes.
        assert_eq!(
            downsample_peaks(&samples, 4),
            vec![0.125, 0.75, 0.25, 0.5]
        );
    }

    #[test]
    fn short_input_is_padded_with_zero_buckets() {
        // 2 samples over 4 buckets: the integer partition
        // gives buckets [empty, 0.5, empty, 0.25] — buckets
        // covering no samples read 0.0, and the output is
        // still exactly `buckets` long.
        let out = downsample_peaks(&[0.5, -0.25], 4);
        assert_eq!(out.len(), 4);
        assert_eq!(out, vec![0.0, 0.5, 0.0, 0.25]);
    }

    #[test]
    fn silence_is_all_zeros_at_any_bucket_count() {
        assert_eq!(downsample_peaks(&[0.0; 8], 3), vec![0.0, 0.0, 0.0]);
    }
}
