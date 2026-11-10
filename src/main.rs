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
//!
//! Wave V (V4): the three labels this file renders
//! directly — the waveform card's title and the two
//! column splitters' help labels — resolve through the
//! i18n catalogue (`rune_sec::i18n_wire::tr`; keys filed
//! as IA-V4-1). A full string census of this file and
//! `src/shell.rs` (wave-v/findings/v4.md) found nothing
//! else rendered here: the app-bar / transport /
//! workspace-selector / status-strip labels live in
//! their own modules and were wired there, and
//! `shell.rs` is a data module whose every production
//! literal is a file name, a JSON field name, an error
//! string or persisted workspace-name data.

mod chrome_view;
mod live_mcp;
mod rune_sec;
mod shell_control;
mod shell_tools_file;
mod shell_tools_layout;
mod shell_tools_selection;
mod shell_tools_settings;
mod shell_tools_state;
mod shell_tools_style;
mod shell_tools_view;
mod stage_sprites;
mod stage_view;

#[cfg(test)]
#[path = "rune_sec/snap_tests.rs"]
mod snap_tests;

#[cfg(test)]
#[path = "rune_sec/shell_control_tests.rs"]
mod shell_control_tests;

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

thread_local! {
    /// The layout-prefs shadow for the two persisted view
    /// toggles `poll` writes back (Focused Wave, D1):
    /// `(timeline_collapsed, sidebar_visible)` as last
    /// persisted to `LayoutPrefs`. `init` seeds it from
    /// the loaded prefs; `poll` compares the live values
    /// (`State::frame_ops.timeline_collapsed`,
    /// `State::show_left`) against it and load-modify-saves
    /// on divergence — the same shadow-compare discipline
    /// as `State::last_dark`, with the shadow in a
    /// thread-local because the `State` field census is
    /// integrator-gated (see MCP_ROUTER above). Every
    /// writer of the two toggles — the View menu, the
    /// palette's panel commands, the shell layout tools —
    /// mutates `State` only; this shadow is how `poll`
    /// notices.
    static LAYOUT_PREFS_SHADOW: std::cell::Cell<(bool, bool)> =
        const { std::cell::Cell::new((false, true)) };
}

/// How often `poll` re-wakes the event loop while live MCP
/// control is enabled (Wave 11). Rune's desktop loop sleeps
/// in `ControlFlow::Wait` whenever nothing is animating and
/// no wake deadline exists (crates/rune/src/window.rs,
/// `about_to_wait`), and `poll` runs only at the start of a
/// frame — so without a self-scheduled wake, a request the
/// listener thread has queued sits undrained until the
/// user's next input event, and its client waits with it
/// (the bridge's reply timeout is 600 s). While the bridge
/// is on, `poll` therefore always leaves a wake this far
/// out: queued requests drain within one heartbeat of
/// arriving, however idle the window is. The wake is one
/// more deadline among the ones `Tx::wake_after` already
/// collects — the earliest always wins — so it can never
/// postpone the playback (1/fps) or toast (500 ms) wakes;
/// it only bounds the silence when nothing else is
/// scheduled. The cost is one cheap `poll` per heartbeat
/// that finds an empty queue and returns false (no view
/// rebuild). The framework's designed path — `AppSpec::setup`'s
/// `Waker` handed to the listener thread — LANDED in
/// Wave 11 (see `live_mcp::set_waker` and the `setup` hook
/// in `main`): the listener now wakes the loop the moment a
/// request lands. The heartbeat stays anyway, deliberately:
/// it is cheap, proven, and bounds the damage if the waker
/// path ever regresses. Retiring it is a later decision.
const MCP_POLL_HEARTBEAT: Duration = Duration::from_millis(100);

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
/// recovery snapshot has not captured. Since Wave 10 the
/// swap paths themselves restart the counters
/// (`EditorCore::note_project_swapped`, called by the
/// project-tools and Home swap actions, the core's own
/// load paths and the MCP io tools), so a confirmed swap
/// reads clean immediately rather than staying dirty
/// until the next autosave — the paragraph that used to
/// sit here described the pre-Wave-10 behaviour. The
/// counter's remaining limit is the core's: it counts
/// snapshot-taking ops only, so an edit path that
/// mutates without a snapshot is invisible to it — a
/// limit GUI and MCP edits share alike.
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
    /// Sprite instances panel (Wave 13; Animate workspace).
    pub sprites_panel: rune_sec::sprites_panel::SpritesPanelState,
    pub home_panel: rune_sec::home_panel::HomePanelState,
    pub frame_ops: rune_sec::frame_ops::FrameOpsState,
    pub graph_panel: rune_sec::graph_panel::GraphPanelState,
    pub audio_panel: rune_sec::audio_panel::AudioPanelState,
    pub subtitle_panel: rune_sec::subtitle_panel::SubtitlePanelState,
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
    /// Selected subtitle cue, index into
    /// `project.subtitles` — written by the timeline's
    /// subtitle lane (`subtitle_lane::press_cue`), read
    /// by the Subtitles panel (P31). The core has no
    /// subtitle selection state (subtitle_lane section,
    /// Wave P).
    pub subtitle_selected: Option<usize>,
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
    /// `show_left` is the figures sidebar: its persisted
    /// mirror is `LayoutPrefs::sidebar_visible` — seeded
    /// from the prefs in `init`, written back by `poll`
    /// (Focused Wave, D1). `show_right` (the inspector
    /// sidebar) has no persisted mirror and never did.
    pub show_left: bool,
    pub show_right: bool,
    /// Graph-key clipboard (keyframe_tools section):
    /// the copied keys tagged with their source channel
    /// (Wave AC, AC1 — see `KeyClipboard` there).
    pub key_clipboard: Option<rune_sec::keyframe_tools::KeyClipboard>,
    /// Keyframe tools follow-the-graph-channel mode
    /// (keyframe_tools section, Wave AC, AC1): when on,
    /// the section's single-target ops act on the graph
    /// panel's drawn channel instead of the selected
    /// joint's angle channel. Session-only, default off.
    pub key_follow_graph: bool,
    /// Last export recap line (export_panel section).
    pub export_recap: String,
    /// The export panel's running background export job,
    /// if any (export_panel section, Wave W W1): the job's
    /// worker thread reports the two phases' progress into
    /// shared slots and its outcome over a channel; `poll`
    /// drains it (see `export_panel::poll_export_job`).
    pub export_job: Option<rune_sec::export_panel::ExportJob>,
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
    // which defaults to off — live control starts at launch
    // only when the user has enabled it in Settings and the
    // choice persisted). A failed start — in practice
    // another editor instance already holding the port —
    // is not fatal and needs no message here: the bridge
    // simply stays off, and the Settings card's Live MCP
    // toggle reads the bridge's real state, so the user
    // sees it off and can retry once the port is free.
    if layout.live_mcp {
        let on = live_mcp::set_enabled(true);
        // Wave 11 (A4): record how the start went so the
        // Settings card can say so — a failed bind (port in
        // use) otherwise looked identical to "never tried".
        stykanimate_rs::shell::note_live_mcp_start(
            if on {
                stykanimate_rs::shell::LiveMcpStartOutcome::Listening
            } else {
                stykanimate_rs::shell::LiveMcpStartOutcome::FailedToStart
            },
        );
    }
    // Focused Wave (D1): the two persisted view toggles
    // seed their live State mirrors from the same prefs
    // (timeline_collapsed -> frame_ops below,
    // sidebar_visible -> show_left below), and the poll
    // shadow starts at the persisted pair so the first
    // poll does not write the file back unchanged.
    LAYOUT_PREFS_SHADOW
        .with(|c| c.set((layout.timeline_collapsed, layout.sidebar_visible)));
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
        sprites_panel: Default::default(),
        home_panel: rune_sec::home_panel::initial_state(),
        frame_ops: rune_sec::frame_ops::FrameOpsState {
            // Focused Wave (D1): seeded from the
            // persisted prefs; `poll` writes changes
            // back (see LAYOUT_PREFS_SHADOW).
            timeline_collapsed: layout.timeline_collapsed,
            ..Default::default()
        },
        graph_panel: Default::default(),
        audio_panel: rune_sec::audio_panel::AudioPanelState::new(),
        subtitle_panel: rune_sec::subtitle_panel::SubtitlePanelState::new(),
        menus: Default::default(),
        palette: rune_sec::command_palette::PaletteState::new(),
        settings: Default::default(),
        toasts: rune_sec::toasts::ToastsState::new(),
        text_tool: rune_sec::text_tool::TextToolState::new(),
        name_edit: TextEdit::new(name),
        export_use_range: false,
        fx_selected: None,
        subtitle_selected: None,
        pose_clipboard: None,
        workspace: rune_sec::workspaces::from_prefs(&layout),
        guides: false,
        // Focused Wave (D1): the figures sidebar starts
        // where the persisted prefs left it; `poll`
        // writes View-menu / palette flips back.
        show_left: layout.sidebar_visible,
        show_right: true,
        key_clipboard: None,
        key_follow_graph: false,
        export_recap: String::new(),
        export_job: None,
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
                // starts on a joint and reports the miss).
                // Sprites composite OVER the figures and
                // their strokes (media_items' order), so a
                // sprite under the point is picked before
                // the stroke beneath it (Wave X, X5 —
                // SpriteLoop GAP P-3's stage click-to-
                // select): the pick writes the Sprites
                // panel's selection, which the stage's
                // sprite pass already draws corner marks
                // for, and clears the stroke pick the
                // sprite was covering. The press starts
                // no drag — the same gesture shape as the
                // stroke pick below.
                if let Some(sprite) = stage_sprites::sprite_pick_at(core, p) {
                    s.sprites_panel.selected = Some(sprite);
                    s.stroke_selected = None;
                    // Wave Z (Z3): the press that selects
                    // also arms a drag on the sprite — a
                    // Move from here moves it (writes land
                    // through the sprite-key op family at
                    // release; see stage_sprites' Dragging
                    // docs). Arming is refused while playing,
                    // inside sprite_drag_down; the pick
                    // above stands either way.
                    stage_sprites::sprite_drag_down(core, sprite, p);
                } else {
                    // Pick the nearest stroke instead —
                    // the selection the stage highlight
                    // draws and the stroke panel steps
                    // (Phase G). A clean miss clears the
                    // selection.
                    s.stroke_selected = rune_sec::stage_layers::stroke_at_point(
                        core,
                        p,
                        rune_sec::stage_layers::PICK_TOLERANCE,
                    );
                }
            }
        }
        Phase::Move => {
            if core.draw_mode {
                core.draw_to(p);
            } else if !stage_sprites::sprite_drag_move(core, p) {
                core.drag_to(p);
            }
        }
        Phase::Up => {
            // Wave Z (Z3): settle a sprite drag first.
            // It never set `core.dragging` or draw mode,
            // so the cancel guard below neither sees it
            // nor undoes for it — its own settle commits
            // through the sprite-key op (one undo step)
            // or restores the pre-drag keys on cancel.
            // No record: a no-op, and the release belongs
            // to the pose/camera/draw paths below.
            stage_sprites::sprite_drag_up(core, p, d.cancelled());
            // Wave Q (Q2; P6's filed observation): a
            // cancelled gesture — Escape, focus loss, the
            // pointer taken away (`Drag::cancelled`, whose
            // own framework doc says "put back whatever
            // the drag moved") — rolls back like the graph
            // canvas's `drag_up`. Every stage gesture
            // takes exactly one snapshot at Down
            // (`begin_drag` on a hit, `begin_camera_drag`,
            // `begin_draw`), so one `undo()` after the
            // gesture-end call restores the pre-gesture
            // project; the end call runs first so the
            // drag/draw state clears (its recap is
            // superseded by the undo's "Undone.").
            // `gesture_live` is captured before the end
            // call clears the state it reads, and guards
            // the one Down that takes no snapshot — a
            // pose press that missed every joint and only
            // picked a stroke — so a cancelled miss never
            // pops an unrelated edit.
            let gesture_live = core.dragging.is_some() || core.draw_mode;
            if core.draw_mode {
                core.end_draw();
            } else {
                core.end_drag();
            }
            if d.cancelled() && gesture_live {
                core.undo();
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
/// `view` itself uses) — then resolve the pick at the
/// stage point through [`secondary_pick_stack`].
fn stage_secondary(s: &mut State, e: PointerEvent) {
    let size = STAGE_EL_SIZE.with(|c| c.get());
    let size = if size[0] > 0.0 && size[1] > 0.0 {
        size
    } else {
        s.core.project.stage
    };
    let rt = stage_view::drag_transform(&s.core, size);
    let p = rt.to_stage([e.x, e.y]);
    secondary_pick_stack(s, p);
}

/// The right-press pick stack at stage point `p` (Wave
/// AC, AC6): the selection half of the left press's
/// composite in `stage_drag`'s Down phase, resolved in
/// the same order with the same selection writes —
/// core figure/joint first (`stage_view::secondary_
/// select`, whose charter is untouched: it still mirrors
/// `begin_drag`'s core writes only, no more, no less),
/// then the sprite over the point, then the stroke
/// under it. Selection only, by the seat's charter: no
/// snapshot is taken, no drag starts, and the sprite
/// leg deliberately does NOT arm
/// `stage_sprites::sprite_drag_down` — a right-press
/// picks, it never drags.
///
/// The fall-through runs whenever the core pick did
/// not land — a joint miss, or a locked figure's joint
/// (whose status `secondary_select` has already set,
/// exactly as the left press falls through a locked
/// `begin_drag` to the same legs) — with one exception:
/// while playing, `secondary_select`'s standing rule
/// for this path is that nothing changes, so the shell
/// legs stay inert too rather than repealing that rule
/// for the composite path as a whole.
fn secondary_pick_stack(s: &mut State, p: [f32; 2]) {
    if stage_view::secondary_select(&mut s.core, p) {
        return;
    }
    if s.core.playing {
        return;
    }
    if let Some(sprite) = stage_sprites::sprite_pick_at(&s.core, p) {
        // The same two writes the left press makes on
        // a sprite pick: the sprite composites over the
        // stroke it covers, so the stroke pick goes.
        s.sprites_panel.selected = Some(sprite);
        s.stroke_selected = None;
    } else {
        // The left press's stroke leg, verbatim: the
        // stroke pick becomes whatever stroke is at the
        // point — a clean miss clears the stroke
        // selection, and nothing else (figure and
        // sprite selections are not this leg's to
        // clear, on either press).
        s.stroke_selected = rune_sec::stage_layers::stroke_at_point(
            &s.core,
            p,
            rune_sec::stage_layers::PICK_TOLERANCE,
        );
    }
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
/// `live_mcp`). While live control is enabled the loop also
/// re-wakes on [`MCP_POLL_HEARTBEAT`] so a queued request
/// never waits for unrelated user input, and a status a
/// drained call wrote is toasted with the toasts module's
/// `MCP: ` attribution prefix — stamped here, at the push
/// site only, never written into `core.status` itself (the
/// status text is also an MCP API return value; see
/// `rune_sec::toasts` module docs, Wave 11). Playback:
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
    // Export job (Wave W, W1): poll the export panel's
    // background job on this same tick. A job that finished
    // writes its status/recap here — before the live-MCP
    // drain below, so the drain's status-attribution
    // snapshot never mistakes a job's completion line for
    // a client's (it reads as a status written earlier in
    // the tick, the autosave case) — and the status check
    // further down toasts it in the tick it lands. A job
    // still running re-wakes the loop at the job cadence
    // so its progress readout keeps repainting while the
    // window is otherwise idle.
    let (job_redraw, job_active) = rune_sec::export_panel::poll_export_job(s);
    if job_redraw {
        redraw = true;
    }
    if job_active {
        tx.wake_after(rune_sec::export_panel::JOB_WAKE);
    }
    // Live MCP control: requests queued by the bridge's
    // listener thread run HERE, on the UI thread, against
    // the live core — so the stage, timeline and panels
    // show the client's edits as they land (the `live_mcp`
    // module docs carry the synchronisation contract).
    // Drained before the status check below so a tool's
    // status line toasts in the same tick it was written,
    // and capped per tick so a request burst cannot stall
    // the frame; the queue keeps the rest for the next one.
    //
    // Wave 11 (A3): two additions. (1) The heartbeat wake —
    // see `MCP_POLL_HEARTBEAT`: while the bridge is on,
    // this poll always schedules the next one, so a queued
    // request drains even when the window is otherwise
    // idle. (2) Attribution: the status is snapshotted
    // before the drain and compared after it, so the
    // status check below can tell a status the DRAIN wrote
    // (stamp the toast with the MCP prefix) from one
    // written earlier in this same tick by the autosave
    // check above (no stamp — it was not the client's
    // doing). The comparison is drain-window-exact for the
    // same reason: nothing else in `poll` writes
    // `core.status` after this block.
    let mut mcp_status_changed = false;
    if live_mcp::is_enabled() {
        let status_before_drain = s.core.status.clone();
        // Wave 11 (E9): watch the core's project-swap counter
        // across the drain — an MCP load/import replaces the
        // project under the shell, and shell-side selection
        // state (a selected stroke/particle/FX row, an armed
        // destructive confirm) would otherwise point into a
        // project that no longer exists.
        let swaps_before_drain = s.core.project_swaps();
        let drained = MCP_ROUTER.with(|router| {
            live_mcp::drain(&mut s.core, &mut router.borrow_mut(), 8)
        });
        if drained > 0 {
            redraw = true;
            mcp_status_changed = s.core.status != status_before_drain;
            if s.core.project_swaps() != swaps_before_drain {
                s.stroke_selected = None;
                s.particle_selected = None;
                s.fx_selected = None;
                s.subtitle_selected = None;
                s.subtitle_panel.text_target = None;
                s.confirm_action = None;
            }
            // Wave 12 (E7): refresh the core's cached project
            // list through B3's seam (`projects::
            // refresh_project_files`) once per drained batch.
            // The GUI's save paths refresh the list inside the
            // core op (or insert the new row at once, for a
            // copy), but the MCP file writers could not — the
            // Wave 11 §6.6 remainder: an MCP save, save-a-copy
            // or duplicate that created a file left
            // `project_files` stale, so the selector and
            // Home's current-project marking missed the file.
            // This block is the one choke point every live
            // file write passes through, and the refresh is
            // deliberately NOT gated on the swap counter
            // alone: save-a-copy and duplicate write files
            // without bumping it. The seam preserves the
            // selection by file identity and is a no-op when
            // ./projects is missing, so an unrelated
            // mutation's drain can only add rows for files
            // that genuinely exist on disk.
            stykanimate_rs::projects::refresh_project_files(&mut s.core);
            // Errors the drain buffered (protocol errors and
            // refusals that never wrote the status line) toast
            // here with the same MCP attribution; errors that
            // DID write the status are already on the status
            // path below and are not repeated.
            for err in live_mcp::take_drain_errors() {
                s.toasts.push_status(
                    format!("{}{}", rune_sec::toasts::MCP_STATUS_PREFIX, err),
                    now,
                );
            }
        }
        tx.wake_after(MCP_POLL_HEARTBEAT);
    }
    // Wave 14 (shell control): drain the shell queue the
    // live drain filled (shell calls intercepted by
    // live_mcp's peek). This runs whether or not the
    // bridge is currently enabled: requests only arrive
    // through the bridge, but if the bridge was switched
    // off with shell calls still queued, those requests
    // are cancelled by then and the drain discards them
    // here instead of leaving them to execute late after
    // a re-enable. Executed calls answer through their
    // own reply channels; a productive drain redraws.
    if shell_control::drain(s) > 0 {
        redraw = true;
    }
    if s.core.status != s.last_status {
        s.last_status = s.core.status.clone();
        if !s.last_status.is_empty() {
            if mcp_status_changed {
                s.toasts.push_status(
                    format!(
                        "{}{}",
                        rune_sec::toasts::MCP_STATUS_PREFIX,
                        s.last_status
                    ),
                    now,
                );
            } else {
                s.toasts.push_status(s.last_status.clone(), now);
            }
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
    // Focused Wave (D1): the two persisted view toggles
    // ride the same shadow-compare discipline — the
    // timeline's collapsed flag (frame_ops, mirrored by
    // LayoutPrefs::timeline_collapsed) and the figures
    // sidebar's visibility (show_left, mirrored by
    // LayoutPrefs::sidebar_visible). Every writer — the
    // View menu, the palette's panel toggles, the shell
    // layout tools through the drain above, which runs
    // earlier in this same poll — mutates State only;
    // when the live pair diverges from the shadow,
    // load-modify-save BOTH fields so the prefs file
    // carries the pair the shell is actually showing,
    // preserving every other field.
    let layout_pair = (s.frame_ops.timeline_collapsed, s.show_left);
    if LAYOUT_PREFS_SHADOW.with(|c| c.get()) != layout_pair {
        LAYOUT_PREFS_SHADOW.with(|c| c.set(layout_pair));
        let mut prefs = stykanimate_rs::shell::load_layout();
        prefs.timeline_collapsed = layout_pair.0;
        prefs.sidebar_visible = layout_pair.1;
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
        || s.subtitle_panel.path.focused
        || (s.subtitle_panel.text_target.is_some() && s.subtitle_panel.text.focused)
        || (s.subtitle_panel.text_target.is_some() && s.subtitle_panel.speaker.focused)
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
        // Wave P (P9): the Home rename session's field —
        // the one live TextEdit this list missed when the
        // session landed (UI mission Wave 4), so typing a
        // project name there fell through to the global
        // bindings (Space toggled playback, N added a
        // frame, Delete deleted a segment mid-rename).
        // Counted only while the session is armed:
        // `cancel_rename` stands the session down without
        // resetting the buffer, so the bare focus flag can
        // outlive the field's last render, and a bare-flag
        // check would leave every shortcut dead afterwards.
        || (s.home_rename.armed.is_some() && s.home_rename.edit.focused)
}

/// App shortcuts. The command palette gets first claim
/// (Ctrl+K opens it; arrows/Enter/Escape drive it while
/// open). Escape then cancels the topmost remaining scope
/// (an open menu, an armed destructive confirm, Home's
/// armed delete confirm — see the tier block in the body).
/// While a text field is focused, everything else is
/// left to the field. Then: Ctrl+Z undo, Ctrl+Shift+Z /
/// Ctrl+Y redo, Ctrl+S quick-save (the app-bar Save
/// button's press, nothing more), Space play/pause,
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
    // Escape discipline (Phase O charter rule 8; Wave P,
    // P9): Escape cancels the topmost open scope and never
    // falls through to the stage. The palette claimed its
    // Escape above — it sits above every scope here. The
    // remaining tiers, topmost first: an open menu; an
    // armed destructive confirm (the shell-wide
    // ConfirmAction gate); Home's armed per-row delete
    // confirm. Both confirm scopes are press-again gates
    // that had no keyboard exit at all before this. The
    // tiers run *before* the text-field bail on purpose:
    // a menu opened by mouse over a still-flagged field is
    // the topmost scope, and its Escape used to die on the
    // bail and leave the menu open. Disarming is silent,
    // matching the poll loop's project-swap clear of the
    // same state. With no scope open, Escape falls through
    // untouched — a focused field's own Escape (the
    // framework blurs the field) or nothing at all.
    if matches!(ev.key, Key::Escape) {
        if s.menus.open.is_some() {
            s.menus.close();
            return true;
        }
        if s.confirm_action.is_some() {
            s.confirm_action = None;
            return true;
        }
        if s.home_panel.confirm_delete.is_some() {
            rune_sec::home_panel::cancel_delete(&mut s.home_panel);
            return true;
        }
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
        // Wave 10 (C6): the stroke selection never
        // survives history navigation (see chrome_view's
        // undo button for the hazard).
        s.stroke_selected = None;
        return true;
    }
    if ev.is_command('y') {
        s.core.redo();
        s.stroke_selected = None;
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
        // No Escape arm here: the tier block at the top of
        // this function owns Escape (Wave P, P9).
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
                            &rune_sec::i18n_wire::tr(core, "waveform_title"),
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
    if vis(Sec::Sprites) {
        right = right.child(rune_sec::sprites_panel::sprites_panel(s, &t));
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
                            sprite_selected: s.sprites_panel.selected,
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
            &rune_sec::i18n_wire::tr(core, "splitter_resize_left"),
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
            &rune_sec::i18n_wire::tr(core, "splitter_resize_right"),
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

/// Wave P (P19): the window-exit flush. The Rune framework
/// offers no close veto (its window close runs the hooks,
/// then closes unconditionally — wave-p/findings/p19.md),
/// so a dirty exit cannot be confirmed or refused;
/// fail-closed here means the exit loses nothing: any
/// counted edit the 25-edit autosave cadence has not yet
/// captured is flushed to the recovery snapshot the
/// startup offer reads. Registered on both exit hooks —
/// `on_close` (window close) and `on_quit` (OS quit /
/// session end) — and the flush is idempotent, so the
/// second hook to run writes nothing.
fn flush_on_exit(s: &mut State, _tx: &mut Tx) {
    s.core.flush_recovery_on_close();
}

fn main() {
    rune::App::new(AppSpec::<State> {
        min_size: Some((1280.0, 720.0)),
        scenes: Box::leak(rune_sec::scenes::scenes().into_boxed_slice()),
        poll: Some(poll),
        on_key: Some(on_key),
        // Wave 11 (A3 request A): the listener thread wakes
        // the event loop when a live-MCP request lands, so a
        // queued request drains promptly even in an idle
        // window. The 100 ms poll heartbeat stays as the
        // proven floor; the waker is additive.
        setup: Some(|_: &mut State, waker| live_mcp::set_waker(waker)),
        ..AppSpec::new("StykAnimate RS", (1440.0, 900.0), init, view)
    })
    .on_close(flush_on_exit)
    .on_quit(flush_on_exit)
    .run();
}

/// Wave P (P9) — key-routing pins: the Escape tier order,
/// the text-field scoping of the global bindings, and the
/// zero-key-loss census of every binding `on_key` owns.
/// The census test presses each documented binding once;
/// Ctrl+S is the one exception — its arm runs
/// `core.save()`, and the suite's no-pollution rule bars
/// writing `./projects` from a test, so that arm is
/// verified by the unchanged code path (and by the help
/// panel / palette hint drift guards), not by a press.
#[cfg(test)]
mod key_routing_tests {
    use super::{init, on_key, text_focused, ConfirmAction, State};
    use crate::rune_sec;
    use rune::{Key, KeyEvent, Mods, Tx};
    use std::path::PathBuf;

    fn plain(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods::default(),
        }
    }

    fn typed(text: &str) -> KeyEvent {
        plain(Key::Text(text.to_string()))
    }

    fn command(c: char) -> KeyEvent {
        KeyEvent {
            key: Key::Command(c),
            mods: Mods {
                shift: false,
                cmd: true,
                alt: false,
                ctrl: true,
                logo: false,
            },
        }
    }

    fn press(s: &mut State, ev: &KeyEvent) -> bool {
        let mut tx = Tx::default();
        on_key(s, ev, &mut tx)
    }

    #[test]
    fn esc_closes_the_menu_even_while_a_field_is_focused() {
        // The regression: the menu's Escape used to sit
        // below the text-field bail, so a menu opened by
        // mouse over a still-flagged field could not be
        // Escaped out of.
        let mut s = init();
        s.menus.open = Some(rune_sec::menus::FILE);
        s.name_edit.focused = true;
        assert!(press(&mut s, &plain(Key::Escape)), "Escape is consumed");
        assert_eq!(s.menus.open, None, "the topmost scope closed");
    }

    #[test]
    fn esc_closes_the_menu_with_no_field_focused() {
        let mut s = init();
        s.menus.open = Some(rune_sec::menus::FILE);
        assert!(press(&mut s, &plain(Key::Escape)));
        assert_eq!(s.menus.open, None);
    }

    #[test]
    fn esc_disarms_an_armed_confirm_without_running_it() {
        let mut s = init();
        s.core.project.name = "Unsaved Work".into();
        s.confirm_action = Some(ConfirmAction::NewProject);
        assert!(press(&mut s, &plain(Key::Escape)));
        assert_eq!(s.confirm_action, None, "Escape disarms the gate");
        assert_eq!(
            s.core.project.name, "Unsaved Work",
            "the gated op did not run"
        );
    }

    #[test]
    fn esc_disarms_homes_armed_delete_confirm() {
        let mut s = init();
        s.home_panel.confirm_delete = Some(PathBuf::from("projects/demo.json"));
        assert!(press(&mut s, &plain(Key::Escape)));
        assert_eq!(s.home_panel.confirm_delete, None);
    }

    #[test]
    fn esc_takes_the_topmost_scope_first() {
        let mut s = init();
        s.menus.open = Some(rune_sec::menus::FILE);
        s.confirm_action = Some(ConfirmAction::OpenProject);
        assert!(press(&mut s, &plain(Key::Escape)));
        assert_eq!(s.menus.open, None, "menu (topmost) closed first");
        assert_eq!(
            s.confirm_action,
            Some(ConfirmAction::OpenProject),
            "the confirm stays armed"
        );
        assert!(press(&mut s, &plain(Key::Escape)));
        assert_eq!(s.confirm_action, None, "second Escape reaches it");
    }

    #[test]
    fn esc_with_no_scope_open_falls_through() {
        let mut s = init();
        assert!(
            !press(&mut s, &plain(Key::Escape)),
            "nothing claims it — the field/framework layer owns Escape"
        );
    }

    #[test]
    fn home_rename_typing_never_reaches_the_global_bindings() {
        // The leak this wave fixes: the Home rename field
        // was missing from `text_focused`, so its typing
        // drove the stage bindings.
        let mut s = init();
        rune_sec::home_panel::begin_rename(
            &mut s.home_rename,
            PathBuf::from("projects/demo.json"),
            "Demo",
        );
        assert!(text_focused(&s), "the armed, focused field scopes keys");
        let frames = s.core.frame_count();
        let playing = s.core.playing;
        assert!(!press(&mut s, &typed("n")), "N belongs to the field");
        assert!(!press(&mut s, &typed(" ")), "Space belongs to the field");
        assert!(
            !press(&mut s, &plain(Key::Delete)),
            "Delete edits text, not the segment"
        );
        assert!(
            !press(&mut s, &command('z')),
            "Ctrl+Z belongs to the field, not undo"
        );
        assert_eq!(s.core.frame_count(), frames, "no frame was added");
        assert_eq!(s.core.playing, playing, "playback did not toggle");
    }

    #[test]
    fn home_rename_stale_focus_flag_alone_scopes_nothing() {
        // `cancel_rename` leaves the buffer's focus flag
        // set after the session stands down; unarmed, that
        // stale flag must not scope keys (the conjunction
        // in `text_focused` is what stands between this
        // wart and a dead keyboard).
        let mut s = init();
        s.home_rename.edit.focused = true;
        assert!(!text_focused(&s));
        let frames = s.core.frame_count();
        assert!(press(&mut s, &typed("n")), "global bindings are live");
        assert_eq!(s.core.frame_count(), frames + 1);
    }

    #[test]
    fn binding_census_every_documented_binding_still_fires() {
        let mut s = init();
        // Ctrl+K opens the palette; Escape closes it.
        assert!(press(&mut s, &command('k')));
        assert!(s.palette.open);
        assert!(press(&mut s, &plain(Key::Escape)));
        assert!(!s.palette.open);
        // Alt+2 switches to the Draw workspace.
        let alt2 = KeyEvent {
            key: Key::Text("2".into()),
            mods: Mods {
                shift: false,
                cmd: false,
                alt: true,
                ctrl: false,
                logo: false,
            },
        };
        assert!(press(&mut s, &alt2));
        assert_eq!(s.workspace, rune_sec::workspaces::Workspace::Draw);
        // N adds a frame; Ctrl+Z undoes it; Ctrl+Shift+Z
        // and Ctrl+Y redo it.
        let frames = s.core.frame_count();
        assert!(press(&mut s, &typed("n")));
        assert_eq!(s.core.frame_count(), frames + 1);
        assert!(press(&mut s, &command('z')));
        assert_eq!(s.core.frame_count(), frames);
        let shift_z = KeyEvent {
            key: Key::Command('z'),
            mods: Mods {
                shift: true,
                cmd: true,
                alt: false,
                ctrl: true,
                logo: false,
            },
        };
        assert!(press(&mut s, &shift_z));
        assert_eq!(s.core.frame_count(), frames + 1);
        assert!(press(&mut s, &command('z')));
        assert!(press(&mut s, &command('y')));
        assert_eq!(s.core.frame_count(), frames + 1);
        // Right steps the playhead (wrapping); Left back.
        let n = s.core.frame_count() as i32;
        let cur = s.core.current as i32;
        assert!(press(&mut s, &plain(Key::Right)));
        assert_eq!(s.core.current as i32, (cur + 1).rem_euclid(n));
        assert!(press(&mut s, &plain(Key::Left)));
        assert_eq!(s.core.current as i32, cur);
        // Space toggles playback; C camera mode; O onion.
        let playing = s.core.playing;
        assert!(press(&mut s, &typed(" ")));
        assert_eq!(s.core.playing, !playing);
        let cam = s.core.camera_mode;
        assert!(press(&mut s, &typed("c")));
        assert_eq!(s.core.camera_mode, !cam);
        let onion = s.core.onion;
        assert!(press(&mut s, &typed("o")));
        assert_eq!(s.core.onion, !onion);
        // I inserts an inbetween (playhead is at frame 0
        // of a multi-frame project here, so it lands).
        let frames = s.core.frame_count();
        assert!(press(&mut s, &typed("i")));
        assert_eq!(s.core.frame_count(), frames + 1);
        // Delete is claimed (the op no-ops safely — status
        // only — when nothing deletable is selected).
        assert!(press(&mut s, &plain(Key::Delete)));
    }
}

/// Wave V (V4) — shell-chrome i18n pin: the three labels
/// `view` renders directly in this file (the waveform
/// card's title, the two column splitters' help labels)
/// resolve through the catalogue under the keys IA-V4-1
/// files. The pin passes only after the integrator
/// inserts IA-V4-1 into `src/i18n.rs` (the Q7 pattern) —
/// until then `tr` falls back to the key text and this
/// test is red by design; the wiring and the keys land
/// in the same integration push, and with the keys in
/// the English rendering is byte-identical to the
/// literals it replaced.
#[cfg(test)]
mod chrome_i18n_tests {
    use crate::rune_sec::i18n_wire;
    use stykanimate_rs::editor::EditorCore;

    #[test]
    fn chrome_i18n_keys_exist_and_match_the_english_labels() {
        let core = EditorCore::new();
        for (key, english) in [
            ("waveform_title", "Waveform"),
            (
                "splitter_resize_left",
                "Drag to resize the left panel column",
            ),
            (
                "splitter_resize_right",
                "Drag to resize the right panel column",
            ),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }
}

#[cfg(test)]
mod secondary_pick_tests {
    //! Wave AC (AC6): the right-press pick stack
    //! (`secondary_pick_stack`) — the left press's
    //! selection composite resolved without any drag.
    //! The core leg's own four cases stay pinned in
    //! stage_view's tests; these pin the shell legs,
    //! their order, and the selection-only charter.

    use super::{init, secondary_pick_stack, stage_sprites, State};
    use stykanimate_rs::model::{ImageAsset, InstanceKey, SpriteInstance, Stroke};

    /// A real 4x2 PNG (the stage_sprites fixture's
    /// idiom): sprite quads derive from the decoded
    /// asset, so a dummy byte string would exercise
    /// only the failure path.
    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 200, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("encoding a 4x2 PNG cannot fail");
        buf
    }

    fn key(x: f32, y: f32) -> InstanceKey {
        InstanceKey {
            frame: 0.0,
            x,
            y,
            scale: 10.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    /// The demo project plus one sprite (the 4x2 asset
    /// at scale 10 → a 40x20 quad centred on (300, 200))
    /// and one stroke (frame 0, along y = 100 from
    /// x = 80 to x = 120) — placed apart from each other
    /// and from the demo figure's joints (rooted at
    /// (480, 470)), so each leg of the stack can be
    /// pressed in isolation.
    fn state_with_sprite_and_stroke() -> State {
        let mut s = init();
        s.core.project.assets.push(ImageAsset {
            name: "badge.png".into(),
            png: tiny_png(),
        });
        s.core.project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![key(300.0, 200.0)],
        });
        s.core.project.strokes.push(Stroke {
            frame: 0,
            points: vec![[80.0, 100.0], [120.0, 100.0]],
            color: 0x111111,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        s
    }

    /// Move the fixture's sprite so its quad centres
    /// exactly on figure 0's root joint at frame 0, and
    /// return that point.
    fn cover_first_joint(s: &mut State) -> [f32; 2] {
        let jp = s.core.positions(0, 0)[0];
        s.core.project.sprites[0].keys[0].x = jp[0];
        s.core.project.sprites[0].keys[0].y = jp[1];
        jp
    }

    #[test]
    fn right_press_on_a_sprite_selects_it_and_clears_the_stroke_pick() {
        let mut s = state_with_sprite_and_stroke();
        s.stroke_selected = Some(0);
        secondary_pick_stack(&mut s, [300.0, 200.0]);
        assert_eq!(s.sprites_panel.selected, Some(0), "the sprite is picked");
        assert_eq!(
            s.stroke_selected, None,
            "the sprite composites over the stroke pick it covers"
        );
        // Selection only: no drag of either family is
        // armed and no undo step is spent.
        assert!(s.core.dragging.is_none(), "no pose drag starts");
        assert!(
            !stage_sprites::sprite_drag_active(),
            "no sprite drag is armed"
        );
        assert_eq!(s.core.undo_depth(), 0, "picking never snapshots");
    }

    #[test]
    fn right_press_on_a_stroke_picks_it() {
        let mut s = state_with_sprite_and_stroke();
        secondary_pick_stack(&mut s, [100.0, 100.0]);
        assert_eq!(
            s.stroke_selected,
            Some(0),
            "the stroke under the point is picked"
        );
        assert_eq!(s.sprites_panel.selected, None, "no sprite was at the point");
        assert_eq!(s.core.undo_depth(), 0);
    }

    #[test]
    fn right_press_clean_miss_clears_only_the_stroke_pick() {
        let mut s = state_with_sprite_and_stroke();
        s.stroke_selected = Some(0);
        s.sprites_panel.selected = Some(0);
        let node_before = s.core.selected_node;
        let figure_before = s.core.selected_figure;
        secondary_pick_stack(&mut s, [10.0, 10.0]);
        assert_eq!(
            s.stroke_selected, None,
            "the left press's miss rule: the stroke pick clears"
        );
        assert_eq!(
            s.sprites_panel.selected,
            Some(0),
            "the sprite selection is not the stroke leg's to clear"
        );
        assert_eq!(
            s.core.selected_node, node_before,
            "the core selection is untouched on a miss"
        );
        assert_eq!(s.core.selected_figure, figure_before);
        assert_eq!(s.core.undo_depth(), 0);
    }

    #[test]
    fn right_press_prefers_the_joint_over_the_sprite_covering_it() {
        let mut s = state_with_sprite_and_stroke();
        let jp = cover_first_joint(&mut s);
        secondary_pick_stack(&mut s, jp);
        assert_eq!(
            s.core.selected_node,
            Some(0),
            "the joint wins, as on the left press"
        );
        assert_eq!(s.sprites_panel.selected, None, "the sprite leg never ran");
        assert!(
            s.core.dragging.is_none(),
            "selection only — the drag half of begin_drag stays home"
        );
        assert_eq!(s.core.undo_depth(), 0, "no snapshot was taken");
    }

    #[test]
    fn right_press_while_playing_changes_nothing() {
        let mut s = state_with_sprite_and_stroke();
        s.core.playing = true;
        s.stroke_selected = Some(0);
        let figure_before = s.core.selected_figure;
        let node_before = s.core.selected_node;
        // Over the sprite, over the stroke, and over
        // empty stage: the whole stack stays inert.
        secondary_pick_stack(&mut s, [300.0, 200.0]);
        secondary_pick_stack(&mut s, [100.0, 100.0]);
        secondary_pick_stack(&mut s, [10.0, 10.0]);
        assert_eq!(s.sprites_panel.selected, None, "no sprite pick while playing");
        assert_eq!(
            s.stroke_selected,
            Some(0),
            "the stroke pick is neither made nor cleared"
        );
        assert_eq!(s.core.selected_figure, figure_before);
        assert_eq!(s.core.selected_node, node_before);
        assert_eq!(s.core.undo_depth(), 0);
    }

    #[test]
    fn right_press_on_a_locked_joint_falls_through_to_the_sprite() {
        // The left press falls through a locked
        // begin_drag to the same legs; the stack keeps
        // that parity. The core leg sets the locked
        // status (its own behaviour, pinned in
        // stage_view's tests); the sprite covering the
        // joint is then picked.
        let mut s = state_with_sprite_and_stroke();
        let jp = cover_first_joint(&mut s);
        s.core.project.figures[0].locked = true;
        let node_before = s.core.selected_node;
        secondary_pick_stack(&mut s, jp);
        assert_eq!(
            s.core.status, "Figure is locked — unlock it in the figures list.",
            "the core leg's status stands"
        );
        assert_eq!(
            s.core.selected_node, node_before,
            "the locked figure selects nothing"
        );
        assert_eq!(
            s.sprites_panel.selected,
            Some(0),
            "the covering sprite is picked"
        );
        assert_eq!(s.core.undo_depth(), 0);
    }
}
