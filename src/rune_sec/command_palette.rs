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
//!   the recents entirely. Recents are session-only — the
//!   list starts empty on every launch.
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

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, text_input, Color, El, Key, KeyEvent, TextEdit, Tx};
use stykanimate_rs::vfx::ClipKind;

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
}

fn cmd_redo(s: &mut State) {
    s.core.redo();
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
    /// with no duplicates. Session-only: it starts empty on
    /// every launch and survives open/close cycles.
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
/// focus pattern) so typing lands in it immediately.
pub fn open_palette(s: &mut State) {
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
/// list.
pub fn run_selected(s: &mut State) {
    let (results, _) = display_commands(&s.palette.query.text, &s.palette.recent);
    let row = clamped_selected(s.palette.selected, results.len());
    if let Some(&ci) = results.get(row) {
        (COMMANDS[ci].run)(s);
        push_recent(&mut s.palette.recent, COMMANDS[ci].key);
    }
    close_palette(s);
}

/// Run the command at table index `ci` (a result-row press)
/// and close the palette. The run is recorded in the recents
/// list.
fn run_command(s: &mut State, ci: usize) {
    if let Some(cmd) = COMMANDS.get(ci) {
        (cmd.run)(s);
        push_recent(&mut s.palette.recent, cmd.key);
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

// ----------------------------------------------------------------
// View
// ----------------------------------------------------------------

/// A small non-interactive heading row over a section of
/// the results list ("Recent" / "All commands"). Headings
/// are not result rows: they never take the selection,
/// which indexes [`display_commands`]' rows only.
fn section_heading(label: &str, t: &Theme) -> El {
    row()
        .h(22.0)
        .px(10.0)
        .center_y()
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
}

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

    let field = text_input(&s.palette.query, type_::BODY.lh(26.0))
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
        .border(1.0, Color::WHITE.alpha(0.10))
        .child(
            row()
                .h(52.0)
                .px(16.0)
                .gap(10.0)
                .center_y()
                .child(text("⌕").font(type_::TITLE.lh(26.0)).color(t.text_secondary))
                .child(field),
        );

    if results.is_empty() {
        panel = panel
            .child(el().h(1.0).w_full().fill(t.separator))
            .child(
                row()
                    .h(36.0)
                    .px(16.0)
                    .center_y()
                    .child(text("No commands match").font(type_::BODY).color(t.text_secondary)),
            );
    } else {
        let mut list = col().padding(6.0).gap(2.0);
        for (row_i, &ci) in results.iter().enumerate() {
            if recent_rows > 0 && row_i == 0 {
                list = list.child(section_heading("Recent", t));
            }
            if recent_rows > 0 && row_i == recent_rows {
                list = list.child(section_heading("All commands", t));
            }
            let cmd = &COMMANDS[ci];
            let on = row_i == selected;
            list = list.child(
                row()
                    .key(format!("cmd-{}", cmd.key))
                    .h(34.0)
                    .px(10.0)
                    .radius(8.0)
                    .center_y()
                    .gap(8.0)
                    .fill(if on { accent.alpha(0.85) } else { Color::TRANSPARENT })
                    .hover(if on { accent.alpha(0.85) } else { t.control_track })
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
                    ),
            );
        }
        panel = panel.child(el().h(1.0).w_full().fill(t.separator)).child(list);
    }

    panel.child(
        row()
            .h(26.0)
            .px(16.0)
            .center_y()
            .child(
                text("↑↓ navigate · Enter run · Esc close")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
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
        // explanation when nothing is selected).
        assert_eq!(total, 103, "palette should cover 103 commands, has {total}");
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
        });
        s.core.project.audio.push(AudioClip {
            path: "b.wav".into(),
            start_frame: 4,
            gain: 1.0,
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
}
