//! Shared key selection (Roadmap Wave 1, RM5 — F-4).
//!
//! ONE selected-key state for the whole shell: the graph
//! panel, the dopesheet and (through IA-RM5-2) the timeline
//! marker row all read and write this one model instead of
//! each surface inventing its own notion of "the selected
//! key" — before F-4 the graph panel's selected key was
//! implicit (the key nearest the playhead), the dopesheet
//! had no key selection at all, and the marker row's press
//! only sought the playhead.
//!
//! ## The idiom
//!
//! A selection names ONE channel (an index into
//! `Project::channels`, the same index space the graph
//! panel's `effective_channel_index` resolves in) plus a
//! SET of key frames on it. Frames, not indices: channel
//! key lists re-sort whenever a key moves (the graph
//! panel's `drag_anchor` lesson — an index recorded before
//! a move can name a different key after it), while a
//! frame keeps naming the same key across re-sorts for as
//! long as the key sits there. Frames are stored as
//! `f32::to_bits` in a `BTreeSet`, so the set is ordered,
//! deduplicated and hashable without float-key hacks.
//!
//! Every read resolves DEFENSIVELY against the live
//! channel: a stored frame only counts while a key
//! actually sits within [`KEY_MATCH_TOL`] of it on the
//! named channel ([`KeySel::indices`] /
//! [`KeySel::primary`] / [`KeySel::contains`]). A stale
//! selection — channel deleted, keys deleted elsewhere,
//! project swapped — therefore reads as empty everywhere
//! instead of pointing at the wrong key, and no consumer
//! has to prune before reading. [`KeySel::prune`] /
//! [`KeySel::retain_live`] exist for the writers that want
//! the store itself cleaned (the cancelled-drag path, and
//! RM14's dopesheet editing).
//!
//! ## Gestures, honestly
//!
//! The model carries the full idiom — single select,
//! toggle, range-from-anchor ([`KeySel::extend_to`], the
//! shift-click of audits A5 R2 / A6 R17), marquee range
//! ([`KeySel::select_range`]) and select-all. This Rune
//! revision's `Drag` carries NO modifier state, so in the
//! shell today the live gestures are: click/drag a key in
//! the graph panel (selects it), drag on empty graph
//! canvas (marquee — the range grows live), press a
//! dopesheet cell holding a channel key (selects it).
//! Toggle and extend_to are model-level until a Rune rev
//! exposes modifiers; nothing in the UI pretends a
//! ctrl/shift gesture exists.
//!
//! Selection is SESSION state: never serialised, never
//! snapshotted — selecting keys is not an undoable edit
//! (the frame-ops range selection's precedent). The one
//! mutating consumer, [`delete_selected_keys`], takes
//! exactly ONE snapshot for the whole batch (A5 R2's
//! one-undo-step rule) and refuses — snapshot-free — when
//! nothing live is selected.
//!
//! Scope: CHANNEL keys only. Pose/state/bend key events
//! are not in this model — the dope lane selects the
//! channel key a cell carries; per-kind key events await
//! the one-key-definition work (A5 R4) and RM14 builds
//! dopesheet editing on [`KeySel::indices`] +
//! [`delete_selected_keys`] plus the per-kind paths.
//!
//! ## Editing verbs (Roadmap Wave 1, RM14 — P-2)
//!
//! The dopesheet's Move/Copy ride this model beside the
//! delete: [`move_selected_keys`] shifts the whole
//! selection by whole frames under the nudge collision
//! rule ([`selected_move_error`] is the shared pure
//! check; [`move_selected_keys_by`] is the raw apply
//! the dopesheet drag steps through), and
//! [`copy_selected_keys`] clones the selection for the
//! graph clipboard. Each button-level verb is exactly
//! one undo step; refusals are status-only.

use std::collections::BTreeSet;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, Key};

use crate::rune_sec::graph_panel::channel_label;

/// How near a stored frame must sit to a live key for the
/// two to be the same key: `Channel::add_key`'s replace
/// window — two keys closer than this cannot coexist as
/// distinct authored keys on one channel.
pub const KEY_MATCH_TOL: f32 = 0.01;

/// The shared selected-key state (see the module docs).
/// Lives on the shell `State` as `key_sel` (IA-RM5-1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeySel {
    /// The channel the selection names (index into
    /// `Project::channels`); `None` = no selection.
    pub channel: Option<usize>,
    /// The selected keys' frames, as `f32::to_bits`.
    frames: BTreeSet<u32>,
    /// The range anchor: the frame the last single select
    /// landed on, which [`KeySel::extend_to`] ranges from.
    pub anchor: Option<f32>,
}

impl KeySel {
    /// Nothing selected (no channel, or an empty set).
    pub fn is_empty(&self) -> bool {
        self.channel.is_none() || self.frames.is_empty()
    }

    /// Stored frames count (NOT existence-checked — use
    /// [`KeySel::indices`] against the live channel for
    /// the count that names real keys).
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// The stored frames, ascending.
    pub fn frames(&self) -> Vec<f32> {
        self.frames.iter().map(|&b| f32::from_bits(b)).collect()
    }

    /// Drop the whole selection.
    pub fn clear(&mut self) {
        self.channel = None;
        self.frames.clear();
        self.anchor = None;
    }

    /// The stored entry naming `frame` (within
    /// [`KEY_MATCH_TOL`]), if one is stored.
    fn stored_match(&self, frame: f32) -> Option<u32> {
        self.frames
            .iter()
            .copied()
            .find(|&b| (f32::from_bits(b) - frame).abs() <= KEY_MATCH_TOL)
    }

    /// Select exactly one key: the selection becomes
    /// `frame` on `channel`, and `frame` is the new range
    /// anchor. The one write every consumer's plain press
    /// performs.
    pub fn select_one(&mut self, channel: usize, frame: f32) {
        self.channel = Some(channel);
        self.frames.clear();
        self.frames.insert(frame.to_bits());
        self.anchor = Some(frame);
    }

    /// Toggle one key in/out of the selection (the
    /// ctrl-click of the audits, model-level until Rune
    /// exposes modifiers). A toggle on a DIFFERENT channel
    /// restarts the selection there, exactly like
    /// [`KeySel::select_one`] — the selection names one
    /// channel, always. Toggling the anchor key off moves
    /// the anchor to the lowest remaining frame.
    pub fn toggle(&mut self, channel: usize, frame: f32) {
        if self.channel != Some(channel) {
            self.select_one(channel, frame);
            return;
        }
        match self.stored_match(frame) {
            Some(bits) => {
                self.frames.remove(&bits);
                if self.anchor.map_or(false, |a| (a - frame).abs() <= KEY_MATCH_TOL) {
                    self.anchor = self.frames.iter().next().map(|&b| f32::from_bits(b));
                }
                if self.frames.is_empty() {
                    self.channel = None;
                    self.anchor = None;
                }
            }
            None => {
                self.frames.insert(frame.to_bits());
            }
        }
    }

    /// Range-select on `channel` from the anchor to
    /// `frame` (inclusive both ends) — the shift-click
    /// idiom, at model level. With no live anchor on this
    /// channel it degenerates to [`KeySel::select_one`].
    /// The anchor itself does not move.
    pub fn extend_to(&mut self, ch: &Channel, channel: usize, frame: f32) {
        let Some(anchor) = self.anchor else {
            self.select_one(channel, frame);
            return;
        };
        if self.channel != Some(channel) {
            self.select_one(channel, frame);
            return;
        }
        self.select_range(ch, channel, anchor, frame);
        self.anchor = Some(anchor);
    }

    /// Select every key of `ch` whose frame lies between
    /// `a` and `b` (either order, inclusive) — the
    /// marquee gesture's write, applied live as the
    /// pointer moves. `a` becomes the anchor.
    pub fn select_range(&mut self, ch: &Channel, channel: usize, a: f32, b: f32) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.channel = Some(channel);
        self.frames = ch
            .keys
            .iter()
            .filter(|k| k.frame >= lo && k.frame <= hi)
            .map(|k| k.frame.to_bits())
            .collect();
        self.anchor = Some(a);
        if self.frames.is_empty() {
            self.channel = None;
            self.anchor = None;
        }
    }

    /// Select every key on the channel.
    pub fn select_all(&mut self, ch: &Channel, channel: usize) {
        self.channel = Some(channel);
        self.frames = ch.keys.iter().map(|k| k.frame.to_bits()).collect();
        self.anchor = ch.keys.first().map(|k| k.frame);
        if self.frames.is_empty() {
            self.channel = None;
            self.anchor = None;
        }
    }

    /// Whether `frame` is selected AND names a live key
    /// on `ch`: the stored set holds a frame within
    /// [`KEY_MATCH_TOL`] of `frame`, `channel` is the
    /// selected channel, and a key of `ch` sits within
    /// tolerance of `frame` too.
    pub fn contains(&self, channel: usize, ch: &Channel, frame: f32) -> bool {
        self.channel == Some(channel)
            && self.stored_match(frame).is_some()
            && ch.keys.iter().any(|k| (k.frame - frame).abs() <= KEY_MATCH_TOL)
    }

    /// The live key indices the selection names on
    /// `channel`, ascending — the resolution every
    /// consumer acts on. Empty when the selection names
    /// another channel or only dead frames.
    pub fn indices(&self, channel: usize, ch: &Channel) -> Vec<usize> {
        if self.channel != Some(channel) {
            return Vec::new();
        }
        ch.keys
            .iter()
            .enumerate()
            .filter(|(_, k)| self.stored_match(k.frame).is_some())
            .map(|(i, _)| i)
            .collect()
    }

    /// The selection's primary key on `channel`: the
    /// anchor's index while it is selected and live, else
    /// the first selected live key. The graph panel draws
    /// THIS key's tangent handles (A6 R1: handles follow
    /// the selection, surviving playhead moves).
    pub fn primary(&self, channel: usize, ch: &Channel) -> Option<usize> {
        let idxs = self.indices(channel, ch);
        if let Some(anchor) = self.anchor {
            if let Some(&i) = idxs
                .iter()
                .find(|&&i| (ch.keys[i].frame - anchor).abs() <= KEY_MATCH_TOL)
            {
                return Some(i);
            }
        }
        idxs.first().copied()
    }

    /// Follow a dragged key: if `old` is stored on
    /// `channel`, the stored frame becomes `new` (the
    /// anchor follows too when it named `old`). Called by
    /// the graph drag after each successful move, so the
    /// selection rides the key across re-sorts.
    pub fn replace_frame(&mut self, channel: usize, old: f32, new: f32) {
        if self.channel != Some(channel) {
            return;
        }
        if let Some(bits) = self.stored_match(old) {
            self.frames.remove(&bits);
            self.frames.insert(new.to_bits());
        }
        if self.anchor.map_or(false, |a| (a - old).abs() <= KEY_MATCH_TOL) {
            self.anchor = Some(new);
        }
    }

    /// Drop stored frames that no longer name a live key
    /// on `ch` (channel must still be the selected one).
    pub fn retain_live(&mut self, channel: usize, ch: &Channel) {
        if self.channel != Some(channel) {
            return;
        }
        self.frames
            .retain(|&b| ch.keys.iter().any(|k| (k.frame - f32::from_bits(b)).abs() <= KEY_MATCH_TOL));
        if self
            .anchor
            .map_or(false, |a| self.stored_match(a).is_none())
        {
            self.anchor = self.frames.iter().next().map(|&b| f32::from_bits(b));
        }
        if self.frames.is_empty() {
            self.channel = None;
            self.anchor = None;
        }
    }

    /// Clean the store against the whole project: a dead
    /// channel clears the selection, dead frames drop.
    /// Writers call this when they want the store itself
    /// settled (readers never need to — they resolve
    /// defensively).
    pub fn prune(&mut self, core: &EditorCore) {
        let Some(ci) = self.channel else {
            return;
        };
        match core.project.channels.get(ci) {
            Some(ch) => {
                let ch = ch.clone();
                self.retain_live(ci, &ch);
            }
            None => self.clear(),
        }
    }
}

/// Why moving the selection by `delta` whole frames
/// cannot proceed, as a status-ready sentence — or
/// `None` when the move lands (Roadmap Wave 1, RM14 —
/// P-2: the dopesheet's editing verbs on this model).
/// Pure: the check the button wrapper runs before it
/// may snapshot, and the dopesheet drag runs per step,
/// so both doors share ONE collision rule — the
/// keyframe tools' `nudge_plan` rule widened to a set:
/// no target before frame 0, and no target within 0.5
/// frames of a key that is NOT itself moving (targets
/// keep their mutual distances — the delta is whole
/// frames — so movers can never collide with each
/// other). An empty or stale selection is the refusal
/// `"No keys selected to move."`.
pub fn selected_move_error(core: &EditorCore, sel: &KeySel, delta: i32) -> Option<String> {
    let Some(ci) = sel.channel else {
        return Some("No keys selected to move.".to_string());
    };
    let Some(ch) = core.project.channels.get(ci) else {
        return Some("No keys selected to move.".to_string());
    };
    let idxs = sel.indices(ci, ch);
    if idxs.is_empty() {
        return Some("No keys selected to move.".to_string());
    }
    let moving: std::collections::BTreeSet<usize> = idxs.iter().copied().collect();
    for &i in &idxs {
        let from = ch.keys[i].frame;
        let to = from + delta as f32;
        if to < 0.0 {
            return Some(format!(
                "Key at frame {} cannot move earlier.",
                from + 1.0
            ));
        }
        if ch
            .keys
            .iter()
            .enumerate()
            .any(|(j, k)| !moving.contains(&j) && (k.frame - to).abs() <= 0.5)
        {
            return Some(format!(
                "Frame {} already has a key on this channel.",
                to + 1.0
            ));
        }
    }
    None
}

/// Apply a selection move already cleared by
/// [`selected_move_error`]: every selected key's frame
/// shifts by `delta`, the channel re-sorts, and the
/// selection (and its anchor) follows to the new
/// frames. NO snapshot, NO status — the callers own
/// both (the button wrapper stages one snapshot for
/// the whole move; the dopesheet drag stages one for
/// the whole gesture and steps through here). Returns
/// how many keys moved; 0 when the live selection is
/// empty (the error check's refusal case, which a
/// caller that checked first cannot reach).
pub fn move_selected_keys_by(core: &mut EditorCore, sel: &mut KeySel, delta: i32) -> usize {
    let Some(ci) = sel.channel else {
        return 0;
    };
    let Some(ch) = core.project.channels.get(ci) else {
        return 0;
    };
    let idxs = sel.indices(ci, ch);
    if idxs.is_empty() {
        return 0;
    }
    let orig: Vec<f32> = idxs.iter().map(|&i| ch.keys[i].frame).collect();
    let anchor_moves = sel
        .anchor
        .map_or(false, |a| orig.iter().any(|&f| (f - a).abs() <= KEY_MATCH_TOL));
    let targets: Vec<f32> = orig.iter().map(|&f| f + delta as f32).collect();
    let ch = &mut core.project.channels[ci];
    for (n, &i) in idxs.iter().enumerate() {
        ch.keys[i].frame = targets[n];
    }
    ch.sort_keys();
    sel.frames = targets.iter().map(|f| f.to_bits()).collect();
    if anchor_moves {
        if let Some(a) = sel.anchor {
            sel.anchor = Some(a + delta as f32);
        }
    }
    idxs.len()
}

/// Move every selected key by `delta` whole frames as
/// ONE undo step (Roadmap Wave 1, RM14 — P-2): the
/// dopesheet's Move −1/+1 buttons ride this. A refused
/// move (see [`selected_move_error`]) is status-only
/// and spends no snapshot; a landed move reports its
/// count and leaves the selection naming the moved
/// keys at their new frames. Returns the count moved.
pub fn move_selected_keys(core: &mut EditorCore, sel: &mut KeySel, delta: i32) -> usize {
    if let Some(err) = selected_move_error(core, sel, delta) {
        core.status = err;
        return 0;
    }
    let ci = sel.channel.expect("a cleared move names a channel");
    let (figure, label) = {
        let ch = &core.project.channels[ci];
        (ch.figure, channel_label(ch.target))
    };
    core.snapshot_labeled("Move selected keys");
    let n = move_selected_keys_by(core, sel, delta);
    core.status = format!(
        "Moved {} selected key{} by {:+} frame{} on figure {} {}.",
        n,
        if n == 1 { "" } else { "s" },
        delta,
        if delta.abs() == 1 { "" } else { "s" },
        figure + 1,
        label
    );
    n
}

/// Clone the selected keys, in frame order — the
/// dopesheet's Copy source for the graph clipboard
/// (Roadmap Wave 1, RM14 — P-2). `None` when nothing
/// live is selected. Read-only: copying never
/// snapshots and never touches the project.
pub fn copy_selected_keys(core: &EditorCore, sel: &KeySel) -> Option<Vec<Key>> {
    let ci = sel.channel?;
    let ch = core.project.channels.get(ci)?;
    let idxs = sel.indices(ci, ch);
    if idxs.is_empty() {
        return None;
    }
    Some(idxs.iter().map(|&i| ch.keys[i]).collect())
}

/// Delete every selected key on the selection's channel
/// (Roadmap F-4 / audit A6 R2: the graph panel's
/// delete-key rides the shared model). Exactly ONE
/// snapshot covers the whole batch — the batch is one
/// undo step. Nothing live selected: the refusal goes to
/// `core.status` and NO snapshot is spent. On success the
/// selection is cleared (its keys are gone) and the count
/// is returned.
pub fn delete_selected_keys(core: &mut EditorCore, sel: &mut KeySel) -> usize {
    let Some(ci) = sel.channel else {
        core.status = "No keys selected to delete.".to_string();
        return 0;
    };
    let Some(ch) = core.project.channels.get(ci) else {
        sel.clear();
        core.status = "No keys selected to delete.".to_string();
        return 0;
    };
    let idxs = sel.indices(ci, ch);
    if idxs.is_empty() {
        core.status = "No keys selected to delete.".to_string();
        return 0;
    }
    let figure = ch.figure;
    let label = channel_label(ch.target);
    core.snapshot_labeled("Delete selected keys");
    let ch = &mut core.project.channels[ci];
    for &i in idxs.iter().rev() {
        ch.keys.remove(i);
    }
    let n = idxs.len();
    core.status = format!(
        "Deleted {} selected key{} on figure {} {}.",
        n,
        if n == 1 { "" } else { "s" },
        figure + 1,
        label
    );
    sel.clear();
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::{ChannelTarget, Key};

    fn channel(figure: usize, target: ChannelTarget, frames: &[f32]) -> Channel {
        Channel {
            figure,
            target,
            keys: frames.iter().map(|&f| Key::flat(f, 0.0)).collect(),
        }
    }

    fn core_two_channels() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(0, ChannelTarget::RootX, &[0.0, 4.0, 8.0, 12.0]),
            channel(0, ChannelTarget::RootY, &[2.0, 6.0]),
        ];
        core
    }

    #[test]
    fn select_one_replaces_channel_and_frames() {
        let core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_one(0, 4.0);
        assert_eq!(sel.indices(0, &core.project.channels[0]), vec![1]);
        sel.select_range(&core.project.channels[0], 0, 0.0, 12.0);
        assert_eq!(sel.len(), 4);
        sel.select_one(1, 2.0);
        assert_eq!(sel.channel, Some(1));
        assert_eq!(sel.frames(), vec![2.0]);
        assert!(sel.indices(0, &core.project.channels[0]).is_empty());
        assert_eq!(sel.indices(1, &core.project.channels[1]), vec![0]);
    }

    #[test]
    fn toggle_adds_removes_and_restarts_across_channels() {
        let core = core_two_channels();
        let ch = &core.project.channels[0];
        let mut sel = KeySel::default();
        sel.select_one(0, 0.0);
        sel.toggle(0, 8.0);
        assert_eq!(sel.indices(0, ch), vec![0, 2]);
        sel.toggle(0, 0.0);
        assert_eq!(sel.indices(0, ch), vec![2]);
        assert_eq!(sel.anchor, Some(8.0), "anchor falls to the remaining frame");
        sel.toggle(0, 8.0);
        assert!(sel.is_empty());
        sel.toggle(1, 6.0);
        assert_eq!(sel.channel, Some(1));
        assert_eq!(sel.frames(), vec![6.0]);
    }

    #[test]
    fn extend_to_ranges_from_the_anchor_without_moving_it() {
        let core = core_two_channels();
        let ch = &core.project.channels[0];
        let mut sel = KeySel::default();
        sel.select_one(0, 4.0);
        sel.extend_to(ch, 0, 12.0);
        assert_eq!(sel.indices(0, ch), vec![1, 2, 3]);
        assert_eq!(sel.anchor, Some(4.0));
        sel.extend_to(ch, 0, 0.0);
        assert_eq!(sel.indices(0, ch), vec![0, 1]);
        // No anchor anywhere: extend degenerates to select_one.
        let mut fresh = KeySel::default();
        fresh.extend_to(ch, 0, 8.0);
        assert_eq!(fresh.frames(), vec![8.0]);
    }

    #[test]
    fn select_range_and_all_cover_the_channel() {
        let core = core_two_channels();
        let ch = &core.project.channels[0];
        let mut sel = KeySel::default();
        sel.select_range(ch, 0, 10.0, 3.0);
        assert_eq!(sel.indices(0, ch), vec![1, 2], "reversed bounds still range");
        sel.select_all(ch, 0);
        assert_eq!(sel.indices(0, ch), vec![0, 1, 2, 3]);
        sel.select_range(ch, 0, 100.0, 200.0);
        assert!(sel.is_empty(), "a range over no keys selects nothing");
    }

    #[test]
    fn selection_is_frame_keyed_and_survives_a_resort() {
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_one(0, 8.0);
        // Insert a key BEFORE the selected one: indices
        // shift, the frame keeps naming the same key.
        core.project.channels[0].add_key(2.0, 5.0);
        let ch = &core.project.channels[0];
        let idxs = sel.indices(0, ch);
        assert_eq!(idxs.len(), 1);
        assert_eq!(ch.keys[idxs[0]].frame, 8.0);
        assert_eq!(sel.primary(0, ch), idxs.first().copied());
    }

    #[test]
    fn primary_prefers_the_anchor_then_falls_back() {
        let core = core_two_channels();
        let ch = &core.project.channels[0];
        let mut sel = KeySel::default();
        sel.select_range(ch, 0, 0.0, 12.0);
        sel.anchor = Some(8.0);
        assert_eq!(sel.primary(0, ch), Some(2));
        sel.anchor = Some(99.0);
        assert_eq!(sel.primary(0, ch), Some(0), "dead anchor: first selected");
        sel.clear();
        assert_eq!(sel.primary(0, ch), None);
    }

    #[test]
    fn contains_and_indices_require_a_live_key() {
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_one(0, 4.0);
        assert!(sel.contains(0, &core.project.channels[0], 4.0));
        // The key is deleted elsewhere: the stored frame
        // now names nothing, and every read says so.
        core.project.channels[0].keys.remove(1);
        assert!(!sel.contains(0, &core.project.channels[0], 4.0));
        assert!(sel.indices(0, &core.project.channels[0]).is_empty());
        assert_eq!(sel.primary(0, &core.project.channels[0]), None);
        sel.prune(&core);
        assert!(sel.is_empty(), "prune drops the dead frame");
    }

    #[test]
    fn replace_frame_follows_a_dragged_key() {
        let core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_one(0, 4.0);
        sel.replace_frame(0, 4.0, 5.5);
        assert_eq!(sel.frames(), vec![5.5]);
        assert_eq!(sel.anchor, Some(5.5));
        sel.replace_frame(1, 5.5, 9.0);
        assert_eq!(sel.frames(), vec![5.5], "another channel's drag does not follow");
    }

    #[test]
    fn move_shifts_the_selection_and_the_selection_follows() {
        // RM14: a two-key selection moves as one undo
        // step; the stored frames and the anchor land
        // on the moved keys, and the sibling channel is
        // untouched.
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_range(&core.project.channels[0], 0, 4.0, 8.0);
        let depth = core.undo_depth();
        assert_eq!(move_selected_keys(&mut core, &mut sel, 2), 2);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the move");
        let frames: Vec<f32> = core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 6.0, 10.0, 12.0]);
        assert_eq!(sel.frames(), vec![6.0, 10.0]);
        assert_eq!(sel.anchor, Some(6.0), "the anchor rides its key");
        assert_eq!(core.project.channels[1].keys.len(), 2);
        assert!(core.status.contains("Moved 2 selected keys by +2 frames"));
        core.undo();
        let frames: Vec<f32> = core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 4.0, 8.0, 12.0], "one undo restores all");
    }

    #[test]
    fn move_refusals_spend_no_snapshot() {
        // RM14: blocked by an unselected key, blocked
        // before frame 0, and nothing-selected — each
        // refuses through the one shared check.
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        let depth = core.undo_depth();
        sel.select_one(0, 4.0);
        assert_eq!(move_selected_keys(&mut core, &mut sel, 4), 0);
        assert_eq!(core.status, "Frame 9 already has a key on this channel.");
        assert_eq!(core.undo_depth(), depth);
        sel.select_one(0, 0.0);
        assert_eq!(move_selected_keys(&mut core, &mut sel, -1), 0);
        assert_eq!(core.status, "Key at frame 1 cannot move earlier.");
        assert_eq!(core.undo_depth(), depth);
        sel.clear();
        assert_eq!(move_selected_keys(&mut core, &mut sel, 1), 0);
        assert_eq!(core.status, "No keys selected to move.");
        // Movers never collide with EACH OTHER: two
        // adjacent selected keys shift together.
        sel.select_range(&core.project.channels[0], 0, 4.0, 8.0);
        assert_eq!(move_selected_keys(&mut core, &mut sel, -3), 2);
        let frames: Vec<f32> = core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 1.0, 5.0, 12.0]);
    }

    #[test]
    fn copy_clones_exactly_the_live_selection() {
        // RM14: the clipboard source — frame order,
        // values travelling, stale frames excluded.
        let mut core = core_two_channels();
        core.project.channels[0].keys[1].value = 42.0;
        let mut sel = KeySel::default();
        assert!(copy_selected_keys(&core, &sel).is_none());
        sel.select_range(&core.project.channels[0], 0, 4.0, 12.0);
        let keys = copy_selected_keys(&core, &sel).expect("a live selection copies");
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].frame, 4.0);
        assert_eq!(keys[0].value, 42.0);
        assert_eq!(keys[2].frame, 12.0);
        // A stale stored frame contributes nothing.
        core.project.channels[0].keys.remove(1);
        let keys = copy_selected_keys(&core, &sel).expect("the rest still copies");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn delete_removes_exactly_the_selection_in_one_undo_step() {
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        sel.select_range(&core.project.channels[0], 0, 4.0, 8.0);
        let depth = core.undo_depth();
        assert_eq!(delete_selected_keys(&mut core, &mut sel), 2);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the batch");
        let frames: Vec<f32> = core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 12.0]);
        assert_eq!(core.project.channels[1].keys.len(), 2, "sibling channel untouched");
        assert!(sel.is_empty(), "the selection clears with its keys");
        assert!(
            core.status
                .starts_with("Deleted 2 selected keys on figure 1"),
            "{}",
            core.status
        );
        core.undo();
        assert_eq!(core.project.channels[0].keys.len(), 4, "one undo restores all");
    }

    #[test]
    fn delete_with_nothing_live_selected_refuses_without_a_snapshot() {
        let mut core = core_two_channels();
        let mut sel = KeySel::default();
        let depth = core.undo_depth();
        assert_eq!(delete_selected_keys(&mut core, &mut sel), 0);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, "No keys selected to delete.");
        // A stale selection (key already gone) refuses too.
        sel.select_one(0, 4.0);
        core.project.channels[0].keys.remove(1);
        assert_eq!(delete_selected_keys(&mut core, &mut sel), 0);
        assert_eq!(core.undo_depth(), depth);
    }
}
