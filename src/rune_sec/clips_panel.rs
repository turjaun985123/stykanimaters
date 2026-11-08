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

/// The movieclips panel: header + count, the Make/Insert
/// actions, the clip list, and the selected clip's loop-mode
/// segmented control. Mirrors the GPUI `clips_panel` content
/// (the clip-editing session controls — enter/frame-step/
/// add/delete frame — are a separate section's surface; the
/// ops are `enter_clip`/`clip_goto`/`clip_add_frame`/
/// `clip_delete_frame` in editor.rs).
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
        return panel.child(
            text("No movieclips in this project. \"Make clip\" builds one from the selected figure.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for i in 0..clips.len() {
        panel = panel.child(clip_row(s, t, i, selected == Some(i)));
    }
    panel.child(loop_mode_row(s, t, selected))
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
}
