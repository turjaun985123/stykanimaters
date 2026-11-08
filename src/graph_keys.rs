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
//!
//! ## Duplicating a channel onto another figure
//!
//! [`duplicate_channel_to_figure`] clones a whole channel —
//! target and every key verbatim — and appends the clone to
//! the project retargeted at another figure. It refuses,
//! without snapshotting, when the channel or the figure does
//! not exist, when the destination is the channel's own
//! figure, when the destination figure already has a channel
//! for the same target (the engine resolves duplicates to
//! the first match, so the clone would be shadowed and
//! silently inert), or when an angle/length target names a
//! node the destination figure's rig does not have.
//!
//! ## Deleting channels and clearing key ranges
//!
//! [`delete_channel`] removes a whole channel from the
//! project — the counterpart [`copy_keys`] and
//! [`duplicate_channel_to_figure`] never had.
//! [`delete_keys_in_range`] clears the keys of one channel
//! whose frames fall inside a range, leaving the channel
//! itself (possibly empty) and every out-of-range key in
//! place; [`keys_in_range`] is its read-only counting half,
//! so a caller can ask "how much would this clear?" before
//! committing. Both mutating ops follow the house
//! discipline: a refusal (no such channel, nothing in the
//! range) sets the status line and takes **no snapshot**;
//! a success snapshots exactly once. Range ends are
//! inclusive, and range statuses display the ends 1-based.
//!
//! ## Channel statistics
//!
//! [`channel_stats`] is the read-only companion: one
//! [`ChannelStat`] per channel, in project order, carrying the
//! numbers a channel list shows next to each row (key count,
//! value range, frame span). It never snapshots, never writes
//! the status line and never mutates the project.

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

/// Duplicate the channel at `channel_index` onto figure
/// `to_figure`: append a clone of the channel — the same
/// target, and every key with its frame, value, tangents and
/// [`Interp`] mode verbatim — with its `figure` retargeted.
/// Frames are NOT rebased; the clone drives the same curve,
/// just on another figure. On success exactly one snapshot is
/// taken, the status line reports the duplication and the
/// function returns `true`.
///
/// Returns `false` — no snapshot, project untouched — when:
/// * no channel sits at `channel_index`;
/// * no figure sits at `to_figure`;
/// * `to_figure` is the channel's own figure (the clone would
///   be an exact duplicate in place);
/// * figure `to_figure` already has a channel for the same
///   target — every core graph lookup resolves a
///   (figure, target) pair to the FIRST matching channel, so
///   the appended clone would be shadowed and never read;
/// * the target is `Angle(n)` or `Length(n)` and figure
///   `to_figure`'s rig has no node `n`.
pub fn duplicate_channel_to_figure(
    core: &mut EditorCore,
    channel_index: usize,
    to_figure: usize,
) -> bool {
    let Some(source) = core.project.channels.get(channel_index).cloned() else {
        core.status = format!("No graph channel {} to duplicate.", channel_index + 1);
        return false;
    };
    if to_figure >= core.project.figures.len() {
        core.status = format!(
            "No figure {} to duplicate graph channel {} onto.",
            to_figure + 1,
            channel_index + 1
        );
        return false;
    }
    if to_figure == source.figure {
        core.status = format!(
            "Graph channel {} already belongs to figure {}.",
            channel_index + 1,
            to_figure + 1
        );
        return false;
    }
    if channel_index_of(core, to_figure, source.target).is_some() {
        core.status = format!(
            "Figure {} already has a channel for {} — a duplicate would be shadowed.",
            to_figure + 1,
            target_label(source.target)
        );
        return false;
    }
    // Angle and length targets name a node on the rig; the
    // destination figure must actually have that node, or the
    // cloned channel would drive nothing.
    let node_index = match source.target {
        ChannelTarget::Angle(n) | ChannelTarget::Length(n) => Some(n),
        ChannelTarget::RootX | ChannelTarget::RootY => None,
    };
    if let Some(n) = node_index {
        if n >= core.project.figures[to_figure].def.nodes.len() {
            core.status = format!(
                "Figure {} has no node {} for {} — nothing to duplicate onto.",
                to_figure + 1,
                n + 1,
                target_label(source.target)
            );
            return false;
        }
    }
    let key_count = source.keys.len();
    core.snapshot();
    let mut copy = source;
    copy.figure = to_figure;
    core.project.channels.push(copy);
    core.status = format!(
        "Duplicated graph channel {} onto figure {} ({} key(s)).",
        channel_index + 1,
        to_figure + 1,
        key_count
    );
    true
}

/// Display text for a frame position, mirroring the FX
/// ops' private `frame_label` (in `fx_ops.rs`): whole
/// frames render without a fraction (`4`), fractional ones
/// with one decimal (`4.5`). That helper is private to its
/// module, so the formatting is duplicated here and the two
/// must be kept in step — the same arrangement
/// [`target_label`] has with the graph panel. Range
/// statuses pass `frame + 1.0`, so the ends display
/// 1-based, the FX ops' convention.
fn frame_label(frame: f32) -> String {
    if frame.fract() == 0.0 {
        format!("{}", frame as i64)
    } else {
        format!("{frame:.1}")
    }
}

/// Delete the channel at `channel_index` (an index into
/// `core.project.channels`) from the project, keys and all.
/// On success exactly one snapshot is taken, the channel is
/// removed and the status line names what went — its
/// 1-based channel number, its target's label and its key
/// count; returns `true`.
///
/// Returns `false` — no snapshot, project untouched — when
/// no channel sits at `channel_index`.
pub fn delete_channel(core: &mut EditorCore, channel_index: usize) -> bool {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to delete.", channel_index + 1);
        return false;
    };
    let label = target_label(channel.target);
    let key_count = channel.keys.len();
    core.snapshot();
    core.project.channels.remove(channel_index);
    core.status = format!(
        "Deleted graph channel {} ({}, {} key(s)).",
        channel_index + 1,
        label,
        key_count
    );
    true
}

/// How many keys of the (`figure`, `target`) channel have
/// a frame inside `[from, to]`, both ends inclusive. The
/// channel is resolved exactly like the core's own graph
/// ops resolve one — the first matching channel — so with
/// duplicate channels only the first is counted. No channel
/// for the pair reads as 0. Pure and read-only: no
/// snapshot, no status write, no mutation.
pub fn keys_in_range(
    core: &EditorCore,
    figure: usize,
    target: ChannelTarget,
    from: f32,
    to: f32,
) -> usize {
    let Some(index) = channel_index_of(core, figure, target) else {
        return 0;
    };
    core.project.channels[index]
        .keys
        .iter()
        .filter(|k| (from..=to).contains(&k.frame))
        .count()
}

/// Clear every key of the (`figure`, `target`) channel
/// whose frame lies inside `[from, to]`, both ends
/// inclusive, and return how many keys were removed. Keys
/// outside the range are untouched, and the channel itself
/// is left in place even when the clear empties it — an
/// empty channel is a valid state (see [`channel_stats`]).
/// On success exactly one snapshot is taken and the status
/// line reports the count and the range, its ends displayed
/// 1-based via [`frame_label`].
///
/// Returns 0 — no snapshot, project untouched — when the
/// project has no channel for the pair, or when the channel
/// holds no keys in the range; the status line reports the
/// range either way, so a UI binding never fails silently.
pub fn delete_keys_in_range(
    core: &mut EditorCore,
    figure: usize,
    target: ChannelTarget,
    from: f32,
    to: f32,
) -> usize {
    let range = format!("{}–{}", frame_label(from + 1.0), frame_label(to + 1.0));
    let Some(index) = channel_index_of(core, figure, target) else {
        core.status = format!("No keys to clear in frames {range}.");
        return 0;
    };
    let count = keys_in_range(core, figure, target, from, to);
    if count == 0 {
        core.status = format!("No keys to clear in frames {range}.");
        return 0;
    }
    core.snapshot();
    core.project.channels[index]
        .keys
        .retain(|k| !(from..=to).contains(&k.frame));
    core.status = format!("Cleared {count} key(s) in frames {range}.");
    count
}

/// Human label for a channel target, in UI casing: "Root X",
/// "Root Y", "Angle 3", "Length 3". This mirrors the graph
/// panel's own `channel_label` (in `rune_sec/graph_panel.rs`)
/// string for string — that helper lives in the shell crate
/// and cannot be reused from the library, so the formatting is
/// duplicated here and the two must be kept in step.
pub fn target_label(target: ChannelTarget) -> String {
    match target {
        ChannelTarget::RootX => "Root X".to_string(),
        ChannelTarget::RootY => "Root Y".to_string(),
        ChannelTarget::Angle(n) => format!("Angle {n}"),
        ChannelTarget::Length(n) => format!("Length {n}"),
    }
}

/// Read-only summary of one channel, for a channel list that
/// wants numbers next to each row: which figure and property
/// the channel drives, how many keys it holds, the range of
/// its key values and the span of its key frames.
///
/// An empty channel (no keys) reports `keys` 0 and zeroes for
/// `min`, `max`, `first_frame` and `last_frame` — there is no
/// range to report, and zero keeps the row printable without a
/// special case. `figure_name` is the figure's current name;
/// a channel pointing past the end of the figure list (stale,
/// e.g. after a figure delete) falls back to "Figure {n}"
/// with the 1-based index, so the row still names something.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStat {
    /// Index of the figure the channel drives.
    pub figure: usize,
    /// The figure's name (see the struct docs for the stale
    /// fallback).
    pub figure_name: String,
    /// The target's label, from [`target_label`].
    pub target_label: String,
    /// Number of keys on the channel.
    pub keys: usize,
    /// Smallest key value; 0.0 on an empty channel.
    pub min: f32,
    /// Largest key value; 0.0 on an empty channel.
    pub max: f32,
    /// Frame of the earliest key; 0.0 on an empty channel.
    pub first_frame: f32,
    /// Frame of the latest key; 0.0 on an empty channel.
    pub last_frame: f32,
}

/// One [`ChannelStat`] per channel in `core.project.channels`,
/// in project channel order. Pure and read-only: no snapshot,
/// no status write, no mutation — safe to call on every frame
/// a list is drawn. Value and frame extremes are folded over
/// the keys as stored, without assuming they are sorted, so a
/// hand-built channel reports its true span.
pub fn channel_stats(core: &EditorCore) -> Vec<ChannelStat> {
    core.project
        .channels
        .iter()
        .map(|ch| {
            let figure_name = core
                .project
                .figures
                .get(ch.figure)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("Figure {}", ch.figure + 1));
            let (mut min, mut max) = (0.0f32, 0.0f32);
            let (mut first_frame, mut last_frame) = (0.0f32, 0.0f32);
            if let Some(first) = ch.keys.first() {
                min = first.value;
                max = first.value;
                first_frame = first.frame;
                last_frame = first.frame;
                for k in &ch.keys[1..] {
                    min = min.min(k.value);
                    max = max.max(k.value);
                    first_frame = first_frame.min(k.frame);
                    last_frame = last_frame.max(k.frame);
                }
            }
            ChannelStat {
                figure: ch.figure,
                figure_name,
                target_label: target_label(ch.target),
                keys: ch.keys.len(),
                min,
                max,
                first_frame,
                last_frame,
            }
        })
        .collect()
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

    /// [`core_with_channels`] plus a second figure at index 1
    /// (a renamed clone of the demo figure, full 11-node rig)
    /// with no channels of its own.
    fn core_with_two_figures() -> EditorCore {
        let mut core = core_with_channels();
        let mut second = core.project.figures[0].clone();
        second.name = "Second".to_string();
        core.project.figures.push(second);
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
    fn duplicate_channel_clones_keys_onto_the_other_figure() {
        let mut core = core_with_two_figures();
        let source_keys = core.project.channels[0].keys.clone();
        let undo_before = core.undo_depth();
        let edits_before = core.edits_since_autosave();

        assert!(duplicate_channel_to_figure(&mut core, 0, 1));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "the snapshot is the only edit"
        );

        assert_eq!(core.project.channels.len(), 3, "clone appended");
        let copy = &core.project.channels[2];
        assert_eq!(copy.figure, 1, "clone retargeted at figure 1");
        assert_eq!(copy.target, ChannelTarget::RootX, "target travels verbatim");
        assert_eq!(
            copy.keys, source_keys,
            "every key cloned exactly — frames, values, tangents, modes"
        );
        // The source channel is untouched.
        assert_eq!(core.project.channels[0].figure, 0);
        assert_eq!(core.project.channels[0].keys, source_keys);
        assert!(
            core.status.contains("Duplicated graph channel 1 onto figure 2"),
            "status reports the duplication: {}",
            core.status
        );

        // And it is a single undo step: undoing removes
        // exactly the clone.
        core.undo();
        assert_eq!(core.project.channels.len(), 2, "undo reverts the duplication");
    }

    #[test]
    fn duplicate_channel_refuses_bad_indices_and_same_figure_without_snapshot() {
        let mut core = core_with_two_figures();
        let undo_before = core.undo_depth();

        assert!(!duplicate_channel_to_figure(&mut core, 99, 1), "no such channel");
        assert!(
            core.status.contains("No graph channel 100"),
            "status: {}",
            core.status
        );
        assert!(!duplicate_channel_to_figure(&mut core, 0, 99), "no such figure");
        assert!(
            core.status.contains("No figure 100"),
            "status: {}",
            core.status
        );
        assert!(
            !duplicate_channel_to_figure(&mut core, 0, 0),
            "channel already belongs to figure 0"
        );
        assert!(
            core.status.contains("already belongs to figure 1"),
            "status: {}",
            core.status
        );

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.channels.len(), 2, "no refusal appended a channel");
    }

    #[test]
    fn duplicate_channel_refuses_when_the_target_already_has_a_channel() {
        let mut core = core_with_two_figures();
        assert!(duplicate_channel_to_figure(&mut core, 0, 1), "first duplication lands");
        let undo_before = core.undo_depth();

        assert!(
            !duplicate_channel_to_figure(&mut core, 0, 1),
            "a second clone would be shadowed by the first"
        );
        assert!(
            core.status.contains("already has a channel for Root X"),
            "status must say a channel for that target already exists: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "refusal took no snapshot");
        assert_eq!(core.project.channels.len(), 3, "still exactly one clone");

        // The same refusal when the blocking channel was
        // already in the project rather than made here:
        // figure 1's own RootY blocks figure 0's RootY.
        core.project.channels.push(Channel {
            figure: 1,
            target: ChannelTarget::RootY,
            keys: Vec::new(),
        });
        let undo_before = core.undo_depth();
        assert!(!duplicate_channel_to_figure(&mut core, 1, 1));
        assert!(
            core.status.contains("already has a channel for Root Y"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "refusal took no snapshot");
    }

    #[test]
    fn duplicate_channel_refuses_a_node_the_target_rig_lacks() {
        let mut core = core_with_two_figures();
        // Shrink figure 1's rig to three nodes (indices 0..=2).
        core.project.figures[1].def.nodes.truncate(3);
        // Angle and length channels for node 5 are valid on
        // figure 0's full rig but name no node on figure 1's.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(5),
            keys: vec![Key::flat(0.0, 30.0)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Length(5),
            keys: vec![Key::flat(0.0, 40.0)],
        });
        let undo_before = core.undo_depth();

        assert!(!duplicate_channel_to_figure(&mut core, 2, 1), "angle node missing");
        assert!(
            core.status.contains("has no node 6"),
            "status: {}",
            core.status
        );
        assert!(!duplicate_channel_to_figure(&mut core, 3, 1), "length node missing");
        assert!(
            core.status.contains("has no node 6"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "no snapshot for either refusal");
        assert_eq!(core.project.channels.len(), 4, "nothing appended");

        // A node the small rig DOES have duplicates fine.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(2),
            keys: vec![Key::flat(1.0, -10.0)],
        });
        assert!(
            duplicate_channel_to_figure(&mut core, 4, 1),
            "node 2 exists on both rigs"
        );
        let copy = core.project.channels.last().expect("clone appended");
        assert_eq!(copy.figure, 1);
        assert_eq!(copy.target, ChannelTarget::Angle(2));
        assert_eq!(copy.keys.len(), 1);
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

    #[test]
    fn target_label_matches_the_graph_panels_casing() {
        // The strings are the graph panel's `channel_label`
        // output verbatim — the two formatters must not drift.
        assert_eq!(target_label(ChannelTarget::RootX), "Root X");
        assert_eq!(target_label(ChannelTarget::RootY), "Root Y");
        assert_eq!(target_label(ChannelTarget::Angle(3)), "Angle 3");
        assert_eq!(target_label(ChannelTarget::Length(12)), "Length 12");
    }

    #[test]
    fn channel_stats_reports_every_field_and_stays_read_only() {
        let mut core = core_with_channels();
        // Pin the figure's name so the assertion does not lean
        // on the demo project's naming.
        core.project.figures[0].name = "Hero".to_string();
        let edits_before = core.edits_since_autosave();
        let status_before = core.status.clone();

        let stats = channel_stats(&core);
        assert_eq!(stats.len(), 2, "one stat per channel, project order");

        // Channel 0: RootX with keys (10, 100), (14, 140),
        // (20, 90) — value range 90..=140, frame span 10..=20.
        let s0 = &stats[0];
        assert_eq!(s0.figure, 0);
        assert_eq!(s0.figure_name, "Hero");
        assert_eq!(s0.target_label, "Root X");
        assert_eq!(s0.keys, 3);
        assert!(approx(s0.min, 90.0), "min value: {}", s0.min);
        assert!(approx(s0.max, 140.0), "max value: {}", s0.max);
        assert!(approx(s0.first_frame, 10.0), "first frame: {}", s0.first_frame);
        assert!(approx(s0.last_frame, 20.0), "last frame: {}", s0.last_frame);

        // Channel 1: RootY, empty — the documented zeroes.
        let s1 = &stats[1];
        assert_eq!(s1.figure, 0);
        assert_eq!(s1.figure_name, "Hero");
        assert_eq!(s1.target_label, "Root Y");
        assert_eq!(s1.keys, 0);
        assert!(approx(s1.min, 0.0), "empty min is 0.0");
        assert!(approx(s1.max, 0.0), "empty max is 0.0");
        assert!(approx(s1.first_frame, 0.0), "empty first frame is 0.0");
        assert!(approx(s1.last_frame, 0.0), "empty last frame is 0.0");

        // Pure read: no snapshot taken, no status written.
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "channel_stats takes no snapshot"
        );
        assert_eq!(core.status, status_before, "channel_stats writes no status");
    }

    #[test]
    fn channel_stats_names_a_stale_figure_by_number() {
        let mut core = core_with_channels();
        // A channel whose figure no longer exists still gets a
        // printable row: the 1-based fallback name, and its own
        // key numbers computed as usual.
        core.project.channels.push(Channel {
            figure: 7,
            target: ChannelTarget::Angle(2),
            keys: vec![Key::flat(3.0, -45.0)],
        });
        let stats = channel_stats(&core);
        let stale = &stats[2];
        assert_eq!(stale.figure, 7);
        assert_eq!(stale.figure_name, "Figure 8");
        assert_eq!(stale.target_label, "Angle 2");
        assert_eq!(stale.keys, 1);
        assert!(approx(stale.min, -45.0) && approx(stale.max, -45.0));
        assert!(approx(stale.first_frame, 3.0) && approx(stale.last_frame, 3.0));
    }

    #[test]
    fn channel_stats_empty_project_is_empty() {
        let core = EditorCore::new();
        assert!(core.project.channels.is_empty(), "demo premise");
        assert!(channel_stats(&core).is_empty());
    }

    #[test]
    fn delete_channel_removes_it_and_reports_what_went() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();

        assert!(delete_channel(&mut core, 0));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        assert_eq!(core.project.channels.len(), 1, "one channel remains");
        let remaining = &core.project.channels[0];
        assert_eq!(remaining.target, ChannelTarget::RootY, "the RootY channel survives");
        assert!(
            core.status.contains("Deleted graph channel 1 (Root X, 3 key(s))."),
            "status names the channel, its target and its key count: {}",
            core.status
        );

        // And it is a single undo step: undoing restores the
        // channel with its keys intact.
        core.undo();
        assert_eq!(core.project.channels.len(), 2, "undo restores the channel");
        assert_eq!(core.project.channels[0].keys.len(), 3);
        assert!(approx(core.project.channels[0].keys[0].frame, 10.0));
    }

    #[test]
    fn delete_channel_among_duplicates_removes_the_named_one() {
        let mut core = core_with_channels();
        // A second RootX channel for figure 0, told apart by
        // its single key — deletion is by index, so each twin
        // can be removed without touching the other.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(3.0, 42.0)],
        });

        assert!(delete_channel(&mut core, 2), "remove the appended twin");
        assert_eq!(core.project.channels.len(), 2);
        assert_eq!(
            core.project.channels[0].keys.len(),
            3,
            "the original RootX channel is untouched"
        );
        assert!(
            core.status.contains("Deleted graph channel 3 (Root X, 1 key(s))."),
            "status: {}",
            core.status
        );

        // Deleting index 0 now removes the original; the
        // RootY channel shifts down but is unharmed.
        assert!(delete_channel(&mut core, 0));
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(core.project.channels[0].target, ChannelTarget::RootY);
    }

    #[test]
    fn delete_channel_refuses_out_of_range_without_snapshot() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();
        let channels_before = core.project.channels.clone();

        assert!(!delete_channel(&mut core, 99), "no such channel");
        assert!(
            core.status.contains("No graph channel 100 to delete."),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "refusal took no snapshot");
        assert_eq!(core.project.channels, channels_before, "project untouched");
    }

    #[test]
    fn keys_in_range_counts_inclusively_and_stays_read_only() {
        let mut core = core_with_channels();
        let edits_before = core.edits_since_autosave();
        let status_before = core.status.clone();
        // Channel 0 (RootX) holds keys at frames 10, 14, 20.
        let count = |core: &EditorCore, from: f32, to: f32| {
            keys_in_range(core, 0, ChannelTarget::RootX, from, to)
        };
        assert_eq!(count(&core, 10.0, 20.0), 3, "the whole span");
        assert_eq!(count(&core, 10.0, 14.0), 2, "both ends inclusive");
        assert_eq!(count(&core, 14.0, 14.0), 1, "a single-frame range on a key");
        assert_eq!(count(&core, 11.0, 19.0), 1, "only the middle key");
        assert_eq!(count(&core, 0.0, 9.0), 0, "before every key");
        assert_eq!(count(&core, 21.0, 30.0), 0, "after every key");
        // No channel for the pair reads as 0.
        assert_eq!(keys_in_range(&core, 0, ChannelTarget::Angle(1), 0.0, 100.0), 0);
        assert_eq!(keys_in_range(&core, 3, ChannelTarget::RootX, 0.0, 100.0), 0);
        // Pure read: no snapshot, no status write.
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "keys_in_range takes no snapshot"
        );
        assert_eq!(core.status, status_before, "keys_in_range writes no status");

        // With a duplicate channel present, only the FIRST
        // match is counted — the core's resolution rule.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(12.0, 1.0), Key::flat(13.0, 2.0)],
        });
        assert_eq!(count(&core, 10.0, 20.0), 3, "the shadowed twin is not counted");
    }

    #[test]
    fn delete_keys_in_range_clears_only_the_range() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();

        // Keys at 10/14/20; clearing 14..=20 removes two and
        // leaves the frame-10 key exactly as it was.
        assert_eq!(
            delete_keys_in_range(&mut core, 0, ChannelTarget::RootX, 14.0, 20.0),
            2
        );
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 1, "only the out-of-range key survives");
        assert!(approx(ch.keys[0].frame, 10.0));
        assert!(approx(ch.keys[0].value, 100.0), "survivor's value untouched");
        assert!(
            approx(ch.keys[0].in_tan, 1.5) && approx(ch.keys[0].out_tan, -2.0),
            "survivor's tangents untouched"
        );
        assert_eq!(ch.keys[0].interp, Interp::Linear, "survivor's interp untouched");
        assert!(
            core.status.contains("Cleared 2 key(s) in frames 15–21."),
            "status reports the count and the range, ends shown 1-based: {}",
            core.status
        );

        // Undoing restores all three keys.
        core.undo();
        assert_eq!(core.project.channels[0].keys.len(), 3, "undo restores the clear");
    }

    #[test]
    fn delete_keys_in_range_edges_are_inclusive() {
        let mut core = core_with_channels();
        // A range whose ends sit exactly on keys clears
        // those keys — and only those.
        assert_eq!(
            delete_keys_in_range(&mut core, 0, ChannelTarget::RootX, 10.0, 10.0),
            1,
            "the key on the range's single frame goes"
        );
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames.len(), 2);
        assert!(approx(frames[0], 14.0) && approx(frames[1], 20.0));
    }

    #[test]
    fn delete_keys_in_range_may_leave_the_channel_empty() {
        let mut core = core_with_channels();
        assert_eq!(
            delete_keys_in_range(&mut core, 0, ChannelTarget::RootX, 0.0, 100.0),
            3,
            "every key is in range"
        );
        assert_eq!(
            core.project.channels.len(),
            2,
            "the channel itself is NOT deleted"
        );
        assert!(
            core.project.channels[0].keys.is_empty(),
            "it is left empty, which is a valid state"
        );
        assert!(
            core.status.contains("Cleared 3 key(s) in frames 1–101."),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn delete_keys_in_range_refuses_without_snapshot_when_nothing_matches() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();
        let channels_before = core.project.channels.clone();

        // A range holding no keys of an existing channel.
        assert_eq!(
            delete_keys_in_range(&mut core, 0, ChannelTarget::RootX, 30.0, 40.0),
            0
        );
        assert!(
            core.status.contains("No keys to clear in frames 31–41."),
            "status: {}",
            core.status
        );
        // No channel for the pair at all — same refusal.
        assert_eq!(
            delete_keys_in_range(&mut core, 0, ChannelTarget::Angle(1), 30.0, 40.0),
            0
        );
        assert!(
            core.status.contains("No keys to clear in frames 31–41."),
            "status: {}",
            core.status
        );

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.channels, channels_before, "project untouched");
    }
}
