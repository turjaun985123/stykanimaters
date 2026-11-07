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

/// One keyframe on a channel. Tangents are slopes in value-units
/// per frame: `out_tan` is the curve's slope as it leaves this key,
/// `in_tan` its slope as it arrives at this key.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub frame: f32,
    pub value: f32,
    pub in_tan: f32,
    pub out_tan: f32,
}

impl Key {
    /// A key with flat (zero) tangents.
    pub fn flat(frame: f32, value: f32) -> Self {
        Key {
            frame,
            value,
            in_tan: 0.0,
            out_tan: 0.0,
        }
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
    /// Between two keys the value is the cubic Hermite spline
    /// through them, using the left key's `out_tan` and the right
    /// key's `in_tan` (converted from per-frame slopes to the
    /// segment's normalised parameter). Outside the key range the
    /// value clamps to the nearest key; at a key frame the stored
    /// value is returned exactly. A channel with no keys yields 0.0.
    ///
    /// Angle channels interpolate on the shortest circular path:
    /// each segment's end value is unwrapped relative to its start
    /// (350° → 10° travels through 0°, never the long way through
    /// 180°), and the interpolated result is normalised to
    /// (-180, 180], matching `Pose::lerp`'s convention. Tangents on
    /// an angle channel are interpreted on that unwrapped path.
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
        // Hermite derivatives are with respect to t, so scale the
        // per-frame slopes by the segment's frame span.
        let m0 = k0.out_tan * dt;
        let m1 = k1.in_tan * dt;
        let v0 = k0.value;
        let v1 = if self.is_angle() {
            // Unwrap the end value onto the short path from v0.
            v0 + norm_deg(k1.value - v0)
        } else {
            k1.value
        };
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        let v = h00 * v0 + h10 * m0 + h01 * v1 + h11 * m1;
        if self.is_angle() {
            norm_deg(v)
        } else {
            v
        }
    }
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
}
