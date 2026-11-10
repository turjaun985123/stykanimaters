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
//! * "Auto tangents" → `EditorCore::smooth_graph_tangents`.
//!   This is the core's one smoothing op: it runs
//!   `graph::smooth_channel` over every channel IN THE
//!   PROJECT — all figures, not just the selected one —
//!   which in turn fits `graph::auto_tangents`
//!   (graph.rs:348, :305). The brief's "auto tangents" and
//!   "smooth channel" are the same op at core level, so
//!   there is deliberately ONE button — two buttons calling
//!   one op would be a fake distinction. The write set is
//!   the widest in this section, so the button's row
//!   carries a scope label saying so (Wave 10, see the
//!   note at the end of this header).
//! * Tangent presets → `EditorCore::apply_graph_ease_preset`,
//!   one button per `graph::TangentPreset` variant
//!   (graph.rs:358): Smooth, EaseIn, EaseOut, EaseInOut,
//!   Linear. The write set is EVERY channel belonging to
//!   the selected figure and EVERY key in them — the op
//!   rewrites each key's tangents, and the Linear preset
//!   also rewrites each key's interp mode — never just
//!   the selected joint's channel or the key at the
//!   playhead. The chips' row carries a scope label
//!   saying so (Wave 10). (The per-key interp modes are
//!   section 9's `set_graph_key_interp` — not duplicated
//!   here.)
//! * "Delete key here" → NO `EditorCore` op removes a
//!   GRAPH channel key (verified: the core's graph family
//!   is key/smooth/preset/interp/bake/clear only). Wave 10
//!   added `EditorCore::delete_keyframe`, but that is the
//!   DOPESHEET's op: it deletes a POSE keyframe — a frame
//!   whose stored pose changes versus the previous frame,
//!   in the dense pose store `stats::figure_keyframes`
//!   reads — by overwriting the frame's pose with the
//!   previous frame's. It never reads or writes
//!   `project.channels`, so it cannot remove a graph key,
//!   and this handler cannot delete a pose keyframe: the
//!   two "key" stores are different data. The handler
//!   below therefore still composes the engine's public
//!   API exactly the way `set_graph_key_interp` does
//!   internally: locate the selected figure's
//!   `Angle(selected_node)` channel, `snapshot()`
//!   (editor.rs:226, pub), then `Channel::remove_key`
//!   (graph.rs:144 — removes the key nearest the frame,
//!   within 0.5 frames), then set `status`. Nothing is
//!   removed when no key is near the current frame, and
//!   no snapshot is taken for a no-op. (The same store
//!   split covers Wave 10's `goto_next_keyframe` /
//!   `goto_prev_keyframe`: they move the playhead between
//!   POSE keyframes of a figure. This section has no
//!   playhead navigation control, and no core op
//!   navigates GRAPH keys, so there is nothing here to
//!   converge to them.)
//! * Copy keys / Paste keys → `stykanimate_rs::graph_keys`
//!   (Phase E, section 11): `graph_keys::copy_keys(core,
//!   channel_index)` / `graph_keys::paste_keys(core,
//!   channel_index, keys, at_frame)`. The Phase D gap is
//!   CLOSED: Phase D found no copy/paste-keys op or key
//!   clipboard anywhere in the engine; Phase E added the
//!   engine helpers, and this section supplies the shell
//!   clipboard (`State::key_clipboard`, an
//!   `Option<KeyClipboard>` owned by the shell — the
//!   core's own clipboard holds whole frames, not keys;
//!   the tag is Wave AC's, see the follow-mode bullet
//!   below). Copy stores the
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
//!   key/smooth/preset/interp/bake/clear only). The library's
//!   `graph_keys::shift_keys` is not one either: it shifts
//!   EVERY key of a channel by one delta, where the nudge
//!   moves the single key at the playhead under a collision
//!   rule — a different op. The handler
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
//! * Follow-the-graph mode (Wave AC, AC1) → the same
//!   single-target cluster under a second resolution:
//!   with the section's "Follow graph channel" toggle
//!   on, every op above resolves through the graph
//!   panel's `effective_channel_index` — the DRAWN
//!   channel, whichever target it carries, duplicates
//!   included — through the `*_follow` twin beside each
//!   op. The primary button becomes the generic key op
//!   [`key_here_follow`] (the core's `key_graph_angle`
//!   is angle-only by construction) under a dynamic
//!   "Key {target} here" label, and the target line and
//!   indicator name and read the drawn channel. The
//!   clipboard is tagged with its source channel
//!   ([`KeyClipboard`]): a paste whose target KIND
//!   differs from the copied channel's is refused,
//!   naming both targets, because key values are
//!   unit-typed by kind (degrees / radians / stage
//!   units) — a silent cross-kind paste would be a
//!   silent corruption. The wide ops (Auto tangents,
//!   the tangent presets, Key all here) are untouched
//!   by the mode: their write sets were already wider
//!   than either resolution and already labelled.
//!
//! The keyed-frame indicator is a read-only graph query over
//! `core.project.channels` — the same channel
//! `key_graph_angle` authors.
//!
//! ## Channel scope — Angle by design (Wave AA, AA4)
//!
//! Every single-target op in this section resolves
//! `(selected_figure, Angle(selected_node))` — see
//! [`selected_angle_channel`]. That is the section's
//! charter (Phase D, section 31: the graph-key controls
//! for the selected joint, keying its angle), not an
//! oversight, and the Wave AA audit of the keyframe
//! tools' channel scope re-verified it surface by
//! surface:
//!
//! * The library this section composes
//!   (`stykanimate_rs::graph_keys`) is channel-generic
//!   by design: every op addresses its channel by index
//!   or by (figure, target) pair — Bend, Length and
//!   Root channels included — and the MCP graph tools
//!   inherit that reach (`graph_key` accepts the
//!   `bend:N` / `length:N` / `root_x` spellings; the
//!   index-addressed handlers accept any channel).
//! * The Graph panel, mounted directly above this
//!   section in the same column, edits the DRAWN
//!   channel, whichever target it carries: its delete,
//!   shift, duplicate and canvas edits resolve through
//!   `effective_channel_index`, never a fixed target.
//! * The two angle-named MCP core-op tools
//!   (`graph_key_angle`, `graph_set_interp_at_playhead`)
//!   state their angle scope in their own descriptions
//!   and name the generic counterpart beside them.
//!
//! So no target is unreachable from the shell or from
//! MCP; it is unreachable from THIS section, whose
//! contract is one figure, one joint, its angle
//! channel — framed by the target line and the
//! angle-key indicator, and audited for exactly that
//! honesty in UI mission Wave 10 (the scope-label
//! note below). Widening the section to follow a
//! picked target would duplicate the graph panel's
//! drawn-channel state in a second section; acting on
//! all of the joint's channels at once would break
//! the ops' per-channel semantics (the clipboard holds
//! ONE channel's keys; the nudge collision rule is
//! per channel). The pin
//! `single_target_ops_never_touch_the_joints_other_channels`
//! proves the scope mechanically. Z11's bend-parity
//! audit classified the same scope as documented
//! design ("parity by equal absence", §6.20 remainder
//! (2)); this note is the standing record it pointed
//! at, and the scope changes only by a board call,
//! never as an audit side effect.
//!
//! Wave AC (AC1) is that board call. The board adopted
//! the drawn-target design priced in Wave AB's AB5
//! findings (Variant D) with its recommended defaults:
//! the follow toggle (default OFF, session-only — the
//! new `State::key_follow_graph` field beside the
//! clipboard) makes the single-target ops resolve
//! through the graph panel's `effective_channel_index`
//! instead of `(selected_figure, Angle(selected_node))`.
//! The charter above still governs the default state:
//! with the toggle off, every op resolves exactly as
//! this note describes and the AA4 pin proves it
//! unchanged. With it on, a scope label and the target
//! line name the drawn channel before any press, the
//! key-value rule and the origin authoring refusals
//! are inherited exactly (AB5 Q4/Q8), the clipboard
//! carries its source tag (Q6), and the wide ops keep
//! the write sets recorded here (Q9). The section
//! holds no second selection state — it consumes the
//! graph panel's pick — so the two co-visible sections
//! can never disagree about which channel is live.
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
//!
//! UI mission Wave 9 (agent D9) re-verified the wave 4
//! chip convergence against the easing panel and
//! audited this section's labels against the panels it
//! shares the Graph workspace with (`GRAPH_SECTIONS`
//! in workspaces.rs: Graph, Easing, KeyframeTools —
//! all three mount in the same right column at once):
//!
//! * Chip parity HOLDS. This file's one
//!   [`design::tangent_preset_chip`] call and the
//!   easing panel's one call pass the same preset
//!   order (Smooth, EaseIn, EaseOut, EaseInOut,
//!   Linear), byte-identical labels, `active: false`
//!   and the same `apply_graph_ease_preset` handler.
//!   Sizes and help text cannot differ between the
//!   two: the kit fixes the preview box (52×30,
//!   `TANGENT_PREVIEW_W`/`H`, 24 samples) and derives
//!   `.help()` from the label internally, so neither
//!   call site has a size or help parameter to drift.
//!   Only the element keys differ, by design:
//!   `key-preset-*` here, `ease-graph-*` there.
//! * "Auto tangents" is one dialect already: the
//!   graph panel's button for the same op
//!   (`smooth_graph_tangents`) carries the identical
//!   label, and the easing panel has no such control
//!   at all — nothing to converge.
//! * "Key angle here" (this section) vs the graph
//!   panel's "Key angle" — the same op
//!   (`key_graph_angle`), two dialects, co-visible in
//!   the Graph workspace. The split is deliberate and
//!   stays: every playhead-relative action in THIS
//!   section carries "here" ("Delete key here",
//!   "Key all here"; the section is framed by its
//!   at-frame indicator), so dropping it here would
//!   break the section's own consistency, and
//!   converging the graph panel's side belongs to
//!   that panel's owner. The easing panel's empty
//!   state ("…— Key angle starts one.") uses the
//!   graph panel's short name because the graph
//!   panel is the channel-owning surface directly
//!   above it in the same column — see the note at
//!   that call site.
//!
//! UI mission Wave 10 (agent D8) — round-2 convergence
//! audit against the wave's new core ops and the
//! dopesheet:
//!
//! * The wave's five new `EditorCore` ops
//!   (`delete_keyframe`, `next_keyframe`,
//!   `prev_keyframe`, `goto_next_keyframe`,
//!   `goto_prev_keyframe`) are all POSE-keyframe ops
//!   over the dense pose store (via
//!   `stats::figure_keyframes`); none of them reads or
//!   writes `project.channels`. Every composition in
//!   this section acts on GRAPH channel keys, so none
//!   of them converges to a new core op — the Delete
//!   bullet above carries the full store-split
//!   reasoning, and a test below pins that the pose
//!   ops never touch the graph channels. No panel op
//!   duplicates a core op under different undo/status
//!   semantics: Key angle, Auto tangents and the
//!   presets all call their core ops directly.
//! * Cycle interp vs `EditorCore::set_graph_key_interp`
//!   is a deliberate, documented divergence (the
//!   Phase L bullet predates this note and does not
//!   mention the core op): the core op sets the mode
//!   of the key NEAREST the playhead with no distance
//!   bound, so a press can silently retarget a key
//!   many frames away; this section's contract is the
//!   key AT the playhead (the engine's 0.5-frame
//!   window), guarded inert otherwise, so the button,
//!   its "Interp: {mode}" label and the write can
//!   never disagree about which key is meant. The
//!   handler therefore composes
//!   `graph_keys::set_key_interp` with an explicit key
//!   index instead. A test below pins both halves of
//!   the divergence on identical fixtures.
//! * Easing scope honesty: the section is framed by
//!   its target line (one figure, one joint) and its
//!   channel indicator, but two of its ops write far
//!   wider — Auto tangents smooths every channel in
//!   the project, and the tangent presets rewrite
//!   every key of every channel of the selected
//!   figure. Both rows now carry a kit
//!   `design::section_label` naming the write set
//!   ([`AUTO_TANGENTS_SCOPE_LABEL`] /
//!   [`PRESET_SCOPE_LABEL`]) — the easing panel's own
//!   idiom ("Graph keys — selected figure"). Every
//!   other control's label already matches its traced
//!   write set (this joint/frame, this channel, the
//!   loop range, the playhead key), and Key all here's
//!   "all" is the project channel set its status line
//!   counts.
//! * The "Key angle here" / "Key angle" split (Wave 9
//!   note above) is RE-AFFIRMED after the dopesheet:
//!   the dopesheet adds a third "key" vocabulary —
//!   pose keyframes — to the same shell, which makes
//!   this section's playhead-relative graph dialect
//!   more worth keeping, not less. Nothing in the
//!   dopesheet calls `key_graph_angle` or carries a
//!   competing name for it.
//! * Row widths re-audited against the 316 pt budget
//!   with the longest names in the section: the preset
//!   chips (≈68 pt each — 52 pt preview + padding,
//!   "Ease in-out" the widest label) total ≈364 pt on
//!   one line and wrap 4+1 through the row's
//!   `flex_wrap` (≈296 pt on the first line); the
//!   stamp row ("Key all here" + "Copy key +10")
//!   measures ≈232 pt; the longest single-control row
//!   ("Clear keys in loop range") ≈190 pt; the two new
//!   scope labels are ≤43 characters (≈260 pt at the
//!   SECTION face). Clean.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Interp, Key, TangentPreset};
use stykanimate_rs::graph_keys;

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::rune_sec::graph_panel::{
    channel_label, effective_channel_index, effective_target, GraphPanelState,
};
use crate::rune_sec::icon_btn::{
    danger_icon_btn, icon_btn, icon_btn_inert, icon_only_btn, icon_only_btn_inert,
};
use crate::State;

/// The graph channel this section acts on: the selected
/// figure's `Angle(selected_node)` channel — the one
/// `key_graph_angle` authors and `set_graph_key_interp` edits.
/// The Angle scope is the section's charter, not an
/// oversight — see the module docs' "Channel scope"
/// note (Wave AA, AA4) for the audit trail.
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

/// The shell key clipboard's contents (Wave AC, AC1):
/// one channel's copied keys PLUS the channel they were
/// copied from — its figure and target — recorded at
/// copy time. The Phase E clipboard was a bare
/// `Vec<Key>` and paste resolved only its destination;
/// once this section can follow the graph panel's drawn
/// channel, a cross-TARGET paste becomes reachable, and
/// key values are bare `f32`s whose units differ by
/// target kind (degrees / radians / stage units):
/// pasting bend keys into an angle channel would be
/// mechanically valid and semantically wrong. The tag
/// is what the follow-mode paste refuses on (see
/// [`paste_keys_here_follow`]). The figure is recorded
/// for the refusal's wording; the refusal itself keys
/// on the target KIND ([`same_target_kind`]) — a paste
/// onto the same kind on another joint stays the
/// accepted looseness it has always been (the joint may
/// change between copy and paste, as it could before
/// the tag existed). Session-only shell state, like the
/// clipboard has always been: no persistence, no format
/// impact.
#[derive(Clone, Debug)]
pub struct KeyClipboard {
    /// Figure the keys were copied from (0-based).
    pub figure: usize,
    /// Channel target the keys were copied from.
    pub target: ChannelTarget,
    /// The copied keys themselves.
    pub keys: Vec<Key>,
}

/// Whether two channel targets share a target KIND —
/// Angle with Angle, Length with Length, Bend with
/// Bend, Root X with Root X, Root Y with Root Y — the
/// node index erased. This is the clipboard's paste
/// rule (Wave AC, AC1, on AB5's §5.2 ruling): key
/// values are unit-typed by kind (degrees for Angle,
/// radians for Bend, stage units for Length and the
/// roots), so a paste is unit-safe exactly when the
/// kinds match; the joint a kind names is part of the
/// accepted looseness (copy from joint 1, paste onto
/// joint 2's angle channel — degrees to degrees). Root
/// X and Root Y are distinct kinds: both carry stage
/// units, but an X-motion group is not a Y-motion
/// group.
pub fn same_target_kind(a: ChannelTarget, b: ChannelTarget) -> bool {
    use ChannelTarget::*;
    matches!(
        (a, b),
        (Angle(_), Angle(_))
            | (Length(_), Length(_))
            | (Bend(_), Bend(_))
            | (RootX, RootX)
            | (RootY, RootY)
    )
}

/// The channel this section acts on in follow mode: the
/// graph panel's DRAWN channel, resolved through the
/// panel's own `effective_channel_index` — the single
/// resolution its builder, list, stats and drags share,
/// so this section and the panel above it can never
/// disagree about which channel is live. The resolution
/// is index-exact: a picked duplicate row resolves to
/// THAT twin, where the charter resolution
/// ([`selected_angle_channel_index`]) is first-match.
/// `None` when the selected figure has no channel for
/// the effective target.
pub fn follow_channel_index(core: &EditorCore, gp: &GraphPanelState) -> Option<usize> {
    effective_channel_index(core, gp)
}

/// The drawn channel itself — the reference form of
/// [`follow_channel_index`].
pub fn follow_channel<'a>(core: &'a EditorCore, gp: &GraphPanelState) -> Option<&'a Channel> {
    follow_channel_index(core, gp).map(|ci| &core.project.channels[ci])
}

/// Whether `target` is an ORIGIN target — the node-0
/// form of a segment target (Bend / Angle / Length on
/// node 0). Keying one authors a value into a slot
/// nothing renders, refused by every door (Wave AA
/// AA5's Bend(0) call, Wave AB AB4's Angle(0) /
/// Length(0) call) with the family's exact sentences —
/// see [`origin_key_refusal`]. Root targets have no
/// origin form: the root is a real, rendered position.
pub fn is_origin_target(target: ChannelTarget) -> bool {
    matches!(
        target,
        ChannelTarget::Bend(0) | ChannelTarget::Angle(0) | ChannelTarget::Length(0)
    )
}

/// The authoring-refusal sentence for an origin target,
/// verbatim the sentence the owning door already uses:
/// `key_bend` and MCP `graph_key` for Bend(0),
/// `EditorCore::key_graph_angle` for Angle(0), MCP
/// `graph_key` for Length(0). `None` for a non-origin
/// target. The follow-mode key op ([`key_here_follow`])
/// refuses with these BEFORE any snapshot, on an
/// existing drawn origin channel too — keying one
/// would re-open the dead-write hole AA5 closed. The
/// refusal blocks authoring only: the non-authoring
/// follow ops (delete, nudge, clear, fill, cycle,
/// stamp) act on an existing origin channel freely,
/// exactly as the graph panel's do.
pub fn origin_key_refusal(target: ChannelTarget) -> Option<&'static str> {
    match target {
        ChannelTarget::Bend(0) => Some("The origin has no segment to bend."),
        ChannelTarget::Angle(0) => Some("The origin has no angle to key."),
        ChannelTarget::Length(0) => Some("The origin has no length to key."),
        _ => None,
    }
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

/// The follow-mode target line: "Figure 1 · Bend 3" —
/// the figure and the DRAWN channel, named by the graph
/// panel's own `channel_label`. With no channel drawn
/// yet it names the effective target (the channel a Key
/// press would create one on), so the line never claims
/// a joint the section is not acting on: the charter
/// line's "joint {ni}" framing is false for a drawn
/// Root channel and for another joint's channel, the
/// two cases follow mode exists for.
pub fn target_text_follow(core: &EditorCore, gp: &GraphPanelState) -> String {
    let target = follow_channel(core, gp)
        .map(|ch| ch.target)
        .unwrap_or_else(|| effective_target(core, gp.target));
    format!("Figure {} · {}", core.selected_figure + 1, channel_label(target))
}

/// The follow-mode keyed-frame indicator: the charter
/// indicator's sentences read over the DRAWN channel.
/// (The word "channel" was already target-neutral; the
/// honesty comes from [`target_text_follow`] naming
/// that channel in the line above.) No joint need be
/// selected — a drawn Root channel keys, deletes and
/// nudges with no joint at all. With no channel drawn,
/// the line names the effective target a Key press
/// would create one on, mirroring the charter None
/// branch's shape.
pub fn indicator_text_follow(core: &EditorCore, gp: &GraphPanelState) -> String {
    match follow_channel(core, gp) {
        None => {
            let target = effective_target(core, gp.target);
            format!(
                "No keys yet on {} · frame {}",
                channel_label(target),
                core.current + 1
            )
        }
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

/// The follow-mode Key button's label: "Key {target}
/// here", naming the channel a press would key — the
/// drawn channel, or the effective target a press would
/// create one on. Dynamic by the board's call (AB5 Q7):
/// a fixed "Key channel here" would hide WHICH channel,
/// and the charter's "Key angle here" would lie
/// outright over a drawn Bend channel.
pub fn key_button_label_follow(core: &EditorCore, gp: &GraphPanelState) -> String {
    let target = follow_channel(core, gp)
        .map(|ch| ch.target)
        .unwrap_or_else(|| effective_target(core, gp.target));
    format!("Key {} here", channel_label(target))
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

/// `State` wrapper for [`delete_key_here_core`],
/// dispatching on the section's mode (follow →
/// [`delete_key_here_follow`]).
pub fn delete_key_here(s: &mut State) {
    if s.key_follow_graph {
        delete_key_here_follow(&mut s.core, &s.graph_panel);
    } else {
        delete_key_here_core(&mut s.core);
    }
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

/// `State` wrapper for the copy op, dispatching on the
/// section's mode: charter → [`copy_keys_here_core`],
/// follow → [`copy_keys_here_follow`]. Either way the
/// shell clipboard stores the keys TAGGED with the
/// channel they came from ([`KeyClipboard`], Wave AC
/// AC1) — the charter source is the selected joint's
/// angle channel by construction (a successful copy
/// proves the joint and channel both exist).
pub fn copy_keys_here(s: &mut State) {
    if s.key_follow_graph {
        if let Some(clip) = copy_keys_here_follow(&mut s.core, &s.graph_panel) {
            s.key_clipboard = Some(clip);
        }
    } else if let Some(keys) = copy_keys_here_core(&mut s.core) {
        if let Some(ni) = s.core.selected_node {
            s.key_clipboard = Some(KeyClipboard {
                figure: s.core.selected_figure,
                target: ChannelTarget::Angle(ni),
                keys,
            });
        }
    }
}

/// `State` wrapper for the paste op, dispatching on the
/// section's mode: charter → [`paste_keys_here_core`]
/// over the clipboard's keys (the pre-tag behaviour,
/// byte-identical), follow →
/// [`paste_keys_here_follow`], which reads the tag and
/// refuses a cross-kind paste. Read-only either way —
/// a clipboard can be pasted more than once.
pub fn paste_keys_here(s: &mut State) {
    if s.key_follow_graph {
        paste_keys_here_follow(&mut s.core, &s.graph_panel, s.key_clipboard.as_ref());
    } else {
        paste_keys_here_core(
            &mut s.core,
            s.key_clipboard.as_ref().map(|c| c.keys.as_slice()),
        );
    }
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

/// `State` wrapper for [`nudge_key_here_core`],
/// dispatching on the section's mode (follow →
/// [`nudge_key_here_follow`]).
pub fn nudge_key_here(s: &mut State, delta: i32) {
    if s.key_follow_graph {
        nudge_key_here_follow(&mut s.core, &s.graph_panel, delta);
    } else {
        nudge_key_here_core(&mut s.core, delta);
    }
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

/// `State` wrapper for [`clear_channel_keys_core`],
/// dispatching on the section's mode (follow →
/// [`clear_channel_keys_follow`]).
pub fn clear_channel_keys_here(s: &mut State) {
    if s.key_follow_graph {
        clear_channel_keys_follow(&mut s.core, &s.graph_panel);
    } else {
        clear_channel_keys_core(&mut s.core);
    }
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

/// `State` wrapper for [`clear_keys_in_range_core`],
/// dispatching on the section's mode (follow →
/// [`clear_keys_in_range_follow`]).
pub fn clear_keys_in_range_here(s: &mut State) {
    if s.key_follow_graph {
        clear_keys_in_range_follow(&mut s.core, &s.graph_panel);
    } else {
        clear_keys_in_range_core(&mut s.core);
    }
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

/// `State` wrapper for [`fill_keys_in_range_core`],
/// dispatching on the section's mode (follow →
/// [`fill_keys_in_range_follow`]).
pub fn fill_keys_in_range_here(s: &mut State) {
    if s.key_follow_graph {
        fill_keys_in_range_follow(&mut s.core, &s.graph_panel);
    } else {
        fill_keys_in_range_core(&mut s.core);
    }
}

/// Cycle the interp mode of the selected angle
/// channel's key at the current frame — Hermite →
/// Linear → Step → Hermite (see [`next_interp`]) — and
/// return whether the key's mode changed. Deliberately
/// NOT `EditorCore::set_graph_key_interp`: that op
/// edits the key NEAREST the playhead with no distance
/// bound, while this section's contract is the key AT
/// the playhead — see the Wave 10 note in the module
/// header. The write
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

/// `State` wrapper for [`cycle_key_interp_core`],
/// dispatching on the section's mode (follow →
/// [`cycle_key_interp_follow`]).
pub fn cycle_key_interp_here(s: &mut State) {
    if s.key_follow_graph {
        cycle_key_interp_follow(&mut s.core, &s.graph_panel);
    } else {
        cycle_key_interp_core(&mut s.core);
    }
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

/// `State` wrapper for [`copy_key_to_frame_core`],
/// dispatching on the section's mode (follow →
/// [`copy_key_to_frame_follow`]).
pub fn copy_key_to_frame(s: &mut State) {
    if s.key_follow_graph {
        copy_key_to_frame_follow(&mut s.core, &s.graph_panel);
    } else {
        copy_key_to_frame_core(&mut s.core);
    }
}

/// The follow mode's key op — the generic "key the
/// section's channel here" the charter's angle-only core
/// op cannot express, composed in this file on the graph
/// panel's `key_bend` precedent (Wave AC, AC1, on AB5
/// §5.1 and the board's Q4 key-value rule):
///
/// * A channel already exists for the drawn target →
///   key its EVALUATED value (`Channel::value_at`, via
///   [`key_channel_here`]): keying pins the curve
///   without moving it at the keyed frame.
/// * No channel yet → key the target's pose-store
///   value, creating the channel: Angle delegates to
///   the core op `key_graph_angle` (its pose-angle read
///   and its origin refusal are the angle door's own),
///   Bend delegates to the graph panel's `key_bend`
///   (its `bends_at` read, its origin refusal), and
///   Length / Root read their stores through
///   [`pose_store_value`].
///
/// Origin targets are refused BEFORE any snapshot with
/// the family's exact sentences ([`origin_key_refusal`])
/// — on the existing-channel path too: keying a drawn
/// Bend(0) channel would re-open the dead-write hole
/// AA5 closed. (On the creation path the pre-check also
/// covers Angle(0) / Bend(0) before the delegated doors
/// can be reached; those doors refuse identically
/// themselves.) The refusal blocks authoring only —
/// every non-authoring follow op below acts on an
/// existing origin channel freely. One snapshot per
/// landed key; refusals are status-only. Returns
/// whether a key was authored.
pub fn key_here_follow(core: &mut EditorCore, gp: &GraphPanelState) -> bool {
    let fi = core.selected_figure;
    let frame = core.current as f32;
    if let Some(ci) = follow_channel_index(core, gp) {
        let target = core.project.channels[ci].target;
        if let Some(sentence) = origin_key_refusal(target) {
            core.status = sentence.into();
            return false;
        }
        let value = core.project.channels[ci].value_at(frame);
        core.snapshot();
        key_channel_here(&mut core.project.channels[ci], frame);
        core.status = format!(
            "Graph key: figure {} {} = {} @ frame {}.",
            fi + 1,
            channel_label(target),
            crate::rune_sec::graph_panel::channel_value_text(target, value),
            core.current + 1
        );
        return true;
    }
    // No channel yet: create one on the effective target
    // from its pose-store value. (The effective target is
    // the graph panel's own fallback resolution, so an
    // Angle or Bend target here names the selected node —
    // the delegated doors key exactly that node.)
    let target = effective_target(core, gp.target);
    if let Some(sentence) = origin_key_refusal(target) {
        core.status = sentence.into();
        return false;
    }
    match target {
        ChannelTarget::Angle(_) => {
            // The angle door's own op: pose read, origin
            // refusal, snapshot and status are all its.
            core.key_graph_angle();
            true
        }
        ChannelTarget::Bend(_) => {
            // The graph panel's bend door, same reasoning.
            crate::rune_sec::graph_panel::key_bend(core);
            true
        }
        ChannelTarget::Length(_) | ChannelTarget::RootX | ChannelTarget::RootY => {
            let Some(value) = pose_store_value(core, target) else {
                core.status = format!(
                    "No value to key {} from at this frame.",
                    channel_label(target)
                );
                return false;
            };
            core.snapshot();
            let ch = stykanimate_rs::graph::channel_mut(&mut core.project.channels, fi, target);
            ch.add_key(frame, value);
            core.status = format!(
                "Graph key: figure {} {} = {} @ frame {}.",
                fi + 1,
                channel_label(target),
                crate::rune_sec::graph_panel::channel_value_text(target, value),
                core.current + 1
            );
            true
        }
    }
}

/// The pose-store value a follow-mode Key press authors
/// when the target has no channel yet (the board's Q4
/// rule: a new channel keys the value the target
/// currently evaluates to, so creating the channel never
/// moves the figure). Per target:
///
/// * Length reads the figure def's node length — the
///   store lengths live in outside channels: evaluation
///   starts from the def and length channels override
///   it (graph.rs `apply_channels`), so the def value
///   IS the node's current length.
/// * Root X / Root Y read the frame's pose root
///   (`Project::poses_at`, the same evaluation the
///   stage draws — clips applied), the store root
///   channels override the same way.
///
/// Angle and Bend are NOT read here: their doors exist
/// (`key_graph_angle`, `key_bend`) and the key op
/// delegates to them. `None` when the store has no
/// value to read (no such joint, no frames) — the
/// caller refuses rather than keying a silent 0.0.
pub fn pose_store_value(core: &EditorCore, target: ChannelTarget) -> Option<f32> {
    let fi = core.selected_figure;
    match target {
        ChannelTarget::Length(ni) => core
            .project
            .figures
            .get(fi)
            .and_then(|f| f.def.nodes.get(ni))
            .map(|n| n.length),
        ChannelTarget::RootX => core
            .project
            .poses_at(core.current as f32)
            .get(fi)
            .map(|p| p.root[0]),
        ChannelTarget::RootY => core
            .project
            .poses_at(core.current as f32)
            .get(fi)
            .map(|p| p.root[1]),
        _ => None,
    }
}

/// `State` dispatcher for the section's primary Key
/// button: charter → the core op `key_graph_angle`
/// (the "Key angle here" press, unchanged), follow →
/// [`key_here_follow`].
pub fn key_here(s: &mut State) {
    if s.key_follow_graph {
        key_here_follow(&mut s.core, &s.graph_panel);
    } else {
        s.core.key_graph_angle();
    }
}

/// Delete the key at (nearest to) the current frame on
/// the DRAWN channel (follow mode). Returns whether a
/// key was removed. The charter twin's discipline,
/// transposed: no drawn channel and no key here are
/// status-only refusals, and the snapshot lands only
/// when a key will actually be removed. Non-authoring:
/// an existing origin channel's keys delete freely.
pub fn delete_key_here_follow(core: &mut EditorCore, gp: &GraphPanelState) -> bool {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to delete a key from.".into();
        return false;
    };
    if !keyed_here(&core.project.channels[ci].keys, core.current) {
        core.status = format!("No key at frame {} to delete.", core.current + 1);
        return false;
    }
    core.snapshot();
    let removed = core.project.channels[ci].remove_key(core.current as f32);
    if removed {
        core.status = format!("Deleted graph key at frame {}.", core.current + 1);
    }
    removed
}

/// Copy the DRAWN channel's keys (follow mode),
/// returning them as a tagged [`KeyClipboard`] — the
/// tag is read from the channel itself, so a picked
/// duplicate row tags that twin's pair (the keys are
/// that channel's own either way). `None` (status set)
/// when no channel is drawn or the drawn channel holds
/// no keys — an empty copy never reaches the clipboard,
/// so Paste's muted state always means "nothing to
/// paste", as in charter mode.
pub fn copy_keys_here_follow(
    core: &mut EditorCore,
    gp: &GraphPanelState,
) -> Option<KeyClipboard> {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to copy keys from.".into();
        return None;
    };
    let keys = graph_keys::copy_keys(core, ci)?;
    let ch = &core.project.channels[ci];
    let (figure, target) = (ch.figure, ch.target);
    if keys.is_empty() {
        core.status = format!("No graph keys on {} to copy.", channel_label(target));
        return None;
    }
    core.status = format!(
        "Copied {} keys from figure {} {}.",
        keys.len(),
        figure + 1,
        channel_label(target)
    );
    Some(KeyClipboard {
        figure,
        target,
        keys,
    })
}

/// Paste the clipboard's keys at the current frame into
/// the DRAWN channel (follow mode), through the
/// library's `graph_keys::paste_keys` — it rebases,
/// snapshots and reports the paste itself, so this
/// wrapper sets status only for its own pre-checks.
/// The tag check comes first among them (AB5 §5.2
/// ruling (i), with (iii)'s naming): a clipboard whose
/// target KIND differs from the drawn channel's is
/// refused, naming both targets, before any snapshot —
/// the values are unit-typed by kind and a cross-kind
/// paste would silently corrupt the curve. Same-kind
/// pastes (including cross-joint, the standing
/// looseness) and the no-drawn-channel refusal mirror
/// the charter op's discipline.
pub fn paste_keys_here_follow(
    core: &mut EditorCore,
    gp: &GraphPanelState,
    clipboard: Option<&KeyClipboard>,
) -> bool {
    let Some(clip) = clipboard.filter(|c| !c.keys.is_empty()) else {
        core.status = "Copy some keys first, then Paste keys.".into();
        return false;
    };
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to paste into — key it once first.".into();
        return false;
    };
    let dst = core.project.channels[ci].target;
    if !same_target_kind(clip.target, dst) {
        core.status = format!(
            "Paste refused: the copied keys are {} keys, but the drawn channel is {}.",
            channel_label(clip.target),
            channel_label(dst)
        );
        return false;
    }
    graph_keys::paste_keys(core, ci, &clip.keys, core.current as f32)
}

/// Clear every key on the DRAWN channel (follow mode),
/// as one undo step; returns the number of keys
/// removed. The charter twin's discipline: every no-op
/// — no drawn channel, an already-empty channel —
/// returns 0 with a status line and NO snapshot. The
/// channel itself stays, figure/target pairing intact.
pub fn clear_channel_keys_follow(core: &mut EditorCore, gp: &GraphPanelState) -> usize {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to clear.".into();
        return 0;
    };
    if core.project.channels[ci].keys.is_empty() {
        let target = core.project.channels[ci].target;
        core.status = format!("No graph keys on {} to clear.", channel_label(target));
        return 0;
    }
    core.snapshot();
    let n = clear_channel_keys(&mut core.project.channels[ci]);
    let ch = &core.project.channels[ci];
    core.status = format!(
        "Cleared {n} graph key(s) from figure {} {}.",
        ch.figure + 1,
        channel_label(ch.target)
    );
    n
}

/// Whether Clear channel keys would remove anything in
/// follow mode — the render-time twin of
/// [`clear_channel_keys_follow`]'s refusal set: a
/// channel is drawn and it holds at least one key.
pub fn clear_channel_enabled_follow(core: &EditorCore, gp: &GraphPanelState) -> bool {
    follow_channel(core, gp).is_some_and(|ch| !ch.keys.is_empty())
}

/// Clear the DRAWN channel's keys whose frames lie
/// inside the loop range (follow mode); returns how
/// many keys were removed. The removal is the library's
/// `graph_keys::delete_keys_in_range_at` — the INDEX
/// twin of the charter op's range clear, so a picked
/// duplicate row clears THAT twin, never its first
/// match — and it owns the single snapshot and the
/// status line. The refusal the library cannot phrase
/// — no loop range set, no channel drawn — is answered
/// here, status-only with NO snapshot; the button
/// renders inert in the range state (see
/// [`clear_range_enabled_follow`]), so a press can
/// never reach it.
pub fn clear_keys_in_range_follow(core: &mut EditorCore, gp: &GraphPanelState) -> usize {
    let Some((a, b)) = core.loop_range else {
        core.status = "Set a loop range first, then Clear keys in range.".into();
        return 0;
    };
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to clear keys from.".into();
        return 0;
    };
    graph_keys::delete_keys_in_range_at(core, ci, a as f32, b as f32)
}

/// Whether Clear keys in loop range would remove
/// anything in follow mode — the render-time twin of
/// [`clear_keys_in_range_follow`]'s refusal set: a loop
/// range is set, a channel is drawn, and the DRAWN
/// channel holds at least one key inside the range,
/// counted on the drawn channel itself (index-exact,
/// where the charter guard counts through the library's
/// first-match `keys_in_range`).
pub fn clear_range_enabled_follow(core: &EditorCore, gp: &GraphPanelState) -> bool {
    let Some((a, b)) = core.loop_range else {
        return false;
    };
    let Some(ch) = follow_channel(core, gp) else {
        return false;
    };
    let (a, b) = (a as f32, b as f32);
    ch.keys.iter().any(|k| (a..=b).contains(&k.frame))
}

/// Fill the loop range's keyless frames on the DRAWN
/// channel with copies of the key at the current frame
/// (follow mode); returns how many keys were inserted.
/// The fill is the library's
/// `graph_keys::fill_keys_in_range` over the drawn
/// index — it owns the single snapshot and the status
/// line, including its nothing-to-fill refusal. The
/// refusals it cannot phrase — no loop range set, no
/// channel drawn — are answered here, status-only with
/// NO snapshot; the button renders inert in the range
/// state (see [`fill_range_enabled_follow`]).
pub fn fill_keys_in_range_follow(core: &mut EditorCore, gp: &GraphPanelState) -> usize {
    let Some((a, b)) = core.loop_range else {
        core.status = "Set a loop range first, then Fill range.".into();
        return 0;
    };
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to fill from.".into();
        return 0;
    };
    graph_keys::fill_keys_in_range(core, ci, a, b)
}

/// The interp mode of the DRAWN channel's key at the
/// current frame (follow mode), read through the
/// library's `graph_keys::key_interp_at` over the drawn
/// index. `None` when no channel is drawn or no key
/// sits at the playhead. Pure: reads only.
pub fn current_key_interp_follow(core: &EditorCore, gp: &GraphPanelState) -> Option<Interp> {
    let ci = follow_channel_index(core, gp)?;
    graph_keys::key_interp_at(core, ci, core.current as f32)
}

/// Whether Cycle interp would change anything in
/// follow mode — the render-time twin of
/// [`cycle_key_interp_follow`]'s refusal set.
pub fn interp_cycle_enabled_follow(core: &EditorCore, gp: &GraphPanelState) -> bool {
    current_key_interp_follow(core, gp).is_some()
}

/// Whether Fill range can act in follow mode — the
/// render-time guard beside
/// [`fill_keys_in_range_follow`]: a loop range is set
/// and the drawn channel holds a key at the current
/// frame to fill FROM, read through
/// [`current_key_interp_follow`] — the interp guard's
/// own test — so the two follow guards can never
/// disagree about the playhead key, exactly as their
/// charter twins cannot. The op's all-keyed refusal is
/// deliberately not read here (the charter guard's
/// Phase H precedent: a wired press on the op's own
/// refusal, snapshot-free).
pub fn fill_range_enabled_follow(core: &EditorCore, gp: &GraphPanelState) -> bool {
    core.loop_range.is_some() && current_key_interp_follow(core, gp).is_some()
}

/// The follow-mode `key-interp` button label: "Interp:
/// {current}" while a key sits at the playhead on the
/// drawn channel, the bare "Interp" in the inert state
/// — the charter label's rule, transposed.
pub fn interp_button_label_follow(core: &EditorCore, gp: &GraphPanelState) -> String {
    match current_key_interp_follow(core, gp) {
        Some(interp) => format!("Interp: {}", interp_name(interp)),
        None => "Interp".to_string(),
    }
}

/// Cycle the interp mode of the DRAWN channel's key at
/// the current frame (follow mode) — Hermite → Linear →
/// Step → Hermite — and return whether the key's mode
/// changed. The charter twin's contract transposed: the
/// key AT the playhead (the engine's 0.5-frame window),
/// never the nearest however far; the write is the
/// library's `graph_keys::set_key_interp` over the
/// drawn index, which owns the snapshot and the status
/// line. Panel-side refusals are status-only, NO
/// snapshot, and the button renders inert in exactly
/// those states.
pub fn cycle_key_interp_follow(core: &mut EditorCore, gp: &GraphPanelState) -> bool {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to cycle.".into();
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

/// Whether planning a move of the DRAWN channel's
/// playhead key by `delta` frames lands — the follow
/// twin of [`plan_would_move`], running the same
/// [`nudge_plan`] over the drawn channel's keys, so it
/// is false for exactly the follow nudge/stamp refusal
/// set. Pure: reads only.
fn plan_would_move_follow(core: &EditorCore, gp: &GraphPanelState, delta: i32) -> bool {
    match follow_channel(core, gp) {
        Some(ch) => matches!(
            nudge_plan(&ch.keys, core.current as f32, delta),
            NudgeOutcome::Moved { .. }
        ),
        None => false,
    }
}

/// Whether the follow-mode Nudge button for `delta`
/// would move a key — the render-time twin of
/// [`nudge_key_here_follow`]'s refusals.
pub fn nudge_enabled_follow(core: &EditorCore, gp: &GraphPanelState, delta: i32) -> bool {
    plan_would_move_follow(core, gp, delta)
}

/// Whether follow-mode Copy key +10 would stamp a key —
/// the render-time twin of
/// [`copy_key_to_frame_follow`]'s refusals, planned
/// through the same [`nudge_plan`] with the stamp
/// offset as its delta.
pub fn copy_key_enabled_follow(core: &EditorCore, gp: &GraphPanelState) -> bool {
    plan_would_move_follow(core, gp, COPY_KEY_OFFSET_FRAMES)
}

/// Nudge the key at the current frame on the DRAWN
/// channel by `delta` frames (follow mode), as one undo
/// step. Returns whether a key moved. The charter
/// twin's composition transposed: the plan is checked
/// first ([`nudge_plan`] over the drawn channel), only
/// a `Moved` plan snapshots, and the refusal sentences
/// are the charter op's own — the collision, no-key and
/// before-start outcomes are channel-agnostic facts.
pub fn nudge_key_here_follow(core: &mut EditorCore, gp: &GraphPanelState, delta: i32) -> bool {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to nudge.".into();
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

/// Stamp a copy of the DRAWN channel's key at the
/// current frame onto the frame
/// [`COPY_KEY_OFFSET_FRAMES`] later (follow mode), as
/// one undo step. Returns whether a stamp landed. The
/// charter twin's composition transposed: the stamp
/// plans through [`nudge_plan`] with the offset as its
/// delta, so the collision rule and its refusal
/// sentences are shared exactly.
pub fn copy_key_to_frame_follow(core: &mut EditorCore, gp: &GraphPanelState) -> bool {
    let Some(ci) = follow_channel_index(core, gp) else {
        core.status = "No graph channel is drawn to copy a key from.".into();
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
            // Unreachable for a +10 stamp, as in the
            // charter twin — the match stays exhaustive.
            core.status = format!("Key at frame {} cannot move earlier.", from + 1.0);
            false
        }
    }
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

/// Scope label over the Auto tangents row (Wave 10):
/// `smooth_graph_tangents` smooths every channel in
/// the project — all figures — the widest write set
/// in this section, in a section otherwise framed by
/// one figure and one joint. The label states the
/// write set before the press, the easing panel's
/// idiom for its own figure-wide group. Kept as a
/// constant so the tests can pin the wording.
pub const AUTO_TANGENTS_SCOPE_LABEL: &str = "Smoothing — every channel in the project";

/// Scope label over the tangent-preset row (Wave 10):
/// `apply_graph_ease_preset` rewrites every key of
/// every channel belonging to the selected figure —
/// never just the selected joint's channel or the key
/// at the playhead. Same discipline as
/// [`AUTO_TANGENTS_SCOPE_LABEL`].
pub const PRESET_SCOPE_LABEL: &str = "Tangent presets — selected figure, all keys";

/// Element key of the follow-mode row (Wave AC, AC1):
/// the row holding the "Follow graph channel" toggle,
/// between the indicator and the actions row. One of
/// the mode's two new element keys.
pub const FOLLOW_ROW_KEY: &str = "keyframe-follow";

/// Element key of the follow-mode toggle button itself
/// (Wave AC, AC1) — the mode's second new element key.
/// The button renders accent-filled while the mode is
/// on, the Delete button's state idiom.
pub const FOLLOW_BUTTON_KEY: &str = "key-follow";

/// Scope label under the follow toggle, shown ONLY
/// while follow mode is on (Wave AC, AC1): it names
/// the write set the single-target cluster takes in
/// that mode — the graph panel's drawn channel — before
/// any press, in the same idiom as
/// [`AUTO_TANGENTS_SCOPE_LABEL`] / [`PRESET_SCOPE_LABEL`]
/// and under the same ≤43-character discipline (the
/// dynamic channel name itself belongs to the target
/// line, per AB5 §3: a dynamic section_label would be
/// a new precedent). Kept as a constant so the tests
/// can pin the wording.
pub const FOLLOW_SCOPE_LABEL: &str = "Keys — the graph panel's drawn channel";

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
/// The two wide-scope ops carry kit section labels
/// naming their write sets (Wave 10): Auto tangents
/// smooths every channel in the project, and the
/// tangent presets under
/// [`PRESET_SCOPE_LABEL`] rewrite every key of every
/// channel of the selected figure.
/// The target slot under the header is state-dependent
/// (Phase J): with a joint selected it is the original
/// bare target line; with none it is the dedicated
/// [`no_joint_note_row`], so the note can never paint
/// across the actions row beneath it.
/// Follow mode (Wave AC) inserts one more row under
/// the indicator: the "Follow graph channel" toggle
/// ([`FOLLOW_ROW_KEY`] / [`FOLLOW_BUTTON_KEY`]),
/// accent-filled while on, with [`FOLLOW_SCOPE_LABEL`]
/// under it in that state. In follow mode the target
/// slot always carries the follow target line, the
/// single-target guards and labels come from the
/// `*_follow` twins, and the Key button's label names
/// the drawn channel; the wide rows (Auto tangents,
/// presets, Key all here) render identically in both
/// modes.
pub fn keyframe_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let gp = &s.graph_panel;
    // Follow mode (Wave AC, AC1): with the toggle on,
    // every single-target guard, label and read below
    // resolves through the graph panel's drawn channel
    // (the `*_follow` twins); with it off, everything
    // resolves through the charter angle channel exactly
    // as before. The handlers dispatch on the mode
    // themselves (the reworked `*_here` wrappers), so a
    // press always matches the render that offered it.
    let follow = s.key_follow_graph;
    let keyed = if follow {
        follow_channel(core, gp).is_some_and(|ch| keyed_here(&ch.keys, core.current))
    } else {
        selected_angle_channel(core).is_some_and(|ch| keyed_here(&ch.keys, core.current))
    };
    let interp_label = if follow {
        interp_button_label_follow(core, gp)
    } else {
        interp_button_label(core)
    };
    let key_label = if follow {
        key_button_label_follow(core, gp)
    } else {
        "Key angle here".to_string()
    };
    let copy_live = if follow {
        follow_channel_index(core, gp).is_some()
    } else {
        selected_angle_channel_index(core).is_some()
    };
    let clear_live = if follow {
        clear_channel_enabled_follow(core, gp)
    } else {
        clear_channel_enabled(core)
    };
    let range_live = if follow {
        clear_range_enabled_follow(core, gp)
    } else {
        clear_range_enabled(core)
    };
    let fill_live = if follow {
        fill_range_enabled_follow(core, gp)
    } else {
        fill_range_enabled(core)
    };
    let interp_live = if follow {
        interp_cycle_enabled_follow(core, gp)
    } else {
        interp_cycle_enabled(core)
    };
    let nudge_l_live = if follow {
        nudge_enabled_follow(core, gp, -1)
    } else {
        nudge_enabled(core, -1)
    };
    let nudge_r_live = if follow {
        nudge_enabled_follow(core, gp, 1)
    } else {
        nudge_enabled(core, 1)
    };
    let stamp_live = if follow {
        copy_key_enabled_follow(core, gp)
    } else {
        copy_key_enabled(core)
    };
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
            // had, in the same slot. Follow mode (Wave
            // AC) always renders its own bare target
            // line here, with or without a joint — the
            // no-joint note's premise (nothing in this
            // section can act without a joint) is false
            // when a drawn channel exists, and the line
            // names that channel instead.
            if follow {
                text(target_text_follow(core, gp))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
            } else if let Some(note_row) = no_joint_note_row(core, t) {
                note_row
            } else {
                text(target_text(core))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
            },
        )
        .child(
            text(if follow {
                indicator_text_follow(core, gp)
            } else {
                indicator_text(core)
            })
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        .child({
            // The follow toggle block (Wave AC): the
            // toggle row, accent-filled while the mode
            // is on (the Delete button's state idiom),
            // with the mode's scope label under it —
            // shown only in follow mode, naming the
            // write set before any press.
            let mut block = col()
                .key(FOLLOW_ROW_KEY)
                .gap(6.0)
                .child(btn(FOLLOW_BUTTON_KEY, "Follow graph channel", follow, t, |s| {
                    s.key_follow_graph = !s.key_follow_graph
                }));
            if follow {
                block = block.child(design::section_label(FOLLOW_SCOPE_LABEL, t));
            }
            block
        })
        .child(
            // Phase O (p05, O-5): the actions row joins
            // the panel's wrap discipline — the p05
            // measurement put the unwrapped row at
            // 327.5 pt against the 316 pt column budget.
            // Keys verbatim; wrap only engages past the
            // column width.
            row()
                .key(ACTIONS_ROW_KEY)
                .gap(6.0)
                .flex_wrap()
                .child(btn("key-angle", &key_label, false, t, |s| key_here(s)))
                .child(btn(DELETE_KEY_BUTTON_KEY, "Delete key here", keyed, t, |s| {
                    delete_key_here(s)
                })),
        )
        .child(design::section_label(AUTO_TANGENTS_SCOPE_LABEL, t))
        .child(
            row()
                .key("keyframe-auto")
                .gap(6.0)
                .child(btn("key-auto", "Auto tangents", false, t, |s| {
                    s.core.smooth_graph_tangents()
                })),
        )
        .child(
            // Phase O (p05, O-5): wrap, as on the rows
            // above — the clipboard row measured
            // 318.8 pt against the 316 pt budget.
            row()
                .key("keyframe-clipboard")
                .gap(6.0)
                .flex_wrap()
                .child(if copy_live {
                    btn("key-copy", "Copy keys", false, t, |s| copy_keys_here(s))
                } else {
                    btn_inert("key-copy", "Copy keys", t)
                })
                .child(if s.key_clipboard.as_ref().is_some_and(|c| !c.keys.is_empty()) {
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
                .child(if clear_live {
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
                .child(if range_live {
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
                .child(if fill_live {
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
                .child(if interp_live {
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
                .child(if nudge_l_live {
                    icon_only_btn("key-nudge-l", "chevron-left", "Nudge key earlier", 26.0, false, t, |s| {
                        nudge_key_here(s, -1)
                    })
                } else {
                    icon_only_btn_inert("key-nudge-l", "chevron-left", "Nudge key earlier", 26.0, t)
                })
                .child(if nudge_r_live {
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
                .child(if stamp_live {
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
        .child(design::section_label(PRESET_SCOPE_LABEL, t))
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

    /// The AA4 scope pin: this section's single-target
    /// ops act on the selected joint's ANGLE channel
    /// only — the scope the module docs' "Channel
    /// scope" note promises. The same joint's Bend and
    /// Length channels (same figure, same node) must
    /// come through a delete and a nudge with every
    /// key's frame, value and interp mode untouched,
    /// while the angle channel provably changes — so
    /// the pin fails if the scope ever silently widens
    /// OR if the ops stop acting at all.
    #[test]
    fn single_target_ops_never_touch_the_joints_other_channels() {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(1),
            keys: vec![Key::flat(0.0, 0.3), Key::flat(2.0, -0.4)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Length(1),
            keys: vec![
                Key::flat(0.0, 7.0).with_interp(Interp::Step),
                Key::flat(2.0, 9.0),
            ],
        });
        core.selected_figure = 0;
        core.selected_node = Some(1);
        let others_before: Vec<Vec<(f32, f32, Interp)>> = core.project.channels[1..]
            .iter()
            .map(|ch| ch.keys.iter().map(|k| (k.frame, k.value, k.interp)).collect())
            .collect();

        // Delete removes the angle key at the playhead…
        core.current = 0;
        assert!(delete_key_here_core(&mut core));
        // …and the nudge moves the surviving angle key.
        core.current = 2;
        assert!(nudge_key_here_core(&mut core, 1));

        let angle = selected_angle_channel(&core).expect("angle channel survives");
        assert_eq!(angle.keys.len(), 1, "the frame-0 angle key was deleted");
        assert_eq!(
            (angle.keys[0].frame, angle.keys[0].value),
            (3.0, 30.0),
            "the frame-2 angle key was nudged to frame 3"
        );
        let others_after: Vec<Vec<(f32, f32, Interp)>> = core.project.channels[1..]
            .iter()
            .map(|ch| ch.keys.iter().map(|k| (k.frame, k.value, k.interp)).collect())
            .collect();
        assert_eq!(
            others_after, others_before,
            "the joint's bend and length channels are untouched"
        );
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
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            key_follow_graph: false,
            export_recap: String::new(),
            export_job: None,
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

    #[test]
    fn scope_labels_name_the_wide_write_sets() {
        // Wave 10: the two ops whose write set is wider
        // than this section's one-joint framing carry a
        // kit section label stating it. `El` has no
        // tree-walk, so the wording is pinned through
        // the constants the builder itself renders, plus
        // a full section build.
        assert_eq!(
            AUTO_TANGENTS_SCOPE_LABEL,
            "Smoothing — every channel in the project"
        );
        assert!(
            AUTO_TANGENTS_SCOPE_LABEL.contains("every channel in the project"),
            "the Auto tangents label names its project-wide write set"
        );
        assert_eq!(
            PRESET_SCOPE_LABEL,
            "Tangent presets — selected figure, all keys"
        );
        assert!(
            PRESET_SCOPE_LABEL.contains("selected figure") && PRESET_SCOPE_LABEL.contains("all keys"),
            "the preset label names its figure-wide, all-keys write set"
        );
        // Both fit the right column's 316 pt budget at
        // the SECTION face (≈6 pt per character by the
        // Wave 1 measurements) — a bare section_label
        // has no truncation to fall back on.
        assert!(AUTO_TANGENTS_SCOPE_LABEL.chars().count() <= 43);
        assert!(PRESET_SCOPE_LABEL.chars().count() <= 43);
        let t = rune::tok::theme(true);
        let _ = keyframe_section(&state_with(core_with_angle_keys()), &t);
    }

    #[test]
    fn wave10_pose_keyframe_ops_never_touch_graph_channels() {
        // Wave 10's new core ops (`delete_keyframe`,
        // `goto_next_keyframe`, `goto_prev_keyframe`)
        // act on POSE keyframes in the dense pose store;
        // this section deletes GRAPH channel keys. The
        // stores are disjoint: running the pose ops —
        // including a delete attempt on the base frame
        // and both navigations — must leave the fixture
        // channel's keys exactly as they were, which is
        // why no handler in this section can converge to
        // them.
        let mut core = core_with_angle_keys();
        let before: Vec<(f32, f32)> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| (k.frame, k.value))
            .collect();
        let _ = core.delete_keyframe(0, 0);
        let _ = core.goto_next_keyframe(0);
        let _ = core.goto_prev_keyframe(0);
        let after: Vec<(f32, f32)> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| (k.frame, k.value))
            .collect();
        assert_eq!(after, before, "the pose ops never write the graph store");
        assert_eq!(core.project.channels.len(), 1, "no channel created or removed");
    }

    #[test]
    fn cycle_interp_refuses_the_distant_key_the_core_op_would_edit() {
        // The documented divergence from
        // `EditorCore::set_graph_key_interp` (Wave 10
        // note): on identical fixtures with the playhead
        // on frame 4 — two frames from the nearest key,
        // outside the 0.5-frame window — the core op
        // retargets that distant key, while this
        // section's cycle refuses, inert and
        // snapshot-free.
        let mut via_core = core_with_angle_keys(); // keys at 0 and 2, Hermite
        via_core.current = 4;
        via_core.set_graph_key_interp(Interp::Step);
        let ch = selected_angle_channel(&via_core).expect("channel survives");
        assert_eq!(
            ch.keys[1].interp,
            Interp::Step,
            "the core op edits the nearest key however far the playhead is"
        );

        let mut via_panel = core_with_angle_keys();
        via_panel.current = 4;
        assert!(!interp_cycle_enabled(&via_panel), "the guard reads no key here");
        let depth = via_panel.undo_depth();
        assert!(!cycle_key_interp_core(&mut via_panel));
        assert_eq!(via_panel.undo_depth(), depth, "the refusal never snapshots");
        let ch = selected_angle_channel(&via_panel).expect("channel survives");
        assert_eq!(
            ch.keys[1].interp,
            Interp::Hermite,
            "the distant key keeps its mode"
        );
    }

    // ---- Wave AC (AC1): drawn-target (follow) mode ----

    /// Figure 0 carrying an Angle(1) channel (keys at
    /// 0/2) and a Bend(1) channel (keys at 0/2), joint 1
    /// selected — the mode-pair fixture: identical state
    /// driven through the charter ops and their follow
    /// twins.
    fn core_with_angle_and_bend_keys() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(1),
            keys: vec![Key::flat(0.0, 0.3), Key::flat(2.0, -0.4)],
        });
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core
    }

    /// A graph-panel state whose pick is `target`.
    fn gp_with_target(target: ChannelTarget) -> GraphPanelState {
        GraphPanelState {
            target: Some(target),
            ..Default::default()
        }
    }

    #[test]
    fn follow_mode_delete_acts_on_the_drawn_channel_charter_on_the_angle() {
        // Charter half — the AA4 pin's scenario restated
        // on this fixture: delete removes the ANGLE key;
        // the same joint's bend channel is untouched.
        let mut core = core_with_angle_and_bend_keys();
        core.current = 0;
        assert!(delete_key_here_core(&mut core));
        assert_eq!(
            core.project.channels[0].keys.len(),
            1,
            "charter delete took the angle key"
        );
        assert_eq!(
            core.project.channels[1].keys.len(),
            2,
            "the bend channel is untouched"
        );

        // Follow half, identical fixture: the drawn
        // channel is Bend(1), so delete removes the BEND
        // key and the angle channel is untouched — the
        // mode pair AB5's re-cut of the AA4 pin calls
        // for. The AA4 test itself
        // (`single_target_ops_never_touch_the_joints_other_channels`)
        // stands unchanged above as the charter contract.
        let mut core = core_with_angle_and_bend_keys();
        let gp = gp_with_target(ChannelTarget::Bend(1));
        core.current = 0;
        assert!(delete_key_here_follow(&mut core, &gp));
        assert_eq!(
            core.project.channels[1].keys.len(),
            1,
            "follow delete took the bend key"
        );
        assert_eq!(core.project.channels[1].keys[0].frame, 2.0);
        assert_eq!(
            core.project.channels[0].keys.len(),
            2,
            "the angle channel is untouched"
        );
    }

    #[test]
    fn follow_resolution_is_index_exact_for_a_duplicate_row() {
        // Two Bend(1) channels (duplicates): the graph
        // panel's row pick names the SECOND twin, and the
        // follow resolution honours the index exactly —
        // where the charter resolution is first-match.
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(1),
            keys: vec![Key::flat(0.0, 0.1)],
        });
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(1),
            keys: vec![Key::flat(0.0, 0.2), Key::flat(2.0, 0.5)],
        });
        core.selected_figure = 0;
        core.selected_node = Some(1);
        let gp = GraphPanelState {
            target: Some(ChannelTarget::Bend(1)),
            selected_channel: Some(1),
            ..Default::default()
        };
        assert_eq!(
            follow_channel_index(&core, &gp),
            Some(1),
            "the picked row is the second twin"
        );
        core.current = 0;
        assert!(delete_key_here_follow(&mut core, &gp));
        assert_eq!(
            core.project.channels[0].keys.len(),
            1,
            "the first twin is untouched"
        );
        assert_eq!(
            core.project.channels[1].keys.len(),
            1,
            "the picked twin lost its frame-0 key"
        );
        assert_eq!(core.project.channels[1].keys[0].frame, 2.0);
    }

    #[test]
    fn origin_targets_and_sentences_match_the_doors() {
        assert!(is_origin_target(ChannelTarget::Bend(0)));
        assert!(is_origin_target(ChannelTarget::Angle(0)));
        assert!(is_origin_target(ChannelTarget::Length(0)));
        assert!(!is_origin_target(ChannelTarget::Bend(1)));
        assert!(!is_origin_target(ChannelTarget::RootX));
        assert_eq!(
            origin_key_refusal(ChannelTarget::Bend(0)),
            Some("The origin has no segment to bend.")
        );
        assert_eq!(
            origin_key_refusal(ChannelTarget::Angle(0)),
            Some("The origin has no angle to key.")
        );
        assert_eq!(
            origin_key_refusal(ChannelTarget::Length(0)),
            Some("The origin has no length to key.")
        );
        assert_eq!(origin_key_refusal(ChannelTarget::RootY), None);
    }

    #[test]
    fn follow_key_refuses_drawn_origin_channels_without_snapshotting() {
        // Q8, non-negotiable: keying a drawn origin
        // channel is refused with the owning door's exact
        // sentence, before any snapshot — for all three
        // segment targets. The stored channel itself
        // stays fully editable by the non-authoring ops.
        for (target, sentence) in [
            (ChannelTarget::Bend(0), "The origin has no segment to bend."),
            (ChannelTarget::Angle(0), "The origin has no angle to key."),
            (ChannelTarget::Length(0), "The origin has no length to key."),
        ] {
            let mut core = EditorCore::new();
            core.project.channels.push(Channel {
                figure: 0,
                target,
                keys: vec![Key::flat(0.0, 0.5)],
            });
            core.selected_figure = 0;
            core.selected_node = Some(0);
            let gp = gp_with_target(target);
            let edits = core.edits_since_autosave();
            assert!(!key_here_follow(&mut core, &gp), "{target:?} keying is refused");
            assert_eq!(core.status, sentence);
            assert_eq!(
                core.edits_since_autosave(),
                edits,
                "the refusal never snapshots"
            );
            assert_eq!(
                core.project.channels[0].keys.len(),
                1,
                "no key was authored"
            );
            // Delete — a non-authoring op — still acts on
            // the stored origin channel.
            core.current = 0;
            assert!(
                delete_key_here_follow(&mut core, &gp),
                "delete stays allowed on a stored origin channel"
            );
            assert!(core.project.channels[0].keys.is_empty());
        }
    }

    #[test]
    fn follow_key_on_an_existing_channel_keys_its_evaluated_value() {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(2),
            keys: vec![Key::flat(0.0, 0.0), Key::flat(4.0, 0.4)],
        });
        core.selected_figure = 0;
        core.selected_node = Some(2);
        core.current = 2;
        let gp = gp_with_target(ChannelTarget::Bend(2));
        // The expectation is the engine's own evaluation,
        // read before keying — never hand-computed math.
        let expected = core.project.channels[0].value_at(2.0);
        let edits = core.edits_since_autosave();
        assert!(key_here_follow(&mut core, &gp));
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "exactly one snapshot"
        );
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 3);
        let keyed = ch
            .keys
            .iter()
            .find(|k| (k.frame - 2.0).abs() <= 0.5)
            .expect("a key at frame 2");
        assert_eq!(keyed.value, expected, "the key stores the evaluated value");
        assert_eq!(
            ch.value_at(2.0),
            expected,
            "the curve is unmoved at the keyed frame"
        );
        assert!(
            core.status.contains("Bend 2"),
            "the status names the channel: {}",
            core.status
        );
    }

    #[test]
    fn follow_key_creates_a_root_channel_from_the_pose_root() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        // No joint selected: the graph panel's effective
        // target is Root X, and follow mode can author
        // its first key from the pose store.
        core.selected_node = None;
        core.current = 0;
        let expected = core.project.poses_at(0.0)[0].root[0];
        let gp = GraphPanelState::default();
        assert_eq!(follow_channel_index(&core, &gp), None);
        assert!(key_here_follow(&mut core, &gp));
        assert_eq!(core.project.channels.len(), 1);
        let ch = &core.project.channels[0];
        assert_eq!(ch.target, ChannelTarget::RootX);
        assert_eq!(ch.keys.len(), 1);
        assert_eq!(
            ch.keys[0].value, expected,
            "the key stores the pose's root X"
        );
    }

    #[test]
    fn pose_store_value_reads_def_length_and_pose_root() {
        let core = EditorCore::new();
        let def_len = core.project.figures[0].def.nodes[1].length;
        assert_eq!(
            pose_store_value(&core, ChannelTarget::Length(1)),
            Some(def_len),
            "a length's store is the figure def"
        );
        let root_x = core.project.poses_at(core.current as f32)[0].root[0];
        assert_eq!(
            pose_store_value(&core, ChannelTarget::RootX),
            Some(root_x)
        );
        // No such joint: no value — never a silent 0.0.
        assert_eq!(pose_store_value(&core, ChannelTarget::Length(999)), None);
        // Angle/Bend are not read here: their doors
        // exist and the key op delegates to them.
        assert_eq!(pose_store_value(&core, ChannelTarget::Angle(1)), None);
        assert_eq!(pose_store_value(&core, ChannelTarget::Bend(1)), None);
    }

    #[test]
    fn same_target_kind_erases_the_node_but_not_the_kind() {
        assert!(same_target_kind(
            ChannelTarget::Angle(1),
            ChannelTarget::Angle(2)
        ));
        assert!(same_target_kind(
            ChannelTarget::Bend(0),
            ChannelTarget::Bend(3)
        ));
        assert!(same_target_kind(
            ChannelTarget::Length(2),
            ChannelTarget::Length(1)
        ));
        assert!(same_target_kind(ChannelTarget::RootX, ChannelTarget::RootX));
        assert!(!same_target_kind(ChannelTarget::RootX, ChannelTarget::RootY));
        assert!(!same_target_kind(
            ChannelTarget::Angle(1),
            ChannelTarget::Bend(1)
        ));
        assert!(!same_target_kind(
            ChannelTarget::Length(1),
            ChannelTarget::Angle(1)
        ));
    }

    #[test]
    fn follow_paste_refuses_cross_kind_and_names_both_targets() {
        let mut core = core_with_angle_and_bend_keys();
        let gp_bend = gp_with_target(ChannelTarget::Bend(1));
        let clip = copy_keys_here_follow(&mut core, &gp_bend).expect("bend keys copy");
        assert_eq!(clip.target, ChannelTarget::Bend(1));
        assert_eq!(clip.keys.len(), 2);
        // Aim the paste at the angle channel: refused,
        // naming both targets, before any snapshot, with
        // nothing pasted — radians never land in a
        // degrees channel silently.
        let gp_angle = gp_with_target(ChannelTarget::Angle(1));
        core.current = 4;
        let edits = core.edits_since_autosave();
        assert!(!paste_keys_here_follow(&mut core, &gp_angle, Some(&clip)));
        assert!(
            core.status.contains("Paste refused"),
            "status: {}",
            core.status
        );
        assert!(
            core.status.contains("Bend 1") && core.status.contains("Angle 1"),
            "both targets are named: {}",
            core.status
        );
        assert_eq!(
            core.edits_since_autosave(),
            edits,
            "the refusal never snapshots"
        );
        assert_eq!(
            core.project.channels[0].keys.len(),
            2,
            "nothing was pasted"
        );
    }

    #[test]
    fn follow_paste_allows_the_same_kind_on_another_joint() {
        // The standing looseness, preserved under the
        // tag: keys copied from Angle(1) paste onto
        // Angle(2) — degrees to degrees, the joint may
        // change between copy and paste.
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(2),
            keys: vec![Key::flat(0.0, 5.0)],
        });
        core.selected_figure = 0;
        core.selected_node = Some(2);
        let clip = KeyClipboard {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        };
        let gp = gp_with_target(ChannelTarget::Angle(2));
        core.current = 1;
        assert!(paste_keys_here_follow(&mut core, &gp, Some(&clip)));
        let ch = &core.project.channels[0];
        assert_eq!(ch.keys.len(), 3, "the group landed beside the existing key");
        assert!(ch.keys.iter().any(|k| k.frame == 1.0 && k.value == 10.0));
        assert!(ch.keys.iter().any(|k| k.frame == 3.0 && k.value == 30.0));
    }

    #[test]
    fn follow_ops_work_with_no_joint_selected_on_a_root_channel() {
        let mut core = EditorCore::new();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 100.0), Key::flat(2.0, 140.0)],
        });
        core.selected_figure = 0;
        core.selected_node = None;
        let gp = gp_with_target(ChannelTarget::RootX);
        assert_eq!(target_text_follow(&core, &gp), "Figure 1 · Root X");
        core.current = 0;
        assert!(indicator_text_follow(&core, &gp).starts_with('●'));
        assert!(
            delete_key_here_follow(&mut core, &gp),
            "delete acts with no joint selected"
        );
        assert_eq!(core.project.channels[0].keys.len(), 1);
        // The nudge moves the surviving root key too.
        core.current = 2;
        assert!(nudge_key_here_follow(&mut core, &gp, 1));
        assert_eq!(core.project.channels[0].keys[0].frame, 3.0);
    }

    #[test]
    fn follow_range_ops_act_on_the_drawn_channel_only() {
        let mut core = core_with_spread_angle_keys(); // Angle(1) at 0/2/4
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(1),
            keys: vec![
                Key::flat(0.0, 0.1),
                Key::flat(2.0, 0.2),
                Key::flat(4.0, 0.3),
            ],
        });
        let gp = gp_with_target(ChannelTarget::Bend(1));
        core.loop_range = Some((1, 3));
        assert!(clear_range_enabled_follow(&core, &gp));
        assert_eq!(clear_keys_in_range_follow(&mut core, &gp), 1);
        let bend = &core.project.channels[1];
        let frames: Vec<f32> = bend.keys.iter().map(|k| k.frame).collect();
        assert_eq!(
            frames,
            vec![0.0, 4.0],
            "only the drawn channel's in-range key is gone"
        );
        assert_eq!(
            core.project.channels[0].keys.len(),
            3,
            "the angle channel is untouched"
        );
        assert!(
            core.status.contains("Cleared 1 key(s)"),
            "the library owns the status: {}",
            core.status
        );
        // Fill: the playhead sits on the frame-4 bend
        // key and frame 3 is keyless inside the range.
        core.loop_range = Some((3, 4));
        core.current = 4;
        assert!(fill_range_enabled_follow(&core, &gp));
        assert_eq!(fill_keys_in_range_follow(&mut core, &gp), 1);
        assert_eq!(core.project.channels[1].keys.len(), 3);
        assert_eq!(core.project.channels[1].keys[1].frame, 3.0);
        assert_eq!(core.project.channels[1].keys[1].value, 0.3);
    }

    #[test]
    fn follow_interp_cycle_reads_the_drawn_channels_key() {
        let mut core = core_with_angle_and_bend_keys();
        let gp = gp_with_target(ChannelTarget::Bend(1));
        core.current = 2;
        assert!(interp_cycle_enabled_follow(&core, &gp));
        assert_eq!(interp_button_label_follow(&core, &gp), "Interp: Hermite");
        assert!(cycle_key_interp_follow(&mut core, &gp));
        assert_eq!(core.project.channels[1].keys[1].interp, Interp::Linear);
        assert_eq!(
            core.project.channels[0].keys[1].interp,
            Interp::Hermite,
            "the angle key's mode is untouched"
        );
    }

    #[test]
    fn follow_refusals_are_status_only_when_nothing_is_drawn() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        core.selected_node = Some(1);
        let gp = GraphPanelState::default();
        let edits = core.edits_since_autosave();
        assert!(!delete_key_here_follow(&mut core, &gp));
        assert!(core.status.contains("No graph channel is drawn"));
        assert!(!nudge_key_here_follow(&mut core, &gp, 1));
        assert_eq!(clear_channel_keys_follow(&mut core, &gp), 0);
        assert_eq!(
            core.edits_since_autosave(),
            edits,
            "no refusal snapshots"
        );
        assert!(
            core.project.channels.is_empty(),
            "no channel was created by a refusal"
        );
        // The guards agree: the whole single-target
        // cluster is inert with nothing drawn.
        assert!(!clear_channel_enabled_follow(&core, &gp));
        assert!(!nudge_enabled_follow(&core, &gp, 1));
        assert!(!copy_key_enabled_follow(&core, &gp));
        assert!(!interp_cycle_enabled_follow(&core, &gp));
    }

    #[test]
    fn follow_labels_name_the_drawn_channel_and_the_scope() {
        let core = core_with_angle_and_bend_keys();
        let gp = gp_with_target(ChannelTarget::Bend(1));
        assert_eq!(target_text_follow(&core, &gp), "Figure 1 · Bend 1");
        assert_eq!(key_button_label_follow(&core, &gp), "Key Bend 1 here");
        let gp_angle = gp_with_target(ChannelTarget::Angle(1));
        assert_eq!(key_button_label_follow(&core, &gp_angle), "Key Angle 1 here");
        // The scope label: pinned wording, inside the
        // section's ≤43-character label discipline (the
        // Wave 10 labels' budget).
        assert_eq!(FOLLOW_SCOPE_LABEL, "Keys — the graph panel's drawn channel");
        assert!(FOLLOW_SCOPE_LABEL.chars().count() <= 43);
        // The mode's two new element keys are the values
        // the builder renders under — and the only keys
        // the mode adds.
        assert_eq!(FOLLOW_ROW_KEY, "keyframe-follow");
        assert_eq!(FOLLOW_BUTTON_KEY, "key-follow");
        // The whole section builds in follow mode, with
        // a joint selected and without one.
        let t = rune::tok::theme(true);
        let mut st = state_with(core_with_angle_and_bend_keys());
        st.key_follow_graph = true;
        st.graph_panel = gp_with_target(ChannelTarget::Bend(1));
        let _ = keyframe_section(&st, &t);
        let mut bare = EditorCore::new();
        bare.selected_node = None;
        let mut st = state_with(bare);
        st.key_follow_graph = true;
        let _ = keyframe_section(&st, &t);
    }

    #[test]
    fn state_copy_tags_the_clipboard_and_charter_paste_still_lands() {
        // The tag lands through the State wrapper in
        // charter mode too — and charter paste behaviour
        // is byte-identical over the tagged clipboard.
        let mut st = state_with(core_with_angle_keys());
        copy_keys_here(&mut st);
        let clip = st
            .key_clipboard
            .clone()
            .expect("the copy reached the clipboard");
        assert_eq!(clip.figure, 0);
        assert_eq!(clip.target, ChannelTarget::Angle(1));
        assert_eq!(clip.keys.len(), 2);
        st.core.project.channels[0].keys.clear();
        st.core.current = 1;
        paste_keys_here(&mut st);
        let ch = selected_angle_channel(&st.core).expect("channel survives paste");
        assert_eq!(ch.keys.len(), 2);
        assert_eq!((ch.keys[0].frame, ch.keys[0].value), (1.0, 10.0));
        assert_eq!((ch.keys[1].frame, ch.keys[1].value), (3.0, 30.0));
    }
}
