//! Project tools section (Rune Phase D, section 34) — project
//! name field, New / Save-to-projects / Recover, and the saved
//! project selector. Bin-crate code, declared from `main.rs`
//! only; every control delegates to a real engine op or mirrors
//! a listener the GPUI app had, verbatim where one existed.
//!
//! Wiring the integrator must add (this file owns neither
//! `main.rs` nor the engine):
//! - `pub name_edit: TextEdit` on `State`, seeded in `init()`
//!   with `TextEdit::new(core.project.name.clone())` — the name
//!   field's edit state (Rune core `TextEdit`, audit Q5). The
//!   section re-syncs it after every project switch it performs
//!   (New / Open / Recover) via [`sync_name_edit`], and the
//!   integrator ALSO calls [`sync_name_edit`] from `poll()` on
//!   every tick, so switches this section does not perform —
//!   the app bar's Load, the Home panel's open, an MCP-driven
//!   rename — re-sync the field within a frame as well. See
//!   the fn's docs for the focused-field guard.
//! - a `mod` declaration mounting this file, and one
//!   `.child(project_tools::project_section(s, &t))` in the view.
//!
//! Save / Load of the working file (`project.styk.json`) already
//! live in the app bar (`chrome_view::app_bar`) and are NOT
//! duplicated here; this section covers the *named* projects in
//! `./projects/`.

use std::path::Path;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, text_input, El, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{Easing, FigureDef, FigureInstance, Frame, Project};

use crate::chrome_view::btn;
use crate::State;

// ---------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------

/// Clean a typed project name for assignment to
/// `Project::name`: surrounding whitespace is trimmed. An empty
/// or all-whitespace name is rejected (`None`) — the core never
/// stores an empty project name (named saves slugify the name
/// into a file name, where empty would be meaningless).
pub fn clean_project_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// The selector-row label for one saved project file: the file
/// stem (`my-film` for `./projects/my-film.json` — the slug is
/// the project name as saved). Falls back to the file name,
/// then to the whole path, for exotic entries.
pub fn project_file_label(path: &Path) -> String {
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if !stem.is_empty() {
            return stem.to_string();
        }
    }
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    path.display().to_string()
}

/// One saved-project selector row, derived from the core's own
/// file list (`EditorCore::project_files` / `project_index`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRow {
    pub label: String,
    /// True for the file the core's `project_index` points at —
    /// the one `open_project_file` would open without a row
    /// press changing the index first.
    pub selected: bool,
}

/// The selector rows for the core's current project file list,
/// in list order.
pub fn project_rows(core: &EditorCore) -> Vec<ProjectRow> {
    core.project_files
        .iter()
        .enumerate()
        .map(|(i, path)| ProjectRow {
            label: project_file_label(path),
            selected: i == core.project_index,
        })
        .collect()
}

/// A blank project with the codebase's established New-project
/// defaults: 960×720 stage, 12 fps, the dark stage background,
/// one rest-posed humanoid ("Styk") and a single frame. These
/// are the exact values of the MCP `new_project` tool
/// (`mcp_tools.rs`), the only New definition the codebase has —
/// the core exposes no `new_project` op of its own.
pub fn blank_project(name: &str) -> Project {
    let (width, height) = (960.0, 720.0);
    let def = FigureDef::humanoid();
    let root = [width / 2.0, height * 0.65];
    let pose = def.rest_pose(root);
    Project {
        name: name.to_string(),
        fps: 12.0,
        stage: [width, height],
        background: 0x232830,
        figures: vec![FigureInstance {
            name: "Styk".into(),
            color: 0xE8E8EC,
            def,
            hidden: false,
            clip: None,
            locked: false,
            opacity: 1.0,
        }],
        frames: vec![Frame {
            poses: vec![pose],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }],
        camera: Vec::new(),
        tween_playback: false,
        easing: Easing::Linear,
        camera_follow: None,
        particles: Vec::new(),
        strokes: Vec::new(),
        audio: Vec::new(),
        clips: Vec::new(),
        fx_clips: Vec::new(),
        assets: Vec::new(),
        channels: Vec::new(),
        sprites: Vec::new(),
        texts: Vec::new(),
        weather: None,
    }
}

/// The autosave status line: how many snapshot-counted edits
/// have accumulated since the last recovery snapshot was
/// written (`EditorCore::edits_since_autosave`). Display only —
/// autosave itself is driven by the core's edit cadence
/// (`maybe_autosave`), not by this section.
pub fn autosave_line(edits_since_autosave: u64) -> String {
    match edits_since_autosave {
        0 => "Recovery snapshot up to date.".to_string(),
        1 => "1 edit since the last recovery snapshot.".to_string(),
        n => format!("{n} edits since the last recovery snapshot."),
    }
}

// ---------------------------------------------------------------------
// Core-level actions (handlers below are thin State wrappers)
// ---------------------------------------------------------------------

/// Set the project name from typed text (`Project::name` is a
/// plain field; the core has no rename op — the on-disk file
/// rename is `projects::rename_project`, a library function the
/// GPUI app drove from its Home panel, not from a name field).
/// Returns false and explains in the status line when the name
/// is empty.
pub fn apply_project_name(core: &mut EditorCore, raw: &str) -> bool {
    match clean_project_name(raw) {
        Some(name) => {
            core.project.name = name.clone();
            core.status = format!("Project name set to \"{name}\".");
            true
        }
        None => {
            core.status = "Project name cannot be empty.".into();
            false
        }
    }
}

/// New project, mirroring the MCP `new_project` tool's
/// semantics exactly (the core's own New behaviour): the
/// project is replaced in place and the selection/playback
/// fields reset — with **no confirmation prompt and no undo
/// snapshot**, so unsaved work is dropped the moment this is
/// pressed, exactly as the tool drops it. Undo history is left
/// untouched, as the tool leaves it.
pub fn new_project(core: &mut EditorCore) {
    core.project = blank_project("Untitled");
    core.current = 0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
    core.playing = false;
    core.status = "New project.".into();
}

/// Apply a recovered project: the state reset the GPUI app's
/// Recover button performed after `shell::read_recovery`
/// returned a snapshot (app.rs, pre-removal).
pub fn apply_recovered_project(core: &mut EditorCore, project: Project) {
    core.project = project;
    core.current = 0;
    core.playhead = 0.0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.status = "Recovered the snapshot from ./projects/.recovery.json.".into();
}

/// Recover the autosave snapshot (`./projects/.recovery.json`,
/// via `shell::read_recovery`) — the GPUI Recover button's
/// behaviour, verbatim. Returns whether a snapshot was applied.
pub fn recover_snapshot(core: &mut EditorCore) -> bool {
    match stykanimate_rs::shell::read_recovery() {
        Some(project) => {
            apply_recovered_project(core, project);
            true
        }
        None => {
            core.status = "No recovery snapshot found in ./projects/.".into();
            false
        }
    }
}

// ---------------------------------------------------------------------
// State-level handlers
// ---------------------------------------------------------------------

/// Sync the name field with the open project's name.
///
/// The name field owns its own edit buffer (`State::name_edit`),
/// so anything that changes `Project::name` behind the field's
/// back leaves the field showing a stale name: the app bar's
/// Load, the Home panel's open, an MCP-driven rename — none of
/// them know this field exists. This section's own switches
/// (New / Open / Recover) call this fn from their handlers, but
/// the fix for every other path is polling: **the integrator
/// calls this in `main.rs`'s `poll()` on every tick.** The
/// check is one string compare, so an idle tick costs nothing,
/// and any project switch — Load, Open, Recover, New, or a
/// rename arriving over MCP — re-syncs the field within a
/// frame of the name changing.
///
/// Behaviour, precisely:
///
/// - Field already shows the project name → no-op (the caret
///   and selection are left exactly where they are).
/// - Field differs and is NOT focused → the whole buffer is
///   replaced via `TextEdit::replace_text` (rune `edit.rs`:
///   replaces the draft, discards its undo history, moves the
///   caret to the end of the new text, and preserves the
///   `focused` flag — this fn adds no selection/focus side
///   effects of its own beyond exactly what `replace_text`
///   itself does).
/// - Field differs and IS focused → **skip**. A focused field
///   whose text differs from the project name is a user
///   mid-typing a new name; replacing the buffer would eat
///   their keystrokes. Their text wins until they submit
///   (Enter / "Set name" applies it via [`apply_name`]) or
///   leave the field, at which point the next poll tick syncs
///   normally. The cost of this choice: a project switch that
///   lands while the user is typing in the name field leaves
///   the field stale until they stop editing — deliberate;
///   the alternative destroys in-progress typing.
pub fn sync_name_edit(s: &mut State) {
    if s.name_edit.focused {
        return;
    }
    let name = s.core.project.name.clone();
    if s.name_edit.text != name {
        s.name_edit.replace_text(name);
    }
}

/// "Set name" (and Enter in the field): apply the field's text.
fn apply_name(s: &mut State) {
    let raw = s.name_edit.text.clone();
    apply_project_name(&mut s.core, &raw);
}

/// "New": [`new_project`], then re-sync the name field.
fn new_pressed(s: &mut State) {
    new_project(&mut s.core);
    sync_name_edit(s);
}

/// "Save project": the GPUI `save-proj` listener verbatim —
/// save under the project's name in `./projects/`, then keep
/// the recovery snapshot and recents list in step with the
/// explicit save.
fn save_named_pressed(s: &mut State) {
    s.core.save_named_project();
    let _ = stykanimate_rs::shell::write_recovery(&s.core.project);
    stykanimate_rs::shell::touch_recent(&s.core.project.name);
}

/// One selector row's "Open": point the core's index at the
/// row and open that file (`open_project_file` reads
/// `project_index`), then re-sync the name field.
fn open_row_pressed(s: &mut State, i: usize) {
    s.core.project_index = i;
    s.core.open_project_file();
    sync_name_edit(s);
}

/// "Recover": [`recover_snapshot`], then re-sync the name field.
fn recover_pressed(s: &mut State) {
    recover_snapshot(&mut s.core);
    sync_name_edit(s);
}

// ---------------------------------------------------------------------
// View
// ---------------------------------------------------------------------

/// The Project section: name field + Set name, New / Save
/// project / Recover, the autosave line, and one Open row per
/// saved project in the core's file list.
pub fn project_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let rows = project_rows(core);

    let mut section = col()
        .key("project-tools")
        .w_full()
        .gap(8.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(text("Project").font(type_::TITLE).color(t.text))
                .child(el().grow(1.0))
                .child(
                    text(autosave_line(core.edits_since_autosave()))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    el()
                        .key("proj-name-field")
                        .grow(1.0)
                        .h(32.0)
                        .px(8.0)
                        .radius(6.0)
                        .fill(t.card)
                        .border(1.0, t.card_border)
                        .center_y()
                        .child(
                            text_input(&s.name_edit, type_::BODY)
                                .key("proj-name")
                                .single_line()
                                .color(t.text)
                                .on_edit(|s: &mut State| &mut s.name_edit)
                                .on_submit(|s: &mut State, _tx: &mut Tx| apply_name(s)),
                        ),
                )
                .child(btn("proj-set-name", "Set name", false, t, apply_name)),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("new-proj", "New", false, t, new_pressed))
                .child(btn("save-proj", "Save project", false, t, save_named_pressed))
                .child(btn("recover", "Recover", false, t, recover_pressed)),
        );

    if rows.is_empty() {
        // The core's own wording (cycle_project_file).
        section = section.child(
            text("No saved projects in ./projects/ yet.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        let mut list = col().key("proj-list").w_full().gap(4.0);
        for (i, row_data) in rows.iter().enumerate() {
            list = list.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        text(row_data.label.clone())
                            .font(type_::LABEL)
                            .color(t.text)
                            .grow(1.0),
                    )
                    .child(btn(
                        &format!("open-proj-{i}"),
                        "Open",
                        row_data.selected,
                        t,
                        move |s: &mut State| open_row_pressed(s, i),
                    )),
            );
        }
        section = section.child(list);
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A shell `State` around `core`, built the way the other
    /// sections' tests build one (every section field at its
    /// default; only `core` and `name_edit` matter here).
    fn test_state(core: EditorCore) -> State {
        State {
            core,
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
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            was_playing: false,
        }
    }

    #[test]
    fn clean_name_trims_and_rejects_empty() {
        assert_eq!(clean_project_name("  My Film  "), Some("My Film".to_string()));
        assert_eq!(clean_project_name("Untitled"), Some("Untitled".to_string()));
        assert_eq!(clean_project_name(""), None);
        assert_eq!(clean_project_name("   \t "), None);
        // Inner spacing is the user's; only the ends are trimmed.
        assert_eq!(clean_project_name("a  b"), Some("a  b".to_string()));
    }

    #[test]
    fn file_label_uses_stem_then_name() {
        assert_eq!(
            project_file_label(Path::new("./projects/my-film.json")),
            "my-film"
        );
        assert_eq!(project_file_label(Path::new("notes.txt")), "notes");
        assert_eq!(project_file_label(Path::new("plain")), "plain");
    }

    #[test]
    fn blank_project_matches_new_defaults() {
        let p = blank_project("Agent Film");
        assert_eq!(p.name, "Agent Film");
        assert_eq!(p.fps, 12.0);
        assert_eq!(p.stage, [960.0, 720.0]);
        assert_eq!(p.background, 0x232830);
        assert_eq!(p.figures.len(), 1);
        assert_eq!(p.figures[0].name, "Styk");
        assert_eq!(p.frames.len(), 1);
        assert_eq!(p.frames[0].poses.len(), 1);
        assert!(p.camera.is_empty() && p.audio.is_empty() && p.clips.is_empty());
    }

    #[test]
    fn autosave_line_pluralises() {
        assert_eq!(autosave_line(0), "Recovery snapshot up to date.");
        assert_eq!(autosave_line(1), "1 edit since the last recovery snapshot.");
        assert_eq!(autosave_line(7), "7 edits since the last recovery snapshot.");
    }

    #[test]
    fn apply_name_sets_field_and_rejects_empty() {
        let mut core = EditorCore::new();
        assert!(apply_project_name(&mut core, "  Renamed Film "));
        assert_eq!(core.project.name, "Renamed Film");
        assert!(!apply_project_name(&mut core, "   "));
        assert_eq!(core.project.name, "Renamed Film");
        assert!(core.status.contains("cannot be empty"));
    }

    #[test]
    fn new_project_replaces_and_resets() {
        let mut core = EditorCore::new();
        core.current = 2;
        core.playing = true;
        core.project.name = "Old Work".into();
        new_project(&mut core);
        assert_eq!(core.project.name, "Untitled");
        assert_eq!(core.project.frames.len(), 1);
        assert_eq!(core.current, 0);
        assert!(!core.playing);
        assert_eq!(core.selected_figure, 0);
        assert!(core.selected_node.is_none());
        assert_eq!(core.status, "New project.");
    }

    #[test]
    fn recovered_project_resets_view_state() {
        let mut core = EditorCore::new();
        core.current = 2;
        let mut snapshot = Project::new_demo();
        snapshot.name = "Recovered Work".into();
        apply_recovered_project(&mut core, snapshot);
        assert_eq!(core.project.name, "Recovered Work");
        assert_eq!(core.current, 0);
        assert_eq!(core.playhead, 0.0);
        assert!(core.status.contains("Recovered the snapshot"));
    }

    #[test]
    fn rows_follow_core_index_and_labels() {
        let mut core = EditorCore::new();
        core.project_files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/beta-cut.json"),
        ];
        core.project_index = 1;
        let rows = project_rows(&core);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "alpha");
        assert!(!rows[0].selected);
        assert_eq!(rows[1].label, "beta-cut");
        assert!(rows[1].selected);
    }

    #[test]
    fn rows_empty_when_no_files() {
        let mut core = EditorCore::new();
        core.project_files = Vec::new();
        assert!(project_rows(&core).is_empty());
    }

    #[test]
    fn sync_replaces_stale_text_when_unfocused() {
        let mut core = EditorCore::new();
        core.project.name = "Loaded Project".into();
        let mut s = test_state(core);
        s.name_edit.replace_text("Stale Name");
        assert!(!s.name_edit.focused);
        sync_name_edit(&mut s);
        assert_eq!(s.name_edit.text, "Loaded Project");
        // replace_text's own documented side effect: the caret
        // lands at the end of the replacement text.
        assert_eq!(s.name_edit.focus, "Loaded Project".len());
        assert_eq!(s.name_edit.anchor, "Loaded Project".len());
    }

    #[test]
    fn sync_is_a_no_op_when_text_matches() {
        let mut core = EditorCore::new();
        core.project.name = "Same Name".into();
        let mut s = test_state(core);
        s.name_edit.replace_text("Same Name");
        // Park the caret where replace_text would NOT leave
        // it: if the sync (wrongly) re-applied replace_text,
        // the caret would snap back to the end.
        s.name_edit.focus = 0;
        s.name_edit.anchor = 0;
        sync_name_edit(&mut s);
        assert_eq!(s.name_edit.text, "Same Name");
        assert_eq!(s.name_edit.focus, 0);
        assert_eq!(s.name_edit.anchor, 0);
    }

    #[test]
    fn sync_skips_while_the_field_is_focused() {
        let mut core = EditorCore::new();
        core.project.name = "Other Project".into();
        let mut s = test_state(core);
        s.name_edit.replace_text("Half-typed na");
        s.name_edit.focused = true;
        sync_name_edit(&mut s);
        // The user's in-progress typing is untouched.
        assert_eq!(s.name_edit.text, "Half-typed na");
        // Once the field loses focus, the next sync lands
        // normally — the guard defers, it does not disable.
        s.name_edit.focused = false;
        sync_name_edit(&mut s);
        assert_eq!(s.name_edit.text, "Other Project");
    }
}
