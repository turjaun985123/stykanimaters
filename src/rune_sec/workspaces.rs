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
//! renders in **all eight workspaces**, and no
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
use stykanimate_rs::shell::{load_layout, save_layout, LayoutPrefs};

use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::icon_btn;
use crate::State;

/// One of the eight workspaces. The active workspace
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
/// Figures, Library, Layers) and the right column
/// (inspectors, pose/keyframe tools, camera, scene,
/// onion, playback, frame ops, graph, easing, draw,
/// stroke inspector, text, FX browser/clips, audio,
/// waveform, movieclips, sprites, 3D, export, stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Figures,
    Library,
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
    /// All eight workspaces, in chip-row order.
    pub fn all() -> [Workspace; 8] {
        [
            Workspace::Animate,
            Workspace::Draw,
            Workspace::Graph,
            Workspace::Effects,
            Workspace::Audio,
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
    /// "3D"; the merged-away Build workspace's `ws_build`
    /// key has no chip of its own anymore.
    pub fn i18n_key(self) -> &'static str {
        match self {
            Workspace::Animate => "ws_animate",
            Workspace::Draw => "ws_draw",
            Workspace::Graph => "ws_graph",
            Workspace::Effects => "ws_effects",
            Workspace::Audio => "ws_audio",
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
/// `LayoutPrefs.workspace`). Unknown strings — including
/// `"Build"`, the GPUI workspace that merged into
/// [`Workspace::Build3D`] and has no persisted name of
/// its own anymore — fall back to
/// [`Workspace::Animate`], the shell default.
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

/// Switch the shell's workspace and persist the choice:
/// sets `s.workspace`, then load-modify-saves
/// `LayoutPrefs` with the new workspace name, preserving
/// every other field (theme, timeline, sidebar) — the
/// same pattern `main.rs`'s `poll` uses for theme flips.
pub fn select_workspace(s: &mut State, ws: Workspace) {
    s.workspace = ws;
    let prefs = prefs_with_workspace(&load_layout(), ws);
    save_layout(&prefs);
}

impl Section {
    /// All 30 gated sections (coverage tests iterate
    /// this; the integrator's view maps each mounted
    /// panel to one variant).
    pub fn all() -> [Section; 30] {
        [
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
/// quiet suffix — `Animate · Alt+1` … `Home · Alt+8`,
/// matching the `Alt+1..8` bindings in `main.rs`'s
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
    fn all_eight_workspaces_in_row_order() {
        assert_eq!(
            Workspace::all(),
            [
                Workspace::Animate,
                Workspace::Draw,
                Workspace::Graph,
                Workspace::Effects,
                Workspace::Audio,
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
        // The GPUI "Build" workspace merged into
        // Build3D; its old persisted string has no
        // exact match anymore and falls back too.
        assert_eq!(from_name("Build"), Workspace::Animate);
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
        // clamps (220..=420 / 260..=480) so the disk
        // round-trip test below sees them as written.
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
            col_left_w: 260.0,
            col_right_w: 400.0,
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
        // change: 30 sections in `Section::all()`, and
        // the per-workspace list lengths below (43 list
        // memberships in total — sections legitimately
        // repeat ACROSS workspaces; the no-duplicates
        // test pins them unique WITHIN one list). Wave
        // 13: Animate 16 → 17 for `Section::Sprites`
        // (Animate-only; see the Wave 13 note above the
        // table).
        use Workspace as W;
        assert_eq!(Section::all().len(), 30);
        assert_eq!(sections_of(W::Animate).len(), 17);
        assert_eq!(sections_of(W::Draw).len(), 4);
        assert_eq!(sections_of(W::Graph).len(), 4);
        assert_eq!(sections_of(W::Effects).len(), 5);
        assert_eq!(sections_of(W::Audio).len(), 4);
        assert_eq!(sections_of(W::Build3D).len(), 3);
        assert_eq!(sections_of(W::Export).len(), 3);
        assert_eq!(sections_of(W::Home).len(), 3);
        let total: usize = Workspace::all()
            .into_iter()
            .map(|ws| sections_of(ws).len())
            .sum();
        assert_eq!(total, 43);
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
        for ws in [W::Animate, W::Draw, W::Graph, W::Effects, W::Audio, W::Build3D] {
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
        for ws in [W::Draw, W::Graph, W::Effects, W::Audio, W::Build3D, W::Export, W::Home] {
            assert!(!section_visible(ws, S::Sprites), "{ws:?} must not mount Sprites");
        }
    }
}
