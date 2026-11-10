//! Command palette (Rune Phase D, section 23): a Ctrl+K
//! overlay with a `TextEdit` query field, fuzzy-filtered
//! results and keyboard navigation — the piece the GPUI shell
//! could never build for want of a text-input component
//! (docs/RUNE_API_AUDIT.md Q5/Q10).
//!
//! ## Design
//!
//! * [`COMMANDS`] is a static table of the core editing
//!   commands. Every entry's `run` is a plain
//!   `fn(&mut State)` (Phase H — it was
//!   `fn(&mut EditorCore)` through Phase G): each entry
//!   is a small shim in this module that forwards to the
//!   same target the GPUI button called — an `EditorCore`
//!   op, or a field flip for ops that don't exist
//!   (Play/Pause and Onion have no op; the GPUI app
//!   flipped the field from its button handlers, and
//!   chrome_view still does for those two) — passing
//!   exactly the argument the button passed (deltas
//!   verified against the pre-removal `src/app.rs`).
//!   Migrating the dispatch to shell state is also what
//!   let the table grow the commands that write *shell*
//!   state (Phase H, below).
//! * Phase E added five commands over the new engine ops:
//!   `duplicate-figure`, `delete-last-stroke`, `fade-audio`,
//!   `reverse-audio` and `delete-audio-clip`. The two
//!   index-taking ops (`duplicate_figure`,
//!   `delete_audio_clip`, `delete_stroke_at`) are wrapped by
//!   shims that resolve "the selected figure" / "the last
//!   entry" at call time and stand down on empty lists.
//! * Phase F added recents: every command run from the
//!   palette is recorded in [`PaletteState::recent`]
//!   (most-recent-first, capped at [`MAX_RECENT`], no
//!   duplicates). An empty query lists the recents first,
//!   under a "Recent" heading (see [`display_commands`]);
//!   a typed query filters exactly as before and ignores
//!   the recents entirely. Recents were session-only at
//!   first — the list started empty on every launch;
//!   Phase L persists them (below).
//! * Phase G added four commands over capabilities that
//!   landed as callable core-level functions, each running
//!   the same code path as its panel button:
//!   `restart-playback` (the playback panel's `pb-restart`
//!   body), `particle-add-event`
//!   (`EditorCore::particle_add_event` at the current
//!   frame, stage centre — the particles panel's own add
//!   arguments), `cam-follow-clear`
//!   (`camera_panel::follow_clear`, used directly as the
//!   `run` value) and `key-all-channels`
//!   (`keyframe_tools::key_all_here_core`). Two Phase G
//!   candidates were deliberately NOT added: toggling
//!   loop playback and stepping the stroke selection both
//!   write *shell* state (`State::loop_playback`,
//!   `State::stroke_selected`) that this table's
//!   `fn(&mut EditorCore)` dispatch cannot reach — a
//!   command here that computed a value with nowhere to
//!   store it would do nothing, which is worse than no
//!   command. They become addable if the table's `run`
//!   type ever migrates to `fn(&mut State)`.
//! * Phase H did that migration: `run` is now
//!   `fn(&mut State)`, every pre-existing entry forwards
//!   to its old target over `&mut s.core` (behaviour and
//!   status lines byte-identical), and the two skipped
//!   candidates landed as three commands:
//!   `toggle-loop-playback` (the playback panel's
//!   `pb-loop` flip of `State::loop_playback`, nothing
//!   more) and `stroke-select-next` /
//!   `stroke-select-prev` (delegating to
//!   `stroke_inspector::step_stroke_selection`). The
//!   longer table (84 entries) needs no change to the recents /
//!   display logic: [`filter_commands`] and
//!   [`display_commands`] are length-agnostic and still
//!   cap every listing at [`MAX_RESULTS`].
//! * Phase I added 13 commands, all appended so every
//!   existing entry keeps its place: the eight workspace
//!   jumps (`go-animate` … `go-home`, thin shims over
//!   `workspaces::select_workspace`, so a jump persists
//!   exactly like a chip press) and five View toggles —
//!   `toggle-guides`, `toggle-frustum` and
//!   `toggle-safe-frame` name the settings popover's own
//!   toggle fns directly (they already take
//!   `&mut State`), while `toggle-left-panel` /
//!   `toggle-right-panel` mirror the View menu's field
//!   flips. One Phase I candidate was deliberately NOT
//!   added: an onion toggle already exists here as
//!   `onion` (the GPUI-era key), and a second entry
//!   flipping the same field would only duplicate it.
//! * Phase J added six commands over the phase's new
//!   capabilities: `split-fx-clip` (the selected FX
//!   clip, split at the playhead frame),
//!   `mirror-figure` (the selected figure mirrored on
//!   every frame), `normalise-audio` (every audio clip
//!   normalised to a 0.9 peak), `duplicate-particle`
//!   and `delete-particle` (the selected particle
//!   event) and `delete-stroke` (the selected stroke).
//!   The four selection-taking shims resolve the shell
//!   selections (`State::fx_selected`,
//!   `State::particle_selected`,
//!   `State::stroke_selected`) at call time and stand
//!   down with an explaining status when nothing is
//!   selected; the two delete shims clear the selection
//!   they consumed, and `duplicate-particle` selects
//!   the copy it made.
//! * Phase K (section 17) audited a proposed "fuzzy
//!   fallback": a substring tier first, then subsequence
//!   matches appended by span. The audit found the
//!   premise does not match this tree — there is no
//!   substring filter to fall back FROM. [`fuzzy_score`]
//!   has matched by case-insensitive subsequence, with
//!   quality scoring, since Phase D, over title, key
//!   AND group alike; the listing's contract is that it
//!   is score-ranked (`results_are_capped_and_score_ordered`).
//!   The proposed tiering would also have dropped the
//!   group field from the fuzzy tier, silently breaking
//!   queries that work today ("vew" reaches the View
//!   toggles through their group alone). So no behaviour
//!   changed; the section pinned the two facts instead
//!   (`exprt_finds_go_export_by_subsequence`,
//!   `group_field_matches_by_subsequence_too`). There is
//!   likewise no separate group-query code path: a
//!   "group query" is an ordinary query whose text
//!   scores against the group field.
//! * Phase L made the recents survive restarts:
//!   `shell::palette_recents` /
//!   `shell::save_palette_recents` hold them on disk as
//!   plain strings. This file keeps a process-wide cache
//!   ([`persisted_recents`]) loaded once from disk and
//!   merged against the live table ([`merge_persisted`]
//!   — unknown keys dropped, so a renamed or removed
//!   command never leaves a ghost entry).
//!   [`PaletteState::recent`] stays the working copy:
//!   [`open_palette`] seeds it from the cache while it
//!   is still empty (the first open of a process), and
//!   the run paths record through [`record_recent`],
//!   which pushes, mirrors into the cache and saves.
//!   Seeding lives in the open path, not in
//!   [`PaletteState::new`], so constructing a `State`
//!   stays side-effect-free; and both the disk read and
//!   the disk write are compiled out of test builds, so
//!   no test can pass or fail because of what a real run
//!   last saved into `./projects` — the tests assert the
//!   pure merge and the cache instead.
//! * Phase M added five commands over the phase's new
//!   ops, appended like Phase J's: `mute-audio-clip`
//!   (the audio panel's target clip — its
//!   `effective_target` resolution over
//!   `State::audio_panel`, the same one the panel's own
//!   mute button uses), `swap-frame-prev` /
//!   `swap-frame-next` (`frame_swap::swap_frame_with_neighbour`,
//!   the op owns every status including the end-of-timeline
//!   refusals) and the two Graph commands
//!   `shift-channel-right` / `fill-keys-range`, which
//!   resolve the drawn channel through the graph panel's
//!   `effective_channel_index` over `State::graph_panel`
//!   — the panel's single resolution, reused rather than
//!   copied, so the palette can never act on a different
//!   channel than the one on screen. The selection-taking
//!   shims stand down with an explaining status in the
//!   Phase J form; `fill-keys-range` answers its
//!   loop-range guard first, in the keyframe tools'
//!   wording.
//! * Wave 9 (UI mission follow-up) appended three
//!   commands over features that landed after Phase M:
//!   `toggle-live-mcp` names the settings popover's own
//!   handler directly (it already takes `&mut State`,
//!   like the Phase I guide toggles), and `theme-light` /
//!   `theme-dark` are thin shims over
//!   `settings_popover::set_theme` passing exactly the
//!   argument the settings Dark / Light buttons pass.
//!   The eight workspace jumps needed no work — Phase I
//!   already covers every workspace. One candidate was
//!   deliberately NOT added by E6: a "save a copy"
//!   command — at E6's edit time no State-level handler
//!   called the `projects::save_copy` primitive yet.
//!   Wave 9 integration appended it as the fourth Wave 9
//!   command once D5's handler landed: `save-copy` names
//!   `project_tools::save_copy_pressed` directly (the
//!   derived "{name} copy" naming is the handler's own,
//!   so the palette invents nothing).
//! * Wave 10 appended two commands for figure-keyframe
//!   navigation, `prev-keyframe` / `next-keyframe`
//!   (Transport): the selected figure's pose keyframes
//!   from `stats::figure_keyframes` (the dopesheet
//!   rule), stepped with the playback panel's strict
//!   `prev_key` / `next_key` helpers and jumped via
//!   `EditorCore::goto`. The transport's `pb-prev-key` /
//!   `pb-next-key` buttons walk camera + graph keys, so
//!   figure pose keys had no jump anywhere; with no key
//!   on the pressed side the shims report in the camera
//!   panel's wording family ("No keyframe before /
//!   after this frame." — a `classify_status` Warn) and
//!   the playhead stays. One candidate was deliberately
//!   NOT added: a "Toggle dopesheet" command — the
//!   shell has no dopesheet lane or panel and no
//!   visibility state for one to flip (the word survives
//!   only in the lib's i18n strings and a stats doc
//!   reference to the original app's rule), so the
//!   command would have nothing real to run.
//! * Wave 12 (C5) appended fifteen commands closing the
//!   style domain, which the palette could not reach at
//!   all while MCP gained it in Wave 11: the six style
//!   presets (`style-preset-*`, forwarding to the style
//!   panel's own `apply_preset` over the ONE canonical
//!   table, `stykanimate_rs::style::STYLE_PRESETS` — the
//!   panel consumes the same table through its `pub use`
//!   re-export since C2, so no second copy exists to
//!   drift), the three direct cap setters, the taper
//!   cycle, the opacity stepper pair (the panel's
//!   `step_opacity`), style copy/paste (the panel's
//!   clipboard handlers) and `open-style-inspector`
//!   (Animate is the only workspace whose section list
//!   carries `Section::InspectorStyle`, and the right
//!   column renders only while `State::show_right`
//!   holds — the command does exactly those two real
//!   writes). The asset domain was audited in the same
//!   pass and deliberately NOT added: B2's segment-
//!   inspector Media section (attach-by-asset / cycle /
//!   clear) had not landed as callable handlers at this
//!   edit time, so its three entries are specced for the
//!   integrator in the Wave 12 notes rather than added
//!   against a panel path that does not exist yet — the
//!   Wave 9 save-copy discipline.
//! * Wave 13 appended two sprite-instance commands:
//!   `add-sprite` (the first project asset, through A1's
//!   `add_sprite`) and `remove-sprite` (the last sprite
//!   instance, through A1's `remove_sprite`). A
//!   `next-sprite` cycle was NOT added: A1's ops take an
//!   explicit index, and neither the core nor the shell
//!   `State` carries a sprite selection for a cycle
//!   command to advance, so it would have been a dead
//!   command — the Wave 10 dopesheet discipline.
//! * [`fuzzy_score`] / [`filter_commands`] are pure and
//!   thoroughly unit-tested below.
//! * The view follows the Rune launcher example's field
//!   pattern verbatim (`examples/launcher`): `text_input(..)
//!   .single_line().on_edit(..).on_input_key(..)
//!   .on_input_change(..).on_submit(..)`, with `TextEdit`
//!   living in [`PaletteState`] (audit Q5: core `TextEdit`,
//!   `pub text: String`, `.clear()`, `focused` flag — all read
//!   from `crates/rune/src/edit.rs`).
//!
//! ## Integration contract (for whoever wires `main.rs`)
//!
//! 1. Declare the module tree: `mod rune_sec;` in `main.rs`
//!    plus a `src/rune_sec/mod.rs` with
//!    `pub mod command_palette;`.
//! 2. Add `pub palette: command_palette::PaletteState` to
//!    `State` and initialise it with `PaletteState::new()`.
//! 3. Render [`palette_view`] above the main column while
//!    `state.palette.open` (e.g. as an overlay child); it is a
//!    self-contained 540 pt panel.
//! 4. Route keys: call [`palette_handle_key`] at the top of
//!    the app `on_key` — it opens on Ctrl+K while closed and
//!    consumes Down/Up/Enter/Escape while open. While the
//!    palette is open the integrator should not let Space /
//!    arrows fall through to the transport bindings (a focused
//!    field normally claims typed keys first per audit Q4's
//!    routing, but belt and braces costs nothing).

use std::cell::RefCell;

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El, Key, KeyEvent, TextEdit, Transition, Tx};
use stykanimate_rs::audio_ops;
use stykanimate_rs::graph_keys;
use stykanimate_rs::model::CapKind;
use stykanimate_rs::vfx::ClipKind;

use crate::rune_sec::design;
use crate::rune_sec::icons;
use crate::rune_sec::workspaces::Workspace;
use crate::State;

/// Maximum number of result rows the palette shows at once.
pub const MAX_RESULTS: usize = 12;

/// Maximum number of recently-run commands remembered for
/// the empty-query list's "Recent" section.
pub const MAX_RECENT: usize = 5;

// ----------------------------------------------------------------
// Command table
// ----------------------------------------------------------------

/// One palette command: a stable `key` (the old GPUI control
/// id where one existed), a human `title`, the `group` it is
/// filed under in the results list, and the `run` fn called
/// with the shell state when the command is picked (Phase H:
/// `&mut State`, so commands can reach shell state as well
/// as the editor core).
#[derive(Clone, Copy)]
pub struct Command {
    pub key: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub run: fn(&mut State),
}

// -- Shims: ops that take an argument, or field flips with no
// -- op. Argument values mirror the GPUI buttons exactly
// -- (pre-removal src/app.rs: adjust_length ±4, thickness ±1,
// -- fps ±1, zoom ×1.2, particle kinds [Burst, Sparkle, Smoke]
// -- per EditorCore::PARTICLE_KINDS, editor.rs:1713).

/// Play/Pause has no op: the GPUI `toggle_play` flipped the
/// field, syncing the playhead to the shown frame first (the
/// same sync chrome_view's `toggle_play` performs).
fn cmd_play_pause(s: &mut State) {
    let c = &mut s.core;
    if !c.playing {
        c.playhead = c.current as f32;
    }
    c.playing = !c.playing;
}

/// Previous frame, wrapping (chrome transport semantics).
fn cmd_prev_frame(s: &mut State) {
    let c = &mut s.core;
    let n = c.frame_count();
    if n > 0 {
        c.goto((c.current + n - 1) % n);
    }
}

/// Next frame, wrapping (chrome transport semantics).
fn cmd_next_frame(s: &mut State) {
    let c = &mut s.core;
    let n = c.frame_count();
    if n > 0 {
        c.goto((c.current + 1) % n);
    }
}

fn cmd_first_frame(s: &mut State) {
    let c = &mut s.core;
    if c.frame_count() > 0 {
        c.goto(0);
    }
}

fn cmd_fps_up(s: &mut State) {
    let c = &mut s.core;
    c.adjust_fps(1.0);
}

fn cmd_fps_down(s: &mut State) {
    let c = &mut s.core;
    c.adjust_fps(-1.0);
}

fn cmd_solo_figure(s: &mut State) {
    let c = &mut s.core;
    let fi = c.selected_figure;
    c.toggle_solo(fi);
}

fn cmd_lock_figure(s: &mut State) {
    let c = &mut s.core;
    let fi = c.selected_figure;
    c.toggle_figure_lock(fi);
}

fn cmd_length_up(s: &mut State) {
    let c = &mut s.core;
    c.adjust_length(4.0);
}

fn cmd_length_down(s: &mut State) {
    let c = &mut s.core;
    c.adjust_length(-4.0);
}

fn cmd_thickness_up(s: &mut State) {
    let c = &mut s.core;
    c.adjust_thickness(1.0);
}

fn cmd_thickness_down(s: &mut State) {
    let c = &mut s.core;
    c.adjust_thickness(-1.0);
}

fn cmd_zoom_in(s: &mut State) {
    let c = &mut s.core;
    c.camera_zoom_by(1.2);
}

fn cmd_zoom_out(s: &mut State) {
    let c = &mut s.core;
    c.camera_zoom_by(1.0 / 1.2);
}

/// Onion has no op: the GPUI button flipped the field (and
/// chrome_view's app-bar button still does).
fn cmd_toggle_onion(s: &mut State) {
    let c = &mut s.core;
    c.onion = !c.onion;
}

/// Draw mode is a public core field the GPUI mode toggle
/// flipped; there is no op.
fn cmd_toggle_draw_mode(s: &mut State) {
    let c = &mut s.core;
    c.draw_mode = !c.draw_mode;
}

fn cmd_particle_burst(s: &mut State) {
    let c = &mut s.core;
    c.add_particle(0); // ParticleKind::Burst
}

fn cmd_particle_sparkle(s: &mut State) {
    let c = &mut s.core;
    c.add_particle(1); // ParticleKind::Sparkle
}

fn cmd_fx_confetti(s: &mut State) {
    let c = &mut s.core;
    c.add_fx_clip(ClipKind::Confetti);
}

fn cmd_fx_lightning(s: &mut State) {
    let c = &mut s.core;
    c.add_fx_clip(ClipKind::Lightning);
}

/// Phase E: the duplicate op takes a figure index, so the
/// shim passes the current selection — the same pattern as
/// `cmd_solo_figure` / `cmd_lock_figure`.
fn cmd_duplicate_figure(s: &mut State) {
    let c = &mut s.core;
    let fi = c.selected_figure;
    c.duplicate_figure(fi);
}

/// Phase E: `delete_stroke_at` takes a project-wide stroke
/// index, so the shim resolves the last stroke at call time
/// and stands down when the project has no strokes.
fn cmd_delete_last_stroke(s: &mut State) {
    let c = &mut s.core;
    if let Some(i) = c.project.strokes.len().checked_sub(1) {
        c.delete_stroke_at(i);
    }
}

/// Phase E: `delete_audio_clip` takes a clip index, so the
/// shim resolves the last clip at call time and stands down
/// when the project has no audio clips.
fn cmd_delete_last_audio_clip(s: &mut State) {
    let c = &mut s.core;
    if let Some(i) = c.project.audio.len().checked_sub(1) {
        c.delete_audio_clip(i);
    }
}

/// Phase G: Restart playback. The playback panel's
/// `pb-restart` button calls its private helper
/// `start_playback_at(core, 0)` (playback_controls.rs),
/// whose whole body is `goto(frame)` then `playing = true`
/// — `goto` also resets the fractional playhead, so a
/// restart mid-tween starts cleanly on frame 0. The helper
/// is not public, so this shim drives exactly those two
/// core writes, nothing more.
fn cmd_restart_playback(s: &mut State) {
    let c = &mut s.core;
    c.goto(0);
    c.playing = true;
}

/// Phase G: add a particle event at the current frame, at
/// the stage centre — the exact arguments the particles
/// panel's own Add button passes to
/// `EditorCore::particle_add_event` (particles_panel.rs).
/// The panel handler also selects the new event in shell
/// state afterwards; this command deliberately remains
/// the core op alone (Phase H's migration kept every
/// existing command's behaviour identical).
fn cmd_add_particle_event(s: &mut State) {
    let c = &mut s.core;
    let [w, h] = c.project.stage;
    let frame = c.current;
    let _ = c.particle_add_event(frame, w / 2.0, h / 2.0);
}

/// Phase G: key every graph channel at the current frame.
/// `keyframe_tools::key_all_here_core` is the core half
/// of the keyframe panel's "key all" button (its `State`
/// wrapper `key_all_here` calls exactly this); it owns
/// the single snapshot, the per-channel keying and the
/// status line, including the no-channels refusal. The
/// returned channel count is the panel's business.
fn cmd_key_all_channels(s: &mut State) {
    let c = &mut s.core;
    let _ = crate::rune_sec::keyframe_tools::key_all_here_core(c);
}

// -- Phase H forwarding shims: the entries that used to
// -- name an `EditorCore` method (or `camera_panel::
// -- follow_clear`) directly as `run`. Since the
// -- dispatch migrated to `fn(&mut State)`, each is a
// -- one-line forwarder to the same target over
// -- `&mut s.core` — behaviour and status lines are the
// -- target's own, unchanged.

/// Phase H: camera follow clear — forwards to
/// `camera_panel::follow_clear`, the panel button's own
/// handler (it sets `project.camera_follow = None` and
/// the "Camera follow off." status).
fn cmd_cam_follow_clear(s: &mut State) {
    crate::rune_sec::camera_panel::follow_clear(&mut s.core);
}

/// Phase H: toggle loop playback — the two Phase G skips
/// this migration unblocked (see the module docs). The
/// playback panel's `pb-loop` handler is exactly this
/// flip of the shell flag: no core call, no status
/// change; `poll` reads the flag to decide whether the
/// playhead wraps or stops at the end.
fn cmd_toggle_loop_playback(s: &mut State) {
    s.loop_playback = !s.loop_playback;
}

/// Phase H: step the stroke selection to the next
/// stroke — delegates to the stroke panel's own
/// stepping fn, so the selection space, wrap rules and
/// the empty-project status ("No strokes to select.")
/// are the panel's, byte-identical.
fn cmd_stroke_select_next(s: &mut State) {
    crate::rune_sec::stroke_inspector::step_stroke_selection(s, 1);
}

/// Phase H: step the stroke selection to the previous
/// stroke; same delegation as the next-stroke command.
fn cmd_stroke_select_prev(s: &mut State) {
    crate::rune_sec::stroke_inspector::step_stroke_selection(s, -1);
}

// -- Phase I shims: workspace jumps (select_workspace
// -- takes the target as an argument, so each jump is a
// -- one-line forwarder naming its variant) and the two
// -- panel-visibility flips (field flips with no op —
// -- the View menu's own handlers in menus.rs do exactly
// -- these writes). The three guide toggles need no shim:
// -- settings_popover's toggle fns already take
// -- `&mut State`, so the table names them directly.

/// Phase I: go to the Animate workspace — forwards to
/// `workspaces::select_workspace`, the workspace chips'
/// own switch fn, so the jump persists the choice in
/// `LayoutPrefs` exactly as a chip press does.
fn cmd_go_animate(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Animate);
}

/// Phase I: go to the Draw workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_draw(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Draw);
}

/// Phase I: go to the Graph workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_graph(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Graph);
}

/// Phase I: go to the Effects workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_effects(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Effects);
}

/// Phase I: go to the Audio workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_audio(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Audio);
}

/// Phase I: go to the 3D workspace; same forwarding as
/// [`cmd_go_animate`].
fn cmd_go_build3d(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Build3D);
}

/// Phase I: go to the Export workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_export(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Export);
}

/// Phase I: go to the Home workspace; same forwarding
/// as [`cmd_go_animate`].
fn cmd_go_home(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Home);
}

/// Phase I: toggle the left panel column — the View
/// menu's own flip of the shell flag; no core call, no
/// status change.
fn cmd_toggle_left_panel(s: &mut State) {
    s.show_left = !s.show_left;
}

/// Phase I: toggle the right panel column; same flip
/// as [`cmd_toggle_left_panel`], on `show_right`.
fn cmd_toggle_right_panel(s: &mut State) {
    s.show_right = !s.show_right;
}

// -- Phase J shims: the phase's new capabilities. Four
// -- of them act on a shell selection (fx_selected,
// -- particle_selected, stroke_selected — the same
// -- fields the panels write), so each resolves its
// -- selection at call time and stands down with an
// -- explaining status when nothing is selected: a
// -- palette command that silently did nothing, or
// -- guessed a target, would be worse than the
// -- explanation. The ops themselves own every other
// -- status line (success and refusal alike).

/// Phase J: split the selected FX clip at the
/// playhead. The split point is the playhead's
/// authored frame, `core.current` as f32 — the frame
/// boundary the timeline shows the playhead resting
/// on, on the same frame axis the clip spans use.
/// The op refuses (status only, `false`) when that
/// point is not strictly inside the clip, so an edge
/// playhead explains itself through the op's wording.
fn cmd_split_fx_clip(s: &mut State) {
    match s.fx_selected {
        Some(i) => {
            let at = s.core.current as f32;
            let _ = s.core.fx_split_clip(i, at);
        }
        None => {
            s.core.status = "No FX clip selected to split.".into();
        }
    }
}

/// Phase J: mirror the selected figure on every
/// frame — the all-frames counterpart of
/// `mirror-pose` (which mirrors one frame's pose).
/// Like [`cmd_duplicate_figure`], the shim resolves
/// the selected figure at call time and forwards it;
/// the op owns the snapshot, the status and the
/// no-such-figure refusal.
fn cmd_mirror_figure(s: &mut State) {
    let c = &mut s.core;
    let fi = c.selected_figure;
    let _ = c.mirror_figure_all_frames(fi);
}

/// Phase J: normalise every audio clip so its peak
/// sits at 0.9 — headroom below clipping, the level
/// the audio panel's own normalise works to. The op
/// rewrites each clip's gain from its measured peak
/// and reports how many clips it adjusted; the count
/// is the panel's business, so the shim discards it.
fn cmd_normalise_audio(s: &mut State) {
    let _ = s.core.normalise_all_clips(0.9);
}

/// Phase J: duplicate the selected particle event.
/// The op inserts the copy immediately after the
/// original and returns its index; the shim selects
/// the copy, as the particles panel selects a newly
/// added event — what the user just made is what they
/// most likely want to adjust next. A stale selection
/// is the op's status-only refusal, and the selection
/// is left as it was.
fn cmd_duplicate_particle(s: &mut State) {
    match s.particle_selected {
        Some(i) => {
            if let Some(new) = s.core.particle_duplicate_event(i) {
                s.particle_selected = Some(new);
            }
        }
        None => {
            s.core.status = "No particle event selected to duplicate.".into();
        }
    }
}

/// Phase J: delete the selected stroke — the
/// selection-taking counterpart of
/// `delete-last-stroke`. The op owns the snapshot
/// and the status (including the stale-index
/// refusal); the shim clears the selection it
/// consumed either way, since the index it named no
/// longer addresses the same stroke once the list
/// shifts.
fn cmd_delete_stroke(s: &mut State) {
    match s.stroke_selected {
        Some(i) => {
            s.core.delete_stroke_at(i);
            s.stroke_selected = None;
        }
        None => {
            s.core.status = "No stroke selected to delete.".into();
        }
    }
}

/// Phase J: delete the selected particle event;
/// same shape as [`cmd_delete_stroke`] over
/// `particle_selected` — the op owns the snapshot
/// and the status, the shim clears the consumed
/// selection.
fn cmd_delete_particle(s: &mut State) {
    match s.particle_selected {
        Some(i) => {
            s.core.particle_delete_event(i);
            s.particle_selected = None;
        }
        None => {
            s.core.status = "No particle event selected to delete.".into();
        }
    }
}

// -- Phase M shims: five commands over the phase's new
// -- ops. The frame swaps forward to the frame_swap op,
// -- which owns every status (including the first/last
// -- frame refusals); the other three resolve a target
// -- the way their panels do — the audio panel's
// -- effective_target for the mute, the graph panel's
// -- effective_channel_index for the graph pair — and
// -- stand down with an explaining status when there is
// -- nothing to act on, the Phase J discipline above.

/// Phase M: toggle mute on the audio panel's target
/// clip — the selected clip when the selection is live,
/// the last clip otherwise, exactly the resolution the
/// audio panel's own mute button uses
/// (`audio_panel::effective_target` over
/// `State::audio_panel`). The indexed op owns the
/// snapshot and the status; with no clips there is no
/// target at all, so the shim explains instead.
fn cmd_mute_audio_clip(s: &mut State) {
    match crate::rune_sec::audio_panel::effective_target(
        s.audio_panel.selected,
        s.core.project.audio.len(),
    ) {
        Some(i) => {
            let _ = audio_ops::audio_toggle_muted_at(&mut s.core, i);
        }
        None => {
            s.core.status = "No audio clip selected to mute.".into();
        }
    }
}

/// Phase M: swap the current frame's content with the
/// previous frame's. The op owns the snapshot, the
/// attached-data remap and every status, including the
/// first-frame refusal — the shim is a pure forward.
fn cmd_swap_frame_prev(s: &mut State) {
    let _ = stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut s.core, -1);
}

/// Phase M: swap the current frame's content with the
/// next frame's; same shape as [`cmd_swap_frame_prev`].
fn cmd_swap_frame_next(s: &mut State) {
    let _ = stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut s.core, 1);
}

/// Phase M: shift every key on the drawn graph channel
/// one frame later. The channel resolves through the
/// graph panel's own [`crate::rune_sec::graph_panel::effective_channel_index`]
/// — the panel's single resolution, reused so the
/// command always acts on the channel the panel is
/// showing. The op owns the snapshot, the status and
/// the before-frame-0 refusal (the whole shift is
/// refused, never clamped per key).
fn cmd_shift_channel_right(s: &mut State) {
    match crate::rune_sec::graph_panel::effective_channel_index(&s.core, &s.graph_panel) {
        Some(idx) => {
            let _ = graph_keys::shift_keys(&mut s.core, idx, 1.0);
        }
        None => {
            s.core.status = "No graph channel selected to shift.".into();
        }
    }
}

/// Phase M: fill the loop range's keyless frames on
/// the drawn graph channel with copies of the key at
/// the current frame. Two guards the op cannot phrase
/// for this command are answered here, status-only:
/// no loop range (the keyframe tools' wording) and no
/// drawn channel. The op owns everything else — the
/// no-key-at-the-playhead and already-full refusals
/// included — and the count it returns is the panel's
/// business, so the shim discards it, as
/// [`cmd_normalise_audio`] discards its count.
fn cmd_fill_keys_range(s: &mut State) {
    let Some((a, b)) = s.core.loop_range else {
        s.core.status = "Set a loop range first, then Fill keys.".into();
        return;
    };
    match crate::rune_sec::graph_panel::effective_channel_index(&s.core, &s.graph_panel) {
        Some(idx) => {
            let _ = graph_keys::fill_keys_in_range(&mut s.core, idx, a, b);
        }
        None => {
            s.core.status = "No graph channel selected to fill.".into();
        }
    }
}

// -- Wave 9 shims: post-Phase-M features. The theme
// -- setter takes `&mut EditorCore` plus the target, so
// -- each direction is a one-line forwarder passing
// -- exactly the argument the settings popover's own
// -- Dark / Light buttons pass (settings_popover.rs).
// -- The Live MCP toggle needs no shim: its settings
// -- handler already takes `&mut State`, so the table
// -- names it directly, like the Phase I guide toggles.

/// Wave 9: switch to the light theme — forwards to
/// `settings_popover::set_theme`, the settings Light
/// button's own call.
fn cmd_theme_light(s: &mut State) {
    crate::rune_sec::settings_popover::set_theme(&mut s.core, true);
}

/// Wave 9: switch to the dark theme; same forwarding
/// as [`cmd_theme_light`], with the Dark button's
/// argument.
fn cmd_theme_dark(s: &mut State) {
    crate::rune_sec::settings_popover::set_theme(&mut s.core, false);
}

/// Wave 10: jump the playhead to the selected figure's
/// previous pose keyframe. The key list is
/// `stats::figure_keyframes` over
/// `EditorCore::selected_figure` — the dopesheet rule:
/// frame 0 counts when the figure is posed there, and
/// every later frame whose pose changes is a key. The
/// step itself reuses the playback panel's strict
/// [`crate::rune_sec::playback_controls::prev_key`], so
/// the palette and the transport walk a key list the
/// same way (standing on a key steps to the one before
/// it). The transport's `pb-prev-key` walks camera +
/// graph keys instead — figure pose keys had no jump.
/// With no key before the playhead the command reports
/// in the camera panel's wording family
/// (`camera_panel::goto_prev_key`'s "No camera key
/// before this frame.") and the playhead does not move;
/// on a hit it only `goto`s, like every other jump.
fn cmd_prev_keyframe(s: &mut State) {
    let c = &mut s.core;
    let keys = stykanimate_rs::stats::figure_keyframes(&c.project, c.selected_figure);
    match crate::rune_sec::playback_controls::prev_key(c.current, &keys) {
        Some(frame) => c.goto(frame),
        None => c.status = "No keyframe before this frame.".into(),
    }
}

/// Wave 10: jump the playhead to the selected figure's
/// next pose keyframe; the mirror of
/// [`cmd_prev_keyframe`] over
/// [`crate::rune_sec::playback_controls::next_key`],
/// with the refusal in `camera_panel::goto_next_key`'s
/// wording family.
fn cmd_next_keyframe(s: &mut State) {
    let c = &mut s.core;
    let keys = stykanimate_rs::stats::figure_keyframes(&c.project, c.selected_figure);
    match crate::rune_sec::playback_controls::next_key(c.current, &keys) {
        Some(frame) => c.goto(frame),
        None => c.status = "No keyframe after this frame.".into(),
    }
}

// -- Wave 12 shims (C5): the style domain. Until this
// -- wave the palette had NO path to the segment style
// -- inspector's actions — no preset, no cap setter, no
// -- taper, no copy/paste — while MCP gained the whole
// -- domain in Wave 11 (style.rs B10). Every shim below
// -- forwards to the style panel's own handler or the
// -- core op the panel's control presses, so the palette
// -- can never apply a style the panel would not.
//
// -- The presets read the ONE canonical table,
// -- `stykanimate_rs::style::STYLE_PRESETS`: the panel
// -- consumes the same table through its `pub use`
// -- re-export (inspector_style.rs, Wave 12 C2) and
// -- MCP's `apply_style_preset` reads it too. The
// -- palette carries no preset names, colours or caps
// -- of its own beyond the static titles, and the
// -- Wave 12 test below pins each title to its table
// -- entry, so a renamed preset breaks the test instead
// -- of drifting silently.

/// Wave 12: open the segment style inspector. The
/// panel mounts in the right column under
/// `Section::InspectorStyle`, which only the Animate
/// workspace's section list carries (workspaces.rs),
/// and the right column itself renders only while
/// `State::show_right` holds (main.rs) — so opening
/// the inspector is exactly: jump to Animate through
/// the workspace chips' own switch fn (the Phase I
/// go-* path, persistence included) and make sure the
/// right column is shown. With no segment selected the
/// panel renders its own select-a-joint state; the
/// command fakes nothing.
fn cmd_open_style_inspector(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(s, Workspace::Animate);
    s.show_right = true;
}

/// Wave 12: apply the style preset at `index` in the
/// canonical table to the selected segment — forwards
/// to the style panel's own `apply_preset` (one atomic
/// undo step; colour + cap change, gradient end and
/// opacity pass through; the status names the preset;
/// the colour joins the panel's recent-colours list).
/// `apply_preset` is the one panel handler that
/// refuses SILENTLY — it returns false and writes no
/// status, because its chips only render with a
/// selection — so the shim supplies the style setters'
/// own refusal wording, the Phase J stand-down
/// discipline.
fn apply_style_preset_at(s: &mut State, index: usize) {
    let preset = &stykanimate_rs::style::STYLE_PRESETS[index];
    if !crate::rune_sec::inspector_style::apply_preset(&mut s.core, preset) {
        s.core.status = "Select a joint first.".into();
    }
}

fn cmd_style_preset_ink(s: &mut State) {
    apply_style_preset_at(s, 0);
}

fn cmd_style_preset_crimson(s: &mut State) {
    apply_style_preset_at(s, 1);
}

fn cmd_style_preset_ocean(s: &mut State) {
    apply_style_preset_at(s, 2);
}

fn cmd_style_preset_forest(s: &mut State) {
    apply_style_preset_at(s, 3);
}

fn cmd_style_preset_violet(s: &mut State) {
    apply_style_preset_at(s, 4);
}

fn cmd_style_preset_amber(s: &mut State) {
    apply_style_preset_at(s, 5);
}

/// Wave 12: set the selected segment's cap directly —
/// the style panel's segmented control, whose chips
/// call `set_node_cap` with exactly their own kind.
/// The op owns the snapshot, the status and the
/// no-selection refusal, so each shim is a pure
/// forward with its chip's argument.
fn cmd_style_cap_round(s: &mut State) {
    s.core.set_node_cap(CapKind::Round);
}

/// Wave 12: set the cap to Square; same forward as
/// [`cmd_style_cap_round`] with the Square chip's
/// argument.
fn cmd_style_cap_square(s: &mut State) {
    s.core.set_node_cap(CapKind::Square);
}

/// Wave 12: set the cap to Point; same forward as
/// [`cmd_style_cap_round`] with the Point chip's
/// argument.
fn cmd_style_cap_point(s: &mut State) {
    s.core.set_node_cap(CapKind::Point);
}

/// Wave 12: cycle the selected segment's taper one
/// step (1.0 → 0.6 → 0.3 → 0.0 → 1.0) — the style
/// panel's Taper Step button, which calls the same
/// core op; the op owns the status and the refusal.
fn cmd_cycle_taper(s: &mut State) {
    s.core.cycle_node_taper();
}

/// Wave 12: one opacity stepper press against the
/// selected segment, through the style panel's own
/// `step_opacity` (its ±0.05 step, percent rounding
/// and the `set_node_opacity` write). `step_opacity`
/// no-ops silently with no selection — its stepper
/// only renders with one — so the shim answers the
/// refusal in the style setters' wording first.
fn step_opacity_guarded(s: &mut State, dir: i32) {
    if crate::rune_sec::inspector_style::selected_style(&s.core).is_none() {
        s.core.status = "Select a joint first.".into();
        return;
    }
    crate::rune_sec::inspector_style::step_opacity(&mut s.core, dir);
}

/// Wave 12: opacity + one step; see
/// [`step_opacity_guarded`].
fn cmd_style_opacity_up(s: &mut State) {
    step_opacity_guarded(s, 1);
}

/// Wave 12: opacity − one step; see
/// [`step_opacity_guarded`].
fn cmd_style_opacity_down(s: &mut State) {
    step_opacity_guarded(s, -1);
}

/// Wave 12: copy the selected segment's style into
/// the style panel's clipboard — names the panel's
/// own Copy handler, which owns the capture, the
/// clipboard write and both status lines.
fn cmd_copy_style(s: &mut State) {
    crate::rune_sec::inspector_style::copy_style(&mut s.core);
}

/// Wave 12: paste the style panel's clipboard onto
/// the selected segment — names the panel's own Paste
/// handler (one atomic undo step through
/// `set_node_style_atomic`); it owns the empty-
/// clipboard and no-selection refusals, so the
/// returned bool is the panel's business.
fn cmd_paste_style(s: &mut State) {
    let _ = crate::rune_sec::inspector_style::paste_style(&mut s.core);
}

/// Wave 12 (C5 asset spec, applied at integration): attach
/// a sprite from the ./import folder to the selected
/// segment — the segment inspector's "Attach from import"
/// press, i.e. the core op itself.
fn cmd_attach_media_import(s: &mut State) {
    s.core.attach_image_to_node();
}

/// Wave 12: clear the selected segment's media — the
/// segment inspector's "Clear media" press (the core op;
/// its selection check runs before its snapshot, so a
/// refusal spends no undo step).
fn cmd_clear_node_media(s: &mut State) {
    s.core.clear_node_media();
}

/// Wave 12: attach the NEXT asset in the table to the
/// selected segment — the segment inspector's "Next
/// sprite" press, resolved through the panel's own named
/// helpers (the same composition its button closure runs:
/// `next_asset_target` over `attached_asset`, then
/// `attach_asset`).
fn cmd_cycle_attached_sprite(s: &mut State) {
    if let Some(a) = crate::rune_sec::inspector_segment::next_asset_target(
        crate::rune_sec::inspector_segment::attached_asset(&s.core),
        s.core.project.assets.len(),
    ) {
        crate::rune_sec::inspector_segment::attach_asset(&mut s.core, a);
    }
}

/// Wave 13: add a free-floating sprite instance from
/// the project's assets — A1's `add_sprite` over the
/// first asset in the table (asset 0). The palette has
/// no asset-picker state of its own; choosing a
/// specific asset is the Sprites panel's picker. The
/// op owns the snapshot and the status, including the
/// no-assets refusal (a call with an empty table asks
/// it for asset 1 and is refused in its own wording).
fn cmd_add_sprite(s: &mut State) {
    let _ = s.core.add_sprite(0);
}

/// Wave Y (Y6): import a layered set staged in the
/// ./import folder — Y5's `import_layered_folder` over
/// the selected figure, the MCP `import_layered_set`
/// tool's GUI twin. The importer owns every validation
/// and refusal past the staging check and sets the
/// status itself on success; a refusal comes back as
/// the `Err` text and is surfaced verbatim (the shim
/// refusal pattern, cf. cmd_split_fx_clip).
fn cmd_import_layered_set(s: &mut State) {
    let Some(root) = layered_set_candidate() else {
        s.core.status = "Put a layered set in the import folder first \
            (a package folder holding spriteloop.import.json, or a folder of PNG/JPEG images)."
            .into();
        return;
    };
    if let Err(e) = stykanimate_rs::layered_import::import_layered_folder(
        &mut s.core,
        &root,
        s.core.selected_figure,
    ) {
        s.core.status = e;
    }
}

/// Does `dir` directly hold a layered set — a
/// `spriteloop.import.json` manifest or at least one
/// PNG/JPEG file? (The importer's own folder reading,
/// mirrored for the staging check only.)
fn layered_set_dir(dir: &std::path::Path) -> bool {
    if dir.join("spriteloop.import.json").is_file() {
        return true;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                e.path().extension().is_some_and(|x| {
                    ["png", "jpg", "jpeg"]
                        .iter()
                        .any(|ext| x.eq_ignore_ascii_case(ext))
                })
            })
        })
        .unwrap_or(false)
}

/// Resolve the layered set staged in ./import,
/// deterministically: the import folder itself when it
/// holds a set (the loose staging), else the first
/// subfolder in sorted order that does (the natural
/// staging — the exporter's package folder dropped in
/// whole). The `./import` base mirrors the core's own
/// `first_file_with` resolution.
fn layered_set_candidate() -> Option<std::path::PathBuf> {
    let import_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("import");
    if layered_set_dir(&import_dir) {
        return Some(import_dir);
    }
    let mut subdirs: Vec<std::path::PathBuf> = std::fs::read_dir(&import_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    subdirs.sort();
    subdirs.into_iter().find(|d| layered_set_dir(d))
}

/// Wave 13: remove the LAST free-floating sprite
/// instance — A1's `remove_sprite` over the final
/// index. There is no sprite selection in the core or
/// the shell `State` for the palette to resolve (the
/// same absence that keeps `next-sprite` out of this
/// table — see the module docs), so the command names
/// the last instance, like `delete-last-stroke` names
/// the last stroke; the op owns the snapshot and the
/// success status. An empty sprite table stands down
/// here with its own explanation.
fn cmd_remove_sprite(s: &mut State) {
    match s.core.project.sprites.len() {
        0 => {
            s.core.status = "No sprite to remove.".into();
        }
        n => {
            s.core.remove_sprite(n - 1);
        }
    }
}

fn cmd_add_frame(s: &mut State) {
    s.core.add_frame();
}

fn cmd_duplicate_frame(s: &mut State) {
    s.core.duplicate_frame();
}

fn cmd_delete_frame(s: &mut State) {
    s.core.delete_frame();
}

fn cmd_insert_inbetween(s: &mut State) {
    s.core.insert_inbetween();
}

fn cmd_copy_frame(s: &mut State) {
    s.core.copy_frame();
}

fn cmd_paste_frame(s: &mut State) {
    s.core.paste_frame();
}

fn cmd_undo(s: &mut State) {
    s.core.undo();
    s.stroke_selected = None;
}

fn cmd_redo(s: &mut State) {
    s.core.redo();
    s.stroke_selected = None;
}

fn cmd_save(s: &mut State) {
    s.core.save();
}

fn cmd_load(s: &mut State) {
    s.core.load();
}

fn cmd_save_named_project(s: &mut State) {
    s.core.save_named_project();
}

fn cmd_cycle_project_file(s: &mut State) {
    s.core.cycle_project_file();
}

fn cmd_open_project_file(s: &mut State) {
    s.core.open_project_file();
}

fn cmd_add_figure(s: &mut State) {
    s.core.add_figure();
}

fn cmd_add_quadruped(s: &mut State) {
    s.core.add_quadruped();
}

fn cmd_delete_figure(s: &mut State) {
    s.core.delete_figure();
}

fn cmd_toggle_figure_hidden(s: &mut State) {
    s.core.toggle_figure_hidden();
}

fn cmd_mirror_selected_pose(s: &mut State) {
    s.core.mirror_selected_pose();
}

fn cmd_insert_library_figure(s: &mut State) {
    s.core.insert_library_figure();
}

fn cmd_add_segment(s: &mut State) {
    s.core.add_segment();
}

fn cmd_delete_segment(s: &mut State) {
    s.core.delete_segment();
}

fn cmd_cycle_shape(s: &mut State) {
    s.core.cycle_shape();
}

fn cmd_cycle_bend(s: &mut State) {
    s.core.cycle_bend();
}

fn cmd_cycle_node_color(s: &mut State) {
    s.core.cycle_node_color();
}

fn cmd_frame_all_figures(s: &mut State) {
    s.core.frame_all_figures();
}

fn cmd_centre_on_selection(s: &mut State) {
    s.core.centre_on_selection();
}

fn cmd_camera_clear_key(s: &mut State) {
    s.core.camera_clear_key();
}

fn cmd_toggle_camera_follow(s: &mut State) {
    s.core.toggle_camera_follow();
}

fn cmd_key_graph_angle(s: &mut State) {
    s.core.key_graph_angle();
}

fn cmd_clear_graph(s: &mut State) {
    s.core.clear_graph();
}

fn cmd_toggle_tween_playback(s: &mut State) {
    s.core.toggle_tween_playback();
}

fn cmd_cycle_easing(s: &mut State) {
    s.core.cycle_easing();
}

fn cmd_cycle_brush(s: &mut State) {
    s.core.cycle_brush();
}

fn cmd_toggle_draw_tool(s: &mut State) {
    s.core.toggle_draw_tool();
}

fn cmd_clear_frame_drawings(s: &mut State) {
    s.core.clear_frame_drawings();
}

fn cmd_mirror_strokes_x(s: &mut State) {
    s.core.mirror_strokes_x();
}

fn cmd_stamp_polygon(s: &mut State) {
    s.core.stamp_polygon();
}

fn cmd_clear_particles(s: &mut State) {
    s.core.clear_particles();
}

fn cmd_add_audio_clip(s: &mut State) {
    s.core.add_audio_clip();
}

fn cmd_mixdown_audio(s: &mut State) {
    s.core.mixdown_audio();
}

fn cmd_audio_echo_last(s: &mut State) {
    s.core.audio_echo_last();
}

fn cmd_audio_lowpass_last(s: &mut State) {
    s.core.audio_lowpass_last();
}

fn cmd_audio_trim_silence_last(s: &mut State) {
    s.core.audio_trim_silence_last();
}

fn cmd_audio_split_at_silences_last(s: &mut State) {
    s.core.audio_split_at_silences_last();
}

fn cmd_audio_fade_last(s: &mut State) {
    s.core.audio_fade_last();
}

fn cmd_audio_reverse_last(s: &mut State) {
    s.core.audio_reverse_last();
}

fn cmd_export_png(s: &mut State) {
    s.core.export_png();
}

fn cmd_export_gif(s: &mut State) {
    s.core.export_gif();
}

fn cmd_export_mp4(s: &mut State) {
    s.core.export_mp4();
}

fn cmd_export_webm(s: &mut State) {
    s.core.export_webm();
}

fn cmd_export_piv(s: &mut State) {
    s.core.export_piv();
}

fn cmd_export_stk(s: &mut State) {
    s.core.export_stk();
}

fn cmd_enter_3d(s: &mut State) {
    s.core.enter_3d();
}

fn cmd_exit_3d(s: &mut State) {
    s.core.exit_3d();
}

/// The command inventory. Order matters: it is the display
/// order for an empty query and the tie-break order for equal
/// fuzzy scores.
pub static COMMANDS: &[Command] = &[
    // -- Transport ------------------------------------------------
    Command { key: "play", title: "Play / Pause", group: "Transport", run: cmd_play_pause },
    Command { key: "prev", title: "Previous frame", group: "Transport", run: cmd_prev_frame },
    Command { key: "next", title: "Next frame", group: "Transport", run: cmd_next_frame },
    Command { key: "first-frame", title: "Go to first frame", group: "Transport", run: cmd_first_frame },
    Command { key: "restart-playback", title: "Restart playback", group: "Transport", run: cmd_restart_playback },
    Command { key: "toggle-loop-playback", title: "Toggle loop playback", group: "Transport", run: cmd_toggle_loop_playback },
    // -- Frames ---------------------------------------------------
    Command { key: "add-frame", title: "Add frame", group: "Frames", run: cmd_add_frame },
    Command { key: "dup-frame", title: "Duplicate frame", group: "Frames", run: cmd_duplicate_frame },
    Command { key: "del-frame", title: "Delete frame", group: "Frames", run: cmd_delete_frame },
    Command { key: "inbetween", title: "Insert inbetween", group: "Frames", run: cmd_insert_inbetween },
    Command { key: "copy-frame", title: "Copy frame", group: "Frames", run: cmd_copy_frame },
    Command { key: "paste-frame", title: "Paste frame", group: "Frames", run: cmd_paste_frame },
    Command { key: "fps-up", title: "Increase FPS", group: "Frames", run: cmd_fps_up },
    Command { key: "fps-down", title: "Decrease FPS", group: "Frames", run: cmd_fps_down },
    // -- Edit -----------------------------------------------------
    Command { key: "undo", title: "Undo", group: "Edit", run: cmd_undo },
    Command { key: "redo", title: "Redo", group: "Edit", run: cmd_redo },
    // -- Projects -------------------------------------------------
    Command { key: "save", title: "Save project", group: "Projects", run: cmd_save },
    Command { key: "load", title: "Load project", group: "Projects", run: cmd_load },
    Command { key: "save-named", title: "Save named project", group: "Projects", run: cmd_save_named_project },
    Command { key: "cycle-project", title: "Cycle project file", group: "Projects", run: cmd_cycle_project_file },
    Command { key: "open-project", title: "Open project file", group: "Projects", run: cmd_open_project_file },
    // -- Figures --------------------------------------------------
    Command { key: "add-figure", title: "Add figure", group: "Figures", run: cmd_add_figure },
    Command { key: "add-quadruped", title: "Add quadruped figure", group: "Figures", run: cmd_add_quadruped },
    Command { key: "del-figure", title: "Delete figure", group: "Figures", run: cmd_delete_figure },
    Command { key: "duplicate-figure", title: "Duplicate figure", group: "Figures", run: cmd_duplicate_figure },
    Command { key: "hide-figure", title: "Toggle figure hidden", group: "Figures", run: cmd_toggle_figure_hidden },
    Command { key: "solo-figure", title: "Solo selected figure", group: "Figures", run: cmd_solo_figure },
    Command { key: "lock-figure", title: "Lock selected figure", group: "Figures", run: cmd_lock_figure },
    Command { key: "mirror-pose", title: "Mirror selected pose", group: "Figures", run: cmd_mirror_selected_pose },
    Command { key: "insert-library-figure", title: "Insert library figure", group: "Figures", run: cmd_insert_library_figure },
    // -- Segments -------------------------------------------------
    Command { key: "add-segment", title: "Add segment", group: "Segments", run: cmd_add_segment },
    Command { key: "del-segment", title: "Delete segment", group: "Segments", run: cmd_delete_segment },
    Command { key: "len-up", title: "Lengthen segment", group: "Segments", run: cmd_length_up },
    Command { key: "len-down", title: "Shorten segment", group: "Segments", run: cmd_length_down },
    Command { key: "thick-up", title: "Thicken segment", group: "Segments", run: cmd_thickness_up },
    Command { key: "thick-down", title: "Thin segment", group: "Segments", run: cmd_thickness_down },
    Command { key: "cycle-shape", title: "Cycle segment shape", group: "Segments", run: cmd_cycle_shape },
    Command { key: "cycle-bend", title: "Cycle segment bend", group: "Segments", run: cmd_cycle_bend },
    Command { key: "cycle-node-color", title: "Cycle segment colour", group: "Segments", run: cmd_cycle_node_color },
    Command { key: "attach-media-import", title: "Attach sprite from import folder", group: "Segments", run: cmd_attach_media_import },
    Command { key: "clear-node-media", title: "Clear segment media", group: "Segments", run: cmd_clear_node_media },
    Command { key: "cycle-attached-sprite", title: "Attach next sprite in table", group: "Segments", run: cmd_cycle_attached_sprite },
    Command { key: "add-sprite", title: "Add sprite from assets", group: "Segments", run: cmd_add_sprite },
    Command { key: "remove-sprite", title: "Remove last sprite", group: "Segments", run: cmd_remove_sprite },
    // -- Camera ---------------------------------------------------
    Command { key: "cam-zoom-in", title: "Camera zoom in", group: "Camera", run: cmd_zoom_in },
    Command { key: "cam-zoom-out", title: "Camera zoom out", group: "Camera", run: cmd_zoom_out },
    Command { key: "cam-frame-all", title: "Frame all figures", group: "Camera", run: cmd_frame_all_figures },
    Command { key: "cam-centre", title: "Centre on selection", group: "Camera", run: cmd_centre_on_selection },
    Command { key: "cam-clear-key", title: "Clear camera key", group: "Camera", run: cmd_camera_clear_key },
    Command { key: "cam-follow", title: "Toggle camera follow", group: "Camera", run: cmd_toggle_camera_follow },
    Command { key: "cam-follow-clear", title: "Camera follow: clear", group: "Camera", run: cmd_cam_follow_clear },
    // -- Graph ----------------------------------------------------
    Command { key: "key-graph-angle", title: "Key graph angle", group: "Graph", run: cmd_key_graph_angle },
    Command { key: "key-all-channels", title: "Key all channels here", group: "Graph", run: cmd_key_all_channels },
    Command { key: "clear-graph", title: "Clear graph curves", group: "Graph", run: cmd_clear_graph },
    // -- View -----------------------------------------------------
    Command { key: "onion", title: "Toggle onion skin", group: "View", run: cmd_toggle_onion },
    Command { key: "draw-mode", title: "Toggle draw mode", group: "View", run: cmd_toggle_draw_mode },
    Command { key: "tween-play", title: "Toggle tween playback", group: "View", run: cmd_toggle_tween_playback },
    Command { key: "cycle-easing", title: "Cycle easing", group: "View", run: cmd_cycle_easing },
    // -- Draw -----------------------------------------------------
    Command { key: "cycle-brush", title: "Cycle brush", group: "Draw", run: cmd_cycle_brush },
    Command { key: "draw-tool", title: "Toggle brush / eraser", group: "Draw", run: cmd_toggle_draw_tool },
    Command { key: "clear-drawings", title: "Clear frame drawings", group: "Draw", run: cmd_clear_frame_drawings },
    Command { key: "mirror-strokes", title: "Mirror strokes horizontally", group: "Draw", run: cmd_mirror_strokes_x },
    Command { key: "stamp-polygon", title: "Stamp polygon", group: "Draw", run: cmd_stamp_polygon },
    Command { key: "delete-last-stroke", title: "Delete last stroke", group: "Draw", run: cmd_delete_last_stroke },
    Command { key: "stroke-select-next", title: "Select next stroke", group: "Draw", run: cmd_stroke_select_next },
    Command { key: "stroke-select-prev", title: "Select previous stroke", group: "Draw", run: cmd_stroke_select_prev },
    // -- Effects --------------------------------------------------
    Command { key: "particle-burst", title: "Add particle: Burst", group: "Effects", run: cmd_particle_burst },
    Command { key: "particle-sparkle", title: "Add particle: Sparkle", group: "Effects", run: cmd_particle_sparkle },
    Command { key: "particle-add-event", title: "Add particle event", group: "Effects", run: cmd_add_particle_event },
    Command { key: "fx-clip-confetti", title: "Add FX clip: Confetti", group: "Effects", run: cmd_fx_confetti },
    Command { key: "fx-clip-lightning", title: "Add FX clip: Lightning", group: "Effects", run: cmd_fx_lightning },
    Command { key: "clear-particles", title: "Clear particles and FX", group: "Effects", run: cmd_clear_particles },
    // -- Audio ----------------------------------------------------
    Command { key: "add-audio", title: "Add audio clip", group: "Audio", run: cmd_add_audio_clip },
    Command { key: "mixdown", title: "Mix down audio", group: "Audio", run: cmd_mixdown_audio },
    Command { key: "audio-echo", title: "Audio: echo last clip", group: "Audio", run: cmd_audio_echo_last },
    Command { key: "audio-lowpass", title: "Audio: lowpass last clip", group: "Audio", run: cmd_audio_lowpass_last },
    Command { key: "audio-trim-silence", title: "Audio: trim silence", group: "Audio", run: cmd_audio_trim_silence_last },
    Command { key: "audio-split-silences", title: "Audio: split at silences", group: "Audio", run: cmd_audio_split_at_silences_last },
    Command { key: "fade-audio", title: "Audio: fade last clip", group: "Audio", run: cmd_audio_fade_last },
    Command { key: "reverse-audio", title: "Audio: reverse last clip", group: "Audio", run: cmd_audio_reverse_last },
    Command { key: "delete-audio-clip", title: "Audio: delete last clip", group: "Audio", run: cmd_delete_last_audio_clip },
    // -- Export ---------------------------------------------------
    Command { key: "export-png", title: "Export PNG sequence", group: "Export", run: cmd_export_png },
    Command { key: "export-gif", title: "Export GIF", group: "Export", run: cmd_export_gif },
    Command { key: "export-mp4", title: "Export MP4", group: "Export", run: cmd_export_mp4 },
    Command { key: "export-webm", title: "Export WebM", group: "Export", run: cmd_export_webm },
    Command { key: "export-piv", title: "Export .piv", group: "Export", run: cmd_export_piv },
    Command { key: "export-stk", title: "Export .stk", group: "Export", run: cmd_export_stk },
    // -- 3D -------------------------------------------------------
    Command { key: "enter-3d", title: "Enter 3D mode", group: "3D", run: cmd_enter_3d },
    Command { key: "exit-3d", title: "Exit 3D mode", group: "3D", run: cmd_exit_3d },
    // -- Workspaces (Phase I, appended: every entry above
    // -- keeps its place) ------------------------------------------
    Command { key: "go-animate", title: "Go to Animate workspace", group: "Workspaces", run: cmd_go_animate },
    Command { key: "go-draw", title: "Go to Draw workspace", group: "Workspaces", run: cmd_go_draw },
    Command { key: "go-graph", title: "Go to Graph workspace", group: "Workspaces", run: cmd_go_graph },
    Command { key: "go-effects", title: "Go to Effects workspace", group: "Workspaces", run: cmd_go_effects },
    Command { key: "go-audio", title: "Go to Audio workspace", group: "Workspaces", run: cmd_go_audio },
    Command { key: "go-build3d", title: "Go to 3D workspace", group: "Workspaces", run: cmd_go_build3d },
    Command { key: "go-export", title: "Go to Export workspace", group: "Workspaces", run: cmd_go_export },
    Command { key: "go-home", title: "Go to Home workspace", group: "Workspaces", run: cmd_go_home },
    // -- View toggles (Phase I) ------------------------------------
    Command { key: "toggle-guides", title: "Toggle stage guides", group: "View", run: crate::rune_sec::settings_popover::toggle_guides },
    Command { key: "toggle-frustum", title: "Toggle camera frame guide", group: "View", run: crate::rune_sec::settings_popover::toggle_guides_frustum },
    Command { key: "toggle-safe-frame", title: "Toggle safe frames guide", group: "View", run: crate::rune_sec::settings_popover::toggle_guides_safe },
    Command { key: "toggle-left-panel", title: "Toggle left panel", group: "View", run: cmd_toggle_left_panel },
    Command { key: "toggle-right-panel", title: "Toggle right panel", group: "View", run: cmd_toggle_right_panel },
    // -- Phase J (appended: every entry above keeps its
    // -- place) ------------------------------------------
    Command { key: "split-fx-clip", title: "Split FX clip at playhead", group: "Effects", run: cmd_split_fx_clip },
    Command { key: "mirror-figure", title: "Mirror figure (all frames)", group: "Figures", run: cmd_mirror_figure },
    Command { key: "normalise-audio", title: "Normalise audio levels", group: "Audio", run: cmd_normalise_audio },
    Command { key: "duplicate-particle", title: "Duplicate particle event", group: "Effects", run: cmd_duplicate_particle },
    Command { key: "delete-stroke", title: "Delete selected stroke", group: "Draw", run: cmd_delete_stroke },
    Command { key: "delete-particle", title: "Delete selected particle event", group: "Effects", run: cmd_delete_particle },
    // -- Phase M (appended: every entry above keeps its
    // -- place) ------------------------------------------
    Command { key: "mute-audio-clip", title: "Toggle mute on the selected audio clip", group: "Audio", run: cmd_mute_audio_clip },
    Command { key: "swap-frame-prev", title: "Swap frame with previous", group: "Frames", run: cmd_swap_frame_prev },
    Command { key: "swap-frame-next", title: "Swap frame with next", group: "Frames", run: cmd_swap_frame_next },
    Command { key: "shift-channel-right", title: "Shift graph channel keys +1 frame", group: "Graph", run: cmd_shift_channel_right },
    Command { key: "fill-keys-range", title: "Fill keys across the loop range", group: "Graph", run: cmd_fill_keys_range },
    // -- Wave 9 (appended: every entry above keeps its
    // -- place) ------------------------------------------
    Command { key: "toggle-live-mcp", title: "Toggle Live MCP", group: "View", run: crate::rune_sec::settings_popover::toggle_live_mcp },
    Command { key: "theme-light", title: "Switch to light theme", group: "View", run: cmd_theme_light },
    Command { key: "theme-dark", title: "Switch to dark theme", group: "View", run: cmd_theme_dark },
    Command { key: "save-copy", title: "Save a copy of the project", group: "Projects", run: crate::rune_sec::project_tools::save_copy_pressed },
    // -- Wave 10 (appended: every entry above keeps its
    // -- place) ------------------------------------------
    Command { key: "prev-keyframe", title: "Go to previous keyframe", group: "Transport", run: cmd_prev_keyframe },
    Command { key: "next-keyframe", title: "Go to next keyframe", group: "Transport", run: cmd_next_keyframe },
    // -- Wave 12 (appended: every entry above keeps its
    // -- place) — the style domain; see the Wave 12
    // -- shims above for each forward's target. The six
    // -- preset titles repeat the canonical table's
    // -- names verbatim (stykanimate_rs::style::
    // -- STYLE_PRESETS); the Wave 12 test pins the
    // -- pairing so a rename breaks the test, never the
    // -- palette's honesty. -----------------------------
    Command { key: "open-style-inspector", title: "Open style inspector", group: "Style", run: cmd_open_style_inspector },
    Command { key: "style-preset-ink", title: "Style preset: Ink", group: "Style", run: cmd_style_preset_ink },
    Command { key: "style-preset-crimson", title: "Style preset: Crimson", group: "Style", run: cmd_style_preset_crimson },
    Command { key: "style-preset-ocean", title: "Style preset: Ocean", group: "Style", run: cmd_style_preset_ocean },
    Command { key: "style-preset-forest", title: "Style preset: Forest", group: "Style", run: cmd_style_preset_forest },
    Command { key: "style-preset-violet", title: "Style preset: Violet", group: "Style", run: cmd_style_preset_violet },
    Command { key: "style-preset-amber", title: "Style preset: Amber", group: "Style", run: cmd_style_preset_amber },
    Command { key: "style-cap-round", title: "Set segment cap: Round", group: "Style", run: cmd_style_cap_round },
    Command { key: "style-cap-square", title: "Set segment cap: Square", group: "Style", run: cmd_style_cap_square },
    Command { key: "style-cap-point", title: "Set segment cap: Point", group: "Style", run: cmd_style_cap_point },
    Command { key: "cycle-taper", title: "Cycle segment taper", group: "Style", run: cmd_cycle_taper },
    Command { key: "style-opacity-up", title: "Increase segment opacity", group: "Style", run: cmd_style_opacity_up },
    Command { key: "style-opacity-down", title: "Decrease segment opacity", group: "Style", run: cmd_style_opacity_down },
    Command { key: "copy-style", title: "Copy segment style", group: "Style", run: cmd_copy_style },
    Command { key: "paste-style", title: "Paste segment style", group: "Style", run: cmd_paste_style },
    // -- Wave Y (appended: every entry above keeps its
    // -- place) ------------------------------------------
    Command { key: "import-layered-set", title: "Import layered set", group: "Projects", run: cmd_import_layered_set },
];

// ----------------------------------------------------------------
// Fuzzy filter (pure)
// ----------------------------------------------------------------

/// Score `query` against one text field: a case-insensitive
/// subsequence match, or `None` when the query's characters do
/// not appear in order. Higher is better:
///
/// * +10 per matched character;
/// * +15 when a match is consecutive with the previous one;
/// * +20 when a match starts a word (position 0 or after a
///   space, `-`, `_` or `/` separator);
/// * −1 per skipped character between two matches (gap
///   penalty), so tight matches beat scattered ones.
///
/// An empty query matches everything with score 0.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut qi = 0usize;
    let mut score: i32 = 0;
    let mut prev: Option<usize> = None;
    for (ti, ch) in t.iter().enumerate() {
        if qi < q.len() && *ch == q[qi] {
            score += 10;
            if prev.map_or(false, |p| p + 1 == ti) {
                score += 15;
            }
            if ti == 0 || matches!(t[ti - 1], ' ' | '-' | '_' | '/') {
                score += 20;
            }
            if let Some(p) = prev {
                score -= (ti - p - 1) as i32;
            }
            prev = Some(ti);
            qi += 1;
        }
    }
    if qi == q.len() {
        Some(score)
    } else {
        None
    }
}

/// A command's score for a query: the best of its title, key
/// and group fields, so "add-frame" finds *Add frame* (via the
/// key), "export" lists the Export group, and plain words
/// match titles.
pub fn command_score(query: &str, cmd: &Command) -> Option<i32> {
    [cmd.title, cmd.key, cmd.group]
        .iter()
        .filter_map(|field| fuzzy_score(query, field))
        .max()
}

/// The filtered, ranked command indices for a query: best
/// score first, table order breaking ties, capped at
/// [`MAX_RESULTS`]. An empty (or whitespace) query yields the
/// first `MAX_RESULTS` commands in table order.
pub fn filter_commands(query: &str) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..COMMANDS.len()).take(MAX_RESULTS).collect();
    }
    let mut scored: Vec<(i32, usize)> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(i, cmd)| command_score(query, cmd).map(|s| (s, i)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).take(MAX_RESULTS).collect()
}

// ----------------------------------------------------------------
// Recents (pure)
// ----------------------------------------------------------------

/// Record `key` as the most recently run command: it moves
/// to the front, any earlier entry for the same key is
/// dropped (no duplicates), and the list is capped at
/// [`MAX_RECENT`] with the oldest entries falling off the
/// end.
pub fn push_recent(recent: &mut Vec<&'static str>, key: &'static str) {
    recent.retain(|&k| k != key);
    recent.insert(0, key);
    recent.truncate(MAX_RECENT);
}

/// The rows the palette actually lists, as table indices,
/// plus how many *leading* rows are recents (the view puts
/// a "Recent" heading over them).
///
/// An empty (or whitespace) query leads with the recents —
/// resolved to table indices in most-recent-first order,
/// unknown keys skipped, never repeated — followed by the
/// plain table-order list ([`filter_commands`] with an
/// empty query) with the recents removed, the whole thing
/// capped at [`MAX_RESULTS`]. A non-empty query defers to
/// [`filter_commands`] unchanged and reports 0 recents.
pub fn display_commands(query: &str, recent: &[&'static str]) -> (Vec<usize>, usize) {
    if !query.trim().is_empty() {
        return (filter_commands(query), 0);
    }
    let mut rows: Vec<usize> = Vec::new();
    for key in recent {
        if let Some(i) = COMMANDS.iter().position(|c| c.key == *key) {
            if !rows.contains(&i) {
                rows.push(i);
            }
        }
    }
    let recent_rows = rows.len();
    for i in filter_commands("") {
        if !rows.contains(&i) {
            rows.push(i);
        }
    }
    rows.truncate(MAX_RESULTS);
    let recent_rows = recent_rows.min(rows.len());
    (rows, recent_rows)
}

// ----------------------------------------------------------------
// Recents persistence (Phase L)
// ----------------------------------------------------------------

thread_local! {
    /// The persisted recents, cached for the life of the
    /// process: loaded once (lazily, on first access) from
    /// `shell::palette_recents`, then kept in step with
    /// every recorded run by [`record_recent`]. A
    /// `thread_local` because the palette's callers all
    /// run on the UI thread, and the sibling panels keep
    /// their process-lifetime state the same way (the
    /// camera panel's clipboard, `clips_panel`'s
    /// selection).
    static RECENTS_CACHE: RefCell<Option<Vec<&'static str>>> = RefCell::new(None);
}

/// Merge a persisted recents list — plain strings, as
/// `shell::palette_recents` returns them — into palette
/// keys: order preserved, strings that no longer name a
/// command in the table dropped, duplicates dropped, and
/// the result capped at [`MAX_RECENT`]. Pure, so the disk
/// format is fully testable without touching the disk.
pub fn merge_persisted(saved: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for name in saved {
        if let Some(cmd) = COMMANDS.iter().find(|c| c.key == name.as_str()) {
            if !out.contains(&cmd.key) {
                out.push(cmd.key);
            }
        }
    }
    out.truncate(MAX_RECENT);
    out
}

/// The persisted form of a recents list: the exact
/// strings [`record_recent`] hands to
/// `shell::save_palette_recents`. [`merge_persisted`] is
/// its inverse over the keys that still exist.
pub fn recent_strings(recent: &[&'static str]) -> Vec<String> {
    recent.iter().map(|key| (*key).to_string()).collect()
}

/// The first cache load: the on-disk recents, merged
/// against the live table. Test builds never touch the
/// disk — the cache starts empty there and the tests
/// seed it explicitly, so no test's outcome can depend
/// on whatever a real run last saved into `./projects`.
fn load_recents_cache() -> Vec<&'static str> {
    #[cfg(test)]
    {
        Vec::new()
    }
    #[cfg(not(test))]
    {
        merge_persisted(&stykanimate_rs::shell::palette_recents())
    }
}

/// The persisted recents for this process, loading them
/// on first access. After a [`record_recent`] this is
/// exactly the working list that was recorded.
pub fn persisted_recents() -> Vec<&'static str> {
    RECENTS_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .get_or_insert_with(load_recents_cache)
            .clone()
    })
}

/// Record a command run — the one write path for recents
/// since Phase L. [`push_recent`] applied to the working
/// list `recent`, the result mirrored into the process
/// cache and written back through
/// `shell::save_palette_recents`, so the list survives a
/// restart. A working list that was never seeded (still
/// empty) is seeded from the cache first, so a run can
/// never clobber earlier sessions' recents. The disk
/// write itself is compiled out of test builds (see
/// [`load_recents_cache`]); there the cache mirror —
/// asserted by the tests — is the observable effect.
pub fn record_recent(recent: &mut Vec<&'static str>, key: &'static str) {
    if recent.is_empty() {
        *recent = persisted_recents();
    }
    push_recent(recent, key);
    RECENTS_CACHE.with(|cache| *cache.borrow_mut() = Some(recent.clone()));
    #[cfg(not(test))]
    stykanimate_rs::shell::save_palette_recents(&recent_strings(recent));
}

/// Test-only: empty the process cache, so a test starts
/// from the no-recents state whatever earlier code on
/// this thread recorded.
#[cfg(test)]
fn reset_recents_cache() {
    RECENTS_CACHE.with(|cache| *cache.borrow_mut() = Some(Vec::new()));
}

/// Test-only: plant the process cache, standing in for a
/// disk load of a previous session's recents.
#[cfg(test)]
fn seed_recents_cache(keys: &[&'static str]) {
    RECENTS_CACHE.with(|cache| *cache.borrow_mut() = Some(keys.to_vec()));
}

/// How many rows the palette currently lists for `s`
/// (recents-aware): the length keyboard navigation wraps
/// within.
fn listed_len(s: &State) -> usize {
    display_commands(&s.palette.query.text, &s.palette.recent).0.len()
}

/// Move a selection index by `delta` within `len` rows,
/// wrapping at both ends; 0 when there are no rows.
pub fn wrap_move(selected: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    (selected as i32 + delta).rem_euclid(len as i32) as usize
}

/// Clamp a (possibly stale) selection index into `len` rows.
pub fn clamped_selected(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        selected.min(len - 1)
    }
}

// ----------------------------------------------------------------
// Palette state
// ----------------------------------------------------------------

/// Everything the palette needs between frames: whether it is
/// open, the query field's edit state, the highlighted
/// result row (an index into the *filtered* list, clamped by
/// [`clamped_selected`] wherever it is read), and the recents
/// list (keys of the last commands run from the palette,
/// most-recent-first — see [`push_recent`]).
pub struct PaletteState {
    pub open: bool,
    pub query: TextEdit,
    pub selected: usize,
    /// Keys of the commands most recently run from the
    /// palette, most-recent-first, capped at [`MAX_RECENT`]
    /// with no duplicates. The working copy of the
    /// persisted recents (Phase L): [`PaletteState::new`]
    /// starts it empty, [`open_palette`] seeds it from
    /// [`persisted_recents`] on the first open of a
    /// process, and every run writes it back through
    /// [`record_recent`] — so it survives restarts as
    /// well as open/close cycles.
    pub recent: Vec<&'static str>,
}

impl PaletteState {
    pub fn new() -> Self {
        PaletteState {
            open: false,
            query: TextEdit::new(""),
            selected: 0,
            recent: Vec::new(),
        }
    }
}

impl Default for PaletteState {
    fn default() -> Self {
        Self::new()
    }
}

/// Open the palette: fresh empty query, first row selected,
/// and the field flagged focused (the launcher example's
/// focus pattern) so typing lands in it immediately. On
/// the first open of a process the recents working list
/// is still empty, so it is seeded from the persisted
/// recents ([`persisted_recents`]) and the previous
/// session's commands lead the list; a non-empty list is
/// this session's own — at least as new as the cache —
/// and is never overwritten.
pub fn open_palette(s: &mut State) {
    if s.palette.recent.is_empty() {
        s.palette.recent = persisted_recents();
    }
    s.palette.open = true;
    s.palette.query.clear();
    s.palette.selected = 0;
    s.palette.query.focused = true;
}

/// Close the palette and release the field's focus flag.
pub fn close_palette(s: &mut State) {
    s.palette.open = false;
    s.palette.query.focused = false;
}

/// Run the currently highlighted command against the shell
/// state and close the palette. A stale selection (query
/// changed under it) is clamped first; an empty result
/// list just closes. The run is recorded in the recents
/// list (and persisted — see [`record_recent`]).
pub fn run_selected(s: &mut State) {
    let (results, _) = display_commands(&s.palette.query.text, &s.palette.recent);
    let row = clamped_selected(s.palette.selected, results.len());
    if let Some(&ci) = results.get(row) {
        (COMMANDS[ci].run)(s);
        record_recent(&mut s.palette.recent, COMMANDS[ci].key);
    }
    close_palette(s);
}

/// Run the command at table index `ci` (a result-row press)
/// and close the palette. The run is recorded in the recents
/// list (and persisted — see [`record_recent`]).
fn run_command(s: &mut State, ci: usize) {
    if let Some(cmd) = COMMANDS.get(ci) {
        (cmd.run)(s);
        record_recent(&mut s.palette.recent, cmd.key);
    }
    close_palette(s);
}

/// App-level key routing for the palette, for the integrator
/// to call at the top of `AppSpec::on_key`: Ctrl+K opens it
/// while closed; while open, Down/Up move the highlight,
/// Enter runs the selection and Escape closes. Returns true
/// when the key was consumed. (Field-focused typing is
/// handled by the field's own handlers in [`palette_view`];
/// this covers the case where focus sits elsewhere.)
pub fn palette_handle_key(s: &mut State, ev: &KeyEvent) -> bool {
    if !s.palette.open {
        if ev.is_command('k') {
            open_palette(s);
            return true;
        }
        return false;
    }
    match ev.key {
        Key::Down => {
            let len = listed_len(s);
            s.palette.selected = wrap_move(s.palette.selected, len, 1);
            true
        }
        Key::Up => {
            let len = listed_len(s);
            s.palette.selected = wrap_move(s.palette.selected, len, -1);
            true
        }
        Key::Enter => {
            run_selected(s);
            true
        }
        Key::Escape => {
            close_palette(s);
            true
        }
        _ => false,
    }
}

/// The query field's own key handler (launcher pattern):
/// Down/Up move the highlight, Escape closes. Enter is *not*
/// handled here — a single-line field's plain Return routes
/// to `on_submit`, which runs the selection.
fn palette_field_key(s: &mut State, ev: &KeyEvent) -> bool {
    match ev.key {
        Key::Down => {
            let len = listed_len(s);
            s.palette.selected = wrap_move(s.palette.selected, len, 1);
            true
        }
        Key::Up => {
            let len = listed_len(s);
            s.palette.selected = wrap_move(s.palette.selected, len, -1);
            true
        }
        Key::Escape => {
            close_palette(s);
            true
        }
        _ => false,
    }
}

/// The keyboard shortcut a command duplicates, if any
/// (Phase N, P14 §4.4): the 17 commands whose action is
/// also bound to a real key in `main.rs::on_key`, so the
/// result row can teach the key at the moment of
/// discovery via a trailing `design::kbd` chip. Keyed by
/// *command* key — a different namespace from
/// `menus::shortcut_hint`'s menu-item keys, so the two
/// tables stay separate. Spellings mirror
/// `help_panel::shortcut_rows` exactly; the drift-guard
/// test below fails if a spelling stops naming a real
/// binding there. Commands not listed here have no key
/// binding and must not grow a chip — no invented hints.
fn command_hint(key: &str) -> Option<&'static str> {
    Some(match key {
        "play" => "Space",
        "prev" => "Left",
        "next" => "Right",
        "add-frame" => "N",
        "inbetween" => "I",
        "undo" => "Ctrl+Z",
        "redo" => "Ctrl+Shift+Z",
        "save" => "Ctrl+S",
        "onion" => "O",
        "go-animate" => "Alt+1",
        "go-draw" => "Alt+2",
        "go-graph" => "Alt+3",
        "go-effects" => "Alt+4",
        "go-audio" => "Alt+5",
        "go-build3d" => "Alt+6",
        "go-export" => "Alt+7",
        "go-home" => "Alt+8",
        _ => return None,
    })
}

// ----------------------------------------------------------------
// View
// ----------------------------------------------------------------

/// The palette panel: a query field over a ranked result list
/// (title left, group right; the highlighted row is
/// accent-filled). With an empty query the recents lead under
/// a "Recent" heading, followed by the full list under "All
/// commands". Self-contained — the integrator renders it
/// while `state.palette.open`, typically as an overlay above
/// the main column.
pub fn palette_view(s: &State, t: &Theme) -> El {
    let accent = tok::accent(0, true);
    let (results, recent_rows) = display_commands(&s.palette.query.text, &s.palette.recent);
    let selected = clamped_selected(s.palette.selected, results.len());

    let field = design::text_field(&s.palette.query, type_::BODY.lh(26.0), s.palette.query.focused, t)
        .key("palette-query")
        .single_line()
        .grow(1.0)
        .color(t.text)
        .placeholder("Type a command…", t.text_secondary)
        .on_edit(|s: &mut State| &mut s.palette.query)
        .on_input_change(|s: &mut State, _tx: &mut Tx| {
            s.palette.selected = 0;
        })
        .on_input_key(|s: &mut State, ev: &KeyEvent, _tx: &mut Tx| palette_field_key(s, ev))
        .on_submit(|s: &mut State, _tx: &mut Tx| run_selected(s));

    let mut panel = col()
        .key("command-palette")
        .w(540.0)
        .radius(12.0)
        .clip_rounded(12.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .transition(
            Transition::offset(0.0, -4.0)
                .combined(Transition::opacity())
                .springs(tok::motion::pair::SNAP),
        )
        .child(
            row()
                .h(52.0)
                .px(16.0)
                .gap(10.0)
                .center_y()
                .child(icons::icon_el("magnifying-glass", 16.0, t.text_secondary))
                .child(field),
        );

    if results.is_empty() {
        panel = panel.child(design::empty_state(
            "magnifying-glass",
            "No commands match",
            t,
        ));
    } else {
        let mut list = col().padding(6.0).gap(2.0);
        for (row_i, &ci) in results.iter().enumerate() {
            if recent_rows > 0 && row_i == 0 {
                list = list.child(
                    row()
                        .h(22.0)
                        .px(8.0)
                        .center_y()
                        .child(design::section_label("Recent", t)),
                );
            }
            if recent_rows > 0 && row_i == recent_rows {
                list = list.child(
                    row()
                        .h(22.0)
                        .px(8.0)
                        .center_y()
                        .child(design::section_label("All commands", t)),
                );
            }
            let cmd = &COMMANDS[ci];
            let on = row_i == selected;
            let mut result_row = row()
                .key(format!("cmd-{}", cmd.key))
                .h(34.0)
                .px(8.0)
                .radius(8.0)
                .center_y()
                .gap(8.0)
                .fill(if on { accent.alpha(0.85) } else { Color::TRANSPARENT })
                .hover(if on { accent.alpha(0.85) } else { t.control_track })
                // Wave P (P4): the press state the btn family
                // derives — full accent on the selected row,
                // `control_off` (one step past the hover) off
                // it. The row carried fill + hover only, so a
                // press gave no visual answer.
                .press(if on { accent } else { t.control_off })
                .on_press(move |s: &mut State| run_command(s, ci))
                .child(
                    text(cmd.title)
                        .font(type_::BODY)
                        .color(if on { Color::WHITE } else { t.text })
                        .grow(1.0),
                )
                .child(
                    text(cmd.group)
                        .font(type_::LABEL)
                        .color(if on { Color::WHITE } else { t.text_secondary }),
                );
            if let Some(hint) = command_hint(cmd.key) {
                result_row = result_row.child(design::kbd(hint, t));
            }
            list = list.child(result_row);
        }
        panel = panel.child(el().h(1.0).w_full().fill(t.separator)).child(list);
    }

    panel.child(
        row()
            .h(26.0)
            .px(16.0)
            .gap(6.0)
            .center_y()
            .child(design::kbd("↑↓", t))
            .child(text("navigate").font(type_::LABEL).color(t.text_secondary))
            .child(text("·").font(type_::LABEL).color(t.text_secondary))
            .child(design::kbd("Enter", t))
            .child(text("run").font(type_::LABEL).color(t.text_secondary))
            .child(text("·").font(type_::LABEL).color(t.text_secondary))
            .child(design::kbd("Esc", t))
            .child(text("close").font(type_::LABEL).color(t.text_secondary)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx_of(key: &str) -> usize {
        COMMANDS
            .iter()
            .position(|c| c.key == key)
            .unwrap_or_else(|| panic!("no command with key {key}"))
    }

    // -- Table hygiene -----------------------------------------

    #[test]
    fn command_keys_are_unique_and_fields_non_empty() {
        let mut keys: Vec<&str> = COMMANDS.iter().map(|c| c.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(total, keys.len(), "duplicate command keys");
        // 77 commands through Phase F, +4 in Phase G
        // (restart-playback, particle-add-event,
        // cam-follow-clear, key-all-channels), +3 in
        // Phase H (toggle-loop-playback,
        // stroke-select-next, stroke-select-prev — the
        // two Phase G skips, unblocked by the dispatch
        // migration to fn(&mut State)), +13 in Phase I
        // (the eight go-* workspace jumps, toggle-guides,
        // toggle-frustum, toggle-safe-frame,
        // toggle-left-panel, toggle-right-panel — a
        // toggle-onion candidate was skipped because the
        // onion toggle already exists as `onion`), +6 in
        // Phase J (split-fx-clip, mirror-figure,
        // normalise-audio, duplicate-particle,
        // delete-stroke, delete-particle — the four
        // selection-taking ones resolve the shell
        // selections at call time and stand down with an
        // explanation when nothing is selected), +5 in
        // Phase M (mute-audio-clip, swap-frame-prev,
        // swap-frame-next, shift-channel-right,
        // fill-keys-range — the mute resolves the audio
        // panel's target, the graph pair the graph
        // panel's drawn channel, and both stand down
        // with an explanation when there is no target),
        // +3 in Wave 9 (toggle-live-mcp, theme-light,
        // theme-dark — the Live MCP toggle names the
        // settings handler directly; the theme pair
        // forward to settings_popover::set_theme with
        // the settings buttons' own arguments. The
        // go-* workspace jumps were already complete
        // since Phase I), +1 at Wave 9 integration
        // (save-copy — names project_tools'
        // save_copy_pressed directly; E6 had skipped it
        // because no State-level handler existed at its
        // edit time, and D5's handler landed after), +2
        // in Wave 10 (prev-keyframe, next-keyframe — the
        // selected figure's pose keyframes from
        // stats::figure_keyframes, stepped with the
        // playback panel's strict prev_key / next_key and
        // jumped via core.goto; the no-key side reports
        // in the camera panel's wording family. A
        // toggle-dopesheet candidate was skipped: the
        // shell has no dopesheet lane or visibility state
        // for a command to flip), +15 in Wave 12
        // (open-style-inspector, the six style-preset-*
        // commands over the canonical STYLE_PRESETS
        // table, style-cap-round / -square / -point,
        // cycle-taper, style-opacity-up / -down,
        // copy-style, paste-style — every one forwarding
        // to the style panel's own handler or the core
        // op its control presses), +3 at Wave 12
        // integration (attach-media-import,
        // clear-node-media, cycle-attached-sprite — the
        // asset entries C5 specced, added once B2's
        // segment-inspector Media handlers had landed as
        // callable fns: the first two name the core ops
        // the panel's buttons press, the third composes
        // the panel's own next_asset_target / attach_asset
        // helpers exactly as its Next sprite closure does),
        // +2 in Wave 13 (add-sprite, remove-sprite — A1's
        // sprite-instance ops over the first asset and the
        // last instance respectively. A next-sprite cycle
        // candidate was skipped: A1's ops take explicit
        // indices and neither the core nor the shell State
        // carries a sprite selection for it to advance, so
        // the command would have nothing real to run),
        // +1 in Wave Y (import-layered-set — Y6's GUI twin
        // of the `import_layered_set` MCP tool over Y5's
        // layered-set importer: the shim resolves the set
        // staged in ./import and forwards to the same
        // `import_layered_folder` entry the tool calls).
        assert_eq!(total, 135, "palette should cover 135 commands, has {total}");
        for c in COMMANDS {
            assert!(!c.title.is_empty() && !c.group.is_empty(), "empty field on {}", c.key);
        }
    }

    #[test]
    fn command_runs_are_wired_to_real_ops() {
        // Smoke: a few safe commands actually change core state
        // through the table's fn pointers (Phase H: the
        // pointers take the shell State; assertions read
        // its core).
        let mut s = crate::init();
        let frames = s.core.frame_count();
        (COMMANDS[idx_of("add-frame")].run)(&mut s);
        assert_eq!(s.core.frame_count(), frames + 1);
        (COMMANDS[idx_of("play")].run)(&mut s);
        assert!(s.core.playing);
        (COMMANDS[idx_of("play")].run)(&mut s);
        assert!(!s.core.playing);
        // Onion defaults to ON in EditorCore::new(), so the
        // command must flip it off and back on.
        assert!(s.core.onion);
        (COMMANDS[idx_of("onion")].run)(&mut s);
        assert!(!s.core.onion);
        (COMMANDS[idx_of("onion")].run)(&mut s);
        assert!(s.core.onion);
        // Undo on a fresh shell is a safe no-op.
        let mut fresh = crate::init();
        (COMMANDS[idx_of("undo")].run)(&mut fresh);
    }

    #[test]
    fn phase_e_commands_are_present_and_wired() {
        use stykanimate_rs::model::{AudioClip, Stroke};
        // All five Phase E keys resolve (idx_of panics if not).
        for key in [
            "duplicate-figure",
            "delete-last-stroke",
            "fade-audio",
            "reverse-audio",
            "delete-audio-clip",
        ] {
            idx_of(key);
        }
        // Duplicate figure: the demo project starts with one
        // figure; the shim passes the selection to the op.
        let mut s = crate::init();
        let figures = s.core.project.figures.len();
        (COMMANDS[idx_of("duplicate-figure")].run)(&mut s);
        assert_eq!(s.core.project.figures.len(), figures + 1);
        // Delete last stroke: the empty guard stands down,
        // then a real run removes only the newest stroke.
        let mut s = crate::init();
        (COMMANDS[idx_of("delete-last-stroke")].run)(&mut s);
        assert!(s.core.project.strokes.is_empty());
        let stroke = |color: u32| Stroke {
            frame: 0,
            points: vec![[0.0, 0.0]],
            color,
            width: 1.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        };
        s.core.project.strokes.push(stroke(0x111111));
        s.core.project.strokes.push(stroke(0x222222));
        (COMMANDS[idx_of("delete-last-stroke")].run)(&mut s);
        assert_eq!(s.core.project.strokes.len(), 1);
        assert_eq!(s.core.project.strokes[0].color, 0x111111);
        // The audio trio on a clipless core: every one is a
        // status-only no-op (the *_last ops self-guard, the
        // delete shim's own guard stands down first).
        let mut s = crate::init();
        (COMMANDS[idx_of("fade-audio")].run)(&mut s);
        (COMMANDS[idx_of("reverse-audio")].run)(&mut s);
        (COMMANDS[idx_of("delete-audio-clip")].run)(&mut s);
        assert!(s.core.project.audio.is_empty());
        // Delete last clip removes exactly the newest entry.
        s.core.project.audio.push(AudioClip {
            path: "a.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.core.project.audio.push(AudioClip {
            path: "b.wav".into(),
            start_frame: 4,
            gain: 1.0,
            muted: false,
        });
        (COMMANDS[idx_of("delete-audio-clip")].run)(&mut s);
        assert_eq!(s.core.project.audio.len(), 1);
        assert_eq!(s.core.project.audio[0].path, "a.wav");
    }

    #[test]
    fn phase_g_commands_are_present_and_wired() {
        use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
        // All four Phase G keys resolve (idx_of panics if not).
        for key in [
            "restart-playback",
            "particle-add-event",
            "cam-follow-clear",
            "key-all-channels",
        ] {
            idx_of(key);
        }
        // Restart: from the last frame, stopped, the command
        // lands on frame 0 playing — the playback panel's
        // pb-restart body (`goto(0)` + `playing = true`).
        let mut s = crate::init();
        let n = s.core.frame_count();
        assert!(n > 1, "demo project needs 2+ frames for this test");
        s.core.goto(n - 1);
        s.core.playing = false;
        (COMMANDS[idx_of("restart-playback")].run)(&mut s);
        assert_eq!(s.core.current, 0);
        assert!(s.core.playing);
        // Particle event: exactly one new event, on the
        // current frame, at the stage centre — the
        // particles panel's own add arguments.
        let mut s = crate::init();
        let frame = s.core.current;
        let [w, h] = s.core.project.stage;
        let before = s.core.project.particles.len();
        (COMMANDS[idx_of("particle-add-event")].run)(&mut s);
        assert_eq!(s.core.project.particles.len(), before + 1);
        let ev = &s.core.project.particles[before];
        assert_eq!(ev.frame, frame);
        assert!((ev.x - w / 2.0).abs() < 1e-4, "event x {} != centre {}", ev.x, w / 2.0);
        assert!((ev.y - h / 2.0).abs() < 1e-4, "event y {} != centre {}", ev.y, h / 2.0);
        // Camera follow clear: the field goes to None with
        // camera_panel::follow_clear's own status wording.
        let mut s = crate::init();
        s.core.project.camera_follow = Some(0);
        (COMMANDS[idx_of("cam-follow-clear")].run)(&mut s);
        assert_eq!(s.core.project.camera_follow, None);
        assert_eq!(s.core.status, "Camera follow off.");
        // Key all channels: with no channels the helper is
        // a status-only refusal; with one channel the
        // current frame gains a key holding the evaluated
        // value, as one undo step.
        let mut s = crate::init();
        s.core.project.channels.clear();
        (COMMANDS[idx_of("key-all-channels")].run)(&mut s);
        assert_eq!(s.core.status, "No graph channels to key.");
        s.core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![Key::flat(0.0, 10.0), Key::flat(2.0, 30.0)],
        });
        if s.core.frame_count() > 1 {
            s.core.goto(1);
        }
        let at = s.core.current as f32;
        (COMMANDS[idx_of("key-all-channels")].run)(&mut s);
        assert_eq!(
            s.core.status,
            format!("Keyed 1 channels at frame {}.", s.core.current + 1)
        );
        let keys = &s.core.project.channels[0].keys;
        assert!(
            keys.iter().any(|k| (k.frame - at).abs() < 1e-6),
            "no key at frame {at}: {keys:?}"
        );
    }

    #[test]
    fn phase_h_commands_are_present_and_wired() {
        // All three Phase H keys resolve (idx_of panics if not).
        for key in ["toggle-loop-playback", "stroke-select-next", "stroke-select-prev"] {
            idx_of(key);
        }
        // Loop toggle: flips the shell flag and nothing
        // else — the pb-loop handler's exact behaviour —
        // and a second run flips it back. The status line
        // is untouched (no toast spam).
        let mut s = crate::init();
        let before = s.loop_playback;
        let status = s.core.status.clone();
        (COMMANDS[idx_of("toggle-loop-playback")].run)(&mut s);
        assert_eq!(s.loop_playback, !before);
        assert_eq!(s.core.status, status, "loop toggle must not set a status");
        (COMMANDS[idx_of("toggle-loop-playback")].run)(&mut s);
        assert_eq!(s.loop_playback, before);
        assert_eq!(s.core.status, status);
        // Stroke stepping on an empty project (the demo
        // has no strokes): the selection stays unset and
        // the status is the stroke panel's own refusal —
        // proof the command delegates to
        // step_stroke_selection rather than reimplementing.
        let mut s = crate::init();
        assert!(s.core.project.strokes.is_empty());
        (COMMANDS[idx_of("stroke-select-next")].run)(&mut s);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
        (COMMANDS[idx_of("stroke-select-prev")].run)(&mut s);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
    }

    #[test]
    fn phase_i_commands_are_present_and_wired() {
        // All thirteen Phase I keys resolve (idx_of panics if not).
        for key in [
            "go-animate",
            "go-draw",
            "go-graph",
            "go-effects",
            "go-audio",
            "go-build3d",
            "go-export",
            "go-home",
            "toggle-guides",
            "toggle-frustum",
            "toggle-safe-frame",
            "toggle-left-panel",
            "toggle-right-panel",
        ] {
            idx_of(key);
        }
        // Workspace jumps: each command lands on its own
        // workspace. The state is first parked on a
        // different workspace, so the landing is the
        // command's doing, not the seed's. (The run also
        // persists the choice through select_workspace,
        // like a chip press; the assertion reads the
        // in-memory field the switch fn sets first.)
        let jumps: [(&str, Workspace); 8] = [
            ("go-animate", Workspace::Animate),
            ("go-draw", Workspace::Draw),
            ("go-graph", Workspace::Graph),
            ("go-effects", Workspace::Effects),
            ("go-audio", Workspace::Audio),
            ("go-build3d", Workspace::Build3D),
            ("go-export", Workspace::Export),
            ("go-home", Workspace::Home),
        ];
        for (key, ws) in jumps {
            let mut s = crate::init();
            s.workspace = if ws == Workspace::Animate {
                Workspace::Home
            } else {
                Workspace::Animate
            };
            (COMMANDS[idx_of(key)].run)(&mut s);
            assert_eq!(s.workspace, ws, "{key} landed on the wrong workspace");
        }
        // Guide toggles: each flips its own shell flag
        // and flips back, leaving the other two flags
        // exactly as they were.
        let mut s = crate::init();
        let g = (s.guides, s.guides_frustum, s.guides_safe);
        (COMMANDS[idx_of("toggle-guides")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), (!g.0, g.1, g.2));
        (COMMANDS[idx_of("toggle-guides")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), g);
        (COMMANDS[idx_of("toggle-frustum")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), (g.0, !g.1, g.2));
        (COMMANDS[idx_of("toggle-frustum")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), g);
        (COMMANDS[idx_of("toggle-safe-frame")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), (g.0, g.1, !g.2));
        (COMMANDS[idx_of("toggle-safe-frame")].run)(&mut s);
        assert_eq!((s.guides, s.guides_frustum, s.guides_safe), g);
        // Panel toggles: the View menu's field flips —
        // each column flips independently and flips back.
        let mut s = crate::init();
        let (l, r) = (s.show_left, s.show_right);
        (COMMANDS[idx_of("toggle-left-panel")].run)(&mut s);
        assert_eq!((s.show_left, s.show_right), (!l, r));
        (COMMANDS[idx_of("toggle-right-panel")].run)(&mut s);
        assert_eq!((s.show_left, s.show_right), (!l, !r));
        (COMMANDS[idx_of("toggle-left-panel")].run)(&mut s);
        (COMMANDS[idx_of("toggle-right-panel")].run)(&mut s);
        assert_eq!((s.show_left, s.show_right), (l, r));
    }

    #[test]
    fn phase_j_commands_are_present_and_wired() {
        use stykanimate_rs::model::{FxClipEntry, Stroke};
        use stykanimate_rs::vfx::ParticleClip;
        // All six Phase J keys resolve (idx_of panics if not).
        for key in [
            "split-fx-clip",
            "mirror-figure",
            "normalise-audio",
            "duplicate-particle",
            "delete-stroke",
            "delete-particle",
        ] {
            idx_of(key);
        }
        // Split FX clip: with nothing selected the shim
        // stands down with an explanation and touches
        // nothing; with a clip selected, the playhead
        // frame splits it into two clips that between
        // them span the original.
        let mut s = crate::init();
        (COMMANDS[idx_of("split-fx-clip")].run)(&mut s);
        assert_eq!(s.core.status, "No FX clip selected to split.");
        assert!(s.core.project.fx_clips.is_empty());
        s.core.project.fx_clips.push(FxClipEntry {
            clip: ParticleClip {
                start: 0.0,
                duration: 10.0,
                kind: ClipKind::Burst,
                color: 0xFF5D5D,
                color2: None,
                seed: 1,
                angle: 0.0,
                length: 100.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        });
        s.fx_selected = Some(0);
        s.core.goto(2);
        (COMMANDS[idx_of("split-fx-clip")].run)(&mut s);
        assert_eq!(s.core.project.fx_clips.len(), 2);
        assert_eq!(s.core.project.fx_clips[0].clip.duration, 2.0);
        assert_eq!(s.core.project.fx_clips[1].clip.start, 2.0);
        assert_eq!(s.core.project.fx_clips[1].clip.duration, 8.0);
        // Mirror figure: the shim passes the selected
        // figure to the all-frames op, whose own status
        // reports the mirror.
        let mut s = crate::init();
        (COMMANDS[idx_of("mirror-figure")].run)(&mut s);
        assert!(
            s.core.status.contains("mirrored on all"),
            "unexpected status: {}",
            s.core.status
        );
        // Delete stroke: no selection is an explaining
        // no-op; a selection deletes exactly that stroke
        // and clears the selection it consumed.
        let mut s = crate::init();
        (COMMANDS[idx_of("delete-stroke")].run)(&mut s);
        assert_eq!(s.core.status, "No stroke selected to delete.");
        let stroke = |color: u32| Stroke {
            frame: 0,
            points: vec![[0.0, 0.0]],
            color,
            width: 1.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        };
        s.core.project.strokes.push(stroke(0x111111));
        s.core.project.strokes.push(stroke(0x222222));
        s.stroke_selected = Some(0);
        (COMMANDS[idx_of("delete-stroke")].run)(&mut s);
        assert_eq!(s.core.project.strokes.len(), 1);
        assert_eq!(s.core.project.strokes[0].color, 0x222222);
        assert_eq!(s.stroke_selected, None);
        // Duplicate particle: no selection explains; a
        // selection duplicates the event and selects
        // the copy the op reports.
        let mut s = crate::init();
        (COMMANDS[idx_of("duplicate-particle")].run)(&mut s);
        assert_eq!(s.core.status, "No particle event selected to duplicate.");
        let first = s.core.particle_add_event(0, 10.0, 10.0);
        s.particle_selected = Some(first);
        (COMMANDS[idx_of("duplicate-particle")].run)(&mut s);
        assert_eq!(s.core.project.particles.len(), 2);
        assert_eq!(s.particle_selected, Some(first + 1));
        // Delete particle: no selection is an explaining
        // no-op; a selection deletes exactly that event
        // and clears the selection it consumed.
        let mut s = crate::init();
        (COMMANDS[idx_of("delete-particle")].run)(&mut s);
        assert_eq!(s.core.status, "No particle event selected to delete.");
        let a = s.core.particle_add_event(0, 10.0, 10.0);
        s.core.particle_add_event(0, 20.0, 20.0);
        s.particle_selected = Some(a);
        (COMMANDS[idx_of("delete-particle")].run)(&mut s);
        assert_eq!(s.core.project.particles.len(), 1);
        assert_eq!(s.core.project.particles[0].x, 20.0);
        assert_eq!(s.particle_selected, None);
    }

    #[test]
    fn phase_m_commands_are_present_and_wired() {
        use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
        use stykanimate_rs::model::AudioClip;
        // All five Phase M keys resolve (idx_of panics if not).
        for key in [
            "mute-audio-clip",
            "swap-frame-prev",
            "swap-frame-next",
            "shift-channel-right",
            "fill-keys-range",
        ] {
            idx_of(key);
        }
        // Each new command is findable by a query fragment
        // of its key (the key field is scored too, so a
        // dashed fragment reaches the command).
        for (query, key) in [
            ("mute-audio", "mute-audio-clip"),
            ("swap-frame-p", "swap-frame-prev"),
            ("swap-frame-n", "swap-frame-next"),
            ("shift-channel", "shift-channel-right"),
            ("fill-keys", "fill-keys-range"),
        ] {
            let got = filter_commands(query);
            assert!(
                got.iter().any(|&i| COMMANDS[i].key == key),
                "query {query:?} should find {key}, got {:?}",
                got.iter().map(|&i| COMMANDS[i].key).collect::<Vec<_>>()
            );
        }
        // Mute: with no clips the shim stands down with an
        // explanation and touches nothing; with a clip
        // present, the panel resolution (no selection →
        // last clip) reaches it and the op flips its mute
        // flag, twice.
        let mut s = crate::init();
        (COMMANDS[idx_of("mute-audio-clip")].run)(&mut s);
        assert_eq!(s.core.status, "No audio clip selected to mute.");
        assert!(s.core.project.audio.is_empty());
        s.core.project.audio.push(AudioClip {
            path: "clip.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        (COMMANDS[idx_of("mute-audio-clip")].run)(&mut s);
        assert!(s.core.project.audio[0].muted);
        (COMMANDS[idx_of("mute-audio-clip")].run)(&mut s);
        assert!(!s.core.project.audio[0].muted);
        // Swap frame next/prev on a three-frame fixture
        // whose frames are marked by duration: the slots'
        // contents trade places while the playhead index
        // itself never moves (the op's documented
        // semantics). At the first frame, swap-prev is
        // the op's own refusal and nothing moves.
        let durations = |s: &State| -> Vec<f32> {
            s.core.project.frames.iter().map(|f| f.duration).collect()
        };
        let mut s = crate::init();
        s.core.project.frames.truncate(3);
        for (i, f) in s.core.project.frames.iter_mut().enumerate() {
            f.duration = (i + 1) as f32;
        }
        s.core.goto(0);
        (COMMANDS[idx_of("swap-frame-next")].run)(&mut s);
        assert_eq!(durations(&s), [2.0, 1.0, 3.0]);
        assert_eq!(s.core.current, 0);
        (COMMANDS[idx_of("swap-frame-prev")].run)(&mut s);
        assert_eq!(durations(&s), [2.0, 1.0, 3.0]);
        s.core.goto(1);
        (COMMANDS[idx_of("swap-frame-prev")].run)(&mut s);
        assert_eq!(durations(&s), [1.0, 2.0, 3.0]);
        assert_eq!(s.core.current, 1);
        // Shift channel right: with no channels the shim
        // stands down with an explanation; with a channel
        // drawn (the panel state picks it, the resolution
        // the command reuses) every key moves one frame
        // later.
        let mut s = crate::init();
        (COMMANDS[idx_of("shift-channel-right")].run)(&mut s);
        assert_eq!(s.core.status, "No graph channel selected to shift.");
        s.core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0), Key::flat(2.0, 10.0)],
        });
        s.graph_panel.selected_channel = Some(0);
        (COMMANDS[idx_of("shift-channel-right")].run)(&mut s);
        let frames: Vec<f32> =
            s.core.project.channels[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, [1.0, 3.0]);
        // Fill keys: the loop-range guard answers first;
        // with a range set and the channel drawn, the
        // range's keyless frames gain copies of the key
        // at the current frame (frame 0 here).
        let mut s = crate::init();
        (COMMANDS[idx_of("fill-keys-range")].run)(&mut s);
        assert_eq!(s.core.status, "Set a loop range first, then Fill keys.");
        s.core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 5.0)],
        });
        s.graph_panel.selected_channel = Some(0);
        s.core.loop_range = Some((0, 2));
        (COMMANDS[idx_of("fill-keys-range")].run)(&mut s);
        assert_eq!(s.core.project.channels[0].keys.len(), 3);
    }

    #[test]
    fn wave_9_commands_are_present_and_wired() {
        // All four Wave 9 keys resolve (idx_of panics if not).
        for key in ["toggle-live-mcp", "theme-light", "theme-dark", "save-copy"] {
            idx_of(key);
        }
        // Each new command is findable by a query fragment
        // of its key (the key field is scored too, so a
        // dashed fragment reaches the command).
        for (query, key) in [
            ("toggle-live", "toggle-live-mcp"),
            ("theme-l", "theme-light"),
            ("theme-d", "theme-dark"),
            ("save-c", "save-copy"),
        ] {
            let got = filter_commands(query);
            assert!(
                got.iter().any(|&i| COMMANDS[i].key == key),
                "query {query:?} should find {key}, got {:?}",
                got.iter().map(|&i| COMMANDS[i].key).collect::<Vec<_>>()
            );
        }
        // Theme pair: each command writes the core's
        // theme field through settings_popover::set_theme
        // with its own direction, and re-running the
        // active direction is the setter's documented
        // no-op (idempotent), not a toggle.
        let mut s = crate::init();
        (COMMANDS[idx_of("theme-dark")].run)(&mut s);
        assert!(!s.core.light_theme);
        (COMMANDS[idx_of("theme-light")].run)(&mut s);
        assert!(s.core.light_theme);
        (COMMANDS[idx_of("theme-light")].run)(&mut s);
        assert!(s.core.light_theme, "set_theme is idempotent");
        (COMMANDS[idx_of("theme-dark")].run)(&mut s);
        assert!(!s.core.light_theme);
        // toggle-live-mcp is deliberately NOT run here:
        // its handler binds the real loopback port and
        // persists LayoutPrefs, so it is covered by the
        // settings popover's own tests and the live
        // bridge's socket test, not by a palette smoke.
        // Presence + findability above are the palette's
        // contract for it. save-copy is likewise not run:
        // its handler writes a real file into ./projects,
        // which tests must not pollute — project_tools'
        // own tests cover the factored pieces.
    }

    #[test]
    fn wave_10_commands_are_present_and_wired() {
        // Both Wave 10 keys resolve (idx_of panics if not).
        for key in ["prev-keyframe", "next-keyframe"] {
            idx_of(key);
        }
        // Findable by the plain query "keyframe": the
        // word sits whole in both titles (and in both
        // keys, which are scored too).
        let got = filter_commands("keyframe");
        for key in ["prev-keyframe", "next-keyframe"] {
            assert!(
                got.iter().any(|&i| COMMANDS[i].key == key),
                "query \"keyframe\" should find {key}, got {:?}",
                got.iter().map(|&i| COMMANDS[i].key).collect::<Vec<_>>()
            );
        }
        // Wiring: the demo project's figure 0 is posed on
        // every frame and changes on each, so copying
        // frame 0's pose over frames 1 and 2 leaves its
        // keyframes at exactly [0, 3, 4] under the
        // dopesheet rule stats::figure_keyframes
        // implements. The premise is asserted first, so a
        // demo change fails here rather than downstream.
        let mut s = crate::init();
        let pose0 = s.core.project.frames[0].poses[0].clone();
        s.core.project.frames[1].poses[0] = pose0.clone();
        s.core.project.frames[2].poses[0] = pose0;
        assert_eq!(
            stykanimate_rs::stats::figure_keyframes(&s.core.project, s.core.selected_figure),
            vec![0, 3, 4]
        );
        // At the first key, Prev has nowhere to go: the
        // playhead stays and the refusal is the camera
        // panel's wording family — a classify_status Warn.
        s.core.goto(0);
        (COMMANDS[idx_of("prev-keyframe")].run)(&mut s);
        assert_eq!(s.core.current, 0, "no key before frame 0: the playhead stays");
        assert_eq!(s.core.status, "No keyframe before this frame.");
        // Next walks the key list strictly: 0 -> 3 -> 4.
        (COMMANDS[idx_of("next-keyframe")].run)(&mut s);
        assert_eq!(s.core.current, 3);
        (COMMANDS[idx_of("next-keyframe")].run)(&mut s);
        assert_eq!(s.core.current, 4);
        // At the last key, Next refuses the same way.
        (COMMANDS[idx_of("next-keyframe")].run)(&mut s);
        assert_eq!(s.core.current, 4, "no key after the last key: the playhead stays");
        assert_eq!(s.core.status, "No keyframe after this frame.");
        // Prev from the last key steps back to the key
        // before it (standing on a key is not a target).
        (COMMANDS[idx_of("prev-keyframe")].run)(&mut s);
        assert_eq!(s.core.current, 3);
    }

    #[test]
    fn wave_12_style_commands_are_present_and_wired() {
        use stykanimate_rs::style::STYLE_PRESETS;
        // All fifteen Wave 12 keys resolve (idx_of panics if not).
        for key in [
            "open-style-inspector",
            "style-preset-ink",
            "style-preset-crimson",
            "style-preset-ocean",
            "style-preset-forest",
            "style-preset-violet",
            "style-preset-amber",
            "style-cap-round",
            "style-cap-square",
            "style-cap-point",
            "cycle-taper",
            "style-opacity-up",
            "style-opacity-down",
            "copy-style",
            "paste-style",
        ] {
            idx_of(key);
        }
        // Preset pairing: entry i is titled with the
        // canonical table's name i — verbatim, so the
        // palette can never advertise a preset the table
        // does not hold — and running it applies exactly
        // that preset through the panel's apply_preset:
        // the preset's colour and cap, and the status
        // naming the preset.
        let preset_keys = [
            "style-preset-ink",
            "style-preset-crimson",
            "style-preset-ocean",
            "style-preset-forest",
            "style-preset-violet",
            "style-preset-amber",
        ];
        for (i, key) in preset_keys.iter().enumerate() {
            let cmd = &COMMANDS[idx_of(key)];
            assert_eq!(
                cmd.title,
                format!("Style preset: {}", STYLE_PRESETS[i].name),
                "{key} title must name canonical preset {i}"
            );
            let mut s = crate::init();
            s.core.selected_node = Some(1);
            (cmd.run)(&mut s);
            let node = &s.core.project.figures[0].def.nodes[1];
            assert_eq!(node.color, Some(STYLE_PRESETS[i].color), "{key} colour");
            assert_eq!(node.style.cap, STYLE_PRESETS[i].cap, "{key} cap");
            assert_eq!(
                s.core.status,
                format!("Style preset: {}.", STYLE_PRESETS[i].name),
                "{key} status"
            );
        }
        // Findability: the plain word reaches the presets
        // (it sits whole in every preset title).
        let got = filter_commands("preset");
        for key in preset_keys {
            assert!(
                got.iter().any(|&i| COMMANDS[i].key == key),
                "query \"preset\" should find {key}, got {:?}",
                got.iter().map(|&i| COMMANDS[i].key).collect::<Vec<_>>()
            );
        }
        // No selection (the core's default): a preset
        // command stands down with the style setters'
        // refusal and changes nothing — apply_preset
        // itself refuses silently, so this status is the
        // shim's, and the node's colour stays unset.
        let mut s = crate::init();
        assert_eq!(s.core.selected_node, None);
        (COMMANDS[idx_of("style-preset-ink")].run)(&mut s);
        assert_eq!(s.core.status, "Select a joint first.");
        assert_eq!(s.core.project.figures[0].def.nodes[1].color, None);
        // Cap setters set exactly their own kind, like
        // the panel's segmented-control chips.
        let mut s = crate::init();
        s.core.selected_node = Some(1);
        (COMMANDS[idx_of("style-cap-point")].run)(&mut s);
        assert_eq!(s.core.project.figures[0].def.nodes[1].style.cap, CapKind::Point);
        (COMMANDS[idx_of("style-cap-square")].run)(&mut s);
        assert_eq!(s.core.project.figures[0].def.nodes[1].style.cap, CapKind::Square);
        (COMMANDS[idx_of("style-cap-round")].run)(&mut s);
        assert_eq!(s.core.project.figures[0].def.nodes[1].style.cap, CapKind::Round);
        // Taper cycles the panel's step: 1.0 -> 0.6.
        (COMMANDS[idx_of("cycle-taper")].run)(&mut s);
        assert_eq!(s.core.project.figures[0].def.nodes[1].style.taper, 0.6);
        // Opacity steps through the panel's stepper:
        // down from 1.0 lands on 0.95, up returns to 1.0.
        (COMMANDS[idx_of("style-opacity-down")].run)(&mut s);
        let opacity = s.core.project.figures[0].def.nodes[1].style.opacity;
        assert!((opacity - 0.95).abs() < 1e-6, "opacity after one step down: {opacity}");
        (COMMANDS[idx_of("style-opacity-up")].run)(&mut s);
        assert_eq!(s.core.project.figures[0].def.nodes[1].style.opacity, 1.0);
        // Copy/paste round-trips through the panel's own
        // clipboard: node 1's Point cap lands on node 2,
        // with the panel handlers' own status lines.
        (COMMANDS[idx_of("style-cap-point")].run)(&mut s);
        (COMMANDS[idx_of("copy-style")].run)(&mut s);
        assert_eq!(s.core.status, "Style copied.");
        s.core.selected_node = Some(2);
        (COMMANDS[idx_of("paste-style")].run)(&mut s);
        assert_eq!(s.core.status, "Style pasted.");
        assert_eq!(s.core.project.figures[0].def.nodes[2].style.cap, CapKind::Point);
        // Open style inspector: parked on Home with the
        // right column hidden, the command lands on
        // Animate — the only workspace whose section list
        // carries InspectorStyle — with the column shown.
        let mut s = crate::init();
        s.workspace = Workspace::Home;
        s.show_right = false;
        (COMMANDS[idx_of("open-style-inspector")].run)(&mut s);
        assert_eq!(s.workspace, Workspace::Animate);
        assert!(s.show_right);
    }

    #[test]
    fn wave_13_sprite_commands_are_present_and_wired() {
        use stykanimate_rs::model::ImageAsset;
        // Both Wave 13 keys resolve (idx_of panics if not),
        // in the Segments group beside the Wave 12 media
        // commands.
        for key in ["add-sprite", "remove-sprite"] {
            let cmd = &COMMANDS[idx_of(key)];
            assert_eq!(cmd.group, "Segments", "{key} group");
        }
        // No next-sprite cycle: neither the core nor the
        // shell State carries a sprite selection for one
        // to advance (see the module docs), so the key
        // must not exist.
        assert!(
            COMMANDS.iter().all(|c| c.key != "next-sprite"),
            "next-sprite must stay out until a sprite selection exists"
        );
        // Empty project (the demo carries no assets and no
        // sprites): both commands stand down in words and
        // change nothing.
        let mut s = crate::init();
        assert!(s.core.project.assets.is_empty());
        assert!(s.core.project.sprites.is_empty());
        (COMMANDS[idx_of("add-sprite")].run)(&mut s);
        assert_eq!(s.core.status, "Sprite add failed: no asset 1.");
        assert!(s.core.project.sprites.is_empty());
        (COMMANDS[idx_of("remove-sprite")].run)(&mut s);
        assert_eq!(s.core.status, "No sprite to remove.");
        assert!(s.core.project.sprites.is_empty());
        // Add forwards to A1's op over the FIRST asset:
        // two assets in the table, the instance references
        // asset 0 and lands at the stage centre (the
        // 960×720 demo stage) with its single frame-0 key,
        // in the op's own status wording.
        for name in ["first.png", "second.png"] {
            s.core.project.assets.push(ImageAsset {
                name: name.into(),
                png: Vec::new(),
            });
        }
        (COMMANDS[idx_of("add-sprite")].run)(&mut s);
        assert_eq!(s.core.project.sprites.len(), 1);
        assert_eq!(s.core.project.sprites[0].asset, 0, "the first asset");
        let key = s.core.project.sprites[0].keys[0];
        assert_eq!((key.frame, key.x, key.y), (0.0, 480.0, 360.0));
        assert_eq!(s.core.status, "Sprite 1 added (asset 1).");
        // A second add, its instance re-pointed at the
        // second asset so the survivor is identifiable:
        // remove takes the LAST instance and leaves the
        // first untouched.
        (COMMANDS[idx_of("add-sprite")].run)(&mut s);
        assert_eq!(s.core.project.sprites.len(), 2);
        s.core.project.sprites[1].asset = 1;
        (COMMANDS[idx_of("remove-sprite")].run)(&mut s);
        assert_eq!(s.core.project.sprites.len(), 1);
        assert_eq!(s.core.project.sprites[0].asset, 0, "the first instance survives");
        assert_eq!(s.core.status, "Sprite 2 deleted.");
    }

    // -- command_hint (Phase N, P14 §4.4) -----------------------

    /// The full hint table, pinned: exactly these 17
    /// commands duplicate a real key binding.
    const EXPECTED_HINTS: [(&str, &str); 17] = [
        ("play", "Space"),
        ("prev", "Left"),
        ("next", "Right"),
        ("add-frame", "N"),
        ("inbetween", "I"),
        ("undo", "Ctrl+Z"),
        ("redo", "Ctrl+Shift+Z"),
        ("save", "Ctrl+S"),
        ("onion", "O"),
        ("go-animate", "Alt+1"),
        ("go-draw", "Alt+2"),
        ("go-graph", "Alt+3"),
        ("go-effects", "Alt+4"),
        ("go-audio", "Alt+5"),
        ("go-build3d", "Alt+6"),
        ("go-export", "Alt+7"),
        ("go-home", "Alt+8"),
    ];

    #[test]
    fn command_hint_table_matches_the_pinned_pairs() {
        for (key, hint) in EXPECTED_HINTS {
            assert_eq!(command_hint(key), Some(hint), "hint for {key}");
            // The hinted command must exist in the table.
            idx_of(key);
        }
        // And nothing else in the table carries a hint.
        let hinted = COMMANDS
            .iter()
            .filter(|c| command_hint(c.key).is_some())
            .count();
        assert_eq!(hinted, EXPECTED_HINTS.len());
        assert_eq!(command_hint("no-such-command"), None);
    }

    #[test]
    fn command_hints_name_only_real_bindings() {
        // Drift guard: every hint spelling must be a key
        // label in the help panel's rows, so renaming a
        // binding breaks this test instead of leaving a
        // lying chip on the row. "Ctrl+S" was the one
        // documented exception while the help panel
        // carried it only in Features prose (P14 gap H1);
        // the Phase N V-wave help pass made it a
        // Shortcuts key label, so the exception is
        // retired and the guard covers it like the rest.
        let rows = crate::rune_sec::help_panel::shortcut_rows();
        for (_, hint) in EXPECTED_HINTS {
            let as_label = rows.iter().any(|(label, _)| *label == hint);
            assert!(
                as_label,
                "hint {hint} is not a help-panel key label"
            );
        }
    }

    // -- fuzzy_score -------------------------------------------

    #[test]
    fn empty_query_scores_zero_against_anything() {
        assert_eq!(fuzzy_score("", "Add frame"), Some(0));
        assert_eq!(fuzzy_score("", ""), Some(0));
    }

    #[test]
    fn non_subsequence_is_no_match() {
        assert_eq!(fuzzy_score("zz", "Zoom in"), None);
        assert_eq!(fuzzy_score("framex", "Add frame"), None);
        assert_eq!(fuzzy_score("abc", "acb"), None);
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(fuzzy_score("ADD", "Add frame"), fuzzy_score("add", "Add frame"));
        assert!(fuzzy_score("FrAmE", "Add frame").is_some());
    }

    #[test]
    fn word_start_beats_mid_word() {
        assert!(fuzzy_score("ab", "Abacus").unwrap() > fuzzy_score("ab", "Grab").unwrap());
    }

    #[test]
    fn consecutive_beats_scattered() {
        assert!(fuzzy_score("abc", "Abc").unwrap() > fuzzy_score("abc", "Axbxc").unwrap());
    }

    #[test]
    fn gaps_cost_score() {
        assert!(fuzzy_score("ab", "Ab").unwrap() > fuzzy_score("ab", "Axxb").unwrap());
    }

    #[test]
    fn separator_starts_a_word() {
        // 'f' after '-' counts as a word start, like after ' '.
        assert_eq!(fuzzy_score("f", "Add-frame"), fuzzy_score("f", "Add frame"));
        assert!(fuzzy_score("f", "Add-frame").unwrap() > fuzzy_score("f", "Adfx").unwrap());
    }

    // -- filter_commands ----------------------------------------

    #[test]
    fn empty_query_lists_table_order_capped() {
        let got = filter_commands("");
        assert_eq!(got.len(), MAX_RESULTS);
        assert_eq!(got, (0..MAX_RESULTS).collect::<Vec<_>>());
        assert_eq!(filter_commands("   "), got);
    }

    #[test]
    fn exact_title_query_ranks_its_command_first() {
        let got = filter_commands("undo");
        assert_eq!(COMMANDS[got[0]].key, "undo");
        let got = filter_commands("export mp4");
        assert_eq!(COMMANDS[got[0]].key, "export-mp4");
    }

    #[test]
    fn dashed_key_query_finds_the_command() {
        // The title "Add frame" has no '-', so this only works
        // because the key field is scored too.
        let got = filter_commands("add-frame");
        assert_eq!(COMMANDS[got[0]].key, "add-frame");
    }

    #[test]
    fn group_query_lists_the_whole_group() {
        let got = filter_commands("export");
        assert!(!got.is_empty());
        // Every Export-group command is listed...
        let exports: Vec<usize> = COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, c)| c.group == "Export")
            .map(|(i, _)| i)
            .collect();
        assert!(exports.iter().all(|i| got.contains(i)));
        // ...and the only outsider is the Phase I workspace
        // jump, whose title names the workspace ("Go to
        // Export workspace") and so matches the query
        // honestly.
        assert!(got.iter().all(|&i| {
            COMMANDS[i].group == "Export" || COMMANDS[i].key == "go-export"
        }));
        assert_eq!(got.len(), (exports.len() + 1).min(MAX_RESULTS));
    }

    #[test]
    fn gibberish_matches_nothing() {
        assert!(filter_commands("qqqq").is_empty());
    }

    #[test]
    fn results_are_capped_and_score_ordered() {
        let got = filter_commands("e");
        assert_eq!(got.len(), MAX_RESULTS);
        let scores: Vec<i32> = got
            .iter()
            .map(|&i| command_score("e", &COMMANDS[i]).unwrap())
            .collect();
        assert!(scores.windows(2).all(|w| w[0] >= w[1]), "not ranked: {scores:?}");
    }

    #[test]
    fn subsequence_query_finds_scattered_match() {
        // 'a' then 'f' scattered across "Add frame".
        let got = filter_commands("af");
        assert!(got.iter().any(|&i| COMMANDS[i].key == "add-frame"));
    }

    #[test]
    fn exprt_finds_go_export_by_subsequence() {
        // Phase K (section 17): the briefed "fuzzy
        // fallback" case. "exprt" is NOT a contiguous
        // substring of the command's key or title, so
        // only the subsequence matcher can find it — and
        // it already does (fuzzy_score, Phase D).
        let cmd = COMMANDS
            .iter()
            .find(|c| c.key == "go-export")
            .expect("go-export is in the table");
        assert!(!cmd.key.contains("exprt"));
        assert!(!cmd.title.to_lowercase().contains("exprt"));
        let got = filter_commands("exprt");
        assert!(
            got.iter().any(|&i| COMMANDS[i].key == "go-export"),
            "exprt should find go-export by subsequence: {got:?}"
        );
    }

    #[test]
    fn group_field_matches_by_subsequence_too() {
        // The five View toggles are reachable by "vew"
        // ONLY through their group field ("View": v, e,
        // w in order) — no key or title holds v…e…w.
        // Pinned at Phase K (section 17): any future
        // search restructure that drops the group field
        // from matching must break this test loudly
        // rather than silently narrowing the palette.
        let got = filter_commands("vew");
        for key in [
            "toggle-guides",
            "toggle-frustum",
            "toggle-safe-frame",
            "toggle-left-panel",
            "toggle-right-panel",
        ] {
            assert!(
                got.iter().any(|&i| COMMANDS[i].key == key),
                "vew should reach {key} via its group: {got:?}"
            );
        }
    }

    // -- selection helpers ---------------------------------------

    #[test]
    fn wrap_move_wraps_both_ways() {
        assert_eq!(wrap_move(0, 3, 1), 1);
        assert_eq!(wrap_move(2, 3, 1), 0);
        assert_eq!(wrap_move(0, 3, -1), 2);
        assert_eq!(wrap_move(1, 3, -4), 0);
        assert_eq!(wrap_move(0, 0, 1), 0);
    }

    #[test]
    fn clamped_selected_stays_in_range() {
        assert_eq!(clamped_selected(0, 5), 0);
        assert_eq!(clamped_selected(4, 5), 4);
        assert_eq!(clamped_selected(9, 5), 4);
        assert_eq!(clamped_selected(3, 0), 0);
    }

    // -- recents ----------------------------------------------

    #[test]
    fn push_recent_puts_newest_first() {
        let mut recent: Vec<&'static str> = Vec::new();
        push_recent(&mut recent, "undo");
        assert_eq!(recent, vec!["undo"]);
        push_recent(&mut recent, "play");
        assert_eq!(recent, vec!["play", "undo"]);
    }

    #[test]
    fn push_recent_dedups_by_moving_to_front() {
        let mut recent: Vec<&'static str> = Vec::new();
        push_recent(&mut recent, "undo");
        push_recent(&mut recent, "play");
        push_recent(&mut recent, "redo");
        push_recent(&mut recent, "undo");
        assert_eq!(recent, vec!["undo", "redo", "play"]);
    }

    #[test]
    fn push_recent_caps_at_max_recent() {
        let mut recent: Vec<&'static str> = Vec::new();
        for key in ["play", "undo", "redo", "save", "load", "add-frame"] {
            push_recent(&mut recent, key);
        }
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0], "add-frame");
        assert_eq!(recent[MAX_RECENT - 1], "undo");
        assert!(!recent.contains(&"play"), "oldest entry fell off: {recent:?}");
        // Re-running a survivor keeps the cap and the order.
        push_recent(&mut recent, "save");
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0], "save");
    }

    #[test]
    fn empty_query_without_recents_is_the_plain_list() {
        let (rows, recent_rows) = display_commands("", &[]);
        assert_eq!(recent_rows, 0);
        assert_eq!(rows, filter_commands(""));
        // A whitespace query counts as empty here too.
        let (rows, recent_rows) = display_commands("   ", &[]);
        assert_eq!(recent_rows, 0);
        assert_eq!(rows, filter_commands(""));
    }

    #[test]
    fn empty_query_leads_with_recents_then_table_order() {
        // Recents drawn from three spots in the table: the
        // first page ("play"), just past it ("undo") and far
        // down it ("export-mp4"). The one already on the
        // first page must not appear twice.
        let recent: Vec<&'static str> = vec!["export-mp4", "undo", "play"];
        let (rows, recent_rows) = display_commands("", &recent);
        assert_eq!(recent_rows, 3);
        assert_eq!(rows[0], idx_of("export-mp4"));
        assert_eq!(rows[1], idx_of("undo"));
        assert_eq!(rows[2], idx_of("play"));
        assert_eq!(rows.len(), MAX_RESULTS);
        // No command appears twice.
        let mut deduped = rows.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), rows.len(), "duplicate rows: {rows:?}");
        // The tail is the plain list with the recents removed.
        let tail: Vec<usize> = filter_commands("")
            .into_iter()
            .filter(|i| !recent.iter().any(|k| idx_of(k) == *i))
            .take(rows.len() - recent_rows)
            .collect();
        assert_eq!(rows[recent_rows..], tail[..]);
    }

    #[test]
    fn nonempty_query_ignores_recents() {
        let recent: Vec<&'static str> = vec!["export-mp4", "undo"];
        for q in ["undo", "export", "e", "add-frame"] {
            let (rows, recent_rows) = display_commands(q, &recent);
            assert_eq!(recent_rows, 0, "query {q:?} must not surface recents");
            assert_eq!(rows, filter_commands(q), "query {q:?} changed behaviour");
        }
    }

    #[test]
    fn unknown_recent_keys_are_skipped() {
        let recent: Vec<&'static str> = vec!["no-such-command", "undo"];
        let (rows, recent_rows) = display_commands("", &recent);
        assert_eq!(recent_rows, 1);
        assert_eq!(rows[0], idx_of("undo"));
    }

    // -- recents persistence (Phase L) -------------------------
    //
    // The disk itself is out of reach of these tests on
    // purpose: test builds compile the read and the write
    // out (see `load_recents_cache`), so what is asserted
    // here is the pure merge at the disk boundary and the
    // process cache the save call mirrors.

    #[test]
    fn merge_persisted_drops_unknown_keys_and_duplicates() {
        let saved: Vec<String> = ["undo", "no-such-command", "play", "undo", "save"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(merge_persisted(&saved), vec!["undo", "play", "save"]);
    }

    #[test]
    fn merge_persisted_caps_at_max_recent() {
        // Six distinct live keys on disk: the oldest falls
        // off, exactly as `push_recent` caps the live list.
        let saved: Vec<String> = ["add-frame", "save", "load", "undo", "redo", "play"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let merged = merge_persisted(&saved);
        assert_eq!(merged.len(), MAX_RECENT);
        assert_eq!(merged[0], "add-frame");
        assert_eq!(merged[MAX_RECENT - 1], "redo");
        assert!(!merged.contains(&"play"), "oldest entry fell off: {merged:?}");
    }

    #[test]
    fn recent_strings_round_trip_through_merge_persisted() {
        let recent: Vec<&'static str> = vec!["export-mp4", "undo", "play"];
        assert_eq!(merge_persisted(&recent_strings(&recent)), recent);
    }

    #[test]
    fn record_recent_mirrors_the_working_list_into_the_cache() {
        reset_recents_cache();
        seed_recents_cache(&["undo"]);
        // An unseeded (empty) working list inherits the
        // persisted recents before the push, so the run
        // cannot clobber the previous session's list.
        let mut recent: Vec<&'static str> = Vec::new();
        record_recent(&mut recent, "play");
        assert_eq!(recent, vec!["play", "undo"]);
        // The cache now holds exactly the recorded list —
        // and its `recent_strings` form is the payload
        // `save_palette_recents` receives in a real build.
        assert_eq!(persisted_recents(), vec!["play", "undo"]);
        assert_eq!(
            recent_strings(&persisted_recents()),
            vec!["play".to_string(), "undo".to_string()]
        );
        // A second run on the seeded list records on top.
        record_recent(&mut recent, "save");
        assert_eq!(recent, vec!["save", "play", "undo"]);
        assert_eq!(persisted_recents(), recent);
    }

    #[test]
    fn open_palette_seeds_recents_from_the_cache_once() {
        reset_recents_cache();
        seed_recents_cache(&["export-mp4", "undo"]);
        let mut s = crate::init();
        assert!(s.palette.recent.is_empty());
        open_palette(&mut s);
        assert_eq!(s.palette.recent, vec!["export-mp4", "undo"]);
        // Once the session has a list of its own, reopening
        // never overwrites it with the cache.
        s.palette.recent = vec!["play"];
        close_palette(&mut s);
        open_palette(&mut s);
        assert_eq!(s.palette.recent, vec!["play"]);
    }

    /// Wave P (P4): the result rows construct with their
    /// press state in place — the unfiltered list (row 0
    /// is the selected `on` branch, the rest the off
    /// branch) and a filtered list, under both themes.
    /// Element internals are opaque to a bin-crate unit
    /// test, so construction is the assertable level; the
    /// presented-tree proof is the headless `palette-open`
    /// scene's job.
    #[test]
    fn palette_view_rows_construct_in_both_selection_branches() {
        for dark in [true, false] {
            let t = tok::theme(dark);
            let mut s = crate::init();
            open_palette(&mut s);
            let _ = palette_view(&s, &t);
            s.palette.query.text = "export".to_string();
            s.palette.selected = 0;
            let _ = palette_view(&s, &t);
        }
    }
}
