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
//!
//! Phase H (section 18/40) puts Font Awesome glyphs on four
//! of this section's controls through `icon_btn`: "Key all
//! here" (`key`), "Copy key +10" (`copy`) and "Clear
//! channel keys" (`xmark`) render as icon+label buttons,
//! and the nudge pair renders as icon-only chevrons
//! (`chevron-left` / `chevron-right`). Keys, labels and
//! handlers are unchanged. One honest difference from the
//! old `kbtn` rendering of the stamp row: those two
//! buttons used to go inert (no handler) when there was
//! nothing to act on; `icon_btn` has no inert mode, so
//! they now stay wired and the press lands on the ops'
//! own status-only refusals ("No graph channels to
//! key." / the copy refusals) — still snapshot-free,
//! exactly as the ops were already written.
//!
//! Phase I (section 26/40) closes that difference: the
//! five icon controls Phase H converted — Clear channel
//! keys, the nudge pair, Key all here and Copy key +10 —
//! render through `icon_btn_inert` / `icon_only_btn_inert`
//! exactly when their op would refuse, decided by the
//! pure predicates `clear_channel_enabled`,
//! `nudge_enabled`, `key_all_enabled` and
//! `copy_key_enabled` below. The nudge and stamp guards
//! plan through the ops' own `nudge_plan`, so the tint
//! can never disagree with the press. The plain `btn`
//! controls (Key angle here, Delete key here, Auto
//! tangents, the five presets) never carried an inert
//! tint and stay wired — their refusals remain
//! status-only — and Copy/Paste keys keep the muting
//! `kbtn` already gave them.
//!
//! Phase J (section 9/40) fixes a layout overlap from
//! the Phase I headless screenshot: with no joint
//! selected, the "No joint selected" note was a bare
//! text child whose painted line could run across the
//! Delete key button. The note now renders through
//! [`no_joint_note_row`] — its own full-width row above
//! the button rows, its text in Rune's truncating
//! one-line mode — and only in the no-joint state. When
//! a joint IS selected the section keeps the original
//! bare target line in the same slot, so the
//! selected-state layout is unchanged.
//!
//! Phase K (section 31/40) adds one more icon control
//! in the same live/inert discipline: "Clear keys in
//! loop range" (`key-clear-range`, `eraser` glyph), in
//! its own row under the clipboard row. It clears the
//! selected joint's angle keys whose frames fall inside
//! the loop range, through the library's
//! `graph_keys::delete_keys_in_range` — the op owns the
//! single snapshot and the status line. The channel is
//! resolved exactly as every other op in this section
//! resolves it: the panel computes the pair itself,
//! `(selected_figure, Angle(selected_node))` (see
//! [`selected_angle_channel`]); nothing is delegated to
//! a core method. The guard is [`clear_range_enabled`]:
//! a loop range is set AND the resolved channel holds
//! at least one key inside it (counted read-only by
//! `graph_keys::keys_in_range`).
//!
//! Phase L (section 32/40) adds one more icon control
//! in the same live/inert discipline: "Cycle interp"
//! (`key-interp`, `bezier-curve` glyph), in its own row
//! under the clear-range row. It cycles the selected
//! joint's angle key at the current frame through the
//! [`Interp`] modes — Hermite → Linear → Step →
//! Hermite — through the library's
//! `graph_keys::set_key_interp`; the current mode is
//! read back through `graph_keys::key_interp_at`, and
//! the live button's label names it ("Interp:
//! Hermite"), so the control says what the key IS, not
//! what a press would make it. The guard is
//! [`interp_cycle_enabled`]: a joint is selected, its
//! angle channel resolves, and a key sits at the
//! playhead.
//!
//! Phase M (section 23/40) adds one more icon control
//! in the same live/inert discipline: "Fill range"
//! (`key-fill-range`, `grip-vertical` glyph — the icon
//! table carries no plain `grip` and no `ellipsis`),
//! in its own row under the clear-range row. It fills
//! the loop range's keyless frames on the selected
//! joint's angle channel with copies of the key at the
//! current frame, through the library's
//! `graph_keys::fill_keys_in_range` — the op owns the
//! single snapshot and the status line. The channel is
//! resolved exactly as Cycle interp resolves it:
//! [`selected_angle_channel_index`], the
//! `graph_keys::channel_index_of` lookup. The guard is
//! [`fill_range_enabled`]: a loop range is set AND the
//! resolved channel holds a key at the current frame,
//! read through [`current_key_interp`] — the interp
//! guard's own playhead-key test — so the two buttons
//! can never disagree about which key is "at" the
//! playhead.
//!
//! Phase N (adoption) restyles the section to the
//! charter's card language: the root is the kit
//! `design::panel_col` under the same `keyframe-tools`
//! key, the header is `design::panel_title` over a
//! hairline `design::divider`, and the local `kbtn`
//! clipboard-button dialect is deleted. Copy/Paste keys
//! now dispatch on their unchanged guards to the shared
//! family — `chrome_view::btn` when the guard holds,
//! `chrome_view::btn_inert` when it does not — which is
//! exactly the live/inert split `kbtn` itself drew: a
//! disabled `kbtn` carried NO press handler, so no
//! press behaviour changes, and the guards were never
//! refusal-status sites (that is the `btn` row above
//! them). The one visual standardisation is the inert
//! fill: `control_off` → the family's card fill, the
//! same delta the icon inert twins recorded in Phase I.
//! Keys, labels, guards and handlers are byte-identical.
//!
//! UI Mission Wave 1 (overflow repairs; audit S12-03…05)
//! fixes the three rows of this section that crossed the
//! right column's 316 pt content budget. The presets row
//! measured 361 pt and painted 17 pt past the window edge;
//! it now wraps (`flex_wrap`, the graph/frame-ops
//! medicine), so Linear drops to a second line. The
//! actions row (327.5 pt) splits into two lines: Key angle
//! here + Delete key here keep `keyframe-actions`, Auto
//! tangents moves to its own `keyframe-auto` row beneath.
//! And Clear channel keys moves out of the clipboard row
//! (318.8 pt) into its own `keyframe-clear-channel` row
//! directly above the clear-range row — the one-control
//! pattern the clear-range and fill-range rows already
//! use — leaving Copy + Paste at ≈162 pt. The two new keys
//! are container rows only; every button key, label, guard
//! and handler is byte-identical.
//!
//! UI mission wave 4 (audit S12-09 / S12-10): the preset
//! row's bare text buttons become the kit's
//! [`design::tangent_preset_chip`] — the graph panel's
//! drawn-preview chip construction, lifted into the kit —
//! under the same `key-preset-*` keys, labels and
//! `apply_graph_ease_preset` handlers; presets are
//! one-shot actions, so the chips never show active.
//! And the two destructive clears (`key-clear-channel`,
//! `key-clear-range`) wear the danger family in their
//! live form (`danger_icon_btn`, the Phase N reds) —
//! keys, labels, guards and handlers unchanged; the
//! inert forms stay `icon_btn_inert`, the handler-free
//! muted state, as before.

use rune::tok::{type_, Theme};
use rune::{row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Interp, Key, TangentPreset};
use stykanimate_rs::graph_keys;

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{
    danger_icon_btn, icon_btn, icon_btn_inert, icon_only_btn, icon_only_btn_inert,
};
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

/// Whether planning a move of the selected channel's
/// playhead key by `delta` frames lands — the shared
/// render-time guard behind [`nudge_enabled`] and
/// [`copy_key_enabled`]. It runs the ops' own
/// [`nudge_plan`] over the selected angle channel, so it
/// is false for exactly the ops' refusal set: no joint
/// selected, no channel on the joint, no key at the
/// playhead, a key within 0.5 frames of the landing spot,
/// or a landing spot before frame 0. Pure: reads only.
fn plan_would_move(core: &EditorCore, delta: i32) -> bool {
    match selected_angle_channel(core) {
        Some(ch) => matches!(
            nudge_plan(&ch.keys, core.current as f32, delta),
            NudgeOutcome::Moved { .. }
        ),
        None => false,
    }
}

/// Whether the Nudge button for `delta` (−1 left, +1
/// right) would move a key — the render-time twin of
/// [`nudge_key_here_core`]'s refusals. When this is
/// false the button renders inert (`icon_only_btn_inert`)
/// instead of landing on the op's status-only refusal.
pub fn nudge_enabled(core: &EditorCore, delta: i32) -> bool {
    plan_would_move(core, delta)
}

/// Whether Copy key +10 would stamp a key — the
/// render-time twin of [`copy_key_to_frame_core`]'s
/// refusals, planned through the same [`nudge_plan`]
/// with the stamp offset as its delta, collision at the
/// landing spot included. When this is false the button
/// renders inert (`icon_btn_inert`).
pub fn copy_key_enabled(core: &EditorCore) -> bool {
    plan_would_move(core, COPY_KEY_OFFSET_FRAMES)
}

/// Whether Key all here would key anything — its op's
/// only refusal is a project with no graph channels, so
/// the guard is the channel count. No selection is
/// needed: keying everything is the point. When this is
/// false the button renders inert (`icon_btn_inert`).
pub fn key_all_enabled(core: &EditorCore) -> bool {
    !core.project.channels.is_empty()
}

/// Whether Clear channel keys would remove anything —
/// the render-time twin of
/// [`clear_channel_keys_core`]'s refusal set: a joint is
/// selected, its angle channel exists, and the channel
/// holds at least one key. When this is false the
/// button renders inert (`icon_btn_inert`).
pub fn clear_channel_enabled(core: &EditorCore) -> bool {
    selected_angle_channel(core).is_some_and(|ch| !ch.keys.is_empty())
}

/// Whether Clear keys in loop range would remove
/// anything — the render-time twin of
/// [`clear_keys_in_range_core`]'s refusal set: a joint
/// is selected, a loop range is set, and the selected
/// figure's `Angle(selected_node)` channel holds at
/// least one key inside the range, counted by the
/// library's read-only `graph_keys::keys_in_range` over
/// the same `(figure, target)` pair every op in this
/// section resolves. When this is false the button
/// renders inert (`icon_btn_inert`). Pure: reads only.
pub fn clear_range_enabled(core: &EditorCore) -> bool {
    let Some(ni) = core.selected_node else {
        return false;
    };
    let Some((a, b)) = core.loop_range else {
        return false;
    };
    graph_keys::keys_in_range(
        core,
        core.selected_figure,
        ChannelTarget::Angle(ni),
        a as f32,
        b as f32,
    ) > 0
}

/// Whether Fill range can act — the render-time guard
/// beside [`fill_keys_in_range_core`]: a loop range is
/// set, a joint is selected, its angle channel
/// resolves, and the channel holds a key at the current
/// frame to fill FROM. The playhead-key half is read
/// through [`current_key_interp`] — the interp guard's
/// own test, over the same
/// [`selected_angle_channel_index`] resolution and the
/// library's nearest-key-within-0.5 lookup
/// (`graph_keys::key_interp_at`) — so this guard and
/// [`interp_cycle_enabled`] can never disagree about
/// whether a key sits at the playhead. One refusal of
/// the op is deliberately NOT read here: a range whose
/// frames are all keyed already. That state depends on
/// the whole range's contents; in it the button stays
/// live and the press lands on the op's own
/// status-only refusal, snapshot-free exactly as the
/// op is written (the Phase H precedent: a wired press
/// on an op's own refusal). When this is false the
/// button renders inert (`icon_btn_inert`). Pure:
/// reads only.
pub fn fill_range_enabled(core: &EditorCore) -> bool {
    core.loop_range.is_some() && current_key_interp(core).is_some()
}

/// The display name of an [`Interp`] mode — the same
/// words the graph panel's interp buttons carry
/// ("Hermite" / "Linear" / "Step").
pub fn interp_name(interp: Interp) -> &'static str {
    match interp {
        Interp::Hermite => "Hermite",
        Interp::Linear => "Linear",
        Interp::Step => "Step",
    }
}

/// The next [`Interp`] mode in the cycle the
/// `key-interp` button walks: Hermite → Linear → Step →
/// Hermite.
pub fn next_interp(interp: Interp) -> Interp {
    match interp {
        Interp::Hermite => Interp::Linear,
        Interp::Linear => Interp::Step,
        Interp::Step => Interp::Hermite,
    }
}

/// The interp mode of the selected angle channel's key
/// at the current frame, read through the library's
/// `graph_keys::key_interp_at` over the channel
/// [`selected_angle_channel_index`] resolves. `None`
/// when no joint is selected, no channel exists on the
/// joint, or no key sits at the playhead. Pure: reads
/// only.
pub fn current_key_interp(core: &EditorCore) -> Option<Interp> {
    let ci = selected_angle_channel_index(core)?;
    graph_keys::key_interp_at(core, ci, core.current as f32)
}

/// Whether Cycle interp would change anything — the
/// render-time twin of [`cycle_key_interp_core`]'s
/// refusal set: a joint is selected, its angle channel
/// exists, and the channel holds a key at the current
/// frame. When this is false the button renders inert
/// (`icon_btn_inert`). Pure: reads only.
pub fn interp_cycle_enabled(core: &EditorCore) -> bool {
    current_key_interp(core).is_some()
}

/// The `key-interp` button's label: "Interp: {current}"
/// while a key sits at the playhead — naming the mode
/// the key currently holds, the one a press cycles
/// away from — and the bare "Interp" in the inert
/// state, where there is no current mode to name.
pub fn interp_button_label(core: &EditorCore) -> String {
    match current_key_interp(core) {
        Some(interp) => format!("Interp: {}", interp_name(interp)),
        None => "Interp".to_string(),
    }
}

/// The tangent presets: every `TangentPreset` variant,
/// in menu order, as (preset, button label, element-key
/// suffix). The row renders them through the kit's
/// [`design::tangent_preset_chip`] (UI mission wave 4,
/// audit S12-09) — the drawn-preview chip construction
/// the graph panel's chips canonicalised — under these
/// same `key-preset-*` keys and labels.
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

/// Clear the selected angle channel's keys whose frames
/// lie inside the loop range; returns how many keys were
/// removed. The removal itself is the library's
/// `graph_keys::delete_keys_in_range` over this
/// section's own `(selected_figure, Angle(selected_node))`
/// resolution — it takes the single snapshot and sets
/// the status line. The two refusals the library cannot
/// phrase — no joint selected, no loop range set — are
/// answered here, status-only with NO snapshot, matching
/// this section's discipline; the button renders inert
/// in exactly those states (see [`clear_range_enabled`]),
/// so a press can never reach them.
pub fn clear_keys_in_range_core(core: &mut EditorCore) -> usize {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Clear keys in range.".into();
        return 0;
    };
    let Some((a, b)) = core.loop_range else {
        core.status = "Set a loop range first, then Clear keys in range.".into();
        return 0;
    };
    graph_keys::delete_keys_in_range(
        core,
        core.selected_figure,
        ChannelTarget::Angle(ni),
        a as f32,
        b as f32,
    )
}

/// `State` wrapper for [`clear_keys_in_range_core`].
pub fn clear_keys_in_range_here(s: &mut State) {
    clear_keys_in_range_core(&mut s.core);
}

/// Fill the loop range's keyless frames on the
/// selected angle channel with copies of the key at
/// the current frame; returns how many keys were
/// inserted. The fill itself is the library's
/// `graph_keys::fill_keys_in_range` over the channel
/// [`selected_angle_channel_index`] resolves — the
/// resolution Cycle interp uses — and the range is the
/// core's `loop_range` pair exactly as it stands (the
/// engine clamps it when set; Clear keys in range
/// trusts it the same way). The library takes the
/// single snapshot and sets the status line, including
/// for its own nothing-to-fill refusals. The refusals
/// the library cannot phrase for this panel — no joint
/// selected, no loop range set, no channel on the
/// joint — are answered here, status-only with NO
/// snapshot, matching this section's discipline; the
/// button renders inert in the joint/range states (see
/// [`fill_range_enabled`]), so a press can never reach
/// them.
pub fn fill_keys_in_range_core(core: &mut EditorCore) -> usize {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Fill range.".into();
        return 0;
    };
    let Some((a, b)) = core.loop_range else {
        core.status = "Set a loop range first, then Fill range.".into();
        return 0;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {} to fill from.", fi + 1, ni);
        return 0;
    };
    graph_keys::fill_keys_in_range(core, ci, a, b)
}

/// `State` wrapper for [`fill_keys_in_range_core`].
pub fn fill_keys_in_range_here(s: &mut State) {
    fill_keys_in_range_core(&mut s.core);
}

/// Cycle the interp mode of the selected angle
/// channel's key at the current frame — Hermite →
/// Linear → Step → Hermite (see [`next_interp`]) — and
/// return whether the key's mode changed. The write
/// itself is the library's `graph_keys::set_key_interp`
/// over the channel [`selected_angle_channel_index`]
/// resolves: it owns the single snapshot and the status
/// line. The key is found with this section's own
/// [`key_index_near`] — the nearest key within half a
/// frame, the window [`keyed_here`] reads by — and the
/// current mode through `graph_keys::key_interp_at`,
/// so the guard ([`interp_cycle_enabled`]) and the op
/// agree on which key is "at" the playhead. The
/// refusals the library cannot phrase — no joint
/// selected, no channel on the joint, no key at the
/// playhead — are answered here, status-only with NO
/// snapshot, matching this section's discipline; the
/// button renders inert in exactly those states, so a
/// press can never reach them.
pub fn cycle_key_interp_core(core: &mut EditorCore) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Cycle interp.".into();
        return false;
    };
    let fi = core.selected_figure;
    let Some(ci) = selected_angle_channel_index(core) else {
        core.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
        return false;
    };
    let Some(current) = graph_keys::key_interp_at(core, ci, core.current as f32) else {
        core.status = format!("No key at frame {} to cycle.", core.current + 1);
        return false;
    };
    let Some(ki) = key_index_near(&core.project.channels[ci].keys, core.current as f32) else {
        core.status = format!("No key at frame {} to cycle.", core.current + 1);
        return false;
    };
    graph_keys::set_key_interp(core, ci, ki, next_interp(current))
}

/// `State` wrapper for [`cycle_key_interp_core`].
pub fn cycle_key_interp_here(s: &mut State) {
    cycle_key_interp_core(&mut s.core);
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

/// Element key of the dedicated no-joint note row
/// (Phase J). The note lives under this key, in its own
/// row — never inside the actions row below.
pub const NO_JOINT_NOTE_ROW_KEY: &str = "keyframe-no-joint-note";

/// Element key of the row holding Key angle here /
/// Delete key here. (Auto tangents shared this row
/// until UI Mission Wave 1 split it onto its own
/// `keyframe-auto` row — see the header note.) Kept as
/// a constant so the layout tests can prove the
/// no-joint note row and the Delete button's row are
/// different rows; the value is the key the row has
/// always carried.
pub const ACTIONS_ROW_KEY: &str = "keyframe-actions";

/// Element key of the Delete key button itself — the
/// value it has always carried, named so the layout
/// tests can pin that the fix did not rename it.
pub const DELETE_KEY_BUTTON_KEY: &str = "key-delete";

/// The note shown when no joint is selected, or `None`
/// when one is. The text is [`target_text`]'s no-joint
/// wording, unchanged — Phase J changes where the note
/// is laid out, not what it says.
pub fn no_joint_note(core: &EditorCore) -> Option<&'static str> {
    match core.selected_node {
        None => Some("No joint selected"),
        Some(_) => None,
    }
}

/// The no-joint note as its OWN row (Phase J): a
/// full-width row holding one muted text in Rune's
/// truncating one-line mode (`.grow(1.0).truncate()` —
/// the status strip's tail-segment discipline), so the
/// note's painted line is confined to its own box and
/// elides with "…" under width pressure instead of
/// running across the Delete key button below. Returns
/// `None` when a joint is selected: the caller then
/// renders the original bare target line, keeping the
/// selected-state layout exactly as it was.
pub fn no_joint_note_row(core: &EditorCore, t: &Theme) -> Option<El> {
    no_joint_note(core).map(|note| {
        row()
            .key(NO_JOINT_NOTE_ROW_KEY)
            .w_full()
            .child(
                text(note)
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .grow(1.0)
                    .truncate(),
            )
    })
}

/// The keyframe tools section: header, selection target,
/// keyed-frame indicator, the key/delete buttons and the
/// auto-tangents row beneath them, the copy/paste-keys
/// clipboard row, the clear-channel row, the clear-range
/// row, the fill-range row, the interp cycle row, the
/// nudge row, the key-all / copy-key stamp row,
/// and the five tangent preset buttons (scoped by the op
/// to the selected figure). Buttons highlight with real
/// state: Delete is accent-filled exactly when a key sits
/// at this frame; Copy is muted until a channel exists to
/// copy from, and Paste is muted while the key clipboard
/// is empty. The icon controls (Clear channel keys,
/// Clear keys in loop range, Fill range, Cycle interp, the nudge pair, Key all
/// here, Copy key +10) render
/// inert — muted, handler-free — exactly when their op
/// would refuse, via the `*_enabled` predicates above
/// (Phases I, K, L and M); the plain text buttons stay wired in every
/// state and land on their ops' status-only refusals.
/// The target slot under the header is state-dependent
/// (Phase J): with a joint selected it is the original
/// bare target line; with none it is the dedicated
/// [`no_joint_note_row`], so the note can never paint
/// across the actions row beneath it.
pub fn keyframe_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let keyed = selected_angle_channel(core).is_some_and(|ch| keyed_here(&ch.keys, core.current));
    let interp_label = interp_button_label(core);
    let mut presets = row().key("keyframe-presets").gap(6.0).flex_wrap();
    for (preset, label, suffix) in preset_options() {
        presets = presets.child(design::tangent_preset_chip(
            &format!("key-preset-{suffix}"),
            preset,
            label,
            false,
            t,
            move |s: &mut State| s.core.apply_graph_ease_preset(preset),
        ));
    }
    design::panel_col("keyframe-tools", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Keyframes", None, t)),
        )
        .child(design::divider(t))
        .child(
            // The target slot (Phase J): the no-joint
            // note gets its own row; a selected joint
            // keeps the bare target line it has always
            // had, in the same slot.
            if let Some(note_row) = no_joint_note_row(core, t) {
                note_row
            } else {
                text(target_text(core))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
            },
        )
        .child(
            text(indicator_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .key(ACTIONS_ROW_KEY)
                .gap(6.0)
                .child(btn("key-angle", "Key angle here", false, t, |s| {
                    s.core.key_graph_angle()
                }))
                .child(btn(DELETE_KEY_BUTTON_KEY, "Delete key here", keyed, t, |s| {
                    delete_key_here(s)
                })),
        )
        .child(
            row()
                .key("keyframe-auto")
                .gap(6.0)
                .child(btn("key-auto", "Auto tangents", false, t, |s| {
                    s.core.smooth_graph_tangents()
                })),
        )
        .child(
            row()
                .key("keyframe-clipboard")
                .gap(6.0)
                .child(if selected_angle_channel_index(core).is_some() {
                    btn("key-copy", "Copy keys", false, t, |s| copy_keys_here(s))
                } else {
                    btn_inert("key-copy", "Copy keys", t)
                })
                .child(if s.key_clipboard.as_ref().is_some_and(|k| !k.is_empty()) {
                    btn("key-paste", "Paste keys", false, t, |s| {
                        paste_keys_here(s)
                    })
                } else {
                    btn_inert("key-paste", "Paste keys", t)
                }),
        )
        .child(
            row()
                .key("keyframe-clear-channel")
                .gap(6.0)
                .child(if clear_channel_enabled(core) {
                    danger_icon_btn(
                        "key-clear-channel",
                        "xmark",
                        "Clear channel keys",
                        false,
                        t,
                        |s| clear_channel_keys_here(s),
                    )
                } else {
                    icon_btn_inert("key-clear-channel", "xmark", "Clear channel keys", t)
                }),
        )
        .child(
            row()
                .key("keyframe-clear-range")
                .gap(6.0)
                .child(if clear_range_enabled(core) {
                    danger_icon_btn(
                        "key-clear-range",
                        "eraser",
                        "Clear keys in loop range",
                        false,
                        t,
                        |s| clear_keys_in_range_here(s),
                    )
                } else {
                    icon_btn_inert("key-clear-range", "eraser", "Clear keys in loop range", t)
                }),
        )
        .child(
            row()
                .key("keyframe-fill-range")
                .gap(6.0)
                .child(if fill_range_enabled(core) {
                    icon_btn(
                        "key-fill-range",
                        "grip-vertical",
                        "Fill range",
                        false,
                        t,
                        |s| fill_keys_in_range_here(s),
                    )
                } else {
                    icon_btn_inert("key-fill-range", "grip-vertical", "Fill range", t)
                }),
        )
        .child(
            row()
                .key("keyframe-interp")
                .gap(6.0)
                .child(if interp_cycle_enabled(core) {
                    icon_btn(
                        "key-interp",
                        "bezier-curve",
                        &interp_label,
                        false,
                        t,
                        |s| cycle_key_interp_here(s),
                    )
                } else {
                    icon_btn_inert("key-interp", "bezier-curve", "Interp", t)
                }),
        )
        .child(
            row()
                .key("keyframe-nudge")
                .gap(6.0)
                .child(if nudge_enabled(core, -1) {
                    icon_only_btn("key-nudge-l", "chevron-left", "Nudge key earlier", 26.0, false, t, |s| {
                        nudge_key_here(s, -1)
                    })
                } else {
                    icon_only_btn_inert("key-nudge-l", "chevron-left", "Nudge key earlier", 26.0, t)
                })
                .child(if nudge_enabled(core, 1) {
                    icon_only_btn("key-nudge-r", "chevron-right", "Nudge key later", 26.0, false, t, |s| {
                        nudge_key_here(s, 1)
                    })
                } else {
                    icon_only_btn_inert("key-nudge-r", "chevron-right", "Nudge key later", 26.0, t)
                }),
        )
        .child(
            row()
                .key("keyframe-stamp")
                .gap(6.0)
                .child(if key_all_enabled(core) {
                    icon_btn("key-all-here", "key", "Key all here", false, t, |s| {
                        key_all_here(s)
                    })
                } else {
                    icon_btn_inert("key-all-here", "key", "Key all here", t)
                })
                .child(if copy_key_enabled(core) {
                    icon_btn(
                        "key-copy-to-frame",
                        "copy",
                        "Copy key +10",
                        false,
                        t,
                        |s| copy_key_to_frame(s),
                    )
                } else {
                    icon_btn_inert("key-copy-to-frame", "copy", "Copy key +10", t)
                }),
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

    #[test]
    fn key_all_enabled_follows_the_project_channel_count() {
        // No channels at all: the op's only refusal, so
        // the button is inert.
        let mut core = EditorCore::new();
        core.project.channels.clear();
        assert!(!key_all_enabled(&core));
        // Any channel makes it live, with no selection
        // required — keying everything is the point.
        let mut core = core_with_angle_keys();
        assert!(key_all_enabled(&core));
        core.selected_node = None;
        assert!(key_all_enabled(&core), "key-all needs no joint");
    }

    #[test]
    fn clear_channel_enabled_needs_a_selected_channel_with_keys() {
        // Channel with keys, joint selected: live.
        let mut core = core_with_angle_keys();
        assert!(clear_channel_enabled(&core));
        // The channel emptied: the op would refuse, so
        // the button goes inert.
        core.project.channels[0].keys.clear();
        assert!(!clear_channel_enabled(&core));
        // No joint selected: inert.
        let mut core = core_with_angle_keys();
        core.selected_node = None;
        assert!(!clear_channel_enabled(&core));
        // A joint with no channel on it: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        assert!(!clear_channel_enabled(&bare));
    }

    /// A core whose figure 0 / joint 1 angle channel holds
    /// flat keys at frames 0, 2 and 4, joint 1 selected —
    /// the clear-range fixture: the keys straddle the loop
    /// ranges the guard and handler tests set.
    fn core_with_spread_angle_keys() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![
                Key::flat(0.0, 10.0),
                Key::flat(2.0, 30.0),
                Key::flat(4.0, 50.0),
            ],
        });
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core
    }

    #[test]
    fn clear_range_enabled_needs_a_loop_range_with_keys_inside() {
        // No loop range set: inert, however many keys the
        // channel holds.
        let core = core_with_spread_angle_keys();
        assert!(!clear_range_enabled(&core));
        // A loop range holding the frame-2 key: live.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        assert!(clear_range_enabled(&core));
        // A loop range over a keyless stretch — the keys
        // sit only OUTSIDE it, at 0, 2 and 4: inert.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 1));
        assert!(!clear_range_enabled(&core));
        // The range is inclusive: a key on its edge counts.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((2, 2));
        assert!(clear_range_enabled(&core));
        // No joint selected: the channel cannot be
        // resolved, so inert even with range and keys.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.selected_node = None;
        assert!(!clear_range_enabled(&core));
        // A joint with no channel on it: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        bare.loop_range = Some((0, 4));
        assert!(!clear_range_enabled(&bare));
    }

    #[test]
    fn clear_keys_in_range_core_removes_only_the_in_range_keys_of_the_selected_channel() {
        let mut core = core_with_spread_angle_keys();
        // A bystander channel on another joint, holding a
        // key INSIDE the same range: it must survive.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(2),
            keys: vec![Key::flat(2.0, 99.0)],
        });
        core.loop_range = Some((1, 3));
        let depth = core.undo_depth();
        assert_eq!(clear_keys_in_range_core(&mut core), 1);
        let ch = selected_angle_channel(&core).expect("channel survives a range clear");
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 4.0], "only the in-range key is gone");
        assert_eq!(
            core.project.channels[1].keys.len(),
            1,
            "the other channel is untouched"
        );
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert!(
            core.status.contains("Cleared 1 key(s)"),
            "the library owns the status: {}",
            core.status
        );
        // Nothing left inside the range: the guard drops,
        // and a direct call is a snapshot-free no-op.
        assert!(!clear_range_enabled(&core));
        assert_eq!(clear_keys_in_range_core(&mut core), 0);
        assert_eq!(core.undo_depth(), depth + 1, "no second snapshot");
    }

    #[test]
    fn clear_keys_in_range_core_refuses_without_a_joint_or_a_loop_range() {
        // No loop range: status-only, no snapshot, keys intact.
        let mut core = core_with_spread_angle_keys();
        let depth = core.undo_depth();
        assert_eq!(clear_keys_in_range_core(&mut core), 0);
        assert!(core.status.contains("loop range"));
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(
            selected_angle_channel(&core).map(|ch| ch.keys.len()),
            Some(3)
        );
        // No joint selected: the same discipline.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.selected_node = None;
        let depth = core.undo_depth();
        assert_eq!(clear_keys_in_range_core(&mut core), 0);
        assert!(core.status.contains("Select a joint"));
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn interp_cycle_enabled_needs_a_key_at_the_playhead() {
        // Keys at 0, 2 and 4 on the selected joint's
        // channel: live exactly on those frames.
        let mut core = core_with_spread_angle_keys();
        core.current = 2;
        assert!(interp_cycle_enabled(&core));
        core.current = 0;
        assert!(interp_cycle_enabled(&core));
        core.current = 4;
        assert!(interp_cycle_enabled(&core));
        // A bare frame between keys: no key at the
        // playhead, so inert.
        core.current = 1;
        assert!(!interp_cycle_enabled(&core));
        core.current = 3;
        assert!(!interp_cycle_enabled(&core));
        // No joint selected: the channel cannot be
        // resolved, so inert even on a keyed frame.
        core.current = 2;
        core.selected_node = None;
        assert!(!interp_cycle_enabled(&core));
        // A joint with no channel on it: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        bare.current = 0;
        assert!(!interp_cycle_enabled(&bare));
    }

    #[test]
    fn cycle_key_interp_core_walks_the_whole_cycle_on_the_playhead_key() {
        let mut core = core_with_spread_angle_keys(); // keys at 0, 2, 4, all Hermite
        core.current = 2;
        assert_eq!(current_key_interp(&core), Some(Interp::Hermite));
        assert_eq!(interp_button_label(&core), "Interp: Hermite");
        let depth = core.undo_depth();
        // Hermite → Linear: the mode lands on the frame-2
        // key only, the library's status names the new
        // mode, and the press is exactly one snapshot.
        assert!(cycle_key_interp_core(&mut core));
        assert_eq!(current_key_interp(&core), Some(Interp::Linear));
        assert_eq!(interp_button_label(&core), "Interp: Linear");
        assert!(
            core.status.contains("Linear"),
            "the library owns the status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        let ch = selected_angle_channel(&core).expect("channel survives an interp cycle");
        assert_eq!(ch.keys[0].interp, Interp::Hermite, "the frame-0 key is untouched");
        assert_eq!(ch.keys[2].interp, Interp::Hermite, "the frame-4 key is untouched");
        // Linear → Step → Hermite: the cycle closes.
        assert!(cycle_key_interp_core(&mut core));
        assert_eq!(current_key_interp(&core), Some(Interp::Step));
        assert!(cycle_key_interp_core(&mut core));
        assert_eq!(current_key_interp(&core), Some(Interp::Hermite));
        assert_eq!(core.undo_depth(), depth + 3, "one snapshot per press");
    }

    #[test]
    fn cycle_key_interp_core_refuses_without_a_joint_channel_or_key() {
        // No key at the playhead: status-only, no
        // snapshot, modes intact.
        let mut core = core_with_spread_angle_keys();
        core.current = 1;
        let depth = core.undo_depth();
        assert!(!cycle_key_interp_core(&mut core));
        assert!(core.status.contains("No key at frame"));
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(
            selected_angle_channel(&core).map(|ch| ch.keys[1].interp),
            Some(Interp::Hermite)
        );
        // No joint selected: the same discipline.
        let mut core = core_with_spread_angle_keys();
        core.current = 2;
        core.selected_node = None;
        let depth = core.undo_depth();
        assert!(!cycle_key_interp_core(&mut core));
        assert!(core.status.contains("Select a joint"));
        assert_eq!(core.undo_depth(), depth);
        // A joint with no channel on it: refused, no
        // snapshot.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        let depth = bare.undo_depth();
        assert!(!cycle_key_interp_core(&mut bare));
        assert!(bare.status.contains("No graph keys"));
        assert_eq!(bare.undo_depth(), depth);
    }

    #[test]
    fn interp_row_builds_live_and_inert_with_matching_labels() {
        // `El` has no public tree-walk, so — the file's
        // established pattern — the row is asserted
        // through the helpers and label the builder
        // itself uses, plus a full section build in each
        // state.
        let t = rune::tok::theme(true);
        // No key at the playhead: the inert form, under
        // the bare label.
        let mut off = core_with_spread_angle_keys();
        off.current = 1;
        assert!(!interp_cycle_enabled(&off));
        assert_eq!(interp_button_label(&off), "Interp");
        let _ = keyframe_section(&state_with(off), &t);
        // A key at the playhead: the live form, its
        // label naming the key's current mode.
        let mut on = core_with_spread_angle_keys();
        on.current = 2;
        assert!(interp_cycle_enabled(&on));
        assert_eq!(interp_button_label(&on), "Interp: Hermite");
        let _ = keyframe_section(&state_with(on), &t);
    }

    #[test]
    fn fill_range_enabled_needs_a_loop_range_and_a_key_at_the_playhead() {
        // Loop range set, playhead on the frame-2 key of
        // the spread fixture (keys at 0, 2 and 4): live.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.current = 2;
        assert!(fill_range_enabled(&core));
        // No loop range: inert, even on a keyed frame.
        let mut core = core_with_spread_angle_keys();
        core.current = 2;
        assert!(!fill_range_enabled(&core));
        // Loop range set, playhead on the bare frames
        // between keys: nothing to fill FROM, so inert.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.current = 1;
        assert!(!fill_range_enabled(&core));
        core.current = 3;
        assert!(!fill_range_enabled(&core));
        // No joint selected: the channel cannot be
        // resolved, so inert even with range and key.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.current = 2;
        core.selected_node = None;
        assert!(!fill_range_enabled(&core));
        // A joint with no channel on it: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        bare.loop_range = Some((0, 4));
        assert!(!fill_range_enabled(&bare));
    }

    #[test]
    fn fill_range_guard_agrees_with_the_interp_guard_on_the_playhead_key() {
        // Both guards read "a key sits at the playhead on
        // the resolved channel" through the same helper
        // (`current_key_interp`), so with a loop range set
        // they agree on every frame of the spread fixture
        // — the resolution-parity pin the fill button's
        // wiring rests on.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((0, 4));
        for frame in 0..=4 {
            core.current = frame;
            assert_eq!(
                fill_range_enabled(&core),
                interp_cycle_enabled(&core),
                "frame {}: the two guards read the playhead key alike",
                frame + 1
            );
        }
    }

    #[test]
    fn fill_keys_in_range_core_fills_the_selected_channel_only() {
        let mut core = core_with_spread_angle_keys(); // Angle(1) keys at 0 (10), 2 (30), 4 (50)
        // A bystander channel on another joint, keyed
        // inside the same range: resolution must leave it
        // alone — the fill acts on the channel
        // `selected_angle_channel_index` resolves, the one
        // Cycle interp reads.
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(2),
            keys: vec![Key::flat(2.0, 99.0)],
        });
        core.loop_range = Some((1, 3));
        core.current = 2;
        let depth = core.undo_depth();
        assert_eq!(fill_keys_in_range_core(&mut core), 2);
        let ch = selected_angle_channel(&core).expect("channel survives a fill");
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 1.0, 2.0, 3.0, 4.0], "frames 1 and 3 filled");
        assert_eq!(ch.keys[1].value, 30.0, "the fill copies the playhead key");
        assert_eq!(ch.keys[3].value, 30.0, "the fill copies the playhead key");
        assert_eq!(
            core.project.channels[1].keys.len(),
            1,
            "the other joint's channel is untouched"
        );
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert!(
            core.status.contains("Filled 2 keys"),
            "the library owns the status: {}",
            core.status
        );
        // The range is fully keyed now: the op's own
        // nothing-to-do refusal, snapshot-free.
        assert_eq!(fill_keys_in_range_core(&mut core), 0);
        assert_eq!(core.undo_depth(), depth + 1, "no second snapshot");
    }

    #[test]
    fn fill_keys_in_range_core_refuses_without_a_joint_range_or_channel() {
        // No loop range: status-only, no snapshot, keys intact.
        let mut core = core_with_spread_angle_keys();
        core.current = 2;
        let depth = core.undo_depth();
        assert_eq!(fill_keys_in_range_core(&mut core), 0);
        assert!(core.status.contains("loop range"));
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(selected_angle_channel(&core).map(|ch| ch.keys.len()), Some(3));
        // No joint selected: the same discipline.
        let mut core = core_with_spread_angle_keys();
        core.loop_range = Some((1, 3));
        core.current = 2;
        core.selected_node = None;
        let depth = core.undo_depth();
        assert_eq!(fill_keys_in_range_core(&mut core), 0);
        assert!(core.status.contains("Select a joint"));
        assert_eq!(core.undo_depth(), depth);
        // A joint with no channel on it: refused here,
        // because the op needs the resolved index.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        bare.loop_range = Some((0, 4));
        let depth = bare.undo_depth();
        assert_eq!(fill_keys_in_range_core(&mut bare), 0);
        assert!(bare.status.contains("No graph keys"));
        assert_eq!(bare.undo_depth(), depth);
    }

    #[test]
    fn fill_range_row_builds_live_and_inert() {
        // The `interp_row_builds_live_and_inert...`
        // pattern: the row is asserted through the guard
        // the builder itself uses, plus a full section
        // build in each state.
        let t = rune::tok::theme(true);
        // No loop range: the inert form.
        let mut off = core_with_spread_angle_keys();
        off.current = 2;
        assert!(!fill_range_enabled(&off));
        let _ = keyframe_section(&state_with(off), &t);
        // Loop range set and a key at the playhead: the
        // live form.
        let mut on = core_with_spread_angle_keys();
        on.loop_range = Some((1, 3));
        on.current = 2;
        assert!(fill_range_enabled(&on));
        let _ = keyframe_section(&state_with(on), &t);
    }

    #[test]
    fn nudge_enabled_mirrors_the_nudge_plans_refusals() {
        let mut core = core_with_angle_keys(); // keys at 0 and 2
        // Playhead on the frame-0 key: right lands on the
        // free frame 1, left would leave the timeline.
        core.current = 0;
        assert!(nudge_enabled(&core, 1));
        assert!(!nudge_enabled(&core, -1), "BeforeStart is inert");
        // Playhead on the frame-2 key: both ways land.
        core.current = 2;
        assert!(nudge_enabled(&core, -1));
        assert!(nudge_enabled(&core, 1));
        // Playhead on a bare frame: no key here, both inert.
        core.current = 4;
        assert!(!nudge_enabled(&core, -1), "NoKeyHere is inert");
        assert!(!nudge_enabled(&core, 1), "NoKeyHere is inert");
        // A key on frame 1 blocks both neighbours' nudges
        // into it — the collision refusal, read at render.
        core.project.channels[0].keys.push(Key::flat(1.0, 99.0));
        core.project.channels[0].sort_keys();
        core.current = 0;
        assert!(!nudge_enabled(&core, 1), "Blocked is inert");
        core.current = 2;
        assert!(!nudge_enabled(&core, -1), "Blocked is inert");
        // No channel on the joint, then no joint: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        assert!(!nudge_enabled(&bare, 1));
        bare.selected_node = None;
        assert!(!nudge_enabled(&bare, 1));
    }

    #[test]
    fn copy_key_enabled_mirrors_the_stamp_plans_refusals() {
        let mut core = core_with_angle_keys(); // keys at 0 and 2
        // Playhead key at frame 0, landing frame 10 free.
        core.current = 0;
        assert!(copy_key_enabled(&core));
        // The landing spot occupied — including by the
        // fractional occupant inside the 0.5 window — is
        // the stamp's collision refusal, so inert.
        core.project.channels[0].keys.push(Key::flat(10.4, 99.0));
        core.project.channels[0].sort_keys();
        assert!(!copy_key_enabled(&core), "Blocked is inert");
        // Playhead on a bare frame: nothing to stamp.
        let mut core = core_with_angle_keys();
        core.current = 4;
        assert!(!copy_key_enabled(&core), "NoKeyHere is inert");
        // No channel on the joint, then no joint: inert.
        let mut bare = EditorCore::new();
        bare.selected_node = Some(1);
        assert!(!copy_key_enabled(&bare));
        bare.selected_node = None;
        assert!(!copy_key_enabled(&bare));
    }

    /// A shell `State` over the given core, field-for-field
    /// the literal the other sections' build tests use —
    /// present so the Phase J layout test can build the
    /// whole section, not just its helpers.
    fn state_with(core: EditorCore) -> State {
        State {
            core,
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: None,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    #[test]
    fn no_joint_note_gets_its_own_row_not_the_delete_buttons_row() {
        // `El` has no public tree-walk, so — the timeline
        // marks pattern — the layout is asserted through
        // the helpers and key constants the builder itself
        // uses: the note row is built by
        // `no_joint_note_row` under
        // `NO_JOINT_NOTE_ROW_KEY`, and the Delete button
        // sits in the row keyed `ACTIONS_ROW_KEY`. Two
        // different keyed rows, built by two different
        // children of the section column, cannot be
        // siblings in one row.
        let t = rune::tok::theme(true);

        // No joint selected: the note exists, and its row
        // builds. The whole section builds in this state.
        let mut no_joint = EditorCore::new();
        no_joint.selected_node = None;
        assert_eq!(no_joint_note(&no_joint), Some("No joint selected"));
        assert!(
            no_joint_note_row(&no_joint, &t).is_some(),
            "the no-joint state renders the dedicated note row"
        );
        let _ = keyframe_section(&state_with(no_joint), &t);

        // A joint selected: no note, no note row — the
        // target slot falls back to the original bare
        // target line. The whole section builds here too.
        let selected = core_with_angle_keys();
        assert_eq!(no_joint_note(&selected), None);
        assert!(
            no_joint_note_row(&selected, &t).is_none(),
            "the selected state must not gain an empty note row"
        );
        assert!(target_text(&selected).starts_with("Figure"));
        let _ = keyframe_section(&state_with(selected), &t);

        // The rows are distinct, and neither the actions
        // row nor the Delete button was renamed by the
        // fix — their keys are the values they have
        // always carried.
        assert_ne!(NO_JOINT_NOTE_ROW_KEY, ACTIONS_ROW_KEY);
        assert_eq!(ACTIONS_ROW_KEY, "keyframe-actions");
        assert_eq!(DELETE_KEY_BUTTON_KEY, "key-delete");
    }
}
