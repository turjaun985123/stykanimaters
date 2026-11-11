//! Workspaces for the Rune editor shell (Phase E,
//! section 1 — workspaces). Declared from `main.rs` by the
//! integrator; bin-crate code, not part of the engine
//! library.
//!
//! The GPUI app had eight workspaces (Animate / Draw /
//! Build / Graph / Effects / Audio / 3D / Export) whose
//! switching genuinely re-gated which tool groups and
//! panels rendered (the `ws_has` table, verified group for
//! group in docs/QA_WAVE15.md §3). The Rune shell from
//! Phase D mounts every section at once; this section is
//! the missing switch: a [`Workspace`] enum, a
//! [`Section`] enum naming every mounted `rune_sec`
//! panel, a gating table ([`section_visible`]), and the
//! workspace chip row ([`workspace_row`]).
//!
//! The workspace set follows the Phase E brief: the GPUI
//! **Build** and **3D** workspaces merge into
//! [`Workspace::Build3D`] (labelled "3D", persisted as
//! `"3D"` — the exact string the GPUI app wrote to
//! `LayoutPrefs.workspace` for its 3D workspace), and
//! **Home** becomes a workspace of its own (in the GPUI
//! app "Home" was a group shown in every workspace, never
//! a workspace itself).
//!
//! Wave 2 amendment (W2-21, roadmap P-8 / RM19 design):
//! the merge is **split back out** — [`Workspace::Build`]
//! exists again as its own workspace (inserted before
//! [`Workspace::Build3D`] in row order, per RM19's
//! D-RM19-1 recommendation), homing the 2D rigging
//! surface ([`Section::RigTree`] plus the figure/library/
//! inspector sections that already ride Animate).
//! `Build3D` keeps its variant name, label and persisted
//! `"3D"` string untouched, so old prefs files resolve
//! exactly as before; `"Build"` is a new persisted string
//! no old prefs file can contain.
//!
//! # Integrator wiring (required)
//!
//! ```ignore
//! // rune_sec/mod.rs:  pub mod workspaces;
//! // State: add `pub workspace: workspaces::Workspace`,
//! // init from the persisted prefs main.rs already loads:
//! //     workspace: workspaces::from_prefs(&layout),
//! // view(): mount `workspaces::workspace_row(s, &t)` as
//! // its own row under the menu bar, and gate each mounted
//! // panel on `workspaces::section_visible(s.workspace,
//! // Section::…)` — e.g. only `.child(…graph_panel…)` when
//! // `section_visible(ws, Section::Graph)` is true.
//! ```
//!
//! Not gated (no [`Section`] variant, by design): the
//! settings popover, the help panel, and the shell's
//! permanent furniture (app bar, menus, stage, camera
//! overlay pill, transport, the timeline card — see
//! below — status strip, toasts, and the column
//! splitters) — those stay available in every
//! workspace, exactly as the GPUI app kept
//! Transport/Home visible in several of its
//! workspaces. Figure-add is NOT on this list: since
//! UI mission Wave 3 it mounts under the
//! [`Section::Figures`] gate in `main.rs`, because it
//! orphaned into Home/Export before that.
//!
//! # The timeline card and its lanes (chrome, not sections)
//!
//! The timeline card (`timeline_marks::timeline_marks`,
//! mounted by `main.rs` in the centre column — the
//! centre always mounts; only the side columns are
//! View-toggle gated) is permanent furniture: it
//! renders in **all nine workspaces**, and no
//! [`Section`] gates it or anything inside it.
//! (`chrome_view::timeline_row` is the card's unused
//! predecessor builder; the live card is
//! timeline_marks'.) Its lanes stack inside one
//! horizontal scroll viewport on the shared chip
//! pitch (the timeline's live zoom pitch;
//! `timeline_marks::CHIP_PITCH` at the default zoom),
//! top to bottom:
//!
//! - **Ruler** (`timeline-ruler`) and **frame chips**
//!   (`timeline-chips`, chips keyed `frame-N`):
//!   always, whenever the project has frames.
//! - **Marker row** (`timeline-markers`): data-gated,
//!   not workspace-gated — mounted only when at least
//!   one frame carries a camera/graph marker.
//! - **Clips lane** (`timeline-clips`): data-gated
//!   the same way — mounted only when the project
//!   holds at least one audio or FX clip. It
//!   summarises the Audio/Effects sections' content
//!   but appears in every workspace; do not "fix"
//!   that by gating it to Audio/Effects.
//! - **Dopesheet lane** (Wave 10): one row per figure
//!   (capped at 4) over `stats::figure_keyframes`,
//!   inside the same scrolled column. It is briefed
//!   as an Animate-workspace lane (docs/DOPESHEET.md
//!   §3): if it lands workspace-scoped, the gate is a
//!   `s.workspace` check inside the timeline builder,
//!   **not** a new [`Section`] — the card is chrome,
//!   and a variant for a surface no workspace mounts
//!   as a panel would break the census discipline
//!   below for nothing. Reconcile this paragraph with
//!   the landed lane when Wave 10 closes.
//!
//! Under the scroll viewport, still inside the card,
//! the **loop row** (`timeline-loop-row`) mounts
//! whenever frames exist; on a zero-frame project the
//! whole scroll stack is replaced by the kit empty
//! state. Lane presence is therefore a function of
//! project data (and, for the dopesheet, the
//! workspace) — never of [`section_visible`] — so
//! audits should not look for lane entries in the
//! gating table below.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::shell::{load_layout, save_layout, LayoutPrefs, WorkspaceLayout};

use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::icon_btn;
use crate::State;

/// One of the nine workspaces. The active workspace
/// decides which [`Section`]s the shell mounts
/// ([`section_visible`]); it is persisted across runs as
/// [`Workspace::name`] in `LayoutPrefs.workspace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Workspace {
    Animate,
    Draw,
    Graph,
    Effects,
    Audio,
    Build,
    Build3D,
    Export,
    Home,
}

impl Default for Workspace {
    /// The first-run workspace — the same default
    /// `LayoutPrefs` itself uses (`"Animate"`).
    fn default() -> Self {
        Workspace::Animate
    }
}

/// Every mounted `rune_sec` panel the workspace switch
/// can gate. Variants follow the mounted builders in
/// `main.rs`'s view: the left column (Home, Project,
/// Figures, Library, RigTree, Layers) and the right column
/// (inspectors, pose/keyframe tools, camera, scene,
/// onion, playback, frame ops, graph, easing, draw,
/// stroke inspector, text, FX browser/clips, audio,
/// waveform, movieclips, sprites, 3D, export, stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Figures,
    Library,
    RigTree,
    Layers,
    Project,
    InspectorSegment,
    InspectorFigure,
    InspectorStyle,
    PoseTools,
    KeyframeTools,
    Camera,
    Scene,
    Onion,
    Playback,
    FrameOps,
    Graph,
    Easing,
    DrawTools,
    StrokeInspector,
    TextTool,
    FxBrowser,
    FxClips,
    Particles,
    Audio,
    Waveform,
    Clips,
    Sprites,
    ThreeD,
    Export,
    Stats,
    Home,
}

impl Workspace {
    /// All nine workspaces, in chip-row order. Build sits
    /// immediately before Build3D (RM19 D-RM19-1, the
    /// INSERT resolution): the two rigging workspaces are
    /// adjacent, at the price of renumbering the Alt
    /// chords behind them (3D Alt+7, Export Alt+8, Home
    /// Alt+9).
    pub fn all() -> [Workspace; 9] {
        [
            Workspace::Animate,
            Workspace::Draw,
            Workspace::Graph,
            Workspace::Effects,
            Workspace::Audio,
            Workspace::Build,
            Workspace::Build3D,
            Workspace::Export,
            Workspace::Home,
        ]
    }

    /// The chip label shown in the workspace row.
    pub fn label(self) -> &'static str {
        match self {
            Workspace::Animate => "Animate",
            Workspace::Draw => "Draw",
            Workspace::Graph => "Graph",
            Workspace::Effects => "Effects",
            Workspace::Audio => "Audio",
            Workspace::Build => "Build",
            Workspace::Build3D => "3D",
            Workspace::Export => "Export",
            Workspace::Home => "Home",
        }
    }

    /// The persisted name — the string stored in
    /// `LayoutPrefs.workspace`. Identical to
    /// [`Workspace::label`] for every variant (the GPUI
    /// app persisted `"3D"` for its 3D workspace, which
    /// is why [`Workspace::Build3D`] persists as `"3D"`,
    /// not `"Build"` or `"Build3D"`).
    pub fn name(self) -> &'static str {
        self.label()
    }

    /// The chip's element key (`ws-animate` … `ws-home`,
    /// the GPUI-era `ws_*` naming in hyphen form).
    pub fn key(self) -> &'static str {
        match self {
            Workspace::Animate => "ws-animate",
            Workspace::Draw => "ws-draw",
            Workspace::Graph => "ws-graph",
            Workspace::Effects => "ws-effects",
            Workspace::Audio => "ws-audio",
            Workspace::Build => "ws-build",
            Workspace::Build3D => "ws-3d",
            Workspace::Export => "ws-export",
            Workspace::Home => "ws-home",
        }
    }

    /// The i18n catalogue key for this workspace's chip
    /// label (the `ws_*` block of `src/i18n.rs`). Kept
    /// distinct from [`Workspace::key`]: that one is the
    /// element key (`ws-animate`, hyphen form); this one
    /// is the translation key (`ws_animate`, underscore
    /// form). `Build3D` maps to `ws_3d` — its chip reads
    /// "3D"; `Build` reclaims the `ws_build` key the
    /// Wave 14 workspace key set reserved for it (the key
    /// itself is spliced into the tables by IA-W2-21-3 —
    /// until it lands, the catalogue pin below fails by
    /// design, the shell's adopt-ahead idiom).
    pub fn i18n_key(self) -> &'static str {
        match self {
            Workspace::Animate => "ws_animate",
            Workspace::Draw => "ws_draw",
            Workspace::Graph => "ws_graph",
            Workspace::Effects => "ws_effects",
            Workspace::Audio => "ws_audio",
            Workspace::Build => "ws_build",
            Workspace::Build3D => "ws_3d",
            Workspace::Export => "ws_export",
            Workspace::Home => "ws_home",
        }
    }

    /// The chip label in the shell's current language:
    /// [`Workspace::label`] resolved through the i18n
    /// table (`i18n_wire::tr` over `core.lang`). This is
    /// the text [`workspace_row`] renders. `label()`
    /// itself stays the canonical English string — it is
    /// also the persisted name ([`Workspace::name`]), and
    /// persistence must never vary with the UI language.
    /// In English the two agree for every workspace
    /// (pinned by the tests below).
    pub fn display_label(self, core: &EditorCore) -> String {
        i18n_wire::tr(core, self.i18n_key())
    }

    /// One sentence saying what this workspace is for —
    /// the same summaries that head the gating-table
    /// lists below. Shown under the chip row by
    /// [`workspace_row`] (via [`blurb_line`]) and also
    /// exposed for surfaces that describe the workspaces
    /// without mounting them (the help panel, the status
    /// strip). Keep each to a single sentence ending in
    /// a period.
    pub fn blurb(self) -> &'static str {
        match self {
            Workspace::Animate => {
                "Pose figures and build the animation frame by frame, with the inspectors, camera, playback and frame tools."
            }
            Workspace::Draw => {
                "Draw freehand strokes, shapes and text over the figures."
            }
            Workspace::Graph => {
                "Edit the motion curves and easing of the selected figure's keys."
            }
            Workspace::Effects => {
                "Add and edit particle and visual-effects clips anchored to the scene."
            }
            Workspace::Audio => {
                "Place sound clips and line them up against the animation."
            }
            Workspace::Build => {
                "Build and edit figure rigs: the node tree, segment properties, joint limits and the setup pose."
            }
            Workspace::Build3D => {
                "Pose the 3D figure and frame it with the camera."
            }
            Workspace::Export => {
                "Render and export the project to images, video and other formats."
            }
            Workspace::Home => {
                "Open, create and manage projects, and check project stats."
            }
        }
    }
}

/// Parse a persisted workspace name (from
/// `LayoutPrefs.workspace`). Unknown strings fall back
/// to [`Workspace::Animate`], the shell default.
/// `"Build"` resolves to [`Workspace::Build`] again
/// since the Wave 2 split (W2-21); prefs written before
/// the split can only contain the eight older names, so
/// nothing migrates.
pub fn from_name(name: &str) -> Workspace {
    Workspace::all()
        .into_iter()
        .find(|ws| ws.name() == name)
        .unwrap_or_default()
}

/// The workspace a persisted prefs set names
/// ([`from_name`] over `prefs.workspace`) — the
/// integrator's `init()` seed for the `State` field.
pub fn from_prefs(prefs: &LayoutPrefs) -> Workspace {
    from_name(&prefs.workspace)
}

/// A copy of `prefs` with only the workspace name
/// replaced — the load-modify-save step
/// [`select_workspace`] persists, factored out so the
/// field-preservation contract is testable without
/// touching the disk.
pub fn prefs_with_workspace(prefs: &LayoutPrefs, ws: Workspace) -> LayoutPrefs {
    let mut out = prefs.clone();
    out.workspace = ws.name().to_string();
    out
}

/// The setup-mode value a workspace switch leaves
/// behind (Wave 2, W2-21 — RM19 §2): entering Build
/// enters setup mode (the saved prior value rides
/// `saved`); leaving Build restores the saved value;
/// any other switch — including re-selecting the
/// workspace already active — changes nothing. Pure, so
/// the enter/leave contract is pinnable without a
/// `State` (the `State` field itself is IA-W2-21-2).
pub fn setup_mode_after_switch(
    was_build: bool,
    is_build: bool,
    current: bool,
    saved: &mut bool,
) -> bool {
    if is_build && !was_build {
        *saved = current;
        true
    } else if was_build && !is_build {
        *saved
    } else {
        current
    }
}

/// Switch the shell's workspace and persist the choice:
/// applies the Build setup-mode rule ([`setup_mode_after_switch`]),
/// sets `s.workspace`, runs the layout-memory transaction
/// ([`switch_layouts`] — the outgoing workspace's live
/// shape is stored, the incoming workspace's remembered
/// shape is applied), then load-modify-saves `LayoutPrefs`
/// with the new workspace name, preserving every other
/// field (theme, timeline, sidebar) — the same pattern
/// `main.rs`'s `poll` uses for theme flips. Every
/// workspace change funnels here (chip row, Alt+number,
/// palette jumps), so the setup rule and the layout
/// memory cover all three doors alike.
pub fn select_workspace(s: &mut State, ws: Workspace) {
    // Stage focus never survives a workspace switch
    // (W2-10): the restore belongs to the outgoing
    // workspace's shape, and the incoming workspace
    // applies its own remembered layout below — leaving
    // focus set would strand the restore and make the
    // capture below read a dead restore. The columns
    // come back with the incoming record's own
    // visibility via `apply_layout`. (The capture reads
    // the restore's visibility either way — see
    // `capture_visibility` — so the outgoing workspace
    // remembers its pre-focus shape, not the hidden
    // one.)
    s.stage_focus_restore = None;
    let was_build = s.workspace == Workspace::Build;
    let is_build = ws == Workspace::Build;
    s.core.setup_mode = setup_mode_after_switch(
        was_build,
        is_build,
        s.core.setup_mode,
        &mut s.setup_mode_before_build,
    );
    let outgoing = s.workspace;
    s.workspace = ws;
    let mut prefs = load_layout();
    let live = live_layout(s, &prefs);
    let incoming = switch_layouts(&mut prefs, outgoing, ws, live);
    apply_layout(s, incoming);
    prefs.workspace = ws.name().to_string();
    save_layout(&prefs);
}

// ---- Layout memory (Roadmap Wave 2, W2-61 — PL-10) -------
//
// RM12's design (§4 of rm12.md), built. Every workspace
// remembers its own panel layout — column widths, column
// visibility, timeline collapse — across switches and
// launches. The memory lives in
// `LayoutPrefs.workspace_layouts` (shell.rs —
// IA-W2-61-1); the record type is
// `stykanimate_rs::shell::WorkspaceLayout`.
//
// The legacy global fields stay as the ACTIVE workspace's
// mirror: the shell reads widths through
// `main.rs::column_widths()` (which re-reads the prefs
// file) and MCP's `shell_get_layout` reads the globals,
// so every switch and every startup copies the active
// record into the globals — older binaries and the layout
// tools keep reading sane values, and the globals always
// describe the workspace the user is looking at.
//
// Session vs persisted split: the five record fields are
// the persisted half. NOT in the records, by RM12 §4.3.1:
// the per-lane visibility flags (they answer project
// data, not workspace furniture — they stay global), the
// timeline zoom/follow pair (global prefs, RM13), and any
// transient stage-focus state (W2-10, built in the
// stage-focus section below — it lives only in `State`,
// never in a record or a prefs field, so a restart can
// never restore a focused stage).

/// The shell's live panel layout at one instant — the
/// bin-side shape of a [`WorkspaceLayout`] record, taken
/// from the places the shell actually keeps the values:
/// the widths from the prefs globals (the prefs file IS
/// the live width store — `main.rs::column_widths()`
/// re-reads it every frame and the splitter drags write
/// it directly) and the visibility/collapse flags from
/// `State`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveLayout {
    pub col_left_w: f32,
    pub col_right_w: f32,
    pub show_left: bool,
    pub show_right: bool,
    pub timeline_collapsed: bool,
}

impl LiveLayout {
    /// This live layout as a persisted record (widths
    /// clamped to the load ranges — a record never holds
    /// a width the loader would reject).
    pub fn to_record(self) -> WorkspaceLayout {
        WorkspaceLayout {
            col_left_w: self.col_left_w,
            col_right_w: self.col_right_w,
            show_left: self.show_left,
            show_right: self.show_right,
            timeline_collapsed: self.timeline_collapsed,
        }
        .clamped()
    }

    /// The live layout a persisted record restores.
    pub fn from_record(record: WorkspaceLayout) -> LiveLayout {
        let record = record.clamped();
        LiveLayout {
            col_left_w: record.col_left_w,
            col_right_w: record.col_right_w,
            show_left: record.show_left,
            show_right: record.show_right,
            timeline_collapsed: record.timeline_collapsed,
        }
    }
}

/// Capture the live layout (see [`LiveLayout`] for where
/// each value lives). While stage focus is active
/// (W2-10 — see the stage-focus section below) the
/// visibility half is read from the focus restore, not
/// from the live flags: the flags read hidden *because*
/// focus hid them, and a capture that believed them
/// would remember "both columns hidden" as the
/// workspace's layout.
pub fn live_layout(s: &State, prefs: &LayoutPrefs) -> LiveLayout {
    let (show_left, show_right) =
        capture_visibility(s.show_left, s.show_right, s.stage_focus_restore);
    LiveLayout {
        col_left_w: prefs.col_left_w,
        col_right_w: prefs.col_right_w,
        show_left,
        show_right,
        timeline_collapsed: s.frame_ops.timeline_collapsed,
    }
}

/// The visibility half of a layout capture, factored
/// out pure so the focus rule is pinnable without a
/// `State` (W2-10): a focus restore, when one is held,
/// is the truth about the columns; otherwise the live
/// flags are. Widths need no such care — the splitters
/// unmount with their columns, so the prefs widths
/// cannot move during focus — and `timeline_collapsed`
/// is centre furniture that stays live under focus.
fn capture_visibility(
    show_left: bool,
    show_right: bool,
    restore: Option<LiveLayout>,
) -> (bool, bool) {
    match restore {
        Some(saved) => (saved.show_left, saved.show_right),
        None => (show_left, show_right),
    }
}

/// Apply a remembered record to the live shell: the
/// `State`-resident half. The width half needs no work
/// here — [`switch_layouts`] / [`startup_layout`] mirror
/// the record into the prefs globals, and
/// `column_widths()` reads those.
pub fn apply_layout(s: &mut State, record: WorkspaceLayout) {
    let live = LiveLayout::from_record(record);
    s.show_left = live.show_left;
    s.show_right = live.show_right;
    s.frame_ops.timeline_collapsed = live.timeline_collapsed;
}

/// The record the legacy global fields describe — the
/// pre-PL-10 layout, and the fallback for any workspace
/// with no record of its own. `show_right` has no legacy
/// mirror (it was never persisted before PL-10 and has
/// always started shown), so the legacy record shows it.
pub fn legacy_layout(prefs: &LayoutPrefs) -> WorkspaceLayout {
    WorkspaceLayout {
        col_left_w: prefs.col_left_w,
        col_right_w: prefs.col_right_w,
        show_left: prefs.sidebar_visible,
        show_right: true,
        timeline_collapsed: prefs.timeline_collapsed,
    }
    .clamped()
}

/// The record a factory-fresh prefs file's legacy
/// globals describe — the long-standing single layout
/// (300 / 340, both columns shown, timeline expanded).
/// The seeding rule below compares against it to tell
/// "no arrangement was ever made" apart from "the user
/// arranged the pre-memory layout".
fn factory_layout() -> WorkspaceLayout {
    legacy_layout(&LayoutPrefs::default())
}

/// One workspace's stage-share profile (Roadmap Wave
/// 2, W2-10 — RM12 §5.3's priced table): the column
/// widths this workspace seeds with when it has no
/// remembered record of its own, so a fresh install
/// opens each workspace at the shape the design priced
/// for it instead of the one global 300 / 340. The
/// centre column is what is left of the window's inner
/// width after the two columns and the two 12 pt
/// splitter junctions, so at 1440×900 (inner 1408) the
/// shares run: Animate 924 pt = 65.6% — inside the
/// 65–80% band the 2.0 directive sets for the stage,
/// and the only workspace the band binds; Graph 848
/// (60.2%), Draw 884 (62.8%), Build / Build3D /
/// Effects 828 (58.8%), Audio / Export 808 (57.4%).
/// Visibility and collapse are not profiled — every
/// profile shows both columns with the timeline
/// expanded, the historical defaults.
///
/// Two derivations, recorded: RM12 priced eight
/// workspaces (its table predates the Build split), so
/// Build inherits its sibling Build3D's 260 / 320 —
/// the two rigging workspaces share the stage form.
/// And Animate's 240 right column is only reachable
/// because W2-10 also took RM12 §4.3.2's clamp decision
/// (the right-column load minimum 260 → 240, in
/// `shell.rs`); every width here sits inside the load
/// clamps, so a profile can never be clamped away.
///
/// `None` for Home: RM12 priced no Home profile (its
/// row awaited the A1 R11 centre takeover, which does
/// not exist in this tree), so Home keeps the legacy
/// seed until a takeover re-bases it.
pub fn default_layout_for(ws: Workspace) -> Option<WorkspaceLayout> {
    let widths = match ws {
        Workspace::Animate => (220.0, 240.0),
        Workspace::Draw => (220.0, 300.0),
        Workspace::Graph => (260.0, 300.0),
        Workspace::Effects => (260.0, 320.0),
        Workspace::Audio => (280.0, 320.0),
        Workspace::Build => (260.0, 320.0),
        Workspace::Build3D => (260.0, 320.0),
        Workspace::Export => (260.0, 340.0),
        Workspace::Home => return None,
    };
    Some(WorkspaceLayout {
        col_left_w: widths.0,
        col_right_w: widths.1,
        show_left: true,
        show_right: true,
        timeline_collapsed: false,
    })
}

/// Seed a record for every workspace that lacks one
/// (RM12 §4.2 migration, re-based by W2-10's
/// stage-share recovery): a workspace with a priced
/// profile ([`default_layout_for`]) seeds from it, and
/// a workspace without one seeds from the legacy
/// globals. One protection: when the legacy globals
/// differ from the factory layout — evidence the user
/// arranged the pre-memory single layout — the
/// workspace the prefs name as active seeds from the
/// legacy globals instead of its profile, because that
/// arrangement was in effect *there*; the profiles are
/// defaults, and defaults never override an existing
/// user's arrangement. (A factory file's active
/// workspace takes its profile: its "arrangement" is
/// the factory default itself.) Nothing is prompted —
/// this is layout, not user data (RM9's
/// prompted-migration caution covers the projects dir,
/// not prefs values). Existing records are never
/// overwritten, so re-running is a no-op.
pub fn seed_missing_workspace_layouts(prefs: &mut LayoutPrefs) {
    let legacy = legacy_layout(prefs);
    let protect = if legacy != factory_layout() {
        Some(from_prefs(prefs))
    } else {
        None
    };
    for ws in Workspace::all() {
        if !prefs
            .workspace_layouts
            .iter()
            .any(|(name, _)| name == ws.name())
        {
            let seed = if protect == Some(ws) {
                legacy
            } else {
                default_layout_for(ws).unwrap_or(legacy)
            };
            prefs.workspace_layouts.push((ws.name().to_string(), seed));
        }
    }
}

/// One workspace's remembered layout, clamped — its own
/// record when it has one, the legacy globals otherwise.
pub fn record_for(prefs: &LayoutPrefs, ws: Workspace) -> WorkspaceLayout {
    prefs
        .workspace_layouts
        .iter()
        .find(|(name, _)| name == ws.name())
        .map(|(_, record)| *record)
        .unwrap_or_else(|| legacy_layout(prefs))
        .clamped()
}

/// Store one workspace's record (replacing any it had).
pub fn store_record(prefs: &mut LayoutPrefs, ws: Workspace, record: WorkspaceLayout) {
    let record = record.clamped();
    if let Some(slot) = prefs
        .workspace_layouts
        .iter_mut()
        .find(|(name, _)| name == ws.name())
    {
        slot.1 = record;
    } else {
        prefs.workspace_layouts.push((ws.name().to_string(), record));
    }
}

/// Copy a record into the legacy global fields — the
/// active-workspace mirror rule (see the section head).
/// `show_right` has no global to mirror into; it lives
/// only in the records.
fn mirror_globals(prefs: &mut LayoutPrefs, record: WorkspaceLayout) {
    prefs.col_left_w = record.col_left_w;
    prefs.col_right_w = record.col_right_w;
    prefs.sidebar_visible = record.show_left;
    prefs.timeline_collapsed = record.timeline_collapsed;
}

/// The switch transaction, pure over the prefs (RM12
/// §4.2's choke-point wiring): store the OUTGOING
/// workspace's record from its live layout, seed any
/// workspace still missing a record (its stage-share
/// profile, or the legacy globals where no profile is
/// priced — see [`seed_missing_workspace_layouts`]),
/// then read the INCOMING workspace's record
/// and mirror it into the globals. Returns the incoming
/// record for [`apply_layout`]. Storing before seeding
/// is the order that makes a first-ever switch behave
/// as designed: the incoming workspace takes its own
/// seeded shape, while the outgoing one keeps the
/// shape it was actually left in.
/// Re-selecting the active workspace stores and reapplies
/// its own live layout — an identity.
pub fn switch_layouts(
    prefs: &mut LayoutPrefs,
    from: Workspace,
    to: Workspace,
    live: LiveLayout,
) -> WorkspaceLayout {
    store_record(prefs, from, live.to_record());
    seed_missing_workspace_layouts(prefs);
    let incoming = record_for(prefs, to);
    mirror_globals(prefs, incoming);
    incoming
}

/// The startup half: seed any missing records, read the
/// STARTING workspace's record (the one
/// `prefs.workspace` names) and mirror it into the
/// globals, so `init`'s existing seeds — the poll shadow,
/// `column_widths()` — read the starting workspace's own
/// shape. Returns the record for the `State`-resident
/// seeds (`show_left` / `show_right` /
/// `timeline_collapsed`). Wired into `main.rs::init` by
/// IA-W2-61-2.
pub fn startup_layout(prefs: &mut LayoutPrefs) -> WorkspaceLayout {
    seed_missing_workspace_layouts(prefs);
    let record = record_for(prefs, from_prefs(prefs));
    mirror_globals(prefs, record);
    record
}

/// Keep the ACTIVE workspace's record fresh from `poll`'s
/// layout write-back (wired in by IA-W2-61-3): captures
/// the live layout into the record of the workspace
/// `prefs.workspace` names. Only the active record is
/// ever written here, so the write-back cannot clobber
/// another workspace's memory (RM12 §4.2). A `show_right`
/// flip on its own does not trigger `poll`'s write-back
/// (the shadow watches the collapsed/sidebar pair and the
/// zoom pair); `show_right` is captured here whenever a
/// write-back does fire, and always at the switch itself
/// ([`switch_layouts`]), so its record can lag a flip by
/// at most the rest of the session in that workspace.
pub fn sync_active_record(prefs: &mut LayoutPrefs, s: &State) {
    let ws = from_name(&prefs.workspace);
    let live = live_layout(s, prefs);
    store_record(prefs, ws, live.to_record());
}

// ---- Stage focus (Roadmap Wave 2, W2-10 — RM12 §5.1) ----
//
// The transient top of the stage-share band: both side
// columns hidden, the centre — and with it the stage —
// takes the whole main row (1408 pt at 1440×900, the
// full 100% of the row; RM12 estimated ≈98.6% before
// the mount rules were this exact). A1 R4's rule, kept:
// focus is session-only — the restore lives in
// `State::stage_focus_restore` (IA-W2-10-3), never in a
// layout record or a prefs field, so a restart can
// never restore a focused stage, and
// [`select_workspace`] clears it (a switch applies the
// incoming workspace's own remembered layout, which is
// the honest shape to land on).
//
// The view needs no focus awareness: entering focus
// writes the live flags themselves (both false) and
// leaving writes the restore back, so `main.rs`'s
// existing `show_left` / `show_right` mount gates do
// the whole job. The one subtlety is capture: every
// layout capture ([`live_layout`], the single door the
// switch transaction and the poll write-back both use)
// reads visibility from the restore while focus is
// held ([`capture_visibility`]), so the workspace
// remembers its pre-focus shape.

/// The layout stage focus applies over `live`: both
/// columns hidden, widths and timeline state carried
/// untouched (they are not focus's to change — the
/// widths cannot move while their splitters are
/// unmounted, and the timeline is centre furniture).
/// Pure, so the enter/exit contract is pinnable
/// without a `State`.
pub fn focus_enter(live: LiveLayout) -> LiveLayout {
    LiveLayout {
        show_left: false,
        show_right: false,
        ..live
    }
}

/// The layout leaving stage focus restores: the
/// saved record's column visibility over the live
/// layout's everything-else — a timeline collapse the
/// user made *during* focus survives the exit, because
/// the timeline was never hidden.
pub fn focus_exit(saved: LiveLayout, live: LiveLayout) -> LiveLayout {
    LiveLayout {
        show_left: saved.show_left,
        show_right: saved.show_right,
        ..live
    }
}

/// Toggle stage focus (the View-menu door is
/// IA-W2-10-4): entering saves the live layout into
/// `State::stage_focus_restore` and hides both columns;
/// leaving restores the saved visibility. The saved
/// layout is a [`LiveLayout`], not a record — it is
/// never clamped, persisted or mirrored.
pub fn toggle_stage_focus(s: &mut State) {
    match s.stage_focus_restore.take() {
        Some(saved) => {
            let restored = focus_exit(saved, live_layout(s, &load_layout()));
            s.show_left = restored.show_left;
            s.show_right = restored.show_right;
        }
        None => {
            let live = live_layout(s, &load_layout());
            s.stage_focus_restore = Some(live);
            let focused = focus_enter(live);
            s.show_left = focused.show_left;
            s.show_right = focused.show_right;
        }
    }
}

impl Section {
    /// All 31 gated sections (coverage tests iterate
    /// this; the integrator's view maps each mounted
    /// panel to one variant).
    pub fn all() -> [Section; 31] {
        [
            Section::Figures,
            Section::Library,
            Section::RigTree,
            Section::Layers,
            Section::Project,
            Section::InspectorSegment,
            Section::InspectorFigure,
            Section::InspectorStyle,
            Section::PoseTools,
            Section::KeyframeTools,
            Section::Camera,
            Section::Scene,
            Section::Onion,
            Section::Playback,
            Section::FrameOps,
            Section::Graph,
            Section::Easing,
            Section::DrawTools,
            Section::StrokeInspector,
            Section::TextTool,
            Section::FxBrowser,
            Section::FxClips,
            Section::Particles,
            Section::Audio,
            Section::Waveform,
            Section::Clips,
            Section::Sprites,
            Section::ThreeD,
            Section::Export,
            Section::Stats,
            Section::Home,
        ]
    }
}

// ---- The gating table ------------------------------------
//
// One list per workspace. Every Section appears in at
// least one list (proven by the coverage test below);
// Animate is deliberately the richest — it is the
// default workspace and the GPUI app's Animate set was
// likewise its largest. Each list's doc comment opens
// with the one-sentence summary of what that workspace
// is for; the same sentences are exposed at runtime as
// [`Workspace::blurb`] for the help/status surfaces —
// keep the two in step.
//
// Wave 9 audit (E9) — orphan check, table vs reality:
// every (workspace, section) pair below was checked
// against the panels the workspace's scenes exercise
// and the snap suite's `workspace_card_roots_sweep`.
// No orphan mounts remain: the one historical case
// (figure-add mounting where Figures was gated off)
// is gated on `Section::Figures` at its mount site in
// `main.rs`. `Section::Figures` appearing in Draw /
// Graph / Effects / Audio / Build3D is deliberate,
// NOT an orphan: figure selection is global editor
// state, the panel's controls (select, show/hide,
// lock, reorder, opacity, delete) act from any
// workspace, and the Graph / FX / 3D panels all scope
// their editing to the figure this panel selects —
// removing it would strand those workspaces with no
// way to change the figure they edit. (The Wave 9
// scene facts 22–25 in `scenes.rs` claim no
// `figures-panel` in those workspaces; that is the
// facts' error, not this table's — the sweep pins
// `figures-panel` present in Draw and Graph.)
//
// Wave 10 audit (E8) — orphan check re-run on the
// unchanged table: the union of the eight lists is
// exactly the 29 variants of `Section::all()` (no
// uncovered section, no phantom entry, no duplicate
// — the coverage tests below pin the first and
// third), and `main.rs`'s view gates on exactly 29
// distinct `Section` variants, each exactly once, so
// every mounted panel maps to a section and every
// section has its one mount site (figure-add rides
// the Figures gate). The Alt+1..8 order is intact:
// `main.rs`'s `on_key` indexes `Workspace::all()`
// with `n − 1` and the chip suffix is the position
// in the same array + 1, both pinned by
// `all_eight_workspaces_in_row_order`. The timeline
// card's lanes are chrome, not sections — see the
// module header; the Wave 10 dopesheet lane changes
// nothing in this table.
//
// Wave 13 (E8) — the Sprites panel gets its home:
// `Section::Sprites` is added to Animate ONLY, taking
// the union to exactly the 30 variants of
// `Section::all()` and Animate's list to 17 (the
// per-workspace counts are pinned by
// `section_counts_per_workspace_are_pinned`). The
// decision, argued from this table's own precedents:
// sprites are asset-backed stage content edited by
// KEYFRAMING — the panel's per-key steppers (x, y,
// rotation, scale) write instance keys at the current
// frame, so the panel belongs with the playhead-centred
// editing family (KeyframeTools, FrameOps, Playback),
// all Animate-only or Animate-first here. The closest
// analogue already in the table is `Section::Clips`:
// movieclips ride in Animate, NOT in their thematic
// workspace (Effects), on GPUI precedent — its
// "Library & Clips" group was an Animate/Build group.
// Sprites are the same shape of thing (library/asset-
// backed, keyframed, scene-anchored) and follow the
// clips home, not the thematic pull. The counter-
// precedent is `Section::TextTool` in Draw, and it
// does NOT carry: text lives in Draw because the text
// tool is a DRAWING tool — its instances are authored
// by drawing with it, alongside strokes and shapes
// (Draw's blurb: "Draw freehand strokes, shapes and
// text over the figures"), and the panel is the tool's
// own inspector. Sprites have no drawing tool: they
// are placed from the project's asset table and then
// keyframed, so placement + keyframing, not drawing,
// decides the home. Draw was the alternative argued
// and rejected on exactly that ground; the spot checks
// pin Sprites absent from Draw so the decision cannot
// drift silently. The Scene panel's placed-media
// readout (Wave 13 A7) points at this panel, and Scene
// itself mounts in Animate, so the pointer's primary
// home and the panel's home agree. Animate's blurb is
// NOT extended to name sprites: it does not name
// movieclips either — Clips was homed here without a
// blurb change, and the blurb strings stay pinned as
// they are. Integrator wiring still owed outside this
// file (not this table's truth): the `sprites_panel`
// module declaration, the panel's `State` field, and
// the one mount site in `main.rs`'s view gated on
// `Sec::Sprites` through the same `vis` closure the
// other 29 mounts use — until that lands, this entry
// is the gating truth the mount must follow, and the
// coverage test below already requires Sprites to be
// visible somewhere, which Animate satisfies.
//
// Wave 2 (W2-21) — the Build split (roadmap P-8, RM19
// §2): `Workspace::Build` is inserted before Build3D in
// `Workspace::all()` (the D-RM19-1 INSERT resolution —
// see the module header), taking the workspace count
// 8 → 9 and renumbering the Alt chords behind the
// insertion (3D Alt+6 → Alt+7, Export Alt+7 → Alt+8,
// Home Alt+8 → Alt+9; the `main.rs` Alt range and the
// keymap/palette bindings move by IA-W2-21-1/4).
// `Section::RigTree` joins `Section::all()` (30 → 31)
// and the union of the nine lists is exactly those 31
// variants (50 list memberships). Build's own list is
// the rig set: Figures, Library, RigTree, the three
// inspectors and Project — all of which also stay in
// Animate's list, so no section loses a home; Build3D's
// list is unchanged, which is what makes the split
// honest ("3D" now names only the 3D set). Reversing
// D-RM19-1 (append instead of insert) is a table-only
// change: move `Workspace::Build` to the end of `all()`
// and re-move the same pins.

/// Animate: the core animating set — figures, library,
/// layers, project, all three inspectors, pose and
/// keyframe tools, camera, scene, onion, playback,
/// frame ops, movieclips, sprites and stats. Movieclips
/// ride here (not Effects) on GPUI precedent: its
/// "Library & Clips" group was an Animate/Build group,
/// and Effects' clips are the FX clips. Sprites ride
/// here on the same precedent — asset-backed, keyframed
/// scene content follows the clips home, not the
/// thematic pull (see the Wave 13 note above the table
/// for the full argument and the TextTool distinction).
const ANIMATE_SECTIONS: &[Section] = &[
    Section::Figures,
    Section::Library,
    Section::Layers,
    Section::Project,
    Section::InspectorSegment,
    Section::InspectorFigure,
    Section::InspectorStyle,
    Section::PoseTools,
    Section::KeyframeTools,
    Section::Camera,
    Section::Scene,
    Section::Onion,
    Section::Playback,
    Section::FrameOps,
    Section::Clips,
    Section::Sprites,
    Section::Stats,
];

/// Draw: the drawing set over the figures it draws on.
const DRAW_SECTIONS: &[Section] = &[
    Section::DrawTools,
    Section::StrokeInspector,
    Section::TextTool,
    Section::Figures,
];

/// Graph: curve editing — graph, easing, keyframe tools.
const GRAPH_SECTIONS: &[Section] = &[
    Section::Graph,
    Section::Easing,
    Section::KeyframeTools,
    Section::Figures,
];

/// Effects: FX browser + FX clips, the scene they play
/// in, and the figures they anchor to.
const EFFECTS_SECTIONS: &[Section] = &[
    Section::FxBrowser,
    Section::FxClips,
    Section::Particles,
    Section::Scene,
    Section::Figures,
];

/// Audio: clips, waveform, playback to hear them
/// against, and the figures being timed.
const AUDIO_SECTIONS: &[Section] = &[
    Section::Audio,
    Section::Waveform,
    Section::Playback,
    Section::Figures,
];

/// Build: the rig-construction set (Wave 2, W2-21 —
/// RM19 §2) — the figures, their library, the rig tree,
/// all three inspectors that edit the rig, and the
/// project tools that name/save it. Every entry also
/// rides Animate (which is unchanged); Build is an
/// additional, rigging-focused home, not a move.
const BUILD_SECTIONS: &[Section] = &[
    Section::Figures,
    Section::Library,
    Section::RigTree,
    Section::InspectorFigure,
    Section::InspectorSegment,
    Section::InspectorStyle,
    Section::Project,
];

/// 3D: the 3D panel, the figures it poses, and the
/// camera it frames with.
const BUILD3D_SECTIONS: &[Section] = &[
    Section::ThreeD,
    Section::Figures,
    Section::Camera,
];

/// Export: the export centre, project stats for the
/// summary, and the project tools that name/save it.
const EXPORT_SECTIONS: &[Section] = &[
    Section::Export,
    Section::Stats,
    Section::Project,
];

/// Home: the projects home panel, project tools, stats.
const HOME_SECTIONS: &[Section] = &[
    Section::Home,
    Section::Project,
    Section::Stats,
];

/// The sections mounted in `ws`, in panel order.
pub fn sections_of(ws: Workspace) -> &'static [Section] {
    match ws {
        Workspace::Animate => ANIMATE_SECTIONS,
        Workspace::Draw => DRAW_SECTIONS,
        Workspace::Graph => GRAPH_SECTIONS,
        Workspace::Effects => EFFECTS_SECTIONS,
        Workspace::Audio => AUDIO_SECTIONS,
        Workspace::Build => BUILD_SECTIONS,
        Workspace::Build3D => BUILD3D_SECTIONS,
        Workspace::Export => EXPORT_SECTIONS,
        Workspace::Home => HOME_SECTIONS,
    }
}

/// The gating question the integrator's view asks per
/// panel: is `section` mounted in workspace `ws`?
pub fn section_visible(ws: Workspace, section: Section) -> bool {
    sections_of(ws).contains(&section)
}

/// The line [`workspace_row`] shows under the chips:
/// the active workspace's own [`Workspace::blurb`].
/// Factored out of the builder so the row's text source
/// is unit-testable without mounting the shell.
pub fn blurb_line(ws: Workspace) -> &'static str {
    ws.blurb()
}

/// The workspace row: one chip per workspace, keyed
/// `ws-*`, the active workspace rendered active. A press
/// runs [`select_workspace`] (switch + persist). Mount
/// it as its own row under the menu bar.
///
/// Each chip's text carries its keyboard shortcut as a
/// quiet suffix — `Animate · Alt+1` … `Home · Alt+9`,
/// matching the `Alt+1..9` bindings in `main.rs`'s
/// `on_key` (chip position in [`Workspace::all`] + 1).
/// The suffix is composed here, at the call site, rather
/// than folded into [`Workspace::label`]: `label()` is
/// also the persisted name and is pinned by tests, and
/// the kit's `btn` renders a single plain text run — it
/// has no muted/secondary span — so the middle-dot
/// suffix in the same label is the least noisy rendering
/// available.
///
/// Wave Q (i18n wiring): the name part of each chip now
/// resolves through the i18n table via
/// [`Workspace::display_label`], so the row follows the
/// shell's current language; `label()` itself is
/// unchanged (it is the persisted name, and persistence
/// must never vary with the UI language). The
/// `· Alt+N` suffix and the blurb line under the chips
/// stay as they were — the catalogue carries no blurb
/// keys.
///
/// Under the chips, a single muted line (key `ws-blurb`)
/// shows the ACTIVE workspace's [`Workspace::blurb`] —
/// that method's consumer. The root element keeps the
/// `workspace-row` key it has always carried; it is now
/// a column (chips row, keyed `workspace-chips`, over
/// the blurb line) instead of the bare chips row, which
/// is transparent to the mount site in `main.rs` (it
/// mounts the returned element as one child). The blurb
/// line `.truncate()`s — Rune's one-line mode, the same
/// overflow handling the status strip's tail segment
/// uses — so a narrow window elides it with "…" instead
/// of letting it wrap or push the layout wider.
///
/// UI mission Wave 3: the chips were the shell's last
/// text-only chip row; each now leads with the
/// `border-all` grid glyph through `icon_btn` — the
/// app-grid mark the shell's curated icon set carries
/// for a grid of destinations. Key (`ws-*`), label text
/// (shortcut suffix included), the active state and the
/// press handler are exactly the ones the plain `btn`
/// carried; `icon_btn` itself falls back to that same
/// `btn` if the glyph ever leaves the curated set.
pub fn workspace_row(s: &State, t: &Theme) -> El {
    let mut chips = row()
        .key("workspace-chips")
        .gap(design::PANEL_GAP)
        .center_y();
    for (i, ws) in Workspace::all().into_iter().enumerate() {
        let chip_label = format!("{} · Alt+{}", ws.display_label(&s.core), i + 1);
        chips = chips.child(icon_btn(
            ws.key(),
            "border-all",
            &chip_label,
            s.workspace == ws,
            t,
            move |s: &mut State| {
                select_workspace(s, ws);
            },
        ));
    }
    col()
        .key("workspace-row")
        .gap(2.0)
        .child(chips)
        .child(
            text(blurb_line(s.workspace))
                .key("ws-blurb")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .truncate(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_nine_workspaces_in_row_order() {
        // Wave 2 (W2-21): Build inserted before Build3D
        // (D-RM19-1 INSERT) — the array IS the chip order
        // and the Alt+number order, so this pin carries
        // the renumber (3D 7, Export 8, Home 9).
        assert_eq!(
            Workspace::all(),
            [
                Workspace::Animate,
                Workspace::Draw,
                Workspace::Graph,
                Workspace::Effects,
                Workspace::Audio,
                Workspace::Build,
                Workspace::Build3D,
                Workspace::Export,
                Workspace::Home,
            ]
        );
    }

    #[test]
    fn labels_names_and_keys_are_unique() {
        let all = Workspace::all();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.label(), b.label());
                assert_ne!(a.name(), b.name());
                assert_ne!(a.key(), b.key());
            }
        }
    }

    #[test]
    fn every_workspace_has_a_unique_one_sentence_blurb() {
        let all = Workspace::all();
        for (i, ws) in all.iter().enumerate() {
            let b = ws.blurb();
            assert!(!b.is_empty(), "{ws:?} has an empty blurb");
            assert!(b.ends_with('.'), "{ws:?} blurb is not one sentence: {b}");
            for other in &all[i + 1..] {
                assert_ne!(b, other.blurb(), "blurbs must be unique");
            }
        }
    }

    #[test]
    fn blurb_line_is_the_workspaces_own_blurb() {
        // The helper the chip row renders must be the
        // workspace's blurb itself — non-empty for every
        // variant, so the line under the chips can never
        // come up blank.
        for ws in Workspace::all() {
            assert_eq!(blurb_line(ws), ws.blurb());
            assert!(!blurb_line(ws).is_empty(), "{ws:?} blurb line is empty");
        }
    }

    #[test]
    fn blurb_strings_are_pinned() {
        // The exact sentences shown under the chip row.
        // Pinning them means a wording edit is a
        // deliberate test change, never a silent one —
        // and an emptied blurb fails here even if the
        // non-empty checks above are ever relaxed.
        assert_eq!(
            blurb_line(Workspace::Animate),
            "Pose figures and build the animation frame by frame, with the inspectors, camera, playback and frame tools."
        );
        assert_eq!(
            blurb_line(Workspace::Draw),
            "Draw freehand strokes, shapes and text over the figures."
        );
        assert_eq!(
            blurb_line(Workspace::Graph),
            "Edit the motion curves and easing of the selected figure's keys."
        );
        assert_eq!(
            blurb_line(Workspace::Effects),
            "Add and edit particle and visual-effects clips anchored to the scene."
        );
        assert_eq!(
            blurb_line(Workspace::Audio),
            "Place sound clips and line them up against the animation."
        );
        assert_eq!(
            blurb_line(Workspace::Build),
            "Build and edit figure rigs: the node tree, segment properties, joint limits and the setup pose."
        );
        assert_eq!(
            blurb_line(Workspace::Build3D),
            "Pose the 3D figure and frame it with the camera."
        );
        assert_eq!(
            blurb_line(Workspace::Export),
            "Render and export the project to images, video and other formats."
        );
        assert_eq!(
            blurb_line(Workspace::Home),
            "Open, create and manage projects, and check project stats."
        );
    }

    #[test]
    fn build3d_is_labelled_and_persisted_as_3d() {
        assert_eq!(Workspace::Build3D.label(), "3D");
        assert_eq!(Workspace::Build3D.name(), "3D");
        assert_eq!(Workspace::Build3D.key(), "ws-3d");
    }

    #[test]
    fn build_is_labelled_and_persisted_as_build() {
        // Wave 2 (W2-21): the split workspace carries its
        // own label/persisted name/element key — none of
        // them is Build3D's, so prefs and chips can never
        // confuse the two rigging workspaces.
        assert_eq!(Workspace::Build.label(), "Build");
        assert_eq!(Workspace::Build.name(), "Build");
        assert_eq!(Workspace::Build.key(), "ws-build");
    }

    #[test]
    fn ws_build_i18n_key_is_reused() {
        // RM19 §7 / W2-21: Build's chip resolves through
        // the Wave 14 workspace key reserved for it,
        // `ws_build` — no new key family is invented.
        // (The table entry itself is IA-W2-21-3; the
        // catalogue pin in
        // `workspace_i18n_keys_exist_in_the_table` covers
        // its presence there.)
        assert_eq!(Workspace::Build.i18n_key(), "ws_build");
        assert_eq!(Workspace::Build3D.i18n_key(), "ws_3d");
    }

    #[test]
    fn build_workspace_lists_the_rig_sections() {
        // RM19 §2's list, exactly: the rig set, with the
        // rig tree among them.
        assert_eq!(
            sections_of(Workspace::Build),
            [
                Section::Figures,
                Section::Library,
                Section::RigTree,
                Section::InspectorFigure,
                Section::InspectorSegment,
                Section::InspectorStyle,
                Section::Project,
            ]
            .as_slice()
        );
    }

    #[test]
    fn build3d_keeps_only_its_three_sections() {
        // The split is honest only if Build3D's list did
        // not grow a rig section on the way.
        assert_eq!(
            sections_of(Workspace::Build3D),
            [Section::ThreeD, Section::Figures, Section::Camera].as_slice()
        );
        assert!(!section_visible(Workspace::Build3D, Section::RigTree));
        assert!(!section_visible(Workspace::Build, Section::ThreeD));
    }

    #[test]
    fn entering_build_enters_setup_mode_and_leaving_restores() {
        // RM19 §2's plumbing, over the pure helper
        // `select_workspace` applies: entering saves the
        // prior flag and turns setup on; leaving restores
        // the saved value — in both prior states.
        let mut saved = false;
        // Enter with setup off: saved 0, now on.
        assert!(setup_mode_after_switch(false, true, false, &mut saved));
        assert!(!saved);
        // Leave: restored to the saved off.
        assert!(!setup_mode_after_switch(true, false, true, &mut saved));
        // Enter with setup already on: saved on, stays on.
        assert!(setup_mode_after_switch(false, true, true, &mut saved));
        assert!(saved);
        // Leave: restored to on.
        assert!(setup_mode_after_switch(true, false, false, &mut saved));
        // Non-Build switches never touch the flag or the
        // saved value, and re-selecting Build while in
        // Build does not re-save over the original.
        let mut saved2 = true;
        assert!(!setup_mode_after_switch(false, false, false, &mut saved2));
        assert!(saved2);
        let mut saved3 = false;
        assert!(setup_mode_after_switch(true, true, true, &mut saved3));
        assert!(!saved3);
    }

    #[test]
    fn workspace_i18n_keys_exist_in_the_table() {
        // Every chip's translation key must resolve in the
        // catalogue — a missing key would render the key
        // text itself in the row.
        for ws in Workspace::all() {
            assert!(
                i18n_wire::key_in_table(ws.i18n_key()),
                "{} ({}) not in the i18n table",
                ws.i18n_key(),
                ws.label()
            );
        }
    }

    #[test]
    fn english_display_labels_match_the_persisted_labels() {
        // English is the shell's default language, and the
        // persisted name is the English label — the rendered
        // chip text must not change for English users.
        let core = EditorCore::new();
        for ws in Workspace::all() {
            assert_eq!(ws.display_label(&core), ws.label(), "{ws:?}");
        }
    }

    #[test]
    fn display_labels_follow_the_core_language() {
        use stykanimate_rs::i18n::Lang;
        let mut core = EditorCore::new();
        core.lang = Lang::Bn;
        assert_eq!(Workspace::Animate.display_label(&core), "অ্যানিমেট");
        assert_eq!(Workspace::Export.display_label(&core), "এক্সপোর্ট");
        assert_eq!(Workspace::Home.display_label(&core), "হোম");
        // `ws_3d` reads "3D" in every table.
        assert_eq!(Workspace::Build3D.display_label(&core), "3D");
        // `ws_build` in Bengali — rides IA-W2-21-3's
        // splice (the value pinned here is the IA's).
        assert_eq!(Workspace::Build.display_label(&core), "বিল্ড");
        core.lang = Lang::De;
        assert_eq!(Workspace::Animate.display_label(&core), "Animieren");
        assert_eq!(Workspace::Home.display_label(&core), "Startseite");
        core.lang = Lang::ZhHans;
        assert_eq!(Workspace::Audio.display_label(&core), "音频");
        // The persisted name never follows the language.
        assert_eq!(Workspace::Home.name(), "Home");
        assert_eq!(Workspace::Animate.name(), "Animate");
    }

    #[test]
    fn from_name_roundtrips_every_workspace() {
        for ws in Workspace::all() {
            assert_eq!(from_name(ws.name()), ws);
        }
    }

    #[test]
    fn from_name_falls_back_to_animate() {
        assert_eq!(from_name(""), Workspace::Animate);
        assert_eq!(from_name("bogus"), Workspace::Animate);
        // "Build" is NOT a fallback case since the
        // Wave 2 split: it names Workspace::Build
        // exactly (round-trip pinned above).
        assert_eq!(from_name("Build"), Workspace::Build);
        // Names are exact: case matters.
        assert_eq!(from_name("animate"), Workspace::Animate);
        assert_eq!(from_name("Animate"), Workspace::Animate);
    }

    #[test]
    fn default_workspace_is_animate_like_layout_prefs() {
        assert_eq!(Workspace::default(), Workspace::Animate);
        assert_eq!(LayoutPrefs::default().workspace, "Animate");
        assert_eq!(from_prefs(&LayoutPrefs::default()), Workspace::Animate);
    }

    #[test]
    fn prefs_with_workspace_preserves_other_fields() {
        // Non-default values in every field, including
        // the column widths (`col_left_w` / `col_right_w`,
        // added to `LayoutPrefs` in the Wave 9 shell
        // work): the load-modify-save behind a workspace
        // switch must carry them across untouched, or
        // every chip press would reset the user's column
        // arrangement. The values sit inside the load
        // clamps (220..=420 / 240..=480 since W2-10) so
        // the disk round-trip test below sees them as
        // written.
        let prefs = LayoutPrefs {
            workspace: "Animate".to_string(),
            dark: false,
            lang: stykanimate_rs::i18n::Lang::Bn,
            timeline_collapsed: true,
            timeline_zoom: 30.0,
            timeline_follow: false,
            sidebar_visible: false,
            palette_recents: Vec::new(),
            live_mcp: false,
            autosave_enabled: true,
            autosave_every: 25,
            recovery_keep: 1,
            col_left_w: 260.0,
            col_right_w: 400.0,
            workspace_layouts: Vec::new(),
            export_dir: String::new(),
            export_naming: "project".to_string(),
            export_quality: "standard".to_string(),
        };
        let out = prefs_with_workspace(&prefs, Workspace::Audio);
        assert_eq!(out.workspace, "Audio");
        assert!(!out.dark);
        assert_eq!(out.lang, stykanimate_rs::i18n::Lang::Bn);
        assert!(out.timeline_collapsed);
        assert!(!out.sidebar_visible);
        assert!(!out.live_mcp);
        assert_eq!(out.col_left_w, 260.0);
        assert_eq!(out.col_right_w, 400.0);
        assert_eq!(from_prefs(&out), Workspace::Audio);
    }

    #[test]
    fn persisted_workspace_survives_a_disk_roundtrip() {
        // Uses shell's explicit-dir API so the test
        // never touches the real ./projects prefs.
        let dir = std::env::temp_dir().join(format!(
            "styka-workspaces-test-{}",
            std::process::id()
        ));
        let prefs = prefs_with_workspace(&LayoutPrefs::default(), Workspace::Graph);
        stykanimate_rs::shell::save_layout_in(&dir, &prefs);
        let loaded = stykanimate_rs::shell::load_layout_in(&dir);
        assert_eq!(from_prefs(&loaded), Workspace::Graph);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_section_is_visible_in_some_workspace() {
        for section in Section::all() {
            assert!(
                Workspace::all()
                    .into_iter()
                    .any(|ws| section_visible(ws, section)),
                "section {section:?} is visible in no workspace"
            );
        }
    }

    #[test]
    fn section_counts_per_workspace_are_pinned() {
        // The census this file maintains, pinned exactly
        // so a gating change is always a deliberate test
        // change: 31 sections in `Section::all()`, and
        // the per-workspace list lengths below (50 list
        // memberships in total — sections legitimately
        // repeat ACROSS workspaces; the no-duplicates
        // test pins them unique WITHIN one list). Wave
        // 13: Animate 16 → 17 for `Section::Sprites`
        // (Animate-only; see the Wave 13 note above the
        // table). Wave 2 (W2-21): Build joins with 7
        // (the RM19 rig set) and `Section::RigTree`
        // takes `Section::all()` 30 → 31.
        use Workspace as W;
        assert_eq!(Section::all().len(), 31);
        assert_eq!(sections_of(W::Animate).len(), 17);
        assert_eq!(sections_of(W::Draw).len(), 4);
        assert_eq!(sections_of(W::Graph).len(), 4);
        assert_eq!(sections_of(W::Effects).len(), 5);
        assert_eq!(sections_of(W::Audio).len(), 4);
        assert_eq!(sections_of(W::Build).len(), 7);
        assert_eq!(sections_of(W::Build3D).len(), 3);
        assert_eq!(sections_of(W::Export).len(), 3);
        assert_eq!(sections_of(W::Home).len(), 3);
        let total: usize = Workspace::all()
            .into_iter()
            .map(|ws| sections_of(ws).len())
            .sum();
        assert_eq!(total, 50);
    }

    #[test]
    fn section_lists_have_no_duplicates() {
        for ws in Workspace::all() {
            let list = sections_of(ws);
            for (i, a) in list.iter().enumerate() {
                assert!(!list[i + 1..].contains(a), "{ws:?} lists {a:?} twice");
            }
        }
    }

    #[test]
    fn animate_is_the_richest_workspace() {
        let animate = sections_of(Workspace::Animate).len();
        for ws in Workspace::all() {
            if ws != Workspace::Animate {
                assert!(
                    animate > sections_of(ws).len(),
                    "Animate ({animate}) should out-list {ws:?} ({})",
                    sections_of(ws).len()
                );
            }
        }
    }

    #[test]
    fn gating_spot_checks() {
        use Section as S;
        use Workspace as W;
        // Animate carries the core set.
        assert!(section_visible(W::Animate, S::Figures));
        assert!(section_visible(W::Animate, S::InspectorStyle));
        assert!(section_visible(W::Animate, S::FrameOps));
        assert!(section_visible(W::Animate, S::Clips));
        assert!(!section_visible(W::Animate, S::Graph));
        assert!(!section_visible(W::Animate, S::ThreeD));
        // Draw is drawing-only (plus figures).
        assert!(section_visible(W::Draw, S::DrawTools));
        assert!(section_visible(W::Draw, S::StrokeInspector));
        assert!(section_visible(W::Draw, S::TextTool));
        assert!(!section_visible(W::Draw, S::Graph));
        // Graph workspace.
        assert!(section_visible(W::Graph, S::Graph));
        assert!(section_visible(W::Graph, S::Easing));
        assert!(section_visible(W::Graph, S::KeyframeTools));
        assert!(!section_visible(W::Graph, S::DrawTools));
        // Effects: FX, not movieclips.
        assert!(section_visible(W::Effects, S::FxBrowser));
        assert!(section_visible(W::Effects, S::FxClips));
        assert!(section_visible(W::Effects, S::Scene));
        assert!(!section_visible(W::Effects, S::Clips));
        // Audio.
        assert!(section_visible(W::Audio, S::Audio));
        assert!(section_visible(W::Audio, S::Waveform));
        assert!(section_visible(W::Audio, S::Playback));
        assert!(!section_visible(W::Audio, S::FxBrowser));
        // 3D.
        assert!(section_visible(W::Build3D, S::ThreeD));
        assert!(section_visible(W::Build3D, S::Camera));
        assert!(!section_visible(W::Build3D, S::Export));
        // Build (Wave 2, W2-21): the rig set — rig tree,
        // library, the inspectors, project; NOT the 3D
        // panel, NOT the animation-timeline family.
        assert!(section_visible(W::Build, S::RigTree));
        assert!(section_visible(W::Build, S::Library));
        assert!(section_visible(W::Build, S::InspectorSegment));
        assert!(section_visible(W::Build, S::InspectorFigure));
        assert!(section_visible(W::Build, S::InspectorStyle));
        assert!(section_visible(W::Build, S::Project));
        assert!(!section_visible(W::Build, S::ThreeD));
        assert!(!section_visible(W::Build, S::FrameOps));
        assert!(!section_visible(W::Build, S::Graph));
        // The rig tree lives ONLY in Build.
        for ws in [W::Animate, W::Draw, W::Graph, W::Effects, W::Audio, W::Build3D, W::Export, W::Home] {
            assert!(!section_visible(ws, S::RigTree), "{ws:?} must not mount RigTree");
        }
        // Export + Home.
        assert!(section_visible(W::Export, S::Export));
        assert!(section_visible(W::Export, S::Project));
        assert!(section_visible(W::Export, S::Stats));
        assert!(section_visible(W::Home, S::Home));
        assert!(section_visible(W::Home, S::Stats));
        assert!(!section_visible(W::Home, S::Figures));
        // Wave 9 audit (E9): the Figures row across all
        // eight — mounted wherever a workspace's work
        // scopes to the selected figure (Animate, Draw,
        // Graph, Effects, Audio, Build3D), gated out
        // only where nothing edits figures (Export,
        // Home). See the gating-table comment for why
        // these mounts are legitimate, not orphans.
        for ws in [W::Animate, W::Draw, W::Graph, W::Effects, W::Audio, W::Build, W::Build3D] {
            assert!(section_visible(ws, S::Figures), "{ws:?} mounts Figures");
        }
        for ws in [W::Export, W::Home] {
            assert!(!section_visible(ws, S::Figures), "{ws:?} gates Figures out");
        }
        // FrameOps is Animate-only: the sweep pins the
        // `frames` root absent in Draw and Graph alike.
        assert!(section_visible(W::Animate, S::FrameOps));
        assert!(!section_visible(W::Draw, S::FrameOps));
        assert!(!section_visible(W::Graph, S::FrameOps));
        // Wave 13 (E8): Sprites is Animate-only — the
        // clips precedent, NOT the text precedent (see
        // the Wave 13 note above the table). Draw is
        // pinned absent deliberately: it was the argued
        // alternative home, and this assert is what
        // keeps the rejection from drifting.
        assert!(section_visible(W::Animate, S::Sprites));
        for ws in [W::Draw, W::Graph, W::Effects, W::Audio, W::Build, W::Build3D, W::Export, W::Home] {
            assert!(!section_visible(ws, S::Sprites), "{ws:?} must not mount Sprites");
        }
    }

    // ---- Layout memory (W2-61 — PL-10) -------------------

    fn live(col_left_w: f32, col_right_w: f32, show_left: bool, show_right: bool, collapsed: bool) -> LiveLayout {
        LiveLayout {
            col_left_w,
            col_right_w,
            show_left,
            show_right,
            timeline_collapsed: collapsed,
        }
    }

    #[test]
    fn layout_memory_first_switch_seeds_every_workspace() {
        // Fresh prefs (no records): one switch stores the
        // outgoing workspace's live shape, seeds all nine
        // records, and hands back the incoming workspace's
        // record — since W2-10's stage-share re-base, its
        // priced profile (Graph: 260 / 300), not the
        // legacy globals W2-61 originally seeded; a
        // factory file carries no user arrangement the
        // legacy seed would be protecting.
        let mut prefs = LayoutPrefs::default();
        let left = live(260.0, 400.0, false, false, true);
        let incoming = switch_layouts(&mut prefs, Workspace::Animate, Workspace::Graph, left);
        assert_eq!(prefs.workspace_layouts.len(), 9);
        assert_eq!(record_for(&prefs, Workspace::Animate), left.to_record());
        let graph_profile = WorkspaceLayout {
            col_left_w: 260.0,
            col_right_w: 300.0,
            workspace_layouts: Vec::new(),
            export_dir: String::new(),
            export_naming: "project".to_string(),
            export_quality: "standard".to_string(),
            show_left: true,
            show_right: true,
            timeline_collapsed: false,
        };
        assert_eq!(incoming, graph_profile);
        assert_eq!(record_for(&prefs, Workspace::Graph), graph_profile);
        // The globals mirror the now-active workspace.
        assert_eq!(prefs.col_left_w, 260.0);
        assert_eq!(prefs.col_right_w, 300.0);
        assert!(prefs.sidebar_visible);
        assert!(!prefs.timeline_collapsed);
    }

    #[test]
    fn layout_memory_round_trips_per_workspace() {
        // RM12 §4.4's snap sketch, over the pure
        // transaction: Animate gets one shape, Graph
        // another; switching back and forth restores each
        // workspace's own and keeps the other's.
        let mut prefs = LayoutPrefs::default();
        let animate = live(260.0, 400.0, false, true, true);
        let graph = live(220.0, 260.0, true, false, false);
        let got = switch_layouts(&mut prefs, Workspace::Animate, Workspace::Graph, animate);
        // Graph's first landing is its seeded shape —
        // since W2-10, the stage-share profile, not the
        // factory legacy record this assert originally
        // pinned.
        assert_eq!(got, default_layout_for(Workspace::Graph).unwrap());
        let got = switch_layouts(&mut prefs, Workspace::Graph, Workspace::Animate, graph);
        assert_eq!(got, animate.to_record());
        let got = switch_layouts(&mut prefs, Workspace::Animate, Workspace::Graph, animate);
        assert_eq!(got, graph.to_record());
        assert_eq!(record_for(&prefs, Workspace::Animate), animate.to_record());
        assert_eq!(record_for(&prefs, Workspace::Graph), graph.to_record());
    }

    #[test]
    fn layout_memory_reselecting_active_workspace_is_identity() {
        let mut prefs = LayoutPrefs::default();
        let current = live(280.0, 360.0, true, true, false);
        let got = switch_layouts(&mut prefs, Workspace::Draw, Workspace::Draw, current);
        assert_eq!(got, current.to_record());
        assert_eq!(record_for(&prefs, Workspace::Draw), current.to_record());
    }

    #[test]
    fn layout_memory_record_widths_clamp_on_read() {
        // A hand-edited record cannot squeeze the stage
        // away: reads clamp to the loader's ranges, and a
        // non-finite width falls back to the field
        // default (the `clamp_col_width` contract).
        let mut prefs = LayoutPrefs::default();
        store_record(
            &mut prefs,
            Workspace::Audio,
            WorkspaceLayout {
                col_left_w: 9999.0,
                col_right_w: f32::NAN,
                workspace_layouts: Vec::new(),
                export_dir: String::new(),
                export_naming: "project".to_string(),
                export_quality: "standard".to_string(),
                show_left: true,
                show_right: true,
                timeline_collapsed: false,
            },
        );
        let record = record_for(&prefs, Workspace::Audio);
        assert_eq!(record.col_left_w, 420.0);
        assert_eq!(record.col_right_w, 340.0);
    }

    #[test]
    fn layout_memory_missing_record_falls_back_to_legacy() {
        let mut prefs = LayoutPrefs::default();
        prefs.col_left_w = 280.0;
        prefs.sidebar_visible = false;
        store_record(
            &mut prefs,
            Workspace::Animate,
            WorkspaceLayout {
                col_left_w: 240.0,
                col_right_w: 300.0,
                workspace_layouts: Vec::new(),
                export_dir: String::new(),
                export_naming: "project".to_string(),
                export_quality: "standard".to_string(),
                show_left: true,
                show_right: false,
                timeline_collapsed: true,
            },
        );
        // Graph has no record: the legacy globals answer,
        // with `show_right` shown (it has no legacy
        // mirror and always started shown).
        let fallback = record_for(&prefs, Workspace::Graph);
        assert_eq!(fallback.col_left_w, 280.0);
        assert!(!fallback.show_left);
        assert!(fallback.show_right);
        assert!(!fallback.timeline_collapsed);
    }

    #[test]
    fn layout_memory_startup_seeds_and_mirrors() {
        // A prefs file naming Audio, with its own global
        // shape and no records: startup returns Audio's
        // record — seeded from the globals, because a
        // customised legacy layout protects its active
        // workspace (W2-10) and Audio is it — and the
        // globals already mirror it, so the shell's
        // existing seeds read the same values. (The
        // other eight seed from their profiles; this
        // test does not pin them.)
        let mut prefs = LayoutPrefs::default();
        prefs.workspace = "Audio".to_string();
        prefs.col_left_w = 280.0;
        prefs.col_right_w = 360.0;
        prefs.sidebar_visible = false;
        prefs.timeline_collapsed = true;
        let record = startup_layout(&mut prefs);
        assert_eq!(prefs.workspace_layouts.len(), 9);
        assert_eq!(record.col_left_w, 280.0);
        assert_eq!(record.col_right_w, 360.0);
        assert!(!record.show_left);
        assert!(record.show_right);
        assert!(record.timeline_collapsed);
        assert_eq!(prefs.col_left_w, 280.0);
        assert!(!prefs.sidebar_visible);
        // An unknown persisted name falls back to
        // Animate's record, like `from_prefs`.
        let mut prefs = LayoutPrefs::default();
        prefs.workspace = "bogus".to_string();
        let record = startup_layout(&mut prefs);
        assert_eq!(record, record_for(&prefs, Workspace::Animate));
    }

    // ---- Stage-share recovery (W2-10) ----------------

    #[test]
    fn stage_share_profiles_match_rm12_pricing() {
        // RM12 §5.3's table, value for value. Build is
        // the one derivation: RM12 priced eight
        // workspaces, so Build inherits Build3D's
        // 260 / 320 (see `default_layout_for`).
        let priced = [
            (Workspace::Animate, 220.0, 240.0),
            (Workspace::Draw, 220.0, 300.0),
            (Workspace::Graph, 260.0, 300.0),
            (Workspace::Effects, 260.0, 320.0),
            (Workspace::Audio, 280.0, 320.0),
            (Workspace::Build, 260.0, 320.0),
            (Workspace::Build3D, 260.0, 320.0),
            (Workspace::Export, 260.0, 340.0),
        ];
        for (ws, left, right) in priced {
            let profile = default_layout_for(ws).expect("priced profile");
            assert_eq!(profile.col_left_w, left, "{ws:?} left");
            assert_eq!(profile.col_right_w, right, "{ws:?} right");
            // Profiles show both columns with the
            // timeline expanded — the historical
            // defaults; only widths are profiled.
            assert!(profile.show_left && profile.show_right, "{ws:?}");
            assert!(!profile.timeline_collapsed, "{ws:?}");
            // Every profile width sits inside the load
            // clamps (left 220..=420, right 240..=480
            // since the §4.3.2 decision), so a profile
            // can never be clamped away on read.
            assert!((220.0..=420.0).contains(&profile.col_left_w), "{ws:?}");
            assert!((240.0..=480.0).contains(&profile.col_right_w), "{ws:?}");
        }
        // Home has no priced profile (RM12 awaited the
        // centre takeover) and keeps the legacy seed.
        assert_eq!(default_layout_for(Workspace::Home), None);
        // The band check, in the shell's own arithmetic:
        // at 1440×900 the inner row is 1408 pt; the
        // centre takes what the columns and the two
        // 12 pt splitter junctions leave. Animate's
        // profile centre is 924 pt = 65.6% — inside the
        // 65–80% band, past the 915 pt the band's floor
        // requires.
        let animate = default_layout_for(Workspace::Animate).unwrap();
        let centre = 1408.0 - animate.col_left_w - animate.col_right_w - 24.0;
        assert_eq!(centre, 924.0);
        assert!(centre / 1408.0 >= 0.65, "Animate centre share in band");
        assert!(centre / 1408.0 <= 0.80, "Animate centre share in band");
    }

    #[test]
    fn layout_memory_seeding_uses_profiles_on_a_factory_file() {
        // A factory file (no records, globals at the
        // long-standing defaults, Animate active):
        // startup seeds every workspace from its
        // profile; Home, unpriced, keeps the legacy
        // seed; the returned record is Animate's
        // profile and the globals mirror it.
        let mut prefs = LayoutPrefs::default();
        let record = startup_layout(&mut prefs);
        assert_eq!(prefs.workspace_layouts.len(), 9);
        assert_eq!(record, default_layout_for(Workspace::Animate).unwrap());
        assert_eq!(
            record_for(&prefs, Workspace::Graph),
            default_layout_for(Workspace::Graph).unwrap()
        );
        assert_eq!(
            record_for(&prefs, Workspace::Export),
            default_layout_for(Workspace::Export).unwrap()
        );
        assert_eq!(record_for(&prefs, Workspace::Home), factory_layout());
        assert_eq!(prefs.col_left_w, 220.0);
        assert_eq!(prefs.col_right_w, 240.0);
    }

    #[test]
    fn layout_memory_seeding_protects_a_customised_active_workspace() {
        // A pre-memory user who arranged the single
        // global layout (380 / 460, sidebar hidden) and
        // was last in Graph: Graph — the workspace that
        // arrangement was in effect in — seeds from the
        // legacy globals, NOT its profile; every other
        // workspace takes its profile.
        let mut prefs = LayoutPrefs::default();
        prefs.workspace = "Graph".to_string();
        prefs.col_left_w = 380.0;
        prefs.col_right_w = 460.0;
        prefs.sidebar_visible = false;
        let record = startup_layout(&mut prefs);
        assert_eq!(record.col_left_w, 380.0);
        assert_eq!(record.col_right_w, 460.0);
        assert!(!record.show_left);
        assert_eq!(record_for(&prefs, Workspace::Graph), record);
        assert_eq!(
            record_for(&prefs, Workspace::Animate),
            default_layout_for(Workspace::Animate).unwrap()
        );
        // And a customised file whose active workspace
        // has NO profile (Home) changes nothing: Home
        // seeds legacy either way.
        let mut prefs = LayoutPrefs::default();
        prefs.workspace = "Home".to_string();
        prefs.col_left_w = 380.0;
        let record = startup_layout(&mut prefs);
        assert_eq!(record.col_left_w, 380.0);
    }

    #[test]
    fn stage_focus_enter_and_exit_move_visibility_only() {
        // Enter: both columns hide; widths and the
        // timeline flag ride through untouched.
        let live = live(260.0, 400.0, true, false, true);
        let focused = focus_enter(live);
        assert!(!focused.show_left && !focused.show_right);
        assert_eq!(focused.col_left_w, 260.0);
        assert_eq!(focused.col_right_w, 400.0);
        assert!(focused.timeline_collapsed);
        // Exit: the saved visibility returns — including
        // a column that was already hidden before focus
        // (show_right here) — over the live layout, so a
        // timeline change made during focus survives.
        let during = LiveLayout {
            timeline_collapsed: false,
            ..focused
        };
        let restored = focus_exit(live, during);
        assert!(restored.show_left);
        assert!(!restored.show_right);
        assert!(!restored.timeline_collapsed);
        assert_eq!(restored.col_left_w, 260.0);
    }

    #[test]
    fn stage_focus_capture_reads_the_restore_not_the_hidden_flags() {
        // The capture rule `live_layout` applies: with
        // no restore held, the live flags are the truth;
        // with one held, the restore's visibility is —
        // the hidden live flags must never be captured
        // as a workspace's remembered layout.
        assert_eq!(capture_visibility(true, false, None), (true, false));
        let saved = live(260.0, 400.0, true, false, false);
        assert_eq!(
            capture_visibility(false, false, Some(saved)),
            (true, false)
        );
    }
}
