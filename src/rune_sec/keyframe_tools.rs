//! Keyframe tools section (Rune Phase D, section 31) — the
//! graph-key controls for the selected figure's selected joint:
//! key the angle here, delete the key here, auto tangents, and
//! the engine's tangent presets, plus a live indicator showing
//! whether the current frame already holds a key on the channel.
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Op map (verified by reading `src/editor.rs` / `src/graph.rs`):
//!
//! * "Key angle here" → `EditorCore::key_graph_angle`
//!   (editor.rs:1203) — keys `Angle(selected_node)` at the
//!   current frame with its current keyframed value.
//! * "Auto tangents" → `EditorCore::smooth_graph_tangents`
//!   (editor.rs:1240). This is the core's one smoothing op: it
//!   runs `graph::smooth_channel` over every channel, which in
//!   turn fits `graph::auto_tangents` (graph.rs:348, :305). The
//!   brief's "auto tangents" and "smooth channel" are the same
//!   op at core level, so there is deliberately ONE button —
//!   two buttons calling one op would be a fake distinction.
//! * Tangent presets → `EditorCore::apply_graph_ease_preset`
//!   (editor.rs:1263), one button per `graph::TangentPreset`
//!   variant (graph.rs:358): Smooth, EaseIn, EaseOut, EaseInOut,
//!   Linear. Scoped to the selected figure's channels. (The
//!   per-key interp modes are section 9's `set_graph_key_interp`
//!   — not duplicated here.)
//! * "Delete key here" → NO `EditorCore` op exists (verified:
//!   the core's graph family is key/smooth/preset/interp/bake/
//!   clear only). The handler below composes the engine's
//!   public API exactly the way `set_graph_key_interp`
//!   (editor.rs:1305) does internally: locate the selected
//!   figure's `Angle(selected_node)` channel, `snapshot()`
//!   (editor.rs:202, pub), then `Channel::remove_key`
//!   (graph.rs:144 — removes the key nearest the frame, within
//!   0.5 frames), then set `status`. Nothing is removed when no
//!   key is near the current frame, and no snapshot is taken
//!   for a no-op.
//! * Copy keys / Paste keys → `stykanimate_rs::graph_keys`
//!   (Phase E, section 11): `graph_keys::copy_keys(core,
//!   channel_index)` / `graph_keys::paste_keys(core,
//!   channel_index, keys, at_frame)`. The Phase D gap is
//!   CLOSED: Phase D found no copy/paste-keys op or key
//!   clipboard anywhere in the engine; Phase E added the
//!   engine helpers, and this section supplies the shell
//!   clipboard (`State::key_clipboard`, an
//!   `Option<Vec<Key>>` owned by the shell — the core's own
//!   clipboard holds whole frames, not keys). Copy stores the
//!   selected angle channel's keys; Paste inserts them at
//!   the current frame into the channel the panel targets
//!   at paste time — `paste_keys` rebases the group so its
//!   earliest key lands on the current frame, replaces
//!   existing keys within 0.5 frames of a pasted key,
//!   snapshots, and reports the paste on the status line
//!   itself. Paste requires that channel to already exist
//!   (key the joint once to create it) — `paste_keys`
//!   takes a channel index, so there is nothing to paste
//!   into until one does. Channel resolution is
//!   `selected_angle_channel_index`, delegating to
//!   `graph_keys::channel_index_of` for the selected
//!   figure's `Angle(selected_node)` target — the same
//!   lookup `delete_key_here_core` uses.
//! * Nudge left / right → NO `EditorCore` op moves a graph
//!   key (verified: the core's graph family is
//!   key/smooth/preset/interp/bake/clear only). The handler
//!   composes the public graph API the way delete does:
//!   resolve the channel via
//!   `selected_angle_channel_index`, plan the move with
//!   `nudge_plan`, `snapshot()` (editor.rs:202) only when
//!   the plan lands, then apply through
//!   `Channel::move_key` (graph.rs:159) passing the key's
//!   own value, so only its frame changes and its tangents
//!   travel with it. Collision rule: if another key sits
//!   within 0.5 frames of the target frame (the engine's
//!   nearest-key window), the nudge is REFUSED with a
//!   status line — the plan runs before any snapshot, and
//!   it never lets `move_key`'s replace-at-target path
//!   silently destroy the occupying key. A target before
//!   frame 0 is refused the same way.
//! * "Clear channel keys" → also a composition: no
//!   per-channel clear op exists (`clear_graph`,
//!   editor.rs:1239, clears EVERY channel in the project).
//!   One `snapshot()`, then `clear_channel_keys` empties
//!   just the selected channel's key list, and the status
//!   line reports how many keys were removed. The channel
//!   itself stays — figure/target pairing intact — so the
//!   joint remains a paste target.
//! * "Key all here" → also a composition: no core op keys
//!   every channel. `key_all_here_core` reads each
//!   channel's current value with `Channel::value_at`
//!   (graph.rs) and inserts it with `Channel::add_key` —
//!   the engine's own insert-or-replace helper, the same
//!   one `key_graph_angle` authors keys with — under ONE
//!   `snapshot()` for the whole project, so the move is a
//!   single undo step however many channels exist. A
//!   project with no channels is a status-only no-op.
//! * "Copy key +10" → a stamp, planned by the nudge's own
//!   `nudge_plan` with `COPY_KEY_OFFSET_FRAMES` as the
//!   delta, so the stamp honours the nudge collision rule
//!   exactly (nothing within 0.5 frames of the landing
//!   spot; refusals land before any snapshot). Where the
//!   nudge MOVES the playhead key, `copy_key_in_channel`
//!   clones it — value, tangents and interp mode — onto
//!   the target frame and leaves the source in place: the
//!   "hold this pose for 10 frames" move.
//!
//! The keyed-frame indicator is a read-only graph query over
//! `core.project.channels` — the same channel
//! `key_graph_angle` authors.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Key, TangentPreset};
use stykanimate_rs::graph_keys;

use crate::chrome_view::btn;
use crate::State;

/// The graph channel this section acts on: the selected
/// figure's `Angle(selected_node)` channel — the one
/// `key_graph_angle` authors and `set_graph_key_interp` edits.
/// `None` when no joint is selected or no channel exists yet.
pub fn selected_angle_channel(core: &EditorCore) -> Option<&Channel> {
    let ni = core.selected_node?;
    core.project
        .channels
        .iter()
        .find(|ch| ch.figure == core.selected_figure && ch.target == ChannelTarget::Angle(ni))
}

/// The index of the channel [`selected_angle_channel`]
/// returns — the form the `graph_keys` copy/paste helpers
/// take. Resolved by `graph_keys::channel_index_of` (the
/// engine's own first-match lookup) for the selected
/// figure's `Angle(selected_node)` target.
pub fn selected_angle_channel_index(core: &EditorCore) -> Option<usize> {
    let ni = core.selected_node?;
    graph_keys::channel_index_of(core, core.selected_figure, ChannelTarget::Angle(ni))
}

/// Whether `keys` holds a key at `frame` (the engine's own
/// tolerance: keys land on whole frames; 0.5 matches
/// `Channel::remove_key`'s nearest-key window, graph.rs:144).
pub fn keyed_here(keys: &[Key], frame: usize) -> bool {
    let f = frame as f32;
    keys.iter().any(|k| (k.frame - f).abs() <= 0.5)
}

/// The tangent preset buttons: every `TangentPreset` variant,
/// in menu order, as (preset, button label, element-key suffix).
pub fn preset_options() -> [(TangentPreset, &'static str, &'static str); 5] {
    [
        (TangentPreset::Smooth, "Smooth", "smooth"),
        (TangentPreset::EaseIn, "Ease in", "ease-in"),
        (TangentPreset::EaseOut, "Ease out", "ease-out"),
        (TangentPreset::EaseInOut, "Ease in-out", "ease-in-out"),
        (TangentPreset::Linear, "Linear", "linear"),
    ]
}

/// "Figure 1 · joint 3" for the current selection, or the
/// prompt to select one.
pub fn target_text(core: &EditorCore) -> String {
    match core.selected_node {
        Some(ni) => format!("Figure {} · joint {}", core.selected_figure + 1, ni),
        None => "No joint selected".to_string(),
    }
}

/// The keyed-frame indicator line: whether the current frame
/// holds a key on the selected angle channel, and how many
/// keys the channel holds in total.
pub fn indicator_text(core: &EditorCore) -> String {
    if core.selected_node.is_none() {
        return "Select a joint to edit its angle keys.".to_string();
    }
    match selected_angle_channel(core) {
        None => format!("No keys yet on this joint · frame {}", core.current + 1),
        Some(ch) => {
            let n = ch.keys.len();
            let noun = if n == 1 { "key" } else { "keys" };
            if keyed_here(&ch.keys, core.current) {
                format!("● Key at frame {} · {n} {noun} on channel", core.current + 1)
            } else {
                format!("○ No key at frame {} · {n} {noun} on channel", core.current + 1)
            }
        }
    }
}

/// Delete the key at (nearest to) the current frame on the
/// selected angle channel. Returns whether a key was removed.
/// See the module docs for why this is composed from the
/// engine's public API instead of calling a core op.
pub fn delete_key_here_core(core: &mut EditorCore) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Delete key.".into();
        return false;
    };
    let fi = core.selected_figure;
    let frame = core.current as f32;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
        return false;
    };
    if !keyed_here(&core.project.channels[ci].keys, core.current) {
        core.status = format!("No key at frame {} to delete.", core.current + 1);
        return false;
    }
    core.snapshot();
    let removed = core.project.channels[ci].remove_key(frame);
    if removed {
        core.status = format!("Deleted graph key at frame {}.", core.current + 1);
    }
    removed
}

/// `State` wrapper for [`delete_key_here_core`].
pub fn delete_key_here(s: &mut State) {
    delete_key_here_core(&mut s.core);
}

/// Copy the selected angle channel's keys via
/// `graph_keys::copy_keys`. Returns the copied keys for the
/// shell clipboard, or `None` (status set) when there is no
/// joint selected, no channel on it, or the channel holds
/// no keys — an empty copy never reaches the clipboard, so
/// Paste's muted state always means "nothing to paste".
pub fn copy_keys_here_core(core: &mut EditorCore) -> Option<Vec<Key>> {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Copy keys.".into();
        return None;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {} to copy.", fi + 1, ni);
        return None;
    };
    let keys = graph_keys::copy_keys(core, ci)?;
    if keys.is_empty() {
        core.status = format!("No graph keys for figure {} joint {} to copy.", fi + 1, ni);
        return None;
    }
    core.status = format!(
        "Copied {} keys from figure {} joint {}.",
        keys.len(),
        fi + 1,
        ni
    );
    Some(keys)
}

/// Paste `clipboard`'s keys at the current frame into the
/// selected angle channel via `graph_keys::paste_keys`,
/// which rebases the group so its earliest key lands on
/// the current frame, snapshots, and reports the paste on
/// the status line itself — so this wrapper sets status
/// only for its own pre-checks (no clipboard, no joint, no
/// target channel). Returns whether keys were pasted. The
/// target channel must already exist (see the module
/// docs).
pub fn paste_keys_here_core(core: &mut EditorCore, clipboard: Option<&[Key]>) -> bool {
    let Some(keys) = clipboard.filter(|k| !k.is_empty()) else {
        core.status = "Copy some keys first, then Paste keys.".into();
        return false;
    };
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Paste keys.".into();
        return false;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!(
            "No graph channel for figure {} joint {} to paste into — key it once first.",
            fi + 1,
            ni
        );
        return false;
    };
    graph_keys::paste_keys(core, ci, keys, core.current as f32)
}

/// `State` wrapper for [`copy_keys_here_core`]: stores the
/// copied keys into the shell key clipboard.
pub fn copy_keys_here(s: &mut State) {
    if let Some(keys) = copy_keys_here_core(&mut s.core) {
        s.key_clipboard = Some(keys);
    }
}

/// `State` wrapper for [`paste_keys_here_core`]: pastes
/// from the shell key clipboard (read-only — a clipboard
/// can be pasted more than once).
pub fn paste_keys_here(s: &mut State) {
    paste_keys_here_core(&mut s.core, s.key_clipboard.as_deref());
}

/// What nudging the key at the playhead by `delta` frames
/// does — the read-only plan the nudge helpers share.
/// The copy-to-frame stamp plans through the same type
/// (its `Moved` reads "a stamp lands `from` → `to`"), so
/// both ops share one collision rule. Computed without
/// touching any keys: the core wrapper must know the
/// nudge will land before it may snapshot (this file's
/// rule — no snapshot for a no-op), and the collision
/// rule lives here, in one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NudgeOutcome {
    /// A key moves (or would move) `from` one frame `to`
    /// another.
    Moved { from: f32, to: f32 },
    /// No key sits at the playhead frame (within the
    /// engine's 0.5-frame key window).
    NoKeyHere,
    /// Another key already occupies the target frame `at`
    /// (within 0.5 frames) — the nudge is refused rather
    /// than letting `Channel::move_key` silently replace
    /// the occupant.
    Blocked { at: f32 },
    /// The key sits at `from` and the target lies before
    /// frame 0 — keys never live at negative frames.
    BeforeStart { from: f32 },
}

/// Index of the key nearest to `frame`, when one is within
/// the engine's 0.5-frame tolerance — the public twin of
/// `Channel`'s private `nearest_key_index` (graph.rs),
/// over a bare key slice.
pub fn key_index_near(keys: &[Key], frame: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, k) in keys.iter().enumerate() {
        let d = (k.frame - frame).abs();
        if d <= 0.5 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Plan a nudge of the key at `frame` (the playhead) by
/// `delta` frames over a bare key slice: which key moves,
/// where to, or why not. Pure — shared by
/// [`nudge_key_in_channel`] (which applies the plan) and
/// [`nudge_key_here_core`] (which snapshots only when the
/// plan says `Moved`), and by the copy-to-frame stamp
/// ([`copy_key_in_channel`] / [`copy_key_to_frame_core`]),
/// which plans with the stamp offset as its delta.
pub fn nudge_plan(keys: &[Key], frame: f32, delta: i32) -> NudgeOutcome {
    let Some(idx) = key_index_near(keys, frame) else {
        return NudgeOutcome::NoKeyHere;
    };
    let from = keys[idx].frame;
    let to = from + delta as f32;
    if to < 0.0 {
        return NudgeOutcome::BeforeStart { from };
    }
    if keys
        .iter()
        .enumerate()
        .any(|(i, k)| i != idx && (k.frame - to).abs() <= 0.5)
    {
        return NudgeOutcome::Blocked { at: to };
    }
    NudgeOutcome::Moved { from, to }
}

/// Apply a nudge to a bare channel: move the key at
/// `frame` by `delta` frames via `Channel::move_key`
/// (graph.rs:159 — keeps the key's tangents and the
/// sorted order), passing the key's own value so only its
/// frame changes. Refusals leave the channel untouched.
/// Because the plan already refused every key within 0.5
/// frames of the target, `move_key`'s replace-at-target
/// path (a 0.01 window) can never fire from here — a
/// nudge never destroys another key.
pub fn nudge_key_in_channel(ch: &mut Channel, frame: f32, delta: i32) -> NudgeOutcome {
    let plan = nudge_plan(&ch.keys, frame, delta);
    if let NudgeOutcome::Moved { from, to } = plan {
        let idx = key_index_near(&ch.keys, frame).expect("a Moved plan found the key");
        let value = ch.keys[idx].value;
        let moved = ch.move_key(from, to, value);
        debug_assert!(moved, "a Moved plan always moves: {plan:?}");
    }
    plan
}

/// Nudge the key at the current frame on the selected
/// angle channel by `delta` frames (−1 left, +1 right),
/// as one undo step. Returns whether a key moved. See
/// the module docs for why this is a composition: the
/// plan is checked first, and only a `Moved` plan
/// snapshots — refusals are status-only, like every
/// other no-op in this section.
pub fn nudge_key_here_core(core: &mut EditorCore, delta: i32) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Nudge.".into();
        return false;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {} to nudge.", fi + 1, ni);
        return false;
    };
    let frame = core.current as f32;
    match nudge_plan(&core.project.channels[ci].keys, frame, delta) {
        NudgeOutcome::Moved { from, to } => {
            core.snapshot();
            let applied = nudge_key_in_channel(&mut core.project.channels[ci], frame, delta);
            debug_assert_eq!(applied, NudgeOutcome::Moved { from, to });
            core.status = format!(
                "Moved graph key from frame {} to frame {}.",
                from + 1.0,
                to + 1.0
            );
            true
        }
        NudgeOutcome::NoKeyHere => {
            core.status = format!("No key at frame {} to nudge.", core.current + 1);
            false
        }
        NudgeOutcome::Blocked { at } => {
            core.status = format!("Frame {} already has a key on this channel.", at + 1.0);
            false
        }
        NudgeOutcome::BeforeStart { from } => {
            core.status = format!("Key at frame {} cannot move earlier.", from + 1.0);
            false
        }
    }
}

/// `State` wrapper for [`nudge_key_here_core`].
pub fn nudge_key_here(s: &mut State, delta: i32) {
    nudge_key_here_core(&mut s.core, delta);
}

/// Remove every key from a bare channel, returning how
/// many keys it held. The channel itself stays — an
/// emptied channel still carries the figure/target
/// pairing (and still counts as "keyed once", so Paste
/// keeps a home in it).
pub fn clear_channel_keys(ch: &mut Channel) -> usize {
    let n = ch.keys.len();
    ch.keys.clear();
    n
}

/// Clear every key on the selected angle channel, as one
/// undo step; returns the number of keys removed. Every
/// no-op — no joint selected, no channel on the joint, an
/// already-empty channel — returns 0 with a status line
/// and NO snapshot, matching this section's discipline.
/// The status line reports the count on success.
pub fn clear_channel_keys_core(core: &mut EditorCore) -> usize {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Clear channel.".into();
        return 0;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {} to clear.", fi + 1, ni);
        return 0;
    };
    if core.project.channels[ci].keys.is_empty() {
        core.status = format!("No graph keys for figure {} joint {} to clear.", fi + 1, ni);
        return 0;
    }
    core.snapshot();
    let n = clear_channel_keys(&mut core.project.channels[ci]);
    core.status = format!(
        "Cleared {n} graph key(s) from figure {} joint {}.",
        fi + 1,
        ni
    );
    n
}

/// `State` wrapper for [`clear_channel_keys_core`].
pub fn clear_channel_keys_here(s: &mut State) {
    clear_channel_keys_core(&mut s.core);
}

/// Key one bare channel at `frame` with the value the
/// channel itself evaluates to there: `Channel::value_at`
/// is read first, then `Channel::add_key` — the engine's
/// own insert helper, the same one `key_graph_angle`
/// authors keys with — inserts a fresh flat-tangent key,
/// replacing any key already at that frame rather than
/// duplicating it. Because the keyed value IS the
/// evaluated value, keying never moves the channel's
/// curve at the keyed frame.
pub fn key_channel_here(ch: &mut Channel, frame: f32) {
    let value = ch.value_at(frame);
    ch.add_key(frame, value);
}

/// Key EVERY channel in the project at the current frame,
/// each holding its own current evaluated value — the
/// standard "key everything here" move. Returns how many
/// channels were keyed. The whole operation is ONE undo
/// step: a single `snapshot()` covers every channel,
/// however many exist (the clear-channel composition,
/// widened to the project). No selection is required —
/// keying everything is the point. A project with no
/// channels is a status-only no-op: 0 returned, no
/// snapshot, matching this section's discipline.
pub fn key_all_here_core(core: &mut EditorCore) -> usize {
    let n = core.project.channels.len();
    if n == 0 {
        core.status = "No graph channels to key.".into();
        return 0;
    }
    let frame = core.current as f32;
    core.snapshot();
    for ch in &mut core.project.channels {
        key_channel_here(ch, frame);
    }
    core.status = format!("Keyed {n} channels at frame {}.", core.current + 1);
    n
}

/// `State` wrapper for [`key_all_here_core`].
pub fn key_all_here(s: &mut State) {
    key_all_here_core(&mut s.core);
}

/// Frames ahead of the playhead key that
/// [`copy_key_to_frame_core`] stamps its copy at — the
/// "hold this pose for 10 frames" span.
pub const COPY_KEY_OFFSET_FRAMES: i32 = 10;

/// Stamp a copy of the key at `frame` onto `frame +
/// offset` in a bare channel. The plan is [`nudge_plan`]
/// with the stamp offset as its delta, so the stamp
/// honours exactly the nudge collision rule — no key
/// within 0.5 frames of the landing spot — and a refused
/// stamp leaves the channel untouched. A landed stamp is
/// the WHOLE key cloned (value, tangents and interp mode
/// travel; only the frame changes), inserted at its
/// sorted position; unlike [`nudge_key_in_channel`],
/// which moves the key, the source key stays where it
/// was, so the channel ends up holding the pose at both
/// frames. The plan's guarantee (nothing within 0.5
/// frames of the target) is also why a plain insert is
/// safe here: no replace-at-target path can ever fire.
pub fn copy_key_in_channel(ch: &mut Channel, frame: f32, offset: i32) -> NudgeOutcome {
    let plan = nudge_plan(&ch.keys, frame, offset);
    if let NudgeOutcome::Moved { to, .. } = plan {
        let idx = key_index_near(&ch.keys, frame).expect("a Moved plan found the key");
        let mut stamped = ch.keys[idx];
        stamped.frame = to;
        let pos = ch.keys.partition_point(|k| k.frame < to);
        ch.keys.insert(pos, stamped);
    }
    plan
}

/// Stamp a copy of the selected channel's key at the
/// current frame onto the frame
/// [`COPY_KEY_OFFSET_FRAMES`] later, as one undo step —
/// the "hold this pose for 10 frames" move. Returns
/// whether a stamp landed. Refusals mirror the nudge's
/// exactly, because both plan through [`nudge_plan`]: no
/// key at the playhead (`NoKeyHere`) and a key within 0.5
/// frames of the landing spot (`Blocked`) are status-only
/// and never snapshot.
pub fn copy_key_to_frame_core(core: &mut EditorCore) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Copy key.".into();
        return false;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {} to copy.", fi + 1, ni);
        return false;
    };
    let frame = core.current as f32;
    match nudge_plan(&core.project.channels[ci].keys, frame, COPY_KEY_OFFSET_FRAMES) {
        NudgeOutcome::Moved { from, to } => {
            core.snapshot();
            let applied =
                copy_key_in_channel(&mut core.project.channels[ci], frame, COPY_KEY_OFFSET_FRAMES);
            debug_assert_eq!(applied, NudgeOutcome::Moved { from, to });
            core.status = format!(
                "Copied graph key from frame {} to frame {}.",
                from + 1.0,
                to + 1.0
            );
            true
        }
        NudgeOutcome::NoKeyHere => {
            core.status = format!("No key at frame {} to copy.", core.current + 1);
            false
        }
        NudgeOutcome::Blocked { at } => {
            core.status = format!("Frame {} already has a key on this channel.", at + 1.0);
            false
        }
        NudgeOutcome::BeforeStart { from } => {
            // Unreachable for a +10 stamp — its targets
            // only ever move later — but the match stays
            // exhaustive if the offset ever changes sign.
            core.status = format!("Key at frame {} cannot move earlier.", from + 1.0);
            false
        }
    }
}

/// `State` wrapper for [`copy_key_to_frame_core`].
pub fn copy_key_to_frame(s: &mut State) {
    copy_key_to_frame_core(&mut s.core);
}

/// One clipboard button, in the chrome's metrics
/// (`chrome_view::btn`). When `enabled` is false the button
/// renders muted (off fill, secondary text) and carries NO
/// press handler — an inert element, not a control that
/// silently does nothing (the pose_tools muting pattern).
fn kbtn(
    key: &str,
    label: &str,
    enabled: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let base = el().key(key).px(10.0).py(5.0).radius(6.0);
    if enabled {
        base.fill(t.card)
            .hover(t.control_track)
            .press(t.control_off)
            .on_press(f)
            .child(text(label).font(type_::LABEL).color(t.text))
    } else {
        base.fill(t.control_off)
            .child(text(label).font(type_::LABEL).color(t.text_secondary))
    }
}

/// The keyframe tools section: header, selection target,
/// keyed-frame indicator, the key/delete/auto buttons, the
/// copy/paste-keys clipboard row (with Clear channel
/// keys), the nudge row, the key-all / copy-key stamp row,
/// and the five tangent preset buttons (scoped by the op
/// to the selected figure). Buttons highlight with real
/// state: Delete is accent-filled exactly when a key sits
/// at this frame; Copy is muted until a channel exists to
/// copy from, and Paste is muted while the key clipboard
/// is empty. The stamp row mutes the same way: Key all
/// needs at least one channel in the project, Copy key
/// +10 needs the selected joint's channel.
pub fn keyframe_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let keyed = selected_angle_channel(core).is_some_and(|ch| keyed_here(&ch.keys, core.current));
    let mut presets = row().key("keyframe-presets").gap(6.0);
    for (preset, label, suffix) in preset_options() {
        presets = presets.child(btn(
            &format!("key-preset-{suffix}"),
            label,
            false,
            t,
            move |s: &mut State| s.core.apply_graph_ease_preset(preset),
        ));
    }
    col()
        .key("keyframe-tools")
        .w_full()
        .gap(6.0)
        .child(text("Keyframes").font(type_::TITLE).color(t.text))
        .child(
            text(target_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(indicator_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .key("keyframe-actions")
                .gap(6.0)
                .child(btn("key-angle", "Key angle here", false, t, |s| {
                    s.core.key_graph_angle()
                }))
                .child(btn("key-delete", "Delete key here", keyed, t, |s| {
                    delete_key_here(s)
                }))
                .child(btn("key-auto", "Auto tangents", false, t, |s| {
                    s.core.smooth_graph_tangents()
                })),
        )
        .child(
            row()
                .key("keyframe-clipboard")
                .gap(6.0)
                .child(kbtn(
                    "key-copy",
                    "Copy keys",
                    selected_angle_channel_index(core).is_some(),
                    t,
                    |s| copy_keys_here(s),
                ))
                .child(kbtn(
                    "key-paste",
                    "Paste keys",
                    s.key_clipboard.as_ref().is_some_and(|k| !k.is_empty()),
                    t,
                    |s| paste_keys_here(s),
                ))
                .child(btn("key-clear-channel", "Clear channel keys", false, t, |s| {
                    clear_channel_keys_here(s)
                })),
        )
        .child(
            row()
                .key("keyframe-nudge")
                .gap(6.0)
                .child(btn("key-nudge-l", "Nudge left", false, t, |s| {
                    nudge_key_here(s, -1)
                }))
                .child(btn("key-nudge-r", "Nudge right", false, t, |s| {
                    nudge_key_here(s, 1)
                })),
        )
        .child(
            row()
                .key("keyframe-stamp")
                .gap(6.0)
                .child(kbtn(
                    "key-all-here",
                    "Key all here",
                    !core.project.channels.is_empty(),
                    t,
                    |s| key_all_here(s),
                ))
                .child(kbtn(
                    "key-copy-to-frame",
                    "Copy key +10",
                    selected_angle_channel_index(core).is_some(),
                    t,
                    |s| copy_key_to_frame(s),
                )),
        )
        .child(presets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::Interp;

    #[test]
    fn keyed_here_matches_whole_frames_within_half_a_frame() {
        let keys = [Key::flat(0.0, 10.0), Key::flat(4.0, 20.0), Key::flat(9.0, 30.0)];
        assert!(keyed_here(&keys, 0));
        assert!(keyed_here(&keys, 4));
        assert!(keyed_here(&keys, 9));
        assert!(!keyed_here(&keys, 1));
        assert!(!keyed_here(&keys, 5));
        assert!(!keyed_here(&[], 0));
    }

    #[test]
    fn preset_options_cover_every_tangent_preset_once() {
        let opts = preset_options();
        assert_eq!(opts.len(), 5);
        let mut seen: Vec<TangentPreset> = opts.iter().map(|(p, _, _)| *p).collect();
        seen.sort_by_key(|p| format!("{p:?}"));
        seen.dedup();
        assert_eq!(seen.len(), 5, "presets must be distinct: {opts:?}");
        assert!(opts.iter().any(|(p, _, _)| *p == TangentPreset::Smooth));
        assert!(opts.iter().any(|(p, _, _)| *p == TangentPreset::Linear));
        // Element-key suffixes must be unique too.
        let mut suffixes: Vec<&str> = opts.iter().map(|(_, _, s)| *s).collect();
        suffixes.sort();
        suffixes.dedup();
        assert_eq!(suffixes.len(), 5);
    }

    #[test]
    fn indicator_prompts_for_a_joint_when_none_selected() {
        let core = EditorCore::new();
        // A fresh core may or may not preselect a node; force
        // the no-selection case the text is written for.
        let mut core = core;
        core.selected_node = None;
        assert_eq!(target_text(&core), "No joint selected");
        assert!(indicator_text(&core).starts_with("Select a joint"));
    }

    #[test]
    fn delete_key_here_removes_only_the_key_at_the_current_frame() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.key_graph_angle();
        let at_first = core.current;
        assert!(
            selected_angle_channel(&core).is_some_and(|ch| keyed_here(&ch.keys, at_first)),
            "key_graph_angle must key the current frame"
        );
        assert!(indicator_text(&core).starts_with('●'));

        // Key a second frame, then delete only the first.
        core.goto((at_first + 2) % core.frame_count().max(3));
        core.key_graph_angle();
        core.goto(at_first);
        assert!(delete_key_here_core(&mut core));
        let ch = selected_angle_channel(&core).expect("channel survives key deletion");
        assert!(!keyed_here(&ch.keys, at_first), "the key here is gone");
        assert_eq!(ch.keys.len(), 1, "the other frame's key is untouched");
        assert!(indicator_text(&core).starts_with('○'));

        // Deleting again is a clean no-op (status only).
        assert!(!delete_key_here_core(&mut core));
    }

    #[test]
    fn delete_key_here_noops_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert!(!delete_key_here_core(&mut core));
        assert!(core.status.contains("Select a joint"));
    }

    /// A core whose figure 0 / joint 1 angle channel holds
    /// flat keys at frames 0 and 2 (values 10 and 30), with
    /// joint 1 selected — the copy/paste fixture. Both keys
    /// sit inside the demo's 5-frame timeline so the pasted
    /// group (shifted by the paste frame) stays in range.
    fn core_with_angle_keys() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        });
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core
    }

    #[test]
    fn channel_index_matches_the_selected_angle_channel() {
        let mut core = core_with_angle_keys();
        assert_eq!(selected_angle_channel_index(&core), Some(0));
        core.selected_node = Some(2);
        assert_eq!(selected_angle_channel_index(&core), None);
        core.selected_node = None;
        assert_eq!(selected_angle_channel_index(&core), None);
    }

    #[test]
    fn copy_keys_here_returns_the_selected_channels_keys() {
        let mut core = core_with_angle_keys();
        let keys = copy_keys_here_core(&mut core).expect("channel has keys to copy");
        assert_eq!(keys.len(), 2);
        assert_eq!((keys[0].frame, keys[0].value), (0.0, 10.0));
        assert_eq!((keys[1].frame, keys[1].value), (2.0, 30.0));
        assert!(core.status.contains("Copied 2 keys"));
    }

    #[test]
    fn copy_then_paste_lands_the_keys_at_the_current_frame() {
        let mut core = core_with_angle_keys();
        let keys = copy_keys_here_core(&mut core).expect("copied");
        // Retarget: same channel, emptied, playhead on frame
        // index 1. The pasted group lands starting there, so
        // the keys copied from frames 0/2 arrive at 1/3 with
        // their values intact.
        core.project.channels[0].keys.clear();
        core.current = 1;
        assert!(paste_keys_here_core(&mut core, Some(&keys)));
        let ch = selected_angle_channel(&core).expect("channel survives paste");
        assert_eq!(ch.keys.len(), 2);
        assert_eq!((ch.keys[0].frame, ch.keys[0].value), (1.0, 10.0));
        assert_eq!((ch.keys[1].frame, ch.keys[1].value), (3.0, 30.0));
        // paste_keys reports the paste itself, in its own
        // wording ("Pasted 2 graph key(s) at frame 1 …").
        assert!(core.status.contains("Pasted 2 graph key(s)"));
    }

    #[test]
    fn paste_keys_here_noops_without_a_clipboard() {
        let mut core = core_with_angle_keys();
        assert!(!paste_keys_here_core(&mut core, None));
        assert!(core.status.contains("Copy some keys first"));
        assert!(!paste_keys_here_core(&mut core, Some(&[])));
        // The channel is untouched either way.
        assert_eq!(selected_angle_channel(&core).map(|ch| ch.keys.len()), Some(2));
    }

    #[test]
    fn copy_keys_here_noops_without_a_channel() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        assert!(copy_keys_here_core(&mut core).is_none());
        assert!(core.status.contains("No graph keys"));
        core.selected_node = None;
        assert!(copy_keys_here_core(&mut core).is_none());
        assert!(core.status.contains("Select a joint"));
    }

    #[test]
    fn paste_keys_here_noops_without_a_target_channel() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        let keys = [Key::flat(0.0, 10.0)];
        assert!(!paste_keys_here_core(&mut core, Some(&keys)));
        assert!(core.status.contains("key it once first"));
    }

    #[test]
    fn key_index_near_takes_the_nearest_key_within_half_a_frame() {
        let keys = [Key::flat(0.0, 10.0), Key::flat(4.0, 20.0), Key::flat(9.0, 30.0)];
        assert_eq!(key_index_near(&keys, 0.0), Some(0));
        assert_eq!(key_index_near(&keys, 4.4), Some(1));
        assert_eq!(key_index_near(&keys, 8.6), Some(2));
        assert_eq!(key_index_near(&keys, 2.0), None, "2 frames from any key");
        assert_eq!(key_index_near(&[], 0.0), None);
    }

    #[test]
    fn nudge_plan_moves_the_playhead_key_one_frame_either_way() {
        let keys = [Key::flat(0.0, 10.0), Key::flat(2.0, 30.0), Key::flat(5.0, 50.0)];
        assert_eq!(
            nudge_plan(&keys, 2.0, 1),
            NudgeOutcome::Moved { from: 2.0, to: 3.0 }
        );
        assert_eq!(
            nudge_plan(&keys, 2.0, -1),
            NudgeOutcome::Moved { from: 2.0, to: 1.0 }
        );
        assert_eq!(
            nudge_plan(&keys, 5.0, 1),
            NudgeOutcome::Moved { from: 5.0, to: 6.0 },
            "no upper clamp: channels may outlive the timeline"
        );
    }

    #[test]
    fn nudge_plan_refuses_collisions_edges_and_empty_spots() {
        let keys = [Key::flat(0.0, 10.0), Key::flat(1.0, 20.0), Key::flat(4.0, 40.0)];
        // Adjacent keys block each other in both directions.
        assert_eq!(nudge_plan(&keys, 0.0, 1), NudgeOutcome::Blocked { at: 1.0 });
        assert_eq!(nudge_plan(&keys, 1.0, -1), NudgeOutcome::Blocked { at: 0.0 });
        // A key 0.4 frames from the target occupies it too —
        // the refusal window is the engine's 0.5 nearest-key
        // window, not an exact-frame match.
        let frac = [Key::flat(2.0, 10.0), Key::flat(3.4, 20.0)];
        assert_eq!(nudge_plan(&frac, 2.0, 1), NudgeOutcome::Blocked { at: 3.0 });
        // Frame 0 cannot move earlier; bare frames have no key.
        assert_eq!(
            nudge_plan(&keys, 0.0, -1),
            NudgeOutcome::BeforeStart { from: 0.0 }
        );
        assert_eq!(nudge_plan(&keys, 3.0, 1), NudgeOutcome::NoKeyHere);
        assert_eq!(nudge_plan(&[], 0.0, 1), NudgeOutcome::NoKeyHere);
    }

    #[test]
    fn nudge_key_in_channel_moves_the_key_and_keeps_its_shape() {
        let mut shaped = Key::flat(2.0, 30.0);
        shaped.in_tan = 1.5;
        shaped.out_tan = -2.0;
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), shaped, Key::flat(5.0, 50.0)],
        };
        let out = nudge_key_in_channel(&mut ch, 2.0, 1);
        assert_eq!(out, NudgeOutcome::Moved { from: 2.0, to: 3.0 });
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 3.0, 5.0], "the moved key stays sorted");
        let moved = ch.keys[1];
        assert_eq!(moved.value, 30.0, "a frame nudge never changes the value");
        assert_eq!(
            (moved.in_tan, moved.out_tan),
            (1.5, -2.0),
            "tangents travel with the key"
        );
        // A refused nudge changes nothing at all.
        let before = ch.keys.clone();
        assert_eq!(
            nudge_key_in_channel(&mut ch, 5.0, -2),
            NudgeOutcome::Blocked { at: 3.0 }
        );
        assert_eq!(ch.keys, before, "the occupant survived the refused nudge");
    }

    #[test]
    fn nudge_key_here_core_moves_the_playhead_key_as_one_undo_step() {
        let mut core = core_with_angle_keys(); // keys at 0 (10) and 2 (30)
        core.current = 0;
        let edits_before = core.edits_since_autosave();
        assert!(nudge_key_here_core(&mut core, 1));
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "exactly one snapshot"
        );
        assert!(
            core.status.contains("Moved graph key from frame 1 to frame 2"),
            "status reports the move: {}",
            core.status
        );
        let ch = selected_angle_channel(&core).unwrap();
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![1.0, 2.0]);
        assert_eq!(ch.keys[0].value, 10.0, "the moved key kept its value");
        core.undo();
        let ch = selected_angle_channel(&core).unwrap();
        assert_eq!(ch.keys[0].frame, 0.0, "undo restores the key's frame");
    }

    #[test]
    fn nudge_key_here_core_refuses_a_collision_without_snapshotting() {
        let mut core = core_with_angle_keys();
        core.project.channels[0].keys.push(Key::flat(1.0, 99.0));
        core.project.channels[0].sort_keys();
        core.current = 0;
        let edits_before = core.edits_since_autosave();
        assert!(!nudge_key_here_core(&mut core, 1));
        assert!(core.status.contains("already has a key"), "status: {}", core.status);
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "a refused nudge never snapshots"
        );
        let ch = selected_angle_channel(&core).unwrap();
        assert_eq!(ch.keys.len(), 3, "no key moved and none was destroyed");
        assert!(keyed_here(&ch.keys, 0) && keyed_here(&ch.keys, 1));
    }

    #[test]
    fn nudge_key_here_core_noops_cleanly() {
        let mut core = core_with_angle_keys();
        // Playhead on a frame with no key.
        core.current = 4;
        assert!(!nudge_key_here_core(&mut core, 1));
        assert!(core.status.contains("No key at frame 5 to nudge"));
        // The frame-0 key cannot move earlier.
        core.current = 0;
        assert!(!nudge_key_here_core(&mut core, -1));
        assert!(core.status.contains("cannot move earlier"));
        // No channel on the selected joint.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        assert!(!nudge_key_here_core(&mut bare, 1));
        assert!(bare.status.contains("No graph keys"));
        // No joint selected at all.
        bare.selected_node = None;
        assert!(!nudge_key_here_core(&mut bare, 1));
        assert!(bare.status.contains("Select a joint"));
    }

    #[test]
    fn clear_channel_keys_empties_a_bare_channel_and_counts() {
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        };
        assert_eq!(clear_channel_keys(&mut ch), 2);
        assert!(ch.keys.is_empty());
        assert_eq!(
            clear_channel_keys(&mut ch),
            0,
            "clearing an empty channel counts zero"
        );
    }

    #[test]
    fn clear_channel_keys_core_clears_as_one_undo_step_and_reports_the_count() {
        let mut core = core_with_angle_keys();
        let edits_before = core.edits_since_autosave();
        assert_eq!(clear_channel_keys_core(&mut core), 2);
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "exactly one snapshot"
        );
        assert!(
            core.status.contains("Cleared 2 graph key(s)"),
            "status reports the count: {}",
            core.status
        );
        let ch = selected_angle_channel(&core).expect("the channel itself survives");
        assert!(ch.keys.is_empty());
        // Clearing again is a status-only no-op.
        assert_eq!(clear_channel_keys_core(&mut core), 0);
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "no second snapshot for the no-op"
        );
        // Undo restores the cleared keys.
        core.undo();
        assert_eq!(selected_angle_channel(&core).map(|c| c.keys.len()), Some(2));
    }

    #[test]
    fn clear_channel_keys_core_noops_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert_eq!(clear_channel_keys_core(&mut core), 0);
        assert!(core.status.contains("Select a joint"));
    }

    #[test]
    fn key_channel_here_inserts_the_channels_own_evaluated_value() {
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        };
        // The expectation is the engine's own evaluation,
        // read before keying — not hand-computed curve math.
        let expected = ch.value_at(5.0);
        key_channel_here(&mut ch, 5.0);
        assert_eq!(ch.keys.len(), 3);
        let keyed = ch.keys[1];
        assert_eq!(keyed.frame, 5.0);
        assert_eq!(keyed.value, expected, "the key stores the evaluated value");
        assert_eq!(
            ch.value_at(5.0),
            expected,
            "keying does not move the curve at the keyed frame"
        );
        // Keying a frame that already holds a key replaces
        // it (add_key semantics) instead of duplicating it.
        key_channel_here(&mut ch, 5.0);
        assert_eq!(ch.keys.len(), 3, "still one key at frame 5");
    }

    /// A core carrying three channels over figure 0 — an
    /// angle channel keyed at frames 0/4, a root channel
    /// already keyed at the target frame 2, and a
    /// step-interpolated length channel keyed at 0/4 — with
    /// the playhead on frame 2: the key-all fixture. The
    /// three targets evaluate differently at frame 2
    /// (Hermite midpoint, stored key, held step), which is
    /// what makes the fixture a real evaluation test.
    fn core_with_three_channels() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(4.0, 50.0)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                Key::flat(0.0, 100.0),
                Key::flat(2.0, 130.0),
                Key::flat(10.0, 200.0),
            ],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Length(2),
            keys: vec![
                Key::flat(0.0, 7.0).with_interp(Interp::Step),
                Key::flat(4.0, 42.0),
            ],
        });
        core.current = 2;
        core
    }

    #[test]
    fn key_all_here_keys_every_channel_at_its_evaluated_value_as_one_undo_step() {
        let mut core = core_with_three_channels();
        // Expectations come from the engine's own
        // evaluation, read before the op runs — never
        // hardcoded curve math.
        let frame = core.current as f32;
        let expected: Vec<f32> = core
            .project
            .channels
            .iter()
            .map(|ch| ch.value_at(frame))
            .collect();
        let counts_before: Vec<usize> =
            core.project.channels.iter().map(|ch| ch.keys.len()).collect();
        let edits_before = core.edits_since_autosave();

        assert_eq!(key_all_here_core(&mut core), 3);
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "exactly one snapshot for the whole operation"
        );
        assert_eq!(core.status, "Keyed 3 channels at frame 3.");

        for (i, ch) in core.project.channels.iter().enumerate() {
            let keyed: Vec<&Key> = ch
                .keys
                .iter()
                .filter(|k| (k.frame - frame).abs() <= 0.5)
                .collect();
            assert_eq!(keyed.len(), 1, "channel {i} holds one key at the frame");
            assert_eq!(
                keyed[0].value, expected[i],
                "channel {i} keyed its evaluated value"
            );
            assert_eq!(
                ch.value_at(frame),
                expected[i],
                "channel {i}'s curve is unmoved at the keyed frame"
            );
        }
        // The angle and length channels gained a key; the
        // root channel's existing key at frame 2 was
        // replaced, not duplicated.
        assert_eq!(core.project.channels[0].keys.len(), counts_before[0] + 1);
        assert_eq!(core.project.channels[1].keys.len(), counts_before[1]);
        assert_eq!(core.project.channels[2].keys.len(), counts_before[2] + 1);

        // One undo restores every channel at once.
        core.undo();
        let counts_after: Vec<usize> =
            core.project.channels.iter().map(|ch| ch.keys.len()).collect();
        assert_eq!(counts_after, counts_before, "undo reverts the whole key-all");
    }

    #[test]
    fn key_all_here_noops_without_channels_and_never_snapshots() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        let edits_before = core.edits_since_autosave();
        assert_eq!(key_all_here_core(&mut core), 0);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "no snapshot for the no-op"
        );
    }

    #[test]
    fn copy_key_in_channel_stamps_a_whole_key_copy_and_keeps_the_source() {
        let mut shaped = Key::flat(2.0, 30.0).with_interp(Interp::Step);
        shaped.in_tan = 1.5;
        shaped.out_tan = -2.0;
        let mut ch = Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), shaped],
        };
        let out = copy_key_in_channel(&mut ch, 2.0, COPY_KEY_OFFSET_FRAMES);
        assert_eq!(out, NudgeOutcome::Moved { from: 2.0, to: 12.0 });
        assert_eq!(ch.keys.len(), 3, "the source stays; the stamp is added");
        assert_eq!(ch.keys[1], shaped, "the source key is untouched");
        let stamped = ch.keys[2];
        assert_eq!(stamped.frame, 12.0);
        assert_eq!(
            (stamped.value, stamped.in_tan, stamped.out_tan),
            (30.0, 1.5, -2.0),
            "value and tangents travel with the copy"
        );
        assert_eq!(stamped.interp, Interp::Step, "interp mode travels too");
        // A refused stamp changes nothing at all: the
        // stamp's own landing spot now blocks a repeat.
        let before = ch.keys.clone();
        assert_eq!(
            copy_key_in_channel(&mut ch, 2.0, COPY_KEY_OFFSET_FRAMES),
            NudgeOutcome::Blocked { at: 12.0 }
        );
        assert_eq!(ch.keys, before, "the occupant survived the refused stamp");
    }

    #[test]
    fn copy_key_to_frame_core_stamps_ten_frames_on_as_one_undo_step() {
        let mut core = core_with_angle_keys(); // keys at 0 (10) and 2 (30)
        core.current = 0;
        let edits_before = core.edits_since_autosave();
        assert!(copy_key_to_frame_core(&mut core));
        assert_eq!(
            core.edits_since_autosave(),
            edits_before + 1,
            "exactly one snapshot"
        );
        assert!(
            core.status.contains("Copied graph key from frame 1 to frame 11"),
            "status reports the stamp: {}",
            core.status
        );
        let ch = selected_angle_channel(&core).unwrap();
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 2.0, 10.0], "source keys kept, stamp added");
        assert_eq!(ch.keys[2].value, 10.0, "the stamp copies the playhead key");
        core.undo();
        assert_eq!(
            selected_angle_channel(&core).unwrap().keys.len(),
            2,
            "undo removes the stamp"
        );
    }

    #[test]
    fn copy_key_to_frame_core_refuses_a_collision_without_snapshotting() {
        let mut core = core_with_angle_keys();
        core.project.channels[0].keys.push(Key::flat(10.0, 99.0));
        core.project.channels[0].sort_keys();
        core.current = 0;
        let edits_before = core.edits_since_autosave();
        assert!(!copy_key_to_frame_core(&mut core));
        assert!(
            core.status.contains("Frame 11 already has a key"),
            "status: {}",
            core.status
        );
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "a refused stamp never snapshots"
        );
        let ch = selected_angle_channel(&core).unwrap();
        assert_eq!(ch.keys.len(), 3, "nothing stamped and nothing destroyed");

        // A key 0.4 frames past the landing spot occupies
        // it too — the refusal window is the same 0.5-frame
        // window the nudge uses.
        let mut core = core_with_angle_keys();
        core.project.channels[0].keys.push(Key::flat(10.4, 99.0));
        core.project.channels[0].sort_keys();
        core.current = 0;
        assert!(!copy_key_to_frame_core(&mut core));
        assert!(
            core.status.contains("already has a key"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn copy_key_to_frame_core_noops_without_a_key_here() {
        let mut core = core_with_angle_keys();
        // Playhead on a frame with no key (keys sit at 0/2).
        core.current = 4;
        let edits_before = core.edits_since_autosave();
        assert!(!copy_key_to_frame_core(&mut core));
        assert!(
            core.status.contains("No key at frame 5 to copy"),
            "status: {}",
            core.status
        );
        assert_eq!(
            core.edits_since_autosave(),
            edits_before,
            "no snapshot for the refusal"
        );
        // No channel on the selected joint.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        assert!(!copy_key_to_frame_core(&mut bare));
        assert!(bare.status.contains("No graph keys"));
        // No joint selected at all.
        bare.selected_node = None;
        assert!(!copy_key_to_frame_core(&mut bare));
        assert!(bare.status.contains("Select a joint"));
    }
}
