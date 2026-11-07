//! Pure view-model for the graph-editor panel: the geometry that
//! maps channel data (frame, value) to panel pixels and back.
//!
//! This module deliberately knows nothing about GPUI. A panel
//! widget keeps a [`GraphView`], draws [`GraphView::curve_points`]
//! as a polyline and [`GraphView::key_handles`] as draggable
//! handles, uses [`GraphView::hit_key`] to pick a handle from a
//! pointer position, and [`GraphView::to_data`] to turn a drag back
//! into data for the editing operations in [`crate::graph`].
//!
//! Pixel space follows screen convention: x grows right with
//! frame, y grows *down*, so larger values sit higher on screen
//! (smaller pixel y). [`GraphView::auto_fit`] frames a set of
//! channels: the frame range covers the project timeline and every
//! key; the value range covers every key value with 10% padding —
//! except that a view containing any angle channel uses exactly
//! -180..=180, the full circle of the normalised angle convention
//! (see `Channel::value_at`).

use crate::graph::{Channel, ChannelTarget};
use crate::model::V2;

/// A rectangular window onto channel data: frames
/// `frame_min..=frame_max` and values `val_min..=val_max` are
/// mapped onto a `width` x `height` pixel area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphView {
    pub frame_min: f32,
    pub frame_max: f32,
    pub val_min: f32,
    pub val_max: f32,
    pub width: f32,
    pub height: f32,
}

impl GraphView {
    /// Data point -> panel pixel. Points outside the view's ranges
    /// map outside the pixel area (callers may clip); a degenerate
    /// (zero-span) range maps to the near edge instead of NaN.
    pub fn to_px(&self, frame: f32, value: f32) -> V2 {
        let fx = if self.frame_max > self.frame_min {
            (frame - self.frame_min) / (self.frame_max - self.frame_min)
        } else {
            0.0
        };
        let fy = if self.val_max > self.val_min {
            (value - self.val_min) / (self.val_max - self.val_min)
        } else {
            0.0
        };
        [fx * self.width, self.height - fy * self.height]
    }

    /// Panel pixel -> `(frame, value)`; the exact inverse of
    /// [`GraphView::to_px`].
    pub fn to_data(&self, px: V2) -> (f32, f32) {
        let frame = if self.width > 0.0 {
            self.frame_min + (px[0] / self.width) * (self.frame_max - self.frame_min)
        } else {
            self.frame_min
        };
        let value = if self.height > 0.0 {
            self.val_min + ((self.height - px[1]) / self.height) * (self.val_max - self.val_min)
        } else {
            self.val_min
        };
        (frame, value)
    }

    /// The channel's curve as a pixel polyline: `steps + 1` points
    /// sampled evenly across the view's frame range through
    /// `Channel::value_at`. `steps == 0` yields no points.
    pub fn curve_points(&self, ch: &Channel, steps: usize) -> Vec<V2> {
        if steps == 0 {
            return Vec::new();
        }
        (0..=steps)
            .map(|i| {
                let f = self.frame_min
                    + (self.frame_max - self.frame_min) * (i as f32 / steps as f32);
                self.to_px(f, ch.value_at(f))
            })
            .collect()
    }

    /// Pixel position of every key handle, in key order — index i
    /// here is index i in `ch.keys` (and in [`GraphView::hit_key`]
    /// results).
    pub fn key_handles(&self, ch: &Channel) -> Vec<V2> {
        ch.keys
            .iter()
            .map(|k| self.to_px(k.frame, k.value))
            .collect()
    }

    /// Index of the key handle nearest to pixel `px`, when that
    /// handle is within `radius` pixels (Euclidean, inclusive).
    pub fn hit_key(&self, ch: &Channel, px: V2, radius: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, h) in self.key_handles(ch).into_iter().enumerate() {
            let dx = h[0] - px[0];
            let dy = h[1] - px[1];
            let d2 = dx * dx + dy * dy;
            if best.map_or(true, |(_, bd2)| d2 < bd2) {
                best = Some((i, d2));
            }
        }
        match best {
            Some((i, d2)) if d2 <= radius * radius => Some(i),
            _ => None,
        }
    }

    /// Build a view that frames `channels` on a timeline of
    /// `frames` frames (indices 0..frames-1) in a `width` x
    /// `height` panel. See the module docs for the value-range
    /// rules (10% padding; exactly -180..=180 when any channel is
    /// an angle channel; -1..=1 when there are no keys at all).
    pub fn auto_fit(channels: &[Channel], frames: usize, width: f32, height: f32) -> GraphView {
        let mut frame_min = 0.0f32;
        let mut frame_max = (frames as f32 - 1.0).max(1.0);
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut has_angle = false;
        for ch in channels {
            has_angle |= matches!(ch.target, ChannelTarget::Angle(_));
            for k in &ch.keys {
                frame_min = frame_min.min(k.frame);
                frame_max = frame_max.max(k.frame);
                lo = lo.min(k.value);
                hi = hi.max(k.value);
            }
        }
        if frame_max <= frame_min {
            frame_max = frame_min + 1.0;
        }
        let (val_min, val_max) = if has_angle {
            (-180.0, 180.0)
        } else if lo > hi {
            (-1.0, 1.0) // no keys anywhere
        } else if hi - lo <= f32::EPSILON {
            (lo - 1.0, hi + 1.0) // a single constant value
        } else {
            let pad = (hi - lo) * 0.1;
            (lo - pad, hi + pad)
        };
        GraphView {
            frame_min,
            frame_max,
            val_min,
            val_max,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Key;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn view() -> GraphView {
        GraphView {
            frame_min: 0.0,
            frame_max: 20.0,
            val_min: -50.0,
            val_max: 150.0,
            width: 400.0,
            height: 200.0,
        }
    }

    fn channel(target: ChannelTarget, keys: Vec<Key>) -> Channel {
        Channel {
            figure: 0,
            target,
            keys,
        }
    }

    #[test]
    fn px_corners_and_roundtrip() {
        let v = view();
        // Top-left is (frame_min, val_max); bottom-right is
        // (frame_max, val_min) because pixel y grows downward.
        let tl = v.to_px(0.0, 150.0);
        assert!(approx(tl[0], 0.0) && approx(tl[1], 0.0), "got {tl:?}");
        let br = v.to_px(20.0, -50.0);
        assert!(approx(br[0], 400.0) && approx(br[1], 200.0), "got {br:?}");
        // Roundtrip through a spread of in-range points.
        for (f, val) in [(0.0, -50.0), (20.0, 150.0), (7.5, 33.25), (13.0, 0.0)] {
            let (f2, v2) = v.to_data(v.to_px(f, val));
            assert!(
                approx(f2, f) && approx(v2, val),
                "roundtrip ({f}, {val}) -> ({f2}, {v2})"
            );
        }
    }

    #[test]
    fn curve_passes_through_key_pixels() {
        let v = GraphView {
            frame_min: 0.0,
            frame_max: 10.0,
            val_min: 0.0,
            val_max: 100.0,
            width: 500.0,
            height: 250.0,
        };
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(5.0, 80.0),
                Key::flat(10.0, 20.0),
            ],
        );
        // 100 steps over 10 frames samples every 0.1 frame, landing
        // exactly on each key frame.
        let pts = v.curve_points(&ch, 100);
        assert_eq!(pts.len(), 101);
        for h in v.key_handles(&ch) {
            let nearest = pts
                .iter()
                .map(|p| (p[0] - h[0]).abs() + (p[1] - h[1]).abs())
                .fold(f32::INFINITY, f32::min);
            assert!(nearest < 1e-2, "curve misses key handle {h:?} by {nearest}");
        }
        assert!(v.curve_points(&ch, 0).is_empty(), "zero steps: no points");
    }

    #[test]
    fn hit_key_picks_nearest_and_misses_far() {
        let v = view();
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, -50.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 150.0),
            ],
        );
        let handles = v.key_handles(&ch);
        // Clicking on handle 1 hits key 1.
        assert_eq!(v.hit_key(&ch, handles[1], 6.0), Some(1));
        // Near handle 2 (but not on it) still picks 2 over 1.
        let near2 = [handles[2][0] - 3.0, handles[2][1] + 2.0];
        assert_eq!(v.hit_key(&ch, near2, 6.0), Some(2));
        // A point 90px from the nearest handle (handle 1 sits at
        // [200, 100] in this view) misses at radius 6.
        assert_eq!(v.hit_key(&ch, [200.0, 190.0], 6.0), None);
        // Just outside the radius misses even the nearest handle.
        let just_out = [handles[0][0] + 6.5, handles[0][1]];
        assert_eq!(v.hit_key(&ch, just_out, 6.0), None);
        // Empty channel never hits.
        let empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(v.hit_key(&empty, handles[0], 6.0), None);
    }

    #[test]
    fn auto_fit_contains_all_keys_with_padding() {
        let channels = vec![
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(2.0, 10.0), Key::flat(30.0, 90.0)],
            ),
            channel(ChannelTarget::RootY, vec![Key::flat(5.0, -20.0)]),
        ];
        let v = GraphView::auto_fit(&channels, 24, 800.0, 300.0);
        // The key at frame 30 lies beyond the 24-frame timeline, so
        // the frame range must have stretched to include it.
        assert!(v.frame_min <= 0.0 && v.frame_max >= 30.0);
        // Values are padded: the range strictly contains -20..=90.
        assert!(v.val_min < -20.0 && v.val_max > 90.0);
        for ch in &channels {
            for h in v.key_handles(ch) {
                assert!(
                    h[0] >= 0.0 && h[0] <= 800.0 && h[1] >= 0.0 && h[1] <= 300.0,
                    "handle {h:?} outside the fitted view"
                );
            }
        }
    }

    #[test]
    fn auto_fit_angle_channel_uses_full_circle() {
        let channels = vec![channel(
            ChannelTarget::Angle(1),
            vec![Key::flat(0.0, -170.0), Key::flat(9.0, 170.0)],
        )];
        let v = GraphView::auto_fit(&channels, 10, 400.0, 200.0);
        assert!(approx(v.val_min, -180.0) && approx(v.val_max, 180.0));
        for h in v.key_handles(&channels[0]) {
            assert!(h[0] >= 0.0 && h[0] <= 400.0 && h[1] >= 0.0 && h[1] <= 200.0);
        }
        // No keys at all still yields a usable, non-degenerate view.
        let empty = GraphView::auto_fit(&[], 12, 400.0, 200.0);
        assert!(empty.val_max > empty.val_min && empty.frame_max > empty.frame_min);
    }
}
