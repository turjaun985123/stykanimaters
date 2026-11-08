//! StykAnimate RS — Rune editor shell (Phases C–D of the
//! GPUI→Rune migration).
//!
//! A working editor on the pinned Rune framework: the stage is
//! one canvas element whose `Shape` is built from the solved
//! rig each frame (`stage_view`), pointer drags pose through
//! the real `EditorCore` ops, the timeline/transport/app bar
//! live in `chrome_view`, playback runs on `AppSpec::poll` +
//! `Tx::wake_after`, and shortcuts on `AppSpec::on_key`.
//!
//! Phase D mounts the feature sections from `rune_sec` (one
//! module per area: figures, inspectors, camera, scene,
//! frames, graph, easing, draw, FX, audio, clips, library,
//! 3D, export, home/projects, menus, command palette,
//! settings, help, toasts, status) into a three-column
//! studio layout around the stage. Everything delegates to
//! `stykanimate_rs::editor::EditorCore` — the engine is
//! untouched.

mod chrome_view;
mod rune_sec;
mod stage_view;

#[cfg(test)]
#[path = "rune_sec/snap_tests.rs"]
mod snap_tests;

use std::time::Duration;

use rune::tok::{self};
use rune::{
    canvas, col, row, AppSpec, Cx, Drag, El, Key, KeyEvent, Phase, ScrollSpec, TextEdit, Tx,
    WheelEvent,
};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

use stage_view::stage_shape;

pub struct State {
    pub core: EditorCore,
    /// Last `poll` timestamp (seconds since start), for the
    /// playback delta. View never reads it.
    last_poll: Option<f64>,
    /// Sub-step wheel-zoom accumulation: `camera_zoom_by`
    /// snapshots once per call, so precise (trackpad) deltas are
    /// gathered here until they make a full 1.1× step instead of
    /// flooding the undo stack with micro-zooms.
    wheel_zoom: f32,
    // ---- Phase D section view-state (one field per section
    // that declared one; all engine state stays in `core`).
    pub figures_panel: rune_sec::figures_panel::FiguresPanelState,
    pub figure_add: rune_sec::figure_add::FigureAddState,
    pub library_panel: rune_sec::library_panel::LibraryPanelState,
    pub home_panel: rune_sec::home_panel::HomePanelState,
    pub frame_ops: rune_sec::frame_ops::FrameOpsState,
    pub graph_panel: rune_sec::graph_panel::GraphPanelState,
    pub audio_panel: rune_sec::audio_panel::AudioPanelState,
    pub menus: rune_sec::menus::MenusState,
    pub palette: rune_sec::command_palette::PaletteState,
    pub settings: rune_sec::settings_popover::SettingsState,
    pub toasts: rune_sec::toasts::ToastsState,
    pub text_tool: rune_sec::text_tool::TextToolState,
    /// Project-name field (project_tools section).
    pub name_edit: TextEdit,
    /// Export centre range toggle (export_panel section).
    pub export_use_range: bool,
    /// Selected FX clip (fx_clips section; the core has no
    /// FX-clip selection state).
    pub fx_selected: Option<usize>,
    /// Pose clipboard (pose_tools section; the core clipboard
    /// holds whole frames, not single poses).
    pub pose_clipboard: Option<Pose>,
    /// Active workspace (Phase E workspaces section; persists
    /// through LayoutPrefs.workspace and gates which panels
    /// mount).
    pub workspace: rune_sec::workspaces::Workspace,
    /// Stage guides overlay on/off (settings section).
    pub guides: bool,
    /// Left/right column visibility (View menu toggles).
    pub show_left: bool,
    pub show_right: bool,
    /// Graph-key clipboard (keyframe_tools section).
    pub key_clipboard: Option<Vec<stykanimate_rs::graph::Key>>,
    /// Last export recap line (export_panel section).
    pub export_recap: String,
    /// Last status line fed to the toast stack.
    last_status: String,
    /// Shell clock (seconds) as of the last `poll`, for the
    /// toast view's age maths.
    toast_now: f32,
    /// Last persisted dark-theme value (LayoutPrefs.dark), so
    /// `poll` saves only on change.
    last_dark: bool,
}

fn init() -> State {
    let mut core = EditorCore::new();
    // Theme comes from the persisted layout prefs (dark is
    // the default); the settings section flips `light_theme`
    // and `poll` writes changes back.
    let layout = stykanimate_rs::shell::load_layout();
    core.light_theme = !layout.dark;
    if let Some(msg) = rune_sec::home_panel::startup_status() {
        core.status = msg;
    }
    let name = core.project.name.clone();
    let status = core.status.clone();
    State {
        core,
        last_poll: None,
        wheel_zoom: 0.0,
        figures_panel: Default::default(),
        figure_add: Default::default(),
        library_panel: Default::default(),
        home_panel: rune_sec::home_panel::initial_state(),
        frame_ops: Default::default(),
        graph_panel: Default::default(),
        audio_panel: rune_sec::audio_panel::AudioPanelState::new(),
        menus: Default::default(),
        palette: rune_sec::command_palette::PaletteState::new(),
        settings: Default::default(),
        toasts: rune_sec::toasts::ToastsState::new(),
        text_tool: rune_sec::text_tool::TextToolState::new(),
        name_edit: TextEdit::new(name),
        export_use_range: false,
        fx_selected: None,
        pose_clipboard: None,
        workspace: rune_sec::workspaces::from_prefs(&layout),
        guides: false,
        show_left: true,
        show_right: true,
        key_clipboard: None,
        export_recap: String::new(),
        last_status: status,
        toast_now: 0.0,
        last_dark: layout.dark,
    }
}

/// Canvas drag → posing/camera/draw ops. The element-local
/// point converts to stage coordinates through the same
/// `ViewTransform` the drawing uses (built here from the drag's
/// own `w`/`h` and the camera at the current frame — the GPUI
/// stage's `view_transform`/`to_stage` pairing). The mode
/// branches mirror the core's own mode fields, as the GPUI
/// canvas did: draw mode paints, camera mode pans, otherwise
/// the press hit-tests and drags a joint.
fn stage_drag(s: &mut State, d: &mut Drag) {
    let core = &mut s.core;
    let rt = rune_sec::stage_camera::RotTransform::for_drag(core, [d.w, d.h]);
    let p = rt.to_stage([d.x, d.y]);
    match d.phase {
        Phase::Down => {
            if core.draw_mode {
                core.begin_draw(p);
            } else if core.camera_mode {
                core.begin_camera_drag(p);
            } else {
                core.begin_drag(p);
            }
        }
        Phase::Move => {
            if core.draw_mode {
                core.draw_to(p);
            } else {
                core.drag_to(p);
            }
        }
        Phase::Up => {
            if core.draw_mode {
                core.end_draw();
            } else {
                core.end_drag();
            }
        }
    }
}

/// Canvas wheel → camera zoom (`EditorCore::camera_zoom_by`).
/// Notched wheels step once per event; precise deltas
/// accumulate in `State::wheel_zoom` (see its docs). Returning
/// true captures the rest of the gesture.
fn stage_wheel(s: &mut State, e: WheelEvent) -> bool {
    if e.dy == 0.0 {
        return false;
    }
    s.wheel_zoom += if e.precise { -e.dy * 0.02 } else { -e.dy.signum() };
    let steps = s.wheel_zoom.trunc();
    if steps != 0.0 {
        s.core.camera_zoom_by(1.1f32.powf(steps).clamp(0.4, 2.5));
        s.wheel_zoom -= steps;
    }
    true
}

/// Shell housekeeping + playback clock. Housekeeping: status
/// changes become toasts (expired here; while any toast is
/// live the shell wakes twice a second so they fade on time),
/// and theme flips persist to the layout prefs. Playback:
/// while `core.playing`, advance the playhead by the real
/// elapsed time (clamped so a long stall doesn't jump) and
/// schedule the next frame at the project fps interval — the
/// Rune form of the GPUI app's 33 ms tick calling
/// `advance_play`.
fn poll(s: &mut State, t: f64, tx: &mut Tx) -> bool {
    let dt = s
        .last_poll
        .map(|last| ((t - last) as f32).clamp(0.0, 0.1))
        .unwrap_or(0.0);
    s.last_poll = Some(t);
    rune_sec::project_tools::sync_name_edit(s);
    let now = t as f32;
    s.toast_now = now;
    let mut redraw = false;
    if s.core.status != s.last_status {
        s.last_status = s.core.status.clone();
        if !s.last_status.is_empty() {
            s.toasts
                .push(s.last_status.clone(), rune_sec::toasts::Severity::Info, now);
        }
    }
    if !s.toasts.is_empty() {
        s.toasts.expire(now);
        tx.wake_after(Duration::from_millis(500));
        redraw = true;
    }
    let dark = !s.core.light_theme;
    if dark != s.last_dark {
        s.last_dark = dark;
        let mut prefs = stykanimate_rs::shell::load_layout();
        prefs.dark = dark;
        stykanimate_rs::shell::save_layout(&prefs);
    }
    if !s.core.playing {
        return redraw;
    }
    if dt > 0.0 {
        s.core.advance_play(dt);
    }
    let fps = s.core.project.fps.max(1.0);
    tx.wake_after(Duration::from_secs_f32(1.0 / fps));
    true
}

/// Wrap-around frame step (the GPUI Left/Right binding).
fn step_frame(s: &mut State, dir: i32) {
    let n = s.core.frame_count();
    if n == 0 {
        return;
    }
    let next = (s.core.current as i32 + dir).rem_euclid(n as i32);
    s.core.goto(next as usize);
}

/// True while any shell text field holds keyboard focus —
/// global shortcuts must not fire into someone's typing.
fn text_focused(s: &State) -> bool {
    s.palette.query.focused
        || s.name_edit.focused
        || s.audio_panel.name.focused
        || s.home_panel.search.focused
        || s.library_panel.search.focused
        || s.text_tool.field.focused
}

/// App shortcuts. The command palette gets first claim
/// (Ctrl+K opens it; arrows/Enter/Escape drive it while
/// open). While a text field is focused, everything else is
/// left to the field. Then: Ctrl+Z undo, Ctrl+Shift+Z /
/// Ctrl+Y redo, Escape closes an open menu, Space play/pause,
/// Left/Right frame step, Delete deletes the selected
/// segment (`delete_segment` no-ops safely — status only —
/// when nothing deletable is selected), C toggles camera
/// mode, O toggles onion skin, N adds a frame, I inserts an
/// inbetween — the bindings the help panel documents.
fn on_key(s: &mut State, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    if rune_sec::command_palette::palette_handle_key(s, ev) {
        return true;
    }
    if text_focused(s) {
        return false;
    }
    if ev.mods.alt {
        if let Key::Text(text) = &ev.key {
            if let Some(n) = text.chars().next().and_then(|c| c.to_digit(10)) {
                if (1..=8).contains(&n) {
                    let ws = rune_sec::workspaces::Workspace::all()[(n - 1) as usize];
                    rune_sec::workspaces::select_workspace(s, ws);
                    return true;
                }
            }
        }
    }
    if ev.is_command('z') {
        if ev.mods.shift {
            s.core.redo();
        } else {
            s.core.undo();
        }
        return true;
    }
    if ev.is_command('y') {
        s.core.redo();
        return true;
    }
    match &ev.key {
        Key::Escape if s.menus.open.is_some() => {
            s.menus.close();
            true
        }
        Key::Text(text) if text == " " && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            chrome_view::toggle_play(s);
            true
        }
        Key::Left => {
            step_frame(s, -1);
            true
        }
        Key::Right => {
            step_frame(s, 1);
            true
        }
        Key::Delete => {
            s.core.delete_segment();
            true
        }
        Key::Text(text) if text == "c" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.camera_mode = !s.core.camera_mode;
            true
        }
        Key::Text(text) if text == "o" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.onion = !s.core.onion;
            true
        }
        Key::Text(text) if text == "n" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.add_frame();
            true
        }
        Key::Text(text) if text == "i" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.insert_inbetween();
            true
        }
        _ => false,
    }
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = tok::theme(!s.core.light_theme);
    let core = &s.core;
    // The canvas size from the last committed layout (the read
    // settles on the first layout); before any layout exists,
    // draw at stage size (fit 1:1).
    let stage_size = cx.layout_size("stage").unwrap_or(core.project.stage);
    let stage_fill = stage_view::hex_color(core.project.background).unwrap_or(t.card);

    // Workspace gating (Phase E): a panel mounts only in the
    // workspaces whose table lists it; a column left empty by
    // gating isn't mounted at all. Figure-add, settings and
    // help are deliberately ungated (the workspaces module
    // documents why).
    use rune_sec::workspaces::Section as Sec;
    let ws = s.workspace;
    let vis = |sec: Sec| rune_sec::workspaces::section_visible(ws, sec);

    // Left column: projects + figures + library + layers.
    let mut left = col().key("side-left").gap(12.0);
    if vis(Sec::Home) {
        left = left.child(rune_sec::home_panel::home_panel(s, &t));
    }
    if vis(Sec::Project) {
        left = left.child(rune_sec::project_tools::project_section(s, &t));
    }
    if vis(Sec::Figures) {
        left = left.child(rune_sec::figures_panel::figures_panel(s, &t));
    }
    left = left.child(rune_sec::figure_add::figure_add_panel(s, &t));
    if vis(Sec::Library) {
        left = left.child(rune_sec::library_panel::library_panel(s, &t));
    }
    if vis(Sec::Layers) {
        left = left.child(rune_sec::layers_panel::panel(s, &t));
    }

    // Right column: contextual sections in workflow order —
    // selection first, then scene/camera, graph/easing, draw,
    // effects, audio, clips, 3D, export, stats.
    let mut right = col().key("side-right").gap(12.0);
    if s.menus.help_open {
        right = right.child(rune_sec::help_panel::help_panel(core, &t));
    }
    right = right.child(rune_sec::settings_popover::settings_popover(s, &t));
    if vis(Sec::InspectorSegment) {
        right = right.child(rune_sec::inspector_segment::inspector_segment(s, &t));
    }
    if vis(Sec::InspectorFigure) {
        right = right.child(rune_sec::inspector_figure::inspector_figure(s, &t));
    }
    if vis(Sec::InspectorStyle) {
        right = right.child(rune_sec::inspector_style::style_panel(s, &t));
    }
    if vis(Sec::PoseTools) {
        right = right.child(rune_sec::pose_tools::pose_tools(s, &t));
    }
    if vis(Sec::KeyframeTools) {
        right = right.child(rune_sec::keyframe_tools::keyframe_section(s, &t));
    }
    if vis(Sec::Camera) {
        right = right.child(rune_sec::camera_panel::panel(s, &t));
    }
    if vis(Sec::Scene) {
        right = right.child(rune_sec::scene_panel::scene_panel(s, &t));
    }
    if vis(Sec::Onion) {
        right = right.child(rune_sec::onion_controls::onion_controls(s, &t));
    }
    if vis(Sec::Playback) {
        right = right.child(rune_sec::playback_controls::playback_controls(s, &t));
    }
    if vis(Sec::FrameOps) {
        right = right.child(rune_sec::frame_ops::frames_section(s, &t));
    }
    if vis(Sec::Graph) {
        right = right.child(rune_sec::graph_panel::graph_panel(s, cx, &t));
    }
    if vis(Sec::Easing) {
        right = right.child(rune_sec::easing_panel::easing_panel(s, &t));
    }
    if vis(Sec::DrawTools) {
        right = right.child(rune_sec::draw_tools::draw_tools(s, &t));
    }
    if vis(Sec::StrokeInspector) {
        right = right.child(rune_sec::stroke_inspector::stroke_panel(s, &t));
    }
    if vis(Sec::TextTool) {
        right = right.child(rune_sec::text_tool::text_tool(s, &t));
    }
    if vis(Sec::FxBrowser) {
        right = right.child(rune_sec::fx_browser::fx_browser(s, &t));
    }
    if vis(Sec::FxClips) {
        right = right.child(rune_sec::fx_clips::fx_section(s, &t));
    }
    if vis(Sec::Audio) {
        right = right.child(rune_sec::audio_panel::audio_panel(s, &t));
    }
    if vis(Sec::Waveform) {
        right = right.child(rune_sec::waveform_view::waveform_el(core, [308.0, 56.0]));
    }
    if vis(Sec::Clips) {
        right = right.child(rune_sec::clips_panel::clips_panel(s, &t));
    }
    if vis(Sec::ThreeD) {
        right = right.child(rune_sec::three_d_panel::three_d_panel(s, &t));
    }
    if vis(Sec::Export) {
        right = right.child(rune_sec::export_panel::export_panel(s, &t));
    }
    if vis(Sec::Stats) {
        right = right.child(rune_sec::stats_view::stats_panel(s, &t));
    }

    // Centre column: (palette), stage, camera overlay strip,
    // transport, timeline, status strip.
    let mut centre = col().key("centre").gap(12.0).grow(1.0).min_w(0.0);
    if s.palette.open {
        centre = centre.child(rune_sec::command_palette::palette_view(s, &t));
    }
    centre = centre
        .child(
            canvas(stage_shape(core, stage_size, s.guides))
                .key("stage")
                .grow(1.0)
                .w_full()
                .min_h(0.0)
                .fill(stage_fill)
                .radius(8.0)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| stage_drag(s, d))
                .on_wheel(|s: &mut State, e: WheelEvent, _tx: &mut Tx| stage_wheel(s, e)),
        )
        .child(
            row()
                .key("stage-overlay-row")
                .w_full()
                .child(row().grow(1.0))
                .child(rune_sec::camera_panel::overlay(core)),
        )
        .child(chrome_view::transport_row(s, &t))
        .child(rune_sec::timeline_marks::timeline_marks(s, &t))
        .child(rune_sec::status_strip::status_strip(core));

    // The side columns mount only while the View toggles show
    // them (menus section); the centre always mounts.
    let mut main_row = row()
        .key("main-row")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0);
    if s.show_left {
        main_row = main_row.child(
            left.scroll(ScrollSpec { stick_bottom: false })
                .w(300.0)
                .grow(1.0)
                .min_h(0.0),
        );
    }
    main_row = main_row.child(centre);
    if s.show_right {
        main_row = main_row.child(
            right
                .scroll(ScrollSpec { stick_bottom: false })
                .w(340.0)
                .grow(1.0)
                .min_h(0.0),
        );
    }

    col()
        .w_full()
        .h_full()
        .fill(t.bg)
        .padding(16.0)
        .gap(12.0)
        .child(chrome_view::app_bar(s, &t))
        .child(rune_sec::menus::menu_bar(s, &t))
        .child(rune_sec::workspaces::workspace_row(s, &t))
        .child(main_row)
        .child(rune_sec::toasts::toasts_view(&s.toasts, s.toast_now))
        .child(rune_sec::menus::menu_scrim(s))
        .child(rune_sec::menus::menu_dropdown(s, &t))
}

fn main() {
    rune::run(AppSpec::<State> {
        min_size: Some((1280.0, 720.0)),
        scenes: Box::leak(rune_sec::scenes::scenes().into_boxed_slice()),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("StykAnimate RS", (1440.0, 900.0), init, view)
    });
}
