//! Movieclips panel for the Rune editor shell (Phase D,
//! section 17 — `src/rune_sec/`). Declared from `main.rs` by
//! the integrator; bin-crate code, not part of the engine
//! library.
//!
//! Lists the project's movieclips (`Project::clips` —
//! `model::ClipDef`s; NOT FX clips from `fx_edit` and NOT
//! audio clips), builds new ones from the selected figure,
//! inserts clip-playing figures, and sets the selected clip's
//! loop mode. Every control calls a real `EditorCore`
//! movieclip op:
//!
//! * "Make clip" -> `EditorCore::make_clip_from_figure`
//!   (editor.rs:3203) — builds the clip from the selected
//!   figure's pose on EVERY project frame and points the
//!   figure at it. There is deliberately no frame-range
//!   stepper: no core op accepts a range for clip building
//!   (the parameterised sibling `create_clip_from_figure`,
//!   editor.rs:3430, is whole-timeline too), and the core's
//!   only frame-range state, `loop_range` (editor.rs:61,
//!   `set_loop_range` editor.rs:738), drives playback looping,
//!   not clip building — steppers wired to either would be
//!   dead or misleading controls.
//! * "Insert clip" -> `EditorCore::insert_clip_figure`
//!   (editor.rs:3225) — adds a figure playing the FIRST clip;
//!   the op takes no index, and with no clips it only sets the
//!   status line, exactly as the GPUI button behaved.
//! * Per-row "Insert" -> `insert_clip_figure_at` in this
//!   module (Phase M) — the engine op's construction with
//!   the row's own clip index parameterised, under this
//!   module's field-write discipline ("No such clip.", no
//!   snapshot, when out of range; exactly one snapshot
//!   otherwise). The clips vec is untouched, so no stored
//!   clip index shifts and neither the panel selection nor
//!   an open rename target needs repair.
//! * The loop-mode segmented control -> `apply_loop_mode`,
//!   which reaches the target mode through the one loop op
//!   the core exposes, `EditorCore::cycle_clip_loop`
//!   (editor.rs:3473).
//! * The derived state section (Wave X, P-1 / GAP #13) ->
//!   `states_section` under the loop-mode row: the selected
//!   figure's base link and state keys presented by clip
//!   name, with click-to-key-at-playhead over the pool. No
//!   new model field — the registry derives from
//!   `FigureInstance::states` + `clip` against
//!   `Project::clips`, and the writes (`key_state_at` /
//!   `clear_state_at`) route through the core's state ops
//!   (`EditorCore::set_state_key` / `remove_state_key`,
//!   Wave X X2) — one authoring path for panel and MCP.
//! * Per-row "Dup" -> `duplicate_clip` in this module
//!   (Phase F). The core has NO duplicate/rename movieclip
//!   op (its clip ops are make/insert/enter/exit/frame-edit/
//!   cycle only), so these two are field writes mirroring
//!   the snapshot discipline of `cycle_clip_loop` exactly:
//!   an out-of-range index only sets the status line ("No
//!   such clip.", no snapshot); otherwise exactly one
//!   `core.snapshot()`, the write, and a status line.
//!   Duplicate APPENDS the clone (named `"<name> copy"`,
//!   like `duplicate_figure`), so no stored clip index
//!   shifts — figures keep playing the original and the
//!   copy starts unused.
//! * Per-row "Rename" -> `begin_rename` loads the clip's
//!   name into the rename field; "Apply" (or Enter in the
//!   field) writes it back through `rename_clip`, which
//!   resolves the typed text via `resolved_clip_name`
//!   (trimmed; blank falls back to the current name) and
//!   treats a rename to the same name as a no-op with no
//!   snapshot.
//! * Per-row "Del" -> `delete_clip` in this module
//!   (Phase G). Same field-write discipline ("No such
//!   clip.", no snapshot, when out of range; otherwise
//!   exactly one snapshot). Removing a clip shifts every
//!   later index, so the same write remaps the other
//!   stored clip indices: figures (`FigureInstance::clip`
//!   — a figure whose clip is deleted falls back to its
//!   timeline poses; references past the gap shift down)
//!   and the clip-edit session's clip, which follows its
//!   clip and ends when that clip is deleted. The panel
//!   selection and an open rename target are repaired by
//!   the `delete_clip_at` dispatch (`selection_after_delete`,
//!   mirroring the FX panel's helper of the same name;
//!   `target_after_delete` for the rename target, which
//!   closes when its clip is the one deleted).
//! * Per-row "Up" / "Dn" -> `move_clip` in this module
//!   (Phase G): a `Vec::swap` with the neighbour plus the
//!   same index remap (`target_after_swap`), so figures
//!   keep playing the clip they were playing and the
//!   session / selection / rename target follow the moved
//!   clip. Moving past either end is a status-only
//!   refusal, no snapshot. On what order MEANS here: no
//!   consumer plays clips in list order —
//!   `Project::apply_clips` (model.rs) resolves each
//!   figure's own stored index — so reordering is list
//!   organisation; its one playback-adjacent effect is
//!   that `EditorCore::insert_clip_figure` always inserts
//!   the FIRST clip, so moving a clip to the top changes
//!   what "Insert clip" adds.
//! * The clip-edit session section (Phase E) -> the core's
//!   Wave-7 session ops: "Edit clip" enters the session for
//!   the panel-selected clip via `EditorCore::enter_clip`
//!   (editor.rs:3335, by clip index); while `core.clip_edit`
//!   is active the section shows the session label and the
//!   frame controls — Prev/Next via `EditorCore::clip_goto`
//!   (editor.rs:3447), `clip_add_frame` (editor.rs:3403),
//!   `clip_delete_frame` (editor.rs:3426) and the exit op
//!   `EditorCore::exit_clip` (editor.rs:3352).
//!
//! Selection is panel-local state (`ClipsPanelState`): the
//! core has no selected-clip field (its only clip selection
//! is the `clip_edit` editing session), and the shell's
//! `State` has no clips field yet, so the panel keeps its
//! selection in a thread-local cell — the Rune shell's view
//! and handlers all run on the one UI thread.
//!
//! The rename field is the one piece of panel state that
//! cannot live in that cell: a `rune::TextEdit` is not
//! `Copy`, and `text_input`'s `on_edit` needs a
//! `&mut TextEdit` path into the shell `State` (the audio
//! panel's name field works the same way). It lives in
//! `ClipRenameState` (declared below), which the integrator
//! homes in `State` with exactly these lines — field:
//! `pub clips_rename: rune_sec::clips_panel::ClipRenameState,`
//! and init: `clips_rename: rune_sec::clips_panel::ClipRenameState::new(),`.
//! `main.rs`'s `text_focused` must also gain
//! `|| s.clips_rename.field.focused`, or global shortcuts
//! (Space, N, Delete…) would fire while typing a name.
//!
//! Phase N (adoption): the panel root is the kit card
//! (`design::panel_col`, key `clips-panel` unchanged), the
//! header is `design::panel_title` (film glyph + title +
//! clip count) over a `design::divider`, clip rows are
//! `design::list_row` shells with the row's `.on_press`
//! and children attached verbatim, and the no-clips line
//! is a `design::empty_state` carrying the old message
//! byte-identical. Keys, handlers and status strings are
//! untouched — row keys stay 1-based (`clip-row-{n}`),
//! the standing exception the gate preserves.
//!
//! UI mission (residual overflow): this message (79
//! chars, ≈442 pt at LABEL) is wider than either side
//! column, so as an unwrapped centred line it crossed
//! the card's right edge and past the window (mission
//! overflow scan: right edge 1475, +63 pt, in every
//! Animate tree). Fixed at the root in wave 2:
//! `design::empty_state` now wraps its message at the
//! card width, so this panel uses the kit builder
//! unchanged and the message stays inside the card.
//!
//! UI mission Wave 9 (deep polish): the populated clip
//! ROW had the same disease the empty state had, one
//! level down — six action buttons plus three text runs
//! on one line totalled ≈424–504 pt of fixed width
//! against the row's 300 pt, and no headless scene ever
//! populates `Project::clips`, so the overflow scan
//! never contained a clip row to catch it. The row is
//! now a three-line stack in the same `list_row` shell
//! (see `clip_row`'s docs for the full accounting) and
//! carries an "Editing" marker for the clip an edit
//! session is on (`is_editing`). Keys and handlers are
//! unchanged.
//!
//! UI mission Wave 10 (session honesty, round 2): a
//! lifecycle audit of `core.clip_edit` against the panel.
//! Panel-mediated paths were already sound — `delete_clip`
//! / `move_clip` remap the core session in the same write,
//! and `delete_clip_at` / `move_clip_at` repair the
//! selection and rename target (pinned by tests below,
//! including the session remap, which round 1 left
//! untested at this level). Two defects were panel-side
//! and are fixed here: `session_label` rendered a
//! dangling session (reachable via undo — snapshots
//! restore `Project` only) as "frame 1 of 0", and
//! `apply_rename` trusted its stored index even when the
//! clip list had changed underneath the open field
//! through a path the panel did not mediate (undo/redo,
//! live MCP); it now checks the name `begin_rename`
//! captured and refuses a write onto a different clip.
//! One gap is the ENGINE's, reported not fixed: undo/redo
//! can silently retarget a live session — undoing a clip
//! deletion restores the clip at its old index while
//! `clip_edit.clip` keeps its remapped value, so the
//! session (and this panel's marker and label, which
//! faithfully follow the index) ends up editing a
//! different clip than the user entered. Nothing
//! panel-side can detect that; the repair belongs with
//! the snapshot format or the undo/redo ops in
//! editor.rs.
//!
//! UI mission Wave 11 (MCP end-to-end audit): the
//! undo/redo half of that gap is closed core-side now —
//! `EditorCore::undo` / `redo` run
//! `repair_clip_session_after_history` (editor.rs:296),
//! which drops a session whose clip's name no longer
//! matches the name captured at entry, and the MCP
//! `undo` / `redo` tools call those same ops, so both
//! transports get the guard. The remaining unguarded
//! path is the WHOLESALE PROJECT SWAP: `open_project_file`
//! (editor.rs:4116), `load` (editor.rs:1787),
//! `open_via_dialog`, the shell's raw swaps
//! (`project_tools::new_project`, `apply_recovered_project`)
//! and MCP's `new_project` tool all replace
//! `core.project` without touching `clip_edit`, and
//! `note_project_swapped` (editor.rs:4057) resets only
//! the edit counters. When the incoming project has a
//! clip at the session's index, the core's own session
//! validation (`clip_session`, editor.rs:3822) checks
//! only that the index exists — so without a panel-side
//! guard the session would keep editing, and this panel
//! would keep labelling, a clip from a different project
//! than the one the user entered. The engine repair
//! (drop or name-check the session in
//! `note_project_swapped`, the one call every swap path
//! already makes) is reported for the integrator; until
//! it lands, this panel gates on identity itself:
//! `session_clip_intact` applies the core repair's own
//! name test, `session_label` calls a mismatched session
//! stale instead of naming the clip now at its index,
//! the row's "Editing" marker (`is_editing`) requires
//! the same match, and the session section offers only
//! Exit — never the mutating frame controls — while the
//! session is stale or its clip is gone.
//!
//! Wave U (full i18n): every label and composed readout
//! in this panel resolves through `i18n_wire::tr` (the
//! subtitle/camera/3D idiom) — the title, the clip-count
//! meta, the header actions, the empty state, the row's
//! "Editing" marker and loop-mode words, the six row
//! actions' help labels, the rename row (placeholder,
//! Apply, Cancel), the loop-mode caption, hint and
//! segmented words, the session section's buttons and
//! hint, and the composed readouts themselves: the row's
//! span · usage line, `span_text`, `usage_text` and all
//! four `session_label` shapes resolve from `clips_*`
//! keys (IA-U2-1), the readouts as `{slot}` templates
//! substituted with `.replace()` at the composing site
//! (the camera `follow_label` precedent). English is
//! byte-identical; the pin is
//! `clips_i18n_keys_exist_and_match_the_english_labels`.
//! The keys land via IA-U2-1. Status sentences stay
//! English by design — they are handler feedback on
//! `core.status` ("No such clip.", "Clip changed while
//! renaming — rename cancelled."), not labels. Clip and
//! figure NAMES inside the readouts are user data carried
//! through the `{name}` / `{names}` slots, never
//! translated; the `", "` name join inside `usage_text`
//! is data punctuation, kept literal.

use std::cell::RefCell;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El, TextEdit, Tx};
use stykanimate_rs::editor::{ClipEditSession, EditorCore};
use stykanimate_rs::model::{ClipDef, ClipLoop, FigureInstance, StateKey};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons;
use crate::State;

/// Panel-local state for the movieclips panel (see the module
/// docs for why this lives neither in the core nor in the
/// shell `State`). Only the selection is needed: clip
/// building and insertion are whole-timeline ops with no
/// range input to hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipsPanelState {
    /// Index into `Project::clips` of the selected clip, if
    /// any. May go stale when the clip list changes; readers
    /// clamp it with `clamped` before use.
    pub selected: Option<usize>,
}

impl ClipsPanelState {
    /// Select clip `index` (a list-row press).
    pub fn select(&mut self, index: usize) {
        self.selected = Some(index);
    }

    /// The selection, if it still names an existing clip.
    pub fn clamped(&self, clip_count: usize) -> Option<usize> {
        match self.selected {
            Some(i) if i < clip_count => Some(i),
            _ => None,
        }
    }
}

thread_local! {
    static CLIPS_PANEL: RefCell<ClipsPanelState> =
        RefCell::new(ClipsPanelState::default());
}

/// Rename-field state for the movieclips panel: which clip
/// is being renamed and the name being typed. Unlike the
/// selection (see the module docs) this cannot live in the
/// thread-local cell — a `TextEdit` is not `Copy`, and
/// `text_input`'s `on_edit` needs a `&mut TextEdit` path
/// into the shell `State`. The integrator homes this in
/// `State` (the exact field + init lines are in the module
/// docs).
pub struct ClipRenameState {
    /// Index into `Project::clips` of the clip being
    /// renamed, or `None` while the rename row is closed.
    /// May go stale (undo can shrink the clip list);
    /// readers bounds-check before use.
    pub target: Option<usize>,
    /// The name the target clip carried when the rename
    /// was opened (`begin_rename`). Indices are not
    /// identity: paths that mutate the clip list without
    /// the panel mediating — undo/redo (their snapshots
    /// restore `Project` only) or the live MCP server —
    /// can leave a different clip sitting at the target
    /// index, and `apply_rename` refuses to write when the
    /// clip now there does not carry this name. `None`
    /// when no rename was opened through `begin_rename`.
    pub opened_name: Option<String>,
    /// The in-progress name, loaded from the target clip
    /// by `begin_rename`.
    pub field: TextEdit,
}

impl ClipRenameState {
    pub fn new() -> Self {
        ClipRenameState {
            target: None,
            opened_name: None,
            field: TextEdit::new(""),
        }
    }
}

/// Read the panel state (a copy — it is one word).
fn panel_state() -> ClipsPanelState {
    CLIPS_PANEL.with(|p| *p.borrow())
}

/// Mutate the panel state (row presses).
fn update_panel_state(f: impl FnOnce(&mut ClipsPanelState)) {
    CLIPS_PANEL.with(|p| f(&mut p.borrow_mut()));
}

/// Wave 14 (shell control): the clips panel's selection,
/// exposed for the MCP shell channel — `shell_select_clip`
/// writes it (after validating the index against the live
/// project), the shell-state snapshot reads it. Until this
/// wave the selection was reachable only from inside this
/// module, which is why the Wave 13 state read could not
/// report it.
pub fn set_clip_selection(selected: Option<usize>) {
    update_panel_state(|p| p.selected = selected);
}

/// The clips panel's current selection (see
/// [`set_clip_selection`]).
pub fn clip_selection() -> Option<usize> {
    CLIPS_PANEL.with(|p| p.borrow().selected)
}

/// Display label for a clip loop mode (the GPUI panel's
/// wording: "Loop" / "Ping-pong" / "Once") — resolved
/// through the `clips_loop*` keys (Wave U, IA-U2-1); the
/// loop-mode segmented control's buttons share the keys.
/// These are display words only: `ClipLoop` serialises
/// as its enum variants, never as these strings.
pub fn loop_label(core: &EditorCore, mode: ClipLoop) -> String {
    match mode {
        ClipLoop::Loop => i18n_wire::tr(core, "clips_loop"),
        ClipLoop::PingPong => i18n_wire::tr(core, "clips_loop_pingpong"),
        ClipLoop::Once => i18n_wire::tr(core, "clips_loop_once"),
    }
}

/// Position in the core's cycle order (Loop -> PingPong ->
/// Once -> Loop, as `EditorCore::cycle_clip_loop` steps it).
fn loop_order(mode: ClipLoop) -> u32 {
    match mode {
        ClipLoop::Loop => 0,
        ClipLoop::PingPong => 1,
        ClipLoop::Once => 2,
    }
}

/// How many `cycle_clip_loop` calls take a clip from `from`
/// to `to` (0, 1 or 2).
pub fn cycle_steps(from: ClipLoop, to: ClipLoop) -> u32 {
    (loop_order(to) + 3 - loop_order(from)) % 3
}

/// Set clip `clip_idx`'s loop mode to `target` through the
/// core's only loop op, `cycle_clip_loop` (editor.rs:3473).
/// Each cycle is one undo step (the op snapshots), so this
/// costs at most two; targeting the current mode calls
/// nothing. An out-of-range index is a silent no-op — the
/// panel only ever passes indices from its own rendered,
/// clamped list.
pub fn apply_loop_mode(core: &mut EditorCore, clip_idx: usize, target: ClipLoop) {
    let Some(clip) = core.project.clips.get(clip_idx) else {
        return;
    };
    let steps = cycle_steps(clip.loop_mode, target);
    for _ in 0..steps {
        core.cycle_clip_loop(clip_idx);
    }
}

/// The name a rename applies: the field text trimmed, with
/// the clip's current name as the fallback when nothing
/// usable was typed. Clip names are printed by the list
/// rows, the session label and the core's own status
/// lines, so a clip must never end up nameless.
pub fn resolved_clip_name(field_text: &str, current: &str) -> String {
    let trimmed = field_text.trim();
    if trimmed.is_empty() {
        current.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Duplicate clip `clip_idx` (the per-row "Dup" button's
/// dispatch). No core op exists for this, so it is a field
/// write mirroring `cycle_clip_loop`'s discipline exactly:
/// an out-of-range index only sets the status line ("No
/// such clip.", no snapshot); otherwise exactly one
/// `snapshot()`, the write, and a status line. The clone
/// is APPENDED (named `"<name> copy"`, like
/// `duplicate_figure`), so no stored clip index shifts:
/// figures keep playing the original and the copy starts
/// unused. Returns the copy's index.
pub fn duplicate_clip(core: &mut EditorCore, clip_idx: usize) -> Option<usize> {
    // Wave 11 port (C6 §5): the op lives on EditorCore now;
    // this dispatch delegates so panel and MCP share it.
    core.duplicate_clip(clip_idx)
}

/// Rename clip `clip_idx` to the resolved field text (the
/// rename row's "Apply" dispatch). Field-write discipline
/// as `duplicate_clip`; a rename that resolves to the
/// current name is a no-op — no snapshot, no status churn
/// (the `apply_loop_mode` precedent of not calling the op
/// when already at the target). Returns true when the name
/// actually changed (exactly one undo step).
pub fn rename_clip(core: &mut EditorCore, clip_idx: usize, field_text: &str) -> bool {
    // Wave 11 port (C6 §5): delegates to EditorCore.
    core.rename_clip(clip_idx, field_text)
}

/// Selection after clip `deleted` was removed, with
/// `len_after` clips remaining: a selection before the gap
/// is untouched, one after it shifts down by one, and the
/// deleted clip's own selection lands on the clip that
/// slid into its slot (or the new last clip, or `None`
/// when the list is now empty). Mirrors the FX panel's
/// helper of the same name (`fx_clips.rs`) exactly — the
/// two panels repair selection the same way.
pub fn selection_after_delete(
    selected: Option<usize>,
    deleted: usize,
    len_after: usize,
) -> Option<usize> {
    let sel = selected?;
    if len_after == 0 {
        return None;
    }
    if sel < deleted {
        Some(sel)
    } else if sel > deleted {
        Some(sel - 1)
    } else {
        Some(deleted.min(len_after - 1))
    }
}

/// Where a tracked clip index (the rename target, the
/// clip-edit session's clip, a figure's `clip` reference)
/// lands when clip `deleted` is removed: before the gap it
/// is untouched, after it it shifts down by one, and the
/// deleted clip itself maps to `None` — tracking ends,
/// because the clip it named is gone. (The panel selection
/// deliberately differs: it lands on the slid-in clip
/// instead — see `selection_after_delete`.)
pub fn target_after_delete(target: Option<usize>, deleted: usize) -> Option<usize> {
    let t = target?;
    if t < deleted {
        Some(t)
    } else if t > deleted {
        Some(t - 1)
    } else {
        None
    }
}

/// Where a tracked clip index lands when clips `a` and
/// `b` trade places: the two swap, every other index is
/// untouched.
pub fn target_after_swap(target: Option<usize>, a: usize, b: usize) -> Option<usize> {
    target.map(|t| {
        if t == a {
            b
        } else if t == b {
            a
        } else {
            t
        }
    })
}

/// Delete clip `clip_idx` (the per-row "Del" button's
/// dispatch). No core op exists for this, so it is a field
/// write with the same discipline as `duplicate_clip`: an
/// out-of-range index only sets the status line ("No such
/// clip.", no snapshot); otherwise exactly one
/// `snapshot()`, the removal, and a status line. Because
/// removal shifts every later clip index, the same write
/// remaps the project's other stored clip indices through
/// `target_after_delete`: figure `clip` references (a
/// figure whose clip is deleted falls back to its
/// timeline poses) and the clip-edit session's clip (the
/// session follows its clip and ends when that clip is
/// deleted). The panel selection and the rename target
/// live outside the core; `delete_clip_at` repairs them.
/// Returns true when a clip was actually removed.
pub fn delete_clip(core: &mut EditorCore, clip_idx: usize) -> bool {
    // Wave 11 port (C6 §5): delegates to EditorCore; the
    // panel-local repairs stay in `delete_clip_at` below.
    core.delete_clip(clip_idx)
}

/// Move clip `clip_idx` one slot toward the top (`up`) or
/// the bottom of the list (the per-row "Up" / "Dn"
/// buttons' dispatch). Field-write discipline as
/// `delete_clip`: an out-of-range index, or a clip already
/// at the end it would move past, only sets the status
/// line (no snapshot); otherwise exactly one `snapshot()`,
/// a `Vec::swap` with the neighbour, and a status line.
/// Stored clip indices are remapped through
/// `target_after_swap` in the same write, so figures keep
/// playing the clip they were playing and the clip-edit
/// session follows its clip. Returns true when the swap
/// happened.
pub fn move_clip(core: &mut EditorCore, clip_idx: usize, up: bool) -> bool {
    // Wave 11 port (C6 §5): delegates to EditorCore; the
    // panel-local repairs stay in `move_clip_at` below.
    core.move_clip(clip_idx, up)
}


/// Insert a figure playing clip `clip_idx` (the per-row
/// "Insert" button's dispatch). The core's own op,
/// `EditorCore::insert_clip_figure`, always inserts the
/// FIRST clip — it takes no index — so this is that op's
/// body with the index parameterised, under this module's
/// field-write discipline: an out-of-range index only
/// sets the status line ("No such clip.", no snapshot);
/// otherwise exactly one `snapshot()`, then the engine
/// op's construction unchanged — a figure built by
/// `Project::add_figure_with` from the clip's def and
/// name (unlocked, visible, full opacity, the def's rest
/// pose rooted at `[380 + 40 * idx, 450]` pushed into
/// every frame), coloured `FIGURE_COLORS[(idx + 3) % 6]`
/// where `idx` is the pre-insert figure count, then
/// pointed at the clip (`clip: Some(clip_idx)`), selected,
/// and confirmed with the engine op's own status line
/// ("Inserted movieclip {name}."). The clips vec itself
/// is untouched — nothing added, removed or reordered —
/// so no stored clip index shifts and neither the panel
/// selection nor an open rename target needs repair
/// (contrast `delete_clip` / `move_clip`, whose dispatch
/// wrappers exist for exactly that repair). Returns true
/// when a figure was inserted.
pub fn insert_clip_figure_at(core: &mut EditorCore, clip_idx: usize) -> bool {
    // Wave 11 port (C6 §5): delegates to EditorCore
    // (the parameterised op; `insert_clip_figure` remains
    // the first-clip engine op).
    core.insert_clip_figure_at(clip_idx)
}

/// Delete clip `clip_idx` from the panel (the "Del"
/// button's handler): `delete_clip` on the core, then the
/// panel-local repairs the core cannot do — the
/// thread-local selection via `selection_after_delete`
/// and an open rename target via `target_after_delete`
/// (a rename whose clip was deleted closes, because its
/// target maps to `None`).
pub fn delete_clip_at(s: &mut State, clip_idx: usize) {
    if !delete_clip(&mut s.core, clip_idx) {
        return;
    }
    let len_after = s.core.project.clips.len();
    update_panel_state(|p| {
        p.selected = selection_after_delete(p.selected, clip_idx, len_after);
    });
    s.clips_rename.target = target_after_delete(s.clips_rename.target, clip_idx);
    if s.clips_rename.target.is_none() {
        s.clips_rename.opened_name = None;
    }
}

/// Move clip `clip_idx` one slot from the panel (the
/// "Up" / "Dn" buttons' handler): `move_clip` on the
/// core, then the panel selection and an open rename
/// target follow the moved clip via `target_after_swap`.
pub fn move_clip_at(s: &mut State, clip_idx: usize, up: bool) {
    if !move_clip(&mut s.core, clip_idx, up) {
        return;
    }
    let other = if up { clip_idx - 1 } else { clip_idx + 1 };
    update_panel_state(|p| {
        p.selected = target_after_swap(p.selected, clip_idx, other);
    });
    s.clips_rename.target = target_after_swap(s.clips_rename.target, clip_idx, other);
}

/// Load clip `clip_idx`'s name into the rename field and
/// aim the field at it (the per-row "Rename" button's
/// dispatch). The fresh field claims keyboard focus (the
/// command palette's open pattern), so typing goes to the
/// name at once. The clip's current name is captured in
/// `opened_name` — the identity `apply_rename` checks
/// before writing (see `ClipRenameState::opened_name`).
/// An out-of-range index only sets the core's status line
/// and leaves the field closed.
pub fn begin_rename(s: &mut State, clip_idx: usize) {
    let Some(name) = s.core.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
        s.core.status = "No such clip.".into();
        return;
    };
    let mut field = TextEdit::new(name.clone());
    field.focused = true;
    s.clips_rename.field = field;
    s.clips_rename.target = Some(clip_idx);
    s.clips_rename.opened_name = Some(name);
}

/// Write the rename field back to its target clip (the
/// "Apply" button's and the field's Enter dispatch). The
/// editor closes either way: `rename_clip` refuses a stale
/// target with a status line only, and a successful or
/// no-op rename is done.
///
/// One refusal is the panel's own: when the clip now
/// sitting at the target index is not the clip the rename
/// was opened on — its name differs from the name
/// `begin_rename` captured, because the clip list changed
/// underneath the open field through a path the panel did
/// not mediate (undo/redo restore `Project` only; the
/// live MCP server can mutate clips at any time) — the
/// write is refused with a status line rather than
/// renaming a clip the user never opened. A target set
/// without `begin_rename` (no captured name) keeps the
/// old behaviour: `rename_clip` alone decides.
pub fn apply_rename(s: &mut State) {
    let Some(target) = s.clips_rename.target else {
        return;
    };
    if let Some(opened) = s.clips_rename.opened_name.clone() {
        if let Some(clip) = s.core.project.clips.get(target) {
            if clip.name != opened {
                s.core.status = "Clip changed while renaming — rename cancelled.".into();
                s.clips_rename.target = None;
                s.clips_rename.opened_name = None;
                return;
            }
        }
    }
    let text = s.clips_rename.field.text.clone();
    // The dispatch resolves the typed text (the module doc's
    // contract); the core op resolves again, idempotently.
    let current = s
        .core
        .project
        .clips
        .get(target)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let resolved = resolved_clip_name(&text, &current);
    rename_clip(&mut s.core, target, &resolved);
    s.clips_rename.target = None;
    s.clips_rename.opened_name = None;
}

/// Whether a clip-editing session is active on the core
/// (`core.clip_edit` set) — the pure predicate the session
/// section's visibility hangs on. The core self-heals a
/// session whose clip vanished (its `clip_session` drops it
/// on the next session op), so a bare `is_some` is the right
/// view-level test; the Exit control stays reachable either
/// way.
pub fn session_active(core: &EditorCore) -> bool {
    core.clip_edit.is_some()
}

/// Whether the live session still names the clip it was
/// entered on (Wave 11): the clip at the session's index
/// exists AND carries the name `enter_clip` captured
/// into the session. This is the core's own identity
/// test — `repair_clip_session_after_history`
/// (editor.rs:296) applies exactly it after undo/redo —
/// extended to the paths that never run that repair:
/// wholesale project swaps (open / load / new / recover,
/// including MCP's `new_project` tool on the live core)
/// replace `core.project` without touching `clip_edit`,
/// so a session can survive onto a different project's
/// clip at the same index, and the core's `clip_session`
/// validation (editor.rs:3822) checks only that the
/// index exists. Every panel-mediated flow keeps the
/// identity intact by construction — `enter_clip`
/// captures the name, this module's `rename_clip` syncs
/// it, `delete_clip` / `move_clip` carry the session
/// with its clip — so a mismatch can only arrive from
/// outside the panel, and the panel treats it as stale:
/// the label says so, the "Editing" marker stays off,
/// and the session section offers Exit only. Two clips
/// sharing one name remain indistinguishable, the same
/// documented residual the core repair carries.
pub fn session_clip_intact(core: &EditorCore) -> bool {
    let Some(session) = core.clip_edit.as_ref() else {
        return false;
    };
    core.project
        .clips
        .get(session.clip)
        .map(|clip| clip.name == session.name)
        .unwrap_or(false)
}

/// Whether the clip-edit session is editing clip `index`
/// (UI mission Wave 9) — the clip row's "Editing" marker
/// predicate. The session section below the list names the
/// edited clip, but the row itself carried no sign of it:
/// the selection can move mid-session (row presses stay
/// live), so the edited clip and the selected clip can be
/// different rows with nothing to tell them apart. Wave 11:
/// the marker also requires `session_clip_intact` — after
/// an unmediated project swap the session's index can sit
/// on a different project's clip, and marking that row
/// "Editing" would claim the session is editing a clip it
/// was never entered on.
pub fn is_editing(core: &EditorCore, index: usize) -> bool {
    core.clip_edit.as_ref().map(|session| session.clip) == Some(index)
        && session_clip_intact(core)
}

/// The session section's label while a session is active:
/// which clip is being edited and where the session sits in
/// it ("Editing <name> · frame <f> of <n>"). `None` when no
/// session is active. A session whose clip is missing from
/// the project — reachable via undo, whose snapshots restore
/// `Project` only and never repair `clip_edit` — still
/// labels, but honestly (Wave 10): it names the clip by
/// index and says it no longer exists, instead of the old
/// "frame 1 of 0" the live format produced for a zero
/// count; a clip that exists with no frames says "no
/// frames" the same way. Either way the next session op
/// closes such a session (the core's `clip_session` drops
/// it) and Exit stays reachable throughout. Wave 11 adds
/// the third dishonest shape: a session whose clip EXISTS
/// at its index but is not the clip it was entered on
/// (its name differs from the captured `session.name` —
/// the wholesale-swap retarget; see
/// `session_clip_intact`). That labels as a stale session
/// naming the clip the user actually entered, never the
/// stranger now sitting at the index.
pub fn session_label(core: &EditorCore) -> Option<String> {
    let session = core.clip_edit.as_ref()?;
    let Some(clip) = core.project.clips.get(session.clip) else {
        return Some(
            i18n_wire::tr(core, "clips_session_gone")
                .replace("{index}", &(session.clip + 1).to_string()),
        );
    };
    if clip.name != session.name {
        return Some(
            i18n_wire::tr(core, "clips_session_stale")
                .replace("{name}", &session.name),
        );
    }
    if clip.frames.is_empty() {
        return Some(
            i18n_wire::tr(core, "clips_session_no_frames")
                .replace("{name}", &clip.name),
        );
    }
    let frames = clip.frames.len();
    let frame = session.frame.min(frames.saturating_sub(1));
    Some(
        i18n_wire::tr(core, "clips_session_frame")
            .replace("{name}", &clip.name)
            .replace("{frame}", &(frame + 1).to_string())
            .replace("{total}", &frames.to_string()),
    )
}

/// Enter the editing session for the panel-selected clip
/// (the "Edit clip" button's dispatch). `enter_clip` takes a
/// clip INDEX (editor.rs:3335) — the panel's clamped
/// thread-local selection. With no valid selection nothing is
/// called and this returns false; an empty clip is refused by
/// the core itself (it sets the status line).
pub fn enter_selected_clip(core: &mut EditorCore) -> bool {
    match panel_state().clamped(core.project.clips.len()) {
        Some(i) => core.enter_clip(i),
        None => false,
    }
}

/// Step the session one clip frame back (the "Prev frame"
/// button's dispatch): `clip_goto` to the previous frame,
/// clamped by the core at the first frame. No-op without a
/// session.
pub fn session_prev(core: &mut EditorCore) {
    if let Some(frame) = core.clip_edit.as_ref().map(|s| s.frame) {
        core.clip_goto(frame.saturating_sub(1));
    }
}

/// Step the session one clip frame forward (the "Next frame"
/// button's dispatch): `clip_goto` to the next frame, clamped
/// by the core at the clip's last frame. No-op without a
/// session.
pub fn session_next(core: &mut EditorCore) {
    if let Some(frame) = core.clip_edit.as_ref().map(|s| s.frame) {
        core.clip_goto(frame + 1);
    }
}

/// The clip's frame span as shown in its list row: a clip
/// always spans its own frames 1..=len. Resolved through
/// the `clips_span*` keys (Wave U, IA-U2-1), the many
/// shape as the `{end}` / `{count}` template.
pub fn span_text(core: &EditorCore, clip: &ClipDef) -> String {
    match clip.frames.len() {
        0 => i18n_wire::tr(core, "clips_span_none"),
        1 => i18n_wire::tr(core, "clips_span_one"),
        n => i18n_wire::tr(core, "clips_span_many")
            .replace("{end}", &n.to_string())
            .replace("{count}", &n.to_string()),
    }
}

/// Which figures play clip `clip_idx` (the GPUI row's usage
/// text). Resolved through the `clips_usage*` keys (Wave
/// U, IA-U2-1); the names are user data carried through
/// the `{names}` slot, joined with the literal `", "`.
/// Wave X (P-1): a figure also counts as playing the clip
/// when one of its state keys names it — the base-link-only
/// filter would call a state-keyed clip "not used".
pub fn usage_text(core: &EditorCore, figures: &[FigureInstance], clip_idx: usize) -> String {
    let names: Vec<&str> = figures
        .iter()
        .filter(|f| {
            f.clip == Some(clip_idx) || f.states.iter().any(|k| k.clip == clip_idx)
        })
        .map(|f| f.name.as_str())
        .collect();
    if names.is_empty() {
        i18n_wire::tr(core, "clips_usage_none")
    } else {
        i18n_wire::tr(core, "clips_usage_some").replace("{names}", &names.join(", "))
    }
}

/// The figure's state keys in timeline order, each paired
/// with its clip's name (Wave X, P-1 / GAP #13 — the derived
/// named-state registry: no new model field, the rows are
/// `FigureInstance::states` read against the clip pool).
/// Keys whose clip no longer exists (a clip deleted outside
/// the remapping ops) are skipped — the derivation never
/// panics on a dangling index. The sort is stable, so keys
/// sharing a frame keep authoring order, matching the
/// model's tie rule in `model::active_state_key`.
pub fn state_rows(core: &EditorCore, fi: usize) -> Vec<(StateKey, String)> {
    let Some(fig) = core.project.figures.get(fi) else {
        return Vec::new();
    };
    let mut rows: Vec<(StateKey, String)> = fig
        .states
        .iter()
        .filter_map(|key| {
            core.project
                .clips
                .get(key.clip)
                .map(|clip| (*key, clip.name.clone()))
        })
        .collect();
    rows.sort_by_key(|(key, _)| key.frame);
    rows
}

/// Which clip figure `fi` plays at `frame`: the state key
/// in force (`model::active_state_key`) when one applies,
/// else the figure's base link. `None` = the figure plays
/// its timeline poses there.
pub fn state_clip_at(core: &EditorCore, fi: usize, frame: usize) -> Option<usize> {
    let fig = core.project.figures.get(fi)?;
    if let Some(key) = stykanimate_rs::model::active_state_key(fig, frame as f32) {
        return Some(key.clip);
    }
    fig.clip
}

/// Key `clip` for figure `fi` at `frame`, starting at the
/// clip's first frame at the timeline rate (offset 0, speed
/// 1.0) — the panel's click-to-key-at-playhead write. The
/// write itself is the core's `set_state_key` op (Wave X,
/// X2), so the panel and MCP author keys through one path,
/// rig guard included; an existing key at the frame is
/// replaced wholesale by the op — its offset/speed belonged
/// to the old clip's clock. A key that already says exactly
/// this is a no-op with no snapshot (a panel-side read,
/// the `set_clip_loop_mode` discipline); every other case
/// is the op's one snapshot and status line.
pub fn key_state_at(core: &mut EditorCore, fi: usize, frame: usize, clip: usize) {
    let already = core.project.figures.get(fi).is_some_and(|fig| {
        fig.states.iter().any(|k| {
            k.frame == frame && k.clip == clip && k.offset == 0 && k.speed == 1.0
        })
    });
    if already {
        return;
    }
    core.set_state_key(
        fi,
        StateKey {
            frame,
            clip,
            offset: 0,
            speed: 1.0,
        },
    );
}

/// Remove figure `fi`'s state key at `frame` — the core's
/// `remove_state_key` op (Wave X, X2): false with a status
/// line (no snapshot) when the figure does not exist or no
/// key sits at that frame; otherwise one snapshot and the
/// removal.
pub fn clear_state_at(core: &mut EditorCore, fi: usize, frame: usize) -> bool {
    core.remove_state_key(fi, frame)
}

/// One state key's row label: the plain template, or the
/// offset/speed template when the key departs from the
/// defaults (a key authored through MCP with an offset or
/// a non-1.0 speed). Frames display 1-based, as everywhere
/// in this panel (`span_text`).
pub fn state_key_text(core: &EditorCore, key: &StateKey, clip_name: &str) -> String {
    if key.offset == 0 && key.speed == 1.0 {
        i18n_wire::tr(core, "clips_states_key")
            .replace("{frame}", &(key.frame + 1).to_string())
            .replace("{clip}", clip_name)
    } else {
        i18n_wire::tr(core, "clips_states_key_fx")
            .replace("{frame}", &(key.frame + 1).to_string())
            .replace("{clip}", clip_name)
            .replace("{offset}", &key.offset.to_string())
            .replace("{speed}", &format!("{}", key.speed))
    }
}

/// The derived named-state section (Wave X, P-1 / GAP #13),
/// mounted under the loop-mode row: for the SELECTED figure —
/// its base link, its state keys in timeline order (each
/// with a clear button), and a keying strip: one button per
/// clip in the pool, labelled with the clip's name, the clip
/// in force at the playhead marked selected; pressing keys
/// that clip at the playhead frame (`key_state_at`).
/// Everything shown is derived from
/// `FigureInstance::{clip, states}` + the pool — the section
/// stores nothing of its own. With no figure at the
/// selection it renders the bare keyed column.
fn states_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let mut section = col().key("clip-states-section").w_full().gap(6.0);
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return section;
    };
    let fi = core.selected_figure;
    let frame = core.playhead.floor() as usize;
    let title = i18n_wire::tr(core, "clips_states_figure").replace("{name}", &fig.name);
    section = section.child(design::section_label(&title, t));
    let base = match fig.clip {
        Some(ci) => match core.project.clips.get(ci) {
            Some(clip) => i18n_wire::tr(core, "clips_states_base").replace("{clip}", &clip.name),
            None => i18n_wire::tr(core, "clips_states_no_base"),
        },
        None => i18n_wire::tr(core, "clips_states_no_base"),
    };
    section = section.child(
        text(base)
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
    );
    let rows = state_rows(core, fi);
    if rows.is_empty() {
        section = section.child(
            text(i18n_wire::tr(core, "clips_states_none"))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        );
    }
    for (key, clip_name) in rows {
        section = section.child(
            row()
                .key(&format!("clip-state-row-{}", key.frame))
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text(state_key_text(core, &key, &clip_name))
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0)
                        .truncate(),
                )
                .child(icon_only_btn(
                    &format!("clip-state-clear-{}", key.frame),
                    "xmark",
                    &i18n_wire::tr(core, "clips_states_clear"),
                    26.0,
                    false,
                    t,
                    move |s: &mut State| {
                        clear_state_at(&mut s.core, fi, key.frame);
                    },
                )),
        );
    }
    let strip_label = i18n_wire::tr(core, "clips_states_key_here")
        .replace("{frame}", &(frame + 1).to_string());
    section = section.child(
        text(strip_label)
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    let active = state_clip_at(core, fi, frame);
    for (ci, clip) in core.project.clips.iter().enumerate() {
        section = section.child(btn(
            &format!("clip-state-key-{ci}"),
            &clip.name,
            active == Some(ci),
            t,
            move |s: &mut State| {
                key_state_at(&mut s.core, fi, frame, ci);
            },
        ));
    }
    section
}

/// One clip row: name, span + usage, current loop mode.
/// Pressing the row selects the clip (panel-local state) for
/// the loop-mode segmented control below the list.
///
/// UI mission Wave 9 (width proof): the row used to lay ALL
/// of that plus six action buttons on ONE line inside
/// `list_row`. The fixed children alone — four labelled
/// `icon_btn`s (≈58–87 pt each: 2×10 padding + 13 glyph +
/// 6 gap + label), two 26 pt icon-only buttons, the loop
/// label and the row's eight 8 pt gaps — total ≈424 pt at
/// realistic LABEL advances, ≈504 pt at the status strip's
/// conservative 8 pt/char accounting, against the 300 pt a
/// right-column card leaves inside the row's own padding
/// (316 content − 2×8). A populated row overflowed its
/// card under EVERY accounting, name and span text still
/// uncounted; the mission's overflow scan never saw it
/// because no headless scene populates `Project::clips`
/// (scenes seed `fx_clips` only), so no tree ever contained
/// a clip row. The row is now a three-line stack inside the
/// same `list_row` shell: name (truncating, grown) + the
/// session's "Editing" marker + loop mode on line 1, the
/// span · usage line wrapped on line 2 (the kit's prose
/// treatment), and the six actions as `icon_only_btn`s on
/// line 3 — 6×26 + 5×8 = 196 pt, inside the budget under
/// every accounting. Keys and handlers are byte-identical;
/// the four buttons that lost their visible words carry
/// them as help labels instead (the Wave 6 icon-only
/// convention: the label names the action).
fn clip_row(s: &State, t: &Theme, index: usize, selected: bool) -> El {
    let core = &s.core;
    let clip = &core.project.clips[index];
    let mut name_line = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(clip.name.clone())
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0)
                .truncate(),
        );
    if is_editing(core, index) {
        name_line = name_line.child(
            text(i18n_wire::tr(core, "clips_editing"))
                .font(type_::LABEL)
                .color(crate::rune_sec::app_theme::accent(t)),
        );
    }
    name_line = name_line.child(
        text(loop_label(core, clip.loop_mode))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    design::list_row(&format!("clip-row-{}", index + 1), selected, t)
        .w_full()
        .on_press(move |_s: &mut State| {
            update_panel_state(|p| p.select(index));
        })
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(name_line)
                .child(
                    text(
                        i18n_wire::tr(core, "clips_row_detail")
                            .replace("{span}", &span_text(core, clip))
                            .replace(
                                "{usage}",
                                &usage_text(core, &core.project.figures, index),
                            ),
                    )
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(icon_only_btn(
                            &format!("clip-dup-{index}"),
                            "copy",
                            &i18n_wire::tr(core, "clips_duplicate"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                if let Some(new_index) = duplicate_clip(&mut s.core, index) {
                                    update_panel_state(|p| p.select(new_index));
                                }
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("clip-rename-{index}"),
                            "pencil",
                            &i18n_wire::tr(core, "clips_rename"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                update_panel_state(|p| p.select(index));
                                begin_rename(s, index);
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("clip-insert-{index}"),
                            "plus",
                            &i18n_wire::tr(core, "clips_insert"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                insert_clip_figure_at(&mut s.core, index);
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("clip-up-{index}"),
                            "chevron-up",
                            &i18n_wire::tr(core, "clips_move_up"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                move_clip_at(s, index, true);
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("clip-dn-{index}"),
                            "chevron-down",
                            &i18n_wire::tr(core, "clips_move_down"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                move_clip_at(s, index, false);
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("clip-del-{index}"),
                            "trash-can",
                            &i18n_wire::tr(core, "clips_delete"),
                            26.0,
                            false,
                            t,
                            move |s: &mut State| {
                                delete_clip_at(s, index);
                            },
                        )),
                ),
        )
}

/// The rename editor row, mounted under the clip list
/// while a rename is in progress (`ClipRenameState::target`
/// set and still naming an existing clip): the loaded name
/// field plus "Apply" / "Cancel"; Enter in the field
/// applies too, Cancel closes the row without writing.
/// Closed — or a target gone stale — it renders an empty
/// keyed row, so the panel never shows a dead field.
fn rename_row(s: &State, t: &Theme) -> El {
    let open = s
        .clips_rename
        .target
        .is_some_and(|i| i < s.core.project.clips.len());
    if !open {
        return row().key("clip-rename-row").w_full();
    }
    row()
        .key("clip-rename-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            design::text_field(&s.clips_rename.field, type_::BODY, s.clips_rename.field.focused, t)
                .key("clip-rename-field")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder(
                    &i18n_wire::tr(&s.core, "clips_rename_placeholder"),
                    t.text_secondary,
                )
                .on_edit(|s: &mut State| &mut s.clips_rename.field)
                .on_submit(|s: &mut State, _tx: &mut Tx| apply_rename(s)),
        )
        .child(icon_btn(
            "clip-rename-apply",
            "check",
            &i18n_wire::tr(&s.core, "clips_apply"),
            false,
            t,
            |s: &mut State| apply_rename(s),
        ))
        .child(icon_btn(
            "clip-rename-cancel",
            "xmark",
            &i18n_wire::tr(&s.core, "clips_cancel"),
            false,
            t,
            |s: &mut State| {
                s.clips_rename.target = None;
                // Wave 11: clear the captured identity too —
                // `apply_rename` and `delete_clip_at` both
                // leave the pair (target, opened_name)
                // either fully set or fully cleared, and a
                // stale opened_name beside a closed target
                // was the one path that broke the pairing.
                // (Unreachable today — `apply_rename`
                // early-returns on a `None` target and
                // `begin_rename` overwrites both — but the
                // invariant is now uniform.)
                s.clips_rename.opened_name = None;
            },
        ))
}

/// The loop-mode segmented control for the selected clip:
/// Loop / Ping-pong / Once, the clip's current mode
/// accent-filled (via `chrome_view::btn`). With no selection
/// it renders a hint instead of dead controls.
fn loop_mode_row(s: &State, t: &Theme, selected: Option<usize>) -> El {
    let Some(index) = selected else {
        return row().key("clip-loop-mode").w_full().child(
            text(i18n_wire::tr(&s.core, "clips_loop_hint"))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    let mode = s.core.project.clips[index].loop_mode;
    row()
        .key("clip-loop-mode")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "clips_loop_mode"),
            t,
        ))
        .child(btn(
            "clip-loop-loop",
            &i18n_wire::tr(&s.core, "clips_loop"),
            mode == ClipLoop::Loop,
            t,
            |s: &mut State| {
                if let Some(i) = panel_state().clamped(s.core.project.clips.len()) {
                    apply_loop_mode(&mut s.core, i, ClipLoop::Loop);
                }
            },
        ))
        .child(btn(
            "clip-loop-pingpong",
            &i18n_wire::tr(&s.core, "clips_loop_pingpong"),
            mode == ClipLoop::PingPong,
            t,
            |s: &mut State| {
                if let Some(i) = panel_state().clamped(s.core.project.clips.len()) {
                    apply_loop_mode(&mut s.core, i, ClipLoop::PingPong);
                }
            },
        ))
        .child(btn(
            "clip-loop-once",
            &i18n_wire::tr(&s.core, "clips_loop_once"),
            mode == ClipLoop::Once,
            t,
            |s: &mut State| {
                if let Some(i) = panel_state().clamped(s.core.project.clips.len()) {
                    apply_loop_mode(&mut s.core, i, ClipLoop::Once);
                }
            },
        ))
}

/// The clip-edit session section, appended under the
/// loop-mode row. Three states, all driven by the core:
///
/// * Session active (`core.clip_edit` set): the session label
///   (clip name + frame position from `session_label`), the
///   frame controls — Prev/Next clip frame (`session_prev` /
///   `session_next` over `clip_goto`), Add frame
///   (`clip_add_frame`), Delete frame (`clip_delete_frame`) —
///   and Exit (`exit_clip`). Switching clips mid-session is
///   deliberately not offered: exit first, then enter another.
/// * Session active but STALE (Wave 11 — the session's clip
///   is gone, or a different clip sits at its index after an
///   unmediated project swap; `session_clip_intact`): the
///   honest label and Exit ONLY. The mutating frame controls
///   are withheld because the core ops behind them validate
///   only that the index exists (`clip_session`,
///   editor.rs:3822) — offering them would let one press
///   edit a clip the session was never entered on. Exit is
///   always safe: it just drops the session.
/// * No session, clips exist: the "Edit clip" entry button
///   (`enter_selected_clip` on the panel selection), or a hint
///   when nothing is selected — no dead controls.
/// * No session, no clips: an empty section (nothing exists
///   that could be edited).
fn clip_edit_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    if session_active(core) {
        let label = session_label(core).unwrap_or_default();
        let section = col()
            .key("clip-edit-section")
            .w_full()
            .gap(6.0)
            .child(
                text(label)
                    .font(type_::LABEL)
                    .color(t.text),
            );
        if !session_clip_intact(core) {
            // Stale session (Wave 11): Exit only — see the
            // section docs. The frame controls stay
            // unmounted rather than inert, matching the
            // shell's no-dead-controls rule.
            return section.child(btn(
                "clip-edit-exit",
                &i18n_wire::tr(&s.core, "clips_exit"),
                false,
                t,
                |s: &mut State| s.core.exit_clip(),
            ));
        }
        return section
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(btn(
                        "clip-edit-prev",
                        &i18n_wire::tr(&s.core, "clips_prev_frame"),
                        false,
                        t,
                        |s: &mut State| session_prev(&mut s.core),
                    ))
                    .child(btn(
                        "clip-edit-next",
                        &i18n_wire::tr(&s.core, "clips_next_frame"),
                        false,
                        t,
                        |s: &mut State| session_next(&mut s.core),
                    )),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(btn(
                        "clip-edit-add-frame",
                        &i18n_wire::tr(&s.core, "clips_add_frame"),
                        false,
                        t,
                        |s: &mut State| s.core.clip_add_frame(),
                    ))
                    .child(btn(
                        "clip-edit-delete-frame",
                        &i18n_wire::tr(&s.core, "clips_delete_frame"),
                        false,
                        t,
                        |s: &mut State| s.core.clip_delete_frame(),
                    )),
            )
            .child(btn(
                "clip-edit-exit",
                &i18n_wire::tr(&s.core, "clips_exit"),
                false,
                t,
                |s: &mut State| s.core.exit_clip(),
            ));
    }
    let section = col().key("clip-edit-section").w_full().gap(6.0);
    if core.project.clips.is_empty() {
        return section;
    }
    match panel_state().clamped(core.project.clips.len()) {
        Some(_) => section.child(btn(
            "clip-edit-enter",
            &i18n_wire::tr(&s.core, "clips_edit_clip"),
            false,
            t,
            |s: &mut State| {
                enter_selected_clip(&mut s.core);
            },
        )),
        None => section.child(
            text(i18n_wire::tr(&s.core, "clips_edit_hint"))
                .font(type_::LABEL)
                .color(t.text_secondary),
        ),
    }
}

/// The movieclips panel: header + count, the Make/Insert
/// actions, the clip list (each row with Dup, Rename, Up,
/// Dn and Del buttons), the rename editor row while a rename is in
/// progress, the selected clip's loop-mode segmented
/// control, the selected figure's derived state section
/// (`states_section`, Wave X), and the clip-edit session section
/// (`clip_edit_section`: enter/frame-step/add/delete/exit over
/// the core's `enter_clip`/`clip_goto`/`clip_add_frame`/
/// `clip_delete_frame`/`exit_clip` session ops).
pub fn clips_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let clips = &core.project.clips;
    let selected = panel_state().clamped(clips.len());
    let count = i18n_wire::tr(core, "clips_count")
        .replace("{count}", &clips.len().to_string());
    let mut panel = design::panel_col("clips-panel", t)
        .child(
            row()
                .w_full()
                .gap(design::PANEL_GAP)
                .center_y()
                .child(design::panel_title(
                    Some("film"),
                    &i18n_wire::tr(core, "clips_title"),
                    Some(&count),
                    t,
                )),
        )
        .child(design::divider(t))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(btn(
                    "make-clip",
                    &i18n_wire::tr(core, "clips_make"),
                    false,
                    t,
                    |s| s.core.make_clip_from_figure(),
                ))
                .child(btn(
                    "insert-clip",
                    &i18n_wire::tr(core, "clips_insert_clip"),
                    false,
                    t,
                    |s| s.core.insert_clip_figure(),
                )),
        );
    if clips.is_empty() {
        return panel
            .child(design::empty_state(
                "film",
                &i18n_wire::tr(core, "clips_empty"),
                t,
            ))
            .child(clip_edit_section(s, t));
    }
    for i in 0..clips.len() {
        panel = panel.child(clip_row(s, t, i, selected == Some(i)));
    }
    panel
        .child(rename_row(s, t))
        .child(loop_mode_row(s, t, selected))
        .child(states_section(s, t))
        .child(clip_edit_section(s, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::FigureDef;

    fn clip_with_frames(n: usize) -> ClipDef {
        let def = FigureDef::humanoid();
        let frames = vec![def.rest_pose([0.0, 0.0]); n];
        ClipDef {
            name: "test clip".to_string(),
            def,
            frames,
            loop_mode: ClipLoop::Loop,
        }
    }

    #[test]
    fn loop_label_covers_all_modes() {
        let core = EditorCore::new();
        assert_eq!(loop_label(&core, ClipLoop::Loop), "Loop");
        assert_eq!(loop_label(&core, ClipLoop::PingPong), "Ping-pong");
        assert_eq!(loop_label(&core, ClipLoop::Once), "Once");
    }

    #[test]
    fn cycle_steps_full_table() {
        use ClipLoop::*;
        assert_eq!(cycle_steps(Loop, Loop), 0);
        assert_eq!(cycle_steps(Loop, PingPong), 1);
        assert_eq!(cycle_steps(Loop, Once), 2);
        assert_eq!(cycle_steps(PingPong, Loop), 2);
        assert_eq!(cycle_steps(PingPong, PingPong), 0);
        assert_eq!(cycle_steps(PingPong, Once), 1);
        assert_eq!(cycle_steps(Once, Loop), 1);
        assert_eq!(cycle_steps(Once, PingPong), 2);
        assert_eq!(cycle_steps(Once, Once), 0);
    }

    #[test]
    fn span_text_zero_one_many() {
        let core = EditorCore::new();
        assert_eq!(span_text(&core, &clip_with_frames(0)), "no frames");
        assert_eq!(span_text(&core, &clip_with_frames(1)), "frame 1 · 1 frame");
        assert_eq!(span_text(&core, &clip_with_frames(5)), "frames 1-5 · 5 frames");
    }

    #[test]
    fn usage_text_used_and_unused() {
        let mut core = EditorCore::new();
        // No figure plays clip 99 (or any clip) yet.
        assert_eq!(
            usage_text(&core, &core.project.figures, 99),
            "not used by any figure"
        );
        let fig_name = core.project.figures[core.selected_figure].name.clone();
        core.make_clip_from_figure();
        let used = usage_text(&core, &core.project.figures, 0);
        assert!(used.starts_with("used by "), "unexpected usage: {used}");
        assert!(used.contains(&fig_name), "usage misses figure: {used}");
        assert_eq!(
            usage_text(&core, &core.project.figures, 99),
            "not used by any figure"
        );
    }

    #[test]
    fn selection_clamps_to_the_clip_list() {
        let mut st = ClipsPanelState::default();
        assert_eq!(st.clamped(3), None);
        st.select(2);
        assert_eq!(st.clamped(3), Some(2));
        assert_eq!(st.clamped(2), None);
        st.select(0);
        assert_eq!(st.clamped(0), None);
    }

    #[test]
    fn apply_loop_mode_reaches_every_target() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        assert_eq!(core.project.clips[0].loop_mode, ClipLoop::Loop);
        apply_loop_mode(&mut core, 0, ClipLoop::Once);
        assert_eq!(core.project.clips[0].loop_mode, ClipLoop::Once);
        apply_loop_mode(&mut core, 0, ClipLoop::PingPong);
        assert_eq!(core.project.clips[0].loop_mode, ClipLoop::PingPong);
        apply_loop_mode(&mut core, 0, ClipLoop::Loop);
        assert_eq!(core.project.clips[0].loop_mode, ClipLoop::Loop);
        // Already there: no-op. Out of range: no panic, no change.
        apply_loop_mode(&mut core, 0, ClipLoop::Loop);
        apply_loop_mode(&mut core, 99, ClipLoop::Once);
        assert_eq!(core.project.clips[0].loop_mode, ClipLoop::Loop);
        assert_eq!(core.project.clips.len(), 1);
    }

    #[test]
    fn panel_thread_local_round_trips() {
        assert_eq!(panel_state().selected, None);
        update_panel_state(|p| p.select(1));
        assert_eq!(panel_state().selected, Some(1));
        update_panel_state(|p| *p = ClipsPanelState::default());
        assert_eq!(panel_state().selected, None);
    }

    #[test]
    fn clips_panel_builds_for_empty_and_populated_projects() {
        let t = rune::tok::theme(true);
        // Empty: demo project starts with no movieclips.
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        let _ = clips_panel(&state, &t);
        // Populated + selected: exercises the rows and the
        // segmented control (through the thread-local).
        state.core.make_clip_from_figure();
        let _ = clips_panel(&state, &t);
        update_panel_state(|p| p.select(0));
        let _ = clips_panel(&state, &t);
        update_panel_state(|p| *p = ClipsPanelState::default());
    }

    #[test]
    fn session_visibility_follows_enter_and_exit() {
        let mut core = EditorCore::new();
        // No clips, no session.
        assert!(!session_active(&core));
        assert_eq!(session_label(&core), None);
        core.make_clip_from_figure();
        // A clip exists, but visibility waits for the session.
        assert!(!session_active(&core));
        assert_eq!(session_label(&core), None);
        assert!(core.enter_clip(0));
        assert!(session_active(&core));
        let label = session_label(&core).expect("label while editing");
        let clip = &core.project.clips[0];
        assert!(label.contains(&clip.name), "label misses clip name: {label}");
        assert!(
            label.contains(&format!("frame 1 of {}", clip.frames.len())),
            "label misses frame position: {label}"
        );
        core.exit_clip();
        assert!(!session_active(&core));
        assert_eq!(session_label(&core), None);
    }

    #[test]
    fn enter_selected_clip_uses_the_panel_selection() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        // No selection: refused without touching the core.
        update_panel_state(|p| *p = ClipsPanelState::default());
        assert!(!enter_selected_clip(&mut core));
        assert!(core.clip_edit.is_none());
        // Stale selection (past the clip list): also refused.
        update_panel_state(|p| p.select(9));
        assert!(!enter_selected_clip(&mut core));
        assert!(core.clip_edit.is_none());
        // A real selection enters that clip at its first frame.
        update_panel_state(|p| p.select(0));
        assert!(enter_selected_clip(&mut core));
        assert_eq!(core.clip_edit.as_ref().map(|s| s.clip), Some(0));
        assert_eq!(core.clip_edit.as_ref().map(|s| s.frame), Some(0));
        core.exit_clip();
        update_panel_state(|p| *p = ClipsPanelState::default());
    }

    #[test]
    fn is_editing_tracks_the_session_clip() {
        let mut core = EditorCore::new();
        // No clips, no session: nothing is being edited.
        assert!(!is_editing(&core, 0));
        core.make_clip_from_figure();
        // A clip exists but no session is open yet.
        assert!(!is_editing(&core, 0));
        assert!(core.enter_clip(0));
        assert!(is_editing(&core, 0));
        assert!(!is_editing(&core, 1));
        core.exit_clip();
        assert!(!is_editing(&core, 0));
    }

    #[test]
    fn clip_row_action_line_fits_the_column_budget() {
        // Wave 9 width proof, pinned (see `clip_row`'s
        // docs): the action line is six 26 pt icon-only
        // buttons and five 8 pt gaps = 196 pt, inside the
        // 300 pt a right-column card leaves within a list
        // row's padding (316 content − 2×8 row px). The
        // pre-Wave-9 single-line row's FIXED children
        // alone (four labelled icon_btns at 2×10 padding
        // + 13 glyph + 6 gap + label, two 26 pt squares,
        // the loop label, eight 8 pt gaps) exceeded that
        // 300 pt under every text accounting.
        assert_eq!(6.0 * 26.0 + 5.0 * 8.0, 196.0);
        assert!(196.0 <= 316.0 - 16.0);
    }

    #[test]
    fn session_dispatch_steps_and_resizes_the_clip_only() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        let n0 = core.project.clips[0].frames.len();
        let project_frames = core.project.frames.len();
        assert!(n0 >= 2, "demo clip should have several frames, got {n0}");
        assert!(core.enter_clip(0));
        // Prev/Next dispatch moves the session frame, clamped
        // at both ends by the core.
        session_next(&mut core);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, 1);
        session_prev(&mut core);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, 0);
        session_prev(&mut core);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, 0);
        // Add frame (the button's core op): the clip grows and
        // the session lands on the new frame; the project
        // timeline is untouched.
        core.clip_add_frame();
        assert_eq!(core.project.clips[0].frames.len(), n0 + 1);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, n0);
        assert_eq!(core.project.frames.len(), project_frames);
        // Delete frame: the clip shrinks back.
        core.clip_delete_frame();
        assert_eq!(core.project.clips[0].frames.len(), n0);
        assert_eq!(core.project.frames.len(), project_frames);
        core.exit_clip();
        assert!(core.clip_edit.is_none());
    }

    #[test]
    fn clips_panel_builds_with_an_active_session() {
        let t = rune::tok::theme(true);
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        state.core.make_clip_from_figure();
        // Entry state (no session): the Edit clip entry row.
        let _ = clips_panel(&state, &t);
        // Active session: label + frame controls + Exit.
        assert!(state.core.enter_clip(0));
        let _ = clips_panel(&state, &t);
        state.core.exit_clip();
        let _ = clips_panel(&state, &t);
    }

    #[test]
    fn resolved_clip_name_trims_and_falls_back() {
        assert_eq!(resolved_clip_name("Walk cycle", "Old"), "Walk cycle");
        assert_eq!(resolved_clip_name("  Walk cycle  ", "Old"), "Walk cycle");
        // Blank (or whitespace-only) input keeps the current
        // name — a clip must never end up nameless.
        assert_eq!(resolved_clip_name("", "Old"), "Old");
        assert_eq!(resolved_clip_name("   ", "Old"), "Old");
    }

    #[test]
    fn duplicate_clip_appends_a_named_copy_in_one_undo_step() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        let original = core.project.clips[0].clone();
        let depth = core.undo_depth();
        let new_index = duplicate_clip(&mut core, 0).expect("copy index");
        assert_eq!(new_index, 1);
        assert_eq!(core.project.clips.len(), 2);
        let copy = &core.project.clips[1];
        assert_eq!(copy.name, format!("{} copy", original.name));
        assert_eq!(copy.frames.len(), original.frames.len());
        assert_eq!(copy.loop_mode, original.loop_mode);
        // The original is untouched (appending shifted no
        // stored index, so figures keep playing it).
        assert_eq!(core.project.clips[0].name, original.name);
        // Exactly one snapshot: one undo removes the copy.
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(core.project.clips.len(), 1);
        // Out of range: refusal via the status line, no copy.
        assert_eq!(duplicate_clip(&mut core, 9), None);
        assert_eq!(core.project.clips.len(), 1);
        assert_eq!(core.status, "No such clip.");
    }

    #[test]
    fn rename_clip_writes_once_and_no_ops_without_change() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        let old = core.project.clips[0].name.clone();
        let depth = core.undo_depth();
        // Blank field falls back to the current name: no-op,
        // no snapshot.
        assert!(!rename_clip(&mut core, 0, "   "));
        assert_eq!(core.project.clips[0].name, old);
        assert_eq!(core.undo_depth(), depth);
        // Same name after trimming: also a no-op.
        assert!(!rename_clip(&mut core, 0, &old));
        assert_eq!(core.undo_depth(), depth);
        // A real rename: trimmed, exactly one undo step, and
        // undo restores the old name.
        assert!(rename_clip(&mut core, 0, "  Walk cycle "));
        assert_eq!(core.project.clips[0].name, "Walk cycle");
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(core.project.clips[0].name, old);
        // Out of range: refusal via the status line.
        assert!(!rename_clip(&mut core, 9, "Nope"));
        assert_eq!(core.status, "No such clip.");
    }

    #[test]
    fn rename_round_trip_through_the_panel_state() {
        let t = rune::tok::theme(true);
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        state.core.make_clip_from_figure();
        let old = state.core.project.clips[0].name.clone();
        // Closed by default; the panel builds with the row shut.
        assert_eq!(state.clips_rename.target, None);
        let _ = clips_panel(&state, &t);
        // Rename loads the field with the clip's name + focus.
        begin_rename(&mut state, 0);
        assert_eq!(state.clips_rename.target, Some(0));
        assert_eq!(state.clips_rename.field.text, old);
        assert!(state.clips_rename.field.focused);
        let _ = clips_panel(&state, &t);
        // Apply writes the typed name and closes the editor.
        state.clips_rename.field.text = "Hero walk".to_string();
        apply_rename(&mut state);
        assert_eq!(state.core.project.clips[0].name, "Hero walk");
        assert_eq!(state.clips_rename.target, None);
        // An out-of-range Rename press never opens the field.
        begin_rename(&mut state, 9);
        assert_eq!(state.clips_rename.target, None);
        // A target gone stale closes without renaming.
        state.clips_rename.target = Some(9);
        apply_rename(&mut state);
        assert_eq!(state.clips_rename.target, None);
        assert_eq!(state.core.project.clips[0].name, "Hero walk");
        update_panel_state(|p| *p = ClipsPanelState::default());
    }

    fn core_with_three_clips() -> EditorCore {
        let mut core = EditorCore::new();
        for name in ["Alpha", "Bravo", "Charlie"] {
            let mut clip = clip_with_frames(2);
            clip.name = name.to_string();
            core.project.clips.push(clip);
        }
        core
    }

    fn clip_names(core: &EditorCore) -> Vec<&str> {
        core.project.clips.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn selection_after_delete_repairs_each_position() {
        // No selection stays no selection.
        assert_eq!(selection_after_delete(None, 0, 3), None);
        // Deleting the only clip clears the selection.
        assert_eq!(selection_after_delete(Some(0), 0, 0), None);
        // A selection before the gap is untouched.
        assert_eq!(selection_after_delete(Some(0), 2, 4), Some(0));
        // A selection after the gap shifts down by one.
        assert_eq!(selection_after_delete(Some(3), 1, 4), Some(2));
        // The deleted clip's own selection lands on the clip
        // that slid into its slot…
        assert_eq!(selection_after_delete(Some(1), 1, 4), Some(1));
        // …or on the new last clip when it was the last one.
        assert_eq!(selection_after_delete(Some(2), 2, 2), Some(1));
        assert_eq!(selection_after_delete(Some(0), 0, 2), Some(0));
    }

    #[test]
    fn target_tracking_tables_for_delete_and_swap() {
        // Delete: before untouched, at -> None, after -> -1.
        assert_eq!(target_after_delete(None, 1), None);
        assert_eq!(target_after_delete(Some(0), 1), Some(0));
        assert_eq!(target_after_delete(Some(1), 1), None);
        assert_eq!(target_after_delete(Some(2), 1), Some(1));
        // Swap: the pair trades, everything else untouched.
        assert_eq!(target_after_swap(None, 0, 1), None);
        assert_eq!(target_after_swap(Some(0), 0, 1), Some(1));
        assert_eq!(target_after_swap(Some(1), 0, 1), Some(0));
        assert_eq!(target_after_swap(Some(2), 0, 1), Some(2));
    }

    #[test]
    fn delete_clip_remaps_figures_in_one_undo_step() {
        // A figure past the gap keeps its clip, shifted.
        let mut core = core_with_three_clips();
        core.project.figures[0].clip = Some(2); // Charlie
        let depth = core.undo_depth();
        assert!(delete_clip(&mut core, 0));
        assert_eq!(clip_names(&core), ["Bravo", "Charlie"]);
        assert_eq!(core.project.figures[0].clip, Some(1));
        assert_eq!(core.project.clips[1].name, "Charlie");
        assert_eq!(core.status, "Clip 1 (Alpha) deleted.");
        // Exactly one snapshot: one undo restores all three.
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(clip_names(&core), ["Alpha", "Bravo", "Charlie"]);

        // A figure whose clip is deleted falls back to its
        // timeline poses (no clip reference left).
        let mut core = core_with_three_clips();
        core.project.figures[0].clip = Some(1); // Bravo
        assert!(delete_clip(&mut core, 1));
        assert_eq!(clip_names(&core), ["Alpha", "Charlie"]);
        assert_eq!(core.project.figures[0].clip, None);

        // Out of range: status-only refusal, no snapshot.
        let mut core = core_with_three_clips();
        let depth = core.undo_depth();
        assert!(!delete_clip(&mut core, 9));
        assert_eq!(core.status, "No such clip.");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.clips.len(), 3);
    }

    #[test]
    fn move_clip_swaps_neighbours_and_figures_follow() {
        let mut core = core_with_three_clips();
        core.project.figures[0].clip = Some(0); // Alpha
        let depth = core.undo_depth();
        assert!(move_clip(&mut core, 0, false));
        assert_eq!(clip_names(&core), ["Bravo", "Alpha", "Charlie"]);
        // The figure still plays Alpha, now at index 1.
        assert_eq!(core.project.figures[0].clip, Some(1));
        assert_eq!(core.status, "Clip 2 (Alpha) moved down.");
        assert_eq!(core.undo_depth(), depth + 1);
        // Moving back up restores the order and the reference.
        assert!(move_clip(&mut core, 1, true));
        assert_eq!(clip_names(&core), ["Alpha", "Bravo", "Charlie"]);
        assert_eq!(core.project.figures[0].clip, Some(0));
        // A middle swap works from the other side too.
        assert!(move_clip(&mut core, 2, true));
        assert_eq!(clip_names(&core), ["Alpha", "Charlie", "Bravo"]);
    }

    #[test]
    fn move_clip_ends_refuse_without_a_snapshot() {
        let mut core = core_with_three_clips();
        let depth = core.undo_depth();
        assert!(!move_clip(&mut core, 0, true));
        assert_eq!(core.status, "Clip 1 is already first.");
        assert!(!move_clip(&mut core, 2, false));
        assert_eq!(core.status, "Clip 3 is already last.");
        assert!(!move_clip(&mut core, 9, true));
        assert_eq!(core.status, "No such clip.");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(clip_names(&core), ["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn rename_target_and_selection_track_swap_and_delete() {
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        for name in ["Alpha", "Bravo", "Charlie"] {
            let mut clip = clip_with_frames(2);
            clip.name = name.to_string();
            state.core.project.clips.push(clip);
        }
        // Renaming Alpha (index 0), with Alpha selected.
        state.clips_rename.target = Some(0);
        update_panel_state(|p| p.select(0));
        // Move Alpha down: the rename target and the
        // selection follow the clip to its new slot.
        move_clip_at(&mut state, 0, false);
        assert_eq!(state.clips_rename.target, Some(1));
        assert_eq!(panel_state().selected, Some(1));
        // Delete Alpha (now at 1): the rename session ends
        // (its clip is gone); the selection lands on the
        // clip that slid into the slot.
        delete_clip_at(&mut state, 1);
        assert_eq!(state.clips_rename.target, None);
        assert_eq!(panel_state().selected, Some(1));
        assert_eq!(clip_names(&state.core), ["Bravo", "Charlie"]);
        update_panel_state(|p| *p = ClipsPanelState::default());
    }

    fn core_with_two_clips() -> EditorCore {
        let mut core = EditorCore::new();
        for name in ["Alpha", "Bravo"] {
            let mut clip = clip_with_frames(2);
            clip.name = name.to_string();
            core.project.clips.push(clip);
        }
        core
    }

    #[test]
    fn insert_clip_figure_at_inserts_the_chosen_clip() {
        let mut core = core_with_two_clips();
        let figures_before = core.project.figures.len();
        let depth = core.undo_depth();
        assert!(insert_clip_figure_at(&mut core, 1));
        // Exactly one snapshot, exactly one new figure.
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures.len(), figures_before + 1);
        let idx = figures_before;
        let fig = &core.project.figures[idx];
        // The chosen clip — index 1, not the engine op's
        // hard-wired first clip.
        assert_eq!(fig.clip, Some(1));
        // The engine op's construction, field by field:
        // the clip's name and def, the palette colour at
        // (idx + 3) % 6, unlocked, visible, full opacity.
        assert_eq!(fig.name, "Bravo");
        assert_eq!(fig.def.nodes.len(), core.project.clips[1].def.nodes.len());
        assert_eq!(fig.def.nodes[0].name, core.project.clips[1].def.nodes[0].name);
        assert_eq!(fig.color, 0xFFC65D, "FIGURE_COLORS[(1 + 3) % 6]");
        assert!(!fig.locked);
        assert!(!fig.hidden);
        assert_eq!(fig.opacity, 1.0);
        // The def's rest pose went into every frame at the
        // op's root, [380 + 40 * idx, 450].
        for frame in &core.project.frames {
            assert_eq!(frame.poses[idx].root, [420.0, 450.0]);
        }
        // The new figure is selected, under the engine
        // op's own status line.
        assert_eq!(core.selected_figure, idx);
        assert_eq!(core.status, "Inserted movieclip Bravo.");
        // The clips vec is untouched — no stored index
        // shifted, so no selection or rename repair is
        // owed (the rename-session guarantee).
        assert_eq!(clip_names(&core), ["Alpha", "Bravo"]);
        // One undo removes the inserted figure.
        core.undo();
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn insert_clip_figure_at_zero_matches_the_engine_op() {
        // Parity at index 0: the same project through the
        // engine op and through this module's op must end
        // with the same figure, selection and status.
        let mut engine = core_with_two_clips();
        engine.insert_clip_figure();
        let mut panel = core_with_two_clips();
        assert!(insert_clip_figure_at(&mut panel, 0));
        assert_eq!(panel.project.figures.len(), engine.project.figures.len());
        let a = &engine.project.figures[engine.selected_figure];
        let b = &panel.project.figures[panel.selected_figure];
        assert_eq!(b.clip, Some(0));
        assert_eq!(a.clip, b.clip);
        assert_eq!(a.name, b.name);
        assert_eq!(a.color, b.color);
        assert_eq!(a.def.nodes.len(), b.def.nodes.len());
        assert_eq!(
            panel.project.frames[0].poses[panel.selected_figure].root,
            engine.project.frames[0].poses[engine.selected_figure].root
        );
        assert_eq!(panel.selected_figure, engine.selected_figure);
        assert_eq!(panel.status, engine.status);
    }

    #[test]
    fn insert_clip_figure_at_bad_index_refuses_without_a_snapshot() {
        let mut core = core_with_two_clips();
        let figures_before = core.project.figures.len();
        let depth = core.undo_depth();
        assert!(!insert_clip_figure_at(&mut core, 2));
        assert!(!insert_clip_figure_at(&mut core, 99));
        assert_eq!(core.status, "No such clip.");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.figures.len(), figures_before);
        // No clips at all: even index 0 refuses the same
        // way, with no snapshot and no figure.
        let mut empty = EditorCore::new();
        assert!(empty.project.clips.is_empty());
        let depth = empty.undo_depth();
        let figures_before = empty.project.figures.len();
        assert!(!insert_clip_figure_at(&mut empty, 0));
        assert_eq!(empty.status, "No such clip.");
        assert_eq!(empty.undo_depth(), depth);
        assert_eq!(empty.project.figures.len(), figures_before);
    }

    #[test]
    fn delete_and_move_remap_the_core_session() {
        // Wave 10: the session remap inside `delete_clip`
        // / `move_clip` was previously untested at this
        // level — the panel's promise is that a session
        // follows its clip through list surgery and ends
        // when its clip is deleted, never stranding over
        // a dead index.
        let mut core = core_with_three_clips();
        assert!(core.enter_clip(1)); // Bravo
        // Deleting a clip BEFORE the session's shifts the
        // session down with its clip.
        assert!(delete_clip(&mut core, 0)); // Alpha
        assert_eq!(core.clip_edit.as_ref().map(|s| s.clip), Some(0));
        assert_eq!(core.project.clips[0].name, "Bravo");
        // A swap moves the session with its clip too.
        assert!(move_clip(&mut core, 0, false)); // Bravo <-> Charlie
        assert_eq!(core.clip_edit.as_ref().map(|s| s.clip), Some(1));
        assert_eq!(core.project.clips[1].name, "Bravo");
        // Deleting a clip AFTER the session's leaves it be.
        let mut core = core_with_three_clips();
        assert!(core.enter_clip(0)); // Alpha
        assert!(delete_clip(&mut core, 2)); // Charlie
        assert_eq!(core.clip_edit.as_ref().map(|s| s.clip), Some(0));
        // Deleting the edited clip itself ends the session.
        assert!(delete_clip(&mut core, 0)); // Alpha
        assert_eq!(core.clip_edit, None);
    }

    #[test]
    fn session_label_is_honest_for_a_dangling_session() {
        // Wave 10: undo restores `Project` only, so a
        // session can outlive its clip (enter a session,
        // then undo the clip's creation). The label must
        // not dress that up as a frame position.
        let mut core = EditorCore::new();
        core.clip_edit = Some(ClipEditSession { clip: 4, frame: 0, name: String::new() });
        assert!(session_active(&core), "Exit stays reachable");
        assert_eq!(
            session_label(&core).as_deref(),
            Some("Editing clip 5 · clip no longer exists")
        );
        // The next session op closes the dangling session
        // (the core's self-heal), as the label docs say.
        core.clip_goto(0);
        assert_eq!(core.clip_edit, None);
        assert_eq!(core.status, "Movieclip is gone — editing session closed.");

        // A clip that exists but has no frames (built on
        // an empty timeline) labels just as plainly.
        let mut core = EditorCore::new();
        let mut empty = clip_with_frames(0);
        empty.name = "Empty".to_string();
        core.project.clips.push(empty);
        core.clip_edit = Some(ClipEditSession { clip: 0, frame: 0, name: "Empty".into() });
        assert_eq!(
            session_label(&core).as_deref(),
            Some("Editing Empty · no frames")
        );
    }

    #[test]
    fn apply_rename_refuses_when_the_target_clip_changed_under_the_field() {
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        for name in ["Alpha", "Bravo"] {
            let mut clip = clip_with_frames(2);
            clip.name = name.to_string();
            state.core.project.clips.push(clip);
        }
        // Panel-mediated reorder: the target follows its
        // clip and the captured name still matches, so the
        // rename lands on the clip the user opened.
        begin_rename(&mut state, 0); // Alpha
        assert_eq!(state.clips_rename.opened_name.as_deref(), Some("Alpha"));
        move_clip_at(&mut state, 0, false); // Alpha -> index 1
        assert_eq!(state.clips_rename.target, Some(1));
        state.clips_rename.field.text = "Alpha prime".to_string();
        apply_rename(&mut state);
        assert_eq!(clip_names(&state.core), ["Bravo", "Alpha prime"]);
        assert_eq!(state.clips_rename.target, None);
        assert_eq!(state.clips_rename.opened_name, None);

        // Unmediated delete (the undo / live-MCP shape):
        // Bravo is deleted through the core-level fn,
        // bypassing `delete_clip_at`'s repair, so the open
        // rename still points at index 0 — where a
        // different clip now sits. Apply must refuse
        // rather than rename that clip.
        begin_rename(&mut state, 0); // Bravo
        assert_eq!(state.clips_rename.opened_name.as_deref(), Some("Bravo"));
        assert!(delete_clip(&mut state.core, 0));
        assert_eq!(clip_names(&state.core), ["Alpha prime"]);
        assert_eq!(state.clips_rename.target, Some(0));
        state.clips_rename.field.text = "Hijacked".to_string();
        apply_rename(&mut state);
        assert_eq!(clip_names(&state.core), ["Alpha prime"]);
        assert_eq!(state.clips_rename.target, None);
        assert_eq!(
            state.core.status,
            "Clip changed while renaming — rename cancelled."
        );

        // Unmediated rename of the target clip itself:
        // same refusal — the name under the field is no
        // longer the name the rename was opened on.
        begin_rename(&mut state, 0); // Alpha prime
        assert!(rename_clip(&mut state.core, 0, "Outside"));
        state.clips_rename.field.text = "Hijacked".to_string();
        apply_rename(&mut state);
        assert_eq!(clip_names(&state.core), ["Outside"]);
        assert_eq!(state.clips_rename.target, None);
        assert_eq!(
            state.core.status,
            "Clip changed while renaming — rename cancelled."
        );
        update_panel_state(|p| *p = ClipsPanelState::default());
    }

    #[test]
    fn session_clip_intact_applies_the_core_identity_rule() {
        // Wave 11: the predicate is the core repair's own
        // test (editor.rs:296) — clip exists at the
        // session's index under the captured name.
        let mut core = core_with_two_clips();
        // No session: not intact (the predicate stays
        // total; callers only ask while one is active).
        assert!(!session_clip_intact(&core));
        assert!(core.enter_clip(0)); // Alpha
        assert!(session_clip_intact(&core));
        // The wholesale-swap shape: same index, different
        // clip — written straight onto the project, the
        // way `open_project_file` and MCP's `new_project`
        // replace it without touching `clip_edit`.
        core.project.clips[0].name = "Stranger".to_string();
        assert!(!session_clip_intact(&core));
        // A vanished clip is not intact either.
        core.project.clips.clear();
        assert!(!session_clip_intact(&core));
    }

    #[test]
    fn session_label_and_marker_flag_a_retargeted_session() {
        let mut core = core_with_two_clips();
        assert!(core.enter_clip(0)); // Alpha
        assert!(is_editing(&core, 0));
        // Retarget: a different clip lands at the
        // session's index through an unmediated swap.
        core.project.clips[0].name = "Stranger".to_string();
        // The label names the clip the user entered, not
        // the stranger now at the index, and says what
        // happened.
        assert_eq!(
            session_label(&core).as_deref(),
            Some("Editing Alpha · clip changed — exit and re-enter")
        );
        // The row marker stays off the stranger's row.
        assert!(!is_editing(&core, 0));
        // Exit still works from the stale state (the one
        // control the stale section offers).
        core.exit_clip();
        assert!(!session_active(&core));
    }

    #[test]
    fn panel_rename_keeps_the_session_identity_in_step() {
        // Wave 10's sync, pinned from the panel side: a
        // rename through `rename_clip` rewrites the
        // session's captured name with the clip's, so the
        // Wave 11 identity gate does not mistake a
        // legitimate rename for a retarget.
        let mut core = core_with_two_clips();
        assert!(core.enter_clip(0)); // Alpha
        assert!(rename_clip(&mut core, 0, "Alpha prime"));
        assert_eq!(
            core.clip_edit.as_ref().map(|s| s.name.as_str()),
            Some("Alpha prime")
        );
        assert!(session_clip_intact(&core));
        assert!(is_editing(&core, 0));
        let label = session_label(&core).expect("label after rename");
        assert!(label.contains("Alpha prime"), "label: {label}");
    }

    #[test]
    fn clips_panel_builds_with_a_stale_session() {
        // The stale branch of `clip_edit_section` (Wave
        // 11): label + Exit only. Building the panel is
        // the assertion — the branch must render without
        // panicking, before and after the exit.
        let t = rune::tok::theme(true);
        let mut state = State {
            core: EditorCore::new(),
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
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
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
            clips_rename: ClipRenameState::new(),
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        for name in ["Alpha", "Bravo"] {
            let mut clip = clip_with_frames(2);
            clip.name = name.to_string();
            state.core.project.clips.push(clip);
        }
        assert!(state.core.enter_clip(0));
        let _ = clips_panel(&state, &t);
        // Retarget the session by direct project write
        // (the swap shape), then build again: the stale
        // branch renders the honest label + Exit.
        state.core.project.clips[0].name = "Stranger".to_string();
        assert!(!session_clip_intact(&state.core));
        let _ = clips_panel(&state, &t);
        state.core.exit_clip();
        let _ = clips_panel(&state, &t);
    }

    #[test]
    fn state_rows_come_in_timeline_order_and_skip_dangling_clips() {
        let mut core = core_with_three_clips();
        core.project.figures[0].states = vec![
            StateKey { frame: 9, clip: 2, offset: 0, speed: 1.0 },
            StateKey { frame: 3, clip: 0, offset: 0, speed: 1.0 },
            // Clip 99 does not exist: skipped, never a panic.
            StateKey { frame: 5, clip: 99, offset: 0, speed: 1.0 },
        ];
        let rows = state_rows(&core, 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0.frame, 3);
        assert_eq!(rows[0].1, "Alpha");
        assert_eq!(rows[1].0.frame, 9);
        assert_eq!(rows[1].1, "Charlie");
        assert!(state_rows(&core, 99).is_empty(), "no figure, no rows");
    }

    #[test]
    fn state_clip_at_prefers_the_key_in_force_then_the_base_link() {
        let mut core = core_with_three_clips();
        core.project.figures[0].clip = Some(0);
        core.project.figures[0].states = vec![StateKey {
            frame: 4,
            clip: 1,
            offset: 0,
            speed: 1.0,
        }];
        // Before the first key the base link governs.
        assert_eq!(state_clip_at(&core, 0, 0), Some(0));
        assert_eq!(state_clip_at(&core, 0, 3), Some(0));
        // At and after the key's frame the key governs.
        assert_eq!(state_clip_at(&core, 0, 4), Some(1));
        assert_eq!(state_clip_at(&core, 0, 40), Some(1));
        // No link and no keys: timeline poses.
        assert_eq!(state_clip_at(&core, 1, 0), None);
    }

    #[test]
    fn key_state_at_inserts_replaces_and_noops_under_one_snapshot() {
        let mut core = core_with_three_clips();
        let depth = core.undo_depth();
        key_state_at(&mut core, 0, 6, 1);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the write");
        assert_eq!(core.project.figures[0].states.len(), 1);
        assert_eq!(core.project.figures[0].states[0].clip, 1);
        // Identical key again: a true no-op, no snapshot.
        key_state_at(&mut core, 0, 6, 1);
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures[0].states.len(), 1);
        // A different clip at the same frame replaces wholesale,
        // resetting an MCP-authored offset/speed to the defaults.
        core.project.figures[0].states[0].offset = 3;
        core.project.figures[0].states[0].speed = 2.0;
        key_state_at(&mut core, 0, 6, 2);
        assert_eq!(core.undo_depth(), depth + 2);
        assert_eq!(core.project.figures[0].states.len(), 1);
        let key = core.project.figures[0].states[0];
        assert_eq!((key.clip, key.offset, key.speed), (2, 0, 1.0));
        // Keys stay in timeline order as they accumulate.
        key_state_at(&mut core, 0, 2, 0);
        let frames: Vec<usize> = core.project.figures[0]
            .states
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, [2, 6]);
        // Refusals write nothing (the core op's voice).
        let depth = core.undo_depth();
        key_state_at(&mut core, 99, 0, 0);
        assert_eq!(core.status, "No figure 100 to set a state key on.");
        key_state_at(&mut core, 0, 0, 99);
        assert_eq!(core.status, "No such clip.");
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn clear_state_at_removes_the_key_or_refuses_without_a_snapshot() {
        let mut core = core_with_three_clips();
        key_state_at(&mut core, 0, 6, 1);
        let depth = core.undo_depth();
        assert!(clear_state_at(&mut core, 0, 6));
        assert_eq!(core.undo_depth(), depth + 1);
        assert!(core.project.figures[0].states.is_empty());
        let depth = core.undo_depth();
        assert!(!clear_state_at(&mut core, 0, 6));
        assert_eq!(core.status, "Figure 1 has no state key at frame 7.");
        assert!(!clear_state_at(&mut core, 99, 6));
        assert_eq!(core.status, "No figure 100 to remove a state key from.");
        assert_eq!(core.undo_depth(), depth, "refusals never snapshot");
    }

    #[test]
    fn state_key_text_uses_the_plain_template_until_offset_or_speed_depart() {
        let core = core_with_three_clips();
        let plain = StateKey { frame: 2, clip: 0, offset: 0, speed: 1.0 };
        // Identity is asserted against the same tr() composition
        // (the W1 pin discipline): the pin holds before and after
        // IA-X3-1 lands the keys in the catalogue.
        let expect = i18n_wire::tr(&core, "clips_states_key")
            .replace("{frame}", "3")
            .replace("{clip}", "Alpha");
        assert_eq!(state_key_text(&core, &plain, "Alpha"), expect);
        let fx = StateKey { frame: 2, clip: 0, offset: 4, speed: 1.5 };
        let text = state_key_text(&core, &fx, "Alpha");
        assert!(text.contains("Alpha"), "fx label names the clip: {text}");
        assert_ne!(text, expect, "offset/speed keys take the fx template");
    }

    #[test]
    fn usage_text_counts_state_key_usage() {
        let mut core = core_with_three_clips();
        assert_eq!(
            usage_text(&core, &core.project.figures, 1),
            "not used by any figure"
        );
        key_state_at(&mut core, 0, 2, 1);
        let used = usage_text(&core, &core.project.figures, 1);
        assert!(used.starts_with("used by "), "state usage counts: {used}");
    }

    #[test]
    fn clips_i18n_keys_exist_and_match_the_english_labels() {
        // Wave U (U2): every label and composed readout
        // the panel wires through `i18n_wire::tr` must
        // resolve to exactly the English string the panel
        // rendered before the wiring; template keys are
        // asserted in stored form (slots intact). The
        // keys land via IA-U2-1 (the integrator appends
        // them to all 28 tables after the last Wave T
        // block), so this pin passes only once that IA
        // lands — until then `tr` falls back to the key
        // itself and `key_in_table` is false (the Q7
        // pattern: wiring and keys land together at
        // integration). The composed readouts' substituted
        // forms stay pinned by the pre-existing tests
        // (`span_text_zero_one_many`,
        // `usage_text_used_and_unused`, the session-label
        // tests), which become IA-dependent the same way.
        let core = EditorCore::new();
        for (key, english) in [
            ("clips_title", "Movieclips"),
            ("clips_count", "{count} clip(s)"),
            ("clips_make", "Make clip"),
            ("clips_insert_clip", "Insert clip"),
            (
                "clips_empty",
                "No movieclips in this project. \"Make clip\" builds one from the selected figure.",
            ),
            ("clips_editing", "Editing"),
            ("clips_loop", "Loop"),
            ("clips_loop_pingpong", "Ping-pong"),
            ("clips_loop_once", "Once"),
            ("clips_duplicate", "Duplicate"),
            ("clips_rename", "Rename"),
            ("clips_insert", "Insert"),
            ("clips_move_up", "Move up"),
            ("clips_move_down", "Move down"),
            ("clips_delete", "Delete"),
            ("clips_row_detail", "{span} · {usage}"),
            ("clips_span_none", "no frames"),
            ("clips_span_one", "frame 1 · 1 frame"),
            ("clips_span_many", "frames 1-{end} · {count} frames"),
            ("clips_usage_none", "not used by any figure"),
            ("clips_usage_some", "used by {names}"),
            ("clips_rename_placeholder", "Clip name"),
            ("clips_apply", "Apply"),
            ("clips_cancel", "Cancel"),
            ("clips_loop_mode", "Loop mode:"),
            ("clips_loop_hint", "Select a clip to set its loop mode."),
            (
                "clips_session_gone",
                "Editing clip {index} · clip no longer exists",
            ),
            (
                "clips_session_stale",
                "Editing {name} · clip changed — exit and re-enter",
            ),
            ("clips_session_no_frames", "Editing {name} · no frames"),
            (
                "clips_session_frame",
                "Editing {name} · frame {frame} of {total}",
            ),
            // Wave X (IA-X3-1): the state section's keys —
            // this pin passes once the integrator inserts
            // them into the catalogue (the Q7 pattern).
            ("clips_states_figure", "States for {name}"),
            ("clips_states_base", "Base: {clip}"),
            (
                "clips_states_no_base",
                "Base: timeline poses (no clip linked)",
            ),
            ("clips_states_key", "Frame {frame}: {clip}"),
            (
                "clips_states_key_fx",
                "Frame {frame}: {clip} (offset {offset}, speed {speed})",
            ),
            (
                "clips_states_none",
                "No states keyed — the figure plays its base clip or timeline poses throughout.",
            ),
            ("clips_states_key_here", "Key at playhead (frame {frame}):"),
            ("clips_states_clear", "Clear state key"),
            ("clips_prev_frame", "Prev frame"),
            ("clips_next_frame", "Next frame"),
            ("clips_add_frame", "Add frame"),
            ("clips_delete_frame", "Delete frame"),
            ("clips_exit", "Exit"),
            ("clips_edit_clip", "Edit clip"),
            ("clips_edit_hint", "Select a clip to edit its frames."),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }
}
