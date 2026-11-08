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
//! figure-add toggle (part of the Figures area's chrome),
//! the settings popover, the help panel, and the shell's
//! permanent furniture (app bar, menus, stage, camera
//! overlay strip, transport, timeline, status strip,
//! toasts) — those stay available in every workspace,
//! exactly as the GPUI app kept Transport/Home visible in
//! several of its workspaces.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::shell::{load_layout, save_layout, LayoutPrefs};

use crate::chrome_view::btn;
use crate::rune_sec::design;
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
/// waveform, movieclips, 3D, export, stats).
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
    /// All 29 gated sections (coverage tests iterate
    /// this; the integrator's view maps each mounted
    /// panel to one variant).
    pub fn all() -> [Section; 29] {
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

/// Animate: the core animating set — figures, library,
/// layers, project, all three inspectors, pose and
/// keyframe tools, camera, scene, onion, playback,
/// frame ops, movieclips and stats. Movieclips ride
/// here (not Effects) on GPUI precedent: its
/// "Library & Clips" group was an Animate/Build group,
/// and Effects' clips are the FX clips.
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
pub fn workspace_row(s: &State, t: &Theme) -> El {
    let mut chips = row()
        .key("workspace-chips")
        .gap(design::PANEL_GAP)
        .center_y();
    for (i, ws) in Workspace::all().into_iter().enumerate() {
        let chip_label = format!("{} · Alt+{}", ws.label(), i + 1);
        chips = chips.child(btn(
            ws.key(),
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
        let prefs = LayoutPrefs {
            workspace: "Animate".to_string(),
            dark: false,
            timeline_collapsed: true,
            sidebar_visible: false,
            palette_recents: Vec::new(),
        };
        let out = prefs_with_workspace(&prefs, Workspace::Audio);
        assert_eq!(out.workspace, "Audio");
        assert!(!out.dark);
        assert!(out.timeline_collapsed);
        assert!(!out.sidebar_visible);
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
    }
}
