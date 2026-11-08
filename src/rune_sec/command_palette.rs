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
//!   `fn(&mut EditorCore)`: either a direct `EditorCore` op
//!   (method reference) or a small shim in this module for ops
//!   that take an argument (the shim passes exactly the value
//!   the GPUI button passed — deltas verified against the
//!   pre-removal `src/app.rs`) or that flip a public core field
//!   (Play/Pause and Onion have no op; the GPUI app flipped the
//!   field from its button handlers, and chrome_view still
//!   does for those two).
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
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::vfx::ClipKind;

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
/// with the editor core when the command is picked.
#[derive(Clone, Copy)]
pub struct Command {
    pub key: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub run: fn(&mut EditorCore),
}

// -- Shims: ops that take an argument, or field flips with no
// -- op. Argument values mirror the GPUI buttons exactly
// -- (pre-removal src/app.rs: adjust_length ±4, thickness ±1,
// -- fps ±1, zoom ×1.2, particle kinds [Burst, Sparkle, Smoke]
// -- per EditorCore::PARTICLE_KINDS, editor.rs:1713).

/// Play/Pause has no op: the GPUI `toggle_play` flipped the
/// field, syncing the playhead to the shown frame first (the
/// same sync chrome_view's `toggle_play` performs).
fn cmd_play_pause(c: &mut EditorCore) {
    if !c.playing {
        c.playhead = c.current as f32;
    }
    c.playing = !c.playing;
}

/// Previous frame, wrapping (chrome transport semantics).
fn cmd_prev_frame(c: &mut EditorCore) {
    let n = c.frame_count();
    if n > 0 {
        c.goto((c.current + n - 1) % n);
    }
}

/// Next frame, wrapping (chrome transport semantics).
fn cmd_next_frame(c: &mut EditorCore) {
    let n = c.frame_count();
    if n > 0 {
        c.goto((c.current + 1) % n);
    }
}

fn cmd_first_frame(c: &mut EditorCore) {
    if c.frame_count() > 0 {
        c.goto(0);
    }
}

fn cmd_fps_up(c: &mut EditorCore) {
    c.adjust_fps(1.0);
}

fn cmd_fps_down(c: &mut EditorCore) {
    c.adjust_fps(-1.0);
}

fn cmd_solo_figure(c: &mut EditorCore) {
    let fi = c.selected_figure;
    c.toggle_solo(fi);
}

fn cmd_lock_figure(c: &mut EditorCore) {
    let fi = c.selected_figure;
    c.toggle_figure_lock(fi);
}

fn cmd_length_up(c: &mut EditorCore) {
    c.adjust_length(4.0);
}

fn cmd_length_down(c: &mut EditorCore) {
    c.adjust_length(-4.0);
}

fn cmd_thickness_up(c: &mut EditorCore) {
    c.adjust_thickness(1.0);
}

fn cmd_thickness_down(c: &mut EditorCore) {
    c.adjust_thickness(-1.0);
}

fn cmd_zoom_in(c: &mut EditorCore) {
    c.camera_zoom_by(1.2);
}

fn cmd_zoom_out(c: &mut EditorCore) {
    c.camera_zoom_by(1.0 / 1.2);
}

/// Onion has no op: the GPUI button flipped the field (and
/// chrome_view's app-bar button still does).
fn cmd_toggle_onion(c: &mut EditorCore) {
    c.onion = !c.onion;
}

/// Draw mode is a public core field the GPUI mode toggle
/// flipped; there is no op.
fn cmd_toggle_draw_mode(c: &mut EditorCore) {
    c.draw_mode = !c.draw_mode;
}

fn cmd_particle_burst(c: &mut EditorCore) {
    c.add_particle(0); // ParticleKind::Burst
}

fn cmd_particle_sparkle(c: &mut EditorCore) {
    c.add_particle(1); // ParticleKind::Sparkle
}

fn cmd_fx_confetti(c: &mut EditorCore) {
    c.add_fx_clip(ClipKind::Confetti);
}

fn cmd_fx_lightning(c: &mut EditorCore) {
    c.add_fx_clip(ClipKind::Lightning);
}

/// Phase E: the duplicate op takes a figure index, so the
/// shim passes the current selection — the same pattern as
/// `cmd_solo_figure` / `cmd_lock_figure`.
fn cmd_duplicate_figure(c: &mut EditorCore) {
    let fi = c.selected_figure;
    c.duplicate_figure(fi);
}

/// Phase E: `delete_stroke_at` takes a project-wide stroke
/// index, so the shim resolves the last stroke at call time
/// and stands down when the project has no strokes.
fn cmd_delete_last_stroke(c: &mut EditorCore) {
    if let Some(i) = c.project.strokes.len().checked_sub(1) {
        c.delete_stroke_at(i);
    }
}

/// Phase E: `delete_audio_clip` takes a clip index, so the
/// shim resolves the last clip at call time and stands down
/// when the project has no audio clips.
fn cmd_delete_last_audio_clip(c: &mut EditorCore) {
    if let Some(i) = c.project.audio.len().checked_sub(1) {
        c.delete_audio_clip(i);
    }
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
    // -- Frames ---------------------------------------------------
    Command { key: "add-frame", title: "Add frame", group: "Frames", run: EditorCore::add_frame },
    Command { key: "dup-frame", title: "Duplicate frame", group: "Frames", run: EditorCore::duplicate_frame },
    Command { key: "del-frame", title: "Delete frame", group: "Frames", run: EditorCore::delete_frame },
    Command { key: "inbetween", title: "Insert inbetween", group: "Frames", run: EditorCore::insert_inbetween },
    Command { key: "copy-frame", title: "Copy frame", group: "Frames", run: EditorCore::copy_frame },
    Command { key: "paste-frame", title: "Paste frame", group: "Frames", run: EditorCore::paste_frame },
    Command { key: "fps-up", title: "Increase FPS", group: "Frames", run: cmd_fps_up },
    Command { key: "fps-down", title: "Decrease FPS", group: "Frames", run: cmd_fps_down },
    // -- Edit -----------------------------------------------------
    Command { key: "undo", title: "Undo", group: "Edit", run: EditorCore::undo },
    Command { key: "redo", title: "Redo", group: "Edit", run: EditorCore::redo },
    // -- Projects -------------------------------------------------
    Command { key: "save", title: "Save project", group: "Projects", run: EditorCore::save },
    Command { key: "load", title: "Load project", group: "Projects", run: EditorCore::load },
    Command { key: "save-named", title: "Save named project", group: "Projects", run: EditorCore::save_named_project },
    Command { key: "cycle-project", title: "Cycle project file", group: "Projects", run: EditorCore::cycle_project_file },
    Command { key: "open-project", title: "Open project file", group: "Projects", run: EditorCore::open_project_file },
    // -- Figures --------------------------------------------------
    Command { key: "add-figure", title: "Add figure", group: "Figures", run: EditorCore::add_figure },
    Command { key: "add-quadruped", title: "Add quadruped figure", group: "Figures", run: EditorCore::add_quadruped },
    Command { key: "del-figure", title: "Delete figure", group: "Figures", run: EditorCore::delete_figure },
    Command { key: "duplicate-figure", title: "Duplicate figure", group: "Figures", run: cmd_duplicate_figure },
    Command { key: "hide-figure", title: "Toggle figure hidden", group: "Figures", run: EditorCore::toggle_figure_hidden },
    Command { key: "solo-figure", title: "Solo selected figure", group: "Figures", run: cmd_solo_figure },
    Command { key: "lock-figure", title: "Lock selected figure", group: "Figures", run: cmd_lock_figure },
    Command { key: "mirror-pose", title: "Mirror selected pose", group: "Figures", run: EditorCore::mirror_selected_pose },
    Command { key: "insert-library-figure", title: "Insert library figure", group: "Figures", run: EditorCore::insert_library_figure },
    // -- Segments -------------------------------------------------
    Command { key: "add-segment", title: "Add segment", group: "Segments", run: EditorCore::add_segment },
    Command { key: "del-segment", title: "Delete segment", group: "Segments", run: EditorCore::delete_segment },
    Command { key: "len-up", title: "Lengthen segment", group: "Segments", run: cmd_length_up },
    Command { key: "len-down", title: "Shorten segment", group: "Segments", run: cmd_length_down },
    Command { key: "thick-up", title: "Thicken segment", group: "Segments", run: cmd_thickness_up },
    Command { key: "thick-down", title: "Thin segment", group: "Segments", run: cmd_thickness_down },
    Command { key: "cycle-shape", title: "Cycle segment shape", group: "Segments", run: EditorCore::cycle_shape },
    Command { key: "cycle-bend", title: "Cycle segment bend", group: "Segments", run: EditorCore::cycle_bend },
    Command { key: "cycle-node-color", title: "Cycle segment colour", group: "Segments", run: EditorCore::cycle_node_color },
    // -- Camera ---------------------------------------------------
    Command { key: "cam-zoom-in", title: "Camera zoom in", group: "Camera", run: cmd_zoom_in },
    Command { key: "cam-zoom-out", title: "Camera zoom out", group: "Camera", run: cmd_zoom_out },
    Command { key: "cam-frame-all", title: "Frame all figures", group: "Camera", run: EditorCore::frame_all_figures },
    Command { key: "cam-centre", title: "Centre on selection", group: "Camera", run: EditorCore::centre_on_selection },
    Command { key: "cam-clear-key", title: "Clear camera key", group: "Camera", run: EditorCore::camera_clear_key },
    Command { key: "cam-follow", title: "Toggle camera follow", group: "Camera", run: EditorCore::toggle_camera_follow },
    // -- Graph ----------------------------------------------------
    Command { key: "key-graph-angle", title: "Key graph angle", group: "Graph", run: EditorCore::key_graph_angle },
    Command { key: "clear-graph", title: "Clear graph curves", group: "Graph", run: EditorCore::clear_graph },
    // -- View -----------------------------------------------------
    Command { key: "onion", title: "Toggle onion skin", group: "View", run: cmd_toggle_onion },
    Command { key: "draw-mode", title: "Toggle draw mode", group: "View", run: cmd_toggle_draw_mode },
    Command { key: "tween-play", title: "Toggle tween playback", group: "View", run: EditorCore::toggle_tween_playback },
    Command { key: "cycle-easing", title: "Cycle easing", group: "View", run: EditorCore::cycle_easing },
    // -- Draw -----------------------------------------------------
    Command { key: "cycle-brush", title: "Cycle brush", group: "Draw", run: EditorCore::cycle_brush },
    Command { key: "draw-tool", title: "Toggle brush / eraser", group: "Draw", run: EditorCore::toggle_draw_tool },
    Command { key: "clear-drawings", title: "Clear frame drawings", group: "Draw", run: EditorCore::clear_frame_drawings },
    Command { key: "mirror-strokes", title: "Mirror strokes horizontally", group: "Draw", run: EditorCore::mirror_strokes_x },
    Command { key: "stamp-polygon", title: "Stamp polygon", group: "Draw", run: EditorCore::stamp_polygon },
    Command { key: "delete-last-stroke", title: "Delete last stroke", group: "Draw", run: cmd_delete_last_stroke },
    // -- Effects --------------------------------------------------
    Command { key: "particle-burst", title: "Add particle: Burst", group: "Effects", run: cmd_particle_burst },
    Command { key: "particle-sparkle", title: "Add particle: Sparkle", group: "Effects", run: cmd_particle_sparkle },
    Command { key: "fx-clip-confetti", title: "Add FX clip: Confetti", group: "Effects", run: cmd_fx_confetti },
    Command { key: "fx-clip-lightning", title: "Add FX clip: Lightning", group: "Effects", run: cmd_fx_lightning },
    Command { key: "clear-particles", title: "Clear particles and FX", group: "Effects", run: EditorCore::clear_particles },
    // -- Audio ----------------------------------------------------
    Command { key: "add-audio", title: "Add audio clip", group: "Audio", run: EditorCore::add_audio_clip },
    Command { key: "mixdown", title: "Mix down audio", group: "Audio", run: EditorCore::mixdown_audio },
    Command { key: "audio-echo", title: "Audio: echo last clip", group: "Audio", run: EditorCore::audio_echo_last },
    Command { key: "audio-lowpass", title: "Audio: lowpass last clip", group: "Audio", run: EditorCore::audio_lowpass_last },
    Command { key: "audio-trim-silence", title: "Audio: trim silence", group: "Audio", run: EditorCore::audio_trim_silence_last },
    Command { key: "audio-split-silences", title: "Audio: split at silences", group: "Audio", run: EditorCore::audio_split_at_silences_last },
    Command { key: "fade-audio", title: "Audio: fade last clip", group: "Audio", run: EditorCore::audio_fade_last },
    Command { key: "reverse-audio", title: "Audio: reverse last clip", group: "Audio", run: EditorCore::audio_reverse_last },
    Command { key: "delete-audio-clip", title: "Audio: delete last clip", group: "Audio", run: cmd_delete_last_audio_clip },
    // -- Export ---------------------------------------------------
    Command { key: "export-png", title: "Export PNG sequence", group: "Export", run: EditorCore::export_png },
    Command { key: "export-gif", title: "Export GIF", group: "Export", run: EditorCore::export_gif },
    Command { key: "export-mp4", title: "Export MP4", group: "Export", run: EditorCore::export_mp4 },
    Command { key: "export-webm", title: "Export WebM", group: "Export", run: EditorCore::export_webm },
    Command { key: "export-piv", title: "Export .piv", group: "Export", run: EditorCore::export_piv },
    Command { key: "export-stk", title: "Export .stk", group: "Export", run: EditorCore::export_stk },
    // -- 3D -------------------------------------------------------
    Command { key: "enter-3d", title: "Enter 3D mode", group: "3D", run: EditorCore::enter_3d },
    Command { key: "exit-3d", title: "Exit 3D mode", group: "3D", run: EditorCore::exit_3d },
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
    (rows, recent_rows.min(rows.len()))
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

/// Run the currently highlighted command against the core and
/// close the palette. A stale selection (query changed under
/// it) is clamped first; an empty result list just closes.
/// The run is recorded in the recents list.
pub fn run_selected(s: &mut State) {
    let (results, _) = display_commands(&s.palette.query.text, &s.palette.recent);
    let row = clamped_selected(s.palette.selected, results.len());
    if let Some(&ci) = results.get(row) {
        (COMMANDS[ci].run)(&mut s.core);
        push_recent(&mut s.palette.recent, COMMANDS[ci].key);
    }
    close_palette(s);
}

/// Run the command at table index `ci` (a result-row press)
/// and close the palette. The run is recorded in the recents
/// list.
fn run_command(s: &mut State, ci: usize) {
    if let Some(cmd) = COMMANDS.get(ci) {
        (cmd.run)(&mut s.core);
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
        assert!(total >= 77, "palette should cover 77+ commands, has {total}");
        for c in COMMANDS {
            assert!(!c.title.is_empty() && !c.group.is_empty(), "empty field on {}", c.key);
        }
    }

    #[test]
    fn command_runs_are_wired_to_real_ops() {
        // Smoke: a few safe commands actually change core state
        // through the table's fn pointers.
        let mut core = EditorCore::new();
        let frames = core.frame_count();
        (COMMANDS[idx_of("add-frame")].run)(&mut core);
        assert_eq!(core.frame_count(), frames + 1);
        (COMMANDS[idx_of("play")].run)(&mut core);
        assert!(core.playing);
        (COMMANDS[idx_of("play")].run)(&mut core);
        assert!(!core.playing);
        // Onion defaults to ON in EditorCore::new(), so the
        // command must flip it off and back on.
        assert!(core.onion);
        (COMMANDS[idx_of("onion")].run)(&mut core);
        assert!(!core.onion);
        (COMMANDS[idx_of("onion")].run)(&mut core);
        assert!(core.onion);
        // Undo on a fresh core is a safe no-op.
        let mut fresh = EditorCore::new();
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
        let mut core = EditorCore::new();
        let figures = core.project.figures.len();
        (COMMANDS[idx_of("duplicate-figure")].run)(&mut core);
        assert_eq!(core.project.figures.len(), figures + 1);
        // Delete last stroke: the empty guard stands down,
        // then a real run removes only the newest stroke.
        let mut core = EditorCore::new();
        (COMMANDS[idx_of("delete-last-stroke")].run)(&mut core);
        assert!(core.project.strokes.is_empty());
        let stroke = |color: u32| Stroke {
            frame: 0,
            points: vec![[0.0, 0.0]],
            color,
            width: 1.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
        };
        core.project.strokes.push(stroke(0x111111));
        core.project.strokes.push(stroke(0x222222));
        (COMMANDS[idx_of("delete-last-stroke")].run)(&mut core);
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].color, 0x111111);
        // The audio trio on a clipless core: every one is a
        // status-only no-op (the *_last ops self-guard, the
        // delete shim's own guard stands down first).
        let mut core = EditorCore::new();
        (COMMANDS[idx_of("fade-audio")].run)(&mut core);
        (COMMANDS[idx_of("reverse-audio")].run)(&mut core);
        (COMMANDS[idx_of("delete-audio-clip")].run)(&mut core);
        assert!(core.project.audio.is_empty());
        // Delete last clip removes exactly the newest entry.
        core.project.audio.push(AudioClip {
            path: "a.wav".into(),
            start_frame: 0,
            gain: 1.0,
        });
        core.project.audio.push(AudioClip {
            path: "b.wav".into(),
            start_frame: 4,
            gain: 1.0,
        });
        (COMMANDS[idx_of("delete-audio-clip")].run)(&mut core);
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.project.audio[0].path, "a.wav");
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
        assert!(got.iter().all(|&i| COMMANDS[i].group == "Export"));
        let exports = COMMANDS.iter().filter(|c| c.group == "Export").count();
        assert_eq!(got.len(), exports.min(MAX_RESULTS));
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
