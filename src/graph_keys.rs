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
//! ## Per-key interpolation
//!
//! [`set_key_interp`] sets one key's [`Interp`] mode — the
//! per-key counterpart of the editor core's own
//! `set_graph_key_interp`, which only reaches the key nearest
//! the playhead on the selected figure's angle channel; this
//! one addresses any channel and any key by index, so a graph
//! panel or an MCP tool can set the mode of the exact key it
//! is showing. [`key_interp_at`] is its read-only half: the
//! mode of the key sitting at a given frame, resolved with
//! the shell's key-at-frame convention (see its docs). The
//! setter follows the house discipline: a refusal sets the
//! status line and takes **no snapshot**; a success snapshots
//! exactly once.
//!
//! ## Shifting and filling keys
//!
//! [`shift_keys`] moves every key of one channel by the
//! same frame offset — the graph editor's "nudge the whole
//! curve" operation. It is all-or-nothing: a shift that
//! would land any key before frame 0 is refused whole,
//! never clamped per key, so the curve is never silently
//! distorted. [`fill_keys_in_range`] is the keyframe
//! panel's "fill the loop range" operation: it copies the
//! key sitting at the playhead onto every keyless frame of
//! a range, turning a held value into per-frame keys. Both
//! follow the house discipline: a refusal (no such
//! channel, a useless offset, an empty channel, a reversed
//! range, nothing to fill from, nothing to fill) sets the
//! status line and takes **no snapshot**; a success
//! snapshots exactly once, however many keys it touched.
//!
//! ## Channel statistics
//!
//! [`channel_stats`] is the read-only companion: one
//! [`ChannelStat`] per channel, in project order, carrying the
//! numbers a channel list shows next to each row (key count,
//! value range, frame span). It never snapshots, never writes
//! the status line and never mutates the project.

use crate::editor::EditorCore;
use crate::graph::{ChannelTarget, Interp, Key};

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

/// Display name for an [`Interp`] mode, in UI casing:
/// "Hermite", "Linear", "Step" — the variant names verbatim,
/// as the editor core's own interp status prints them.
fn interp_name(interp: Interp) -> &'static str {
    match interp {
        Interp::Hermite => "Hermite",
        Interp::Linear => "Linear",
        Interp::Step => "Step",
    }
}

/// Set the [`Interp`] mode of one key — the key at
/// `key_index` in the channel at `channel_index` (both
/// indices into `core.project.channels` and that channel's
/// `keys`). The mode governs the segment STARTING at that
/// key (see [`Key`]'s docs). On success exactly one snapshot
/// is taken, the key's mode is replaced — frame, value and
/// tangents untouched — and the status line names the key
/// (1-based), the channel's target label and the new mode;
/// returns `true`.
///
/// Returns `false` — no snapshot, project untouched — when
/// no channel sits at `channel_index` or the channel holds
/// no key at `key_index`; the status line reports both
/// 1-based numbers either way, mirroring [`delete_channel`]'s
/// refusal phrasing, so a UI binding never fails silently.
/// Setting the mode a key already has is still a success
/// (one snapshot): the caller asked for a state, and the
/// state now holds.
pub fn set_key_interp(
    core: &mut EditorCore,
    channel_index: usize,
    key_index: usize,
    interp: Interp,
) -> bool {
    let refusal = format!(
        "No key {} on graph channel {} to set.",
        key_index + 1,
        channel_index + 1
    );
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = refusal;
        return false;
    };
    if key_index >= channel.keys.len() {
        core.status = refusal;
        return false;
    }
    let label = target_label(channel.target);
    core.snapshot();
    core.project.channels[channel_index].keys[key_index].interp = interp;
    core.status = format!(
        "Key {} on {} interpolation set to {}.",
        key_index + 1,
        label,
        interp_name(interp)
    );
    true
}

/// The [`Interp`] mode of the key of the channel at
/// `channel_index` that sits at `frame` — read-only: no
/// snapshot, no status write, no mutation.
///
/// "Sits at" is the shell's key-at-frame convention, the
/// same one `Channel::remove_key` and `Channel::set_tangents`
/// apply (via the channel's nearest-key lookup) and the
/// shell's `keyed_here` check states outright: a key matches
/// when `|key.frame - frame| <= 0.5` — keys land on whole
/// frames, and half a frame either side still names that
/// key. When two keys fall inside the window the NEAREST
/// one wins, ties going to the earlier key, again like the
/// channel's own lookup. `None` when no channel sits at
/// `channel_index`, when the channel is empty, or when no
/// key lies within the window — a frame between keys two or
/// more frames apart therefore reads as no key, never as
/// the nearest key's mode.
pub fn key_interp_at(core: &EditorCore, channel_index: usize, frame: f32) -> Option<Interp> {
    let channel = core.project.channels.get(channel_index)?;
    let mut best: Option<usize> = None;
    let mut best_dist = f32::INFINITY;
    for (i, k) in channel.keys.iter().enumerate() {
        let d = (k.frame - frame).abs();
        if d <= 0.5 && d < best_dist {
            best_dist = d;
            best = Some(i);
        }
    }
    best.map(|i| channel.keys[i].interp)
}

/// Move EVERY key of the channel at `channel_index` by
/// `delta` frames (a negative delta moves keys earlier).
/// Values, tangents and [`Interp`] modes are untouched —
/// only frames move — and because every key moves by the
/// same offset their relative order is preserved, so no
/// re-sort is needed. On success exactly one snapshot is
/// taken and the status line reports the key count, the
/// channel's target label and the signed offset; returns
/// `true`.
///
/// Returns `false` — no snapshot, project untouched — when:
/// * no channel sits at `channel_index`;
/// * `delta` is zero or not finite (a shift by nothing, or
///   by a non-number, is not an edit);
/// * the channel holds no keys (nothing to move);
/// * ANY key would land before frame 0 — the whole shift
///   is refused, never clamped per key, so the curve can
///   never be silently distorted. A key landing exactly
///   ON frame 0 is fine.
pub fn shift_keys(core: &mut EditorCore, channel_index: usize, delta: f32) -> bool {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to shift.", channel_index + 1);
        return false;
    };
    let label = target_label(channel.target);
    if !delta.is_finite() || delta == 0.0 {
        core.status = format!(
            "Nothing to shift on {label} — the offset is zero or not a number."
        );
        return false;
    }
    if channel.keys.is_empty() {
        core.status = format!("No keys on {label} to shift.");
        return false;
    }
    if channel.keys.iter().any(|k| k.frame + delta < 0.0) {
        core.status = "Shifting would move a key before frame 1.".into();
        return false;
    }
    let count = channel.keys.len();
    core.snapshot();
    for k in &mut core.project.channels[channel_index].keys {
        k.frame += delta;
    }
    core.status = format!("Shifted {count} keys on {label} by {delta:+} frames.");
    true
}

/// Fill the keyless frames of `[from, to]` (integer frame
/// numbers, both ends inclusive) in the channel at
/// `channel_index` with copies of the channel's key at the
/// current frame ([`EditorCore::current`]) — the keyframe
/// panel's "fill the loop range" operation: the playhead
/// key's value, tangents and [`Interp`] mode, held across
/// the range as per-frame keys. A frame counts as already
/// keyed when a key sits within 0.5 frames of it — the
/// shell's key-at-frame convention (see [`key_interp_at`]),
/// which also resolves which key is "at" the playhead
/// (nearest within the window, ties to the earlier key).
/// Keyed frames are skipped, never duplicated or
/// overwritten. The inserted keys are merged in and the
/// channel re-sorted, so its keys stay in frame order. On
/// success exactly one snapshot is taken, however many
/// keys were inserted, and the number inserted is
/// returned.
///
/// Returns 0 — no snapshot, project untouched — when:
/// * no channel sits at `channel_index`;
/// * `from` is after `to` (a reversed range fills nothing);
/// * no key of the channel sits at the current frame
///   (there is nothing to fill FROM);
/// * every frame of the range already has a key.
pub fn fill_keys_in_range(
    core: &mut EditorCore,
    channel_index: usize,
    from: usize,
    to: usize,
) -> usize {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to fill.", channel_index + 1);
        return 0;
    };
    let label = target_label(channel.target);
    if from > to {
        core.status = format!(
            "Nothing to fill on {label} — frame {} is after frame {}.",
            from + 1,
            to + 1
        );
        return 0;
    }
    // The source key: the one sitting at the playhead,
    // resolved with the same nearest-within-half-a-frame
    // lookup `key_interp_at` performs.
    let at = core.current as f32;
    let mut source: Option<Key> = None;
    let mut best_dist = f32::INFINITY;
    for k in &channel.keys {
        let d = (k.frame - at).abs();
        if d <= 0.5 && d < best_dist {
            best_dist = d;
            source = Some(*k);
        }
    }
    let Some(source) = source else {
        core.status = "No key at the current frame to fill from.".into();
        return 0;
    };
    // The keyless frames of the range, decided against the
    // keys as they stand. An inserted key sits exactly on
    // its integer frame, a full frame from each neighbour,
    // so it can never fall within 0.5 of another frame of
    // the range and change this list.
    let missing: Vec<usize> = (from..=to)
        .filter(|&f| {
            !channel
                .keys
                .iter()
                .any(|k| (k.frame - f as f32).abs() <= 0.5)
        })
        .collect();
    if missing.is_empty() {
        core.status = "Every frame in the range already has a key.".into();
        return 0;
    }
    let count = missing.len();
    core.snapshot();
    let channel = &mut core.project.channels[channel_index];
    for f in missing {
        channel.keys.push(Key {
            frame: f as f32,
            ..source
        });
    }
    channel.sort_keys();
    core.status = format!("Filled {count} keys on {label}.");
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

// ---------------------------------------------------------------------
// MCP handlers (Wave 11, agent B8): the graph/easing parity
// tools that had no MCP surface. They live here, next to the
// ops they wrap, because `mcp_tools.rs`'s registry is the
// integrator's file: each handler below is a plain
// `pub fn tool_*` with the registry's handler signature
// (`fn(&mut EditorCore, &Value) -> Result<Value, String>`),
// ready to be named by a `Tool { .. }` entry in `tools()`.
//
// Why these exist — the gaps they close, all verified at
// source (see the parity audit in the Wave 11 report):
// * The registry's index-addressed tools
//   (duplicate/delete/set_key_interp) were blind: no tool
//   listed the channels, so an agent could not learn which
//   index names which channel. `tool_graph_list_channels`
//   and `tool_graph_get_channel` close that, built on
//   [`channel_stats`] plus the raw target.
// * The (figure, target)-addressed tools
//   (mirror/scale/copy/paste in the registry) resolve the
//   FIRST matching channel (`channel_index` in
//   `mcp_tools.rs`), so a duplicate `(figure, target)`
//   channel — which the graph panel CAN select, draw and
//   edit by row index (Phase J) — was unreachable. The
//   `*_channel` tools below address channels by the same
//   project-list index the panel's rows use
//   (`channel_rows` in `rune_sec/graph_panel.rs` enumerates
//   `core.project.channels` in order), so MCP channel N
//   IS the panel's row N.
// * `shift_keys`, `fill_keys_in_range` and
//   `delete_keys_in_range` had panel/palette surfaces but
//   no MCP tool at all; neither did single-key delete /
//   move / tangent edits (the panel's drags and the
//   keyframe tools' nudge/delete), `graph::reduce_keys`
//   (the panel's Reduce keys button), clearing one
//   channel's keys, keying every channel, keying the
//   selected joint's angle, or clearing all channels.
//
// Argument helpers are LOCAL copies (the `gk_` prefix):
// `mcp_tools.rs`'s helpers are private to that file, and
// the small parsers duplicated here (`gk_key_json`,
// `gk_interp_name`, the target spellings) follow the
// file's existing precedent — [`target_label`] duplicates
// the graph panel's `channel_label` string-for-string and
// the two must be kept in step. Every handler validates
// ALL of its arguments before touching the project, and
// every numeric argument must be finite: a JSON number
// like 1e400 arrives as f64 infinity (or null, which the
// null-filtered getter reads as missing), and neither may
// reach a channel.
// ---------------------------------------------------------------------

use serde_json::{json, Value};

/// Null-filtered argument lookup, mirroring `mcp_tools`'
/// private `get`: a JSON null reads as "not supplied".
fn gk_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// The `mcp_tools::as_usize` rule, copied: a non-negative
/// integer, as a JSON integer or an integral JSON float.
fn gk_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn gk_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match gk_get(args, key) {
        Some(v) => gk_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn gk_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    let v = gk_opt_f32(args, key)?.ok_or_else(|| format!("missing required argument '{key}'"))?;
    Ok(v)
}

fn gk_opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match gk_get(args, key) {
        Some(v) => {
            let f = v
                .as_f64()
                .ok_or_else(|| format!("argument '{key}' must be a number"))?
                as f32;
            if !f.is_finite() {
                return Err(format!("argument '{key}' must be a finite number"));
            }
            Ok(Some(f))
        }
        None => Ok(None),
    }
}

fn gk_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match gk_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// A required frame coordinate: finite and not negative,
/// the `graph_key` tool's own rule for its `frame` argument.
fn gk_req_frame(args: &Value, key: &str) -> Result<f32, String> {
    let f = gk_req_f32(args, key)?;
    if f < 0.0 {
        return Err(format!("argument '{key}' must not be negative"));
    }
    Ok(f)
}

/// Index of graph channel `idx`, or the clean error the
/// registry's own graph tools share (`check_graph_channel`
/// in `mcp_tools.rs`, wording copied so every graph tool
/// refuses a bad index in the same words).
fn gk_check_channel(core: &EditorCore, idx: usize) -> Result<usize, String> {
    if idx >= core.project.channels.len() {
        Err(format!("no graph channel at index {idx}"))
    } else {
        Ok(idx)
    }
}

/// Lower-case mode name for JSON output — the same strings
/// `mcp_tools`' private `interp_name` returns, so a key
/// read through any graph tool spells its mode identically.
fn gk_interp_name(interp: Interp) -> &'static str {
    match interp {
        Interp::Hermite => "hermite",
        Interp::Linear => "linear",
        Interp::Step => "step",
    }
}

/// One key as JSON, in the exact shape `graph_copy_keys`
/// returns and `graph_paste_keys` accepts
/// ({frame, value, in_tan, out_tan, interp}) — copied from
/// `mcp_tools`' private `key_json`; keep the two in step.
fn gk_key_json(key: &Key) -> Value {
    json!({
        "frame": key.frame,
        "value": key.value,
        "in_tan": key.in_tan,
        "out_tan": key.out_tan,
        "interp": gk_interp_name(key.interp),
    })
}

/// A channel target as JSON: `target` in the spelling
/// `graph_key` accepts ("root_x", "root_y", "angle",
/// "length" — with `node` alongside for the last two),
/// plus the UI `target_label` for display.
fn gk_target_json(target: ChannelTarget) -> Value {
    match target {
        ChannelTarget::RootX => json!({ "target": "root_x", "node": null, "target_label": "Root X" }),
        ChannelTarget::RootY => json!({ "target": "root_y", "node": null, "target_label": "Root Y" }),
        ChannelTarget::Angle(n) => json!({ "target": "angle", "node": n, "target_label": target_label(target) }),
        ChannelTarget::Length(n) => json!({ "target": "length", "node": n, "target_label": target_label(target) }),
    }
}

/// Parse a tangent preset name with the registry's
/// `graph_tangent_preset` spellings and error wording.
fn gk_parse_preset(name: &str) -> Result<crate::graph::TangentPreset, String> {
    use crate::graph::TangentPreset;
    match name {
        "smooth" => Ok(TangentPreset::Smooth),
        "ease-in" => Ok(TangentPreset::EaseIn),
        "ease-out" => Ok(TangentPreset::EaseOut),
        "ease-inout" => Ok(TangentPreset::EaseInOut),
        "linear" => Ok(TangentPreset::Linear),
        other => Err(format!(
            "unknown tangent preset '{other}' (expected \"smooth\", \"ease-in\", \"ease-out\", \"ease-inout\" or \"linear\")"
        )),
    }
}

/// List every graph channel: its project-list index (the
/// index the panel's channel rows show and the
/// index-addressed tools take), figure, target and the
/// [`channel_stats`] numbers. Read-only.
pub fn tool_graph_list_channels(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let stats = channel_stats(core);
    let channels: Vec<Value> = core
        .project
        .channels
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let st = &stats[i];
            json!({
                "channel": i,
                "figure": ch.figure,
                "figure_name": st.figure_name,
                "target": gk_target_json(ch.target),
                "keys": st.keys,
                "min": st.min,
                "max": st.max,
                "first_frame": st.first_frame,
                "last_frame": st.last_frame,
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "channels": channels,
        "count": core.project.channels.len(),
    }))
}

/// Read one channel in full by its project-list index:
/// every key (the shape `graph_paste_keys` accepts), plus
/// the channel's value at `frame` when one is supplied —
/// the graph panel's value readout, over
/// `Channel::value_at`. Read-only.
pub fn tool_graph_get_channel(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let frame = gk_opt_f32(args, "frame")?;
    let ch = &core.project.channels[idx];
    let mut out = json!({
        "ok": true,
        "channel": idx,
        "figure": ch.figure,
        "target": gk_target_json(ch.target),
        "keys": ch.keys.iter().map(gk_key_json).collect::<Vec<_>>(),
        "key_count": ch.keys.len(),
    });
    if let Some(f) = frame {
        out["frame"] = json!(f);
        out["value_at"] = json!(ch.value_at(f));
    }
    Ok(out)
}

/// Remove the key nearest `frame` (within 0.5 frames — the
/// engine's key window) from the channel at
/// `channel_index`, as one undo step; `false` — no
/// snapshot — when the channel or such a key does not
/// exist. This is the library form of the keyframe tools'
/// "Delete key here" composition (`Channel::remove_key`
/// under one snapshot), generalised to any channel.
pub fn remove_key_at(core: &mut EditorCore, channel_index: usize, frame: f32) -> bool {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to delete a key from.", channel_index + 1);
        return false;
    };
    if !channel.keys.iter().any(|k| (k.frame - frame).abs() <= 0.5) {
        core.status = format!("No key at frame {} to delete.", frame_label(frame + 1.0));
        return false;
    }
    core.snapshot();
    let removed = core.project.channels[channel_index].remove_key(frame);
    if removed {
        core.status = format!("Deleted graph key at frame {}.", frame_label(frame + 1.0));
    }
    removed
}

pub fn tool_graph_delete_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let frame = gk_req_frame(args, "frame")?;
    if !remove_key_at(core, idx, frame) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

/// Move the key nearest `from_frame` (within 0.5 frames)
/// of the channel at `channel_index` to `to_frame`,
/// keeping its tangents; its value travels unless
/// `to_value` overrides it. One undo step; `false` — no
/// snapshot — when the channel or such a key does not
/// exist. The move itself is `Channel::move_key`, so a
/// key already sitting at `to_frame` (within 0.01) is
/// replaced — the graph canvas drag's semantics, NOT the
/// keyframe nudge's refusal (that panel guards its own
/// collision rule before ever calling `move_key`).
pub fn move_key_at(
    core: &mut EditorCore,
    channel_index: usize,
    from_frame: f32,
    to_frame: f32,
    to_value: Option<f32>,
) -> bool {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to move a key on.", channel_index + 1);
        return false;
    };
    let Some(key) = channel
        .keys
        .iter()
        .filter(|k| (k.frame - from_frame).abs() <= 0.5)
        .min_by(|a, b| {
            (a.frame - from_frame)
                .abs()
                .partial_cmp(&(b.frame - from_frame).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        core.status = format!("No key at frame {} to move.", frame_label(from_frame + 1.0));
        return false;
    };
    let value = to_value.unwrap_or(key.value);
    core.snapshot();
    let moved = core.project.channels[channel_index].move_key(from_frame, to_frame, value);
    if moved {
        core.status = format!(
            "Moved graph key from frame {} to frame {}.",
            frame_label(from_frame + 1.0),
            frame_label(to_frame + 1.0)
        );
    }
    moved
}

pub fn tool_graph_move_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let from_frame = gk_req_frame(args, "from_frame")?;
    let to_frame = gk_req_frame(args, "to_frame")?;
    let value = gk_opt_f32(args, "value")?;
    if !move_key_at(core, idx, from_frame, to_frame, value) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

/// Set both tangents of the key nearest `frame` (within
/// 0.5 frames) on the channel at `channel_index`, through
/// `Channel::set_tangents` — the value form of the graph
/// panel's tangent-handle drag. One undo step; `false` —
/// no snapshot — when the channel or such a key does not
/// exist.
pub fn set_key_tangents_at(
    core: &mut EditorCore,
    channel_index: usize,
    frame: f32,
    in_tan: f32,
    out_tan: f32,
) -> bool {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to set tangents on.", channel_index + 1);
        return false;
    };
    if !channel.keys.iter().any(|k| (k.frame - frame).abs() <= 0.5) {
        core.status = format!("No key at frame {} to set tangents on.", frame_label(frame + 1.0));
        return false;
    }
    let label = target_label(channel.target);
    core.snapshot();
    let set = core.project.channels[channel_index].set_tangents(frame, in_tan, out_tan);
    if set {
        core.status = format!(
            "Set tangents on {label} key at frame {}: in {in_tan}, out {out_tan}.",
            frame_label(frame + 1.0)
        );
    }
    set
}

pub fn tool_graph_set_tangents(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let frame = gk_req_frame(args, "frame")?;
    let in_tan = gk_req_f32(args, "in_tan")?;
    let out_tan = gk_req_f32(args, "out_tan")?;
    if !set_key_tangents_at(core, idx, frame, in_tan, out_tan) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
    }))
}

pub fn tool_graph_shift_keys(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let delta = gk_req_f32(args, "delta")?;
    if !shift_keys(core, idx, delta) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

pub fn tool_graph_fill_keys(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let from = gk_req_usize(args, "from")?;
    let to = gk_req_usize(args, "to")?;
    let filled = fill_keys_in_range(core, idx, from, to);
    if filled == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "filled": filled,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

/// Clear every key of the channel at `channel_index`
/// whose frame lies inside `[from, to]`, both ends
/// inclusive — the index twin of [`delete_keys_in_range`],
/// which resolves its channel by (figure, target) FIRST
/// match and so can never name a duplicate channel. Same
/// discipline: one snapshot on success, 0 and a status
/// line when nothing is in the range.
pub fn delete_keys_in_range_at(
    core: &mut EditorCore,
    channel_index: usize,
    from: f32,
    to: f32,
) -> usize {
    let range = format!("{}–{}", frame_label(from + 1.0), frame_label(to + 1.0));
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to clear keys from.", channel_index + 1);
        return 0;
    };
    let count = channel
        .keys
        .iter()
        .filter(|k| (from..=to).contains(&k.frame))
        .count();
    if count == 0 {
        core.status = format!("No keys to clear in frames {range}.");
        return 0;
    }
    core.snapshot();
    core.project.channels[channel_index]
        .keys
        .retain(|k| !(from..=to).contains(&k.frame));
    core.status = format!("Cleared {count} key(s) in frames {range}.");
    count
}

pub fn tool_graph_delete_keys_in_range(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let from = gk_req_frame(args, "from")?;
    let to = gk_req_frame(args, "to")?;
    if from > to {
        return Err("argument 'from' must not be after 'to'".into());
    }
    let cleared = delete_keys_in_range_at(core, idx, from, to);
    if cleared == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "cleared": cleared,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

/// Remove EVERY key of the channel at `channel_index`,
/// leaving the channel itself in place — the index form
/// of the keyframe tools' "Clear channel keys"
/// composition. One undo step; returns 0 — no snapshot —
/// when the channel is missing or already empty.
pub fn clear_channel_keys_at(core: &mut EditorCore, channel_index: usize) -> usize {
    let Some(channel) = core.project.channels.get(channel_index) else {
        core.status = format!("No graph channel {} to clear.", channel_index + 1);
        return 0;
    };
    if channel.keys.is_empty() {
        core.status = format!("No graph keys on channel {} to clear.", channel_index + 1);
        return 0;
    }
    let label = target_label(channel.target);
    let n = channel.keys.len();
    core.snapshot();
    core.project.channels[channel_index].keys.clear();
    core.status = format!("Cleared {n} graph key(s) from channel {} ({label}).", channel_index + 1);
    n
}

pub fn tool_graph_clear_channel_keys(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    if clear_channel_keys_at(core, idx) == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "channel": idx,
        "status": core.status,
        "channel_keys": 0,
    }))
}

pub fn tool_graph_reduce_keys(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    // The graph panel's Reduce keys button tolerance
    // (`REDUCE_TOLERANCE`, rune_sec/graph_panel.rs) is the
    // default, so a bare call matches the panel exactly.
    let tolerance = gk_opt_f32(args, "tolerance")?.unwrap_or(0.5);
    if tolerance < 0.0 {
        return Err("argument 'tolerance' must not be negative".into());
    }
    core.snapshot();
    let removed = crate::graph::reduce_keys(&mut core.project.channels[idx], tolerance);
    core.status = format!("Reduced graph channel: removed {removed} key(s).");
    Ok(json!({
        "ok": true,
        "channel": idx,
        "removed": removed,
        "status": core.status,
        "channel_keys": core.project.channels[idx].keys.len(),
    }))
}

pub fn tool_graph_mirror_channel(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let centre = match gk_opt_f32(args, "centre")? {
        Some(c) => Some(c),
        None => gk_opt_f32(args, "center")?,
    }
    .unwrap_or_else(|| (core.project.frames.len().saturating_sub(1)) as f32 / 2.0);
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    crate::graph::mirror_keys_time(&mut ch.keys, centre);
    ch.sort_keys();
    let keys = ch.keys.len();
    core.status = format!("Mirrored graph channel time around frame {centre}.");
    Ok(json!({
        "ok": true,
        "channel": idx,
        "centre": centre,
        "keys": keys,
        "status": core.status,
    }))
}

pub fn tool_graph_scale_channel(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let factor = gk_req_f32(args, "factor")?;
    let pivot = gk_opt_f32(args, "pivot")?.unwrap_or(0.0);
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    crate::graph::scale_key_values(&mut ch.keys, factor, pivot);
    let keys = ch.keys.len();
    core.status = format!("Scaled graph channel values by {factor} around {pivot}.");
    Ok(json!({
        "ok": true,
        "channel": idx,
        "factor": factor,
        "pivot": pivot,
        "keys": keys,
        "status": core.status,
    }))
}

pub fn tool_graph_tangent_preset_channel(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let idx = gk_check_channel(core, gk_req_usize(args, "channel")?)?;
    let preset = gk_parse_preset(gk_req_str(args, "preset")?)?;
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    ch.sort_keys();
    let angular = matches!(ch.target, ChannelTarget::Angle(_));
    crate::graph::apply_tangent_preset(&mut ch.keys, preset, angular);
    core.status = format!(
        "Applied {preset:?} preset to graph channel {} ({}).",
        idx + 1,
        target_label(core.project.channels[idx].target)
    );
    Ok(json!({
        "ok": true,
        "channel": idx,
        "preset": format!("{preset:?}"),
        "status": core.status,
    }))
}

/// Key EVERY channel in the project at `frame`, each
/// holding its own evaluated value there — the library
/// form of the keyframe tools' "Key all here" composition
/// (`key_all_here_core`, which lives in the shell crate
/// and cannot be called from here): read
/// `Channel::value_at`, insert with `Channel::add_key`,
/// ONE snapshot for the whole project. Returns how many
/// channels were keyed; a project with no channels is a
/// status-only no-op (0, no snapshot).
pub fn key_all_channels_at(core: &mut EditorCore, frame: f32) -> usize {
    let n = core.project.channels.len();
    if n == 0 {
        core.status = "No graph channels to key.".into();
        return 0;
    }
    core.snapshot();
    for ch in &mut core.project.channels {
        let value = ch.value_at(frame);
        ch.add_key(frame, value);
    }
    core.status = format!("Keyed {n} channels at frame {}.", frame_label(frame + 1.0));
    n
}

pub fn tool_graph_key_all_channels(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = match gk_opt_f32(args, "frame")? {
        Some(f) => {
            if f < 0.0 {
                return Err("argument 'frame' must not be negative".into());
            }
            f
        }
        None => core.current as f32,
    };
    let keyed = key_all_channels_at(core, frame);
    if keyed == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "keyed": keyed,
        "frame": frame,
        "status": core.status,
    }))
}

pub fn tool_graph_key_angle(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // The core op owns every refusal (no joint selected)
    // and reports it on the status line; like the
    // registry's other direct core-op tools
    // (smooth_graph_tangents, mirror_pose) the handler
    // returns the status rather than re-deriving the
    // refusal.
    core.key_graph_angle();
    Ok(json!({
        "ok": true,
        "status": core.status,
        "channels": core.project.channels.len(),
    }))
}

pub fn tool_graph_clear_channels(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    // TRUTH NOTE: `EditorCore::clear_graph` — the graph
    // panel's "Clear all channels" op — takes NO snapshot,
    // so this is not an undo step, exactly as in the
    // panel. The description in the registry entry must
    // say so.
    let cleared = core.project.channels.len();
    core.clear_graph();
    Ok(json!({
        "ok": true,
        "cleared": cleared,
        "status": core.status,
    }))
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

    #[test]
    fn set_key_interp_sets_each_variant_and_undo_restores() {
        // Channel 0 (RootX) starts with Linear / Hermite /
        // Step on keys 1 / 2 / 3 (frames 10 / 14 / 20).
        let mut core = core_with_channels();

        // Key 1: Linear -> Step. Exactly one snapshot, and the
        // rest of the key is untouched.
        let undo_before = core.undo_depth();
        assert!(set_key_interp(&mut core, 0, 0, Interp::Step));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        let k = core.project.channels[0].keys[0];
        assert_eq!(k.interp, Interp::Step);
        assert!(approx(k.frame, 10.0) && approx(k.value, 100.0), "frame/value untouched");
        assert!(
            approx(k.in_tan, 1.5) && approx(k.out_tan, -2.0),
            "tangents untouched"
        );
        assert!(
            core.status.contains("Key 1 on Root X interpolation set to Step."),
            "status names the key, the target label and the mode: {}",
            core.status
        );
        // Undo restores the mode the key had.
        core.undo();
        assert_eq!(
            core.project.channels[0].keys[0].interp,
            Interp::Linear,
            "undo restores the previous mode"
        );

        // Key 2: Hermite -> Linear.
        assert!(set_key_interp(&mut core, 0, 1, Interp::Linear));
        assert_eq!(core.project.channels[0].keys[1].interp, Interp::Linear);
        assert!(
            core.status.contains("Key 2 on Root X interpolation set to Linear."),
            "status: {}",
            core.status
        );
        core.undo();
        assert_eq!(core.project.channels[0].keys[1].interp, Interp::Hermite);

        // Key 3: Step -> Hermite.
        assert!(set_key_interp(&mut core, 0, 2, Interp::Hermite));
        assert_eq!(core.project.channels[0].keys[2].interp, Interp::Hermite);
        assert!(
            core.status.contains("Key 3 on Root X interpolation set to Hermite."),
            "status: {}",
            core.status
        );
        core.undo();
        assert_eq!(core.project.channels[0].keys[2].interp, Interp::Step);
    }

    #[test]
    fn set_key_interp_refuses_bad_indices_without_snapshot() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();
        let channels_before = core.project.channels.clone();

        // No such channel.
        assert!(!set_key_interp(&mut core, 99, 0, Interp::Step), "no channel 100");
        assert!(
            core.status.contains("No key 1 on graph channel 100 to set."),
            "status: {}",
            core.status
        );
        // Channel exists, no such key.
        assert!(!set_key_interp(&mut core, 0, 99, Interp::Linear), "no key 100");
        assert!(
            core.status.contains("No key 100 on graph channel 1 to set."),
            "status: {}",
            core.status
        );
        // The empty channel (index 1) has no key 1 either.
        assert!(!set_key_interp(&mut core, 1, 0, Interp::Hermite), "empty channel");
        assert!(
            core.status.contains("No key 1 on graph channel 2 to set."),
            "status: {}",
            core.status
        );

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.channels, channels_before, "project untouched");
    }

    #[test]
    fn key_interp_at_finds_the_key_at_its_frame_and_misses_between_keys() {
        let mut core = core_with_channels();
        let edits_before = core.edits_since_autosave();
        let status_before = core.status.clone();

        // Channel 0's keys: frame 10 Linear, 14 Hermite, 20 Step.
        assert_eq!(key_interp_at(&core, 0, 10.0), Some(Interp::Linear));
        assert_eq!(key_interp_at(&core, 0, 14.0), Some(Interp::Hermite));
        assert_eq!(key_interp_at(&core, 0, 20.0), Some(Interp::Step));
        // Inside the half-frame window either side, the key
        // still names itself.
        assert_eq!(key_interp_at(&core, 0, 10.4), Some(Interp::Linear));
        assert_eq!(key_interp_at(&core, 0, 13.6), Some(Interp::Hermite));
        assert_eq!(key_interp_at(&core, 0, 20.5), Some(Interp::Step));

        // Between keys — closer to no key than half a frame —
        // there is no key to read, never the nearest one's mode.
        assert_eq!(key_interp_at(&core, 0, 12.0), None, "midway between 10 and 14");
        assert_eq!(key_interp_at(&core, 0, 17.0), None, "midway between 14 and 20");
        assert_eq!(key_interp_at(&core, 0, 0.0), None, "before every key");
        assert_eq!(key_interp_at(&core, 0, 30.0), None, "after every key");

        // No channel, and the empty channel, both read as None.
        assert_eq!(key_interp_at(&core, 99, 10.0), None);
        assert_eq!(key_interp_at(&core, 1, 10.0), None);

        // Pure read: no snapshot, no status write.
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "key_interp_at takes no snapshot"
        );
        assert_eq!(core.status, status_before, "key_interp_at writes no status");

        // And it reads the live project: a mode set through
        // the setter is the mode found at that key's frame.
        assert!(set_key_interp(&mut core, 0, 0, Interp::Step));
        assert_eq!(key_interp_at(&core, 0, 10.0), Some(Interp::Step));
    }

    #[test]
    fn shift_keys_moves_every_key_and_reports_the_offset() {
        // Channel 0 (RootX) holds keys at frames 10/14/20.
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();

        assert!(shift_keys(&mut core, 0, 4.0));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        let ch = &core.project.channels[0];
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert!(
            approx(frames[0], 14.0) && approx(frames[1], 18.0) && approx(frames[2], 24.0),
            "every key moved by the same +4: {frames:?}"
        );
        // Only frames moved: values, tangents and modes are
        // the keys' own, verbatim.
        assert!(approx(ch.keys[0].value, 100.0));
        assert!(
            approx(ch.keys[0].in_tan, 1.5) && approx(ch.keys[0].out_tan, -2.0),
            "tangents untouched"
        );
        assert_eq!(ch.keys[0].interp, Interp::Linear, "interp untouched");
        assert_eq!(ch.keys[2].interp, Interp::Step);
        assert!(
            core.status.contains("Shifted 3 keys on Root X by +4 frames."),
            "status reports count, label and signed offset: {}",
            core.status
        );

        // One undo step restores the original frames.
        core.undo();
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert!(
            approx(frames[0], 10.0) && approx(frames[1], 14.0) && approx(frames[2], 20.0),
            "undo restores the unshifted frames: {frames:?}"
        );
    }

    #[test]
    fn shift_keys_moves_backwards_and_fractionally() {
        let mut core = core_with_channels();

        assert!(shift_keys(&mut core, 0, -5.0), "a negative delta moves keys earlier");
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert!(
            approx(frames[0], 5.0) && approx(frames[1], 9.0) && approx(frames[2], 15.0),
            "frames after a -5 shift: {frames:?}"
        );
        assert!(
            core.status.contains("by -5 frames."),
            "the signed offset prints its minus: {}",
            core.status
        );

        assert!(shift_keys(&mut core, 0, 0.5), "a fractional delta is a real shift");
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert!(
            approx(frames[0], 5.5) && approx(frames[1], 9.5) && approx(frames[2], 15.5),
            "frames after a +0.5 shift: {frames:?}"
        );
        assert!(
            core.status.contains("by +0.5 frames."),
            "the fractional offset prints with its plus: {}",
            core.status
        );
    }

    #[test]
    fn shift_keys_may_land_a_key_exactly_on_frame_zero() {
        let mut core = core_with_channels();
        // The earliest key sits at frame 10: a -10 shift
        // lands it exactly on frame 0, which is allowed —
        // only landing BEFORE frame 0 refuses.
        assert!(shift_keys(&mut core, 0, -10.0));
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert!(
            approx(frames[0], 0.0) && approx(frames[1], 4.0) && approx(frames[2], 10.0),
            "frames after a -10 shift: {frames:?}"
        );
    }

    #[test]
    fn shift_keys_refuses_without_snapshot() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();
        let channels_before = core.project.channels.clone();

        // No such channel.
        assert!(!shift_keys(&mut core, 99, 4.0), "no channel 100");
        assert!(
            core.status.contains("No graph channel 100 to shift."),
            "status: {}",
            core.status
        );
        // A zero offset moves nothing.
        assert!(!shift_keys(&mut core, 0, 0.0), "zero delta");
        assert!(
            core.status.contains("Nothing to shift on Root X"),
            "status: {}",
            core.status
        );
        // Non-finite offsets are not numbers of frames.
        assert!(!shift_keys(&mut core, 0, f32::NAN), "NaN delta");
        assert!(!shift_keys(&mut core, 0, f32::INFINITY), "infinite delta");
        assert!(!shift_keys(&mut core, 0, f32::NEG_INFINITY), "negative infinite delta");
        // The empty channel has nothing to move.
        assert!(!shift_keys(&mut core, 1, 3.0), "empty channel");
        assert!(
            core.status.contains("No keys on Root Y to shift."),
            "status: {}",
            core.status
        );
        // One key past frame zero refuses the WHOLE shift:
        // -10.5 would put the frame-10 key at -0.5, so not
        // even the keys that could move do.
        assert!(!shift_keys(&mut core, 0, -10.5), "a key would land before frame 0");
        assert_eq!(
            core.status, "Shifting would move a key before frame 1.",
            "the refusal names the frame-1 boundary"
        );

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.channels, channels_before, "project untouched");
    }

    #[test]
    fn fill_keys_in_range_fills_keyless_frames_from_the_current_key() {
        // Channel 0 (RootX) holds keys at frames 10/14/20;
        // the playhead sits on the frame-10 key (value 100,
        // tangents 1.5/-2.0, Linear).
        let mut core = core_with_channels();
        core.current = 10;
        let undo_before = core.undo_depth();

        // Frames 10 and 14 are keyed; 11, 12, 13 are not.
        assert_eq!(fill_keys_in_range(&mut core, 0, 10, 14), 3);
        assert_eq!(
            core.undo_depth(),
            undo_before + 1,
            "three inserts are still exactly one snapshot"
        );
        assert!(
            core.status.contains("Filled 3 keys on Root X."),
            "status reports the count and the label: {}",
            core.status
        );

        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 6, "three originals plus three fills");
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert!(
            approx(frames[0], 10.0)
                && approx(frames[1], 11.0)
                && approx(frames[2], 12.0)
                && approx(frames[3], 13.0)
                && approx(frames[4], 14.0)
                && approx(frames[5], 20.0),
            "keys stay sorted by frame: {frames:?}"
        );
        // Each fill is the playhead key verbatim — value,
        // tangents and interp — with only its frame changed.
        for filled in &ch.keys[1..4] {
            assert!(approx(filled.value, 100.0), "value copied");
            assert!(
                approx(filled.in_tan, 1.5) && approx(filled.out_tan, -2.0),
                "tangents copied"
            );
            assert_eq!(filled.interp, Interp::Linear, "interp copied");
        }
        // The pre-existing keys are untouched.
        assert!(approx(ch.keys[4].value, 140.0), "frame-14 key unchanged");
        assert_eq!(ch.keys[4].interp, Interp::Hermite);
        assert!(approx(ch.keys[5].value, 90.0), "frame-20 key unchanged");

        // One undo removes all three fills at once.
        core.undo();
        assert_eq!(core.project.channels[0].keys.len(), 3, "undo reverts the fill");
    }

    #[test]
    fn fill_keys_in_range_uses_the_half_frame_key_convention() {
        // Keys at 10.4 and 12.4: the playhead on frame 10
        // still names the 10.4 key (0.4 away, inside the
        // half-frame window), frame 10 counts as keyed by
        // it, and frame 12 counts as keyed by the 12.4 key
        // — while frames 11 and 13 sit more than half a
        // frame from every key and get filled.
        let mut core = core_with_channels();
        core.project.channels[0].keys = vec![
            key(10.4, 55.0, 3.0, 4.0, Interp::Step),
            key(12.4, 66.0, 0.0, 0.0, Interp::Hermite),
        ];
        core.current = 10;

        assert_eq!(fill_keys_in_range(&mut core, 0, 10, 13), 2);
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 4, "two originals plus two fills");
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert!(
            approx(frames[0], 10.4)
                && approx(frames[1], 11.0)
                && approx(frames[2], 12.4)
                && approx(frames[3], 13.0),
            "fills merged in sorted, fractional keys undisturbed: {frames:?}"
        );
        // The fills copy the playhead key (the 10.4 one),
        // not the nearer-in-list 12.4 key.
        for filled in [&ch.keys[1], &ch.keys[3]] {
            assert!(approx(filled.value, 55.0), "value copied from the 10.4 key");
            assert!(
                approx(filled.in_tan, 3.0) && approx(filled.out_tan, 4.0),
                "tangents copied"
            );
            assert_eq!(filled.interp, Interp::Step, "interp copied");
        }
    }

    #[test]
    fn fill_keys_in_range_inserts_before_existing_keys_sorted() {
        // Playhead on the frame-20 key (value 90, Step);
        // the range lies entirely BEFORE every existing
        // key, so the fills must merge in at the front in
        // frame order, not append at the end.
        let mut core = core_with_channels();
        core.current = 20;

        assert_eq!(fill_keys_in_range(&mut core, 0, 0, 2), 3);
        let ch = &core.project.channels[0];
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames.len(), 6);
        assert!(
            approx(frames[0], 0.0)
                && approx(frames[1], 1.0)
                && approx(frames[2], 2.0)
                && approx(frames[3], 10.0)
                && approx(frames[4], 14.0)
                && approx(frames[5], 20.0),
            "fills sorted ahead of the originals: {frames:?}"
        );
        for filled in &ch.keys[0..3] {
            assert!(approx(filled.value, 90.0), "copies of the frame-20 key");
            assert_eq!(filled.interp, Interp::Step);
        }
    }

    #[test]
    fn fill_keys_in_range_refuses_without_snapshot() {
        let mut core = core_with_channels();
        let undo_before = core.undo_depth();
        let channels_before = core.project.channels.clone();

        // No such channel.
        core.current = 10;
        assert_eq!(fill_keys_in_range(&mut core, 99, 10, 14), 0, "no channel 100");
        assert!(
            core.status.contains("No graph channel 100 to fill."),
            "status: {}",
            core.status
        );
        // A reversed range fills nothing.
        assert_eq!(fill_keys_in_range(&mut core, 0, 14, 10), 0, "from after to");
        assert!(
            core.status.contains("Nothing to fill on Root X"),
            "status: {}",
            core.status
        );
        // No key at the playhead: frame 12 sits midway
        // between the keys at 10 and 14, more than half a
        // frame from both — there is nothing to fill FROM.
        core.current = 12;
        assert_eq!(fill_keys_in_range(&mut core, 0, 10, 14), 0);
        assert_eq!(
            core.status, "No key at the current frame to fill from.",
            "the refusal says there is no source key"
        );
        // The empty channel has no source key either.
        core.current = 10;
        assert_eq!(fill_keys_in_range(&mut core, 1, 10, 14), 0);
        assert_eq!(core.status, "No key at the current frame to fill from.");

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.channels, channels_before, "project untouched");
    }

    #[test]
    fn fill_keys_in_range_reports_a_fully_keyed_range() {
        let mut core = core_with_channels();
        core.current = 10;
        let undo_before = core.undo_depth();

        // Frame 10 already has the playhead key itself, so
        // a range of just that frame has nothing to fill.
        assert_eq!(fill_keys_in_range(&mut core, 0, 10, 10), 0);
        assert_eq!(
            core.status, "Every frame in the range already has a key.",
            "the status says the range was already full"
        );
        assert_eq!(core.undo_depth(), undo_before, "no snapshot for a no-op fill");
        assert_eq!(core.project.channels[0].keys.len(), 3, "no key duplicated");
    }
}

#[cfg(test)]
mod mcp_graph_tool_tests {
    use super::*;
    use crate::graph::Channel;
    use serde_json::{json, Value};

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Two DUPLICATE (figure 0, RootX) channels told apart
    /// by their keys — twin 0 holds values 100/140, twin 1
    /// holds a single key 42 — plus an empty RootY channel.
    /// Index-addressed MCP tools must be able to name the
    /// second twin, exactly as the graph panel's rows can.
    fn twin_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            Channel {
                figure: 0,
                target: ChannelTarget::RootX,
                keys: vec![Key::flat(0.0, 100.0), Key::flat(4.0, 140.0)],
            },
            Channel {
                figure: 0,
                target: ChannelTarget::RootX,
                keys: vec![Key::flat(2.0, 42.0)],
            },
            Channel {
                figure: 0,
                target: ChannelTarget::RootY,
                keys: Vec::new(),
            },
        ];
        core
    }

    #[test]
    fn list_channels_reports_every_channel_in_project_order() {
        let mut core = twin_core();
        core.project.figures[0].name = "Hero".to_string();
        let out = tool_graph_list_channels(&mut core, &json!({})).unwrap();
        assert_eq!(out["count"], 3);
        let rows = out["channels"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        // Row i IS project channel i — the panel's rows
        // (channel_rows) enumerate the same list in order.
        assert_eq!(rows[0]["channel"], 0);
        assert_eq!(rows[0]["figure"], 0);
        assert_eq!(rows[0]["figure_name"], "Hero");
        assert_eq!(rows[0]["target"]["target"], "root_x");
        assert_eq!(rows[0]["keys"], 2);
        assert!(approx(rows[0]["min"].as_f64().unwrap() as f32, 100.0));
        assert!(approx(rows[0]["max"].as_f64().unwrap() as f32, 140.0));
        assert!(approx(rows[0]["first_frame"].as_f64().unwrap() as f32, 0.0));
        assert!(approx(rows[0]["last_frame"].as_f64().unwrap() as f32, 4.0));
        // The second twin is listed as itself, not merged.
        assert_eq!(rows[1]["channel"], 1);
        assert_eq!(rows[1]["keys"], 1);
        assert!(approx(rows[1]["min"].as_f64().unwrap() as f32, 42.0));
        // The empty channel reports the stats zeroes.
        assert_eq!(rows[2]["target"]["target"], "root_y");
        assert_eq!(rows[2]["keys"], 0);
    }

    #[test]
    fn get_channel_reads_the_named_twin_and_samples_it() {
        let mut core = twin_core();
        let out = tool_graph_get_channel(&mut core, &json!({ "channel": 1, "frame": 2.0 })).unwrap();
        assert_eq!(out["channel"], 1);
        assert_eq!(out["key_count"], 1);
        assert_eq!(out["keys"][0]["value"], 42.0);
        assert_eq!(out["keys"][0]["interp"], "hermite");
        assert!(approx(out["value_at"].as_f64().unwrap() as f32, 42.0));
        // Without a frame, no sample is reported.
        let out = tool_graph_get_channel(&mut core, &json!({ "channel": 0 })).unwrap();
        assert!(out.get("value_at").is_none());
        assert_eq!(out["keys"][1]["value"], 140.0);
    }

    #[test]
    fn index_tools_edit_the_second_twin_not_the_first() {
        let mut core = twin_core();
        // Mirror twin 1 around the timeline middle (frame
        // 2 of the 5-frame demo): its single key at frame 2
        // stays at frame 2, but the call must touch ONLY
        // twin 1's keys array identity — prove via a scale
        // that changes values visibly instead.
        let out = tool_graph_scale_channel(
            &mut core,
            &json!({ "channel": 1, "factor": 3.0 }),
        )
        .unwrap();
        assert_eq!(out["channel"], 1);
        assert!(approx(core.project.channels[1].keys[0].value, 126.0), "twin 1 scaled");
        assert!(approx(core.project.channels[0].keys[0].value, 100.0), "twin 0 untouched");
        // Delete twin 1's only key by index.
        tool_graph_delete_key(&mut core, &json!({ "channel": 1, "frame": 2.0 })).unwrap();
        assert!(core.project.channels[1].keys.is_empty());
        assert_eq!(core.project.channels[0].keys.len(), 2, "twin 0 keeps its keys");
    }

    #[test]
    fn delete_move_and_tangent_tools_follow_snapshot_discipline() {
        let mut core = twin_core();
        let depth = core.undo_depth();
        // Tangents on twin 0's frame-0 key.
        tool_graph_set_tangents(
            &mut core,
            &json!({ "channel": 0, "frame": 0.0, "in_tan": 1.5, "out_tan": -2.0 }),
        )
        .unwrap();
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot");
        let k = core.project.channels[0].keys[0];
        assert!(approx(k.in_tan, 1.5) && approx(k.out_tan, -2.0));
        // Move the frame-4 key to frame 6, value travelling.
        tool_graph_move_key(
            &mut core,
            &json!({ "channel": 0, "from_frame": 4.0, "to_frame": 6.0 }),
        )
        .unwrap();
        let frames: Vec<f32> = core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 6.0]);
        assert!(approx(core.project.channels[0].keys[1].value, 140.0));
        // Refusals (no key near the frame) take no snapshot.
        let depth = core.undo_depth();
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": 0, "frame": 3.0 })).is_err());
        assert!(tool_graph_move_key(
            &mut core,
            &json!({ "channel": 0, "from_frame": 3.0, "to_frame": 8.0 })
        )
        .is_err());
        assert!(tool_graph_set_tangents(
            &mut core,
            &json!({ "channel": 0, "frame": 3.0, "in_tan": 0.0, "out_tan": 0.0 })
        )
        .is_err());
        assert_eq!(core.undo_depth(), depth, "no refusal snapshots");
    }

    #[test]
    fn shift_fill_and_range_clear_tools_delegate_to_the_ops() {
        let mut core = twin_core();
        // Fill needs a key AT the playhead: park it on
        // twin 1's key (current = 2) — fill twin 1 across
        // 0..=4: frames 0,1,3,4 are keyless.
        core.goto(2);
        let out = tool_graph_fill_keys(&mut core, &json!({ "channel": 1, "from": 0, "to": 4 })).unwrap();
        assert_eq!(out["filled"], 4);
        assert_eq!(core.project.channels[1].keys.len(), 5);
        // Shift everything one frame later.
        tool_graph_shift_keys(&mut core, &json!({ "channel": 1, "delta": 1.0 })).unwrap();
        let frames: Vec<f32> = core.project.channels[1].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        // A shift that would cross frame 0 is refused whole.
        assert!(tool_graph_shift_keys(&mut core, &json!({ "channel": 1, "delta": -2.0 })).is_err());
        // Clear the middle of the range by index.
        let out = tool_graph_delete_keys_in_range(
            &mut core,
            &json!({ "channel": 1, "from": 2.0, "to": 4.0 }),
        )
        .unwrap();
        assert_eq!(out["cleared"], 3);
        let frames: Vec<f32> = core.project.channels[1].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![1.0, 5.0]);
        // Reversed range is an argument error, not a no-op.
        assert!(tool_graph_delete_keys_in_range(
            &mut core,
            &json!({ "channel": 1, "from": 4.0, "to": 2.0 })
        )
        .is_err());
    }

    #[test]
    fn clear_channel_keys_keeps_the_channel_itself() {
        let mut core = twin_core();
        tool_graph_clear_channel_keys(&mut core, &json!({ "channel": 0 })).unwrap();
        assert_eq!(core.project.channels.len(), 3, "channel survives, keys go");
        assert!(core.project.channels[0].keys.is_empty());
        // Clearing an already-empty channel is an error.
        assert!(tool_graph_clear_channel_keys(&mut core, &json!({ "channel": 0 })).is_err());
        assert!(tool_graph_clear_channel_keys(&mut core, &json!({ "channel": 2 })).is_err());
    }

    #[test]
    fn reduce_and_preset_channel_tools_shape_one_channel() {
        let mut core = EditorCore::new();
        core.project.channels = vec![Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(5.0, 50.0),
                Key::flat(10.0, 100.0),
            ],
        }];
        // The middle key sits exactly on the line: the
        // panel-default tolerance (0.5) removes it.
        let out = tool_graph_reduce_keys(&mut core, &json!({ "channel": 0 })).unwrap();
        assert_eq!(out["removed"], 1);
        assert_eq!(core.project.channels[0].keys.len(), 2);
        // Per-channel preset: ease-in on the remaining ramp
        // gives the first key a 0.1-scaled out-tangent
        // (secant 10 -> out 1.0), exactly as
        // graph::apply_tangent_preset defines it.
        tool_graph_tangent_preset_channel(
            &mut core,
            &json!({ "channel": 0, "preset": "ease-in" }),
        )
        .unwrap();
        let k = core.project.channels[0].keys[0];
        assert!(approx(k.out_tan, 1.0), "ease-in out-tangent: {}", k.out_tan);
        assert!(approx(k.in_tan, 10.0), "endpoint in-tangent reuses the secant");
        assert!(tool_graph_tangent_preset_channel(
            &mut core,
            &json!({ "channel": 0, "preset": "bouncy" })
        )
        .is_err());
    }

    #[test]
    fn mirror_channel_tool_defaults_to_the_timeline_middle() {
        let mut core = twin_core();
        // 5-frame demo: middle is frame 2. Twin 0's keys at
        // 0 and 4 mirror onto each other, values travelling.
        tool_graph_mirror_channel(&mut core, &json!({ "channel": 0 })).unwrap();
        let keys = &core.project.channels[0].keys;
        assert!(approx(keys[0].frame, 0.0) && approx(keys[0].value, 140.0));
        assert!(approx(keys[1].frame, 4.0) && approx(keys[1].value, 100.0));
    }

    #[test]
    fn key_all_and_key_angle_and_clear_channels_tools() {
        let mut core = twin_core();
        // Key all at frame 2 (the default would be the
        // playhead; pass it explicitly): every channel
        // gains a key holding its evaluated value there.
        let out = tool_graph_key_all_channels(&mut core, &json!({ "frame": 2.0 })).unwrap();
        assert_eq!(out["keyed"], 3);
        assert_eq!(core.project.channels[0].keys.len(), 3);
        assert_eq!(core.project.channels[2].keys.len(), 1, "even the empty channel is keyed");
        // Key angle with no joint selected reports the
        // core op's refusal on the status line.
        let out = tool_graph_key_angle(&mut core, &json!({})).unwrap();
        assert!(
            out["status"].as_str().unwrap().contains("Select a joint"),
            "status: {}",
            out["status"]
        );
        // With a joint selected it authors the angle key.
        core.selected_node = Some(0);
        core.key_graph_angle();
        let before = core.project.channels.len();
        tool_graph_key_angle(&mut core, &json!({})).unwrap();
        assert!(core.project.channels.len() >= before);
        // Clear all: every channel goes, and (the core
        // op's documented truth) the undo depth does not
        // grow — clear_graph takes no snapshot.
        let depth = core.undo_depth();
        let n_channels = core.project.channels.len();
        let out = tool_graph_clear_channels(&mut core, &json!({})).unwrap();
        assert_eq!(out["cleared"], n_channels as u64);
        assert!(core.project.channels.is_empty());
        assert_eq!(core.undo_depth(), depth, "clear_graph is not an undo step");
    }

    #[test]
    fn adversarial_arguments_error_and_never_panic() {
        let mut core = twin_core();
        // Channel index far past the list, on every
        // index-addressed tool.
        for out in [
            tool_graph_get_channel(&mut core, &json!({ "channel": 99 })),
            tool_graph_delete_key(&mut core, &json!({ "channel": 99, "frame": 0.0 })),
            tool_graph_shift_keys(&mut core, &json!({ "channel": 99, "delta": 1.0 })),
            tool_graph_reduce_keys(&mut core, &json!({ "channel": 99 })),
            tool_graph_clear_channel_keys(&mut core, &json!({ "channel": 99 })),
        ] {
            let err = out.expect_err("out-of-range channel must error");
            assert_eq!(err, "no graph channel at index 99");
        }
        // Negative / wrongly-typed / missing arguments.
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": 0, "frame": -1.0 })).is_err());
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": -1, "frame": 0.0 })).is_err());
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": "0", "frame": 0.0 })).is_err());
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": 0 })).is_err());
        assert!(tool_graph_shift_keys(&mut core, &json!({ "channel": 0, "delta": 0.0 })).is_err());
        assert!(tool_graph_move_key(
            &mut core,
            &json!({ "channel": 0, "from_frame": 0.0, "to_frame": -3.0 })
        )
        .is_err());
        // A JSON null reads as missing, never as zero.
        assert!(tool_graph_delete_key(&mut core, &json!({ "channel": 0, "frame": null })).is_err());
        // Nothing above touched the project.
        assert_eq!(core.project.channels.len(), 3);
        assert_eq!(core.project.channels[0].keys.len(), 2);
    }
}
