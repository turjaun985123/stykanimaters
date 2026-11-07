//! Keyframe graph / property-channel engine: the math core of a
//! graph editor.
//!
//! A [`Channel`] animates one scalar property of one figure over
//! time — its root X, its root Y, one node's relative angle, or one
//! node's segment length — as a list of [`Key`]s joined by cubic
//! Hermite splines. Each key carries the slope of the curve entering
//! it (`in_tan`) and leaving it (`out_tan`), in value-units per
//! frame, so an editor can offer the usual broken-tangent handles.
//!
//! Why [`apply_channels`] returns defs as well as poses: [`Pose`]
//! holds only a root position and per-node angles — segment lengths
//! live in [`FigureDef`]. Root and angle channels therefore override
//! the returned poses, while length channels override a cloned set
//! of figure defs (clamped to a minimum of 1.0 so a segment can
//! never collapse or invert). The two vectors are drop-in partners:
//! `evaluate(&result.defs[fi], &result.poses[fi])` gives the final
//! joint positions. Defs are always returned (plain clones when no
//! length channel exists) so callers have a single code path.
//!
//! Channels are pure data: nothing here mutates the project, and
//! channels pointing at a figure or node index that no longer exists
//! are ignored rather than panicking — an editor may hold stale
//! channels after a figure edit. If several channels target the same
//! property, the later one in the slice wins.

use crate::model::{norm_deg, FigureDef, Pose, Project};
use serde::{Deserialize, Serialize};

/// Which scalar property of a figure a [`Channel`] drives. The
/// `usize` payloads are node indices into the figure's def.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelTarget {
    /// The pose root's X coordinate (stage units).
    RootX,
    /// The pose root's Y coordinate (stage units).
    RootY,
    /// Relative angle of node `n`, in degrees (the `Pose` convention).
    Angle(usize),
    /// Segment length of node `n`, in stage units (the `FigureDef`
    /// convention — see the module docs for why this lands in defs).
    Length(usize),
}

/// How a channel segment is interpolated. The mode lives on the
/// key that STARTS the segment: it governs the curve from that key
/// to the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interp {
    /// Cubic Hermite spline through the segment's keys, using
    /// their tangents (the engine's original behaviour).
    #[default]
    Hermite,
    /// Straight-line interpolation between the segment's keys
    /// (angle channels still take the shortest circular path).
    Linear,
    /// Hold this key's value for the whole segment, jumping to the
    /// next key's value exactly at the next key's frame.
    Step,
}

/// One keyframe on a channel. Tangents are slopes in value-units
/// per frame: `out_tan` is the curve's slope as it leaves this key,
/// `in_tan` its slope as it arrives at this key. `interp` selects
/// how the segment STARTING at this key is interpolated; it
/// defaults to [`Interp::Hermite`], so projects saved before the
/// field existed load unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub frame: f32,
    pub value: f32,
    pub in_tan: f32,
    pub out_tan: f32,
    #[serde(default)]
    pub interp: Interp,
}

impl Key {
    /// A key with flat (zero) tangents and Hermite interpolation.
    pub fn flat(frame: f32, value: f32) -> Self {
        Key {
            frame,
            value,
            in_tan: 0.0,
            out_tan: 0.0,
            interp: Interp::Hermite,
        }
    }

    /// This key with a different interpolation mode for the
    /// segment starting at it (builder-style, for editors/tests).
    pub fn with_interp(mut self, interp: Interp) -> Self {
        self.interp = interp;
        self
    }
}

/// A property channel: an ordered list of keys for one property of
/// one figure (an index into `Project::figures`). Keys must be in
/// ascending frame order — call [`Channel::sort_keys`] after edits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub figure: usize,
    pub target: ChannelTarget,
    pub keys: Vec<Key>,
}

impl Channel {
    /// Restore the ascending-frame order `value_at` relies on.
    pub fn sort_keys(&mut self) {
        self.keys
            .sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Index of the key whose frame is closest to `frame`, when
    /// that distance is within `tol`; `None` otherwise.
    fn nearest_key_index(&self, frame: f32, tol: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, k) in self.keys.iter().enumerate() {
            let d = (k.frame - frame).abs();
            if d <= tol && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Insert a flat-tangent key at (`frame`, `value`), keeping
    /// keys sorted. If a key already sits at the same frame (within
    /// 0.01), it is replaced wholesale — new value, flat tangents —
    /// rather than duplicated.
    pub fn add_key(&mut self, frame: f32, value: f32) {
        if let Some(idx) = self.nearest_key_index(frame, 0.01) {
            self.keys[idx] = Key::flat(frame, value);
            self.sort_keys();
            return;
        }
        let pos = self.keys.partition_point(|k| k.frame < frame);
        self.keys.insert(pos, Key::flat(frame, value));
    }

    /// Remove the key nearest to `frame`, when one is within 0.5
    /// frames of it. Returns whether a key was removed.
    pub fn remove_key(&mut self, frame: f32) -> bool {
        match self.nearest_key_index(frame, 0.5) {
            Some(idx) => {
                self.keys.remove(idx);
                true
            }
            None => false,
        }
    }

    /// Move the key nearest to `from` (within 0.5 frames) to
    /// (`to_frame`, `to_value`), keeping its tangents and the
    /// sorted order. If another key already sits at `to_frame`
    /// (within 0.01), the moved key replaces it. Returns whether a
    /// key was moved.
    pub fn move_key(&mut self, from: f32, to_frame: f32, to_value: f32) -> bool {
        let Some(idx) = self.nearest_key_index(from, 0.5) else {
            return false;
        };
        let mut key = self.keys.remove(idx);
        key.frame = to_frame;
        key.value = to_value;
        if let Some(other) = self
            .keys
            .iter()
            .position(|k| (k.frame - to_frame).abs() <= 0.01)
        {
            self.keys.remove(other);
        }
        let pos = self.keys.partition_point(|k| k.frame < key.frame);
        self.keys.insert(pos, key);
        true
    }

    /// Set both tangents of the key nearest to `frame` (within 0.5
    /// frames). Returns whether such a key was found.
    pub fn set_tangents(&mut self, frame: f32, in_tan: f32, out_tan: f32) -> bool {
        match self.nearest_key_index(frame, 0.5) {
            Some(idx) => {
                self.keys[idx].in_tan = in_tan;
                self.keys[idx].out_tan = out_tan;
                true
            }
            None => false,
        }
    }

    fn is_angle(&self) -> bool {
        matches!(self.target, ChannelTarget::Angle(_))
    }

    /// The channel's value at fractional frame `frame_f`.
    ///
    /// Between two keys the segment is interpolated by the mode on
    /// the key that STARTS it (`Key::interp`): Hermite is the cubic
    /// spline through them, using the left key's `out_tan` and the
    /// right key's `in_tan` (converted from per-frame slopes to the
    /// segment's normalised parameter); Linear is a straight lerp;
    /// Step holds the left key's value until the right key's frame.
    /// Outside the key range the value clamps to the nearest key;
    /// at a key frame the stored value is returned exactly. A
    /// channel with no keys yields 0.0.
    ///
    /// Angle channels interpolate on the shortest circular path:
    /// each segment's end value is unwrapped relative to its start
    /// (350° → 10° travels through 0°, never the long way through
    /// 180°), and the interpolated result is normalised to
    /// (-180, 180], matching `Pose::lerp`'s convention. Tangents on
    /// an angle channel are interpreted on that unwrapped path.
    /// (Step needs no unwrapping: it holds the start value.)
    pub fn value_at(&self, frame_f: f32) -> f32 {
        let keys = &self.keys;
        if keys.is_empty() {
            return 0.0;
        }
        let first = &keys[0];
        let last = &keys[keys.len() - 1];
        if frame_f <= first.frame {
            return first.value;
        }
        if frame_f >= last.frame {
            return last.value;
        }
        // Find the segment [i, i+1] containing frame_f.
        let mut i = 0;
        while i + 1 < keys.len() - 1 && keys[i + 1].frame <= frame_f {
            i += 1;
        }
        let (k0, k1) = (&keys[i], &keys[i + 1]);
        let dt = k1.frame - k0.frame;
        if dt <= 0.0 {
            return k1.value;
        }
        let t = (frame_f - k0.frame) / dt;
        let v0 = k0.value;
        let v1 = if self.is_angle() {
            // Unwrap the end value onto the short path from v0.
            v0 + norm_deg(k1.value - v0)
        } else {
            k1.value
        };
        // The segment's interpolation mode comes from the key that
        // starts the segment.
        let v = match k0.interp {
            Interp::Step => v0,
            Interp::Linear => v0 + (v1 - v0) * t,
            Interp::Hermite => {
                // Hermite derivatives are with respect to t, so
                // scale the per-frame slopes by the segment's span.
                let m0 = k0.out_tan * dt;
                let m1 = k1.in_tan * dt;
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                h00 * v0 + h10 * m0 + h01 * v1 + h11 * m1
            }
        };
        if self.is_angle() {
            norm_deg(v)
        } else {
            v
        }
    }
}

/// Recompute every key's tangents as the Catmull-Rom slope through
/// its neighbours — the "auto" / "smooth tangents" operation of a
/// graph editor, for when hand-dragged handles have left a curve
/// lumpy and the user wants the natural spline back.
///
/// For each interior key `i` the slope is the secant through its
/// two neighbours, `(v[i+1] − v[i−1]) / (f[i+1] − f[i−1])`, stored
/// in both `in_tan` and `out_tan`, so the curve passes through the
/// key smoothly (no corner). Endpoints have only one neighbour
/// pair, so they take that pair's one-sided slope; in a two-key
/// slice both keys therefore get the segment's slope. Fewer than
/// two keys have no slope to derive and are left untouched. A
/// degenerate span (neighbour frames not strictly increasing —
/// impossible in channels built through [`Channel::add_key`], but
/// possible in a hand-built slice) yields a flat 0.0 tangent for
/// that key rather than a NaN.
///
/// With `angular` set, value deltas take the shortest circular
/// path ([`norm_deg`]), matching how [`Channel::value_at`] unwraps
/// angle segments: keys at 350° and 30° differ by +40°, not −320°.
/// The caller decides — this function cannot tell an angle channel
/// from a plain one by its values alone.
///
/// Tangents are set on **every** key, whatever its [`Interp`]
/// mode. Evaluation ignores tangents for the segments a Step or
/// Linear key starts, but a key's `in_tan` belongs to the segment
/// *ending* at it (governed by the previous key's mode), and if an
/// editor later flips a key back to Hermite its smoothed tangents
/// are already in place instead of stale hand-set ones.
///
/// Keys must be in ascending frame order (see
/// [`Channel::sort_keys`]); [`smooth_channel`] handles that for
/// you.
pub fn auto_tangents(keys: &mut [Key], angular: bool) {
    let n = keys.len();
    if n < 2 {
        return;
    }
    let delta = |a: f32, b: f32| -> f32 {
        if angular {
            norm_deg(b - a)
        } else {
            b - a
        }
    };
    let pair_slope = |keys: &[Key], i0: usize, i1: usize| -> f32 {
        let dt = keys[i1].frame - keys[i0].frame;
        if dt <= 0.0 {
            0.0
        } else {
            delta(keys[i0].value, keys[i1].value) / dt
        }
    };
    // Compute every slope from the untouched slice first, then
    // apply: writing tangents as we go would feed already-smoothed
    // values into the neighbouring keys' secants.
    let mut slopes = Vec::with_capacity(n);
    for i in 0..n {
        slopes.push(if i == 0 {
            pair_slope(keys, 0, 1)
        } else if i == n - 1 {
            pair_slope(keys, n - 2, n - 1)
        } else {
            pair_slope(keys, i - 1, i + 1)
        });
    }
    for (key, s) in keys.iter_mut().zip(slopes) {
        key.in_tan = s;
        key.out_tan = s;
    }
}

/// Channel-level convenience wrapper around [`auto_tangents`]:
/// sorts the keys (the order evaluation relies on), then smooths
/// them, deriving `angular` from the channel's target so angle
/// channels wrap and every other target stays plain.
pub fn smooth_channel(channel: &mut Channel) {
    channel.sort_keys();
    let angular = channel.is_angle();
    auto_tangents(&mut channel.keys, angular);
}

/// Remove interior keys that contribute at most `tolerance`
/// (in value units) to the channel's curve — the "reduce keys" /
/// "simplify" operation of a graph editor, for channels that were
/// baked or imported with far more keys than their shape needs.
///
/// The algorithm is a greedy single left-to-right pass with a
/// purely **local** test — it is not Douglas-Peucker and makes no
/// global max-error guarantee. Each interior key is considered
/// once, in frame order: the channel is re-evaluated at the key's
/// own frame with the key removed, and the key is dropped when
/// that value differs from the key's stored value by at most
/// `tolerance`. The "removed" curve at that moment runs from the
/// previous **kept** key to the next key, so once a key is dropped
/// it stays dropped: later candidates are tested against the
/// longer segment their removal would actually leave behind, not
/// against their original neighbours.
///
/// Two honest approximations follow from that design. First, the
/// error is measured only at the removed key's own frame — the
/// curve between keys can deviate by more than `tolerance` even
/// when every single removal passed its local test. Second, the
/// test segment is evaluated with the neighbours' stored tangents
/// exactly as they are ([`Channel::value_at`] on the two-key
/// segment): tangents are not refitted to the longer span, so on
/// Hermite channels a key whose neighbours carry strong tangents
/// may be kept even where the values alone look redundant. The
/// segment's interpolation mode is the previous kept key's, as
/// usual. Angle channels compare by shortest-path difference
/// ([`norm_deg`]), so keys straddling the ±180° wrap are judged by
/// their circular distance.
///
/// The first and last keys are endpoints and are never removed.
/// A channel with fewer than three keys has no interior key to
/// test and is returned completely untouched (not even sorted).
/// Otherwise the keys are sorted first and remain sorted. Returns
/// the number of keys removed.
pub fn reduce_keys(channel: &mut Channel, tolerance: f32) -> usize {
    let n = channel.keys.len();
    if n < 3 {
        return 0;
    }
    channel.sort_keys();
    let angular = channel.is_angle();
    let mut kept: Vec<Key> = Vec::with_capacity(n);
    kept.push(channel.keys[0]);
    for i in 1..n - 1 {
        let candidate = channel.keys[i];
        let prev = *kept.last().expect("the first key is always kept");
        let next = channel.keys[i + 1];
        // The curve without the candidate, at the candidate's
        // frame: the two-key segment from the previous kept key
        // to the next key, evaluated by the engine's own rules.
        let probe = Channel {
            figure: channel.figure,
            target: channel.target,
            keys: vec![prev, next],
        };
        let without = probe.value_at(candidate.frame);
        let err = if angular {
            norm_deg(without - candidate.value).abs()
        } else {
            (without - candidate.value).abs()
        };
        if err > tolerance {
            kept.push(candidate);
        }
    }
    kept.push(channel.keys[n - 1]);
    let removed = n - kept.len();
    channel.keys = kept;
    removed
}

/// The result of [`apply_channels`]: channel-overridden poses plus
/// the figure defs to evaluate them with (clones of the project's
/// defs, with any length channels applied).
#[derive(Clone, Debug)]
pub struct ChannelResult {
    pub poses: Vec<Pose>,
    pub defs: Vec<FigureDef>,
}

/// Find the channel for (`figure`, `target`) in `channels`, or
/// create an empty one there, and return it mutably. Editors call
/// this before adding a first key so they never have to special-
/// case a missing channel.
pub fn channel_mut<'a>(
    channels: &'a mut Vec<Channel>,
    figure: usize,
    target: ChannelTarget,
) -> &'a mut Channel {
    if let Some(pos) = channels
        .iter()
        .position(|c| c.figure == figure && c.target == target)
    {
        &mut channels[pos]
    } else {
        channels.push(Channel {
            figure,
            target,
            keys: Vec::new(),
        });
        channels.last_mut().expect("just pushed a channel")
    }
}

/// Evaluate every channel at `frame_f` on top of the project's own
/// animation: start from `project.poses_at(frame_f)`, then override
/// root, angle and length components per channel. Length results
/// are clamped to a minimum of 1.0. See the module docs for the
/// full contract.
pub fn apply_channels(project: &Project, channels: &[Channel], frame_f: f32) -> ChannelResult {
    let mut poses = project.poses_at(frame_f);
    let mut defs: Vec<FigureDef> = project.figures.iter().map(|f| f.def.clone()).collect();
    for ch in channels {
        let v = ch.value_at(frame_f);
        match ch.target {
            ChannelTarget::RootX => {
                if let Some(p) = poses.get_mut(ch.figure) {
                    p.root[0] = v;
                }
            }
            ChannelTarget::RootY => {
                if let Some(p) = poses.get_mut(ch.figure) {
                    p.root[1] = v;
                }
            }
            ChannelTarget::Angle(n) => {
                if let Some(p) = poses.get_mut(ch.figure) {
                    if let Some(a) = p.angles.get_mut(n) {
                        *a = v;
                    }
                }
            }
            ChannelTarget::Length(n) => {
                if let Some(d) = defs.get_mut(ch.figure) {
                    if let Some(node) = d.nodes.get_mut(n) {
                        node.length = v.max(1.0);
                    }
                }
            }
        }
    }
    ChannelResult { poses, defs }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn channel(target: ChannelTarget, keys: Vec<Key>) -> Channel {
        Channel {
            figure: 0,
            target,
            keys,
        }
    }

    #[test]
    fn exact_values_at_keys_and_clamped_outside() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(2.0, 7.0), Key::flat(8.0, -3.0)],
        );
        assert!(approx(ch.value_at(2.0), 7.0), "at first key");
        assert!(approx(ch.value_at(8.0), -3.0), "at last key");
        assert!(approx(ch.value_at(0.0), 7.0), "before range clamps");
        assert!(approx(ch.value_at(-5.0), 7.0), "well before range clamps");
        assert!(approx(ch.value_at(99.0), -3.0), "after range clamps");
        // A single key is a constant channel.
        let one = channel(ChannelTarget::RootY, vec![Key::flat(4.0, 42.0)]);
        assert!(approx(one.value_at(0.0), 42.0));
        assert!(approx(one.value_at(4.0), 42.0));
        assert!(approx(one.value_at(400.0), 42.0));
    }

    #[test]
    fn hermite_midpoint_matches_hand_computed_value() {
        // Keys (0, 0) -> (10, 100), in/out tangents 0 except the
        // first key's out_tan = 2.0 units/frame.
        // dt = 10, so m0 = 20, m1 = 0. At t = 0.5 the Hermite basis
        // is h00 = 0.5, h10 = 0.125, h01 = 0.5, h11 = -0.125, giving
        // 0.5*0 + 0.125*20 + 0.5*100 - 0.125*0 = 52.5 exactly.
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key {
                    frame: 0.0,
                    value: 0.0,
                    in_tan: 0.0,
                    out_tan: 2.0,
                    interp: Interp::Hermite,
                },
                Key::flat(10.0, 100.0),
            ],
        );
        assert!(approx(ch.value_at(5.0), 52.5), "got {}", ch.value_at(5.0));
    }

    #[test]
    fn steeper_out_tangent_raises_the_midpoint() {
        let flat = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        );
        let steep = channel(
            ChannelTarget::RootX,
            vec![
                Key {
                    frame: 0.0,
                    value: 0.0,
                    in_tan: 0.0,
                    out_tan: 2.0,
                    interp: Interp::Hermite,
                },
                Key::flat(10.0, 100.0),
            ],
        );
        assert!(approx(flat.value_at(5.0), 50.0), "flat tangents give the midpoint");
        assert!(
            steep.value_at(5.0) > flat.value_at(5.0),
            "steeper out-tangent must pull the mid value up: {} vs {}",
            steep.value_at(5.0),
            flat.value_at(5.0)
        );
    }

    #[test]
    fn angle_channel_wraps_the_short_way() {
        // 350° -> 10°: the short path passes through 0°, so the
        // midpoint is ~0°, never ~180°.
        let ch = channel(
            ChannelTarget::Angle(3),
            vec![Key::flat(0.0, 350.0), Key::flat(10.0, 10.0)],
        );
        assert!(approx(ch.value_at(0.0), 350.0), "first key verbatim");
        assert!(approx(ch.value_at(10.0), 10.0), "last key verbatim");
        let mid = ch.value_at(5.0);
        assert!(mid.abs() < 1e-3, "midpoint should be ~0, got {mid}");
        assert!((mid - 180.0).abs() > 90.0, "must not take the long way");
        // A non-wrapping angle channel behaves like a plain one.
        let plain = channel(
            ChannelTarget::Angle(0),
            vec![Key::flat(0.0, 20.0), Key::flat(10.0, 60.0)],
        );
        assert!(approx(plain.value_at(5.0), 40.0));
    }

    #[test]
    fn apply_channels_overrides_root_angle_and_length() {
        let project = Project::new_demo();
        let base = project.poses_at(0.0);
        assert!(approx(base[0].root[0], 480.0) && approx(base[0].root[1], 470.0));

        let channels = vec![
            // Constant RootX = 123 from a single key.
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 123.0)]),
            channel(
                ChannelTarget::Angle(1),
                vec![Key::flat(0.0, 45.0), Key::flat(4.0, 45.0)],
            ),
            channel(ChannelTarget::Length(1), vec![Key::flat(0.0, 99.0)]),
            // Length below the floor must clamp to 1.0.
            channel(ChannelTarget::Length(2), vec![Key::flat(0.0, 0.25)]),
        ];
        let out = apply_channels(&project, &channels, 0.0);
        assert!(approx(out.poses[0].root[0], 123.0), "root X overridden");
        assert!(
            approx(out.poses[0].root[1], base[0].root[1]),
            "root Y untouched by a RootX channel"
        );
        assert!(approx(out.poses[0].angles[1], 45.0), "angle overridden");
        assert!(approx(out.defs[0].nodes[1].length, 99.0), "length overridden");
        assert!(approx(out.defs[0].nodes[2].length, 1.0), "length clamped to >= 1");
        // Untouched lengths keep the def's values, and the project's
        // own defs are never mutated (apply works on clones).
        assert!(approx(out.defs[0].nodes[3].length, project.figures[0].def.nodes[3].length));
        assert!(approx(
            project.figures[0].def.nodes[1].length,
            FigureDef::humanoid().nodes[1].length
        ));
    }

    #[test]
    fn stale_channels_are_ignored_not_fatal() {
        let project = Project::new_demo();
        let channels = vec![
            Channel {
                figure: 99,
                target: ChannelTarget::RootX,
                keys: vec![Key::flat(0.0, 0.0)],
            },
            channel(ChannelTarget::Angle(999), vec![Key::flat(0.0, 0.0)]),
            channel(ChannelTarget::Length(999), vec![Key::flat(0.0, 0.0)]),
        ];
        let out = apply_channels(&project, &channels, 0.0);
        assert_eq!(out.poses.len(), 1);
        assert!(approx(out.poses[0].root[0], 480.0), "nothing overridden");
    }

    #[test]
    fn add_key_inserts_sorted_and_replaces_same_frame() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        );
        ch.add_key(5.0, 40.0);
        assert_eq!(ch.keys.len(), 3);
        assert!(
            approx(ch.keys[0].frame, 0.0)
                && approx(ch.keys[1].frame, 5.0)
                && approx(ch.keys[2].frame, 10.0),
            "keys stay sorted: {:?}",
            ch.keys
        );
        assert!(approx(ch.keys[1].value, 40.0));
        assert!(
            approx(ch.keys[1].in_tan, 0.0) && approx(ch.keys[1].out_tan, 0.0),
            "inserted keys have flat tangents"
        );
        // Within 0.01 of an existing frame: replace, don't duplicate.
        // The replacement key sits at the exact new frame.
        ch.add_key(10.005, 111.0);
        assert_eq!(ch.keys.len(), 3, "same-frame add must not duplicate");
        assert!(approx(ch.keys[2].frame, 10.005), "key moved to the new frame");
        assert!(approx(ch.keys[2].value, 111.0), "value replaced");
        assert!(approx(ch.value_at(10.005), 111.0), "exact at the key");
    }

    #[test]
    fn add_key_replacement_resets_tangents() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![Key {
                frame: 5.0,
                value: 1.0,
                in_tan: 3.0,
                out_tan: -2.0,
                interp: Interp::Hermite,
            }],
        );
        ch.add_key(5.0, 9.0);
        assert_eq!(ch.keys.len(), 1);
        assert!(approx(ch.keys[0].value, 9.0));
        assert!(
            approx(ch.keys[0].in_tan, 0.0) && approx(ch.keys[0].out_tan, 0.0),
            "replacement is a fresh flat key"
        );
    }

    #[test]
    fn remove_key_takes_nearest_within_half_frame() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(5.0, 50.0),
                Key::flat(10.0, 100.0),
            ],
        );
        assert!(ch.remove_key(5.4), "0.4 away is within tolerance");
        assert_eq!(ch.keys.len(), 2);
        assert!(approx(ch.keys[1].frame, 10.0), "the 5-frame key went");
        assert!(!ch.remove_key(7.0), "2+ frames from any key: no removal");
        assert_eq!(ch.keys.len(), 2, "failed removal changes nothing");
        let mut empty = channel(ChannelTarget::RootX, vec![]);
        assert!(!empty.remove_key(0.0), "empty channel: nothing to remove");
    }

    #[test]
    fn move_key_keeps_order_and_updates_value() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key {
                    frame: 5.0,
                    value: 50.0,
                    in_tan: 1.0,
                    out_tan: 1.0,
                    interp: Interp::Hermite,
                },
                Key::flat(10.0, 100.0),
            ],
        );
        assert!(ch.move_key(5.0, 2.0, 25.0));
        assert_eq!(ch.keys.len(), 3);
        assert!(
            approx(ch.keys[0].frame, 0.0)
                && approx(ch.keys[1].frame, 2.0)
                && approx(ch.keys[2].frame, 10.0),
            "order restored after move: {:?}",
            ch.keys
        );
        assert!(approx(ch.keys[1].value, 25.0), "value moved with the key");
        assert!(approx(ch.keys[1].in_tan, 1.0), "tangents travel with the key");
        assert!(!ch.move_key(7.5, 8.0, 0.0), "no key near 7.5 to move");
        // Moving onto another key's frame replaces that key.
        assert!(ch.move_key(2.0, 10.0, 90.0));
        assert_eq!(ch.keys.len(), 2, "destination key replaced");
        assert!(approx(ch.value_at(10.0), 90.0));
    }

    #[test]
    fn set_tangents_finds_key_and_changes_curve() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        );
        assert!(approx(ch.value_at(5.0), 50.0), "flat tangents: midpoint 50");
        assert!(ch.set_tangents(0.0, 0.0, 2.0));
        assert!(approx(ch.keys[0].out_tan, 2.0));
        assert!(
            ch.value_at(5.0) > 50.0,
            "steeper out-tangent lifts the curve"
        );
        assert!(!ch.set_tangents(4.0, 1.0, 1.0), "no key within 0.5 of 4");
    }

    #[test]
    fn channel_mut_creates_then_reuses() {
        let mut channels: Vec<Channel> = Vec::new();
        {
            let ch = channel_mut(&mut channels, 2, ChannelTarget::Angle(3));
            assert_eq!(ch.figure, 2);
            assert_eq!(ch.target, ChannelTarget::Angle(3));
            assert!(ch.keys.is_empty(), "fresh channel has no keys");
            ch.add_key(0.0, 10.0);
        }
        assert_eq!(channels.len(), 1, "one channel created");
        {
            let ch = channel_mut(&mut channels, 2, ChannelTarget::Angle(3));
            assert_eq!(ch.keys.len(), 1, "same channel returned, keys kept");
        }
        assert_eq!(channels.len(), 1, "reuse must not duplicate");
        channel_mut(&mut channels, 2, ChannelTarget::RootX).add_key(1.0, 5.0);
        channel_mut(&mut channels, 0, ChannelTarget::Angle(3)).add_key(1.0, 5.0);
        assert_eq!(channels.len(), 3, "figure and target both distinguish");
    }

    #[test]
    fn linear_midpoint_is_exact() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Linear),
                Key::flat(10.0, 100.0),
            ],
        );
        assert!(approx(ch.value_at(0.0), 0.0));
        assert!(approx(ch.value_at(2.5), 25.0), "quarter point lerps");
        assert!(approx(ch.value_at(5.0), 50.0), "midpoint is the exact mean");
        assert!(approx(ch.value_at(7.5), 75.0));
        assert!(approx(ch.value_at(10.0), 100.0));
    }

    #[test]
    fn step_holds_value_then_jumps_at_next_key() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 7.0).with_interp(Interp::Step),
                Key::flat(10.0, 42.0).with_interp(Interp::Step),
                Key::flat(20.0, 5.0),
            ],
        );
        assert!(approx(ch.value_at(0.0), 7.0));
        assert!(approx(ch.value_at(5.0), 7.0), "held mid-segment");
        assert!(approx(ch.value_at(9.99), 7.0), "held until the next key");
        assert!(approx(ch.value_at(10.0), 42.0), "jumps exactly at the key");
        assert!(approx(ch.value_at(19.99), 42.0), "second segment holds too");
        assert!(approx(ch.value_at(20.0), 5.0), "last key verbatim");
    }

    #[test]
    fn linear_angle_wraps_through_zero() {
        // 350° -> 10° under Linear: the wrapped delta is +20°, so
        // the path passes through 0°, exactly like Hermite mode.
        let ch = channel(
            ChannelTarget::Angle(0),
            vec![
                Key::flat(0.0, 350.0).with_interp(Interp::Linear),
                Key::flat(10.0, 10.0),
            ],
        );
        assert!(
            approx(ch.value_at(2.5), -5.0),
            "quarter point is 355° ≡ -5°, got {}",
            ch.value_at(2.5)
        );
        let mid = ch.value_at(5.0);
        assert!(mid.abs() < 1e-3, "midpoint should be ~0, got {mid}");
        assert!(approx(ch.value_at(7.5), 5.0), "three-quarter point is 5°");
    }

    #[test]
    fn segment_mode_comes_from_the_starting_key() {
        // Start key Linear, end key Step: the segment is linear —
        // the end key's mode governs the NEXT segment, not this one.
        let lin = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Linear),
                Key::flat(10.0, 100.0).with_interp(Interp::Step),
            ],
        );
        assert!(approx(lin.value_at(5.0), 50.0), "start key's Linear wins");
        // And the mirror image: start Step, end Linear -> held.
        let step = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Step),
                Key::flat(10.0, 100.0).with_interp(Interp::Linear),
            ],
        );
        assert!(approx(step.value_at(5.0), 0.0), "start key's Step wins");
        assert!(approx(step.value_at(10.0), 100.0));
    }

    #[test]
    fn key_json_without_interp_defaults_to_hermite() {
        // Projects saved before the interp field existed must load
        // with the original Hermite behaviour.
        let old: Key = serde_json::from_str(
            r#"{"frame": 1.0, "value": 2.0, "in_tan": 0.5, "out_tan": -0.5}"#,
        )
        .expect("pre-interp key JSON must parse");
        assert_eq!(old.interp, Interp::Hermite);
        assert!(approx(old.value, 2.0) && approx(old.out_tan, -0.5));
        // And a non-default mode round-trips through JSON.
        let lin = Key::flat(3.0, 9.0).with_interp(Interp::Linear);
        let json = serde_json::to_string(&lin).expect("key serialises");
        assert!(json.contains("Linear"), "mode is stored: {json}");
        let back: Key = serde_json::from_str(&json).expect("key parses back");
        assert_eq!(back, lin);
    }
}

#[cfg(test)]
mod auto_tangent_tests {
    use super::{auto_tangents, smooth_channel, Channel, ChannelTarget, Interp, Key};

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Both tangents of a key, for one-line assertions.
    fn tangents(k: &Key) -> (f32, f32) {
        (k.in_tan, k.out_tan)
    }

    #[test]
    fn middle_key_gets_exact_catmull_rom_slope() {
        // Keys (0, 0), (10, 50), (30, 90): the middle key's
        // Catmull-Rom slope is the neighbour secant
        // (90 - 0) / (30 - 0) = 3.0, stored in both tangents.
        let mut keys = vec![
            Key::flat(0.0, 0.0),
            Key::flat(10.0, 50.0),
            Key::flat(30.0, 90.0),
        ];
        auto_tangents(&mut keys, false);
        let (i, o) = tangents(&keys[1]);
        assert!(approx(i, 3.0) && approx(o, 3.0), "middle tangents {i}, {o}");
    }

    #[test]
    fn endpoints_take_one_sided_slopes() {
        // Same keys: the first key only has the (0,0)->(10,50)
        // pair, slope 5.0; the last only (10,50)->(30,90), slope
        // 40 / 20 = 2.0.
        let mut keys = vec![
            Key::flat(0.0, 0.0),
            Key::flat(10.0, 50.0),
            Key::flat(30.0, 90.0),
        ];
        auto_tangents(&mut keys, false);
        let (fi, fo) = tangents(&keys[0]);
        assert!(approx(fi, 5.0) && approx(fo, 5.0), "first tangents {fi}, {fo}");
        let (li, lo) = tangents(&keys[2]);
        assert!(approx(li, 2.0) && approx(lo, 2.0), "last tangents {li}, {lo}");
        // A two-key slice: both keys are endpoints of the same
        // single pair, so both get the pair's slope.
        let mut pair = vec![Key::flat(0.0, 0.0), Key::flat(4.0, 20.0)];
        auto_tangents(&mut pair, false);
        for k in &pair {
            let (i, o) = tangents(k);
            assert!(approx(i, 5.0) && approx(o, 5.0), "pair tangents {i}, {o}");
        }
    }

    #[test]
    fn degenerate_inputs_are_left_alone() {
        // A single key has no neighbour pair: its tangents (here
        // deliberately non-flat) must survive untouched.
        let mut one = vec![Key {
            frame: 3.0,
            value: 7.0,
            in_tan: 3.0,
            out_tan: -2.0,
            interp: Interp::Hermite,
        }];
        auto_tangents(&mut one, false);
        let (i, o) = tangents(&one[0]);
        assert!(approx(i, 3.0) && approx(o, -2.0), "single key untouched");
        // And an empty slice is simply a no-op (no panic).
        let mut none: Vec<Key> = Vec::new();
        auto_tangents(&mut none, true);
        assert!(none.is_empty());
    }

    #[test]
    fn angular_slopes_wrap_through_zero() {
        // 350° -> 10° -> 30° is steady +20°-per-10-frames motion
        // through 0°: with angular deltas every key's slope is
        // exactly 2.0 (interior: norm_deg(30 - 350) / 20 = 40 / 20).
        let mut keys = vec![
            Key::flat(0.0, 350.0),
            Key::flat(10.0, 10.0),
            Key::flat(20.0, 30.0),
        ];
        auto_tangents(&mut keys, true);
        for k in &keys {
            let (i, o) = tangents(k);
            assert!(approx(i, 2.0) && approx(o, 2.0), "wrapped tangents {i}, {o}");
        }
        // Without the flag the same values are read literally:
        // the middle secant is (30 - 350) / 20 = -16.0, the long
        // way around — the flag is what makes the difference.
        let mut keys = vec![
            Key::flat(0.0, 350.0),
            Key::flat(10.0, 10.0),
            Key::flat(20.0, 30.0),
        ];
        auto_tangents(&mut keys, false);
        let (i, _) = tangents(&keys[1]);
        assert!(approx(i, -16.0), "unwrapped middle slope {i}");
    }

    #[test]
    fn straight_line_channel_keeps_its_slope() {
        // Keys exactly on the line v = 10 * f: every secant is
        // 10.0, so smoothing leaves the (already linear) curve's
        // slopes unchanged instead of flattening or bending them.
        let mut keys = vec![
            Key::flat(0.0, 0.0),
            Key::flat(10.0, 100.0),
            Key::flat(25.0, 250.0),
        ];
        auto_tangents(&mut keys, false);
        for k in &keys {
            let (i, o) = tangents(k);
            assert!(approx(i, 10.0) && approx(o, 10.0), "line tangents {i}, {o}");
        }
    }

    #[test]
    fn step_and_linear_keys_still_get_tangents() {
        // The documented decision: interp mode governs the segment
        // a key STARTS, but its in_tan serves the previous segment
        // — so Step/Linear keys are smoothed too, and their modes
        // are not modified.
        let mut keys = vec![
            Key::flat(0.0, 0.0).with_interp(Interp::Linear),
            Key::flat(10.0, 50.0).with_interp(Interp::Step),
            Key::flat(30.0, 90.0),
        ];
        auto_tangents(&mut keys, false);
        let (i, o) = tangents(&keys[1]);
        assert!(approx(i, 3.0) && approx(o, 3.0), "Step key smoothed: {i}, {o}");
        let (i, o) = tangents(&keys[0]);
        assert!(approx(i, 5.0) && approx(o, 5.0), "Linear key smoothed: {i}, {o}");
        assert_eq!(keys[0].interp, Interp::Linear, "mode untouched");
        assert_eq!(keys[1].interp, Interp::Step, "mode untouched");
    }

    #[test]
    fn smooth_channel_sorts_and_picks_angular_from_target() {
        // Angle channel with keys deliberately out of order:
        // the wrapper sorts them, then applies wrapped slopes.
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![
                Key::flat(20.0, 30.0),
                Key::flat(0.0, 350.0),
                Key::flat(10.0, 10.0),
            ],
        };
        smooth_channel(&mut ch);
        assert!(
            approx(ch.keys[0].frame, 0.0)
                && approx(ch.keys[1].frame, 10.0)
                && approx(ch.keys[2].frame, 20.0),
            "wrapper sorts keys: {:?}",
            ch.keys
        );
        for k in &ch.keys {
            let (i, o) = tangents(k);
            assert!(approx(i, 2.0) && approx(o, 2.0), "angle tangents {i}, {o}");
        }
        // A RootX channel through the same wrapper stays plain:
        // middle secant is (90 - 0) / 30 = 3.0, not a wrapped one.
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(30.0, 90.0),
            ],
        };
        smooth_channel(&mut ch);
        let (i, o) = tangents(&ch.keys[1]);
        assert!(approx(i, 3.0) && approx(o, 3.0), "plain tangents {i}, {o}");
    }
}

#[cfg(test)]
mod reduce_tests {
    use super::{reduce_keys, Channel, ChannelTarget, Interp, Key};

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn channel(target: ChannelTarget, keys: Vec<Key>) -> Channel {
        Channel {
            figure: 0,
            target,
            keys,
        }
    }

    /// A Linear-interp key, so segment evaluation is an exact lerp
    /// and hand-computed expectations hold to float precision.
    fn lin(frame: f32, value: f32) -> Key {
        Key::flat(frame, value).with_interp(Interp::Linear)
    }

    #[test]
    fn straight_line_channel_reduces_to_its_endpoints() {
        // Keys exactly on the line v = 10 * f. Under Linear
        // evaluation every interior key is reproduced exactly by
        // the segment skipping it, so all three are removed —
        // each later one tested against the lengthening segment
        // from frame 0, per the greedy pass.
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                lin(0.0, 0.0),
                lin(4.0, 40.0),
                lin(9.0, 90.0),
                lin(15.0, 150.0),
                lin(20.0, 200.0),
            ],
        );
        let removed = reduce_keys(&mut ch, 0.01);
        assert_eq!(removed, 3, "all interior keys removed");
        assert_eq!(ch.keys.len(), 2, "only the endpoints remain");
        assert!(approx(ch.keys[0].frame, 0.0) && approx(ch.keys[0].value, 0.0));
        assert!(approx(ch.keys[1].frame, 20.0) && approx(ch.keys[1].value, 200.0));
    }

    #[test]
    fn spike_key_survives_a_tight_tolerance() {
        // A one-frame-wide spike to 100 on a flat baseline. The
        // spike's own removal test evaluates the flat segment
        // under it (error 100), so it is kept — and so are its
        // immediate neighbours, whose skipping segments run into
        // the spike. Nothing is removed at tolerance 1.0.
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                lin(0.0, 0.0),
                lin(10.0, 0.0),
                lin(11.0, 100.0),
                lin(12.0, 0.0),
                lin(20.0, 0.0),
            ],
        );
        let removed = reduce_keys(&mut ch, 1.0);
        assert_eq!(removed, 0, "the spike protects itself and its neighbours");
        assert_eq!(ch.keys.len(), 5);
        assert!(
            approx(ch.keys[2].frame, 11.0) && approx(ch.keys[2].value, 100.0),
            "the spike key itself survives: {:?}",
            ch.keys[2]
        );
        // The same channel under a tolerance above the spike's
        // height removes every interior key, spike included —
        // tolerance is the only thing protecting it.
        let removed = reduce_keys(&mut ch, 1000.0);
        assert_eq!(removed, 3);
        assert_eq!(ch.keys.len(), 2);
    }

    #[test]
    fn endpoints_survive_any_tolerance() {
        // Extreme endpoint values, negligible interior ones: even
        // a huge tolerance may only ever drop the interior.
        let mut ch = channel(
            ChannelTarget::RootY,
            vec![
                Key::flat(0.0, -500.0),
                Key::flat(3.0, 0.0),
                Key::flat(7.0, 0.0),
                Key::flat(10.0, 500.0),
            ],
        );
        let removed = reduce_keys(&mut ch, 10000.0);
        assert_eq!(removed, 2, "both interior keys go");
        assert_eq!(ch.keys.len(), 2);
        assert!(
            approx(ch.keys[0].frame, 0.0) && approx(ch.keys[0].value, -500.0),
            "first endpoint verbatim"
        );
        assert!(
            approx(ch.keys[1].frame, 10.0) && approx(ch.keys[1].value, 500.0),
            "last endpoint verbatim"
        );
    }

    #[test]
    fn fewer_than_three_keys_are_untouched() {
        // Empty and single-key channels: nothing to test.
        let mut empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(reduce_keys(&mut empty, 0.0), 0);
        assert!(empty.keys.is_empty());
        let mut one = channel(ChannelTarget::RootX, vec![Key::flat(4.0, 42.0)]);
        assert_eq!(reduce_keys(&mut one, 1000.0), 0);
        assert_eq!(one.keys.len(), 1);
        assert!(approx(one.keys[0].value, 42.0));
        // Two keys have no interior — and "untouched" is literal:
        // even an unsorted pair is not reordered (the pass sorts
        // only once it actually runs).
        let mut two = channel(
            ChannelTarget::RootX,
            vec![Key::flat(10.0, 5.0), Key::flat(0.0, 1.0)],
        );
        assert_eq!(reduce_keys(&mut two, 1000.0), 0);
        assert_eq!(two.keys.len(), 2);
        assert!(
            approx(two.keys[0].frame, 10.0) && approx(two.keys[1].frame, 0.0),
            "unsorted pair left exactly as it was"
        );
    }

    #[test]
    fn angle_channel_reduces_across_the_wrap() {
        // Steady rotation through the wrap: 170°, 180°, 190°,
        // 200° stored wrapped as 170, 180, -170, -160 — exactly
        // on the wrapped line v = 170 + f. On an angle channel
        // the shortest-path comparison sees them as collinear, so
        // both interior keys reduce away.
        let keys = vec![
            lin(0.0, 170.0),
            lin(10.0, 180.0),
            lin(20.0, -170.0),
            lin(30.0, -160.0),
        ];
        let mut ch = channel(ChannelTarget::Angle(0), keys.clone());
        let removed = reduce_keys(&mut ch, 0.01);
        assert_eq!(removed, 2, "wrapped interior keys are redundant");
        assert_eq!(ch.keys.len(), 2);
        assert!(approx(ch.keys[0].value, 170.0));
        assert!(approx(ch.keys[1].value, -160.0));
        // The identical numbers on a plain channel must NOT
        // reduce: read literally, 180 -> -170 is a 350° cliff,
        // and every interior key is load-bearing.
        let mut plain = channel(ChannelTarget::RootX, keys);
        let removed = reduce_keys(&mut plain, 0.01);
        assert_eq!(removed, 0, "no wrap awareness, no reduction");
        assert_eq!(plain.keys.len(), 4);
    }

    #[test]
    fn unsorted_input_comes_out_sorted() {
        // Keys on a line but stored out of order: the pass sorts
        // first, reduces, and leaves ascending endpoints.
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![lin(20.0, 200.0), lin(0.0, 0.0), lin(10.0, 100.0)],
        );
        let removed = reduce_keys(&mut ch, 0.01);
        assert_eq!(removed, 1);
        assert_eq!(ch.keys.len(), 2);
        assert!(
            approx(ch.keys[0].frame, 0.0) && approx(ch.keys[1].frame, 20.0),
            "endpoints in ascending order: {:?}",
            ch.keys
        );
    }
}
