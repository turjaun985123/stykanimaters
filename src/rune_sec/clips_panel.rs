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

use std::cell::RefCell;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{ClipDef, ClipLoop, FigureInstance};

use crate::chrome_view::btn;
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
/// actions, the clip list, the selected clip's loop-mode
/// segmented control, and the clip-edit session section
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
}
