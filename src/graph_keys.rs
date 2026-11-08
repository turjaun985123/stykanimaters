//! Graph key clipboard: copy a channel's keys out of the
//! project and paste them into another (or the same) channel,
//! rebased to land at a chosen frame — the "copy these keys,
//! paste them over there" operation of a graph editor.
//!
//! The clipboard itself lives with the caller (a view, an MCP
//! tool): [`copy_keys`] hands back an owned `Vec<Key>` and
//! [`paste_keys`] consumes any such slice, so nothing here
//! holds state between the two calls and the same pair of
//! functions serves every front-end.
//!
//! ## Paste semantics
//!
//! * **Rebase.** The pasted keys keep their relative spacing:
//!   the earliest key in the slice lands exactly on `at_frame`
//!   and every other key shifts by the same offset. Values,
//!   tangents and [`Interp`] modes travel verbatim — only
//!   frames move.
//! * **Replace near, keep far.** Before the pasted keys are
//!   inserted, every existing key of the destination channel
//!   that sits within 0.5 frames of a pasted key's (rebased)
//!   frame is removed — the tolerance and the removal itself
//!   are [`Channel::remove_key`]'s, applied repeatedly until
//!   the neighbourhood is clear, so two crowded existing keys
//!   near one pasted frame both go. Keys farther than 0.5
//!   frames from every pasted key are untouched, and pasted
//!   keys never remove each other, however dense the slice.
//! * **One undo step.** A successful paste snapshots exactly
//!   once, before mutating, through [`EditorCore::snapshot`].
//! * **No-op cases.** A missing destination channel or an
//!   empty key slice returns `false` with **no snapshot taken**
//!   and the project untouched; the status line still reports
//!   why, in the core's house style, so a UI binding never
//!   fails silently.

use crate::editor::EditorCore;
use crate::graph::{ChannelTarget, Key};

/// Index of the channel for (`figure`, `target`) in
/// `core.project.channels` — the first match, which is the
/// lookup every core graph op performs (see
/// `EditorCore::set_graph_key_interp`). `None` when the
/// project has no channel for that property.
pub fn channel_index_of(core: &EditorCore, figure: usize, target: ChannelTarget) -> Option<usize> {
    core.project
        .channels
        .iter()
        .position(|ch| ch.figure == figure && ch.target == target)
}

/// Clone the keys of the channel at `channel_index` (an index
/// into `core.project.channels`) for later pasting. The result
/// is fully owned: mutating the channel afterwards cannot
/// change it. `None` when no channel sits at that index; a
/// channel that exists but has no keys yields `Some(vec![])`.
pub fn copy_keys(core: &EditorCore, channel_index: usize) -> Option<Vec<Key>> {
    core.project
        .channels
        .get(channel_index)
        .map(|ch| ch.keys.clone())
}

/// Paste `keys` into the channel at `channel_index`, rebased
/// so the slice's earliest key lands at `at_frame`, replacing
/// existing keys within 0.5 frames of a pasted key (see the
/// module docs for the full semantics). On success the
/// channel's keys are re-sorted and the status line reports
/// the paste; returns `true`. Returns `false` — no snapshot,
/// project untouched — when the channel does not exist or
/// `keys` is empty.
pub fn paste_keys(
    core: &mut EditorCore,
    channel_index: usize,
    keys: &[Key],
    at_frame: f32,
) -> bool {
    if keys.is_empty() {
        core.status = "Nothing to paste — copy some graph keys first.".into();
        return false;
    }
    if channel_index >= core.project.channels.len() {
        core.status = format!("No graph channel {} to paste into.", channel_index + 1);
        return false;
    }
    // Rebase: the slice's earliest key lands on `at_frame`;
    // every other key keeps its offset from it.
    let earliest = keys
        .iter()
        .map(|k| k.frame)
        .fold(f32::INFINITY, f32::min);
    let offset = at_frame - earliest;
    let rebased: Vec<Key> = keys
        .iter()
        .map(|k| Key {
            frame: k.frame + offset,
            ..*k
        })
        .collect();

    core.snapshot();
    let channel = &mut core.project.channels[channel_index];
    // Clear the neighbourhood of every pasted key BEFORE
    // inserting any of them, so pasted keys can never remove
    // each other. `remove_key` takes the single nearest key
    // within 0.5 frames; repeat until none is left in range.
    for pk in &rebased {
        while channel.remove_key(pk.frame) {}
    }
    channel.keys.extend(rebased.iter().copied());
    channel.sort_keys();
    core.status = format!(
        "Pasted {} graph key(s) at frame {} into channel {}.",
        rebased.len(),
        at_frame,
        channel_index + 1
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Channel, Interp};

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn key(frame: f32, value: f32, in_tan: f32, out_tan: f32, interp: Interp) -> Key {
        Key {
            frame,
            value,
            in_tan,
            out_tan,
            interp,
        }
    }

    /// A core whose project carries two channels: index 0 is
    /// figure 0's RootX with three tangent-carrying keys at
    /// frames 10/14/20, index 1 is figure 0's RootY, empty.
    fn core_with_channels() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                key(10.0, 100.0, 1.5, -2.0, Interp::Linear),
                key(14.0, 140.0, 0.5, 0.25, Interp::Hermite),
                key(20.0, 90.0, -1.0, 3.0, Interp::Step),
            ],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootY,
            keys: Vec::new(),
        });
        core
    }

    #[test]
    fn copy_clones_keys_and_handles_missing_and_empty() {
        let mut core = core_with_channels();
        let copied = copy_keys(&core, 0).expect("channel 0 exists");
        assert_eq!(copied.len(), 3);
        assert!(approx(copied[0].frame, 10.0) && approx(copied[0].value, 100.0));
        assert!(
            approx(copied[0].in_tan, 1.5) && approx(copied[0].out_tan, -2.0),
            "tangents are part of the copy"
        );
        assert_eq!(copied[0].interp, Interp::Linear, "interp is part of the copy");

        // The copy is owned: later edits to the channel must
        // not leak into it.
        core.project.channels[0].keys[0].value = -999.0;
        core.project.channels[0].keys.clear();
        assert_eq!(copied.len(), 3);
        assert!(approx(copied[0].value, 100.0), "copy survives channel edits");

        // An existing but empty channel copies as an empty vec.
        let empty = copy_keys(&core, 1).expect("channel 1 exists");
        assert!(empty.is_empty());
        // A missing channel copies as None.
        assert!(copy_keys(&core, 99).is_none());
    }

    #[test]
    fn paste_rebases_and_preserves_everything_else() {
        let mut core = core_with_channels();
        let copied = copy_keys(&core, 0).unwrap();
        let edits_before = core.edits_since_autosave();

        // Paste into the other (empty) channel, earliest key
        // landing at frame 2: 10/14/20 rebase to 2/6/12.
        assert!(paste_keys(&mut core, 1, &copied, 2.0));
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "a successful paste is exactly one snapshot"
        );
        let dest = &core.project.channels[1];
        assert_eq!(dest.keys.len(), 3);
        let frames: Vec<f32> = dest.keys.iter().map(|k| k.frame).collect();
        assert!(
            approx(frames[0], 2.0) && approx(frames[1], 6.0) && approx(frames[2], 12.0),
            "rebased frames, sorted: {frames:?}"
        );
        // Values, tangents and interps are the source's, verbatim.
        for (got, want) in dest.keys.iter().zip(copied.iter()) {
            assert!(approx(got.value, want.value), "value preserved");
            assert!(approx(got.in_tan, want.in_tan), "in_tan preserved");
            assert!(approx(got.out_tan, want.out_tan), "out_tan preserved");
            assert_eq!(got.interp, want.interp, "interp preserved");
        }
        assert!(
            core.status.contains("Pasted 3 graph key(s)"),
            "status reports the paste: {}",
            core.status
        );
        // The source channel is untouched by pasting out of it.
        assert_eq!(core.project.channels[0].keys.len(), 3);
        assert!(approx(core.project.channels[0].keys[0].frame, 10.0));

        // And the paste is a real undo step: undoing restores
        // the destination's emptiness.
        core.undo();
        assert!(
            core.project.channels[1].keys.is_empty(),
            "undo reverts the paste"
        );
    }

    #[test]
    fn paste_into_same_channel_at_a_new_frame() {
        let mut core = core_with_channels();
        let copied = copy_keys(&core, 0).unwrap();
        // Rebase 10/14/20 onto 30: the originals are far from
        // every pasted frame, so all six keys coexist, sorted.
        assert!(paste_keys(&mut core, 0, &copied, 30.0));
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 6);
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert!(
            approx(frames[0], 10.0) && approx(frames[5], 40.0),
            "originals and pasted keys interleave sorted: {frames:?}"
        );
    }

    #[test]
    fn paste_replaces_keys_within_half_a_frame_only() {
        let mut core = core_with_channels();
        // Destination: keys at 4.6 and 5.4 crowd the paste
        // frame 5.0 (both within 0.5), one at 5.6 is 0.6 away
        // (outside), one at 9.0 is far.
        core.project.channels[1].keys = vec![
            Key::flat(4.6, 111.0),
            Key::flat(5.4, 222.0),
            Key::flat(5.6, 333.0),
            Key::flat(9.0, 444.0),
        ];
        let single = vec![key(0.0, 777.0, 2.0, 2.0, Interp::Hermite)];
        assert!(paste_keys(&mut core, 1, &single, 5.0));
        let dest = &core.project.channels[1];
        assert_eq!(
            dest.keys.len(),
            3,
            "both crowded keys replaced, the rest kept: {:?}",
            dest.keys
        );
        let values: Vec<f32> = dest.keys.iter().map(|k| k.value).collect();
        assert!(
            approx(values[0], 777.0) && approx(values[1], 333.0) && approx(values[2], 444.0),
            "pasted key landed, 0.6-away and far keys survived: {values:?}"
        );
        assert!(approx(dest.keys[0].frame, 5.0), "pasted key sits at 5.0");
    }

    #[test]
    fn paste_noops_without_snapshot_on_missing_channel_or_empty_keys() {
        let mut core = core_with_channels();
        let edits_before = core.edits_since_autosave();
        let keys_before: Vec<usize> = core
            .project
            .channels
            .iter()
            .map(|ch| ch.keys.len())
            .collect();

        let some = vec![Key::flat(0.0, 1.0)];
        assert!(!paste_keys(&mut core, 99, &some, 0.0), "missing channel");
        assert!(!paste_keys(&mut core, 0, &[], 0.0), "empty slice");

        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "no snapshot was taken for either no-op"
        );
        let keys_after: Vec<usize> = core
            .project
            .channels
            .iter()
            .map(|ch| ch.keys.len())
            .collect();
        assert_eq!(keys_before, keys_after, "project channels untouched");
    }

    #[test]
    fn channel_index_of_finds_first_match() {
        let mut core = core_with_channels();
        assert_eq!(channel_index_of(&core, 0, ChannelTarget::RootX), Some(0));
        assert_eq!(channel_index_of(&core, 0, ChannelTarget::RootY), Some(1));
        assert_eq!(channel_index_of(&core, 0, ChannelTarget::Angle(1)), None);
        assert_eq!(channel_index_of(&core, 3, ChannelTarget::RootX), None);
        // A duplicate (figure, target) channel resolves to the
        // first one, like the core's own graph ops.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: Vec::new(),
        });
        assert_eq!(channel_index_of(&core, 0, ChannelTarget::RootX), Some(0));
    }
}
