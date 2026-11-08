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
//! * The loop-mode segmented control -> `apply_loop_mode`,
//!   which reaches the target mode through the one loop op
//!   the core exposes, `EditorCore::cycle_clip_loop`
//!   (editor.rs:3473).
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

use std::cell::RefCell;

use rune::tok::{type_, Theme};
use rune::{col, row, text, text_input, El, TextEdit, Tx};
use stykanimate_rs::editor::{ClipEditSession, EditorCore};
use stykanimate_rs::model::{ClipDef, ClipLoop, FigureInstance};

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
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
    /// The in-progress name, loaded from the target clip
    /// by `begin_rename`.
    pub field: TextEdit,
}

impl ClipRenameState {
    pub fn new() -> Self {
        ClipRenameState {
            target: None,
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

/// Display label for a clip loop mode (the GPUI panel's
/// wording: "Loop" / "Ping-pong" / "Once").
pub fn loop_label(mode: ClipLoop) -> &'static str {
    match mode {
        ClipLoop::Loop => "Loop",
        ClipLoop::PingPong => "Ping-pong",
        ClipLoop::Once => "Once",
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
    let Some(clip) = core.project.clips.get(clip_idx) else {
        core.status = "No such clip.".into();
        return None;
    };
    let mut copy = clip.clone();
    copy.name = format!("{} copy", copy.name);
    core.snapshot();
    core.project.clips.push(copy);
    let new_index = core.project.clips.len() - 1;
    core.status = format!(
        "Duplicated clip {} as clip {}.",
        clip_idx + 1,
        new_index + 1
    );
    Some(new_index)
}

/// Rename clip `clip_idx` to the resolved field text (the
/// rename row's "Apply" dispatch). Field-write discipline
/// as `duplicate_clip`; a rename that resolves to the
/// current name is a no-op — no snapshot, no status churn
/// (the `apply_loop_mode` precedent of not calling the op
/// when already at the target). Returns true when the name
/// actually changed (exactly one undo step).
pub fn rename_clip(core: &mut EditorCore, clip_idx: usize, field_text: &str) -> bool {
    let Some(current) = core.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
        core.status = "No such clip.".into();
        return false;
    };
    let name = resolved_clip_name(field_text, &current);
    if name == current {
        return false;
    }
    core.snapshot();
    core.project.clips[clip_idx].name = name.clone();
    core.status = format!("Clip {} renamed to {name}.", clip_idx + 1);
    true
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
    let Some(name) = core.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
        core.status = "No such clip.".into();
        return false;
    };
    core.snapshot();
    core.project.clips.remove(clip_idx);
    for fig in &mut core.project.figures {
        fig.clip = target_after_delete(fig.clip, clip_idx);
    }
    if let Some(session) = core.clip_edit {
        core.clip_edit = target_after_delete(Some(session.clip), clip_idx)
            .map(|clip| ClipEditSession { clip, ..session });
    }
    core.status = format!("Clip {} ({name}) deleted.", clip_idx + 1);
    true
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
    if clip_idx >= core.project.clips.len() {
        core.status = "No such clip.".into();
        return false;
    }
    let other = if up {
        if clip_idx == 0 {
            core.status = format!("Clip {} is already first.", clip_idx + 1);
            return false;
        }
        clip_idx - 1
    } else if clip_idx + 1 >= core.project.clips.len() {
        core.status = format!("Clip {} is already last.", clip_idx + 1);
        return false;
    } else {
        clip_idx + 1
    };
    core.snapshot();
    core.project.clips.swap(clip_idx, other);
    for fig in &mut core.project.figures {
        fig.clip = target_after_swap(fig.clip, clip_idx, other);
    }
    if let Some(session) = core.clip_edit {
        if let Some(clip) = target_after_swap(Some(session.clip), clip_idx, other) {
            core.clip_edit = Some(ClipEditSession { clip, ..session });
        }
    }
    let name = core.project.clips[other].name.clone();
    core.status = format!(
        "Clip {} ({name}) moved {}.",
        other + 1,
        if up { "up" } else { "down" }
    );
    true
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
/// name at once. An out-of-range index only sets the
/// core's status line and leaves the field closed.
pub fn begin_rename(s: &mut State, clip_idx: usize) {
    let Some(name) = s.core.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
        s.core.status = "No such clip.".into();
        return;
    };
    let mut field = TextEdit::new(name);
    field.focused = true;
    s.clips_rename.field = field;
    s.clips_rename.target = Some(clip_idx);
}

/// Write the rename field back to its target clip (the
/// "Apply" button's and the field's Enter dispatch). The
/// editor closes either way: `rename_clip` refuses a stale
/// target with a status line only, and a successful or
/// no-op rename is done.
pub fn apply_rename(s: &mut State) {
    let Some(target) = s.clips_rename.target else {
        return;
    };
    let text = s.clips_rename.field.text.clone();
    rename_clip(&mut s.core, target, &text);
    s.clips_rename.target = None;
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

/// The session section's label while a session is active:
/// which clip is being edited and where the session sits in
/// it ("Editing <name> · frame <f> of <n>"). `None` when no
/// session is active. A session whose clip is missing from
/// the project (reachable only via undo) still labels,
/// falling back to the clip's index and a zero count — the
/// next session op closes such a session.
pub fn session_label(core: &EditorCore) -> Option<String> {
    let session = core.clip_edit?;
    let (name, frames) = match core.project.clips.get(session.clip) {
        Some(clip) => (clip.name.clone(), clip.frames.len()),
        None => (format!("clip {}", session.clip + 1), 0),
    };
    let frame = session.frame.min(frames.saturating_sub(1));
    Some(format!(
        "Editing {name} · frame {} of {frames}",
        frame + 1
    ))
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
    if let Some(session) = core.clip_edit {
        core.clip_goto(session.frame.saturating_sub(1));
    }
}

/// Step the session one clip frame forward (the "Next frame"
/// button's dispatch): `clip_goto` to the next frame, clamped
/// by the core at the clip's last frame. No-op without a
/// session.
pub fn session_next(core: &mut EditorCore) {
    if let Some(session) = core.clip_edit {
        core.clip_goto(session.frame + 1);
    }
}

/// The clip's frame span as shown in its list row: a clip
/// always spans its own frames 1..=len.
pub fn span_text(clip: &ClipDef) -> String {
    match clip.frames.len() {
        0 => "no frames".to_string(),
        1 => "frame 1 · 1 frame".to_string(),
        n => format!("frames 1-{n} · {n} frames"),
    }
}

/// Which figures play clip `clip_idx` (the GPUI row's usage
/// text).
pub fn usage_text(figures: &[FigureInstance], clip_idx: usize) -> String {
    let names: Vec<&str> = figures
        .iter()
        .filter(|f| f.clip == Some(clip_idx))
        .map(|f| f.name.as_str())
        .collect();
    if names.is_empty() {
        "not used by any figure".to_string()
    } else {
        format!("used by {}", names.join(", "))
    }
}

/// One clip row: name, span + usage, current loop mode.
/// Pressing the row selects the clip (panel-local state) for
/// the loop-mode segmented control below the list.
fn clip_row(s: &State, t: &Theme, index: usize, selected: bool) -> El {
    let core = &s.core;
    let clip = &core.project.clips[index];
    row()
        .key(format!("clip-row-{}", index + 1))
        .w_full()
        .gap(8.0)
        .center_y()
        .px(8.0)
        .py(4.0)
        .radius(6.0)
        .fill(if selected { t.control_track } else { t.card })
        .hover(t.control_track)
        .on_press(move |_s: &mut State| {
            update_panel_state(|p| p.select(index));
        })
        .child(
            text(clip.name.clone())
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0),
        )
        .child(
            text(format!(
                "{} · {}",
                span_text(clip),
                usage_text(&core.project.figures, index)
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        .child(
            text(loop_label(clip.loop_mode))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(icon_btn(
            &format!("clip-dup-{index}"),
            "copy",
            "Dup",
            false,
            t,
            move |s: &mut State| {
                if let Some(new_index) = duplicate_clip(&mut s.core, index) {
                    update_panel_state(|p| p.select(new_index));
                }
            },
        ))
        .child(icon_btn(
            &format!("clip-rename-{index}"),
            "pencil",
            "Rename",
            false,
            t,
            move |s: &mut State| {
                update_panel_state(|p| p.select(index));
                begin_rename(s, index);
            },
        ))
        .child(icon_only_btn(
            &format!("clip-up-{index}"),
            "chevron-up",
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
            26.0,
            false,
            t,
            move |s: &mut State| {
                move_clip_at(s, index, false);
            },
        ))
        .child(icon_btn(
            &format!("clip-del-{index}"),
            "trash-can",
            "Del",
            false,
            t,
            move |s: &mut State| {
                delete_clip_at(s, index);
            },
        ))
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
            text_input(&s.clips_rename.field, type_::BODY)
                .key("clip-rename-field")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder("Clip name", t.text_secondary)
                .on_edit(|s: &mut State| &mut s.clips_rename.field)
                .on_submit(|s: &mut State, _tx: &mut Tx| apply_rename(s)),
        )
        .child(icon_btn(
            "clip-rename-apply",
            "check",
            "Apply",
            false,
            t,
            |s: &mut State| apply_rename(s),
        ))
        .child(icon_btn(
            "clip-rename-cancel",
            "xmark",
            "Cancel",
            false,
            t,
            |s: &mut State| {
                s.clips_rename.target = None;
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
            text("Select a clip to set its loop mode.")
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
        .child(
            text("Loop mode:")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(btn(
            "clip-loop-loop",
            "Loop",
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
            "Ping-pong",
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
            "Once",
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
/// * No session, clips exist: the "Edit clip" entry button
///   (`enter_selected_clip` on the panel selection), or a hint
///   when nothing is selected — no dead controls.
/// * No session, no clips: an empty section (nothing exists
///   that could be edited).
fn clip_edit_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    if session_active(core) {
        let label = session_label(core).unwrap_or_default();
        return col()
            .key("clip-edit-section")
            .w_full()
            .gap(6.0)
            .child(
                text(label)
                    .font(type_::LABEL)
                    .color(t.text),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(btn("clip-edit-prev", "Prev frame", false, t, |s: &mut State| {
                        session_prev(&mut s.core)
                    }))
                    .child(btn("clip-edit-next", "Next frame", false, t, |s: &mut State| {
                        session_next(&mut s.core)
                    })),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(btn(
                        "clip-edit-add-frame",
                        "Add frame",
                        false,
                        t,
                        |s: &mut State| s.core.clip_add_frame(),
                    ))
                    .child(btn(
                        "clip-edit-delete-frame",
                        "Delete frame",
                        false,
                        t,
                        |s: &mut State| s.core.clip_delete_frame(),
                    )),
            )
            .child(btn("clip-edit-exit", "Exit", false, t, |s: &mut State| {
                s.core.exit_clip()
            }));
    }
    let section = col().key("clip-edit-section").w_full().gap(6.0);
    if core.project.clips.is_empty() {
        return section;
    }
    match panel_state().clamped(core.project.clips.len()) {
        Some(_) => section.child(btn(
            "clip-edit-enter",
            "Edit clip",
            false,
            t,
            |s: &mut State| {
                enter_selected_clip(&mut s.core);
            },
        )),
        None => section.child(
            text("Select a clip to edit its frames.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        ),
    }
}

/// The movieclips panel: header + count, the Make/Insert
/// actions, the clip list (each row with Dup, Rename, Up,
/// Dn and Del buttons), the rename editor row while a rename is in
/// progress, the selected clip's loop-mode segmented
/// control, and the clip-edit session section
/// (`clip_edit_section`: enter/frame-step/add/delete/exit over
/// the core's `enter_clip`/`clip_goto`/`clip_add_frame`/
/// `clip_delete_frame`/`exit_clip` session ops).
pub fn clips_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let clips = &core.project.clips;
    let selected = panel_state().clamped(clips.len());
    let mut panel = col()
        .key("clips-panel")
        .w_full()
        .fill(t.card)
        .radius(8.0)
        .padding(10.0)
        .gap(6.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(icon_el("film", 14.0, t.text_secondary))
                .child(
                    text("Movieclips")
                        .font(type_::SECTION)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    text(format!("{} clip(s)", clips.len()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(btn("make-clip", "Make clip", false, t, |s| {
                    s.core.make_clip_from_figure()
                }))
                .child(btn("insert-clip", "Insert clip", false, t, |s| {
                    s.core.insert_clip_figure()
                })),
        );
    if clips.is_empty() {
        return panel
            .child(
                text("No movieclips in this project. \"Make clip\" builds one from the selected figure.")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            )
            .child(clip_edit_section(s, t));
    }
    for i in 0..clips.len() {
        panel = panel.child(clip_row(s, t, i, selected == Some(i)));
    }
    panel
        .child(rename_row(s, t))
        .child(loop_mode_row(s, t, selected))
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
        assert_eq!(loop_label(ClipLoop::Loop), "Loop");
        assert_eq!(loop_label(ClipLoop::PingPong), "Ping-pong");
        assert_eq!(loop_label(ClipLoop::Once), "Once");
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
        assert_eq!(span_text(&clip_with_frames(0)), "no frames");
        assert_eq!(span_text(&clip_with_frames(1)), "frame 1 · 1 frame");
        assert_eq!(span_text(&clip_with_frames(5)), "frames 1-5 · 5 frames");
    }

    #[test]
    fn usage_text_used_and_unused() {
        let mut core = EditorCore::new();
        // No figure plays clip 99 (or any clip) yet.
        assert_eq!(usage_text(&core.project.figures, 99), "not used by any figure");
        let fig_name = core.project.figures[core.selected_figure].name.clone();
        core.make_clip_from_figure();
        let used = usage_text(&core.project.figures, 0);
        assert!(used.starts_with("used by "), "unexpected usage: {used}");
        assert!(used.contains(&fig_name), "usage misses figure: {used}");
        assert_eq!(usage_text(&core.project.figures, 99), "not used by any figure");
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
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
loop_playback: true,
            was_playing: false,
            clips_rename: ClipRenameState::new(),
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
        assert_eq!(core.clip_edit.map(|s| s.clip), Some(0));
        assert_eq!(core.clip_edit.map(|s| s.frame), Some(0));
        core.exit_clip();
        update_panel_state(|p| *p = ClipsPanelState::default());
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
        assert_eq!(core.clip_edit.unwrap().frame, 1);
        session_prev(&mut core);
        assert_eq!(core.clip_edit.unwrap().frame, 0);
        session_prev(&mut core);
        assert_eq!(core.clip_edit.unwrap().frame, 0);
        // Add frame (the button's core op): the clip grows and
        // the session lands on the new frame; the project
        // timeline is untouched.
        core.clip_add_frame();
        assert_eq!(core.project.clips[0].frames.len(), n0 + 1);
        assert_eq!(core.clip_edit.unwrap().frame, n0);
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
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
loop_playback: true,
            was_playing: false,
            clips_rename: ClipRenameState::new(),
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
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
loop_playback: true,
            was_playing: false,
            clips_rename: ClipRenameState::new(),
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
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
loop_playback: true,
            was_playing: false,
            clips_rename: ClipRenameState::new(),
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
}
