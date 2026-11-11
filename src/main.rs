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
mod stage_gizmo;

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
    /// The layout-prefs shadow for the persisted view
    /// state `poll` writes back (Focused Wave, D1;
    /// Roadmap Wave 1, RM13 added the timeline zoom
    /// pair): `(timeline_collapsed, sidebar_visible,
    /// timeline_zoom, timeline_follow)` as last
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
    static LAYOUT_PREFS_SHADOW: std::cell::Cell<(bool, bool, f32, bool)> =
        const { std::cell::Cell::new((false, true, rune_sec::timeline_marks::CHIP_W, false)) };

    /// The anchored playback deadline (Roadmap Wave 1,
    /// RM7 / F-6): the absolute shell-clock time of the
    /// next playback wake, stepped by
    /// `playback_controls::playback_deadline` from its own
    /// previous value — never re-based on when the current
    /// poll finishes — so poll work cannot accumulate as
    /// playback drift. A thread-local for the same
    /// State-census reason as the shadows above; `poll`
    /// clears it whenever playback stops, so a fresh Play
    /// press anchors a fresh grid.
    static PLAYBACK_DEADLINE: std::cell::Cell<Option<f64>> =
        const { std::cell::Cell::new(None) };

    /// The language shadow for `poll`'s write-back
    /// (Roadmap F-9): `EditorCore.lang` as last persisted
    /// to `LayoutPrefs.lang`. Same discipline as
    /// [`LAYOUT_PREFS_SHADOW`] — `init` seeds it from the
    /// loaded prefs, `poll` compares the live
    /// `core.lang` against it and load-modify-saves on
    /// divergence — with the shadow in a thread-local for
    /// the same State-census reason. Every language
    /// writer (the Settings card's picker and the shell
    /// settings tools, both through
    /// `settings_popover::set_language`, and the core's
    /// `cycle_lang`) mutates `core.lang` only; this
    /// shadow is how `poll` notices.
    static LANG_PREFS_SHADOW: std::cell::Cell<stykanimate_rs::i18n::Lang> =
        const { std::cell::Cell::new(stykanimate_rs::i18n::Lang::En) };
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
    /// Copy the legacy working-directory stores into the
    /// per-user app data folder (Home's migration banner,
    /// Roadmap Wave 2 W2-65 — the GUI half of IA-RM9-2).
    /// A unit variant like its siblings: the plan itself
    /// lives in `State::migration_panel`, captured at
    /// startup; the arm only records that the banner's
    /// first press happened.
    MigrateLegacyData,
    /// Replace the session with starter template
    /// `templates::TEMPLATES[i]` (Home's template
    /// section, Roadmap Wave 2 W2-91).
    NewFromTemplate(usize),
    /// Replace the session with bundled sample
    /// `samples::SAMPLES[i]` (Home's sample row,
    /// Roadmap Wave 2 W2-92).
    OpenSample(usize),
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
    /// Legacy-store migration offer (migration_prompt
    /// section, Roadmap Wave 2 W2-65): the plan captured
    /// at startup, before any store write, by
    /// `migration_prompt::initial_state`.
    pub migration_panel: rune_sec::migration_prompt::MigrationPanelState,
    pub frame_ops: rune_sec::frame_ops::FrameOpsState,
    pub graph_panel: rune_sec::graph_panel::GraphPanelState,
    /// Camera panel number-field sessions (camera_panel
    /// section, Roadmap Wave 1 F-3).
    pub camera_panel: rune_sec::camera_panel::CameraPanelState,
    /// Segment inspector number-field sessions
    /// (inspector_segment section, Roadmap Wave 1 F-3).
    pub segment_numbers: rune_sec::inspector_segment::SegmentNumberState,
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
    /// Setup-mode value saved on entering the Build
    /// workspace (workspaces section, Wave 2).
    pub setup_mode_before_build: bool,
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
    /// The layout stage focus restores on exit
    /// (Roadmap Wave 2, W2-10): `Some(live layout)`
    /// while stage focus is active, `None` otherwise.
    /// Session-only by A1 R4's rule — never persisted,
    /// so a restart can never restore a focused stage.
    /// Owned by
    /// `rune_sec::workspaces::toggle_stage_focus`.
    pub stage_focus_restore: Option<rune_sec::workspaces::LiveLayout>,
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
    /// The shared key selection (Roadmap Wave 1, F-4):
    /// one selected-key state consumed by the graph
    /// panel, the dopesheet and the timeline marker row
    /// (see `rune_sec::key_selection`). Session-only,
    /// never serialised or snapshotted.
    pub key_sel: rune_sec::key_selection::KeySel,
    /// Keyframe-tools panel state (keyframe_tools section,
    /// Roadmap Wave 2 W2-35): the Copy key offset field's
    /// session + value.
    pub keyframe_tools: rune_sec::keyframe_tools::KeyframeToolsState,
    /// Help panel session state (help_panel section,
    /// Roadmap Wave 1, PL-7): the Help search field.
    pub help_panel: rune_sec::help_panel::HelpPanelState,
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
    /// Golden-ratio guide overlay on/off (Roadmap Wave 2,
    /// W2-33; drawn only when `guides` and `guides_frustum`
    /// are also on). Default OFF — opt-in variant.
    pub guides_golden: bool,
    /// Diagonal guide overlay on/off (W2-33; same gates
    /// and default-off footing as `guides_golden`).
    pub guides_diagonals: bool,
    /// Stored-guides overlay on/off (Roadmap Wave 2,
    /// W2-05; the project's user-placed guide lines —
    /// authored content, drawn whenever this is on,
    /// NOT gated by `guides`).
    pub guides_stored: bool,
    /// Rulers overlay on/off (W2-05; graduated stage
    /// rulers along the stage's top/left edges — the
    /// stored guides' source chrome).
    pub rulers: bool,
    /// Joint-limit arcs on the stage (Roadmap Wave 2,
    /// W2-27): the selected node's limit partition drawn
    /// at its joint. Default OFF — opt-in annotation,
    /// toggled from the segment inspector's limit rows.
    pub limit_arcs: bool,
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
    let mut layout = stykanimate_rs::shell::load_layout();
    // Roadmap Wave 2 (W2-61 — PL-10): seed any missing
    // per-workspace layout records and mirror the
    // STARTING workspace's remembered layout into the
    // globals, so every seed below (the poll shadow,
    // the timeline flag, the sidebar) reads this
    // workspace's own shape. In-memory only — the
    // records reach disk on the first layout save.
    let startup = rune_sec::workspaces::startup_layout(&mut layout);
    core.light_theme = !layout.dark;
    // Language likewise (Roadmap F-9): the persisted
    // `LayoutPrefs.lang` seeds the core's UI language —
    // the `dark` precedent above, in the same place.
    // Before the field existed nothing seeded `lang`,
    // so every relaunch reset the UI to English.
    core.lang = layout.lang;
    LANG_PREFS_SHADOW.with(|c| c.set(layout.lang));
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
    // Roadmap Wave 1 (RM13 — P-1): the zoom pair rides
    // the same shadow; the seeded zoom is the clamped
    // value `FrameOpsState::chip_w` will read, so a
    // hand-edited prefs file cannot make the first
    // poll write the file back.
    let zoom_seed = if layout.timeline_zoom.is_finite() {
        layout.timeline_zoom.clamp(
            rune_sec::timeline_marks::ZOOM_MIN_W,
            rune_sec::timeline_marks::ZOOM_MAX_W,
        )
    } else {
        rune_sec::timeline_marks::CHIP_W
    };
    LAYOUT_PREFS_SHADOW.with(|c| {
        c.set((
            layout.timeline_collapsed,
            layout.sidebar_visible,
            zoom_seed,
            layout.timeline_follow,
        ))
    });
    if let Some(msg) = rune_sec::home_panel::startup_status() {
        core.status = msg;
    } else if let Some(msg) = rune_sec::migration_prompt::startup_status() {
        // Roadmap Wave 2 (W2-65): the migration offer
        // speaks only when the recovery offer did not —
        // unsaved work outranks housekeeping on the one
        // boot status line; both banners still render
        // in Home.
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
        migration_panel: rune_sec::migration_prompt::initial_state(),
        frame_ops: rune_sec::frame_ops::FrameOpsState {
            // Focused Wave (D1): seeded from the
            // persisted prefs; `poll` writes changes
            // back (see LAYOUT_PREFS_SHADOW).
            timeline_collapsed: startup.timeline_collapsed,
            timeline_zoom: zoom_seed,
            timeline_follow: layout.timeline_follow,
            ..Default::default()
        },
        graph_panel: Default::default(),
        camera_panel: Default::default(),
        segment_numbers: Default::default(),
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
        setup_mode_before_build: false,
        guides: false,
        // Focused Wave (D1): the figures sidebar starts
        // where the persisted prefs left it; `poll`
        // writes View-menu / palette flips back.
        show_left: startup.show_left,
        show_right: startup.show_right,
        stage_focus_restore: None,
        key_clipboard: None,
        key_follow_graph: false,
        key_sel: Default::default(),
        keyframe_tools: Default::default(),
        help_panel: Default::default(),
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
        guides_golden: false,
        guides_diagonals: false,
        guides_stored: true,
        limit_arcs: false,
        rulers: false,
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
    // Roadmap Wave 1 (RM15 - P-3, IA-RM15-1): feed the
    // pointer probe from the drag's own point on every
    // phase, so hover and the drag HUD track during
    // drags even where pointer-move does not fire
    // mid-gesture. On Up the probe stays (the pointer
    // is still over the stage); the canvas's
    // hover-change handler clears it on leave.
    stage_view::set_stage_pointer(Some(stage_view::StagePointer {
        local: [d.x, d.y],
        size: [d.w, d.h],
    }));
    match d.phase {
        Phase::Down => {
            if stage_view::navigator::nav_down(core, [d.x, d.y], [d.w, d.h]) {
                // Roadmap Wave 2 (W2-04): the press
                // landed on the navigator minimap
                // (chrome over the canvas's top-left
                // corner): it jumps the view and owns
                // the gesture — never a pose, marquee
                // or draw press, in any mode.
                return;
            }
            if core.draw_mode {
                core.begin_draw(p);
            } else if core.camera_mode {
                core.begin_camera_drag(p);
            } else if stage_view::path_insert_down(core, p, d.clicks >= 2) {
                // Roadmap Wave 2 (W2-36): a double-press on
                // a motion-path segment splits it — one new
                // frame, one undo step, complete on the
                // press. No session follows, so the Move/Up
                // arms need no new term (the path session
                // finds no record). Single presses return
                // false at once and fall through untouched.
                // Precedence: within the path family, before
                // `path_drag_down` (a double-press ON a point
                // is excluded by the segment test's end-2%
                // rule, so point grabs keep their meaning),
                // and before the pending gizmo (IA-W2-03-3)
                // and marquee (IA-W2-01-2) arms — those are
                // single-press drags; a double-press is
                // unambiguous, and its first press already
                // ran (and settled) as an ordinary click.
            } else if stage_view::path_drag_down(core, p) {
                // Roadmap Wave 1 (RM16 - P-4): the press
                // grabbed a motion-path point (overlay on,
                // a non-current frame's root point hit):
                // the session seeks the playhead and
                // previews; the release lands one
                // `nudge_path_point`. In path mode the
                // points win over joint grabs - the current
                // frame's point is never among them (the
                // pose drag owns the current root), and
                // with the overlay off this is a cheap miss.
            } else if stage_gizmo::gizmo_down(core, p, rt.vt.scale) {
                // Roadmap Wave 2 (W2-03): the press
                // grabbed the transform gizmo's move
                // grip or rotate knob — the handles are
                // drawn over the figures precisely so
                // they win their own hit discs. Moves
                // preview only; the release lands one
                // labelled undo step inside `gizmo_up`.
                // (IA-W2-01-2's group arm, when present,
                // goes AFTER this arm: the grip is the
                // deliberate target when a press hits
                // both a grip and a set member's joint;
                // its marquee arm stays in the
                // begin-drag miss chain, untouched.)
            } else if stage_view::group_drag_down(
                core,
                p,
                stage_view::pick_radius(rt.vt.scale),
            ) {
                // Roadmap Wave 2 (W2-01): the press
                // landed on a joint of a figure in a
                // multi-figure marquee set: the SET is
                // grabbed, not the figure. Moves
                // preview only; the release lands one
                // "Move figures" undo step (see
                // stage_view's marquee section).
            } else if !core.begin_drag_with_radius(p, stage_view::pick_radius(rt.vt.scale)) {
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
                if let Some((sprite, handle)) = stage_sprites::sprite_handle_pick_at(
                    core,
                    s.sprites_panel.selected,
                    p,
                ) {
                    // Roadmap Wave 1 (RM22 - P-12,
                    // IA-RM22-3): the selected sprite's
                    // handles are drawn over everything,
                    // so a press on one arms its handle
                    // drag (corner-scale / rotate / pivot)
                    // before any body or segment pick;
                    // the Move/Up phases settle it through
                    // the sprite drag machinery it armed.
                    s.sprites_panel.selected = Some(sprite);
                    stage_sprites::sprite_handle_drag_down(core, sprite, handle, p);
                    return;
                }
                if stage_view::bend_drag_down(
                    core,
                    p,
                    stage_view::pick_radius(rt.vt.scale),
                ) {
                    // Roadmap Wave 2 (W2-08): the press
                    // landed on a segment's middle (or its
                    // arc midpoint) — a bend drag: the
                    // session previews write-free and its
                    // release settles as one "Bend segment"
                    // step (see stage_view's bend section).
                    return;
                }
                if let Some((fi, ni, _)) = stage_view::segment_hit(
                    core,
                    p,
                    stage_view::pick_radius(rt.vt.scale),
                ) {
                    // Roadmap Wave 1 (RM15 - P-3,
                    // IA-RM15-5): the press hit no joint
                    // but landed on a segment's middle -
                    // start the drag the child joint's
                    // grab would have started, through
                    // the core's post-hit door. A locked
                    // figure refuses there (status set)
                    // and the press falls through to the
                    // sprite / stroke picks below, exactly
                    // as a locked joint grab falls through.
                    if core.begin_drag_at(fi, ni, p) {
                        return;
                    }
                }
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
                } else if s.guides_stored
                    && stage_view::guide_down(
                        core,
                        p,
                        rt.vt.scale,
                        if s.rulers {
                            stage_view::RULER_THICKNESS_PX
                        } else {
                            0.0
                        },
                    )
                {
                    // Roadmap Wave 2 (W2-05): the press hit
                    // no content; it landed in a ruler band
                    // or on a stored guide's line, and the
                    // guide session owns the gesture. It
                    // writes nothing until its release
                    // settles through one core stored-guide
                    // op. The `guides_stored` gate keeps a
                    // hidden guide family fully inert.
                } else if let Some(stroke) = rune_sec::stage_layers::stroke_at_point(
                    core,
                    p,
                    stage_view::pick_radius(rt.vt.scale),
                ) {
                    // The press hit a stroke: the
                    // standing pick, unchanged (Phase G).
                    s.stroke_selected = Some(stroke);
                } else {
                    // The press hit NOTHING (Roadmap
                    // Wave 2, W2-01): it clears the
                    // stroke pick — the empty press's
                    // standing write — and starts a
                    // marquee: hold and drag a square
                    // to select every character inside
                    // it; a plain click just clears
                    // the figure set at release.
                    s.stroke_selected = None;
                    stage_view::marquee_down(core, p);
                }
            }
        }
        Phase::Move => {
            // Roadmap Wave 1 (RM15 - P-3, IA-RM15-3): snap
            // the point the drag is about to write (roots
            // to landmarks, aim/IK to the angle step).
            // `snap_drag_point` passes camera, draw, sprite
            // and path points through untouched - none of
            // them set a pose dragging kind. (The press's
            // own first `drag_to` inside `begin_drag`
            // stays unsnapped: the drag settles onto the
            // snap on its first Move, IA-RM15-3's recorded
            // caveat.)
            let (p, _marks) = stage_view::snap_drag_point(core, p, rt.vt.scale);
            if core.draw_mode {
                core.draw_to(p);
            } else if stage_view::navigator::nav_move(core, [d.x, d.y], [d.w, d.h]) {
                // Roadmap Wave 2 (W2-04): a navigator
                // drag owns the gesture; its moves only
                // re-centre the view. (The snap call
                // above passes navigator points
                // through: `snap_drag_point` only
                // rewrites pose-drag points, and the
                // session sets no pose dragging kind.)
            } else if stage_view::path_drag_move(core, p) {
                // Roadmap Wave 1 (RM16 - P-4): a path-point
                // drag owns the gesture; its moves only move
                // the overlay's preview.
            } else if stage_view::guide_move(p) {
                // Roadmap Wave 2 (W2-05): a guide drag
                // owns the gesture; its moves only move
                // the preview. (The snap call above passes
                // guide points through: `snap_drag_point`
                // only rewrites pose-drag points, and the
                // session sets no pose dragging kind.)
            } else if stage_view::marquee_move(p) {
                // Roadmap Wave 2 (W2-01): a marquee
                // drag owns the gesture; its moves
                // only move the rubber band. (The
                // snap call above passes marquee
                // points through: `snap_drag_point`
                // only rewrites pose-drag points, and
                // the sessions set no pose dragging
                // kind.)
            } else if stage_view::group_drag_move(core, p) {
                // Roadmap Wave 2 (W2-01): a group
                // drag owns the gesture; its moves
                // only move the previewed outlines.
            } else if stage_view::bend_drag_move(core, p) {
                // Roadmap Wave 2 (W2-08): a bend drag owns
                // the gesture; its moves only move the
                // preview bend (the settle is the Up's).
            } else if stage_gizmo::gizmo_move(core, p) {
                // Roadmap Wave 2 (W2-03): a gizmo drag
                // owns the gesture; its moves only move
                // the previewed frame. (The snap call
                // above passes gizmo points through:
                // `snap_drag_point` rewrites only
                // pose-drag points, and the gizmo sets
                // no pose dragging kind; rotation snap
                // is the gizmo's own, on the accumulated
                // angle, inside its session.)
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
            // Roadmap Wave 1 (RM16 - P-4): settle a
            // path-point drag beside the sprite settle. It
            // never set `core.dragging` or draw mode, so
            // the cancel guard below neither sees it nor
            // undoes for it - and needs none: its moves
            // never wrote, its cancel just drops the
            // record, and a moved release lands exactly one
            // `nudge_path_point` (one undo step) here,
            // before `end_drag`'s recap runs.
            stage_view::path_drag_up(core, p, d.cancelled());
            // Roadmap Wave 2 (W2-04): settle a
            // navigator drag beside the path settle.
            // It never set `core.dragging` or draw
            // mode, so the cancel guard below neither
            // sees it nor undoes for it — and needs
            // none: its moves wrote only view state,
            // and a cancelled navigator drag simply
            // stops (see the module's docs for why
            // the view is not restored here, unlike
            // the pan's own cancel in IA-W2-04-1's
            // wrapper).
            stage_view::navigator::nav_up(d.cancelled());
            // Roadmap Wave 2 (W2-01): settle the
            // marquee and group sessions beside the
            // path settle. Neither set `core.dragging`
            // or draw mode, so the cancel guard below
            // neither sees them nor undoes for them —
            // and needs none: their moves never
            // wrote, a cancelled gesture drops its
            // record, and a moved group release has
            // already landed its one snapshot inside
            // `group_drag_up` when this runs (its
            // `gesture_live` read stays false for
            // both, exactly as for the path session).
            stage_view::marquee_up(core, p, d.cancelled());
            stage_view::group_drag_up(core, p, d.cancelled());
            // Roadmap Wave 2 (W2-03): settle a gizmo
            // drag beside the path settle. It never set
            // `core.dragging` or draw mode, so the
            // cancel guard below neither sees it nor
            // undoes for it — and needs none: its moves
            // never wrote, a cancelled gesture drops
            // its record, and a settled release has
            // already landed its one snapshot inside
            // `gizmo_up` when this runs (its
            // `gesture_live` read stays false for it,
            // exactly as for the path session).
            stage_gizmo::gizmo_up(core, p, d.cancelled());
            // Roadmap Wave 2 (W2-05): settle a guide
            // drag beside the path settle. It never set
            // `core.dragging` or draw mode, so the cancel
            // guard below neither sees it nor undoes for
            // it — and needs none: its moves never wrote,
            // its cancel just drops the record, and a
            // real settle lands exactly one stored-guide
            // op (add/move/delete, one undo step) here,
            // before `end_drag`'s recap runs.
            stage_view::guide_up(core, p, d.cancelled());
            // Roadmap Wave 2 (W2-08): settle a bend drag
            // beside the path settle. It never set
            // `core.dragging`, so the cancel guard below
            // neither sees it nor owes it an undo — its
            // moves never wrote and its Down took no
            // snapshot; a cancelled bend just drops its
            // record inside `bend_drag_up`.
            stage_view::bend_drag_up(core, p, d.cancelled());
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

/// Canvas drag entry with the driving button
/// (Roadmap Wave 2, W2-04): Rune's `Drag` carries no
/// button, so the mount opted into the middle button
/// (`El::pointer_buttons`) and hands `Tx::button()`
/// here. A middle-button drag — or a left drag while
/// Space is held (the shell's key state, IA-W2-04-3)
/// — is a VIEW PAN and never reaches the content
/// chain: it writes only the shell view navigation.
/// Everything else is the standing `stage_drag`.
fn stage_drag_button(s: &mut State, d: &mut Drag, button: rune::PointerButton) {
    let pan = button == rune::PointerButton::Middle
        || (button == rune::PointerButton::Primary && stage_view::space_pan_held());
    if pan {
        let local = [d.x, d.y];
        match d.phase {
            Phase::Down => {
                let source = if button == rune::PointerButton::Middle {
                    stage_view::PanSource::Middle
                } else {
                    stage_view::PanSource::Space
                };
                stage_view::view_pan_down(local, source);
            }
            Phase::Move => {
                stage_view::view_pan_move(local);
            }
            Phase::Up => {
                stage_view::view_pan_up(d.cancelled());
            }
        }
        // Keep the pointer probe fed, as `stage_drag`
        // does, so hover/HUD resume correctly when the
        // pan ends under the pointer.
        stage_view::set_stage_pointer(Some(stage_view::StagePointer {
            local,
            size: [d.w, d.h],
        }));
        return;
    }
    stage_drag(s, d);
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
    secondary_pick_stack_with_radius(s, p, stage_view::pick_radius(rt.vt.scale));
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
    // The historical radius (13.0 stage units, the
    // pre-RM15 constant): the stack's own tests call
    // this two-argument form; the live right-press
    // threads its screen-space radius through the twin
    // below (IA-RM15-2).
    secondary_pick_stack_with_radius(s, p, 13.0);
}

fn secondary_pick_stack_with_radius(s: &mut State, p: [f32; 2], radius: f32) {
    if stage_view::secondary_select_with_radius(&mut s.core, p, radius) {
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
/// schedule the next frame on the anchored playback
/// deadline ([`PLAYBACK_DEADLINE`], Roadmap Wave 1 RM7 /
/// F-6) — the Rune form of the GPUI app's 33 ms tick calling
/// `advance_play`.
fn poll(s: &mut State, t: f64, tx: &mut Tx) -> bool {
    let dt = s
        .last_poll
        .map(|last| ((t - last) as f32).clamp(0.0, 0.1))
        .unwrap_or(0.0);
    s.last_poll = Some(t);
    rune_sec::project_tools::sync_name_edit(s);
    rune_sec::inspector_figure::sync_figure_name_edit(s);
    // Roadmap Wave 1 (F-3, IA-RM4-1; P-10, IA-RM20-1):
    // the number-field sessions' blur/Escape
    // reconciliation for the graph, camera, segment
    // inspector and audio panels.
    rune_sec::graph_panel::sync_number_edits(s);
    rune_sec::camera_panel::sync_number_edits(s);
    rune_sec::inspector_segment::sync_number_edits(s);
    rune_sec::audio_panel::sync_number_edits(s);
    rune_sec::keyframe_tools::sync_number_edits(s);
    // Roadmap Wave 2 (W2-02, IA-W2-02-2): keep the
    // arrow-move character arm in step with the core's
    // selection — the watcher the row presses cannot
    // cover (stage picks, MCP, undo) ride through here.
    stage_view::sync_arrow_selection(&s.core);
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
        if s.toasts.tick_needed(now) {
            tx.wake_after(Duration::from_millis(500));
        }
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
    // Roadmap Wave 1 (RM13 — P-1): the timeline zoom
    // pair (chip width, follow) rides the same shadow —
    // the zoom persists as the CLAMPED live width
    // (`chip_w`), the value the strip actually shows.
    let layout_pair = (
        s.frame_ops.timeline_collapsed,
        s.show_left,
        s.frame_ops.chip_w(),
        s.frame_ops.timeline_follow,
    );
    if LAYOUT_PREFS_SHADOW.with(|c| c.get()) != layout_pair {
        LAYOUT_PREFS_SHADOW.with(|c| c.set(layout_pair));
        let mut prefs = stykanimate_rs::shell::load_layout();
        prefs.timeline_collapsed = layout_pair.0;
        prefs.sidebar_visible = layout_pair.1;
        prefs.timeline_zoom = layout_pair.2;
        prefs.timeline_follow = layout_pair.3;
        rune_sec::workspaces::sync_active_record(&mut prefs, s);
        stykanimate_rs::shell::save_layout(&prefs);
    }
    // Roadmap F-9: the UI language rides the same
    // shadow-compare discipline — when `core.lang`
    // diverges from the persisted shadow (the Settings
    // card's picker, the shell settings tools, or
    // `cycle_lang` changed it), load-modify-save
    // `LayoutPrefs.lang` so the choice survives a
    // relaunch, preserving every other field.
    if LANG_PREFS_SHADOW.with(|c| c.get()) != s.core.lang {
        LANG_PREFS_SHADOW.with(|c| c.set(s.core.lang));
        let mut prefs = stykanimate_rs::shell::load_layout();
        prefs.lang = s.core.lang;
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
        // The anchored deadline dies with playback (RM7):
        // the next Play press anchors a fresh grid.
        PLAYBACK_DEADLINE.with(|c| c.set(None));
        return redraw;
    }
    // Loop off (Phase G): stop on the last frame instead
    // of letting `advance_play` wrap. The sound is stopped
    // here, not left to the flip hook's next tick — after
    // a stop, nothing schedules another prompt wake.
    // Roadmap Wave 2 (W2-40): the loop MODE decides — both
    // looping flavours (wrap and ping-pong) suppress the
    // stop; ping-pong never reaches an "end" at all, it
    // bounces inside `advance_ping_pong`.
    let ping_pong = rune_sec::playback_controls::ping_pong_enabled();
    if rune_sec::playback_controls::should_stop_at_end(
        s.core.playing,
        s.loop_playback || ping_pong,
        s.core.loop_range,
        s.core.current,
        s.core.frame_count(),
    ) {
        s.core.playing = false;
        s.was_playing = false;
        s.core.stop_playback_audio();
        s.last_frame = s.core.current;
        PLAYBACK_DEADLINE.with(|c| c.set(None));
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
        if ping_pong {
            // Roadmap Wave 2 (W2-40): ping-pong travel —
            // the advance reflects at the window's ends
            // instead of wrapping, flipping the core's
            // direction. Sound cannot play in reverse: at
            // the end bounce the pass's audio stops (the
            // wrap/seek/drift syncs below all stand down
            // while no audio is active, so the reverse leg
            // stays silent on its own), and at the start
            // bounce a fresh pass starts, so the audio
            // restarts from the window's start exactly as
            // it does after a wrap. A single tick crossing
            // both bounds of a tiny range flips twice and
            // reads as no flip here — the picture stays
            // exact; only the audio restart is skipped.
            let was_reverse = s.core.play_reverse;
            s.core.advance_ping_pong(dt * s.playback_speed);
            if s.core.play_reverse != was_reverse {
                if s.core.play_reverse {
                    s.core.stop_playback_audio();
                } else {
                    s.core.stop_playback_audio();
                    s.core.start_playback_audio();
                }
            }
        } else {
            s.core.advance_play(dt * s.playback_speed);
        }
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
    // Roadmap Wave 1 (RM13 — P-1, IA-RM13-5): Follow —
    // while playing with follow on, keep the playhead's
    // chip inside the timeline viewport: when the
    // advance above carried it out of view, scroll the
    // strip so the chip sits mid-viewport. The scroll
    // record is the timeline builder's own (written on
    // every scroll-offset change); until the strip has
    // scrolled once there is no record and nothing to
    // follow with.
    if s.frame_ops.timeline_follow {
        let w = s.frame_ops.chip_w();
        if let Some((off, vw)) = rune_sec::timeline_marks::timeline_viewport() {
            if !rune_sec::timeline_marks::chip_visible(off, vw, s.core.current, w) {
                let target = (s.core.current as f32
                    * rune_sec::timeline_marks::pitch(w)
                    - vw / 2.0)
                    .max(0.0);
                tx.scroll_to_x("timeline", target);
            }
        }
    }
    // Roadmap Wave 1 (RM7 / F-6): wake on the anchored
    // deadline, not "one interval from now" — the deadline
    // steps from its own previous value (see
    // `playback_deadline`), so the time this poll spent
    // working is not added to every frame's period.
    let deadline = rune_sec::playback_controls::playback_deadline(
        PLAYBACK_DEADLINE.with(|c| c.get()),
        t,
        s.core.project.fps,
        s.playback_speed,
    );
    PLAYBACK_DEADLINE.with(|c| c.set(Some(deadline)));
    tx.wake_after(Duration::from_secs_f64((deadline - t).max(0.0)));
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
        // Roadmap Wave 1 (P-12, IA-RM22-2): the Asset
        // Manager's rename field, gated on its session
        // like the subtitle panel's text fields — typing
        // there must not reach the global bindings.
        || (s.sprites_panel.asset_renaming.is_some()
            && s.sprites_panel.asset_rename.focused)
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
        // Roadmap Wave 1 (PL-7, IA-RM28-2): the Help
        // search field — typing there must not trigger
        // shortcuts, like every other text field.
        || s.help_panel.search.focused
}

/// App shortcuts. The command palette gets first claim
/// (Ctrl+K opens it; arrows/Enter/Escape drive it while
/// open). Escape then cancels the topmost remaining scope
/// (an open menu, an armed destructive confirm, Home's
/// armed delete confirm — see the tier block in the body).
/// While a text field is focused, everything else is
/// left to the field. Then: Ctrl+Z undo, Ctrl+Shift+Z /
/// Ctrl+Y redo, Ctrl+S quick-save (the app-bar Save
/// Roadmap Wave 2 (W2-04): the Space tap, extracted from the `view()` closure so tests can drive a full tap (press + release); behaviour unchanged.
fn on_space_release(s: &mut State, ev: &KeyEvent) -> bool {
    // A release whose hold no pan consumed is the play/pause press users know, one phase later; a consumed hold (a pan) and a release with no armed press (typed into a field) both do nothing.
    if matches!(&ev.key, Key::Text(text) if text == " ")
        && stage_view::space_pan_release()
    {
        chrome_view::toggle_play(s);
        return true;
    }
    false
}

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
    // Roadmap Wave 1 (PL-7, IA-RM28-5): F1 toggles Help.
    // Placed after the text-field bail and before the
    // Alt block, so the Shortcuts description "all but
    // Ctrl+K pause while typing" stays exactly true.
    if matches!(ev.key, Key::Function(1)) {
        s.menus.help_open = !s.menus.help_open;
        return true;
    }
    if ev.mods.alt {
        if let Key::Text(text) = &ev.key {
            if let Some(n) = text.chars().next().and_then(|c| c.to_digit(10)) {
                if (1..=9).contains(&n) {
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
        // Roadmap Wave 1 (RM26 - PL-4): Ctrl+Shift+S is
        // Save a copy instead - it silently quick-saved
        // before this split.
        if ev.mods.shift {
            rune_sec::project_tools::save_copy_pressed(s);
        } else {
            s.core.save();
        }
        return true;
    }
    // Roadmap Wave 1 (RM26 - PL-4): the File/Edit chord
    // cluster - every chord runs the same core op its
    // menu item or palette command runs.
    if ev.is_command('o') {
        s.core.open_project_file();
        return true;
    }
    if ev.is_command('c') {
        s.core.copy_frame();
        return true;
    }
    if ev.is_command('v') {
        s.core.paste_frame();
        return true;
    }
    if ev.is_command('d') {
        s.core.duplicate_frame();
        return true;
    }
    // Rig tree navigation (Wave 2, W2-22): while the Build
    // workspace is active, the arrows walk the rig tree of
    // the selected figure instead of stepping frames or
    // nudging — Build is rigging mode, and the tree is its
    // primary list. Up/Down are unclaimed in Build today;
    // Left/Right yield to the tree only here.
    if s.workspace == rune_sec::workspaces::Workspace::Build {
        let rig_key = match &ev.key {
            Key::Up => Some(rune_sec::rig_tree::RigKey::Up),
            Key::Down => Some(rune_sec::rig_tree::RigKey::Down),
            Key::Left => Some(rune_sec::rig_tree::RigKey::Left),
            Key::Right => Some(rune_sec::rig_tree::RigKey::Right),
            _ => None,
        };
        if let Some(k) = rig_key {
            if rune_sec::rig_tree::rig_tree_key(&mut s.core, k) {
                return true;
            }
        }
    }
    match &ev.key {
        // No Escape arm here: the tier block at the top of
        // this function owns Escape (Wave P, P9).
        Key::Text(text) if text == " " && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            // Roadmap Wave 2 (W2-04): Space is a
            // hold-to-pan key now. The press only arms
            // the pan state (repeats land on the armed
            // state and do nothing — holding Space used
            // to strobe playback on every OS repeat);
            // play/pause fires on the RELEASE, and
            // only when no pan gesture consumed the
            // hold — a tap. See the root's on_key_up
            // below and stage_view's Space section.
            stage_view::space_pan_press();
            true
        }
        Key::Left => {
            // Roadmap Wave 1 (RM26 - PL-4, subsuming
            // IA-RM15-4) + Wave 2 (W2-02, IA-W2-02-1):
            // the arrows move the selection — a joint
            // nudges (RM15's steps: 1 pt, Shift 10, Alt
            // 0.1), a selected character or marquee set
            // moves whole (stage_view::nudge_arrows
            // resolves which); they step frames, Shift
            // stepping ten, ONLY when nothing is
            // selected. The text-field bail above is
            // RM15's no-text-edit gate by construction.
            if stage_view::arrows_move_selection(&s.core) {
                let st = stage_view::nudge_step(ev.mods.shift, ev.mods.alt);
                stage_view::nudge_arrows(&mut s.core, -st, 0.0);
            } else {
                step_frame(s, if ev.mods.shift { -10 } else { -1 });
            }
            true
        }
        Key::Right => {
            if stage_view::arrows_move_selection(&s.core) {
                let st = stage_view::nudge_step(ev.mods.shift, ev.mods.alt);
                stage_view::nudge_arrows(&mut s.core, st, 0.0);
            } else {
                step_frame(s, if ev.mods.shift { 10 } else { 1 });
            }
            true
        }
        Key::Up => {
            // Nudge only (RM26 - PL-4): with no
            // selection Up/Down stay unclaimed,
            // exactly today's behaviour.
            if stage_view::arrows_move_selection(&s.core) {
                let st = stage_view::nudge_step(ev.mods.shift, ev.mods.alt);
                stage_view::nudge_arrows(&mut s.core, 0.0, -st);
                true
            } else {
                false
            }
        }
        Key::Down => {
            if stage_view::arrows_move_selection(&s.core) {
                let st = stage_view::nudge_step(ev.mods.shift, ev.mods.alt);
                stage_view::nudge_arrows(&mut s.core, 0.0, st);
                true
            } else {
                false
            }
        }
        Key::Home => {
            if s.core.frame_count() > 0 {
                s.core.goto(0);
            }
            true
        }
        Key::End => {
            let n = s.core.frame_count();
            if n > 0 {
                s.core.goto(n - 1);
            }
            true
        }
        Key::Delete | Key::Backspace => {
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
            } else if stage_view::selected_path_point_deletable(&s.core) {
                // Roadmap Wave 2 (W2-36): the motion path's
                // selected point (the current frame's point,
                // overlay on) — claimed only when the guard
                // says the delete will land, so with the
                // overlay off or the point already straight
                // the key keeps its standing behaviour
                // untouched.
                stage_view::delete_selected_path_point(&mut s.core);
            } else if s.core.selected_node.is_none() {
                // Roadmap Wave 1 (RM26 - PL-4, K6): with
                // nothing selected the key names what it
                // would have removed, instead of running
                // the segment op's refusal; when a node
                // IS selected the op's own refusal
                // sentences stand.
                s.core.status = "Delete: nothing selected \u{2014} the key removes a selected stroke, particle event, segment or path point.".into();
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
        // Roadmap Wave 1 (RM26 - PL-4): the keyframing
        // and drawing text clusters - each runs the same
        // op its palette command runs. Alt+K is ordered
        // before plain K; both exclude the command
        // modifiers.
        Key::Text(text) if text == "," && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            rune_sec::command_palette::goto_prev_keyframe(s);
            true
        }
        Key::Text(text) if text == "." && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            rune_sec::command_palette::goto_next_keyframe(s);
            true
        }
        Key::Text(text) if text == "k" && ev.mods.alt && !ev.mods.cmd && !ev.mods.ctrl => {
            rune_sec::keyframe_tools::delete_key_here(s);
            true
        }
        Key::Text(text) if text == "k" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            rune_sec::keyframe_tools::key_all_here(s);
            true
        }
        Key::Text(text) if (text == "B" || text == "b") && ev.mods.shift && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.cycle_brush();
            true
        }
        Key::Text(text) if text == "b" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            s.core.draw_mode = !s.core.draw_mode;
            true
        }
        Key::Text(text) if text == "[" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            rune_sec::stroke_inspector::step_param(
                &mut s.core,
                rune_sec::stroke_inspector::Param::Size,
                -1,
            );
            true
        }
        Key::Text(text) if text == "]" && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            rune_sec::stroke_inspector::step_param(
                &mut s.core,
                rune_sec::stroke_inspector::Param::Size,
                1,
            );
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
    if vis(Sec::RigTree) {
        left = left.child(rune_sec::rig_tree::rig_tree_panel(s, &t));
    }
    if vis(Sec::Layers) {
        left = left.child(rune_sec::layers_panel::panel(s, &t));
    }

    // Right column: contextual sections in workflow order —
    // selection first, then scene/camera, graph/easing, draw,
    // effects, audio, clips, 3D, export, stats.
    let mut right = col().key("side-right").gap(12.0);
    if s.menus.help_open {
        right = right.child(rune_sec::help_panel::help_panel(core, &s.help_panel, &t));
        right = right.child(rune_sec::diagnostics_panel::diagnostics_section(&s, &t));
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
        // Roadmap Wave 1 (RM24, P-14): the History panel rides
        // the FrameOps section - Animate-workspace only (X9),
        // like its playhead family.
        right = right.child(rune_sec::history_panel::history_section(s, &t));
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
    // Per-figure onion overrides (Roadmap Wave 2, W2-37):
    // feed the stage's store from the shell state the Onion
    // panel writes, every frame, before the stage is built
    // below — the VIEW_NAV feed pattern. An empty vec (the
    // default state) renders the global-only onion pass.
    crate::stage_view::set_onion_figure_overrides(s.onion_ui.per_figure.clone());
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
                            guides_golden: s.guides_golden,
                            guides_diagonals: s.guides_diagonals,
                            guides_stored: s.guides_stored,
                            limit_arcs: s.limit_arcs,
                               rulers: s.rulers,
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
                    .pointer_buttons([rune::PointerButton::Middle])
                    .on_drag(|s: &mut State, d: &mut Drag, tx: &mut Tx| {
                        stage_drag_button(s, d, tx.button())
                    })
                    .on_secondary_press_local(
                        |s: &mut State, e: PointerEvent, _tx: &mut Tx| {
                            stage_secondary(s, e)
                        },
                    )
                    .on_wheel(|s: &mut State, e: WheelEvent, _tx: &mut Tx| {
                        stage_wheel(s, e)
                    })
                    // Roadmap Wave 1 (RM15 - P-3, IA-RM15-1):
                    // feed the stage pointer probe on move
                    // (element-local point + the canvas's
                    // laid-out size), clear it when the
                    // pointer leaves, and wear the feedback
                    // pass's cursor for the thing under the
                    // pointer (or the drag in flight).
                    .on_pointer_move_local(
                        move |s: &mut State, e: PointerEvent, _tx: &mut Tx| {
                            crate::stage_view::set_stage_pointer(Some(
                                crate::stage_view::StagePointer {
                                    local: [e.x, e.y],
                                    size: stage_size,
                                },
                            ));
                        },
                    )
                    .on_hover_change(
                        |_s: &mut State, hovered: bool, _tx: &mut Tx| {
                            if !hovered {
                                crate::stage_view::set_stage_pointer(None);
                            }
                        },
                    )
                    .cursor(crate::stage_view::stage_cursor_now(core, stage_size)),
                )
                // Roadmap Wave 2 integration (W2-45, IA-W2-45-1):
                // mount the shaped subtitle element layer over
                // the canvas. The canvas prim stream skips bands
                // claimed by the shaped route, so without this
                // mount a Shaped cue paints nowhere on stage.
                // Empty (no claimed band) mounts nothing.
                .children(rune_sec::stage_text::stage_subtitle_layers(
                    core, stage_size,
                ))
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
                prefs.col_right_w = (prefs.col_right_w - dx).clamp(240.0, 480.0);
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
    // Roadmap Wave 2 (W2-04): the Space tap — see `on_space_release` above.
    root.on_key_up(|s: &mut State, ev: &KeyEvent, _tx: &mut Tx| on_space_release(s, ev))
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
    use super::{init, on_key, on_space_release, text_focused, ConfirmAction, State};
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

    /// A full Space tap (W2-04): press arms the hold-to-pan state, release fires play/pause.
    fn tap_space(s: &mut State) -> bool {
        let ev = typed(" ");
        assert!(press(s, &ev), "Space press arms the hold");
        on_space_release(s, &ev)
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
        // Space toggles playback on release (W2-04 tap); C camera mode; O onion.
        let playing = s.core.playing;
        assert!(tap_space(&mut s));
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

    // Roadmap Wave 1 (RM26 - PL-4, IA-RM26-1 item 7):
    // the chord clusters, one observable effect each.

    fn shifted(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                shift: true,
                cmd: false,
                alt: false,
                ctrl: false,
                logo: false,
            },
        }
    }

    #[test]
    fn home_and_end_jump_to_the_timeline_ends() {
        let mut s = init();
        let n = s.core.frame_count();
        assert!(n > 1, "the demo project has frames to jump across");
        assert!(press(&mut s, &plain(Key::End)));
        assert_eq!(s.core.current, n - 1);
        assert!(press(&mut s, &plain(Key::Home)));
        assert_eq!(s.core.current, 0);
    }

    #[test]
    fn shift_arrows_step_ten_frames_without_a_selection() {
        let mut s = init();
        assert_eq!(s.core.selected_node, None);
        let n = s.core.frame_count() as i32;
        let cur = s.core.current as i32;
        assert!(press(&mut s, &shifted(Key::Right)));
        assert_eq!(s.core.current as i32, (cur + 10).rem_euclid(n));
        assert!(press(&mut s, &shifted(Key::Left)));
        assert_eq!(s.core.current as i32, cur);
    }

    #[test]
    fn ctrl_d_duplicates_the_current_frame() {
        let mut s = init();
        let frames = s.core.frame_count();
        assert!(press(&mut s, &command('d')));
        assert_eq!(s.core.frame_count(), frames + 1);
    }

    #[test]
    fn b_toggles_draw_mode_and_shift_b_cycles_the_brush() {
        let mut s = init();
        let draw = s.core.draw_mode;
        assert!(press(&mut s, &typed("b")));
        assert_eq!(s.core.draw_mode, !draw);
        assert!(press(&mut s, &typed("b")));
        assert_eq!(s.core.draw_mode, draw);
        let brush = s.core.brush_index;
        assert!(press(&mut s, &shifted(Key::Text("B".to_string()))));
        assert_ne!(s.core.brush_index, brush);
    }

    #[test]
    fn backspace_is_delete_and_names_the_empty_selection() {
        let mut s = init();
        assert_eq!(s.core.selected_node, None);
        assert!(press(&mut s, &plain(Key::Delete)));
        let named = s.core.status.clone();
        assert_eq!(
            named,
            "Delete: nothing selected \u{2014} the key removes a selected stroke, particle event, segment or path point."
        );
        let mut s2 = init();
        assert!(press(&mut s2, &plain(Key::Backspace)));
        assert_eq!(s2.core.status, named, "Backspace is the same key");
    }

    #[test]
    fn arrows_move_an_armed_character_not_the_frame() {
        let mut s = init();
        s.core.selected_node = None;
        // Arm figure 0 as its row press does (IA-W2-02-2).
        crate::stage_view::note_character_selected(0);
        let frame = s.core.current;
        let before = s.core.project.frames[frame].poses[0].root;
        assert!(press(&mut s, &plain(Key::Right)));
        assert_eq!(s.core.current, frame, "an armed character captures the arrow");
        let after = s.core.project.frames[frame].poses[0].root;
        assert_eq!(after[0], before[0] + 1.0, "one plain step moves the character");
        assert!(press(&mut s, &plain(Key::Up)));
        let after = s.core.project.frames[frame].poses[0].root;
        assert_eq!(after[1], before[1] - 1.0, "Up moves the character up (stage y is down)");
        crate::stage_view::release_character_arm();
    }

    #[test]
    fn arrows_nudge_the_selected_joint_and_spend_one_step() {
        let mut s = init();
        let root = s.core.project.figures[0]
            .def
            .nodes
            .iter()
            .position(|n| n.parent.is_none())
            .expect("figure 0 has a root");
        s.core.selected_figure = 0;
        s.core.selected_node = Some(root);
        let before = s.core.project.frames[s.core.current].poses[0].root;
        let depth = s.core.undo_depth();
        assert!(press(&mut s, &plain(Key::Right)));
        let after = s.core.project.frames[s.core.current].poses[0].root;
        assert_eq!(after[0], before[0] + 1.0, "one plain step right");
        assert_eq!(after[1], before[1]);
        assert_eq!(s.core.undo_depth(), depth + 1, "one burst snapshot");
        // Up with a selection nudges too - the frame
        // does not move.
        let cur = s.core.current;
        assert!(press(&mut s, &plain(Key::Up)));
        assert_eq!(s.core.current, cur);
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
