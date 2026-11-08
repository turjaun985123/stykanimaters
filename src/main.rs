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
mod live_mcp;
mod rune_sec;
mod stage_view;

#[cfg(test)]
#[path = "rune_sec/snap_tests.rs"]
mod snap_tests;

use std::time::Duration;

use rune::{
    canvas, col, row, Align, AppSpec, Cx, Drag, El, Key, KeyEvent, Phase, PointerEvent, ScrollSpec,
    Side, TextEdit, Tx, WheelEvent,
};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

use stage_view::stage_shape;

thread_local! {
    /// The stage canvas's last laid-out size, written by
    /// `view` on every build and read by the secondary-press
    /// handler (Phase K integrator wiring): a right-press
    /// handler receives only element-local points, with no
    /// element size and no `Cx`, so the size the drawing
    /// used is handed over through here — the same
    /// thread-local hand-off pattern other sections use
    /// (clips selection, camera clipboard).
    static STAGE_EL_SIZE: std::cell::Cell<[f32; 2]> = const { std::cell::Cell::new([0.0, 0.0]) };
}

thread_local! {
    /// The live-MCP router (see `live_mcp`): an `McpServer`
    /// used purely as a JSON-RPC dispatcher. `poll` drains
    /// the bridge's request queue through it against the
    /// LIVE core via `McpServer::handle_on`, so the router's
    /// own core is never the edit target and stays untouched.
    /// A thread-local for the same reason as `STAGE_EL_SIZE`
    /// — shell plumbing with no home in `State` (the State
    /// census is integrator-gated) — and a lazy one:
    /// `McpServer::new` builds an `EditorCore`, so it is not
    /// a const constructor and cannot ride the `const {}`
    /// block above. The UI thread is the only thread that
    /// ever polls, so the router lives where it is used.
    static MCP_ROUTER: std::cell::RefCell<stykanimate_rs::mcp::McpServer> =
        std::cell::RefCell::new(stykanimate_rs::mcp::McpServer::new());
}

/// A destructive session action armed and awaiting its
/// confirm press (UI mission Wave 4, audit S16 D2): the
/// actions these variants name replace the live project —
/// New, an Open (a project-tools row, a Home row, or the
/// app bar's Load of the working file), Reload from disk,
/// and Recover of the autosave snapshot — and each used
/// to discard unsaved work the moment it was pressed,
/// with no confirmation and no undo snapshot. Now, while
/// the session is dirty, the first press only arms (the
/// button re-renders as a danger-styled confirm); the
/// second press on the armed button runs the action and
/// clears the field. Any other press in the same panel —
/// or arming a different action — stands the arm down,
/// the `HomePanelState::confirm_delete` discipline
/// carried to the session level.
///
/// "Dirty" is the shell's one truthful saved-state
/// signal: `core.edits_since_autosave() > 0`, the same
/// counter the app bar's modified dot and the Project
/// section's autosave line read. It counts edits the
/// recovery snapshot has not captured. Its known limit,
/// inherited from the core: the counter is not reset by
/// a project swap (New / Open / Load leave it alone), so
/// shortly after a confirmed swap it can still read
/// dirty until the next autosave (every 25 counted
/// edits) catches up — an arm that asks once more, never
/// a discard that happens silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Replace the session with a blank/demo project
    /// (project-tools New, Home's New project).
    NewProject,
    /// Open a different project over the session
    /// (project-tools rows, Home rows / reopen, the app
    /// bar's Load).
    OpenProject,
    /// Re-open the session's saved file, discarding the
    /// in-memory state (project-tools Reload).
    ReloadProject,
    /// Replace the session with the recovery snapshot
    /// (project-tools Recover, Home's recovery banner).
    RecoverSnapshot,
}

pub struct State {
    pub core: EditorCore,
    /// Last `poll` timestamp (seconds since start), for the
    /// playback delta. View never reads it.
    last_poll: Option<f64>,
    /// Sub-step wheel-zoom accumulation: precise (trackpad)
    /// deltas are gathered here until they make a full 1.1×
    /// step, so one smooth gesture becomes a few view zooms
    /// instead of a stream of micro-zooms. The steps drive the
    /// shell view layer (`stage_view::view_zoom_at`), never the
    /// authored camera (UI mission Wave 5, audit S10 D4).
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
    /// Figure rename field (inspector_figure section, Phase F).
    pub inspector_figure: rune_sec::inspector_figure::InspectorFigureEditState,
    /// Movieclip rename session (clips_panel section, Phase F).
    pub clips_rename: rune_sec::clips_panel::ClipRenameState,
    /// FX browser filter + recents (fx_browser section,
    /// Phase F).
    pub fx_browser: rune_sec::fx_browser::FxBrowserState,
    /// Draw stamp parameters (draw_tools section, Phase F).
    pub draw_tools: rune_sec::draw_tools::DrawToolsState,
    /// Selected stroke, project-wide index — the same index
    /// `delete_stroke_at` and the stage highlight geometry
    /// use (Phase G selection wiring).
    pub stroke_selected: Option<usize>,
    /// Selected particle event, index into
    /// `project.particles` — the index the stage FX gizmo
    /// geometry uses (Phase G selection wiring).
    pub particle_selected: Option<usize>,
    /// Camera-frustum guide overlay on/off (Phase G guide
    /// split; drawn only when `guides` is also on).
    pub guides_frustum: bool,
    /// Safe-frame guide overlay on/off (Phase G guide split;
    /// drawn only when `guides` is also on).
    pub guides_safe: bool,
    /// Onion-skin depths (onion_controls section, Phase G).
    pub onion_ui: rune_sec::onion_controls::OnionControlsState,
    /// Loop-playback toggle (playback_controls section,
    /// Phase G): when true the playhead wraps at the end
    /// (the engine's `advance_play` behaviour); when false
    /// `poll` stops on the last frame instead.
    pub loop_playback: bool,
    /// Playback speed multiplier (Phase K groundwork):
    /// poll scales the advance step by this; the transport
    /// row's speed button cycles it. 1.0 = project fps.
    pub playback_speed: f32,
    /// Pose-library rename session (pose_tools section,
    /// Phase G).
    pub pose_rename: rune_sec::pose_tools::PoseRenameState,
    pub pose_library: rune_sec::pose_tools::PoseLibraryState,
    /// Previous `core.playing` as seen by `poll`, so playback
    /// audio starts/stops on the flip (Phase F audio glue).
    was_playing: bool,
    /// Frame the playhead sat on at the previous `poll`
    /// tick — the wrap detector for playback audio
    /// (Phase G audio loop restart).
    last_frame: usize,
    /// Last status line fed to the toast stack.
    last_status: String,
    /// Shell clock (seconds) as of the last `poll`, for the
    /// toast view's age maths.
    toast_now: f32,
    /// Last persisted dark-theme value (LayoutPrefs.dark), so
    /// `poll` saves only on change.
    last_dark: bool,
    /// Destructive session action armed and awaiting its
    /// confirm press (UI mission Wave 4; see
    /// [`ConfirmAction`]). `None` — the resting state —
    /// means no discard is pending anywhere in the shell.
    pub confirm_action: Option<ConfirmAction>,
    /// Home library rename session (home_panel section,
    /// UI mission Wave 4): the armed row's path and the
    /// name field's edit buffer, homed here exactly as
    /// `clips_rename` / `pose_rename` are (a `text_input`
    /// `on_edit` needs a `&mut TextEdit` path into
    /// `State`, so the session cannot live in a
    /// thread-local).
    pub home_rename: rune_sec::home_panel::HomeRenameState,
}

fn init() -> State {
    let mut core = EditorCore::new();
    // Theme comes from the persisted layout prefs (dark is
    // the default); the settings section flips `light_theme`
    // and `poll` writes changes back.
    let layout = stykanimate_rs::shell::load_layout();
    core.light_theme = !layout.dark;
    // Live MCP control starts with the editor when the
    // persisted preference says so (LayoutPrefs::live_mcp,
    // which defaults to on for prefs files written before
    // the field existed). A failed start — in practice
    // another editor instance already holding the port —
    // is not fatal and needs no message here: the bridge
    // simply stays off, and the Settings card's Live MCP
    // toggle reads the bridge's real state, so the user
    // sees it off and can retry once the port is free.
    if layout.live_mcp {
        let _ = live_mcp::set_enabled(true);
    }
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
        inspector_figure: rune_sec::inspector_figure::InspectorFigureEditState::new(),
        clips_rename: rune_sec::clips_panel::ClipRenameState::new(),
        fx_browser: Default::default(),
        draw_tools: Default::default(),
        stroke_selected: None,
        particle_selected: None,
        guides_frustum: true,
        guides_safe: true,
        onion_ui: Default::default(),
        loop_playback: true,
        playback_speed: 1.0,
        pose_rename: rune_sec::pose_tools::PoseRenameState::new(),
        pose_library: rune_sec::pose_tools::PoseLibraryState::new(),
        was_playing: false,
        last_frame: 0,
        last_status: status,
        toast_now: 0.0,
        last_dark: layout.dark,
        confirm_action: None,
        home_rename: rune_sec::home_panel::HomeRenameState::new(),
    }
}

/// Canvas drag → posing/camera/draw ops. The element-local
/// point converts to stage coordinates through the same
/// transform the drawing uses (built here from the drag's
/// own `w`/`h` and the camera at the current frame, with the
/// shell view navigation folded in — `stage_view::
/// drag_transform` — so a drag lands on the grabbed joint at
/// any view zoom). The mode
/// branches mirror the core's own mode fields, as the GPUI
/// canvas did: draw mode paints, camera mode pans, otherwise
/// the press hit-tests and drags a joint.
fn stage_drag(s: &mut State, d: &mut Drag) {
    let core = &mut s.core;
    let rt = stage_view::drag_transform(core, [d.w, d.h]);
    let p = rt.to_stage([d.x, d.y]);
    match d.phase {
        Phase::Down => {
            if core.draw_mode {
                core.begin_draw(p);
            } else if core.camera_mode {
                core.begin_camera_drag(p);
            } else if !core.begin_drag(p) {
                // The press hit no joint (begin_drag only
                // starts on a joint and reports the miss):
                // pick the nearest stroke instead — the
                // selection the stage highlight draws and
                // the stroke panel steps (Phase G). A clean
                // miss clears the selection.
                s.stroke_selected = rune_sec::stage_layers::stroke_at_point(
                    core,
                    p,
                    rune_sec::stage_layers::PICK_TOLERANCE,
                );
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

/// Canvas right-press → selection only (Phase K
/// integrator wiring for stage_view §9): convert the
/// element-local point through the same transform
/// the drag path uses (`stage_view::drag_transform`, view
/// navigation included) — built from the stage size `view`
/// last laid out (see `STAGE_EL_SIZE`; before any layout
/// the project stage size stands in, the same fallback
/// `view` itself uses) — then hand over to
/// `stage_view::secondary_select`, which mirrors
/// `begin_drag`'s selection writes without starting a
/// drag or taking a snapshot.
fn stage_secondary(s: &mut State, e: PointerEvent) {
    let size = STAGE_EL_SIZE.with(|c| c.get());
    let size = if size[0] > 0.0 && size[1] > 0.0 {
        size
    } else {
        s.core.project.stage
    };
    let rt = stage_view::drag_transform(&s.core, size);
    let p = rt.to_stage([e.x, e.y]);
    stage_view::secondary_select(&mut s.core, p);
}

/// Canvas wheel → VIEW zoom (UI mission Wave 5, audit
/// S10 D4): the steps drive the shell view layer
/// (`stage_view::view_zoom_at`, zoom-to-cursor), never the
/// authored camera — this handler used to call
/// `EditorCore::camera_zoom_by`, so inspecting a detail
/// upserted a camera key and an undo snapshot per step.
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
        let size = STAGE_EL_SIZE.with(|c| c.get());
        stage_view::view_zoom_at(
            &s.core,
            size,
            [e.x, e.y],
            1.1f32.powf(steps).clamp(0.4, 2.5),
        );
        s.wheel_zoom -= steps;
    }
    true
}

/// Shell housekeeping + playback clock. Housekeeping: status
/// changes become toasts (expired here; while any toast is
/// live the shell wakes twice a second so they fade on time),
/// theme flips persist to the layout prefs, and queued
/// live-MCP requests drain against the live core (see
/// `live_mcp`). Playback:
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
    rune_sec::inspector_figure::sync_figure_name_edit(s);
    // Autosave cadence (Phase G wiring): the core counts
    // snapshot edits and writes the recovery snapshot every
    // 25; nothing called the check before, so the counter
    // grew for a whole session and recovery was only ever
    // written by explicit actions.
    s.core.maybe_autosave();
    let now = t as f32;
    s.toast_now = now;
    let mut redraw = false;
    // Live MCP control: requests queued by the bridge's
    // listener thread run HERE, on the UI thread, against
    // the live core — so the stage, timeline and panels
    // show the client's edits as they land (the `live_mcp`
    // module docs carry the synchronisation contract).
    // Drained before the status check below so a tool's
    // status line toasts in the same tick it was written,
    // and capped per tick so a request burst cannot stall
    // the frame; the queue keeps the rest for the next one.
    if live_mcp::is_enabled() {
        let drained = MCP_ROUTER.with(|router| {
            live_mcp::drain(&mut s.core, &mut router.borrow_mut(), 8)
        });
        if drained > 0 {
            redraw = true;
        }
    }
    if s.core.status != s.last_status {
        s.last_status = s.core.status.clone();
        if !s.last_status.is_empty() {
            s.toasts
                .push(
                    s.last_status.clone(),
                    rune_sec::toasts::classify_status(&s.last_status),
                    now,
                );
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
    // Playback audio follows the playing flag (Phase F): the
    // flip is observed here so every play/pause source —
    // transport button, Space, command palette — starts and
    // stops the sound in one place.
    if s.core.playing != s.was_playing {
        s.was_playing = s.core.playing;
        if s.core.playing {
            s.core.start_playback_audio();
        } else {
            s.core.stop_playback_audio();
        }
    }
    if !s.core.playing {
        // Keep the wrap detector honest while paused: seeks
        // and steps move `current` without playing, and the
        // next Play press must not read them as a wrap.
        s.last_frame = s.core.current;
        return redraw;
    }
    // Loop off (Phase G): stop on the last frame instead
    // of letting `advance_play` wrap. The sound is stopped
    // here, not left to the flip hook's next tick — after
    // a stop, nothing schedules another prompt wake.
    if rune_sec::playback_controls::should_stop_at_end(
        s.core.playing,
        s.loop_playback,
        s.core.loop_range,
        s.core.current,
        s.core.frame_count(),
    ) {
        s.core.playing = false;
        s.was_playing = false;
        s.core.stop_playback_audio();
        s.last_frame = s.core.current;
        return true;
    }
    let prev_frame = s.last_frame;
    // Playback speed (Phase K): the audio side is told the
    // current rate on every playing tick, so its drift
    // prediction tracks the same clock the picture runs on
    // (§1's contract in audio_play).
    s.core.note_playback_speed(s.playback_speed);
    if dt > 0.0 {
        // The playhead advances by the speed-scaled elapsed
        // time: at 2.0 a tick moves the playhead twice as
        // far, at 0.5 half as far.
        s.core.advance_play(dt * s.playback_speed);
    }
    // A wrapped playhead restarts the audio pass from the
    // top (Phase G); a no-op while audio is inactive.
    s.core.sync_playback_audio_on_wrap(prev_frame, s.core.current);
    s.core.sync_playback_audio_on_seek(prev_frame, s.core.current);
    // Full-loop restart (Phase L): a tick that crossed a
    // whole loop without the playhead decreasing is invisible
    // to the wrap and seek syncs above; the fold-counting
    // walk detects it from the audio clock and restarts the
    // pass. It stands down on any frame decrease, so a real
    // wrap never restarts twice.
    s.core.sync_playback_audio_on_full_loop(prev_frame);
    // Drift correction (Phase J): on ticks where the frame
    // actually changed, ask the player to re-align the sound
    // with the playhead — the wrap and seek syncs above only
    // fire on jumps, so slow drift within a pass had no
    // remedy. A paused or idle shell never reaches this
    // block, and a tick that did not move the frame skips it.
    if s.core.current != prev_frame {
        s.core.sync_playback_audio_on_drift();
    }
    s.last_frame = s.core.current;
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
        || s.inspector_figure.name.focused
        || s.clips_rename.field.focused
        || s.library_panel.rename.focused
        || s.fx_browser.query.focused
        || s.figures_panel.filter.focused
        || s.pose_rename.field.focused
        || s.pose_library.filter.focused
}

/// App shortcuts. The command palette gets first claim
/// (Ctrl+K opens it; arrows/Enter/Escape drive it while
/// open). While a text field is focused, everything else is
/// left to the field. Then: Ctrl+Z undo, Ctrl+Shift+Z /
/// Ctrl+Y redo, Ctrl+S quick-save (the app-bar Save
/// button's press, nothing more), Escape closes an open
/// menu, Space play/pause,
/// Left/Right frame step, Delete removes the selected stroke
/// if one is selected, else the selected particle event,
/// else the selected segment (`delete_segment` no-ops safely
/// — status only — when nothing deletable is selected),
/// C toggles camera
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
    if ev.is_command('s') {
        // Quick-save (Phase K): exactly the app-bar Save
        // button's press — the core's plain save op, with
        // no second save path invented for the keyboard.
        s.core.save();
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
            // Phase J priority: a selected stroke goes first,
            // then a selected particle event, then the
            // long-standing segment behaviour. The ops refuse
            // safely (status only) on a stale index, and the
            // selection is cleared either way — the thing it
            // named is gone or never was.
            if let Some(i) = s.stroke_selected {
                s.core.delete_stroke_at(i);
                s.stroke_selected = None;
            } else if let Some(i) = s.particle_selected {
                s.core.particle_delete_event(i);
                s.particle_selected = None;
            } else {
                s.core.delete_segment();
            }
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

/// UI mission Wave 9 (splitter prep): the side-column widths
/// the main row lays out with, in points — left first, right
/// second. The Phase J layout fixed them at 300 / 340 with the
/// centre taking the rest; they resolve through this one
/// function so the drag splitter has exactly one place
/// to read them from.
///
/// WAVE9-SPLITTER integration point: `State` holds no prefs —
/// the shell reaches `LayoutPrefs` through
/// `stykanimate_rs::shell::load_layout()` (the route `init`,
/// the theme writer in `poll` and the settings handlers
/// already use), so the widths read the persisted
/// `col_left_w` / `col_right_w` fields through it. Those
/// fields serde-default to the Phase J constants (300 /
/// 340) — which is also what a prefs file written before
/// they existed yields — and `load_layout` clamps them into
/// their sane ranges, so whenever the prefs hold their
/// defaults this returns exactly (300, 340) and the default
/// geometry, stage [328, 157, 744, 530] at 1440×900, stays
/// byte-identical. (The read re-loads the small prefs file
/// on each view pass; if the integrator prefers a State
/// mirror seeded in `init`, this body is the only place
/// that changes.)
fn column_widths() -> (f32, f32) {
    let prefs = stykanimate_rs::shell::load_layout();
    (prefs.col_left_w, prefs.col_right_w)
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = rune_sec::app_theme::app_theme(!s.core.light_theme);
    let core = &s.core;
    // The canvas size from the last committed layout (the read
    // settles on the first layout); before any layout exists,
    // draw at stage size (fit 1:1).
    let stage_size = cx.layout_size("stage").unwrap_or(core.project.stage);
    STAGE_EL_SIZE.with(|c| c.set(stage_size));
    // The stage ELEMENT wears the chrome card colour: the
    // stage rect itself is filled with the project background
    // and framed inside `stage_shape` (UI mission Wave 5,
    // audit S10 D1), so the letterbox around the stage reads
    // as surround instead of extending the stage visually.
    let stage_fill = t.card;

    // Workspace gating (Phase E): a panel mounts only in the
    // workspaces whose table lists it; a column left empty by
    // gating isn't mounted at all. Settings and help are
    // deliberately ungated (the workspaces module documents
    // why). Figure-add used to be ungated too; Wave 3 of the
    // UI mission gates it on the Figures section (below).
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
        // Figure-add is the Figures area's own chrome, so it
        // mounts under exactly the same gate as the figures
        // panel (UI mission Wave 3, audit S05 D1): it used to
        // mount unconditionally and orphan into Home and
        // Export — a figure-creation panel in the two
        // workspaces where the Figures panel it belongs to
        // is gated off.
        left = left.child(rune_sec::figure_add::figure_add_panel(s, &t));
    }
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
    // Phase N mount order (P16): Graph leads the keyframe
    // tools so the curve canvas starts above the fold; the
    // 3D panel and the everyday Playback/Frames panels sit
    // above Camera; Scene is demoted below the
    // Particles/Audio block so it no longer fills the
    // Effects workspace's first screen.
    if vis(Sec::Graph) {
        right = right.child(rune_sec::graph_panel::graph_panel(s, cx, &t));
    }
    if vis(Sec::KeyframeTools) {
        right = right.child(rune_sec::keyframe_tools::keyframe_section(s, &t));
    }
    if vis(Sec::ThreeD) {
        right = right.child(rune_sec::three_d_panel::three_d_panel(s, &t));
    }
    if vis(Sec::Playback) {
        right = right.child(rune_sec::playback_controls::playback_controls(s, &t));
    }
    if vis(Sec::FrameOps) {
        right = right.child(rune_sec::frame_ops::frames_section(s, &t));
    }
    if vis(Sec::Camera) {
        right = right.child(rune_sec::camera_panel::panel(s, &t));
    }
    if vis(Sec::Onion) {
        right = right.child(rune_sec::onion_controls::onion_controls(s, &t));
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
    if vis(Sec::Particles) {
        right = right.child(rune_sec::particles_panel::particles_panel(s, &t));
    }
    if vis(Sec::Audio) {
        right = right.child(rune_sec::audio_panel::audio_panel(s, &t));
    }
    if vis(Sec::Waveform) {
        // Phase N (P4): the waveform canvas sits in a card
        // like every other panel — the canvas keeps its
        // exact [308, 56] size (its drag maths are built
        // for it); the card is integrator-owned because
        // main.rs mounts the canvas directly.
        right = right.child(
            rune_sec::design::panel_col("waveform-card", &t)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(rune_sec::design::panel_title(
                            Some("wave-square"),
                            "Waveform",
                            None,
                            &t,
                        )),
                )
                .child(rune_sec::design::divider(&t))
                .child(rune_sec::waveform_view::waveform_el_state(s, [308.0, 56.0], &t)),
        );
    }
    if vis(Sec::Clips) {
        right = right.child(rune_sec::clips_panel::clips_panel(s, &t));
    }
    if vis(Sec::Scene) {
        right = right.child(rune_sec::scene_panel::scene_panel(s, &t));
    }
    if vis(Sec::Export) {
        right = right.child(rune_sec::export_panel::export_panel(s, &t));
    }
    if vis(Sec::Stats) {
        right = right.child(rune_sec::stats_view::stats_panel(s, &t));
    }

    // Centre column: stage (with the toasts and the camera
    // overlay pill pinned inside it), transport, timeline,
    // status strip. The command
    // palette is NOT a centre child (UI mission Wave 3,
    // audit S19 §2): mounted in flow, its ~521 pt card
    // plus the column's fixed furniture (~230 pt) exceed
    // the centre's height, and the stage — the column's
    // only flexible child — is what gives, collapsing to
    // height 0 while the tail overflows past the window
    // bottom. It now mounts at the root as an anchored
    // overlay (see the root column below), so opening it
    // cannot change the stage's size in any workspace.
    let mut centre = col().key("centre").gap(12.0).grow(1.0).min_w(0.0);
    centre = centre
        .child(
            // The stage wrapper: a positioning parent that
            // takes exactly the layout slot the canvas used
            // to take in the centre column (grow / full
            // width / shrinkable), and the canvas — the
            // wrapper's only in-flow child, growing to fill
            // it — keeps the laid-out box it always had, so
            // every number derived from the stage's size is
            // unchanged. The wrapper exists so the toast
            // stack can dock INSIDE the stage's box: the
            // toasts column is out of flow (`.pin` sets
            // taffy position-absolute, the same mechanism
            // the menu scrim's `.absolute` uses), pinned to
            // the stage's top-right corner, so a toast
            // appearing or expiring overlays the canvas and
            // can never resize the stage or shift any other
            // element. The `.pin(...)` here is the outermost
            // placement call on the returned element (the
            // menu dropdown's `.anchored(...)` discipline):
            // it replaces the window-corner pin `toasts_view`
            // carries for a root mount with stage-corner
            // insets; the stack's keys, contents, timing and
            // dismissal behaviour are untouched.
            col()
                .key("stage-wrap")
                .grow(1.0)
                .w_full()
                .min_h(0.0)
                .child(
                    canvas(stage_shape(
                        core,
                        stage_size,
                        crate::stage_view::StageOverlays {
                            guides: s.guides,
                            guides_frustum: s.guides_frustum,
                            guides_safe: s.guides_safe,
                            stroke_selected: s.stroke_selected,
                            particle_selected: s.particle_selected,
                            fx_selected: s.fx_selected,
                            fig3_stage: s.workspace
                                == rune_sec::workspaces::Workspace::Build3D
                                && core.fig3.is_some(),
                            onion_prev: s.onion_ui.prev,
                            onion_next: s.onion_ui.next,
                        },
                    ))
                    .key("stage")
                    .grow(1.0)
                    .w_full()
                    .min_h(0.0)
                    .fill(stage_fill)
                    .radius(12.0)
                    .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| stage_drag(s, d))
                    .on_secondary_press_local(
                        |s: &mut State, e: PointerEvent, _tx: &mut Tx| {
                            stage_secondary(s, e)
                        },
                    )
                    .on_wheel(|s: &mut State, e: WheelEvent, _tx: &mut Tx| {
                        stage_wheel(s, e)
                    }),
                )
                .child(
                    rune_sec::toasts::toasts_view(s, s.toast_now)
                        .pin(None, Some(12.0), Some(12.0), None),
                )
                // The camera overlay pill, pinned to the
                // stage's bottom-right corner with the same
                // `.pin` discipline (UI mission Wave 5, audit
                // S10 D5): it used to sit in a full-width row
                // under the canvas, costing the stage its
                // height; pinned here it floats over the
                // canvas corner — presses land on the pill,
                // as on the toasts' buttons — and the
                // reclaimed row height returns to the stage.
                .child(
                    rune_sec::camera_panel::overlay(core)
                        .pin(None, None, Some(12.0), Some(12.0)),
                ),
        )
        .child(chrome_view::transport_row(s, &t))
        .child(rune_sec::timeline_marks::timeline_marks(s, &t))
        .child(rune_sec::status_strip::status_strip_state(s));

    // The side columns mount only while the View toggles show
    // them (menus section); the centre always mounts.
    //
    // Phase J layout fix: the side viewports keep their fixed
    // widths (300 / 340 — resolved through `column_widths`
    // since UI mission Wave 9) and the centre takes the
    // rest. They
    // used to also carry `.grow(1.0)`, which flattened all
    // three columns to equal thirds (~461 pt each at 1440
    // wide): the centre — stage, timeline and status strip —
    // was starved to a third of the window, and the status
    // strip's floored segments (Phase F width floors) summed
    // wider than the strip, so its tail painted rightwards
    // over the right column's bottom rows. The scroll
    // viewports' heights were never the problem: main_row is
    // the bounded `grow` child of the root column and the
    // viewports clip their content to it (Rune's `scroll`
    // contract — a bounded ancestor, `min_h(0.0)` on the
    // viewport); only the width split was wrong.
    // WAVE9-SPLITTER: the widths resolve through
    // `column_widths()` (the integration point documented
    // there — LayoutPrefs-backed, the 300 / 340 defaults
    // whenever the prefs hold them). The splitter
    // HANDLES (design::v_splitter, Wave 9) mount in this
    // row: the left handle in the gap between the left
    // column and the centre, the right handle in the gap
    // between the centre and the right column — each
    // mounted only while its column is shown. The row gap
    // drops to 1 pt so every junction still totals the
    // pre-splitter 12 pt (1 + SPLITTER_W 10 + 1) and the
    // default geometry stays byte-identical; a drag
    // writes the clamped width straight back into
    // LayoutPrefs (the load clamps in shell.rs are the
    // same bounds), and `column_widths()` re-reads the
    // prefs on the next view pass, so the resize is live.
    let (left_w, right_w) = column_widths();
    let mut main_row = row()
        .key("main-row")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(1.0);
    if s.show_left {
        main_row = main_row.child(
            left.scroll(ScrollSpec { stick_bottom: false })
                .w(left_w)
                .min_h(0.0),
        );
        main_row = main_row.child(rune_sec::design::v_splitter(
            "split-left",
            "Drag to resize the left panel column",
            &t,
            |_s, dx| {
                let mut prefs = stykanimate_rs::shell::load_layout();
                prefs.col_left_w = (prefs.col_left_w + dx).clamp(220.0, 420.0);
                stykanimate_rs::shell::save_layout(&prefs);
            },
        ));
    }
    main_row = main_row.child(centre);
    if s.show_right {
        main_row = main_row.child(rune_sec::design::v_splitter(
            "split-right",
            "Drag to resize the right panel column",
            &t,
            |_s, dx| {
                let mut prefs = stykanimate_rs::shell::load_layout();
                prefs.col_right_w = (prefs.col_right_w - dx).clamp(260.0, 480.0);
                stykanimate_rs::shell::save_layout(&prefs);
            },
        ));
        main_row = main_row.child(
            right
                .scroll(ScrollSpec { stick_bottom: false })
                .w(right_w)
                .min_h(0.0),
        );
    }

    let mut root = col()
        .w_full()
        .h_full()
        .fill(t.bg)
        .padding(16.0)
        .gap(12.0)
        .child(chrome_view::app_bar(s, &t))
        .child(rune_sec::menus::menu_bar(s, &t))
        .child(rune_sec::workspaces::workspace_row(s, &t))
        // Charter §15: hairline under the chrome block
        // (app bar + menu bar + workspace row).
        .child(rune_sec::design::divider(&t))
        .child(main_row)
        // The menu layer's in-flow slots (UI mission
        // Wave 8): two zero-size spacers, mounted in BOTH
        // menu states. The builders used to BE these two
        // slots — a plain `el()` while closed, an
        // out-of-flow layer while open — so opening a menu
        // removed two of the root column's six 12 pt gaps
        // from the flow and the stage grew 24 pt until the
        // menu closed. Now the spacers stay in flow
        // permanently and the layers proper are appended
        // below, only while a menu is open: the root's
        // in-flow child list — hence its gap accounting
        // and every rect it feeds, the stage's included —
        // is identical open and closed. The spacers are
        // exactly the `el()`s the closed builders returned,
        // so the closed geometry is untouched.
        .child(rune::el())
        .child(rune::el());
    // The open menu's layers proper: the dismissal scrim
    // and the anchored dropdown, appended after the
    // spacers so they still paint (and hit-test) above
    // the whole shell, where they always sat.
    if s.menus.open.is_some() {
        root = root
            .child(rune_sec::menus::menu_scrim(s))
            .child(rune_sec::menus::menu_dropdown(s, &t));
    }
    // The command palette as a root-level overlay (UI
    // mission Wave 3): the menu dropdown's idiom — the
    // `.anchored(...)` placement is the outermost call on
    // the returned element, hanging the card below the
    // workspace row, centred on the window, out of flow
    // (anchored elements are position-absolute and follow
    // their anchor's rect every frame). It floats over
    // the top of the centre column; nothing in the centre
    // moves when it opens or closes. No scrim: Ctrl+K /
    // Esc handling is unchanged, and the palette never
    // had one.
    if s.palette.open {
        root = root.child(
            rune_sec::command_palette::palette_view(s, &t).anchored(
                "workspace-row",
                Side::Bottom,
                Align::Center,
                8.0,
            ),
        );
    }
    root
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
